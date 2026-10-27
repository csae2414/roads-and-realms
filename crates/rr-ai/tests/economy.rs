//! Veteran round 3, #12: the computer players keep their tool supply going.
use rr_rules::{BuildingType as B, Rules};
use rr_sim::{GameSetup, PlayerSetup, Sim};
use std::sync::Arc;

#[test]
fn every_ai_builds_a_metalworks_early() {
    let region = rr_mapgen::world::region("europe").unwrap();
    let map = rr_mapgen::region::generate(region, 1);
    let n = map.players();
    let setup = GameSetup { seed: 1, players: (0..n).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &map, &setup).unwrap();
    let mut ais: Vec<rr_ai::Ai> = (0..n as u8).map(rr_ai::Ai::new).collect();
    // a metalworks may burn with land lost in a war later: count it once built or begun
    let mut had = vec![false; n];
    for _ in 0..30_000 {
        let mut cmds = Vec::new();
        for ai in ais.iter_mut() {
            cmds.extend(ai.think(&sim));
        }
        sim.step(&cmds);
        for (_, b) in sim.buildings().iter() {
            if b.kind == B::Metalworks {
                had[b.player as usize] = true;
            }
        }
    }
    for p in 0..n as u8 {
        let has = had[p as usize];
        let kinds: std::collections::BTreeMap<String, usize> =
            sim.buildings().iter().filter(|(_, b)| b.player == p).fold(Default::default(), |mut m, (_, b)| {
                *m.entry(format!("{:?}", b.kind)).or_default() += 1;
                m
            });
        assert!(has, "player {p} has no metalworks by 30k GF: {kinds:?}");
    }
}

/// QA round 3, #1: the AI keeps soldiers at home in its headquarters (reserves start at none).
#[test]
fn the_ai_keeps_a_reserve_at_home() {
    let map =
        rr_mapgen::generate(&rr_mapgen::GenParams { width: 64, height: 64, players: 2, seed: 3, ..Default::default() });
    let setup = GameSetup { seed: 3, players: (0..2).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &map, &setup).unwrap();
    let mut ais: Vec<rr_ai::Ai> = (0..2).map(rr_ai::Ai::new).collect();
    for _ in 0..20_000 {
        let mut cmds = Vec::new();
        for ai in ais.iter_mut() {
            cmds.extend(ai.think(&sim));
        }
        sim.step(&cmds);
    }
    for p in 0..2 {
        let hq = sim.players()[p].hq.expect("hq");
        let b = sim.building(hq);
        assert!(b.reserve[0] >= 1, "player {p}: reserve {:?}", b.reserve);
        let inv = b.inventory.as_ref().unwrap();
        assert!(rr_rules::Job::SOLDIERS.iter().map(|j| inv.job(*j)).sum::<u32>() >= 1, "player {p}: no soldier home");
    }
}

