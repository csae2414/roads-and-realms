mod common;
use common::*;
use rr_core::Direction::*;
use rr_core::MapCoord;
use rr_rules::{BuildingType as B, Ware};
use rr_sim::{Cmd, Event, Sim};
use rr_world::NodeObj;

fn ok(sim: &mut Sim, cmd: Cmd) {
    let out = exec(sim, 0, cmd.clone());
    assert!(
        out.events.iter().all(|e| !matches!(e, Event::CommandRejected { .. })),
        "{cmd:?} rejected: {:?}",
        out.events
    );
}

fn hq_flag(sim: &Sim) -> MapCoord {
    walk(sim, c(20, 20), SE, 1)
}

#[test]
fn a_new_road_gets_a_carrier() {
    let mut sim = sim1();
    let a = hq_flag(&sim);
    ok(&mut sim, Cmd::BuildRoad { from: a, dirs: vec![E, E, E, E] });
    let r = sim.road_between(a, walk(&sim, a, E, 4)).unwrap();
    let t = run_until(&mut sim, 1000, |s| s.road(r).carrier.is_some()).expect("carrier arrives");
    assert!(t < 400, "carrier took {t} GF");
    let c = sim.road(r).carrier.unwrap();
    assert_eq!(sim.figure(c).pos, walk(&sim, a, E, 2), "carrier waits in the middle");
}

/// Place a building at `at` (its flag is SE) and connect that flag to the HQ flag with `dirs`.
fn place_connected(sim: &mut Sim, kind: B, at: MapCoord, dirs: Vec<rr_core::Direction>) {
    ok(sim, Cmd::PlaceBuilding { at, kind });
    let a = hq_flag(sim);
    ok(sim, Cmd::BuildRoad { from: a, dirs });
}

#[test]
fn woodcutter_gets_built_and_manned() {
    let mut sim = sim1();
    // building at (24,20): flag at SE = (24,21), 4 east of the HQ flag (20,21)
    let at = c(24, 20);
    place_connected(&mut sim, B::Woodcutter, at, vec![E, E, E, E]);
    let b = sim.building_at(at).expect("site");
    assert!(sim.building(b).site().is_some());
    let t = run_until(&mut sim, 6000, |s| s.building(b).is_ready())
        .unwrap_or_else(|| panic!("not built:\n{}", sim.dump_string()));
    assert!(t > 912, "16 build steps at 57 GF can't be faster: {t}");
    run_until(&mut sim, 1000, |s| s.building(b).worker.is_some()).expect("woodcutter arrives");
}

#[test]
fn woodcutter_brings_wood_to_hq() {
    let mut sim = sim1();
    let at = c(24, 20);
    // trees around the hut
    for p in [c(26, 17), c(27, 18), c(25, 16), c(28, 20)] {
        sim.debug_set_obj(p, NodeObj::Tree { species: 0, stage: 3 });
    }
    place_connected(&mut sim, B::Woodcutter, at, vec![E, E, E, E]);
    let before = sim.player_inventory(0).ware(Ware::Wood);
    run_until(&mut sim, 12_000, |s| s.player_inventory(0).ware(Ware::Wood) > before).expect("wood arrives at the HQ");
    // the ware statistics count it
    run_until(&mut sim, 700, |s| {
        s.players()[0].stats.samples.last().is_some_and(|x| x.by_ware[Ware::Wood.index()] > 0)
    })
    .expect("wood in the ware statistics");
}

#[test]
fn sawmill_turns_wood_into_boards() {
    let mut sim = sim1();
    let at = c(24, 20);
    place_connected(&mut sim, B::Sawmill, at, vec![E, E, E, E]);
    let b = sim.building_at(at).unwrap();
    run_until(&mut sim, 10_000, |s| s.building(b).is_ready() && s.building(b).worker.is_some()).expect("sawmill runs");
    let boards = sim.player_inventory(0).ware(Ware::Boards);
    run_until(&mut sim, 6000, |s| s.player_inventory(0).ware(Ware::Boards) > boards).expect("boards arrive");
}

