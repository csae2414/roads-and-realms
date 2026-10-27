mod common;
use common::*;
use rr_core::Direction::*;
use rr_core::{MapCoord, Resource};
use rr_rules::{BuildingType as B, Job, Ware};
use rr_sim::{Cmd, Event, Sim};

fn ok(sim: &mut Sim, cmd: Cmd) {
    let out = exec(sim, 0, cmd.clone());
    assert!(out.events.iter().all(|e| !matches!(e, Event::CommandRejected { .. })), "{cmd:?}: {:?}", out.events);
}

/// Build `kind` at `at` and connect its flag to the HQ flag with a straight road along E.
fn build_east(sim: &mut Sim, kind: B, at: MapCoord) -> rr_sim::BuildingId {
    ok(sim, Cmd::PlaceBuilding { at, kind });
    let b = sim.building_at(at).unwrap();
    let f = sim.flag(sim.building(b).flag).pos;
    let hq_flag = walk(sim, c(20, 20), SE, 1);
    let dirs = sim.road_path(0, f, hq_flag, &[]).expect("road");
    ok(sim, Cmd::BuildRoad { from: f, dirs });
    b
}

fn ready(sim: &mut Sim, b: rr_sim::BuildingId, max: u32) {
    run_until(sim, max, |s| s.building(b).is_ready() && s.building(b).worker.is_some())
        .unwrap_or_else(|| panic!("{:?} not running:\n{}", sim.building(b).kind, sim.dump_string()));
}

#[test]
fn farm_mill_bakery_well_make_bread() {
    let mut sim = sim1();
    sim.debug_add_ware(0, Ware::Scythe, 2);
    sim.debug_add_ware(0, Ware::Rollingpin, 2);
    let farm = build_east(&mut sim, B::Farm, c(26, 16));
    let mill = build_east(&mut sim, B::Mill, c(25, 22));
    let bakery = build_east(&mut sim, B::Bakery, c(16, 22));
    let well = build_east(&mut sim, B::Well, c(15, 18));
    for b in [farm, mill, bakery, well] {
        ready(&mut sim, b, 20_000);
    }
    let bread = sim.player_inventory(0).ware(Ware::Bread);
    run_until(&mut sim, 40_000, |s| s.player_inventory(0).ware(Ware::Bread) > bread).expect("bread arrives");
}

#[test]
fn a_mine_dwindles_but_never_stops() {
    let mut m = rr_core::MapData::flat(64, 64, MEADOW);
    // a mountain east of the HQ with a little coal
    let g = m.grid;
    let mine_at = c(27, 20);
    for p in g.within(mine_at, 3) {
        let i = g.index(p);
        m.terrain_rsu[i] = 1;
        m.terrain_usd[i] = 1;
    }
    // (a node holds 1..=7: 8 units on two nodes)
    let other = g.neighbor(mine_at, NE);
    m.resources[g.index(mine_at)] = Resource::Coal(7);
    m.resources[g.index(other)] = Resource::Coal(1);
    m.hq = vec![Some(c(20, 20))];
    let mut sim = Sim::new(
        std::sync::Arc::new(rr_rules::Rules::classic()),
        &m,
        &rr_sim::GameSetup { seed: 1, players: vec![Default::default()], ..Default::default() },
    )
    .unwrap();
    sim.debug_add_ware(0, Ware::Fish, 200);
    let mine = build_east(&mut sim, B::CoalMine, mine_at);
    ready(&mut sim, mine, 20_000);
    let coal = |s: &Sim| s.players()[0].stats.produced_by_ware[Ware::Coal.index()];
    // owner's playtest 2026-10-27 (RTTR's S4-like exhaustion): 8 units give 40 %, each find
    // takes one, every node keeps its last; under 30 % the player hears it runs low
    let mut low = 0;
    let mut gone = false;
    for _ in 0..200_000 {
        let out = sim.step(&[]);
        low += out.events.iter().filter(|e| matches!(e, Event::MineRunningLow { .. })).count();
        gone |= out.events.iter().any(|e| matches!(e, Event::MineExhausted { .. }));
    }
    for p in [mine_at, other] {
        assert_eq!(sim.world().resources[g.index(p)], Resource::Coal(1), "the last unit stays");
    }
    assert_eq!(low, 1, "told once that it runs low");
    assert!(!gone, "it never stops");
    assert!(sim.building(mine).exhausted, "its window says it runs low");
    // six finds took it down to a unit a node; at 10 % it still finds some now and then
    let at_one = coal(&sim);
    assert!(at_one >= 6, "{at_one}");
    run_until(&mut sim, 200_000, |s| coal(s) > at_one).expect("a find, rare as it is");
}

