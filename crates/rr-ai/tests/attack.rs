use rr_core::{Direction, MapCoord, MapData};
use rr_rules::{BuildingType as B, Job, Rules};
use rr_sim::{Cmd, Event, GameSetup, PlayerCommand, PlayerSetup, Sim};
use std::sync::Arc;

fn c(x: u16, y: u16) -> MapCoord {
    MapCoord::new(x, y)
}

fn ok(sim: &mut Sim, player: u8, cmd: Cmd) {
    let out = sim.step(&[PlayerCommand { player, cmd: cmd.clone() }]);
    assert!(out.events.iter().all(|e| !matches!(e, Event::CommandRejected { .. })), "{cmd:?}: {:?}", out.events);
}

/// Veteran round 8, #1: an HQ held by a few privates is worth attacking. The AI priced every
/// soldier in a warehouse as a sergeant, so North America p5 never attacked an HQ of privates.
/// Player 0 with 9 generals in a guardhouse and a watchtower (2 + 5 may attack), player 1's HQ
/// with `privates` privates and no beer, 18 apart.
fn duel(privates_left: u32) -> (Sim, MapCoord) {
    let (h0, h1) = (c(14, 20), c(32, 20));
    let mut m = MapData::flat(64, 64, 8);
    m.hq = vec![Some(h0), Some(h1)];
    let setup = GameSetup { seed: 42, players: vec![PlayerSetup::default(); 2], ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &m, &setup).unwrap();
    // player 1: four privates at home and no beer to recruit more
    let privates = sim.player_inventory(1).job(Job::Private);
    sim.debug_take_job(1, Job::Private, privates - privates_left);
    sim.debug_take_ware(1, rr_rules::Ware::Beer, 99);
    // player 0: a guardhouse of generals in sight of it
    // (and nobody else: no other post can be manned)
    for j in Job::SOLDIERS {
        let n = sim.player_inventory(0).job(j);
        sim.debug_take_job(0, j, n);
    }
    sim.debug_take_ware(0, rr_rules::Ware::Beer, 99);
    sim.debug_add_job(0, Job::General, 9);
    ok(&mut sim, 0, Cmd::SetMilitary { values: [10, 5, 5, 5, 8, 8, 8, 8] });
    // a guardhouse and a watchtower: 2 + 5 generals may go, 49 HP against 13 privates (39 HP)
    let at = c(21, 20);
    for kind in [B::Guardhouse, B::Watchtower] {
        let g = sim.world().grid();
        let p = if kind == B::Guardhouse {
            at
        } else {
            g.coords()
                .filter(|p| g.distance(*p, h1) <= 18 && g.distance(*p, at) >= 5 && !sim.military_building_near(*p, 0))
                .find(|p| matches!(sim.world().bq_for(*p, 0), rr_world::Bq::House | rr_world::Bq::Castle))
                .expect("a spot for the watchtower")
        };
        ok(&mut sim, 0, Cmd::PlaceBuilding { at: p, kind });
        let flag = sim.world().grid().neighbor(p, Direction::SE);
        let hq_flag = sim.world().grid().neighbor(h0, Direction::SE);
        let dirs = sim.road_path(0, flag, hq_flag, &[]).expect("road");
        ok(&mut sim, 0, Cmd::BuildRoad { from: flag, dirs });
    }
    (sim, h1)
}

#[test]
fn an_hq_of_privates_is_attacked() {
    let (mut sim, h1) = duel(13);
    let at = c(21, 20);
    let mut ai = rr_ai::Ai::new(0);
    let mut attacked = false;
    for _ in 0..30_000 {
        let cmds = ai.think(&sim);
        attacked |= cmds.iter().any(|c| matches!(c.cmd, Cmd::Attack { target, .. } if target == h1));
        if attacked {
            break;
        }
        sim.step(&cmds);
    }
    let g = sim.building_at(at).map(|b| sim.building(b).soldiers.len());
    assert!(attacked, "no attack on an HQ of 13 privates; guardhouse soldiers {g:?}, gf {}", sim.gf().0);
}

