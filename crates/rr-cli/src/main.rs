//! Headless tools: `rr-cli mapinfo <file.wld|swd>`.

use rr_core::{MapObject, Resource};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["mapinfo", path] => mapinfo(path),
        ["convert", from, to] => convert(from, to),
        ["soak", rest @ ..] => soak(rest),
        ["ladder", rest @ ..] => ladder(rest),
        ["fairness", path, rest @ ..] => fairness(path, rest),
        _ => {
            eprintln!("usage: rr-cli mapinfo <map.wld|map.swd>");
            eprintln!("       rr-cli convert <in.rrmap|in.wld|in.swd> <out.rrmap|out.swd>");
            eprintln!(
                "       rr-cli soak [--seed N] [--players N] [--size N] [--gf N] [--map FILE|islands | --mission KEY [--level L]] [--newcomer 1] [--campaign 1] [--audit 1] [--rules FILE] [--peace MINUTES]"
            );
            eprintln!(
                "       rr-cli ladder [--a easy|normal|hard] [--b easy|normal|hard] [--seeds N] [--gf N] [--map FILE]"
            );
            eprintln!("       rr-cli fairness <map> [--level easy|normal|hard] [--seeds N] [--gf N]");
            ExitCode::from(2)
        }
    }
}

fn level(s: &str) -> rr_ai::Level {
    match s {
        "easy" => rr_ai::Level::Easy,
        "hard" => rr_ai::Level::Hard,
        _ => rr_ai::Level::Normal,
    }
}

/// AI ladder (PLAN §23.8): level A against level B, one on one, on random maps; swaps sides
/// every other seed. Scores by defeat, else by land after `gf` frames.
fn ladder(args: &[&str]) -> ExitCode {
    let (mut a, mut b, mut seeds, mut gf) = (rr_ai::Level::Hard, rr_ai::Level::Normal, 10u64, 60_000u32);
    let mut file: Option<rr_core::MapData> = None;
    let mut rules = std::sync::Arc::new(rr_rules::Rules::classic());
    let mut it = args.iter();
    while let Some(k) = it.next() {
        let v = it.next().copied().unwrap_or("");
        match *k {
            "--a" => a = level(v),
            "--b" => b = level(v),
            "--seeds" => seeds = v.parse().unwrap_or(10),
            "--gf" => gf = v.parse().unwrap_or(60_000),
            // a map of the user's own (two players), the same map every seed (map maker round 1, #8)
            "--map" => file = Some(read_map(v)),
            "--rules" => rules = read_rules(v),
            other => fail(&format!("ladder: unknown option {other}")),
        }
    }
    if file.as_ref().is_some_and(|m| m.players() != 2) {
        fail("ladder --map: the map must have two players (rr-cli fairness judges more)");
    }
    let (mut wins_a, mut wins_b) = (0, 0);
    for seed in 1..=seeds {
        let map = file.clone().unwrap_or_else(|| {
            rr_mapgen::generate(&rr_mapgen::GenParams { width: 96, height: 96, players: 2, seed, ..Default::default() })
        });
        let setup =
            rr_sim::GameSetup { seed, players: vec![Default::default(), Default::default()], ..Default::default() };
        let mut sim = rr_sim::Sim::new(rules.clone(), &map, &setup)
            .unwrap_or_else(|e| fail(&format!("this map can't be played: {e:?}")));
        // A plays player 0 on odd seeds, player 1 on even ones
        let a_side = (seed % 2 == 0) as u8;
        let mut ais = [rr_ai::Ai::with_level(a_side, a), rr_ai::Ai::with_level(1 - a_side, b)];
        let mut kinds: [std::collections::BTreeMap<String, u32>; 2] = Default::default();
        for _ in 0..gf {
            let mut cmds = Vec::new();
            for ai in ais.iter_mut() {
                cmds.extend(ai.think(&sim));
            }
            for c in &cmds {
                let k = format!("{:?}", c.cmd);
                let side = (c.player != a_side) as usize;
                *kinds[side].entry(k.split([' ', '{']).next().unwrap_or("").to_string()).or_default() += 1;
            }
            sim.step(&cmds);
            if sim.players().iter().filter(|p| p.alive).count() < 2 {
                break;
            }
        }
        let land = sim.land_by_player();
        let (la, lb) = (land[a_side as usize], land[1 - a_side as usize]);
        let a_won = !sim.players()[1 - a_side as usize].alive || sim.players()[a_side as usize].alive && la > lb;
        if a_won {
            wins_a += 1;
        } else {
            wins_b += 1;
        }
        if std::env::var_os("RR_LADDER_CMDS").is_some() {
            println!("  A {:?}\n  B {:?}", kinds[0], kinds[1]);
        }
        println!(
            "seed {seed}: A {la} land{} / B {lb} land{} -> {}",
            if sim.players()[a_side as usize].alive { "" } else { " (defeated)" },
            if sim.players()[1 - a_side as usize].alive { "" } else { " (defeated)" },
            if a_won { "A" } else { "B" }
        );
    }
    println!("{a:?} (A) {wins_a} : {wins_b} {b:?} (B)");
    ExitCode::SUCCESS
}