#[test]
fn armory_makes_swords_and_shields_and_hq_recruits() {
    let mut sim = sim1();
    sim.debug_add_ware(0, Ware::Iron, 4);
    let armory = build_east(&mut sim, B::Armory, c(25, 22));
    ready(&mut sim, armory, 20_000);
    let (s0, sh0) = (sim.player_inventory(0).ware(Ware::Sword), sim.player_inventory(0).ware(Ware::Shield));
    run_until(&mut sim, 30_000, |s| {
        s.player_inventory(0).ware(Ware::Shield) > sh0 && s.player_inventory(0).ware(Ware::Sword) > s0
    })
    .expect("both");
    // recruiting: helpers + sword + shield + beer become privates
    let privates = sim.player_inventory(0).job(Job::Private);
    sim.debug_add_ware(0, Ware::Beer, 3);
    run_until(&mut sim, 2000, |s| s.player_inventory(0).job(Job::Private) > privates).expect("recruits");
}

#[test]
fn geologist_places_signs_and_reports_finds() {
    let mut m = rr_core::MapData::flat(64, 64, MEADOW);
    let g = m.grid;
    for p in g.within(c(26, 22), 4) {
        let i = g.index(p);
        m.terrain_rsu[i] = 1;
        m.terrain_usd[i] = 1;
        m.resources[i] = Resource::Gold(5);
    }
    m.hq = vec![Some(c(20, 20))];
    let mut sim = Sim::new(
        std::sync::Arc::new(rr_rules::Rules::classic()),
        &m,
        &rr_sim::GameSetup { seed: 3, players: vec![Default::default()], ..Default::default() },
    )
    .unwrap();
    let hq_flag = walk(&sim, c(20, 20), SE, 1);
    ok(&mut sim, Cmd::BuildRoad { from: hq_flag, dirs: vec![E, E, E, E] });
    let flag = walk(&sim, hq_flag, E, 4);
    ok(&mut sim, Cmd::SendGeologist { flag });
    let mut found = 0;
    for _ in 0..8000 {
        let out = sim.step(&[]);
        found +=
            out.events.iter().filter(|e| matches!(e, Event::GeologistFound { what: Resource::Gold(_), .. })).count();
    }
    let signs = g.coords().filter(|p| matches!(sim.world().obj(*p), rr_world::NodeObj::Sign(_))).count();
    assert!(signs >= 8, "only {signs} signs");
    assert!(found >= 1, "no gold reported");
}

#[test]
fn hunter_brings_meat() {
    let mut m = rr_core::MapData::flat(64, 64, MEADOW);
    let g = m.grid;
    for p in [c(26, 16), c(28, 18), c(24, 14), c(30, 22), c(27, 24)] {
        m.animals[g.index(p)] = Some(rr_core::Animal::Deer);
    }
    m.hq = vec![Some(c(20, 20))];
    let mut sim = Sim::new(
        std::sync::Arc::new(rr_rules::Rules::classic()),
        &m,
        &rr_sim::GameSetup { seed: 5, players: vec![Default::default()], ..Default::default() },
    )
    .unwrap();
    let hunter = build_east(&mut sim, B::Hunter, c(25, 20));
    ready(&mut sim, hunter, 10_000);
    let meat = sim.player_inventory(0).ware(Ware::Meat);
    run_until(&mut sim, 20_000, |s| s.player_inventory(0).ware(Ware::Meat) > meat).expect("meat");
}

/// Roast finding: a woodcutter whose hut burns on his way to a tree must free the tree.
#[test]
fn burnt_workplace_frees_the_work_spot() {
    let mut sim = sim1();
    let tree = c(28, 17);
    sim.debug_set_obj(tree, rr_world::NodeObj::Tree { species: 0, stage: 3 });
    let at = c(24, 20);
    let b = build_east(&mut sim, B::Woodcutter, at);
    ready(&mut sim, b, 8000);
    run_until(&mut sim, 4000, |s| s.is_work_reserved(tree)).expect("woodcutter heads for the tree");
    sim.debug_destroy_building(at);
    assert!(!sim.is_work_reserved(tree), "tree still reserved after the hut burnt");
}

/// Roast finding: the distribution sliders share scarce wares in proportion (RTTR), they are
/// not a strict priority list.
#[test]
fn scarce_grain_is_shared_by_the_distribution_ratio() {
    let mut sim = sim1();
    sim.debug_take_ware(0, Ware::Grain, 999);
    sim.debug_add_ware(0, Ware::Water, 40);
    ok(&mut sim, Cmd::SetDistribution { ware: Ware::Grain, building: B::Mill, weight: 6 });
    ok(&mut sim, Cmd::SetDistribution { ware: Ware::Grain, building: B::Brewery, weight: 3 });
    let mill = build_east(&mut sim, B::Mill, c(25, 22));
    let brewery = build_east(&mut sim, B::Brewery, c(16, 22));
    ready(&mut sim, mill, 20_000);
    ready(&mut sim, brewery, 20_000);
    // one grain every 400 frames: much less than the two consume
    let mut to: std::collections::BTreeMap<rr_sim::WareId, rr_sim::BuildingId> = Default::default();
    for i in 0..40_000u32 {
        if i % 400 == 0 {
            sim.debug_add_ware(0, Ware::Grain, 1);
        }
        sim.step(&[]);
        for (id, w) in sim.wares().iter() {
            if w.kind == Ware::Grain
                && let Some(g) = w.goal
            {
                to.entry(id).or_insert(g);
            }
        }
    }
    let m = to.values().filter(|g| **g == mill).count();
    let b = to.values().filter(|g| **g == brewery).count();
    assert!(m + b >= 80, "{m} + {b} grains");
    // about 6 : 3; the brewery asks for grain only every other order (it takes turns with
    // water, RTTR nobUsual), so a little less than a third reaches it
    assert!(2 * m >= 3 * b && m <= 3 * b, "mill {m}, brewery {b}");
}