#[test]
fn wares_travel_over_several_roads() {
    let mut sim = sim1();
    let a = hq_flag(&sim);
    ok(&mut sim, Cmd::BuildRoad { from: a, dirs: vec![E, E, E] });
    let m = walk(&sim, a, E, 3);
    ok(&mut sim, Cmd::BuildRoad { from: m, dirs: vec![E, E, E] });
    let m2 = walk(&sim, m, E, 3);
    ok(&mut sim, Cmd::BuildRoad { from: m2, dirs: vec![NE, NE] });
    let flag = walk(&sim, m2, NE, 2);
    let at = walk(&sim, flag, NW, 1);
    ok(&mut sim, Cmd::PlaceBuilding { at, kind: B::Woodcutter });
    let b = sim.building_at(at).unwrap();
    run_until(&mut sim, 10_000, |s| s.building(b).is_ready()).expect("built across three roads");
}

#[test]
fn guardhouse_extends_territory() {
    let mut sim = sim1();
    let at = c(26, 20);
    place_connected(&mut sim, B::Guardhouse, at, vec![E, E, E, E, E, E]);
    let far = c(35, 20);
    assert_eq!(sim.world().owner(far), None);
    let b = sim.building_at(at).unwrap();
    run_until(&mut sim, 15_000, |s| s.building(b).is_ready() && !s.building(b).soldiers.is_empty()).expect("occupied");
    sim.step(&[]);
    assert_eq!(sim.world().owner(far), Some(0), "radius 9 around (26,20)");
}

#[test]
fn deterministic_economy() {
    let run = || {
        let mut sim = sim1();
        for p in [c(26, 17), c(27, 18), c(25, 16)] {
            sim.debug_set_obj(p, NodeObj::Tree { species: 1, stage: 3 });
        }
        place_connected(&mut sim, B::Woodcutter, c(24, 20), vec![E, E, E, E]);
        ok(&mut sim, Cmd::PlaceBuilding { at: c(20, 24), kind: B::Forester });
        let hf = hq_flag(&sim);
        ok(&mut sim, Cmd::BuildRoad { from: hf, dirs: vec![SE, SW, SE, SW] });
        let mut hashes = Vec::new();
        for i in 0..6000 {
            sim.step(&[]);
            if i % 500 == 0 {
                hashes.push(sim.sync_hash());
            }
        }
        hashes
    };
    assert_eq!(run(), run());
}

#[test]
fn quarry_cuts_stone() {
    let mut sim = sim1();
    sim.debug_set_obj(c(26, 18), NodeObj::Granite { kind: 0, size: 2 });
    place_connected(&mut sim, B::Quarry, c(24, 20), vec![E, E, E, E]);
    let before = sim.player_inventory(0).ware(Ware::Stones);
    run_until(&mut sim, 15_000, |s| s.player_inventory(0).ware(Ware::Stones) >= before + 2).expect("two stones");
    run_until(&mut sim, 4000, |s| s.world().obj(c(26, 18)) == NodeObj::None).expect("pile used up");
}

#[test]
fn forester_plants_trees_that_grow() {
    let mut sim = sim1();
    place_connected(&mut sim, B::Forester, c(24, 20), vec![E, E, E, E]);
    let grown = |s: &Sim| {
        s.world()
            .grid()
            .within(c(24, 20), 6)
            .filter(|p| matches!(s.world().obj(*p), NodeObj::Tree { stage: 3, .. }))
            .count()
    };
    run_until(&mut sim, 20_000, |s| grown(s) >= 2).expect("trees grow");
}