/// A campaign mission's key, or the run ends with the list (QA round 9, #7: an unknown key was
/// taken as a file name).
fn mission_key(v: &str) -> &str {
    if rr_game::missions::mission(v).is_none() {
        let keys = rr_game::missions::MISSIONS.iter().map(|m| m.key).collect::<Vec<_>>().join(", ");
        fail(&format!("soak: no mission {v:?} (missions: {keys})"));
    }
    v
}

/// What a soak plays: a map file, a region or mission, or a random map; and who plays it.
struct SoakGame<'a> {
    map_path: Option<&'a str>,
    seed: u64,
    players: usize,
    size: u16,
    newcomer: bool,
    campaign: bool,
    peace: u32,
    level: Option<rr_ai::Level>,
}

/// A soak's game and its computer players.
fn soak_game(g: &SoakGame, rules: std::sync::Arc<rr_rules::Rules>) -> (rr_sim::Sim, Vec<rr_ai::Ai>) {
    let SoakGame { map_path, seed, players, size, newcomer, campaign, peace, level } = *g;
    // a campaign mission is played as the app sets it up (architect round 3, #4: the newcomer
    // eval played five allied rivals where the app's Europe has two, and three at peace)
    let mission = map_path.and_then(rr_game::missions::mission);
    if level.is_some() && mission.is_none() {
        fail("soak: --level is a mission's level (with --mission)");
    }
    let level = level.unwrap_or(if newcomer { rr_ai::Level::Easy } else { rr_ai::Level::Normal });
    let map = match map_path {
        _ if mission.is_some() => rr_game::mission_map(mission.expect("mission"), seed),
        Some(key) if rr_mapgen::world::region(key).is_some() => {
            rr_mapgen::region::generate(rr_mapgen::world::region(key).expect("region"), seed)
        }
        // an archipelago of the free game's kind (`--map islands`)
        Some("islands") => rr_mapgen::generate(&rr_mapgen::GenParams {
            width: size,
            height: size,
            players,
            seed,
            islands: true,
            ..Default::default()
        }),
        // (a message and exit 1, not a panic: map maker round 1, #6)
        Some(p) => read_map(p),
        None => rr_mapgen::generate(&rr_mapgen::GenParams {
            width: size,
            height: size,
            players,
            seed,
            ..Default::default()
        }),
    };
    let setup = rr_sim::GameSetup {
        seed,
        players: (0..map.players())
            .map(|i| rr_sim::PlayerSetup { name: format!("AI {i}"), ..Default::default() })
            .collect(),
        ..Default::default()
    };
    let mut setup = setup;
    if let Some(m) = mission {
        // the mission's teams at this level; computer players at it, the truce only with a
        // newcomer to spare (an all-computer mission is the app's --auto: no truce)
        let teams = rr_game::mission_players(m, level);
        for (p, t) in setup.players.iter_mut().zip(teams) {
            (p.nation, p.team) = (t.1, Some(t.2));
        }
    } else if newcomer || campaign {
        // the rivals allied against player 0
        let teams = rr_sim::GameSetup::campaign(seed, map.players()).players;
        for (p, t) in setup.players.iter_mut().zip(teams) {
            p.team = t.team;
        }
    }
    let sim =
        rr_sim::Sim::new(rules, &map, &setup).unwrap_or_else(|e| fail(&format!("this map can't be played: {e:?}")));
    let mut ais: Vec<rr_ai::Ai> = (0..sim.players().len() as u8)
        .map(|p| if mission.is_some() { rr_ai::Ai::with_level(p, level) } else { rr_ai::Ai::new(p) })
        .collect();
    for ai in &mut ais {
        ai.peace_until = peace;
    }
    if newcomer {
        // player 0 plays like a newcomer: an easy computer player that keeps the truce
        ais[0] = rr_ai::Ai::with_level(0, rr_ai::Level::Easy);
        match mission {
            Some(m) => rr_game::arm_mission(m, level, &mut ais[1..]),
            None => {
                for ai in &mut ais[1..] {
                    ai.truce = Some((0, rr_ai::MISSION_TRUCE));
                }
            }
        }
    }
    // the cast as the app has it: each slot's team and truce with player 0
    if let Some(m) = mission {
        let slot = |p: usize| {
            let team = sim.players()[p].setup.team.unwrap_or(0);
            match ais[p].truce.map(|t| rr_game::Truce::of(t.1)) {
                Some(rr_game::Truce::ForGood) => format!("p{p} team {team} at peace"),
                Some(rr_game::Truce::Until(gf)) => format!("p{p} team {team} truce {} min", gf / rr_game::MINUTE),
                None => format!("p{p} team {team}"),
            }
        };
        let cast: Vec<String> = (0..ais.len()).map(slot).collect();
        println!("mission {} at {level:?}: {}", m.key, cast.join(", "));
    }
    (sim, ais)
}