/// Roast finding: bred donkeys walk from the breeder to a warehouse.
#[test]
fn donkey_breeder_donkeys_walk_to_the_warehouse() {
    let mut sim = sim1();
    sim.debug_add_ware(0, Ware::Grain, 10);
    sim.debug_add_ware(0, Ware::Water, 10);
    let breeder = build_east(&mut sim, B::DonkeyBreeder, c(26, 16));
    ready(&mut sim, breeder, 20_000);
    let before = sim.player_inventory(0).job(Job::PackDonkey);
    run_until(&mut sim, 10_000, |s| {
        s.figures()
            .iter()
            .any(|(_, f)| f.job == Job::PackDonkey && f.kind == rr_sim::FigKind::Home && f.inside.is_none())
    })
    .expect("a donkey walks out");
    run_until(&mut sim, 3000, |s| s.player_inventory(0).job(Job::PackDonkey) > before).expect("donkey arrives");
}

/// New-player roast #2: a woodcutter with no trees in reach says so, once, and says it again
/// only after it had work in between.
#[test]
fn a_worker_with_nothing_in_reach_tells_once() {
    let mut sim = sim1();
    let at = c(24, 20);
    let b = build_east(&mut sim, B::Woodcutter, at);
    ready(&mut sim, b, 8000);
    let mut told = 0;
    for _ in 0..30_000 {
        let out = sim.step(&[]);
        told += out
            .events
            .iter()
            .filter(|e| matches!(e, Event::NothingInReach { player: 0, at: p, kind: B::Woodcutter } if *p == at))
            .count();
    }
    assert_eq!(told, 1);
    assert!(sim.building(b).exhausted, "the building window shows the reason");
    // trees again: back to work, the reason goes
    for d in [E, NE, NW] {
        sim.debug_set_obj(walk(&sim, at, d, 3), rr_world::NodeObj::Tree { species: 0, stage: 3 });
    }
    run_until(&mut sim, 20_000, |s| !s.building(b).exhausted).expect("works again");
}

/// New-player round 2, C: the interface can show where a worker goes.
#[test]
fn outside_workers_have_a_work_radius() {
    let sim = sim1();
    let e = &sim.rules().economy;
    assert_eq!(sim.work_radius(B::Forester), Some(e.forester_radius));
    assert_eq!(sim.work_radius(B::Woodcutter), Some(e.woodcutter_radius));
    assert_eq!(sim.work_radius(B::Sawmill), None);
    assert_eq!(sim.work_radius(B::Guardhouse), None);
}

/// Veteran round 3, #4 (RTTR nofFarmhand: rings outward, one ring past the first find): a
/// forester plants next to his hut first, so the wood grows outward from it.
#[test]
fn a_forester_plants_close_to_home_first() {
    let mut sim = sim1();
    let at = c(26, 20);
    let b = build_east(&mut sim, B::Forester, at);
    ready(&mut sim, b, 8000);
    let g = sim.world().grid();
    let tree = |s: &Sim| g.within(at, 8).find(|p| matches!(s.world().obj(*p), rr_world::NodeObj::Tree { .. }));
    run_until(&mut sim, 20_000, |s| tree(s).is_some()).expect("a tree planted");
    let d = g.distance(at, tree(&sim).unwrap());
    // ring 1 touches the hut (no trees there), so ring 2 is the first with a spot: 2 or 3
    assert!(d <= 3, "first tree {d} nodes from the hut");
}

/// Veteran round 3, #8 (RTTR nobUsual: one ware every 210 GF, inputs in turn): a new sawmill
/// doesn't pull all six trunks at once.
#[test]
fn workshops_order_one_ware_at_a_time() {
    let mut sim = sim1();
    sim.debug_add_ware(0, Ware::Wood, 30);
    let mill = build_east(&mut sim, B::Sawmill, c(25, 22));
    run_until(&mut sim, 20_000, |s| s.building(mill).is_ready()).expect("built");
    let ordered = |s: &Sim| {
        let b = s.building(mill);
        b.stock[0] as u32 + b.incoming[0] as u32
    };
    for _ in 0..300 {
        sim.step(&[]);
    }
    assert!(ordered(&sim) <= 2, "{} trunks ordered in 300 GF", ordered(&sim));
    run_until(&mut sim, 3000, |s| ordered(s) >= 6).expect("the slots fill in time");
}