#[test]
fn construction_waits_for_material_then_continues() {
    let mut sim = sim1();
    // empty the HQ of boards so the site must wait
    sim.debug_take_ware(0, Ware::Boards, 44);
    place_connected(&mut sim, B::Woodcutter, c(24, 20), vec![E, E, E, E]);
    let b = sim.building_at(c(24, 20)).unwrap();
    for _ in 0..3000 {
        sim.step(&[]);
    }
    assert!(!sim.building(b).is_ready());
    sim.debug_take_ware(0, Ware::Boards, 0);
    sim.debug_add_ware(0, Ware::Boards, 2);
    run_until(&mut sim, 5000, |s| s.building(b).is_ready()).expect("finished after delivery");
}

#[test]
fn removing_a_road_reroutes_wares_and_sends_carrier_home() {
    let mut sim = sim1();
    let a = hq_flag(&sim);
    ok(&mut sim, Cmd::BuildRoad { from: a, dirs: vec![E, E, E, E] });
    let r = sim.road_between(a, walk(&sim, a, E, 4)).unwrap();
    run_until(&mut sim, 1000, |s| s.road(r).carrier.is_some()).unwrap();
    let helpers = sim.player_inventory(0).job(rr_rules::Job::Helper);
    ok(&mut sim, Cmd::RemoveRoad { flag: a, dir: E });
    run_until(&mut sim, 2000, |s| s.player_inventory(0).job(rr_rules::Job::Helper) > helpers)
        .expect("carrier back home");
}

#[test]
fn busy_road_gets_a_donkey() {
    let mut sim = sim1();
    // one road carries everything for several big sites behind it
    let a = hq_flag(&sim);
    ok(&mut sim, Cmd::BuildRoad { from: a, dirs: vec![E, E, E, E, E, E] });
    let far = walk(&sim, a, E, 6);
    let r = sim.road_between(a, far).unwrap();
    let g = sim.world().grid();
    let mut placed = 0;
    for kind in [B::Fortress, B::Sawmill, B::Brewery, B::Armory] {
        let need = match kind {
            B::Fortress => rr_world::Bq::Castle,
            _ => rr_world::Bq::House,
        };
        let spot = g.within(far, 5).find(|p| {
            g.distance(*p, c(20, 20)) > 5 && sim.world().bq_for(*p, 0).allows(need) && {
                let f = g.neighbor(*p, SE);
                f != far && sim.road_path(0, f, far, &[]).is_some()
            }
        });
        let Some(at) = spot else { continue };
        ok(&mut sim, Cmd::PlaceBuilding { at, kind });
        let f = g.neighbor(at, SE);
        let dirs = sim.road_path(0, f, far, &[]).expect("road");
        ok(&mut sim, Cmd::BuildRoad { from: f, dirs });
        placed += 1;
    }
    assert!(placed >= 3);
    run_until(&mut sim, 14_000, |s| s.roads().get(r).is_some_and(|x| x.donkey.is_some()))
        .unwrap_or_else(|| panic!("no donkey; busy {:?}", sim.roads().get(r).map(|x| x.busy)));
    assert_eq!(sim.world().road(walk(&sim, a, E, 1), W), rr_world::RoadKind::Donkey);
    // a flag in the middle keeps both halves donkey roads (the original keeps the upgrade)
    let mid = walk(&sim, a, E, 3);
    ok(&mut sim, Cmd::PlaceFlag { at: mid });
    let (r1, r2) = (sim.road_between(a, mid).unwrap(), sim.road_between(mid, far).unwrap());
    assert_eq!((sim.road(r1).kind, sim.road(r2).kind), (rr_world::RoadKind::Donkey, rr_world::RoadKind::Donkey));
    run_until(&mut sim, 3000, |s| s.road(r1).donkey.is_some() && s.road(r2).donkey.is_some())
        .unwrap_or_else(|| panic!("both halves get a donkey:\n{}", sim.dump_string()));
}