/// Veteran round 5, #1: every computer player expands, also one whose HQ has no free spot near
/// its border (North America p4 used to keep its starting land for 100 minutes).
#[test]
fn every_ai_expands_on_every_region() {
    for name in ["europe", "mediterranean", "northamerica", "fareast", "north"] {
        let region = rr_mapgen::world::region(name).unwrap();
        let map = rr_mapgen::region::generate(region, 1);
        let n = map.players();
        let setup =
            GameSetup { seed: 1, players: (0..n).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
        let mut sim = Sim::new(Arc::new(Rules::classic()), &map, &setup).unwrap();
        let mut ais: Vec<rr_ai::Ai> = (0..n as u8).map(rr_ai::Ai::new).collect();
        let land = |sim: &Sim, p: u8| sim.world().grid().coords().filter(|q| sim.world().owner(*q) == Some(p)).count();
        sim.step(&[]);
        let start: Vec<usize> = (0..n as u8).map(|p| land(&sim, p)).collect();
        for _ in 0..15_000 {
            let mut cmds = Vec::new();
            for ai in ais.iter_mut() {
                cmds.extend(ai.think(&sim));
            }
            sim.step(&cmds);
        }
        for p in 0..n as u8 {
            let now = land(&sim, p);
            assert!(now > start[p as usize], "{name} p{p}: land {now}, at the start {}", start[p as usize]);
        }
    }
}

/// Veteran round 5, #2: the metal chain grows with the iron mines (a smelter per iron mine, an
/// armory per smelter, coal for both), so ore doesn't pile up while the smelter waits.
#[test]
#[ignore = "a whole game: tools/eval.sh runs it (cargo test -p rr-ai -- --ignored)"]
fn iron_ore_does_not_pile_up() {
    let region = rr_mapgen::world::region("europe").unwrap();
    let map = rr_mapgen::region::generate(region, 1);
    let n = map.players();
    let setup = GameSetup { seed: 1, players: (0..n).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &map, &setup).unwrap();
    let mut ais: Vec<rr_ai::Ai> = (0..n as u8).map(rr_ai::Ai::new).collect();
    // fields around ready farms, sampled over the last 20 000 GF (a snapshot depends on harvests)
    let (mut fields, mut farm_samples) = (0usize, 0usize);
    for i in 0..80_000 {
        let mut cmds = Vec::new();
        for ai in ais.iter_mut() {
            cmds.extend(ai.think(&sim));
        }
        sim.step(&cmds);
        if i >= 60_000 && i % 1000 == 0 {
            let g = sim.world().grid();
            for (_, b) in sim.buildings().iter().filter(|(_, b)| b.kind == B::Farm && b.is_ready()) {
                farm_samples += 1;
                fields +=
                    g.within(b.pos, 2).filter(|q| matches!(sim.world().obj(*q), rr_world::NodeObj::Field(_))).count();
            }
        }
    }
    let mut report = Vec::new();
    for p in 0..n as u8 {
        let inv = sim.player_inventory(p);
        let count = |k: B| sim.buildings().iter().filter(|(_, b)| b.player == p && b.kind == k && b.is_ready()).count();
        report.push(format!(
            "p{p}: ore {} coal {} iron mines {} coal mines {} smelters {} armories {}",
            inv.ware(rr_rules::Ware::IronOre),
            inv.ware(rr_rules::Ware::Coal),
            count(B::IronMine),
            count(B::CoalMine),
            count(B::IronSmelter),
            count(B::Armory)
        ));
    }
    // sites count: a building on its way is the plan
    let all = |p: u8, k: B| sim.buildings().iter().filter(|(_, b)| b.player == p && b.kind == k).count();
    // a war may just have burnt a smelter or an armory: one realm out of line is allowed (the
    // old plan left three of six short)
    let short = (0..n as u8)
        .filter(|p| {
            let mines =
                sim.buildings().iter().filter(|(_, b)| b.player == *p && b.kind == B::IronMine && b.is_ready()).count();
            all(*p, B::IronSmelter) < mines || all(*p, B::Armory) < all(*p, B::IronSmelter).min(mines)
        })
        .count();
    assert!(short <= 1, "{short} realms lack a smelter per iron mine or an armory per smelter:\n{}", report.join("\n"));
    // veteran round 5, #5: food follows its users (RTTR BuildingPlanner: farms <= grain users + 3;
    // without mines one bakery and one slaughterhouse are plenty)
    for p in 0..n as u8 {
        let grain_users = all(p, B::Mill) + all(p, B::PigFarm) + all(p, B::Brewery) + all(p, B::DonkeyBreeder);
        assert!(all(p, B::Farm) <= grain_users + 3, "p{p}: {} farms for {grain_users} grain users", all(p, B::Farm));
        let mines = all(p, B::CoalMine) + all(p, B::IronMine) + all(p, B::GoldMine) + all(p, B::GraniteMine);
        if mines == 0 {
            assert!(all(p, B::Bakery) <= 1 && all(p, B::Slaughterhouse) <= 1, "p{p}: food without mines");
        }
        // veteran round 5, #6: a big realm breeds donkeys for its busy roads (RTTR: over 7 farms)
        let ready_farms =
            sim.buildings().iter().filter(|(_, b)| b.player == p && b.kind == B::Farm && b.is_ready()).count();
        if ready_farms > 7 {
            assert!(all(p, B::DonkeyBreeder) >= 1, "p{p}: {ready_farms} farms and no donkey breeder");
        }
    }
    // veteran round 5, #7: the land around a farm stays farmland, so its farmer has room to sow
    // (a lone farm on open meadow keeps about 5 fields, fields may not touch; the old plan kept
    // 2.3 on average here; the new one 2.4-2.8 depending on the map and the economy)
    let avg10 = fields * 10 / farm_samples.max(1);
    assert!(avg10 >= 24, "fields per farm {}.{}", avg10 / 10, avg10 % 10);
}

/// Veteran round 5, #4: recruiting needs beer, sword and shield in the same warehouse, so the AI
/// gathers all three at its headquarters (RTTR SetGatheringForUpgradeWarehouse).
#[test]
fn the_ai_gathers_recruit_goods_at_the_hq() {
    use rr_rules::Ware;
    let mut m = rr_core::MapData::flat(64, 64, 0);
    m.hq = vec![Some(rr_core::MapCoord::new(20, 20))];
    let setup = GameSetup { seed: 1, players: vec![PlayerSetup::default()], ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &m, &setup).unwrap();
    let mut ai = rr_ai::Ai::new(0);
    for _ in 0..3_000 {
        let cmds = ai.think(&sim);
        sim.step(&cmds);
    }
    let hq = sim.building_at(rr_core::MapCoord::new(20, 20)).unwrap();
    for w in [Ware::Beer, Ware::Sword, Ware::Shield] {
        assert_eq!(sim.building(hq).stock_policy[w.index()], rr_sim::StockPolicy::Collect, "{w:?}");
    }
}

/// Veteran round 5, #5: a realm without ore keeps its food small (one bakery, one slaughterhouse)
/// instead of piling up bread and meat nobody eats.
#[test]
fn no_mines_little_food() {
    let mut m = rr_core::MapData::flat(128, 128, 8);
    m.hq = vec![Some(rr_core::MapCoord::new(64, 64))];
    let setup = GameSetup { seed: 3, players: vec![PlayerSetup::default()], ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &m, &setup).unwrap();
    let mut ai = rr_ai::Ai::new(0);
    for _ in 0..70_000 {
        let cmds = ai.think(&sim);
        sim.step(&cmds);
    }
    let all = |k: B| sim.buildings().iter().filter(|(_, b)| b.kind == k).count();
    let kinds: std::collections::BTreeMap<String, usize> =
        sim.buildings().iter().fold(Default::default(), |mut m, (_, b)| {
            *m.entry(format!("{:?}", b.kind)).or_default() += 1;
            m
        });
    assert!(all(B::Bakery) <= 1 && all(B::Slaughterhouse) <= 1, "{kinds:?}");
    assert!(all(B::Farm) >= 2, "still some food: {kinds:?}");
}

/// Veteran round 6, #2: an expedition's colony stays (its harbor site is a root of its own road
/// network, RTTR only ever connects sites); the AI used to burn it seconds after landing. (A
/// harbor site the AI placed itself and can't reach by road is given up like any other site:
/// game design round 1, #7.)
#[test]
#[ignore = "a whole game: tools/eval.sh runs it (cargo test -p rr-ai -- --ignored)"]
fn colonies_are_kept() {
    let region = rr_mapgen::world::region("mediterranean").unwrap();
    let map = rr_mapgen::region::generate(region, 1);
    let n = map.players();
    let setup = GameSetup { seed: 1, players: (0..n).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &map, &setup).unwrap();
    let mut ais: Vec<rr_ai::Ai> = (0..n as u8).map(rr_ai::Ai::new).collect();
    let mut burnt = 0;
    for _ in 0..100_000 {
        let mut cmds = Vec::new();
        for ai in ais.iter_mut() {
            cmds.extend(ai.think(&sim));
        }
        for c in &cmds {
            if let rr_sim::Cmd::RemoveFlag { at } = c.cmd
                && let Some(f) = sim.flag_at(at)
                && let Some(b) = sim.flag(f).building
                && sim.building(b).kind == B::Harbor
                && !matches!(&sim.building(b).state, rr_sim::BState::Site(s) if !s.from_sea)
            {
                burnt += 1;
            }
        }
        sim.step(&cmds);
    }
    let harbors = |p: u8| sim.buildings().iter().filter(|(_, b)| b.player == p && b.kind == B::Harbor).count();
    let most = (0..n as u8).map(harbors).max().unwrap_or(0);
    assert_eq!(burnt, 0, "harbor flags removed by their own AI");
    assert!(most >= 2, "no realm keeps a colony harbor (most harbors: {most})");
}

/// Veteran round 6, #3/#4: a front where the rivals' land touches but neither sees the other's
/// buildings gets a watchtower or fortress pushed into the gap, so the war can start
/// (Mediterranean seed 1 had such fronts for the whole game).
#[test]
#[ignore = "a whole game: tools/eval.sh runs it (cargo test -p rr-ai -- --ignored)"]
fn blind_fronts_get_pushed() {
    // four games: one game's fronts are a few, and chance decides a lot of them
    let results: Vec<(Vec<(u8, u8)>, usize, usize)> = std::thread::scope(|sc| {
        let games: Vec<_> = [1, 2, 3, 4].into_iter().map(|seed| sc.spawn(move || blind_fronts(seed))).collect();
        games.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let blind: usize = results.iter().map(|r| r.0.len()).sum();
    let fronts: usize = results.iter().map(|r| r.1).sum();
    assert!(fronts >= 3, "only {fronts} old fronts");
    // a lookout tower sees 20 into the gap (RTTR VISUALRANGE_LOOKOUTTOWER); a game whose
    // fronts all came into sight without one needs none (item 461: seed 4, three fronts)
    assert!(results.iter().all(|r| r.2 > 0 || r.0.is_empty()), "no lookout tower: {results:?}");
    // (on the maps of item 287 some fronts are slivers of 6-7 touching nodes that neither side's
    // posts see across: two of six in these games, still open)
    assert!(blind * 3 <= fronts, "blind fronts {results:?} (blind pairs, old fronts per game)");
}

/// The old fronts of a Mediterranean game still blind at the end, how many old fronts, and
/// the lookout towers.
fn blind_fronts(seed: u64) -> (Vec<(u8, u8)>, usize, usize) {
    let region = rr_mapgen::world::region("mediterranean").unwrap();
    let map = rr_mapgen::region::generate(region, seed);
    let n = map.players();
    let setup = GameSetup { seed, players: (0..n).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &map, &setup).unwrap();
    let mut ais: Vec<rr_ai::Ai> = (0..n as u8).map(rr_ai::Ai::new).collect();
    let mut attacks = vec![0; n];
    // (on land: realms also own the sea along their coasts)
    let touching = |sim: &Sim, a: u8, b: u8| {
        let w = sim.world();
        let g = w.grid();
        g.coords().any(|p| {
            w.owner(p) == Some(a)
                && w.node_walkable(p)
                && g.neighbors(p).iter().any(|q| w.owner(*q) == Some(b) && w.node_walkable(*q))
        })
    };
    // fronts old enough to have been pushed: touching half an hour before the end
    let mut old_fronts = Vec::new();
    for _ in 0..120_000 {
        if sim.gf().0 == 90_000 {
            for a in 0..n as u8 {
                for b in a + 1..n as u8 {
                    if touching(&sim, a, b) {
                        old_fronts.push((a, b));
                    }
                }
            }
        }
        let mut cmds = Vec::new();
        for ai in ais.iter_mut() {
            cmds.extend(ai.think(&sim));
        }
        for c in &cmds {
            if matches!(c.cmd, rr_sim::Cmd::Attack { .. }) {
                attacks[c.player as usize] += 1;
            }
        }
        sim.step(&cmds);
    }
    let towers = sim.buildings().iter().filter(|(_, b)| matches!(b.kind, B::Watchtower | B::Fortress)).count();
    // every old front still there: at least one side sees a military building of the other
    let mut blind = Vec::new();
    for &(a, b) in &old_fronts {
        {
            if !sim.players()[a as usize].alive || !sim.players()[b as usize].alive {
                continue;
            }
            let touch = touching(&sim, a, b);
            let sees = |x: u8, y: u8| {
                sim.buildings().iter().any(|(_, o)| {
                    o.player == y
                        && sim.rules().building(o.kind).is_military()
                        && sim.visibility(x, o.pos) == rr_sim::Visibility::Visible
                })
            };
            // (a front can't be pushed across a range where nothing can be built, nor by a realm
            // with its last few soldiers: the posts it places there stand empty)
            let room = |x: u8, y: u8| {
                let w = sim.world();
                if sim.player_inventory(x).soldiers() < 4 {
                    return false;
                }
                let g = w.grid();
                g.coords().any(|p| {
                    w.owner(p) == Some(x)
                        && !matches!(w.bq_for(p, x), rr_world::Bq::Nothing | rr_world::Bq::Flag)
                        && g.within(p, 6).any(|q| w.owner(q) == Some(y))
                })
            };
            // being pushed: a military building (site or ready) placed since then by the other's land
            let pushing = |x: u8, y: u8| {
                let w = sim.world();
                let g = w.grid();
                sim.buildings().iter().any(|(_, o)| {
                    o.player == x
                        && o.placed() >= 90_000
                        && sim.rules().building(o.kind).military.is_some()
                        && g.within(o.pos, 4).any(|q| w.owner(q) == Some(y))
                })
            };
            if touch && !sees(a, b) && !sees(b, a) && (room(a, b) || room(b, a)) && !pushing(a, b) && !pushing(b, a) {
                blind.push((a, b));
            }
        }
    }
    eprintln!(
        "seed {seed}: blind fronts {blind:?} of {old_fronts:?}; attacks {attacks:?}, towers and fortresses {towers}"
    );
    let lookouts = sim.buildings().iter().filter(|(_, b)| b.kind == B::LookoutTower).count();
    (blind, old_fronts.len(), lookouts)
}

/// Veteran round 6, #5: attacks come as RTTR's do, every available soldier at once on its
/// interval, not one or two at a time at the same target every few seconds.
#[test]
#[ignore = "a whole game: tools/eval.sh runs it (cargo test -p rr-ai -- --ignored)"]
fn attacks_come_in_force() {
    let region = rr_mapgen::world::region("europe").unwrap();
    let map = rr_mapgen::region::generate(region, 1);
    let n = map.players();
    let setup = GameSetup { seed: 1, players: (0..n).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &map, &setup).unwrap();
    let mut ais: Vec<rr_ai::Ai> = (0..n as u8).map(rr_ai::Ai::new).collect();
    let mut attacks: Vec<(u32, u8, rr_core::MapCoord, u8)> = Vec::new();
    for _ in 0..100_000 {
        let mut cmds = Vec::new();
        for ai in ais.iter_mut() {
            cmds.extend(ai.think(&sim));
        }
        for c in &cmds {
            if let rr_sim::Cmd::Attack { target, soldiers, .. } = c.cmd {
                attacks.push((sim.gf().0, c.player, target, soldiers));
            }
        }
        sim.step(&cmds);
    }
    assert!(attacks.len() >= 10, "only {} attacks", attacks.len());
    let repeats = attacks
        .iter()
        .enumerate()
        .filter(|(i, (gf, p, t, _))| attacks[..*i].iter().any(|(g2, p2, t2, _)| p2 == p && t2 == t && gf - g2 < 750))
        .count();
    let avg10 = attacks.iter().map(|a| a.3 as usize).sum::<usize>() * 10 / attacks.len();
    assert!(
        repeats * 100 <= attacks.len() * 15,
        "{repeats} of {} attacks repeat a target within 750 GF",
        attacks.len()
    );
    assert!(avg10 >= 30, "{}.{} soldiers per attack", avg10 / 10, avg10 % 10);
}

/// Veteran round 6, #6: with a big farming realm the iron mines grow (RTTR: min(farms / 2,
/// smelters + 1)), and coal is shared evenly (RTTR InitDistribution 10 / 10 / 10), so ore
/// doesn't pile up with the smelters starved of coal.
#[test]
#[ignore = "a whole game: tools/eval.sh runs it (cargo test -p rr-ai -- --ignored)"]
fn ore_and_coal_meet() {
    let region = rr_mapgen::world::region("europe").unwrap();
    let map = rr_mapgen::region::generate(region, 1);
    let n = map.players();
    let setup = GameSetup { seed: 1, players: (0..n).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &map, &setup).unwrap();
    let mut ais: Vec<rr_ai::Ai> = (0..n as u8).map(rr_ai::Ai::new).collect();
    // ore waiting with no coal at all, sampled every 5 000 GF from 50k: a snapshot of one
    // game flips with every change, the share of samples doesn't
    let (mut samples, mut waiting) = (0, Vec::new());
    for _ in 0..100_000 {
        let mut cmds = Vec::new();
        for ai in ais.iter_mut() {
            cmds.extend(ai.think(&sim));
        }
        sim.step(&cmds);
        if sim.gf().0 >= 50_000 && sim.gf().0 % 5000 == 0 {
            for p in 0..n as u8 {
                let inv = sim.player_inventory(p);
                let coal_mines = sim.buildings().iter().filter(|(_, b)| b.player == p && b.kind == B::CoalMine).count();
                let (ore, coal) = (inv.ware(rr_rules::Ware::IronOre), inv.ware(rr_rules::Ware::Coal));
                if coal_mines > 0 && sim.players()[p as usize].alive {
                    samples += 1;
                    if ore > 25 && coal == 0 {
                        waiting.push(format!("p{p} at {}: {ore} ore", sim.gf().0));
                    }
                }
            }
        }
    }
    assert!(waiting.len() * 5 <= samples, "ore waits for coal in {} of {samples} samples: {waiting:?}", waiting.len());
    for p in 0..n as u8 {
        let smelter = sim.players()[p as usize]
            .settings
            .distribution
            .iter()
            .find(|(w, b, _)| *w == rr_rules::Ware::Coal && *b == B::IronSmelter)
            .map(|x| x.2);
        assert_eq!(smelter, Some(10), "p{p}: coal for smelters");
    }
}

/// Veteran round 6, #4 (rest): at a front the AI builds catapults too (RTTR
/// ChooseMilitaryBuilding: one time in eight); it built none in 42 realms.
#[test]
#[ignore = "a whole game: tools/eval.sh runs it (cargo test -p rr-ai -- --ignored)"]
fn catapults_at_the_front() {
    // (North America and Far East: on Europe the fronts leave no medium spot in range; two games,
    // as one decides by a 1-in-12 roll at the right moment)
    let placed: usize = std::thread::scope(|sc| {
        let games: Vec<_> =
            ["northamerica", "fareast"].into_iter().map(|r| sc.spawn(move || catapults_placed(r))).collect();
        games.into_iter().map(|h| h.join().unwrap()).sum()
    });
    assert!(placed > 0, "no catapult placed in 100k GF");
}

/// Catapults the AIs place in 100 000 GF of a game on `name`.
fn catapults_placed(name: &str) -> usize {
    let region = rr_mapgen::world::region(name).unwrap();
    let map = rr_mapgen::region::generate(region, 1);
    let n = map.players();
    let setup = GameSetup { seed: 1, players: (0..n).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &map, &setup).unwrap();
    let mut ais: Vec<rr_ai::Ai> = (0..n as u8).map(rr_ai::Ai::new).collect();
    let mut placed = 0;
    for _ in 0..100_000 {
        let mut cmds = Vec::new();
        for ai in ais.iter_mut() {
            cmds.extend(ai.think(&sim));
        }
        placed += cmds.iter().filter(|c| matches!(c.cmd, rr_sim::Cmd::PlaceBuilding { kind: B::Catapult, .. })).count();
        sim.step(&cmds);
    }
    placed
}

/// Veteran round 7, #2: the AI keeps stone coming and leaves no stone-starved sites standing;
/// fish feeds every mine at full weight.
#[test]
#[ignore = "a whole game: tools/eval.sh runs it (cargo test -p rr-ai -- --ignored)"]
fn stone_keeps_coming() {
    let region = rr_mapgen::world::region("europe").unwrap();
    let map = rr_mapgen::region::generate(region, 1);
    let n = map.players();
    let setup = GameSetup { seed: 1, players: (0..n).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &map, &setup).unwrap();
    let mut ais: Vec<rr_ai::Ai> = (0..n as u8).map(rr_ai::Ai::new).collect();
    for _ in 0..100_000 {
        let mut cmds = Vec::new();
        for ai in ais.iter_mut() {
            cmds.extend(ai.think(&sim));
        }
        sim.step(&cmds);
    }
    for p in 0..n as u8 {
        if !sim.players()[p as usize].alive {
            continue;
        }
        let stones = sim.player_inventory(p).ware(rr_rules::Ware::Stones);
        let starved = sim
            .buildings()
            .iter()
            .filter(|(_, b)| b.player == p && !b.is_ready() && sim.rules().building(b.kind).stones > 0)
            .count();
        assert!(stones > 0 || starved <= 2, "p{p}: 0 stones and {starved} sites needing stone");
        for mine in [B::CoalMine, B::IronMine, B::GoldMine, B::GraniteMine] {
            let fish = sim.players()[p as usize]
                .settings
                .distribution
                .iter()
                .find(|(w, b, _)| *w == rr_rules::Ware::Fish && *b == mine)
                .map(|x| x.2);
            assert_eq!(fish, Some(10), "p{p}: fish for {mine:?}");
        }
    }
}

/// Veteran round 7, #4: an AI whose headquarters fell builds on from its other warehouses
/// (North America p1 kept three harbors and placed nothing for the rest of the game).
#[test]
#[ignore = "a whole game: tools/eval.sh runs it (cargo test -p rr-ai -- --ignored)"]
fn a_realm_without_its_hq_builds_on() {
    // (three games: in one, a realm's sawmill was cut off from its last warehouse with the
    // headquarters, and with no board left it could place nothing; item 461)
    let games: Vec<Vec<(u8, u32)>> = std::thread::scope(|sc| {
        let hs: Vec<_> = [1, 2, 3].into_iter().map(|seed| sc.spawn(move || headless_builds(seed))).collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    eprintln!("headless realms and what they placed, per game: {games:?}");
    assert!(games.iter().any(|g| !g.is_empty()), "no realm with a second warehouse by GF 40k");
    let all: Vec<&(u8, u32)> = games.iter().flatten().collect();
    let idle = all.iter().filter(|(_, n)| *n == 0).count();
    assert!(idle <= 1, "headless realms placing nothing: {games:?}");
}

/// The realms that lost their headquarters at GF 40k (having another warehouse) and lived, with
/// how many buildings each placed afterwards.
fn headless_builds(seed: u64) -> Vec<(u8, u32)> {
    let region = rr_mapgen::world::region("mediterranean").unwrap();
    let map = rr_mapgen::region::generate(region, seed);
    let n = map.players();
    let setup = GameSetup { seed, players: (0..n).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &map, &setup).unwrap();
    let mut ais: Vec<rr_ai::Ai> = (0..n as u8).map(rr_ai::Ai::new).collect();
    let mut headless = Vec::new();
    let mut placed = vec![0u32; n];
    for _ in 0..60_000 {
        if sim.gf().0 == 40_000 {
            // every realm with another warehouse loses its headquarters
            for p in 0..n as u8 {
                let mine = |k: B| {
                    sim.buildings()
                        .iter()
                        .find(|(_, b)| b.player == p && b.kind == k && b.is_ready())
                        .map(|(_, b)| b.pos)
                };
                let other = sim
                    .buildings()
                    .iter()
                    .any(|(_, b)| b.player == p && b.is_ready() && b.inventory.is_some() && b.kind != B::Headquarters);
                if let (Some(hq), true) = (mine(B::Headquarters), other) {
                    sim.debug_destroy_building(hq);
                    headless.push(p);
                }
            }
        }
        let mut cmds = Vec::new();
        for ai in ais.iter_mut() {
            cmds.extend(ai.think(&sim));
        }
        if sim.gf().0 > 40_000 {
            for c in &cmds {
                if matches!(c.cmd, rr_sim::Cmd::PlaceBuilding { .. }) {
                    placed[c.player as usize] += 1;
                }
            }
        }
        sim.step(&cmds);
    }
    // (a realm conquered meanwhile places nothing either)
    headless.retain(|p| sim.players()[*p as usize].alive);
    headless.into_iter().map(|p| (p, placed[p as usize])).collect()
}

/// Veteran round 7, #5: AI road networks get loops (RTTR BuildAlternativeRoad), so a ware
/// needn't go the long way round through one hub flag; there were none in 15 networks.
#[test]
#[ignore = "a whole game: tools/eval.sh runs it (cargo test -p rr-ai -- --ignored)"]
fn road_networks_have_loops() {
    let region = rr_mapgen::world::region("europe").unwrap();
    let map = rr_mapgen::region::generate(region, 1);
    let n = map.players();
    let setup = GameSetup { seed: 1, players: (0..n).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &map, &setup).unwrap();
    let mut ais: Vec<rr_ai::Ai> = (0..n as u8).map(rr_ai::Ai::new).collect();
    for _ in 0..60_000 {
        let mut cmds = Vec::new();
        for ai in ais.iter_mut() {
            cmds.extend(ai.think(&sim));
        }
        sim.step(&cmds);
    }
    let mut looped = 0;
    for p in 0..n as u8 {
        // loops = roads − flags + connected parts
        let flags: Vec<_> = sim.flags().iter().filter(|(_, f)| f.player == p).map(|(id, _)| id).collect();
        let roads = sim.roads().iter().filter(|(_, r)| r.player == p).count();
        let mut parts = 0;
        let mut seen = std::collections::BTreeSet::new();
        for f in &flags {
            if seen.insert(*f) {
                parts += 1;
                for g in &flags {
                    if sim.flag_distance(*f, *g).is_some() {
                        seen.insert(*g);
                    }
                }
            }
        }
        let loops = (roads + parts).saturating_sub(flags.len());
        eprintln!("p{p}: {} flags, {roads} roads, {loops} loops", flags.len());
        if loops >= 3 {
            looped += 1;
        }
    }
    assert!(looped * 2 >= n, "{looped} of {n} networks have three loops or more");
}

/// Veteran round 7, #6: coins go to the front, not far inland (RTTR MilUpgradeOptim): 239 of
/// 386 coins went to far-inland buildings.
#[test]
#[ignore = "a whole game: tools/eval.sh runs it (cargo test -p rr-ai -- --ignored)"]
fn coins_go_to_the_front() {
    let region = rr_mapgen::world::region("europe").unwrap();
    let map = rr_mapgen::region::generate(region, 1);
    let n = map.players();
    let setup = GameSetup { seed: 1, players: (0..n).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &map, &setup).unwrap();
    let mut ais: Vec<rr_ai::Ai> = (0..n as u8).map(rr_ai::Ai::new).collect();
    for _ in 0..80_000 {
        let mut cmds = Vec::new();
        for ai in ais.iter_mut() {
            cmds.extend(ai.think(&sim));
        }
        sim.step(&cmds);
    }
    let (mut inland, mut front) = (0, 0);
    for p in 0..n as u8 {
        let mine: Vec<_> = sim
            .buildings()
            .iter()
            .filter(|(_, b)| b.player == p && b.is_ready() && sim.rules().building(b.kind).is_military())
            .filter(|(_, b)| b.kind != B::Headquarters)
            .map(|(id, b)| (sim.frontier_distance(id), b))
            .collect();
        if !mine.iter().any(|(d, _)| *d == 3) {
            continue;
        }
        let mut open = 0;
        for (d, b) in &mine {
            let coins = b.stock[0] + b.incoming[0];
            // (2: by an own harbor; with no attacks over the sea as far inland as 0, veteran
            // round 8, #6: 121 of 254 generals sat there)
            if *d == 0 || *d == 2 {
                inland += coins;
                open += usize::from(!b.coins_stopped);
            } else {
                front += coins;
            }
        }
        // (one toggle a few thoughts apart: a building that just turned inland may lag)
        assert!(open <= 2, "p{p}: coins open at {open} buildings far inland");
    }
    eprintln!("coins inland {inland}, at the fronts {front}");
    assert!(front >= inland, "coins inland {inland}, at the fronts {front}");
}

/// Veteran round 7, #7: the AI keeps its harbor spots free for harbors; it built over them
/// (13 of 98 owned spots still buildable).
#[test]
#[ignore = "a whole game: tools/eval.sh runs it (cargo test -p rr-ai -- --ignored)"]
fn harbor_spots_stay_free() {
    let region = rr_mapgen::world::region("europe").unwrap();
    let map = rr_mapgen::region::generate(region, 1);
    let n = map.players();
    let setup = GameSetup { seed: 1, players: (0..n).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &map, &setup).unwrap();
    let mut ais: Vec<rr_ai::Ai> = (0..n as u8).map(rr_ai::Ai::new).collect();
    for _ in 0..60_000 {
        let mut cmds = Vec::new();
        for ai in ais.iter_mut() {
            cmds.extend(ai.think(&sim));
        }
        sim.step(&cmds);
    }
    let w = sim.world();
    let g = w.grid();
    let (mut owned, mut taken) = (0, 0);
    for p in g.coords().filter(|p| w.is_harbor_spot(*p)) {
        let Some(o) = w.owner(p) else { continue };
        owned += 1;
        // a building of the owner on the spot or right by it (other than a harbor there)
        let harbor = sim.buildings().iter().any(|(_, b)| b.pos == p && b.kind == B::Harbor);
        if !harbor
            && sim
                .buildings()
                .iter()
                .any(|(_, b)| b.player == o && b.kind != B::Headquarters && g.distance(b.pos, p) <= 1)
        {
            taken += 1;
        }
    }
    eprintln!("{taken} of {owned} owned harbor spots built over");
    assert!(owned > 0);
    assert!(taken == 0, "{taken} of {owned} owned harbor spots built over");
}

/// Veteran round 8, #2: woodcutters stay in step with the foresters (RTTR BuildingPlanner:
/// at most 2 × foresters + 1), and the mines get their build slots (60-90 woodcutters at
/// 10-25 % took them, North America p5 had no mine on 748 coal).
#[test]
#[ignore = "a whole game: tools/eval.sh runs it (cargo test -p rr-ai -- --ignored)"]
fn woodcutters_in_step_with_foresters() {
    let region = rr_mapgen::world::region("northamerica").unwrap();
    let map = rr_mapgen::region::generate(region, 1);
    let n = map.players();
    let setup = GameSetup { seed: 1, players: (0..n).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &map, &setup).unwrap();
    let mut ais: Vec<rr_ai::Ai> = (0..n as u8).map(rr_ai::Ai::new).collect();
    // coal ground without a coal mine (site or ready), sampled every 10 000 GF from 70k: a
    // realm rebuilding after a war may lack one for a while, not for good
    let mut no_mine = vec![Vec::new(); n];
    for _ in 0..150_000 {
        let mut cmds = Vec::new();
        for ai in ais.iter_mut() {
            cmds.extend(ai.think(&sim));
        }
        sim.step(&cmds);
        if sim.gf().0 >= 70_000 && sim.gf().0 % 10_000 == 0 {
            let w = sim.world();
            let g = w.grid();
            for p in 0..n as u8 {
                let c = |k: B| sim.buildings().iter().filter(|(_, b)| b.player == p && b.kind == k).count();
                let coal = g
                    .coords()
                    .filter(|q| {
                        w.owner(*q) == Some(p) && matches!(w.resources[g.index(*q)], rr_core::Resource::Coal(_))
                    })
                    .count();
                let boards = sim.player_inventory(p).ware(rr_rules::Ware::Boards);
                if sim.players()[p as usize].alive && coal >= 20 && c(B::CoalMine) == 0 && boards >= 10 {
                    no_mine[p as usize].push(sim.gf().0);
                }
            }
        }
    }
    for p in 0..n as u8 {
        if !sim.players()[p as usize].alive {
            continue;
        }
        let c = |k: B| sim.buildings().iter().filter(|(_, b)| b.player == p && b.kind == k).count();
        let (cutters, foresters) = (c(B::Woodcutter), c(B::Forester));
        assert!(cutters <= 2 * foresters + 4, "p{p}: {cutters} woodcutters for {foresters} foresters");
        assert!(
            no_mine[p as usize].len() <= 2,
            "p{p}: coal ground, boards and no coal mine at {:?}",
            no_mine[p as usize]
        );
    }
}

/// Veteran round 8, #1: a beaten realm is finished off. North p3 had no soldier left for an
/// hour while p2, 76 soldiers next to it, never attacked: the AI wanted every counted post to
/// reach the target, and priced the privates in an HQ as sergeants.
#[test]
#[ignore = "a whole game: tools/eval.sh runs it (cargo test -p rr-ai -- --ignored)"]
fn beaten_realms_are_finished_off() {
    let region = rr_mapgen::world::region("north").unwrap();
    let map = rr_mapgen::region::generate(region, 1);
    let n = map.players();
    let setup = GameSetup { seed: 1, players: (0..n).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &map, &setup).unwrap();
    let mut ais: Vec<rr_ai::Ai> = (0..n as u8).map(rr_ai::Ai::new).collect();
    // samples in a row (every 6 000 GF, 5 minutes) a realm stood defenseless within a rival's reach
    let mut open = vec![0; n];
    for _ in 0..200_000 {
        let mut cmds = Vec::new();
        for ai in ais.iter_mut() {
            cmds.extend(ai.think(&sim));
        }
        sim.step(&cmds);
        if sim.gf().0 % 6000 != 0 || sim.gf().0 < 60_000 {
            continue;
        }
        for p in 0..n as u8 {
            let garrison: usize =
                sim.buildings().iter().filter(|(_, b)| b.player == p).map(|(_, b)| b.soldiers.len()).sum();
            let soldiers = garrison + sim.player_inventory(p).soldiers() as usize;
            let hq = sim.players()[p as usize].hq.and_then(|h| sim.buildings().get(h)).map(|b| b.pos);
            let reachable = hq.is_some_and(|at| {
                (0..n as u8).any(|q| q != p && sim.players()[q as usize].alive && sim.attackers_for(q, at) >= 3)
            });
            open[p as usize] =
                if sim.players()[p as usize].alive && soldiers <= 1 && reachable { open[p as usize] + 1 } else { 0 };
            assert!(open[p as usize] < 4, "p{p} defenseless within reach for 20 minutes at GF {}", sim.gf().0);
        }
    }
}

/// The weapon chain keeps going for 200 minutes: the food chain grows with the realm (RTTR
/// BuildingPlanner: mills after the farms, a bakery per mill, pig farms by the farms, fisheries
/// by the posts), so the mines are fed. Mines ran at 2-46 %; median 41 swords in 200 minutes
/// over four games, 57 with the farms ahead of the mines (item 306).
#[test]
#[ignore = "a whole game: tools/eval.sh runs it (cargo test -p rr-ai -- --ignored)"]
fn swords_keep_coming() {
    // four games: one or two games' few realms decide too much by chance (seeds 1-2 gave a
    // median of 72, seeds 1-4 give 41)
    let mut swords: Vec<u32> = std::thread::scope(|sc| {
        let games: Vec<_> = [1, 2, 3, 4].into_iter().map(|seed| sc.spawn(move || swords_made(seed))).collect();
        games.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    swords.sort_unstable();
    eprintln!("swords made by living realms with an armory: {swords:?}");
    assert!(swords.len() >= 8, "{swords:?}");
    // (a guard against going back; item 306 wants far more)
    // (the median moves between 39 and 57 with small map or AI changes: a guard at 35)
    assert!(swords[swords.len() / 2] >= 35, "median swords {swords:?}");
}

/// Swords made in 200 minutes of a Europe game by each living realm with an armory.
fn swords_made(seed: u64) -> Vec<u32> {
    let region = rr_mapgen::world::region("europe").unwrap();
    let map = rr_mapgen::region::generate(region, seed);
    let n = map.players();
    let setup = GameSetup { seed, players: (0..n).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &map, &setup).unwrap();
    let mut ais: Vec<rr_ai::Ai> = (0..n as u8).map(rr_ai::Ai::new).collect();
    for _ in 0..240_000 {
        let mut cmds = Vec::new();
        for ai in ais.iter_mut() {
            cmds.extend(ai.think(&sim));
        }
        sim.step(&cmds);
    }
    (0..n)
        .filter(|p| sim.players()[*p].alive)
        .filter(|p| sim.buildings().iter().any(|(_, b)| b.player as usize == *p && b.kind == B::Armory))
        .map(|p| sim.players()[p].stats.samples.last().unwrap().by_ware[rr_rules::Ware::Sword.index()])
        .collect()
}

/// Veteran round 8, #4: realms grow while they have soldiers to spare, more than one military
/// site at a time (RTTR WantMoreMilitaryBlds: up to the military buildings + 3). Half the map
/// stayed empty at minute 200, with 70 privates waiting in store.
#[test]
#[ignore = "a whole game: tools/eval.sh runs it (cargo test -p rr-ai -- --ignored)"]
fn the_map_fills_up() {
    // (two seeds per map, their mean: one game moved by 3 % with any AI change, item 461)
    let shares: Vec<(String, u32)> = std::thread::scope(|sc| {
        let games: Vec<_> = ["europe", "northamerica"]
            .into_iter()
            .map(|name| {
                let a = sc.spawn(move || owned_share(name, 1));
                let b = sc.spawn(move || owned_share(name, 2));
                (name, a, b)
            })
            .collect();
        games.into_iter().map(|(n, a, b)| (n.to_string(), (a.join().unwrap() + b.join().unwrap()) / 2)).collect()
    });
    eprintln!("walkable land owned at GF 150k, percent (mean of seeds 1 and 2): {shares:?}");
    // (a guard: 47 and 42 % before items 298-299, 45 and 47 % after, within the noise of one
    // game; the review wants 70 % by minute 200, item 308)
    assert!(shares.iter().all(|(_, s)| *s >= 43), "{shares:?}");
}

/// The percentage of walkable land some realm owns after 150 000 GF of a game of AIs.
fn owned_share(name: &str, seed: u64) -> u32 {
    let region = rr_mapgen::world::region(name).unwrap();
    let map = rr_mapgen::region::generate(region, seed);
    let n = map.players();
    let setup = GameSetup { seed, players: (0..n).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &map, &setup).unwrap();
    let mut ais: Vec<rr_ai::Ai> = (0..n as u8).map(rr_ai::Ai::new).collect();
    for _ in 0..150_000 {
        let mut cmds = Vec::new();
        for ai in ais.iter_mut() {
            cmds.extend(ai.think(&sim));
        }
        sim.step(&cmds);
    }
    let w = sim.world();
    let walk: Vec<_> = w.grid().coords().filter(|p| w.node_walkable(*p)).collect();
    (walk.iter().filter(|p| w.owner(**p).is_some()).count() * 100 / walk.len()) as u32
}

/// New-player round 8, #8: easy rivals bring no catapults.
#[test]
#[ignore = "a whole game: tools/eval.sh runs it (cargo test -p rr-ai -- --ignored)"]
fn easy_brings_no_catapults() {
    let region = rr_mapgen::world::region("northamerica").unwrap();
    let map = rr_mapgen::region::generate(region, 1);
    let n = map.players();
    let setup = GameSetup { seed: 1, players: (0..n).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &map, &setup).unwrap();
    let mut ais: Vec<rr_ai::Ai> = (0..n as u8).map(|p| rr_ai::Ai::with_level(p, rr_ai::Level::Easy)).collect();
    for _ in 0..100_000 {
        let mut cmds = Vec::new();
        for ai in ais.iter_mut() {
            cmds.extend(ai.think(&sim));
        }
        assert!(
            !cmds.iter().any(|c| matches!(c.cmd, rr_sim::Cmd::PlaceBuilding { kind: B::Catapult, .. })),
            "a catapult"
        );
        sim.step(&cmds);
    }
}

/// QA round 5, #2: an expedition isn't sent again and again to a harbor spot it can't land on
/// (a rival owns its flag node): Mediterranean seed 5 sent one 203 times.
#[test]
#[ignore = "a whole game: tools/eval.sh runs it (cargo test -p rr-ai -- --ignored)"]
fn expeditions_go_where_they_can_land() {
    let region = rr_mapgen::world::region("mediterranean").unwrap();
    let map = rr_mapgen::region::generate(region, 5);
    let n = map.players();
    let setup = GameSetup { seed: 5, players: (0..n).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &map, &setup).unwrap();
    let mut ais: Vec<rr_ai::Ai> = (0..n as u8).map(rr_ai::Ai::new).collect();
    let mut sends = 0;
    for _ in 0..150_000 {
        let mut cmds = Vec::new();
        for ai in ais.iter_mut() {
            cmds.extend(ai.think(&sim));
        }
        sends += cmds.iter().filter(|c| matches!(c.cmd, rr_sim::Cmd::SendExpedition { .. })).count();
        sim.step(&cmds);
    }
    assert!(sends < 60, "{sends} expeditions sent");
}

/// Item 307 (veteran round 8, #6): promoted soldiers go to the fronts. 121 of 254 generals sat
/// by own harbors where nobody can attack, Europe p3's front held privates only.
#[test]
#[ignore = "a whole game: tools/eval.sh runs it (cargo test -p rr-ai -- --ignored)"]
fn generals_go_to_the_front() {
    let region = rr_mapgen::world::region("europe").unwrap();
    let map = rr_mapgen::region::generate(region, 1);
    let n = map.players();
    let setup = GameSetup { seed: 1, players: (0..n).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &map, &setup).unwrap();
    let mut ais: Vec<rr_ai::Ai> = (0..n as u8).map(rr_ai::Ai::new).collect();
    for _ in 0..150_000 {
        let mut cmds = Vec::new();
        for ai in ais.iter_mut() {
            cmds.extend(ai.think(&sim));
        }
        sim.step(&cmds);
    }
    // officers and generals in front posts against those far inland, in realms at war (a realm
    // without a front has every post "inland")
    let posts = |id: rr_sim::BuildingId, b: &rr_sim::Building| {
        b.is_ready()
            && b.kind != B::Headquarters
            && sim.rules().building(b.kind).is_military()
            && id.index() < usize::MAX
    };
    let at_war: Vec<u8> = (0..n as u8)
        .filter(|p| {
            sim.buildings().iter().any(|(id, b)| b.player == *p && posts(id, b) && sim.frontier_distance(id) == 3)
        })
        .collect();
    let (mut front, mut back) = (0, 0);
    for (id, b) in sim.buildings().iter() {
        // (a post whose only soldier is a general can't send him: item 347, a swap)
        if !posts(id, b) || !at_war.contains(&b.player) || b.soldiers.len() < 2 {
            continue;
        }
        let high = b.soldiers.iter().filter(|f| sim.figure(**f).job.soldier_rank().is_some_and(|r| r >= 3)).count();
        match sim.frontier_distance(id) {
            3 => front += high,
            0 | 2 => back += high,
            _ => {}
        }
    }
    eprintln!("officers and generals: {front} at the fronts, {back} inland or by harbors");
    assert!(front >= back, "{front} at the fronts, {back} inland");
}

/// Veteran round 9, #6: no castle sniped in the first 40 minutes of a free game (North p4 fell
/// at minute 15 with its army out attacking and 2 soldiers at home).
#[test]
#[ignore = "a whole game: tools/eval.sh runs it (cargo test -p rr-ai -- --ignored)"]
fn no_castle_falls_early() {
    // (and the games of veteran round 10, #2: one map and seed missed five early falls)
    let mut dead = Vec::new();
    for (map, seed) in
        [("north", 1), ("northamerica", 1), ("northamerica", 2), ("fareast", 2), ("fareast", 3), ("europe", 3)]
    {
        let region = rr_mapgen::world::region(map).unwrap();
        let m = rr_mapgen::region::generate(region, seed);
        let n = m.players();
        let setup = GameSetup { seed, players: (0..n).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
        let mut sim = Sim::new(Arc::new(Rules::classic()), &m, &setup).unwrap();
        let mut ais: Vec<rr_ai::Ai> = (0..n as u8).map(rr_ai::Ai::new).collect();
        for _ in 0..48_000 {
            let mut cmds = Vec::new();
            for ai in ais.iter_mut() {
                cmds.extend(ai.think(&sim));
            }
            sim.step(&cmds);
        }
        dead.extend((0..n).filter(|p| !sim.players()[*p].alive).map(|p| (map, seed, p)));
    }
    assert!(dead.is_empty(), "defeated in the first 40 minutes: {dead:?}");
}

/// Eval 46: in a campaign (a truce with the player) normal rivals attack the player at most
/// every 4 minutes each: five allies at normal pace overran a newcomer after the first win.
#[test]
#[ignore = "a whole game: tools/eval.sh runs it (cargo test -p rr-ai -- --ignored)"]
fn campaign_rivals_pace_their_attacks() {
    // normal rivals: spaced out; easy ones: they still come (new-player round 9, #1: the window
    // of item 354 never met the easy AI's attack thought)
    let normal = campaign_attacks_on_player(rr_ai::Level::Normal, 2);
    for (p, t) in normal.iter().enumerate() {
        // (one attack round in the first part of every 4-minute window: 4 100 GF apart at least)
        assert!(t.windows(2).all(|w| w[1] - w[0] >= 4 * 60 * 20 - 700), "p{p} attacks the player at {t:?}");
    }
    // (over three games: easy rivals attack about once in 90 minutes, and one game decides
    // that by chance; veteran round 10's spare-soldier fix took seed 2's one attack)
    let easy: usize = (1..=3)
        .map(|seed| campaign_attacks_on_player(rr_ai::Level::Easy, seed).iter().map(|t| t.len()).sum::<usize>())
        .sum();
    assert!(easy >= 2, "easy rivals attacked {easy} times in three games");
}

/// When each rival attacks the player in 90 minutes of a campaign (Europe, `seed`) with rivals
/// of `level` and the newcomer an easy AI.
fn campaign_attacks_on_player(level: rr_ai::Level, seed: u64) -> Vec<Vec<u32>> {
    let region = rr_mapgen::world::region("europe").unwrap();
    let map = rr_mapgen::region::generate(region, seed);
    let n = map.players();
    let setup = GameSetup::campaign(seed, n);
    let mut sim = Sim::new(Arc::new(Rules::classic()), &map, &setup).unwrap();
    let mut ais: Vec<rr_ai::Ai> = (0..n as u8).map(|p| rr_ai::Ai::with_level(p, level)).collect();
    ais[0] = rr_ai::Ai::with_level(0, rr_ai::Level::Easy);
    let truce = if level == rr_ai::Level::Easy { 2 * rr_ai::MISSION_TRUCE } else { rr_ai::MISSION_TRUCE };
    for ai in ais.iter_mut().skip(1) {
        ai.truce = Some((0, truce));
    }
    let mut on_player: Vec<Vec<u32>> = vec![Vec::new(); n];
    for _ in 0..108_000 {
        let mut cmds = Vec::new();
        for ai in ais.iter_mut() {
            cmds.extend(ai.think(&sim));
        }
        for c in &cmds {
            if let rr_sim::Cmd::Attack { target, .. } = c.cmd
                && c.player != 0
                && sim.building_at(target).is_some_and(|b| sim.building(b).player == 0)
            {
                on_player[c.player as usize].push(sim.gf().0);
            }
        }
        sim.step(&cmds);
    }
    on_player
}

/// Veteran round 10, #1: every realm with a ready harbor gets a shipyard: Britain's only harbor
/// on Europe seed 1 had no room within 6, so it built none and stood still for 140 minutes with
/// 66 soldiers in store (RTTR: within 7 of any harbor, or another harbor where there is room).
#[test]
#[ignore = "a whole game: tools/eval.sh runs it (cargo test -p rr-ai -- --ignored)"]
fn every_harbor_realm_gets_a_shipyard() {
    // (Europe seeds 1 and 3, as the report; a realm out of boards waits for them, rightly)
    let mut without = Vec::new();
    for seed in [1, 3] {
        let region = rr_mapgen::world::region("europe").unwrap();
        let map = rr_mapgen::region::generate(region, seed);
        let n = map.players();
        let setup = GameSetup { seed, players: (0..n).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
        let mut sim = Sim::new(Arc::new(Rules::classic()), &map, &setup).unwrap();
        let mut ais: Vec<rr_ai::Ai> = (0..n as u8).map(rr_ai::Ai::new).collect();
        for _ in 0..120_000 {
            let mut cmds = Vec::new();
            for ai in ais.iter_mut() {
                cmds.extend(ai.think(&sim));
            }
            sim.step(&cmds);
        }
        let has = |p: u8, k: B, ready: bool| {
            sim.buildings().iter().any(|(_, b)| b.player == p && b.kind == k && (b.is_ready() || !ready))
        };
        without.extend(
            (0..n as u8)
                .filter(|p| {
                    sim.players()[*p as usize].alive && has(*p, B::Harbor, true) && !has(*p, B::Shipyard, false)
                })
                .filter(|p| sim.player_inventory(*p).ware(rr_rules::Ware::Boards) >= 30)
                // (a harbor on its way is the answer when no shipyard fits by the first)
                .filter(|p| {
                    !sim.buildings().iter().any(|(_, b)| b.player == *p && b.kind == B::Harbor && !b.is_ready())
                })
                .map(|p| (seed, p)),
        );
    }
    assert!(without.is_empty(), "realms with a harbor, boards and no shipyard at minute 100: {without:?}");
}

/// Veteran round 10, #5: with soldiers to spare a realm builds big posts (RTTR
/// ChooseMilitaryBuilding: a guardhouse only under 15 privates or one time in three); Europe p5
/// had 57 guardhouses and 1 fortress at minute 200, and 63 % of target sightings found nobody to
/// send.
#[test]
#[ignore = "a whole game: tools/eval.sh runs it (cargo test -p rr-ai -- --ignored)"]
fn armies_build_big_posts() {
    let region = rr_mapgen::world::region("europe").unwrap();
    let map = rr_mapgen::region::generate(region, 1);
    let n = map.players();
    let setup = GameSetup { seed: 1, players: (0..n).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &map, &setup).unwrap();
    let mut ais: Vec<rr_ai::Ai> = (0..n as u8).map(rr_ai::Ai::new).collect();
    for _ in 0..144_000 {
        let mut cmds = Vec::new();
        for ai in ais.iter_mut() {
            cmds.extend(ai.think(&sim));
        }
        sim.step(&cmds);
    }
    let count = |k: B| sim.buildings().iter().filter(|(_, b)| b.kind == k).count();
    let (small, big) = (count(B::Guardhouse) + count(B::Barracks), count(B::Watchtower) + count(B::Fortress));
    eprintln!(
        "posts at minute 120: small {small}, watchtowers {}, fortresses {}",
        count(B::Watchtower),
        count(B::Fortress)
    );
    assert!(big * 3 >= small, "big posts {big} against {small} small");
}

/// Veteran round 10, #8: big armies at a blind front fight: Mare Nostrum seed 1's p0 and p1,
/// with 91 and 99 soldiers in their posts and 25 nodes of border, attacked each other 5 times in
/// 200 minutes, their nearest posts 17 apart and neither seeing the other's.
#[test]
#[ignore = "a whole game: tools/eval.sh runs it (cargo test -p rr-ai -- --ignored)"]
fn blind_fronts_get_eyes_and_fight() {
    // (six seeds, not one, and their sum: a single game is chaotic. Seed 1 alone held it, and on
    // 2026-10-27 three changes elsewhere moved single games between 0 and 54 while all their
    // games fought as much as before: [54, 0, 5, 0, 6, 0], [0, 0, 11, 0, 39, 1], [0, 0, 4, 0, 37, 0])
    let counts: Vec<u32> = std::thread::scope(|sc| {
        let games: Vec<_> = (1..=6).map(|seed| sc.spawn(move || attacks_p0_p1(seed))).collect();
        games.into_iter().map(|g| g.join().unwrap()).collect()
    });
    eprintln!("attacks between p0 and p1 by seed: {counts:?}");
    let total: u32 = counts.iter().sum();
    assert!(total >= 30, "p0 and p1 attacked each other {total} times in six games of 200 minutes: {counts:?}");
}

/// How often p0 and p1 of Mare Nostrum (seed `seed`) attack each other in 200 minutes.
fn attacks_p0_p1(seed: u64) -> u32 {
    let region = rr_mapgen::world::region("mediterranean").unwrap();
    let map = rr_mapgen::region::generate(region, seed);
    let n = map.players();
    let setup = GameSetup { seed, players: (0..n).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &map, &setup).unwrap();
    let mut ais: Vec<rr_ai::Ai> = (0..n as u8).map(rr_ai::Ai::new).collect();
    let mut between = 0;
    for _ in 0..240_000 {
        let mut cmds = Vec::new();
        for ai in ais.iter_mut() {
            cmds.extend(ai.think(&sim));
        }
        for c in &cmds {
            if let rr_sim::Cmd::Attack { target, .. } = c.cmd
                && let Some(b) = sim.building_at(target)
                && matches!((c.player, sim.building(b).player), (0, 1) | (1, 0))
            {
                between += 1;
            }
        }
        sim.step(&cmds);
    }
    between
}