/// QA round 3, #4 (RTTR GamePlayer::FindClientForWare): fresh wares at flags go to any
/// building with a free slot at once; only orders from warehouses wait for the 210-GF turn.
#[test]
fn fresh_wares_fill_free_slots_at_once() {
    let mut sim = sim1();
    let mill = build_east(&mut sim, B::Sawmill, c(25, 22));
    run_until(&mut sim, 20_000, |s| s.building(mill).is_ready()).expect("built");
    sim.debug_take_ware(0, Ware::Wood, 999);
    for _ in 0..500 {
        sim.step(&[]);
    }
    // at the sawmill's flag (at the HQ's flag they would go straight in)
    let at = sim.flag(sim.building(mill).flag).pos;
    sim.debug_drop_ware(0, at, Ware::Wood, 4);
    let ordered = |s: &Sim| s.building(mill).stock[0] as u32 + s.building(mill).incoming[0] as u32;
    run_until(&mut sim, 60, |s| ordered(s) >= 4).unwrap_or_else(|| panic!("only {} on their way", ordered(&sim)));
}

/// Veteran round 4, #5: with every tool slider at 0 a metalworks still makes a tool (a
/// random one, as the original), instead of eating iron and boards for nothing.
#[test]
fn metalworks_with_all_sliders_at_zero_still_makes_tools() {
    let mut sim = sim1();
    let zero: Vec<(Ware, u8)> = [
        Ware::Tongs,
        Ware::Hammer,
        Ware::Axe,
        Ware::Saw,
        Ware::PickAxe,
        Ware::Shovel,
        Ware::Crucible,
        Ware::RodAndLine,
        Ware::Scythe,
        Ware::Cleaver,
        Ware::Rollingpin,
        Ware::Bow,
    ]
    .iter()
    .map(|w| (*w, 0))
    .collect();
    ok(&mut sim, Cmd::SetTools { values: zero });
    sim.debug_add_ware(0, Ware::Iron, 6);
    let mw = build_east(&mut sim, B::Metalworks, c(26, 20));
    ready(&mut sim, mw, 20_000);
    let tools = |s: &Sim| s.players()[0].stats.produced;
    let before = tools(&sim);
    run_until(&mut sim, 20_000, |s| tools(s) > before).expect("a tool is made");
}

/// Veteran round 4, #6: a workshop with nothing to work on shows low productivity, not 100 %.
#[test]
fn a_starved_sawmill_is_not_productive() {
    let mut sim = sim1();
    // a little wood, then none: the sawmill works a while, then waits
    sim.debug_take_ware(0, Ware::Wood, 1000);
    sim.debug_add_ware(0, Ware::Wood, 3);
    let saw = build_east(&mut sim, B::Sawmill, c(26, 20));
    ready(&mut sim, saw, 20_000);
    run_until(&mut sim, 20_000, |s| s.building(saw).productivity() > 0).expect("it worked a while");
    for _ in 0..20_000 {
        sim.step(&[]);
    }
    let p = sim.building(saw).productivity();
    assert!(p <= 10, "starved sawmill shows {p} %");
}

/// Veteran round 4, #8: an inside worker starts as soon as a ware arrives, not up to 100 GF
/// later.
#[test]
fn a_delivered_ware_wakes_the_worker() {
    let mut sim = sim1();
    sim.debug_take_ware(0, Ware::Wood, 1000);
    let saw = build_east(&mut sim, B::Sawmill, c(26, 20));
    ready(&mut sim, saw, 20_000);
    for _ in 0..3000 {
        sim.step(&[]);
    }
    let f = sim.flag(sim.building(saw).flag).pos;
    sim.debug_drop_ware(0, f, Ware::Wood, 1);
    // the ware goes in; then the worker must be working within a few frames
    let arrived = run_until(&mut sim, 2000, |s| s.building(saw).stock[0] > 0 || s.building(saw).productivity() > 0)
        .expect("wood delivered");
    let _ = arrived;
    let worker = sim.building(saw).worker.unwrap();
    // after his wait1, as RTTR's TryToWork (fidelity #5: we started at once, saving 96-558 GF a
    // time in every starved workshop)
    let wait1 = sim.rules().job(Job::Carpenter).wait1;
    let started = run_until(&mut sim, 2000, |s| matches!(s.figure(worker).motion, rr_sim::Motion::Work { .. }));
    assert!(
        started.is_some_and(|n| n + 5 >= wait1 && n <= wait1 + 5),
        "worker started after {started:?} frames, wait1 {wait1}"
    );
}