#[test]
fn boat_road_across_a_lake_gets_a_boat_carrier() {
    let mut m = rr_core::MapData::flat(64, 64, MEADOW);
    let g = m.grid;
    // a small lake east of the HQ
    for p in g.within(c(20, 26), 3) {
        let i = g.index(p);
        m.terrain_rsu[i] = 5;
        m.terrain_usd[i] = 5;
    }
    m.hq = vec![Some(c(20, 20))];
    let mut sim = Sim::new(
        std::sync::Arc::new(rr_rules::Rules::classic()),
        &m,
        &rr_sim::GameSetup { seed: 1, players: vec![Default::default()], ..Default::default() },
    )
    .unwrap();
    // a straight crossing: land flag, 2..=4 water points, land flag
    let mut crossing = None;
    'search: for y in 22..32u16 {
        for x in 14..26u16 {
            let west = c(x, y);
            for len in 3..=5u16 {
                let east = walk(&sim, west, E, len);
                let mid_ok = (1..len).all(|k| sim.is_water_point(walk(&sim, west, E, k)));
                let ends = [west, east].iter().all(|p| sim.world().bq_for(*p, 0) != rr_world::Bq::Nothing);
                if mid_ok && ends {
                    crossing = Some((west, east, len));
                    break 'search;
                }
            }
        }
    }
    let (west, east, len) = crossing.expect("a crossing over the lake");
    let hf = hq_flag(&sim);
    let dirs = sim.road_path(0, hf, west, &[]).unwrap();
    ok(&mut sim, Cmd::BuildRoad { from: hf, dirs });
    ok(&mut sim, Cmd::BuildRoad { from: west, dirs: vec![E; len as usize] });
    let r = sim.road_between(west, east).expect("road");
    assert_eq!(sim.road(r).kind, rr_world::RoadKind::Boat);
    let boats = sim.player_inventory(0).ware(Ware::Boat);
    run_until(&mut sim, 3000, |s| s.road(r).carrier.is_some()).expect("boat carrier");
    assert_eq!(sim.player_inventory(0).ware(Ware::Boat), boats - 1, "the carrier took a boat");
    // a site across the lake gets boards by boat; the road goes while a board is in the boat
    let g = sim.world().grid();
    let site = g
        .within(east, 4)
        .find(|p| {
            sim.world().bq_for(*p, 0).allows(rr_world::Bq::Hut)
                && sim.road_path(0, g.neighbor(*p, SE), east, &[]).is_some_and(|d| d.len() <= 4)
        })
        .expect("site spot");
    ok(&mut sim, Cmd::PlaceBuilding { at: site, kind: B::Woodcutter });
    let sf = g.neighbor(site, SE);
    if sf != east {
        let dirs = sim.road_path(0, sf, east, &[]).unwrap();
        ok(&mut sim, Cmd::BuildRoad { from: sf, dirs });
    }
    let c = sim.road(r).carrier.unwrap();
    run_until(&mut sim, 6000, |s| s.figure(c).ware.is_some()).expect("a ware in the boat");
    // the road goes: the carrier rows ashore and brings the boat back
    ok(&mut sim, Cmd::RemoveRoad { flag: west, dir: E });
    run_until(&mut sim, 3000, |s| s.player_inventory(0).ware(Ware::Boat) == boats)
        .unwrap_or_else(|| panic!("boat lost:\n{}", sim.dump_string()));
}

/// Roast finding: a ware waiting for a road that is torn down must find another way
/// (RTTR re-routes the wares at both flags of a removed road).
#[test]
fn wares_reroute_when_their_road_is_removed() {
    let mut sim = sim1();
    let a = hq_flag(&sim);
    let mid = walk(&sim, a, E, 3);
    let end = walk(&sim, mid, E, 3);
    let at = walk(&sim, end, NW, 1);
    ok(&mut sim, Cmd::PlaceBuilding { at, kind: B::Sawmill });
    ok(&mut sim, Cmd::BuildRoad { from: a, dirs: vec![E, E, E] });
    ok(&mut sim, Cmd::BuildRoad { from: mid, dirs: vec![E, E, E] });
    let b = sim.building_at(at).unwrap();
    let mid_flag = sim.flag_at(mid).unwrap();
    run_until(&mut sim, 4000, |s| {
        s.flag(mid_flag).wares.iter().any(|w| s.wares().get(*w).is_some_and(|w| w.goal == Some(b)))
    })
    .expect("a ware waits at the middle flag");
    ok(&mut sim, Cmd::RemoveRoad { flag: mid, dir: E });
    // a detour that enters `end` from another side
    ok(&mut sim, Cmd::BuildRoad { from: mid, dirs: vec![SE, E, E, NE] });
    run_until(&mut sim, 12_000, |s| s.building(b).is_ready())
        .unwrap_or_else(|| panic!("site starved:\n{}", sim.dump_string()));
}

