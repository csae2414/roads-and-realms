mod common;
use common::*;
use rr_core::Direction::*;
use rr_core::{MapCoord, MapData};
use rr_rules::{BuildingType as B, Rules, Ware};
use rr_sim::{Cmd, Event, GameSetup, PlayerCommand, Sim};
use std::sync::Arc;

const WATER: u8 = 5;

/// Two islands in a sea; harbor spots on both coasts facing each other.
fn islands() -> (Sim, MapCoord, MapCoord) {
    let mut m = MapData::flat(64, 64, WATER);
    let g = m.grid;
    for (center, r) in [(c(16, 20), 9u16), (c(46, 42), 9u16)] {
        for p in g.within(center, r) {
            let i = g.index(p);
            m.terrain_rsu[i] = MEADOW;
            m.terrain_usd[i] = MEADOW;
        }
    }
    // spots 8 nodes from each centre, facing the other island
    let (ca, cb) = (c(16, 20), c(46, 42));
    let spot_a = g.ring(ca, 7).filter(|p| p.y % 2 == 0).min_by_key(|p| g.distance(*p, cb)).unwrap();
    let spot_b = g.ring(cb, 7).filter(|p| p.y % 2 == 0).min_by_key(|p| g.distance(*p, ca)).unwrap();
    m.harbors = vec![spot_a, spot_b];
    m.hq = vec![Some(c(16, 20))];
    let sim = Sim::new(
        Arc::new(Rules::classic()),
        &m,
        &GameSetup { seed: 7, players: vec![Default::default()], ..Default::default() },
    )
    .unwrap();
    (sim, spot_a, spot_b)
}

fn ok(sim: &mut Sim, cmd: Cmd) {
    let out = sim.step(&[PlayerCommand { player: 0, cmd: cmd.clone() }]);
    assert!(out.events.iter().all(|e| !matches!(e, Event::CommandRejected { .. })), "{cmd:?}: {:?}", out.events);
}

fn connect(sim: &mut Sim, at: MapCoord) {
    let f = sim.world().grid().neighbor(at, SE);
    let hq_flag = walk(sim, c(16, 20), SE, 1);
    let dirs = sim.road_path(0, f, hq_flag, &[]).expect("road to HQ");
    ok(sim, Cmd::BuildRoad { from: f, dirs });
}

fn harbor_on_a(sim: &mut Sim, spot: MapCoord) -> rr_sim::BuildingId {
    assert_eq!(sim.world().bq_for(spot, 0), rr_world::Bq::Harbor, "harbor spot");
    ok(sim, Cmd::PlaceBuilding { at: spot, kind: B::Harbor });
    connect(sim, spot);
    let h = sim.building_at(spot).unwrap();
    run_until(sim, 20_000, |s| s.building(h).is_ready()).expect("harbor built");
    h
}

/// A medium building spot near the harbor (so the shipwright reaches the shore).
fn yard_spot(sim: &Sim, harbor: MapCoord) -> MapCoord {
    let g = sim.world().grid();
    g.within(harbor, 5)
        .filter(|p| g.distance(*p, harbor) >= 3)
        .find(|p| {
            sim.world().bq_for(*p, 0).allows(rr_world::Bq::House)
                && sim.road_path(0, g.neighbor(*p, SE), walk(sim, c(16, 20), SE, 1), &[]).is_some()
        })
        .expect("yard spot")
}

#[test]
fn seas_are_found_and_harbor_spots_have_coasts() {
    let (sim, a, b) = islands();
    let sa = sim.harbor_sea(a).expect("spot a at a sea");
    let sb = sim.harbor_sea(b).expect("spot b at a sea");
    assert_eq!(sa, sb, "one sea");
}

#[test]
fn harbor_is_a_warehouse_with_territory() {
    let (mut sim, a, _) = islands();
    let h = harbor_on_a(&mut sim, a);
    assert!(sim.building(h).inventory.is_some());
    sim.step(&[]);
    assert_eq!(sim.world().owner(walk(&sim, a, E, 7)), Some(0), "harbor radius 8");
}