/// New-player round 8, #1: an easy rival sends what its rule asks for, not its whole spare army
/// (22 against a guardhouse of 3), and attacks at most every 8 minutes.
#[test]
fn easy_attacks_are_measured() {
    let (mut sim, h1) = duel(4);
    let mut ai = rr_ai::Ai::with_level(0, rr_ai::Level::Easy);
    let mut attacks: Vec<(u32, u8)> = Vec::new();
    for _ in 0..80_000 {
        let cmds = ai.think(&sim);
        for c in &cmds {
            if let Cmd::Attack { target, soldiers, .. } = c.cmd
                && target == h1
            {
                attacks.push((sim.gf().0, soldiers));
            }
        }
        sim.step(&cmds);
    }
    assert!(!attacks.is_empty(), "no attack at all");
    // 4 privates (12 HP) want 1.5 x 12 + 2 = 20 HP: three generals
    assert!(attacks.iter().all(|(_, n)| *n <= 4), "{attacks:?}");
    assert!(attacks.windows(2).all(|w| w[1].0 - w[0].0 >= 8 * 60 * 20), "{attacks:?}");
}

/// New-player round 8, #5: after the truce a newcomer's castle is spared another 15 minutes
/// (normal rivals took it four minutes after the truce, with 14 soldiers).
#[test]
fn the_castle_is_spared_after_the_truce() {
    let (mut sim, h1) = duel(13);
    let mut ai = rr_ai::Ai::new(0);
    let until = 12_000;
    ai.truce = Some((1, until));
    let mut first = None;
    for _ in 0..60_000 {
        let cmds = ai.think(&sim);
        if first.is_none() && cmds.iter().any(|c| matches!(c.cmd, Cmd::Attack { target, .. } if target == h1)) {
            first = Some(sim.gf().0);
        }
        sim.step(&cmds);
    }
    let first = first.expect("the castle is attacked in the end");
    assert!(first >= until + 15 * 60 * 20, "attacked at {first}");
}

/// QA round 5, #6: no fishery by a lake that is fished out (the AI counted empty fish grounds
/// and rebuilt a fishery on the same spot every 15-20 minutes).
#[test]
fn no_fishery_by_an_empty_lake() {
    let mut m = MapData::flat(64, 64, 8);
    let g = m.grid;
    let lake = c(28, 20);
    for p in g.within(lake, 3) {
        let i = g.index(p);
        m.terrain_rsu[i] = 5;
        m.terrain_usd[i] = 5;
        m.resources[i] = rr_core::Resource::Fish(0);
    }
    m.hq = vec![Some(c(20, 20))];
    let setup = GameSetup { seed: 42, players: vec![PlayerSetup::default()], ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &m, &setup).unwrap();
    let mut ai = rr_ai::Ai::new(0);
    for _ in 0..20_000 {
        let cmds = ai.think(&sim);
        assert!(
            !cmds.iter().any(|c| matches!(c.cmd, Cmd::PlaceBuilding { kind: B::Fishery, .. })),
            "a fishery by an empty lake at {}",
            sim.gf().0
        );
        sim.step(&cmds);
    }
}

/// Veteran round 9, #1: a realm cut off from every warehouse keeps its posts (RTTR only
/// reconnects sites, and a surrender leaves the buildings standing); the AI razed its own realm
/// after the HQ fell.
#[test]
fn a_cut_off_realm_keeps_its_posts() {
    let h0 = c(20, 20);
    let mut m = MapData::flat(64, 64, 8);
    m.hq = vec![Some(h0)];
    let setup = GameSetup { seed: 42, players: vec![PlayerSetup::default()], ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &m, &setup).unwrap();
    let g = sim.world().grid();
    let mut posts = Vec::new();
    for at in [c(27, 20), c(20, 27), c(14, 16)] {
        ok(&mut sim, 0, Cmd::PlaceBuilding { at, kind: B::Guardhouse });
        let flag = g.neighbor(at, Direction::SE);
        let dirs = sim.road_path(0, flag, g.neighbor(h0, Direction::SE), &[]).expect("road");
        ok(&mut sim, 0, Cmd::BuildRoad { from: flag, dirs });
        posts.push(at);
    }
    for _ in 0..30_000 {
        sim.step(&[]);
    }
    assert!(posts.iter().all(|p| sim.building_at(*p).is_some_and(|b| !sim.building(b).soldiers.is_empty())), "manned");
    sim.debug_destroy_building(h0);
    let mut ai = rr_ai::Ai::new(0);
    for _ in 0..2_000 {
        let cmds = ai.think(&sim);
        sim.step(&cmds);
    }
    let standing = posts.iter().filter(|p| sim.building_at(**p).is_some()).count();
    assert_eq!(standing, 3, "the AI tore its posts down");
    // with no warehouse left it gives up as RTTR's AI does (TestDefeat), the posts left standing
    // for whoever takes them
    assert!(!sim.players()[0].alive, "surrendered");
}