/// Veteran round 5, #3: forests breed game as in RTTR (noTree: one tree in 20 makes an animal
/// every 6000-8000 GF), however many animals roam elsewhere on the map; the local limit keeps a
/// forest from filling up.
#[test]
fn forests_breed_game_whatever_roams_elsewhere() {
    let mut m = rr_core::MapData::flat(64, 64, MEADOW);
    let g = m.grid;
    // a forest of about 200 grown trees, every other node
    let mut trees = 0;
    for p in g.within(c(40, 40), 11) {
        if (p.x + p.y) % 2 == 0 {
            m.objects[g.index(p)] = rr_core::MapObject::Tree(0);
            trees += 1;
        }
    }
    assert!(trees > 150);
    // a herd far away, more than the old map-wide cap (len / 120 = 34)
    for x in 2..12 {
        for y in 2..6 {
            m.animals[g.index(c(x, y))] = Some(rr_core::Animal::Sheep);
        }
    }
    m.hq = vec![Some(c(10, 20))];
    let mut sim = Sim::new(
        std::sync::Arc::new(rr_rules::Rules::classic()),
        &m,
        &rr_sim::GameSetup { seed: 2, players: vec![Default::default()], ..Default::default() },
    )
    .unwrap();
    let wild = |s: &Sim| s.figures().iter().filter(|(_, f)| f.player == 255).count();
    let start = wild(&sim);
    for _ in 0..14_000 {
        sim.step(&[]);
    }
    let born = wild(&sim) - start;
    // RTTR rate: trees / 20 per 7000 GF, so about 16 in 14 000 GF
    assert!(born >= 10, "only {born} animals born in a forest of {trees} trees");
    for _ in 0..60_000 {
        sim.step(&[]);
    }
    let near = sim.figures().iter().filter(|(_, f)| f.player == 255 && g.distance(f.pos, c(40, 40)) <= 14).count();
    assert!(near <= 60, "{near} animals crowd the forest");
}

/// QA round 4, #3: a delivery wakes only a worker waiting for wares; one resting between
/// cycles rests its full time (RTTR: only WaitingForWaresOrProductionStopped is woken).
#[test]
fn a_delivery_does_not_cut_the_rest_short() {
    let mut m = rr_core::MapData::flat(64, 64, MEADOW);
    let g = m.grid;
    let mine_at = c(27, 20);
    for p in g.within(mine_at, 3) {
        let i = g.index(p);
        m.terrain_rsu[i] = 1;
        m.terrain_usd[i] = 1;
        m.resources[i] = Resource::Coal(7);
    }
    m.hq = vec![Some(c(20, 20))];
    let mut sim = Sim::new(
        std::sync::Arc::new(rr_rules::Rules::classic()),
        &m,
        &rr_sim::GameSetup { seed: 4, players: vec![Default::default()], ..Default::default() },
    )
    .unwrap();
    // food comes one at a time, so deliveries keep arriving while the miner rests
    for w in [Ware::Fish, Ware::Bread, Ware::Meat] {
        sim.debug_take_ware(0, w, 1000);
    }
    let mine = build_east(&mut sim, B::CoalMine, mine_at);
    ready(&mut sim, mine, 20_000);
    let jd = sim.rules().job(rr_rules::Job::Miner).clone();
    let mut starts = Vec::new();
    let mut was_working = false;
    for i in 0..30_000u32 {
        if i % 300 == 0 {
            sim.debug_add_ware(0, Ware::Fish, 1);
        }
        sim.step(&[]);
        let working =
            sim.building(mine).worker.is_some_and(|w| matches!(sim.figure(w).motion, rr_sim::Motion::Work { .. }));
        if working && !was_working {
            starts.push(sim.gf().0);
        }
        was_working = working;
    }
    assert!(starts.len() >= 5, "the mine worked {} times", starts.len());
    let min_gap = starts.windows(2).map(|w| w[1] - w[0]).min().unwrap();
    assert!(min_gap >= jd.work + jd.wait1, "cycles {min_gap} GF apart, work {} + rest {}", jd.work, jd.wait1);
}

/// New-player round 7, #8: a forester leaves the land right round a warehouse free (the
/// briefing's forester near the castle forested every building spot around it).
#[test]
fn foresters_leave_the_castle_free() {
    let mut sim = sim1();
    let hq = c(20, 20);
    let f = build_east(&mut sim, B::Forester, c(24, 20));
    ready(&mut sim, f, 20_000);
    for _ in 0..30_000 {
        sim.step(&[]);
    }
    let g = sim.world().grid();
    let planted = g.within(hq, 3).filter(|p| matches!(sim.world().obj(*p), rr_world::NodeObj::Tree { .. })).count();
    let further =
        g.within(c(24, 20), 6).filter(|p| matches!(sim.world().obj(*p), rr_world::NodeObj::Tree { .. })).count();
    assert_eq!(planted, 0, "trees by the castle");
    assert!(further > 5, "the forester planted only {further} trees");
}