#[test]
fn shipyard_launches_a_ship_that_docks() {
    let (mut sim, a, _) = islands();
    harbor_on_a(&mut sim, a);
    let yard_at = yard_spot(&sim, a);
    ok(&mut sim, Cmd::PlaceBuilding { at: yard_at, kind: B::Shipyard });
    ok(&mut sim, Cmd::SetShipyardMode { at: yard_at, ships: true });
    connect(&mut sim, yard_at);
    sim.debug_add_ware(0, Ware::Boards, 20);
    run_until(&mut sim, 40_000, |s| !s.ships().is_empty()).unwrap_or_else(|| panic!("no ship:\n{}", sim.dump_string()));
    let ship = sim.ships().iter().next().unwrap().0;
    run_until(&mut sim, 3000, |s| s.ship(ship).docked.is_some()).expect("ship docks at the harbor");
    // a ship sees two nodes round it, as in RTTR (VISUALRANGE_SHIP): voyages uncover coasts
    run_until(&mut sim, 40, |_| false);
    let at = sim.ship(ship).pos;
    let g = sim.world().grid();
    assert!(g.within(at, 2).all(|p| sim.visibility(0, p) == rr_sim::Visibility::Visible));
}

#[test]
fn expedition_founds_a_colony_and_ships_carry_wares() {
    let (mut sim, a, b) = islands();
    let ha = harbor_on_a(&mut sim, a);
    let yard_at = yard_spot(&sim, a);
    ok(&mut sim, Cmd::PlaceBuilding { at: yard_at, kind: B::Shipyard });
    ok(&mut sim, Cmd::SetShipyardMode { at: yard_at, ships: true });
    connect(&mut sim, yard_at);
    sim.debug_add_ware(0, Ware::Boards, 30);
    run_until(&mut sim, 40_000, |s| !s.ships().is_empty()).expect("ship");
    ok(&mut sim, Cmd::StartExpedition { harbor: a });
    run_until(&mut sim, 6000, |s| s.expedition_ready(ha)).expect("expedition ready");
    ok(&mut sim, Cmd::SendExpedition { harbor: a, to: b });
    run_until(&mut sim, 6000, |s| s.building_at(b).is_some()).expect("colony founded");
    let hb = sim.building_at(b).unwrap();
    assert_eq!(sim.building(hb).kind, B::Harbor);
    assert_eq!(sim.world().owner(b), Some(0), "the colony claims land at once");
    run_until(&mut sim, 20_000, |s| s.building(hb).is_ready()).expect("colony harbor built");
    // the expedition brought a starter kit along: it is in the colony's store (ours, 2026-10-27)
    let inv = sim.building(hb).inventory.clone().expect("a store");
    let kit = rr_sim::COLONY_KIT;
    assert_eq!((inv.ware(Ware::Boards), inv.ware(Ware::Stones)), (kit.0 as u32, kit.1 as u32));
    sim.debug_take_ware_at(hb, Ware::Boards, 99);
    sim.debug_take_ware_at(hb, Ware::Stones, 99);
    // a site on island B gets its boards by ship
    let site = walk(&sim, b, E, 3);
    let site = walk(&sim, site, SE, 1);
    ok(&mut sim, Cmd::PlaceBuilding { at: site, kind: B::Woodcutter });
    let f = sim.world().grid().neighbor(site, SE);
    let hbf = sim.world().grid().neighbor(b, SE);
    let dirs = sim.road_path(0, f, hbf, &[]).expect("road on island B");
    ok(&mut sim, Cmd::BuildRoad { from: f, dirs });
    let w = sim.building_at(site).unwrap();
    run_until(&mut sim, 30_000, |s| s.building(w).is_ready())
        .unwrap_or_else(|| panic!("not supplied overseas:\n{}", sim.dump_string()));
}