/// Veteran round 9, #3: the soldiers a warehouse keeps at home are no soldiers to man a new post
/// (151 finished posts stood unmanned at minute 200, each with its stones and boards).
#[test]
fn no_posts_for_the_reserve() {
    let mut m = MapData::flat(64, 64, 8);
    m.hq = vec![Some(c(20, 20))];
    let setup = GameSetup { seed: 42, players: vec![PlayerSetup::default()], ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &m, &setup).unwrap();
    // exactly the HQ's reserve of 2 privates, and no way to recruit
    for j in Job::SOLDIERS {
        let n = sim.player_inventory(0).job(j);
        sim.debug_take_job(0, j, n);
    }
    sim.debug_add_job(0, Job::Private, 2);
    sim.debug_take_ware(0, rr_rules::Ware::Beer, 99);
    let mut ai = rr_ai::Ai::new(0);
    let mut posts = 0;
    for _ in 0..6_000 {
        let cmds = ai.think(&sim);
        posts += cmds
            .iter()
            .filter(|c| matches!(c.cmd, Cmd::PlaceBuilding { kind, .. } if sim.rules().building(kind).is_military()))
            .count();
        sim.step(&cmds);
    }
    assert_eq!(posts, 0, "posts placed with the reserve");
}

/// Veteran round 9, #4: a realm out of stone expands toward rock (husk realms sat at 0 stones for
/// an hour with rock beyond their border).
#[test]
fn no_stone_means_grow_toward_rock() {
    let h0 = c(20, 30);
    let rock = c(34, 30);
    let mut m = MapData::flat(64, 64, 8);
    m.hq = vec![Some(h0)];
    let g = m.grid;
    for p in g.within(rock, 1) {
        m.objects[g.index(p)] = rr_core::MapObject::Granite { kind: 0, size: 6 };
    }
    let setup = GameSetup { seed: 42, players: vec![PlayerSetup::default()], ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &m, &setup).unwrap();
    let stones = sim.player_inventory(0).ware(rr_rules::Ware::Stones);
    sim.debug_take_ware(0, rr_rules::Ware::Stones, stones);
    let mut ai = rr_ai::Ai::new(0);
    let got = (0..30_000).find(|_| {
        let cmds = ai.think(&sim);
        sim.step(&cmds);
        sim.world().owner(rock) == Some(0)
    });
    assert!(got.is_some(), "the rock 14 out is still beyond the border");
}

/// New-player round 9, #2: in a campaign, for 30 minutes after the truce, a normal rival sends
/// what easy's rule asks for (27 against a post of one made the second campaign a cliff).
#[test]
fn campaign_rivals_start_in_measure() {
    let (mut sim, h1) = duel(4);
    let mut ai = rr_ai::Ai::new(0);
    ai.truce = Some((1, 1_000));
    let mut sent = Vec::new();
    for _ in 0..30_000 {
        let cmds = ai.think(&sim);
        for c in &cmds {
            if let Cmd::Attack { target, soldiers, .. } = c.cmd
                && target == h1
            {
                sent.push((sim.gf().0, soldiers));
            }
        }
        sim.step(&cmds);
    }
    assert!(!sent.is_empty(), "no attack");
    // 4 privates (12 HP) want 1.5 x 12 + 2 = 20 HP: three generals, not all seven
    assert!(sent.iter().all(|(_, n)| *n <= 4), "{sent:?}");
}

/// New-player round 10, #1: long after the measure the rivals still send only easy's measure
/// against a campaign player's castle (10-21 against a castle of 4-6 took it at minute 73 in two
/// of three games: everything the newcomer owns is in it).
#[test]
fn a_campaign_castle_is_attacked_in_measure() {
    let (mut sim, h1) = duel(4);
    for _ in 0..40_000 {
        sim.step(&[]);
    }
    let mut ai = rr_ai::Ai::new(0);
    ai.truce = Some((1, 1));
    let mut sent = Vec::new();
    for _ in 0..30_000 {
        let cmds = ai.think(&sim);
        for c in &cmds {
            if let Cmd::Attack { target, soldiers, .. } = c.cmd
                && target == h1
            {
                sent.push((sim.gf().0, soldiers));
            }
        }
        sim.step(&cmds);
    }
    assert!(!sent.is_empty(), "no attack");
    assert!(sent.iter().all(|(_, n)| *n <= 4), "{sent:?}");
}