/// Run AIs against each other headless and print the economy every 5000 GF.
#[allow(clippy::too_many_lines)] // to split (architect round 2, #8: no new long ones)
fn soak(args: &[&str]) -> ExitCode {
    let (mut seed, mut players) = (1u64, 2usize);
    let mut size = 96u16;
    let mut gf = 30_000u32;
    let mut map_path: Option<&str> = None;
    let mut newcomer = false;
    let mut campaign = false;
    let mut audit = false;
    let mut rules = std::sync::Arc::new(rr_rules::Rules::classic());
    let mut peace = 0u32;
    let mut level = None;
    let mut it = args.iter();
    while let Some(k) = it.next() {
        let v = it.next().copied().unwrap_or("");
        match *k {
            "--seed" => seed = v.parse().unwrap_or(1),
            "--players" => players = v.parse().unwrap_or(2),
            "--size" => size = v.parse().unwrap_or(96),
            "--gf" => gf = v.parse().unwrap_or(30_000),
            "--map" => map_path = Some(v),
            "--mission" => map_path = Some(mission_key(v)),
            "--europe" => {
                map_path = Some("europe");
                let _ = v;
            }
            // player 0 plays like a newcomer (easy AI); the others keep the mission truce
            "--newcomer" => newcomer = v != "0",
            // the rivals allied against player 0, as in the campaign
            "--campaign" => campaign = v != "0",
            // check the bookkeeping every 500 frames; fail on the first broken rule
            "--audit" => audit = v != "0",
            "--rules" => rules = read_rules(v),
            // nobody attacks for this many minutes (the free game's peace time)
            "--peace" => peace = v.parse::<u32>().unwrap_or(0).saturating_mul(60 * 20),
            // a campaign mission's level (its cast and truce); with --newcomer easy, else normal
            "--level" => {
                level = Some(match v {
                    "easy" | "0" => rr_ai::Level::Easy,
                    "normal" | "1" => rr_ai::Level::Normal,
                    "hard" | "2" => rr_ai::Level::Hard,
                    _ => fail(&format!("soak: --level {v}: easy, normal or hard")),
                })
            }
            // (a typo was swallowed silently: map maker round 1, #8)
            other => fail(&format!("soak: unknown option {other}")),
        }
    }
    let g = SoakGame { map_path, seed, players, size, newcomer, campaign, peace, level };
    let (mut sim, mut ais) = soak_game(&g, rules);
    // the newcomer stand-in keeps the truce as a player would: its attacks during it are dropped
    // (new-player round 8, #7: it took 3 buildings and lost 22 soldiers before minute 60)
    // (per rival: a peaceful one's truce lasts for good, a hostile one's ends)
    let truce_of: Vec<u32> = ais.iter().map(|a| a.truce.filter(|t| t.0 == 0).map_or(0, |t| t.1)).collect();
    let (mut burned0, mut captured0, mut attacks0) = (0u32, 0u32, 0u32);
    let t0 = std::time::Instant::now();
    let mut ai_time = std::time::Duration::ZERO;
    // the slowest single frame of the AIs and of the simulation (stalls, not just totals)
    let (mut ai_max, mut sim_max) = (std::time::Duration::ZERO, (std::time::Duration::ZERO, 0));
    let mut rejected = 0u32;
    let mut by_kind: std::collections::BTreeMap<String, u32> = Default::default();
    for i in 1..=gf {
        if audit
            && i % 500 == 0
            && let Err(e) = sim.audit()
        {
            eprintln!("audit failed at gf {}: {e}", sim.gf().0);
            std::process::exit(1);
        }
        let mut cmds = Vec::new();
        let ta = std::time::Instant::now();
        for ai in ais.iter_mut() {
            cmds.extend(ai.think(&sim));
        }
        ai_time += ta.elapsed();
        ai_max = ai_max.max(ta.elapsed());
        // the peace holds every attack and catapult back, as in the game
        let gf = sim.gf().0;
        cmds.retain(|c| !rr_game::breaks_peace(&c.cmd, gf, peace));
        if newcomer {
            let gf = sim.gf().0;
            let spared = |at: rr_core::MapCoord| {
                sim.building_at(at)
                    .is_some_and(|b| truce_of.get(sim.building(b).player as usize).is_some_and(|u| gf < *u))
            };
            cmds.retain(|c| !(c.player == 0 && matches!(c.cmd, rr_sim::Cmd::Attack { target, .. } if spared(target))));
            attacks0 +=
                cmds.iter().filter(|c| c.player == 0 && matches!(c.cmd, rr_sim::Cmd::Attack { .. })).count() as u32;
        }
        for c in &cmds {
            let k = format!("{:?}", c.cmd);
            *by_kind.entry(k.split([' ', '{']).next().unwrap_or("").to_string()).or_default() += 1;
        }
        let ts = std::time::Instant::now();
        let out = sim.step(&cmds);
        sim_max = sim_max.max((ts.elapsed(), sim.gf().0 - 1));
        rejected += out.events.iter().filter(|e| matches!(e, rr_sim::Event::CommandRejected { .. })).count() as u32;
        for e in &out.events {
            if let rr_sim::Event::PlayerDefeated { player } = e {
                println!("gf {i}: player {player} defeated");
            }
            if let rr_sim::Event::CommandRejected { cmd, reason, .. } = e {
                let k = format!("{cmd:?}");
                *by_kind
                    .entry(format!("rejected:{}:{reason:?}", k.split([' ', '{']).next().unwrap_or("")))
                    .or_default() += 1;
            }
            match e {
                rr_sim::Event::BuildingLost { player: 0, .. } => burned0 += 1,
                rr_sim::Event::BuildingCaptured { player: 0, .. } => captured0 += 1,
                _ => {}
            }
            let k = match e {
                rr_sim::Event::BuildingCaptured { .. } => "captured",
                rr_sim::Event::BuildingLost { .. } => "lost",
                rr_sim::Event::UnderAttack { .. } => "attacked",
                rr_sim::Event::Sound { sound: rr_sim::SoundCue::Sword, .. } => "duel",
                rr_sim::Event::Sound { sound: rr_sim::SoundCue::Death, .. } => "death",
                _ => continue,
            };
            *by_kind.entry(format!("ev:{k}")).or_default() += 1;
        }
        // the newcomer's standing every quarter of an hour
        if newcomer && i % 18_000 == 0 {
            let land = sim.land_by_player()[0];
            let mil = sim
                .buildings()
                .iter()
                .filter(|(_, b)| b.player == 0 && b.is_ready() && rr_sim::is_frontier_military(b.kind))
                .count();
            // (the castle by name: its loss hid among "9 burned"; new-player round 10, #1)
            let castle =
                sim.buildings().iter().any(|(_, b)| b.player == 0 && b.kind == rr_rules::BuildingType::Headquarters);
            println!(
                "newcomer at {} min: land {land}, military buildings {mil}, lost {burned0} burned and {captured0} captured, its attacks {attacks0}, castle {}, alive {}",
                i / 1200,
                if castle { "held" } else { "LOST" },
                sim.players()[0].alive
            );
        }
        if i % 5000 == 0 || i == gf {
            if std::env::var_os("RR_DEBUG_SHIPYARD").is_some() {
                debug_shipyards(&sim);
            }
            // figures whose work motion ended long ago with nothing scheduled are stuck for good
            let frozen = sim
                .figures()
                .iter()
                .filter(|(_, f)| {
                    f.ev.is_none() && matches!(f.motion, rr_sim::Motion::Work { start, dur } if i > start + dur + 100)
                })
                .count();
            println!(
                "gf {i} ({:.1} s, AI {:.1} s, slowest frame sim {:.1} ms (gf {}) AI {:.1} ms, {} figures ({} animals), {} wares, rejected {rejected}, frozen {frozen})",
                t0.elapsed().as_secs_f32(),
                ai_time.as_secs_f32(),
                sim_max.0.as_secs_f64() * 1000.0,
                sim_max.1,
                ai_max.as_secs_f64() * 1000.0,
                sim.figures().len(),
                sim.figures().iter().filter(|(_, f)| matches!(f.kind, rr_sim::FigKind::Animal { .. })).count(),
                sim.wares().len()
            );
            for p in 0..sim.players().len() as u8 {
                let inv = sim.player_inventory(p);
                let blds = sim.buildings().iter().filter(|(_, b)| b.player == p).count();
                let ready = sim.buildings().iter().filter(|(_, b)| b.player == p && b.is_ready()).count();
                let land = sim.land_by_player()[p as usize];
                let wares: Vec<String> = rr_rules::Ware::ALL
                    .iter()
                    .filter(|w| inv.ware(**w) > 0)
                    .map(|w| format!("{}:{}", w.name(), inv.ware(*w)))
                    .collect();
                println!(
                    "  p{p}: land {land:5} buildings {ready}/{blds} soldiers {:3} helpers {:3} | {}",
                    inv.soldiers(),
                    inv.job(rr_rules::Job::Helper),
                    wares.join(" ")
                );
            }
        }
    }
    let mut kinds: std::collections::BTreeMap<String, usize> = Default::default();
    for (_, b) in sim.buildings().iter() {
        *kinds.entry(format!("p{} {:?}", b.player, b.kind)).or_default() += 1;
    }
    println!("{kinds:?}");
    // production buildings that stand without their worker (usually a missing tool)
    let mut unmanned: std::collections::BTreeMap<String, usize> = Default::default();
    for (_, b) in sim.buildings().iter() {
        let def = sim.rules().building(b.kind);
        if b.is_ready() && def.worker.is_some() && b.worker.is_none() && !b.worker_coming {
            *unmanned.entry(format!("p{} {:?}", b.player, b.kind)).or_default() += 1;
        }
    }
    println!("unmanned: {unmanned:?}");
    println!("commands: {by_kind:?}");
    let g = sim.world().grid();
    let mil: Vec<_> = sim
        .buildings()
        .iter()
        .filter(|(_, b)| b.is_ready() && (sim.rules().building(b.kind).is_military() || b.kind.is_warehouse()))
        .map(|(_, b)| (b.player, b.pos, b.soldiers.len()))
        .collect();
    let mut closest = u32::MAX;
    for a in &mil {
        for b in &mil {
            if a.0 != b.0 {
                closest = closest.min(g.distance(a.1, b.1));
            }
        }
    }
    println!("closest enemy military buildings: {closest}");
    if std::env::var_os("RR_DEBUG_MIL").is_some() {
        for p in 0..sim.players().len() as u8 {
            let g: Vec<String> = sim
                .buildings()
                .iter()
                .filter(|(_, b)| b.player == p && b.is_ready() && sim.rules().building(b.kind).is_military())
                .map(|(_, b)| format!("{:?}:{}/{}", b.kind, b.soldiers.len(), b.garrison_wanted))
                .collect();
            println!("  p{p} military {:?} garrisons {}", sim.players()[p as usize].settings.military, g.join(" "));
        }
    }
    let spots: Vec<_> = sim.world().grid().coords().filter(|p| sim.world().is_harbor_spot(*p)).collect();
    let owned = spots.iter().filter(|p| sim.world().owner(**p).is_some()).count();
    println!("harbor spots {} (owned {owned}), ships {}", spots.len(), sim.ships().len());
    debug_shipyards(&sim);
    println!("sync hash {:016x}", sim.sync_hash());
    ExitCode::SUCCESS
}