/// Veteran round 8, #5: an armory fed in trickles makes as many shields as swords. It chose by
/// a counter that also counted its idle waits, so about one weapon in ten had no partner.
#[test]
fn an_armory_makes_swords_and_shields_in_pairs() {
    let mut sim = sim1();
    // no weapons ordered from elsewhere: a clean count
    let armory = build_east(&mut sim, B::Armory, c(26, 16));
    ready(&mut sim, armory, 20_000);
    let made = |s: &Sim, w: Ware| s.players()[0].stats.produced_by_ware[w.index()];
    let (s0, h0) = (made(&sim, Ware::Sword), made(&sim, Ware::Shield));
    // one iron and one coal now and then, with idle waits between
    for _ in 0..12 {
        sim.debug_add_ware(0, Ware::Iron, 1);
        sim.debug_add_ware(0, Ware::Coal, 1);
        for _ in 0..2_500 {
            sim.step(&[]);
        }
    }
    let (swords, shields) = (made(&sim, Ware::Sword) - s0, made(&sim, Ware::Shield) - h0);
    assert!(swords + shields >= 8, "{swords} swords, {shields} shields");
    assert!(swords.abs_diff(shields) <= 1, "{swords} swords, {shields} shields");
}

/// The pig chain (found untested in an audit, 2026-10-25): a pig farm raises pigs on grain and
/// water, the slaughterhouse makes meat of the ham.
#[test]
fn pig_farm_and_slaughterhouse_make_meat() {
    let mut sim = sim1();
    sim.debug_add_ware(0, Ware::Cleaver, 1);
    sim.debug_add_ware(0, Ware::Grain, 10);
    sim.debug_add_ware(0, Ware::Water, 10);
    let pigs = build_east(&mut sim, B::PigFarm, c(26, 16));
    let butcher = build_east(&mut sim, B::Slaughterhouse, c(25, 22));
    for b in [pigs, butcher] {
        ready(&mut sim, b, 20_000);
    }
    let made = |s: &Sim, w: Ware| s.players()[0].stats.produced_by_ware[w.index()];
    run_until(&mut sim, 40_000, |s| made(s, Ware::Ham) > 0 && made(s, Ware::Meat) > 0)
        .unwrap_or_else(|| panic!("ham {} meat {}", made(&sim, Ware::Ham), made(&sim, Ware::Meat)));
}

/// The smelter and the mint (found untested in an audit, 2026-10-25): ore and coal make iron,
/// gold and coal make coins.
#[test]
fn smelter_and_mint_work() {
    let mut sim = sim1();
    sim.debug_add_ware(0, Ware::Crucible, 2);
    sim.debug_add_ware(0, Ware::IronOre, 4);
    sim.debug_add_ware(0, Ware::Gold, 4);
    sim.debug_add_ware(0, Ware::Coal, 8);
    let smelter = build_east(&mut sim, B::IronSmelter, c(26, 16));
    let mint = build_east(&mut sim, B::Mint, c(25, 22));
    for b in [smelter, mint] {
        ready(&mut sim, b, 20_000);
    }
    let made = |s: &Sim, w: Ware| s.players()[0].stats.produced_by_ware[w.index()];
    run_until(&mut sim, 40_000, |s| made(s, Ware::Iron) > 0 && made(s, Ware::Coins) > 0)
        .unwrap_or_else(|| panic!("iron {} coins {}", made(&sim, Ware::Iron), made(&sim, Ware::Coins)));
}

/// The Gold statistic counts the coins minted, as RTTR's minter raises it, not the coins in
/// store (playtest 2026-10-27: it stayed flat all game, promotions spend coins as they come).
#[test]
fn the_gold_statistic_counts_coins_minted() {
    let mut sim = sim1();
    sim.debug_add_ware(0, Ware::Coins, 3);
    sim.debug_add_ware(0, Ware::Gold, 4);
    sim.debug_add_ware(0, Ware::Coal, 8);
    let gold = |s: &Sim| s.players()[0].stats.samples.last().map_or(0, |x| x.gold);
    run_until(&mut sim, 2_000, |s| !s.players()[0].stats.samples.is_empty()).expect("a sample");
    assert_eq!(gold(&sim), 0, "coins in store are not minted");
    let mint = build_east(&mut sim, B::Mint, c(25, 22));
    ready(&mut sim, mint, 20_000);
    run_until(&mut sim, 40_000, |s| gold(s) > 0).expect("minted coins counted");
    let minted = sim.players()[0].stats.produced_by_ware[Ware::Coins.index()];
    assert!(gold(&sim) <= minted);
}