/// Veteran round 10, #6: a post whose garrison was cut (Send the strongest home) asks for its
/// full garrison again once it faces a rival (RTTR HandleBorderChanged resets the troop limits).
#[test]
fn a_cut_garrison_is_raised_again_at_the_front() {
    let (mut sim, _) = duel(4);
    let g = sim.world().grid();
    let tower = loop {
        sim.step(&[]);
        if let Some((_, b)) =
            sim.buildings().iter().find(|(_, b)| b.player == 0 && b.kind == B::Watchtower && b.is_ready())
        {
            break b.pos;
        }
        assert!(sim.gf().0 < 30_000, "no watchtower");
    };
    let _ = g;
    ok(&mut sim, 0, Cmd::SetGarrisonWanted { at: tower, wanted: 1 });
    let mut ai = rr_ai::Ai::new(0);
    let mut raised = false;
    for _ in 0..4_000 {
        let cmds = ai.think(&sim);
        raised |=
            cmds.iter().any(|c| matches!(c.cmd, Cmd::SetGarrisonWanted { at, wanted } if at == tower && wanted > 1));
        sim.step(&cmds);
        if raised {
            break;
        }
    }
    assert!(raised, "the front watchtower stays cut to 1");
}

/// Veteran round 10, #4: a castle is worn down in waves: 4 privates in reach attack a castle of 7
/// (RTTR TryToAttack never strength-checks a warehouse); a castle of 7 privates lived 80 minutes
/// beside a neighbour with 4 attackers.
#[test]
fn a_castle_is_worn_down_in_waves() {
    let (h0, h1) = (c(14, 20), c(32, 20));
    let mut m = MapData::flat(64, 64, 8);
    m.hq = vec![Some(h0), Some(h1)];
    let setup = GameSetup { seed: 42, players: vec![PlayerSetup::default(); 2], ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &m, &setup).unwrap();
    let privates = sim.player_inventory(1).job(Job::Private);
    sim.debug_take_job(1, Job::Private, privates - 7);
    sim.debug_take_ware(1, rr_rules::Ware::Beer, 99);
    for j in Job::SOLDIERS {
        let n = sim.player_inventory(0).job(j);
        sim.debug_take_job(0, j, n);
    }
    sim.debug_take_ware(0, rr_rules::Ware::Beer, 99);
    // (6 for the two guardhouses, 2 the AI keeps in the castle)
    sim.debug_add_job(0, Job::Private, 8);
    ok(&mut sim, 0, Cmd::SetMilitary { values: [0, 5, 5, 5, 8, 8, 8, 8] });
    // two guardhouses of 3 privates in reach: 2 + 2 may go
    let g = sim.world().grid();
    let mut placed = Vec::new();
    for p in g.coords().filter(|p| g.distance(*p, h1) <= 15 && g.distance(*p, h0) <= 9) {
        if placed.len() == 2 {
            break;
        }
        if placed.iter().all(|q| g.distance(*q, p) >= 5)
            && !sim.military_building_near(p, 0)
            && matches!(sim.world().bq_for(p, 0), rr_world::Bq::Hut | rr_world::Bq::House | rr_world::Bq::Castle)
        {
            ok(&mut sim, 0, Cmd::PlaceBuilding { at: p, kind: B::Guardhouse });
            let flag = g.neighbor(p, Direction::SE);
            let dirs = sim.road_path(0, flag, g.neighbor(h0, Direction::SE), &[]).expect("road");
            ok(&mut sim, 0, Cmd::BuildRoad { from: flag, dirs });
            placed.push(p);
        }
    }
    assert_eq!(placed.len(), 2, "two guardhouses");
    let mut ai = rr_ai::Ai::new(0);
    let mut attacked = false;
    // (past the first 40 minutes, when a realm keeps its castle's store)
    for _ in 0..80_000 {
        let cmds = ai.think(&sim);
        attacked |= cmds.iter().any(|c| matches!(c.cmd, Cmd::Attack { target, .. } if target == h1));
        sim.step(&cmds);
        if attacked {
            break;
        }
    }
    assert!(attacked, "4 privates never went for a castle of 7");
}