/// Convert a map between our `.rrmap` and the original's `.swd` (by the output's extension).
fn convert(from: &str, to: &str) -> ExitCode {
    let map = match std::fs::read(from).map_err(|e| e.to_string()).and_then(|b| rr_formats::rrmap::read_any(&b)) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{from}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let lower = to.to_ascii_lowercase();
    let bytes = if lower.ends_with(".swd") || lower.ends_with(".wld") {
        rr_formats::wld::write(&map)
    } else if lower.ends_with(".rrmap") {
        rr_formats::rrmap::write(&map)
    } else {
        eprintln!("{to}: write .rrmap or .swd");
        return ExitCode::from(2);
    };
    if let Err(e) = std::fs::write(to, bytes) {
        eprintln!("{to}: {e}");
        return ExitCode::FAILURE;
    }
    println!("{to}: {}×{}, {} players", map.grid.width, map.grid.height, map.players());
    ExitCode::SUCCESS
}

fn mapinfo(path: &str) -> ExitCode {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let m = match rr_formats::rrmap::read_any(&bytes) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    println!("name:      {}", m.name);
    println!("author:    {}", m.author);
    println!("size:      {} x {}", m.grid.width, m.grid.height);
    println!("landscape: {:?}", m.landscape);
    println!("players:   {}", m.players());
    for (i, p) in m.hq.iter().enumerate() {
        if let Some(p) = p {
            println!("  hq {i}: ({}, {})", p.x, p.y);
        }
    }
    let (mut coal, mut iron, mut gold, mut granite, mut water, mut fish) = (0u32, 0u32, 0u32, 0u32, 0u32, 0u32);
    for r in &m.resources {
        match *r {
            Resource::Coal(a) => coal += a as u32,
            Resource::Iron(a) => iron += a as u32,
            Resource::Gold(a) => gold += a as u32,
            Resource::Granite(a) => granite += a as u32,
            Resource::Water => water += 1,
            Resource::Fish(_) => fish += 1,
            Resource::None => {}
        }
    }
    println!("resources: coal {coal}, iron {iron}, gold {gold}, granite {granite} (sum of amounts)");
    println!("           water nodes {water}, fish nodes {fish}");
    let trees = m.objects.iter().filter(|o| matches!(o, MapObject::Tree(_))).count();
    let stones = m.objects.iter().filter(|o| matches!(o, MapObject::Granite { .. })).count();
    println!("objects:   {trees} trees, {stones} granite piles");
    println!("harbors:   {}", m.harbors.len());
    ExitCode::SUCCESS
}