/// Fidelity audit #8 (RTTR nofWorkman: wait2 after every work, a product or not): a stocked
/// donkey breeder breeds a donkey every work + wait1 + wait2 = 853 GF, not 648.
#[test]
fn the_donkey_breeder_keeps_its_pace() {
    let mut sim = sim1();
    sim.debug_add_ware(0, Ware::Grain, 20);
    sim.debug_add_ware(0, Ware::Water, 20);
    let breeder = build_east(&mut sim, B::DonkeyBreeder, c(26, 16));
    ready(&mut sim, breeder, 20_000);
    let donkeys = |s: &Sim| {
        s.figures().iter().filter(|(_, f)| f.job == Job::PackDonkey).count()
            + s.player_inventory(0).job(Job::PackDonkey) as usize
    };
    let mut born = Vec::new();
    let mut last = donkeys(&sim);
    for _ in 0..8_000 {
        sim.step(&[]);
        let now = donkeys(&sim);
        if now > last {
            born.push(sim.gf().0);
        }
        last = now;
    }
    let gaps: Vec<u32> = born.windows(2).map(|w| w[1] - w[0]).collect();
    assert!(gaps.len() >= 3, "{born:?}");
    // (the later gaps: the first may wait for wares)
    assert!(gaps[1..].iter().all(|g| (820..=900).contains(g)), "gaps {gaps:?}");
}

/// Fidelity audit #9 (RTTR noFire: 3 700 GF): a burned building's ruin stays as long as in RTTR.
#[test]
fn a_ruin_stays_rttrs_time() {
    let mut sim = sim1();
    let w = build_east(&mut sim, B::Woodcutter, c(26, 16));
    ready(&mut sim, w, 20_000);
    let at = sim.building(w).pos;
    sim.debug_destroy_building(at);
    assert_eq!(sim.world().obj(at), rr_world::NodeObj::Fire);
    for _ in 0..3_650 {
        sim.step(&[]);
    }
    assert_eq!(sim.world().obj(at), rr_world::NodeObj::Fire, "still burning at 3 650");
    run_until(&mut sim, 100, |s| s.world().obj(at) != rr_world::NodeObj::Fire).expect("gone by 3 750");
}

/// Owner's playtest (RTTR noBuildingSite::Destroy leaves the ground empty; only a finished
/// noBuilding burns): a building site that is torn down or lost doesn't burn, a finished
/// building does (`a_ruin_stays_rttrs_time`).
#[test]
fn a_building_site_does_not_burn() {
    let mut sim = sim1();
    let w = build_east(&mut sim, B::Woodcutter, c(26, 16));
    let at = sim.building(w).pos;
    assert!(!sim.building(w).is_ready(), "still a site");
    sim.debug_destroy_building(at);
    assert_eq!(sim.world().obj(at), rr_world::NodeObj::None, "no fire on a site");
}

/// `economy.geologist_radius` bounds how far from the flag the geologist looks.
#[test]
fn geologist_radius_bounds_the_signs() {
    let mut m = rr_core::MapData::flat(64, 64, MEADOW);
    let g = m.grid;
    m.hq = vec![Some(c(20, 20))];
    let mut rules = rr_rules::Rules::classic();
    rules.economy.geologist_radius = 2;
    let mut sim = Sim::new(
        std::sync::Arc::new(rules),
        &m,
        &rr_sim::GameSetup { seed: 3, players: vec![Default::default()], ..Default::default() },
    )
    .unwrap();
    let hq_flag = walk(&sim, c(20, 20), SE, 1);
    ok(&mut sim, Cmd::BuildRoad { from: hq_flag, dirs: vec![E, E, E, E] });
    let flag = walk(&sim, hq_flag, E, 4);
    ok(&mut sim, Cmd::SendGeologist { flag });
    for _ in 0..8000 {
        sim.step(&[]);
    }
    let signs: Vec<u32> = g
        .coords()
        .filter(|p| matches!(sim.world().obj(*p), rr_world::NodeObj::Sign(_)))
        .map(|p| g.distance(p, flag))
        .collect();
    assert!(signs.len() >= 4, "only {} signs", signs.len());
    assert!(signs.iter().all(|d| *d <= 2), "signs at {signs:?} from the flag");
}

/// Sound designer 2 #1: a finished building is news (`BuildingFinished`), not a positional
/// fanfare for everyone in earshot.
#[test]
fn a_finished_building_plays_no_fanfare() {
    let mut sim = sim1();
    let at = c(24, 20);
    ok(&mut sim, Cmd::PlaceBuilding { at, kind: rr_rules::BuildingType::Woodcutter });
    let hq_flag = walk(&sim, c(20, 20), SE, 1);
    let site_flag = walk(&sim, at, SE, 1);
    ok(&mut sim, Cmd::BuildRoad { from: hq_flag, dirs: vec![E, E, E, E] });
    assert_eq!(walk(&sim, hq_flag, E, 4), site_flag);
    let (mut done, mut fanfares) = (false, 0);
    for _ in 0..20_000 {
        let out = sim.step(&[]);
        for e in &out.events {
            match e {
                Event::BuildingFinished { .. } => done = true,
                Event::Sound { sound: rr_sim::SoundCue::Fanfare, .. } => fanfares += 1,
                _ => {}
            }
        }
        if done {
            break;
        }
    }
    assert!(done, "the woodcutter wasn't finished");
    assert_eq!(fanfares, 0);
}