/// Game design round 1, #5: a realm with no post, none being built and no soldier left gives
/// up after the first hour (one storehouse kept a beaten realm alive for 2-4 hours).
#[test]
fn a_beaten_realm_gives_up() {
    let (h0, h1) = (c(14, 20), c(46, 20));
    let mut m = MapData::flat(64, 64, 8);
    m.hq = vec![Some(h0), Some(h1)];
    let setup = GameSetup { seed: 1, players: vec![PlayerSetup::default(); 2], ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &m, &setup).unwrap();
    for j in Job::SOLDIERS {
        let n = sim.player_inventory(1).job(j);
        sim.debug_take_job(1, j, n);
    }
    sim.debug_take_ware(1, rr_rules::Ware::Beer, 99);
    for _ in 0..(60 * 60 * 20 + 100) {
        sim.step(&[]);
    }
    let mut ai = rr_ai::Ai::new(1);
    let mut gave_up = false;
    for _ in 0..200 {
        let cmds = ai.think(&sim);
        gave_up |= cmds.iter().any(|c| matches!(c.cmd, Cmd::Surrender));
        sim.step(&cmds);
    }
    assert!(gave_up, "a realm with no post and no soldier holds on");
    // early on, an empty store is no reason to give up
    let mut young = Sim::new(Arc::new(Rules::classic()), &m, &setup).unwrap();
    for j in Job::SOLDIERS {
        let n = young.player_inventory(1).job(j);
        young.debug_take_job(1, j, n);
    }
    let mut ai = rr_ai::Ai::new(1);
    for _ in 0..200 {
        let cmds = ai.think(&young);
        assert!(!cmds.iter().any(|c| matches!(c.cmd, Cmd::Surrender)), "gave up at once");
        young.step(&cmds);
    }
}