fn debug_shipyards(sim: &rr_sim::Sim) {
    for (id, b) in sim.buildings().iter().filter(|(_, b)| b.kind == rr_rules::BuildingType::Shipyard) {
        for (_, w) in sim.wares().iter().filter(|(_, w)| w.goal == Some(id)) {
            println!("    ware to it: {w:?}");
            if let rr_sim::WareLoc::Flag(f) = w.loc {
                let fl = sim.flag(f);
                println!("    flag {:?} wares {}", fl.pos, fl.wares.len());
                for r in fl.roads.iter().flatten() {
                    let rd = sim.road(*r);
                    println!(
                        "    road {:?}->{:?} len {} kind {:?} carrier {:?} wanted {}",
                        sim.flag(rd.from).pos,
                        sim.flag(rd.to).pos,
                        rd.dirs.len(),
                        rd.kind,
                        rd.carrier.map(|c| sim.figure(c)),
                        rd.carrier_wanted
                    );
                }
            }
        }
        println!(
            "  shipyard p{} at {:?}: ready {} worker {} stock {:?} history {:016b} sea {:?} roads {} site {:?}",
            b.player,
            b.pos,
            b.is_ready(),
            b.worker.is_some(),
            b.stock,
            b.history,
            sim.harbor_sea(b.pos),
            sim.flag(b.flag).roads.iter().flatten().count(),
            match &b.state {
                rr_sim::BState::Site(s) => format!("{s:?} incoming {:?}", b.site_incoming),
                _ => String::new(),
            }
        );
    }
}

