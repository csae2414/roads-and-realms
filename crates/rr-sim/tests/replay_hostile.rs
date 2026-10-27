//! Robustness roast #12: a replay (or any map) that is broken is refused when the game is set
//! up, not a crash inside the world.
use rr_core::{MapCoord, MapData};
use rr_rules::Rules;
use rr_sim::{GameSetup, PlayerSetup, SetupError, Sim};
use std::sync::Arc;

#[test]
fn broken_maps_are_refused_by_sim_new() {
    let mut ok = MapData::flat(32, 32, 8);
    ok.hq = vec![Some(MapCoord::new(8, 8))];
    let setup = GameSetup { players: vec![PlayerSetup::default()], ..Default::default() };
    let rules = Arc::new(Rules::classic());
    assert!(Sim::new(rules.clone(), &ok, &setup).is_ok());
    let mut short = ok.clone();
    short.altitude.truncate(10);
    let mut empty_objects = ok.clone();
    empty_objects.objects.clear();
    let mut far_hq = ok.clone();
    far_hq.hq = vec![Some(MapCoord { x: 5000, y: 5000 })];
    let mut far_harbor = ok.clone();
    far_harbor.harbors = vec![MapCoord { x: 3, y: 99 }];
    for bad in [short, empty_objects, far_hq, far_harbor] {
        assert!(matches!(Sim::new(rules.clone(), &bad, &setup), Err(SetupError::BadMap(_))));
    }
}

/// Robustness roast #9: lists longer than any player means are refused, the normal ones taken.
#[test]
fn oversized_lists_are_refused() {
    use rr_sim::{Cmd, Event, PlayerCommand};
    let mut map = MapData::flat(32, 32, 8);
    map.hq = vec![Some(MapCoord::new(8, 8))];
    let setup = GameSetup { players: vec![PlayerSetup::default()], ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &map, &setup).unwrap();
    let rejected = |sim: &mut Sim, cmd: Cmd| {
        sim.step(&[PlayerCommand { player: 0, cmd }]).events.iter().any(|e| matches!(e, Event::CommandRejected { .. }))
    };
    let all: Vec<rr_rules::Ware> = rr_rules::Ware::ALL.to_vec();
    assert!(!rejected(&mut sim, Cmd::SetTransportOrder { order: all.clone() }));
    assert!(rejected(&mut sim, Cmd::SetTransportOrder { order: vec![rr_rules::Ware::Water; 4_000_000] }));
    assert!(rejected(
        &mut sim,
        Cmd::BuildRoad { from: MapCoord::new(9, 9), dirs: vec![rr_core::Direction::E; rr_sim::MAX_ROAD_STEPS + 1] }
    ));
}