/// Roast finding: an expedition whose spot is taken before it lands comes back and can be
/// sent again.
#[test]
fn failed_expedition_returns_and_can_be_resent() {
    let (mut sim, a, b) = islands();
    let ha = harbor_on_a(&mut sim, a);
    let yard_at = yard_spot(&sim, a);
    ok(&mut sim, Cmd::PlaceBuilding { at: yard_at, kind: B::Shipyard });
    ok(&mut sim, Cmd::SetShipyardMode { at: yard_at, ships: true });
    connect(&mut sim, yard_at);
    sim.debug_add_ware(0, Ware::Boards, 30);
    run_until(&mut sim, 40_000, |s| !s.ships().is_empty()).expect("ship");
    ok(&mut sim, Cmd::StartExpedition { harbor: a });
    run_until(&mut sim, 6000, |s| s.expedition_ready(ha)).expect("expedition ready");
    ok(&mut sim, Cmd::SendExpedition { harbor: a, to: b });
    // the spot gets blocked while the ship is at sea
    sim.debug_set_obj(b, rr_world::NodeObj::Granite { kind: 0, size: 3 });
    run_until(&mut sim, 6000, |s| s.expedition_ready(ha))
        .unwrap_or_else(|| panic!("ship lost:\n{}", sim.dump_string()));
    sim.debug_set_obj(b, rr_world::NodeObj::None);
    ok(&mut sim, Cmd::SendExpedition { harbor: a, to: b });
    run_until(&mut sim, 6000, |s| s.building_at(b).is_some()).expect("colony founded on the second try");
}

/// QA round 2: soldiers cross the sea to man a colony's military buildings.
#[test]
fn soldiers_sail_to_a_colony_guardhouse() {
    let (mut sim, a, b) = islands();
    let ha = harbor_on_a(&mut sim, a);
    let yard_at = yard_spot(&sim, a);
    ok(&mut sim, Cmd::PlaceBuilding { at: yard_at, kind: B::Shipyard });
    ok(&mut sim, Cmd::SetShipyardMode { at: yard_at, ships: true });
    connect(&mut sim, yard_at);
    sim.debug_add_ware(0, Ware::Boards, 40);
    sim.debug_add_ware(0, Ware::Stones, 20);
    run_until(&mut sim, 40_000, |s| !s.ships().is_empty()).expect("ship");
    ok(&mut sim, Cmd::StartExpedition { harbor: a });
    run_until(&mut sim, 6000, |s| s.expedition_ready(ha)).expect("expedition ready");
    ok(&mut sim, Cmd::SendExpedition { harbor: a, to: b });
    run_until(&mut sim, 6000, |s| s.building_at(b).is_some()).expect("colony");
    let hb = sim.building_at(b).unwrap();
    run_until(&mut sim, 20_000, |s| s.building(hb).is_ready()).expect("colony harbor built");
    // a guardhouse on island B
    let g = sim.world().grid();
    let hbf = g.neighbor(b, SE);
    let at = g
        .within(b, 6)
        .filter(|p| g.distance(*p, b) >= 3)
        .find(|p| {
            sim.world().bq_for(*p, 0).allows(rr_world::Bq::Hut)
                && !sim.military_building_near(*p, 0)
                && sim.road_path(0, g.neighbor(*p, SE), hbf, &[]).is_some()
        })
        .expect("guardhouse spot on the island");
    ok(&mut sim, Cmd::PlaceBuilding { at, kind: B::Guardhouse });
    let f = g.neighbor(at, SE);
    let dirs = sim.road_path(0, f, hbf, &[]).unwrap();
    ok(&mut sim, Cmd::BuildRoad { from: f, dirs });
    let gh = sim.building_at(at).unwrap();
    run_until(&mut sim, 40_000, |s| s.building(gh).is_ready() && !s.building(gh).soldiers.is_empty())
        .unwrap_or_else(|| panic!("no soldier crossed:\n{}", sim.dump_string()));
}