/// Game design round 1, #7: Mare Nostrum had no war (0-1 attacks in 180 minutes). Egypt, which
/// knows Carthage's coast, lands an expedition there: in most games the first attack between the
/// newcomer and a rival falls before minute 90 (seeds 1-14: 9 of 14, item 471; 0 of 7 before
/// item 461).
/// Rivals each on their own, at normal, with the half-hour truce; the newcomer a normal AI that
/// keeps the truce.
#[test]
#[ignore = "whole games: tools/eval.sh runs it (cargo test -p rr-ai -- --ignored)"]
fn mare_nostrum_has_its_war() {
    let mut firsts = Vec::new();
    let seeds: Vec<u64> =
        std::env::var("SEEDS").map_or((1..=5).collect(), |v| v.split(',').filter_map(|x| x.parse().ok()).collect());
    for seed in seeds {
        let region = rr_mapgen::world::region("mediterranean").unwrap();
        let map = rr_mapgen::region::generate(region, seed);
        let n = map.players();
        let setup = GameSetup { seed, players: (0..n).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
        let mut sim = Sim::new(Arc::new(Rules::classic()), &map, &setup).unwrap();
        let mut ais: Vec<rr_ai::Ai> = (0..n as u8).map(rr_ai::Ai::new).collect();
        for ai in &mut ais[1..] {
            ai.truce = Some((0, rr_ai::MISSION_TRUCE));
        }
        ais[5].invade = Some(0);
        let mut first = None;
        while sim.gf().0 < 90 * 60 * 20 && first.is_none() {
            let mut cmds = Vec::new();
            for ai in ais.iter_mut() {
                cmds.extend(ai.think(&sim));
            }
            // (the newcomer keeps the truce, as a player would)
            cmds.retain(|c| {
                c.player != 0 || sim.gf().0 >= rr_ai::MISSION_TRUCE || !matches!(c.cmd, Cmd::Attack { .. })
            });
            let war = cmds.iter().any(|c| match c.cmd {
                Cmd::Attack { target, .. } => {
                    c.player == 0 || sim.building_at(target).is_some_and(|b| sim.building(b).player == 0)
                }
                _ => false,
            });
            if war {
                first = Some(sim.gf().0 / 1200);
            }
            sim.step(&cmds);
        }
        eprintln!("seed {seed}: first attack at minute {first:?}");
        firsts.push(first);
    }
    assert!(firsts.iter().filter(|f| f.is_some()).count() >= 3, "no war by minute 90: {firsts:?}");
}

/// Genre lessons (PLAN §27, #2): the attack window can show who would go, because the sim says
/// it without sending anyone: the same soldiers the attack then sends.
#[test]
fn the_attack_party_is_known_before() {
    let (mut sim, h1) = duel(4);
    while sim.attackers_for(0, h1) < 7 {
        sim.step(&[]);
        assert!(sim.gf().0 < 30_000, "the posts weren't manned");
    }
    let party = sim.attack_party(0, h1, 3, true);
    assert_eq!(party, vec![4, 4, 4], "three generals");
    assert_eq!(sim.attack_party(0, h1, 99, true).len(), 7, "no more than may go");
    // the window's one search says the same (perf round 3, #5)
    let plan = sim.attack_plan(0, h1);
    assert_eq!((plan.available, plan.posts), (sim.attackers_for(0, h1), sim.attack_posts_near(0, h1)));
    for n in [1, 3, 7, 99] {
        assert_eq!(plan.party(n, true), sim.attack_party(0, h1, n, true).as_slice());
        assert_eq!(plan.party(n, false), sim.attack_party(0, h1, n, false).as_slice());
    }
    let before = sim.gf();
    assert_eq!(sim.attack_party(0, h1, 3, true), party);
    assert_eq!(sim.gf(), before, "asking changes nothing");
    ok(&mut sim, 0, Cmd::Attack { target: h1, soldiers: 3, strong: true });
    assert_eq!(sim.attackers_for(0, h1), 4, "the three that were named left");
}

/// Veteran round 11, #1: a storehouse can't be attacked (RTTR nobStorehouse::IsAttackable is
/// false; 67 were razed in 15 games). The castle and harbors can.
#[test]
fn storehouses_cant_be_attacked() {
    let (mut sim, h1) = duel(4);
    while sim.attackers_for(0, h1) < 7 {
        sim.step(&[]);
        assert!(sim.gf().0 < 30_000, "the posts weren't manned");
    }
    let hq = sim.building_at(h1).unwrap();
    assert!(sim.is_attackable(0, hq), "the castle can be attacked");
    // a storehouse of player 1, built at once
    let g = sim.world().grid();
    let at = g
        .coords()
        .filter(|p| g.distance(*p, h1) == 5 && sim.world().owner(*p) == Some(1))
        .find(|p| matches!(sim.world().bq_for(*p, 1), rr_world::Bq::House | rr_world::Bq::Castle))
        .expect("a spot");
    ok(&mut sim, 1, Cmd::PlaceBuilding { at, kind: B::Storehouse });
    let flag = g.neighbor(at, Direction::SE);
    let dirs = sim.road_path(1, flag, g.neighbor(h1, Direction::SE), &[]).expect("road");
    ok(&mut sim, 1, Cmd::BuildRoad { from: flag, dirs });
    let s = sim.building_at(at).unwrap();
    while !sim.building(s).is_ready() {
        sim.step(&[]);
        assert!(sim.gf().0 < 80_000, "the storehouse wasn't built");
    }
    assert!(!sim.is_attackable(0, s), "a storehouse can't be attacked");
}

/// Item 523 (veteran round 11, #3): an AI that knows the peace plans no attack and no catapult
/// before it ends, and attacks after (architect round 3, #3: today's rules had no behaviour test).
#[test]
fn the_peace_holds_the_ai_back() {
    let (mut sim, h1) = duel(13);
    let mut ai = rr_ai::Ai::new(0);
    ai.peace_until = 20_000;
    let mut attacked_at = None;
    for _ in 0..40_000 {
        let cmds = ai.think(&sim);
        if cmds.iter().any(|c| matches!(c.cmd, Cmd::Attack { .. } | Cmd::PlaceBuilding { kind: B::Catapult, .. })) {
            attacked_at = Some(sim.gf().0);
            break;
        }
        sim.step(&cmds);
    }
    let at = attacked_at.expect("an attack after the peace");
    assert!(at >= 20_000, "attacked in the peace, at gf {at}");
    let _ = h1;
}