/// Roast finding: wares queued in a warehouse that burns must not stay counted as "on the way".
#[test]
fn burning_warehouse_releases_its_queued_wares() {
    let mut sim = sim1();
    let a = hq_flag(&sim);
    let at = walk(&sim, walk(&sim, a, E, 3), NW, 1);
    ok(&mut sim, Cmd::PlaceBuilding { at, kind: B::Sawmill });
    ok(&mut sim, Cmd::BuildRoad { from: a, dirs: vec![E, E, E] });
    let b = sim.building_at(at).unwrap();
    let hq = sim.building_at(c(20, 20)).unwrap();
    run_until(&mut sim, 400, |s| !s.building(hq).out_queue.is_empty()).expect("HQ queues wares for the site");
    sim.debug_destroy_building(c(20, 20));
    let heading = |s: &Sim, k: Ware| s.wares().iter().filter(|(_, w)| w.goal == Some(b) && w.kind == k).count();
    let bd = sim.building(b);
    assert_eq!(bd.site_incoming.0 as usize, heading(&sim, Ware::Boards));
    assert_eq!(bd.site_incoming.1 as usize, heading(&sim, Ware::Stones));
}

/// Roast finding: people from a torn-down building walk home along the roads, not across
/// the fields.
#[test]
fn workers_walk_home_along_the_roads() {
    let mut sim = sim1();
    let a = hq_flag(&sim);
    // an L-shaped road, so that the straight way home would leave it
    let corner = walk(&sim, a, E, 4);
    ok(&mut sim, Cmd::BuildRoad { from: a, dirs: vec![E, E, E, E] });
    ok(&mut sim, Cmd::BuildRoad { from: corner, dirs: vec![NE, NE, NE, NE] });
    let end = walk(&sim, corner, NE, 4);
    let at = walk(&sim, end, NW, 1);
    ok(&mut sim, Cmd::PlaceBuilding { at, kind: B::Woodcutter });
    let b = sim.building_at(at).unwrap();
    run_until(&mut sim, 8000, |s| s.building(b).is_ready() && s.building(b).worker.is_some()).expect("manned");
    let worker = sim.building(b).worker.unwrap();
    run_until(&mut sim, 2000, |s| s.figure(worker).inside.is_some()).expect("inside");
    sim.debug_destroy_building(at);
    for _ in 0..600 {
        sim.step(&[]);
        let Some(f) = sim.figures().get(worker) else { break };
        let p = f.pos;
        assert!(
            p == at || p == c(20, 20) || sim.flag_at(p).is_some() || sim.world().is_on_road(p),
            "worker left the roads at {p:?}"
        );
    }
    assert!(sim.figures().get(worker).is_none(), "worker got home");
}

/// Place and connect a storehouse east of the HQ; returns it once built.
fn storehouse(sim: &mut Sim) -> rr_sim::BuildingId {
    let at = c(25, 22);
    ok(sim, Cmd::PlaceBuilding { at, kind: B::Storehouse });
    let f = sim.world().grid().neighbor(at, SE);
    let dirs = sim.road_path(0, f, hq_flag(sim), &[]).expect("road");
    ok(sim, Cmd::BuildRoad { from: f, dirs });
    let s = sim.building_at(at).unwrap();
    run_until(sim, 20_000, |s2| s2.building(s).is_ready()).expect("storehouse built");
    s
}