/// Veteran round 3, #10 (RTTR nobShipYard): a shipyard makes boats until the player switches it
/// to ships, harbor or not.
#[test]
fn a_shipyard_makes_boats_until_switched() {
    let (mut sim, a, _) = islands();
    harbor_on_a(&mut sim, a);
    let yard_at = yard_spot(&sim, a);
    ok(&mut sim, Cmd::PlaceBuilding { at: yard_at, kind: B::Shipyard });
    connect(&mut sim, yard_at);
    sim.debug_add_ware(0, Ware::Boards, 20);
    let yard = sim.building_at(yard_at).unwrap();
    assert!(!sim.building(yard).build_ships);
    run_until(&mut sim, 20_000, |s| s.player_inventory(0).ware(Ware::Boat) > 0).expect("a boat");
    assert!(sim.ships().is_empty());
    ok(&mut sim, Cmd::SetShipyardMode { at: yard_at, ships: true });
    run_until(&mut sim, 40_000, |s| !s.ships().is_empty()).expect("then a ship");
}

/// Fidelity audit #3 (RTTR nofShipWright: WORKING_TIME_SHIPS 70 per step): a ship is built in
/// about 3 300 GF once the shipyard runs, not 14 000.
#[test]
fn a_ship_is_built_at_rttrs_pace() {
    let (mut sim, a, _) = islands();
    harbor_on_a(&mut sim, a);
    let yard_at = yard_spot(&sim, a);
    ok(&mut sim, Cmd::PlaceBuilding { at: yard_at, kind: B::Shipyard });
    ok(&mut sim, Cmd::SetShipyardMode { at: yard_at, ships: true });
    connect(&mut sim, yard_at);
    sim.debug_add_ware(0, Ware::Boards, 20);
    let yard = sim.building_at(yard_at).unwrap();
    run_until(&mut sim, 30_000, |s| s.building(yard).is_ready() && s.building(yard).worker.is_some())
        .expect("shipyard runs");
    let n = run_until(&mut sim, 40_000, |s| !s.ships().is_empty()).expect("a ship");
    assert!(n <= 4_500, "the ship took {n} GF");
}

/// RTTR's "near harbors" class comes with sea attacks (owner, 2026-10-27): a post within
/// SEA_ATTACK_DISTANCE of an own harbor, with no rival near by land, is manned by the "troops
/// near harbors" setting (fidelity audit #4 had it off while nobody attacked from the sea).
#[test]
fn a_post_by_a_harbor_guards_it() {
    let (mut sim, a, _) = islands();
    harbor_on_a(&mut sim, a);
    let gr = sim.world().grid();
    let at = gr
        .within(a, 6)
        .find(|p| {
            sim.world().bq_for(*p, 0).allows(rr_world::Bq::Hut)
                && !sim.military_building_near(*p, 0)
                && sim.road_path(0, gr.neighbor(*p, SE), walk(&sim, c(16, 20), SE, 1), &[]).is_some()
        })
        .expect("a spot for a guardhouse by the harbor");
    ok(&mut sim, Cmd::PlaceBuilding { at, kind: B::Guardhouse });
    connect(&mut sim, at);
    let g = sim.building_at(at).unwrap();
    run_until(&mut sim, 20_000, |s| s.building(g).is_ready() && !s.building(g).soldiers.is_empty()).expect("manned");
    assert_eq!(sim.frontier_distance(g), 2);
}