/// RTTR nofFarmhand: a work spot counts only if a walk of at most 20 steps gets there (perf-2
/// #9: the search went up to 3000 nodes round lakes for a tree across the water).
#[test]
fn a_tree_across_the_water_is_out_of_reach() {
    let mut m = rr_core::MapData::flat(64, 64, MEADOW);
    let g = m.grid;
    // a water strip below the hut, open only far away
    for x in 0..64u16 {
        if (52..58).contains(&x) {
            continue;
        }
        for y in [24u16, 25] {
            let i = g.index(c(x, y));
            m.terrain_rsu[i] = 5;
            m.terrain_usd[i] = 5;
        }
    }
    m.hq = vec![Some(c(20, 20))];
    let mut sim = Sim::new(
        std::sync::Arc::new(rr_rules::Rules::classic()),
        &m,
        &rr_sim::GameSetup { seed: 3, players: vec![Default::default()], ..Default::default() },
    )
    .unwrap();
    let at = c(24, 20);
    let tree = c(24, 26);
    assert!(g.distance(at, tree) <= 6);
    sim.debug_set_obj(tree, rr_world::NodeObj::Tree { species: 0, stage: 3 });
    let b = build_east(&mut sim, B::Woodcutter, at);
    ready(&mut sim, b, 8000);
    for _ in 0..6000 {
        sim.step(&[]);
        assert!(!sim.is_work_reserved(tree), "the woodcutter set out round the lake");
    }
}

/// QA round 6, #5: a lookout tower's scout only keeps watch; he looped an empty work cycle,
/// so the tower showed 0 % and "works little" for ever.
#[test]
fn a_lookout_keeps_watch_without_a_work_loop() {
    let mut sim = sim1();
    let b = build_east(&mut sim, B::LookoutTower, c(24, 20));
    ready(&mut sim, b, 20_000);
    for _ in 0..2000 {
        sim.step(&[]);
    }
    assert_eq!(sim.building(b).attempts, 0, "the scout works an empty cycle");
}

/// The player's priority (owner's playtest 2026-10-27): of three woodcutter sites and boards
/// for one, the one given priority is built, though it was placed last.
#[test]
fn a_site_with_priority_is_built_first() {
    let mut sim = sim1();
    let boards = sim.player_inventory(0).ware(Ware::Boards);
    sim.debug_take_ware(0, Ware::Boards, boards - 2);
    let sites: Vec<_> =
        [c(26, 16), c(25, 22), c(16, 22)].into_iter().map(|p| build_east(&mut sim, B::Woodcutter, p)).collect();
    let last = sim.building(sites[2]).pos;
    ok(&mut sim, Cmd::SetPriority { at: last, on: true });
    assert!(sim.building(sites[2]).priority() && !sim.building(sites[0]).priority());
    ready(&mut sim, sites[2], 20_000);
    assert!(sites[..2].iter().all(|b| !sim.building(*b).is_ready()), "the boards went to the priority");
    // and it can be taken back
    ok(&mut sim, Cmd::SetPriority { at: last, on: false });
    assert!(!sim.building(sites[2]).priority());
}

/// A workshop with priority gets its inputs before any other kind of consumer, and takes all
/// it lacks at once: the grain goes to the brewery, not the mill.
#[test]
fn a_workshop_with_priority_is_filled_first() {
    let mut sim = sim1();
    sim.debug_add_job(0, Job::Brewer, 1);
    sim.debug_add_job(0, Job::Miller, 1);
    let mill = build_east(&mut sim, B::Mill, c(25, 22));
    let brewery = build_east(&mut sim, B::Brewery, c(26, 16));
    for b in [mill, brewery] {
        ready(&mut sim, b, 20_000);
    }
    let at = sim.building(brewery).pos;
    ok(&mut sim, Cmd::SetPriority { at, on: true });
    let grain = sim.player_inventory(0).ware(Ware::Grain);
    sim.debug_take_ware(0, Ware::Grain, grain);
    sim.debug_add_ware(0, Ware::Grain, 3);
    let got = |s: &Sim, b| s.building(b).stock[0] + s.building(b).incoming[0];
    run_until(&mut sim, 2_000, |s| got(s, brewery) >= 3).expect("all three grain to the brewery");
    assert_eq!(got(&sim, mill), 0);
}

/// Warehouses ask for nothing: the castle takes no priority, and nobody else's buildings.
#[test]
fn priority_is_for_own_workshops_and_sites() {
    let mut sim = sim1();
    let out = sim.step(&[rr_sim::PlayerCommand { player: 0, cmd: Cmd::SetPriority { at: c(20, 20), on: true } }]);
    assert!(out.events.iter().any(|e| matches!(e, Event::CommandRejected { .. })));
}