/// Rules from a file (`--rules FILE`), or a one-line message and exit 1.
fn read_rules(p: &str) -> std::sync::Arc<rr_rules::Rules> {
    std::sync::Arc::new(rr_rules::Rules::from_file(std::path::Path::new(p)).unwrap_or_else(|e| fail(&e)))
}

/// A map file, or a one-line message and exit 1.
fn read_map(p: &str) -> rr_core::MapData {
    std::fs::read(p)
        .map_err(|e| e.to_string())
        .and_then(|b| rr_formats::rrmap::read_any(&b))
        .unwrap_or_else(|e| fail(&format!("{p}: {e}")))
}

/// The slot that won too often with the same AI in every slot, if one did: more than two
/// standard deviations above its share, and at least six games (map maker round 1, #8; QA
/// round 8, #11: "twice its share" needed a clean sweep on a two-player map).
fn fairness_verdict(wins: &[u32]) -> Option<usize> {
    let total = wins.iter().sum::<u32>() as f64;
    let p = 1.0 / wins.len().max(1) as f64;
    let limit = total * p + 2.0 * (total * p * (1.0 - p)).sqrt();
    (0..wins.len()).find(|k| wins[*k] >= 6 && wins[*k] as f64 > limit)
}

/// Is a map fair: the same AI in every slot, `seeds` games; who wins (alive and the most land),
/// and a verdict (map maker round 1, #8: no tool could tell).
fn fairness(path: &str, args: &[&str]) -> ExitCode {
    let (mut lv, mut seeds, mut gf) = (rr_ai::Level::Normal, 8u64, 72_000u32);
    let mut rules = std::sync::Arc::new(rr_rules::Rules::classic());
    let mut it = args.iter();
    while let Some(k) = it.next() {
        let v = it.next().copied().unwrap_or("");
        match *k {
            "--level" => lv = level(v),
            "--seeds" => seeds = v.parse().unwrap_or(8),
            "--gf" => gf = v.parse().unwrap_or(72_000),
            "--rules" => rules = read_rules(v),
            other => fail(&format!("fairness: unknown option {other}")),
        }
    }
    let map = read_map(path);
    let n = map.players();
    if n < 2 {
        fail("fairness: the map needs two players or more");
    }
    let results: Vec<(u64, Vec<u32>, Option<usize>)> = std::thread::scope(|sc| {
        let hs: Vec<_> = (1..=seeds)
            .map(|seed| {
                let (map, rules) = (&map, rules.clone());
                sc.spawn(move || {
                    let setup = rr_sim::GameSetup { seed, players: vec![Default::default(); n], ..Default::default() };
                    let mut sim = rr_sim::Sim::new(rules, map, &setup)
                        .unwrap_or_else(|e| fail(&format!("this map can't be played: {e:?}")));
                    let mut ais: Vec<rr_ai::Ai> = (0..n as u8).map(|p| rr_ai::Ai::with_level(p, lv)).collect();
                    for ai in &mut ais {
                        ai.reseed(seed);
                    }
                    for _ in 0..gf {
                        let mut cmds = Vec::new();
                        for ai in ais.iter_mut() {
                            cmds.extend(ai.think(&sim));
                        }
                        sim.step(&cmds);
                    }
                    let land = sim.land_by_player();
                    let winner = (0..n).filter(|p| sim.players()[*p].alive).max_by_key(|p| land[*p]);
                    (seed, land, winner)
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().expect("a game")).collect()
    });
    let mut wins = vec![0u32; n];
    for (seed, land, winner) in &results {
        println!("seed {seed}: land {land:?}, most: slot {}", winner.map_or("-".into(), |w| (w + 1).to_string()));
        if let Some(w) = winner {
            wins[*w] += 1;
        }
    }
    let shown: Vec<String> = wins.iter().enumerate().map(|(k, w)| format!("slot {}: {w}", k + 1)).collect();
    println!("{lv:?} in every slot, {seeds} games of {} minutes: {}", gf / 1200, shown.join(", "));
    match fairness_verdict(&wins) {
        Some(k) => {
            println!("verdict: slot {} wins too often ({} of {seeds})", k + 1, wins[k]);
            ExitCode::from(1)
        }
        None => {
            println!("verdict: no slot wins too often in {seeds} games");
            ExitCode::SUCCESS
        }
    }
}

/// Say what went wrong and stop (exit 1), without a panic's backtrace.
fn fail(e: &str) -> ! {
    eprintln!("{e}");
    std::process::exit(1)
}

#[cfg(test)]
mod fairness_tests {
    /// Map maker round 1, #8: a map's slots are judged by who wins with the same AI in every one.
    #[test]
    fn a_slot_that_wins_too_often_is_named() {
        assert_eq!(super::fairness_verdict(&[4, 4]), None);
        assert_eq!(super::fairness_verdict(&[5, 3]), None, "5:3 is chance");
        assert_eq!(super::fairness_verdict(&[8, 0]), Some(0));
        // QA round 8, #11: 7 of 8 is no chance either (it took a clean sweep)
        assert_eq!(super::fairness_verdict(&[7, 1]), Some(0));
        assert_eq!(super::fairness_verdict(&[6, 2]), None);
        assert_eq!(super::fairness_verdict(&[1, 1, 10]), Some(2));
    }
}