/// Fidelity audit #6 (RTTR noShip, default speed): a ship sails 20 GF per node and holds 40.
#[test]
fn ships_sail_at_rttrs_speed() {
    assert_eq!(rr_sim::SHIP_CAPACITY, 40);
    let (mut sim, a, _) = islands();
    harbor_on_a(&mut sim, a);
    let yard_at = yard_spot(&sim, a);
    ok(&mut sim, Cmd::PlaceBuilding { at: yard_at, kind: B::Shipyard });
    ok(&mut sim, Cmd::SetShipyardMode { at: yard_at, ships: true });
    connect(&mut sim, yard_at);
    sim.debug_add_ware(0, Ware::Boards, 20);
    run_until(&mut sim, 40_000, |s| !s.ships().is_empty()).expect("a ship");
    let ship = sim.ships().iter().next().unwrap().0;
    run_until(&mut sim, 3000, |s| matches!(s.ship(ship).motion, rr_sim::Motion::Walk { .. })).expect("sails");
    let rr_sim::Motion::Walk { dur, .. } = sim.ship(ship).motion else { unreachable!() };
    assert_eq!(dur, 20);
}

/// Sound designer 2 #6: building a ship and docking were silent.
#[test]
fn a_ship_is_heard_built_launched_and_docking() {
    let (mut sim, a, _) = islands();
    harbor_on_a(&mut sim, a);
    let yard_at = yard_spot(&sim, a);
    ok(&mut sim, Cmd::PlaceBuilding { at: yard_at, kind: B::Shipyard });
    ok(&mut sim, Cmd::SetShipyardMode { at: yard_at, ships: true });
    connect(&mut sim, yard_at);
    sim.debug_add_ware(0, Ware::Boards, 20);
    let mut heard = Vec::new();
    let mut docked = false;
    for _ in 0..45_000 {
        let out = sim.step(&[]);
        for e in &out.events {
            if let rr_sim::Event::Sound { sound, .. } = e {
                heard.push(*sound);
            }
        }
        if heard.contains(&rr_sim::SoundCue::Dock) {
            docked = true;
            break;
        }
    }
    use rr_sim::SoundCue as S;
    assert!(heard.contains(&S::Hammer) && heard.contains(&S::Launch), "{heard:?}");
    assert!(docked, "no ship docked with a sound: {heard:?}");
}

/// Game design round 1, #7: an expedition ready at a harbor calls an idle ship from elsewhere
/// on its sea (RTTR nobHarborBuilding::OrderShip). Egypt's waited for ever, fully loaded, while
/// both its ships lay idle at its other harbor. Here island A has two harbors: the ship lies at
/// the first, the expedition gets its wares by road at the second.
#[test]
fn an_expedition_calls_a_ship() {
    let mut m = MapData::flat(64, 64, WATER);
    let g = m.grid;
    let ca = c(16, 20);
    for p in g.within(ca, 9) {
        let i = g.index(p);
        m.terrain_rsu[i] = MEADOW;
        m.terrain_usd[i] = MEADOW;
    }
    let ring: Vec<MapCoord> = g.ring(ca, 7).filter(|p| p.y % 2 == 0).collect();
    let east = *ring.iter().max_by_key(|p| p.x).unwrap();
    let west = *ring.iter().min_by_key(|p| p.x).unwrap();
    m.harbors = vec![east, west];
    m.hq = vec![Some(ca)];
    let mut sim = Sim::new(
        Arc::new(Rules::classic()),
        &m,
        &GameSetup { seed: 7, players: vec![Default::default()], ..Default::default() },
    )
    .unwrap();
    let he = harbor_on_a(&mut sim, east);
    let yard_at = yard_spot(&sim, east);
    ok(&mut sim, Cmd::PlaceBuilding { at: yard_at, kind: B::Shipyard });
    ok(&mut sim, Cmd::SetShipyardMode { at: yard_at, ships: true });
    connect(&mut sim, yard_at);
    sim.debug_add_ware(0, Ware::Boards, 30);
    run_until(&mut sim, 40_000, |s| !s.ships().is_empty()).expect("ship");
    let ship = sim.ships().iter().next().unwrap().0;
    run_until(&mut sim, 6000, |s| s.ship(ship).docked == Some(he)).expect("the ship docks at the east harbor");
    let hw = harbor_on_a(&mut sim, west);
    ok(&mut sim, Cmd::StartExpedition { harbor: west });
    run_until(&mut sim, 12_000, |s| s.expedition_ready(hw))
        .unwrap_or_else(|| panic!("the expedition at the west harbor got no ship:\n{:?}", sim.ship(ship)));
}

