//! New-player roast #1: a road click that can't work says why.
mod common;
use common::*;
use rr_core::Direction::*;
use rr_sim::{Cmd, RoadTrouble};

#[test]
fn clicking_a_building_aims_at_its_flag() {
    let sim = sim1();
    let hq_flag = walk(&sim, c(20, 20), SE, 1);
    assert_eq!(sim.road_click_target(c(20, 20)), hq_flag);
    // the castle's sprite also covers its extra nodes
    for d in [W, NW, NE] {
        assert_eq!(sim.road_click_target(walk(&sim, c(20, 20), d, 1)), hq_flag, "{d:?}");
    }
    assert_eq!(sim.road_click_target(c(30, 30)), c(30, 30));
}

#[test]
fn the_reason_for_a_failed_road() {
    let mut sim = sim1();
    let hq_flag = walk(&sim, c(20, 20), SE, 1);
    // outside the land
    assert_eq!(sim.road_trouble(0, hq_flag, c(60, 60), &[]), RoadTrouble::NotYourLand);
    // onto the headquarters itself
    assert_eq!(sim.road_trouble(0, hq_flag, c(20, 20), &[]), RoadTrouble::Blocked);
    // a road east and west of the HQ flag: straight to a goal behind it means crossing it
    exec(&mut sim, 0, Cmd::BuildRoad { from: hq_flag, dirs: vec![E; 6] });
    exec(&mut sim, 0, Cmd::BuildRoad { from: hq_flag, dirs: vec![W; 6] });
    let mid = walk(&sim, hq_flag, E, 3);
    let start = walk(&sim, mid, NE, 3);
    exec(&mut sim, 0, Cmd::PlaceFlag { at: start });
    assert!(sim.flag_at(start).is_some());
    let below = walk(&sim, mid, SE, 2);
    match sim.road_trouble(0, start, below, &[]) {
        RoadTrouble::CrossesRoad(p) => assert!(sim.road_at(p).is_some() && sim.flag_at(p).is_none(), "{p:?}"),
        t => panic!("{t:?}"),
    }
}

/// New-player roast #3: aiming at a far flag of the network, the road can join the network
/// close by instead of running along it.
#[test]
fn a_road_can_join_the_network_close_by() {
    let mut sim = sim1();
    let hq_flag = walk(&sim, c(20, 20), SE, 1);
    exec(&mut sim, 0, Cmd::BuildRoad { from: hq_flag, dirs: vec![E; 6] });
    exec(&mut sim, 0, Cmd::BuildRoad { from: hq_flag, dirs: vec![W; 6] });
    let east = walk(&sim, hq_flag, E, 6);
    let start = walk(&sim, walk(&sim, hq_flag, W, 4), SE, 3);
    exec(&mut sim, 0, Cmd::PlaceFlag { at: start });
    assert!(sim.flag_at(start).is_some());
    let direct = sim.road_path(0, start, east, &[]).expect("a road to the far flag");
    let (goal, dirs) = sim.road_join_path(0, start, east, &[]).expect("a way to join");
    assert!(dirs.len() <= 3 && dirs.len() < direct.len(), "{dirs:?} vs {direct:?}");
    assert!(sim.flag_at(goal).is_some() || sim.road_at(goal).is_some(), "joins at {goal:?}");
    // and it can be built: a flag at the goal, then the road
    if sim.flag_at(goal).is_none() {
        exec(&mut sim, 0, Cmd::PlaceFlag { at: goal });
    }
    let out = exec(&mut sim, 0, Cmd::BuildRoad { from: start, dirs });
    assert!(out.events.iter().all(|e| !matches!(e, rr_sim::Event::CommandRejected { .. })), "{:?}", out.events);
    let s = sim.flag_at(start).unwrap();
    assert!(sim.flag_distance(s, sim.flag_at(east).unwrap()).is_some(), "connected");
}