/// The original's warehouse settings: "move out" empties a warehouse of a ware.
#[test]
fn warehouse_moves_a_ware_out() {
    let mut sim = sim1();
    let s = storehouse(&mut sim);
    let hq = sim.building_at(c(20, 20)).unwrap();
    let boards = sim.building(hq).inventory.as_ref().unwrap().ware(Ware::Boards);
    ok(&mut sim, Cmd::SetStockPolicy { at: c(20, 20), ware: Ware::Boards, policy: rr_sim::StockPolicy::MoveOut });
    run_until(&mut sim, 30_000, |x| x.building(hq).inventory.as_ref().unwrap().ware(Ware::Boards) == 0)
        .expect("HQ empties its boards");
    run_until(&mut sim, 6000, |x| x.building(s).inventory.as_ref().unwrap().ware(Ware::Boards) == boards)
        .expect("all boards in the storehouse");
}

/// "Collect" fetches a ware from the other warehouses.
#[test]
fn warehouse_collects_a_ware() {
    let mut sim = sim1();
    let s = storehouse(&mut sim);
    let at = sim.building(s).pos;
    ok(&mut sim, Cmd::SetStockPolicy { at, ware: Ware::Stones, policy: rr_sim::StockPolicy::Collect });
    run_until(&mut sim, 30_000, |x| x.building(s).inventory.as_ref().unwrap().ware(Ware::Stones) >= 10)
        .expect("stones collected");
}

/// The same settings for people: "move out" sends the HQ's builders to the storehouse.
#[test]
fn warehouse_moves_people_out() {
    let mut sim = sim1();
    let s = storehouse(&mut sim);
    let hq = sim.building_at(c(20, 20)).unwrap();
    let builders = sim.building(hq).inventory.as_ref().unwrap().job(rr_rules::Job::Builder);
    assert!(builders > 0);
    ok(
        &mut sim,
        Cmd::SetPeoplePolicy { at: c(20, 20), job: rr_rules::Job::Builder, policy: rr_sim::StockPolicy::MoveOut },
    );
    run_until(&mut sim, 30_000, |x| x.building(s).inventory.as_ref().unwrap().job(rr_rules::Job::Builder) == builders)
        .unwrap_or_else(|| {
            panic!("builders moved: {}", sim.building(s).inventory.as_ref().unwrap().job(rr_rules::Job::Builder))
        });
    assert_eq!(sim.building(hq).inventory.as_ref().unwrap().job(rr_rules::Job::Builder), 0);
}

/// QA round 5, #1: wares heading to a warehouse that burns are sent elsewhere, never to the
/// burning warehouse again (they kept its dead id, and the audit then blamed the next building
/// in that arena slot).
#[test]
fn wares_of_a_burned_warehouse_find_a_live_goal() {
    let mut sim = sim1();
    let s = storehouse(&mut sim);
    let at = sim.building(s).pos;
    // stones on their way from the HQ: one waiting at a flag when the storehouse burns
    ok(&mut sim, Cmd::SetStockPolicy { at, ware: Ware::Stones, policy: rr_sim::StockPolicy::Collect });
    run_until(&mut sim, 20_000, |x| {
        x.wares().iter().any(|(_, w)| w.goal == Some(s) && matches!(w.loc, rr_sim::WareLoc::Flag(_)))
    })
    .expect("stones wait at a flag on their way");
    sim.debug_destroy_building(at);
    for (_, w) in sim.wares().iter() {
        if let Some(goal) = w.goal {
            assert!(sim.buildings().get(goal).is_some(), "{w:?} heads to a building that is gone");
        }
    }
    // a new site in the same slot: the bookkeeping still holds
    for _ in 0..2_000 {
        sim.step(&[]);
    }
    sim.audit().expect("audit");
}