/// Veteran round 11, #5: a rival across the water, however near, makes no border post (RTTR's
/// campaign "reachable" frontier rule): posts facing the sea only held soldiers needed elsewhere.
#[test]
fn a_rival_across_the_water_makes_no_border() {
    let mut m = MapData::flat(64, 64, WATER);
    let g = m.grid;
    for (center, r) in [(c(16, 20), 7u16), (c(34, 20), 7u16)] {
        for p in g.within(center, r) {
            let i = g.index(p);
            m.terrain_rsu[i] = MEADOW;
            m.terrain_usd[i] = MEADOW;
        }
    }
    m.hq = vec![Some(c(16, 20)), Some(c(34, 20))];
    let two = GameSetup { seed: 7, players: vec![Default::default(), Default::default()], ..Default::default() };
    let sim = Sim::new(Arc::new(Rules::classic()), &m, &two).unwrap();
    let hq = sim.building_at(c(16, 20)).unwrap();
    assert_eq!(sim.frontier_distance(hq), 0, "18 apart across water: no border");
    // the same two on one land: a border
    let land = MapData { terrain_rsu: vec![MEADOW; g.len()], terrain_usd: vec![MEADOW; g.len()], ..m };
    let sim = Sim::new(Arc::new(Rules::classic()), &land, &two).unwrap();
    let hq = sim.building_at(c(16, 20)).unwrap();
    assert!(sim.frontier_distance(hq) > 0, "on one land they face each other");
}

/// Two islands close enough to see across, a castle on each; the rival's island small enough
/// to lie wholly inside its castle's border, its harbor spot on the far side.
fn neighbour_islands() -> (Sim, MapCoord, MapCoord) {
    let mut m = MapData::flat(64, 48, WATER);
    let g = m.grid;
    let (ca, cb) = (c(14, 20), c(34, 20));
    for (center, r) in [(ca, 9), (cb, 5)] {
        for p in g.within(center, r) {
            let i = g.index(p);
            m.terrain_rsu[i] = MEADOW;
            m.terrain_usd[i] = MEADOW;
        }
    }
    let spot_a = g.ring(ca, 7).filter(|p| p.y % 2 == 0).min_by_key(|p| g.distance(*p, cb)).unwrap();
    let spot_b = g.ring(cb, 3).filter(|p| p.y % 2 == 0).max_by_key(|p| g.distance(*p, ca)).unwrap();
    m.harbors = vec![spot_a, spot_b];
    // (the rival's castle 11 from our harbor spot: in its sight)
    m.hq = vec![Some(ca), Some(c(32, 20))];
    let setup = GameSetup { seed: 7, players: vec![Default::default(), Default::default()], ..Default::default() };
    (Sim::new(Arc::new(Rules::classic()), &m, &setup).unwrap(), spot_a, spot_b)
}

/// Attacks from the sea (owner, 2026-10-27; RTTR sea attacks): soldiers from a post near our
/// harbor board a ship, land at the harbor spot by the rival's castle across the water, take it,
/// and sail home.
#[test]
fn soldiers_attack_across_the_sea_and_sail_home() {
    let (mut sim, a, b) = neighbour_islands();
    // strong soldiers at home, one private at the rival's castle
    sim.debug_add_job(0, rr_rules::Job::General, 6);
    let privates = sim.player_inventory(1).job(rr_rules::Job::Private);
    sim.debug_take_job(1, rr_rules::Job::Private, privates.saturating_sub(1));
    for j in [rr_rules::Job::PrivateFirstClass, rr_rules::Job::Sergeant, rr_rules::Job::Officer, rr_rules::Job::General]
    {
        let n = sim.player_inventory(1).job(j);
        sim.debug_take_job(1, j, n);
    }
    // (and no recruits: nothing to arm them with)
    for w in [Ware::Sword, Ware::Shield, Ware::Beer] {
        sim.debug_take_ware(1, w, 99);
    }
    ok(&mut sim, Cmd::SetMilitary { values: [10, 5, 5, 5, 8, 8, 8, 8] });
    // (roads to this map's castle at 14,20)
    let link = |sim: &mut Sim, at: MapCoord| {
        let f = sim.world().grid().neighbor(at, SE);
        let dirs = sim.road_path(0, f, walk(sim, c(14, 20), SE, 1), &[]).expect("road to the castle");
        ok(sim, Cmd::BuildRoad { from: f, dirs });
    };
    ok(&mut sim, Cmd::PlaceBuilding { at: a, kind: B::Harbor });
    link(&mut sim, a);
    let ha = sim.building_at(a).unwrap();
    run_until(&mut sim, 20_000, |s| s.building(ha).is_ready()).expect("harbor built");
    // a guardhouse near the harbor, whose soldiers may sail
    let g = sim.world().grid();
    let mut near: Vec<MapCoord> = g.within(a, 8).filter(|p| g.distance(*p, a) >= 4).collect();
    near.sort_by_key(|p| (g.distance(*p, a), p.y, p.x));
    let at = near
        .into_iter()
        .find(|p| {
            sim.world().bq_for(*p, 0).allows(rr_world::Bq::Hut)
                && !sim.military_building_near(*p, 0)
                && sim.road_path(0, g.neighbor(*p, SE), walk(&sim, c(14, 20), SE, 1), &[]).is_some()
        })
        .expect("a guardhouse spot by the harbor");
    ok(&mut sim, Cmd::PlaceBuilding { at, kind: B::Guardhouse });
    link(&mut sim, at);
    let gh = sim.building_at(at).unwrap();
    run_until(&mut sim, 30_000, |s| s.building(gh).soldiers.len() >= 3).expect("manned");
    sim.debug_spawn_ship(0, a);
    let castle = sim.building(sim.players()[1].hq.unwrap()).pos;
    run_until(&mut sim, 2_000, |s| s.is_attackable(0, s.building_at(castle).unwrap())).expect("the castle in sight");
    let t = sim.building_at(castle).unwrap();
    // (owner: "can I attack an island fully inside the enemy's border?") every land node of
    // the rival's island is the rival's, and still the soldiers may land there
    let island: Vec<MapCoord> =
        g.coords().filter(|p| sim.world().node_walkable(*p) && sim.same_land(*p, castle)).collect();
    let free: Vec<_> = island.iter().filter(|p| sim.world().owner(**p) != Some(1)).collect();
    assert!(
        island.len() > 50 && free.is_empty(),
        "the island all the rival's: {} of {} not: {free:?}",
        free.len(),
        island.len()
    );
    assert!(sim.sea_attack_plan(0, t).is_some(), "an attack by sea is possible");
    assert!(sim.attackers_for(0, castle) > 0);
    ok(&mut sim, Cmd::Attack { target: castle, soldiers: 2, strong: true });
    // they land on the rival's island
    let island_b =
        |s: &Sim| s.figures().iter().any(|(_, f)| f.player == 0 && f.inside.is_none() && s.same_land(f.pos, castle));
    run_until(&mut sim, 6_000, island_b).unwrap_or_else(|| panic!("nobody landed:\n{}", sim.dump_string()));
    // and take the castle (a warehouse: it burns)
    run_until(&mut sim, 10_000, |s| s.building_at(castle).is_none()).expect("the castle taken");
    // then they sail home and back into their post
    run_until(&mut sim, 10_000, |s| s.building(gh).soldiers.len() >= 3 && !island_b(s))
        .unwrap_or_else(|| panic!("not home:\n{}", sim.dump_string()));
    assert!(sim.ships().iter().all(|(_, s)| s.task == rr_sim::ShipTask::Idle), "the ship free again");
    let _ = (ha, b);
}
