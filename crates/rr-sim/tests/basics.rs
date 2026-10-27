mod common;
use common::*;
use rr_core::Direction::*;
use rr_rules::{BuildingType, Job, Ware};
use rr_sim::{Cmd, Event, RejectReason};
use rr_world::Bq;

#[test]
fn hq_is_placed_with_flag_and_territory() {
    let sim = sim1();
    let hq = sim.building_at(c(20, 20)).expect("hq");
    assert_eq!(sim.building(hq).kind, BuildingType::Headquarters);
    let flag = walk(&sim, c(20, 20), SE, 1);
    assert!(sim.flag_at(flag).is_some(), "hq flag at SE");
    // radius 9 territory
    assert_eq!(sim.world().owner(walk(&sim, c(20, 20), E, 9)), Some(0));
    assert_eq!(sim.world().owner(walk(&sim, c(20, 20), E, 10)), None);
    // the HQ occupies the large-building extension nodes
    assert_eq!(sim.world().bq(walk(&sim, c(20, 20), W, 1)), Bq::Nothing);
}

#[test]
fn hq_inventory_is_start_goods() {
    let sim = sim1();
    let inv = sim.player_inventory(0);
    assert_eq!(inv.ware(Ware::Boards), 44);
    assert_eq!(inv.job(Job::Helper), 52);
}

#[test]
fn time_advances_and_hash_is_deterministic() {
    let mut a = sim1();
    let mut b = sim1();
    for _ in 0..100 {
        a.step(&[]);
        b.step(&[]);
    }
    assert_eq!(a.gf().0, 100);
    assert_eq!(a.sync_hash(), b.sync_hash());
}

#[test]
fn place_flag_rules() {
    let mut sim = sim1();
    let p = c(26, 22);
    let out = exec(&mut sim, 0, Cmd::PlaceFlag { at: p });
    assert!(out.events.iter().all(|e| !matches!(e, Event::CommandRejected { .. })), "{:?}", out.events);
    assert!(sim.flag_at(p).is_some());
    // adjacent flag rejected
    let q = walk(&sim, p, E, 1);
    let out = exec(&mut sim, 0, Cmd::PlaceFlag { at: q });
    assert!(matches!(out.events[0], Event::CommandRejected { reason: RejectReason::NotBuildable, .. }));
    // outside territory rejected
    let out = exec(&mut sim, 0, Cmd::PlaceFlag { at: c(50, 50) });
    assert!(matches!(out.events[0], Event::CommandRejected { .. }));
    // other player's command on my land rejected
    let out = exec(&mut sim, 1, Cmd::PlaceFlag { at: c(24, 24) });
    assert!(matches!(out.events[0], Event::CommandRejected { .. }));
}

#[test]
fn build_road_between_flags() {
    let mut sim = sim1();
    let hq_flag = walk(&sim, c(20, 20), SE, 1); // (20,21)
    let out = exec(&mut sim, 0, Cmd::BuildRoad { from: hq_flag, dirs: vec![E, E, E, E] });
    assert!(out.events.iter().all(|e| !matches!(e, Event::CommandRejected { .. })), "{:?}", out.events);
    let end = walk(&sim, hq_flag, E, 4);
    assert!(sim.flag_at(end).is_some(), "flag auto-placed at the road end");
    let r = sim.road_between(hq_flag, end).expect("road");
    assert_eq!(sim.road(r).dirs.len(), 4);
    for i in 1..4 {
        assert!(sim.world().is_on_road(walk(&sim, hq_flag, E, i)));
    }
}

#[test]
fn road_rejections() {
    let mut sim = sim1();
    let hq_flag = walk(&sim, c(20, 20), SE, 1);
    // too short (1 edge) - flags can't be adjacent
    let out = exec(&mut sim, 0, Cmd::BuildRoad { from: hq_flag, dirs: vec![E] });
    assert!(matches!(out.events[0], Event::CommandRejected { .. }));
    // self-crossing
    let out = exec(&mut sim, 0, Cmd::BuildRoad { from: hq_flag, dirs: vec![E, E, W, W] });
    assert!(matches!(out.events[0], Event::CommandRejected { .. }));
    // through the HQ itself
    let out = exec(&mut sim, 0, Cmd::BuildRoad { from: hq_flag, dirs: vec![NW, NW, NW] });
    assert!(matches!(out.events[0], Event::CommandRejected { .. }));
    // must start at a flag
    let out = exec(&mut sim, 0, Cmd::BuildRoad { from: c(25, 25), dirs: vec![E, E] });
    assert!(matches!(out.events[0], Event::CommandRejected { .. }));
}

#[test]
fn flag_on_road_splits_it() {
    let mut sim = sim1();
    let a = walk(&sim, c(20, 20), SE, 1);
    exec(&mut sim, 0, Cmd::BuildRoad { from: a, dirs: vec![E, E, E, E, E, E] });
    let b = walk(&sim, a, E, 6);
    let mid = walk(&sim, a, E, 3);
    exec(&mut sim, 0, Cmd::PlaceFlag { at: mid });
    assert!(sim.flag_at(mid).is_some());
    assert!(sim.road_between(a, b).is_none());
    assert_eq!(sim.road(sim.road_between(a, mid).unwrap()).dirs.len(), 3);
    assert_eq!(sim.road(sim.road_between(mid, b).unwrap()).dirs.len(), 3);
    // a flag 1 edge from an existing flag is impossible
    let q = walk(&sim, a, E, 1);
    let out = exec(&mut sim, 0, Cmd::PlaceFlag { at: q });
    assert!(matches!(out.events[0], Event::CommandRejected { .. }));
}

#[test]
fn remove_flag_removes_its_roads() {
    let mut sim = sim1();
    let a = walk(&sim, c(20, 20), SE, 1);
    exec(&mut sim, 0, Cmd::BuildRoad { from: a, dirs: vec![E, E, E] });
    let b = walk(&sim, a, E, 3);
    exec(&mut sim, 0, Cmd::RemoveFlag { at: b });
    assert!(sim.flag_at(b).is_none());
    assert!(sim.road_between(a, b).is_none());
    assert!(!sim.world().is_on_road(walk(&sim, a, E, 1)));
    // the HQ flag can't be removed
    let out = exec(&mut sim, 0, Cmd::RemoveFlag { at: a });
    assert!(matches!(out.events[0], Event::CommandRejected { .. }));
}

/// Roast finding: every command with a node off the map is rejected, never a panic or a
/// wrapped-around node.
#[test]
fn commands_off_the_map_are_rejected() {
    use rr_core::Direction::E;
    let mut sim = sim1();
    for p in [c(5, 9999), c(9999, 5), c(64, 20), c(20, 64)] {
        let cmds = [
            Cmd::PlaceFlag { at: p },
            Cmd::RemoveFlag { at: p },
            Cmd::BuildRoad { from: p, dirs: vec![E, E] },
            Cmd::RemoveRoad { flag: p, dir: E },
            Cmd::PlaceBuilding { at: p, kind: rr_rules::BuildingType::Woodcutter },
            Cmd::RemoveBuilding { at: p },
            Cmd::ToggleProduction { at: p },
            Cmd::SendGeologist { flag: p },
            Cmd::SendScout { flag: p },
            Cmd::Attack { target: p, soldiers: 3, strong: true },
            Cmd::SetGarrisonWanted { at: p, wanted: 1 },
            Cmd::UpgradeRoad { flag: p, dir: E },
            Cmd::StartExpedition { harbor: p },
            Cmd::SendExpedition { harbor: c(20, 20), to: p },
        ];
        for cmd in cmds {
            let out = exec(&mut sim, 0, cmd.clone());
            assert!(
                out.events.iter().any(|e| matches!(e, rr_sim::Event::CommandRejected { .. })),
                "{cmd:?} not rejected"
            );
        }
    }
}

/// The original's starting conditions (RTTR `getStartInventory`: Low, Normal, ALot).
#[test]
fn starting_goods_low_normal_high() {
    let inv = |goods| {
        let mut m = rr_core::MapData::flat(64, 64, MEADOW);
        m.hq = vec![Some(c(20, 20))];
        let setup = rr_sim::GameSetup { seed: 1, players: vec![Default::default()], start_goods: goods };
        let sim = rr_sim::Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &m, &setup).unwrap();
        sim.player_inventory(0)
    };
    let (low, normal, high) =
        (inv(rr_sim::StartGoods::Low), inv(rr_sim::StartGoods::Normal), inv(rr_sim::StartGoods::High));
    assert_eq!((low.ware(Ware::Boards), normal.ware(Ware::Boards), high.ware(Ware::Boards)), (22, 44, 88));
    assert_eq!((low.job(Job::Private), normal.job(Job::Private), high.job(Job::Private)), (26, 46, 92));
    assert_eq!((low.ware(Ware::Beer), low.ware(Ware::Sword)), (0, 0));
    assert_eq!(low.job(Job::Helper), 26);
}

/// Veteran round 3, #9 (RTTR GamePlayer::LoadStandardToolSettings): the tool sliders start
/// where the original puts them.
#[test]
fn tool_settings_start_like_the_original() {
    use rr_rules::Ware as W;
    let sim = sim1();
    let want = [
        (W::Tongs, 1),
        (W::Hammer, 4),
        (W::Axe, 2),
        (W::Saw, 5),
        (W::PickAxe, 7),
        (W::Shovel, 1),
        (W::Crucible, 3),
        (W::RodAndLine, 1),
        (W::Scythe, 2),
        (W::Cleaver, 1),
        (W::Rollingpin, 2),
        (W::Bow, 1),
    ];
    assert_eq!(sim.players()[0].settings.tools, want.to_vec());
}

/// Architect round 2, #1: the campaign's teams in one place: the rivals allied, the player not.
#[test]
fn a_campaign_allies_the_rivals() {
    let setup = rr_sim::GameSetup::campaign(1, 6);
    let mut m = rr_core::MapData::flat(64, 64, MEADOW);
    m.hq = (0..6).map(|i| Some(c(8 + i * 9, 20 + (i % 2) * 20))).collect();
    let sim = rr_sim::Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &m, &setup).unwrap();
    for a in 1..6u8 {
        assert!(!sim.are_allies(0, a));
        for b in 1..6u8 {
            assert!(sim.are_allies(a, b));
        }
    }
}

/// Architect round 3, #5: one land count, the live map's, the same the statistics sample.
#[test]
fn one_land_count() {
    let mut sim = common::sim_with(&[common::c(12, 12), common::c(40, 30)]);
    let land = sim.land_by_player();
    assert_eq!(land.len(), 2);
    assert!(land.iter().all(|l| *l > 0), "{land:?}");
    common::run_until(&mut sim, 2000, |s| !s.players()[0].stats.samples.is_empty());
    let sampled: Vec<u32> = sim.players().iter().map(|p| p.stats.samples.last().unwrap().land).collect();
    assert_eq!(sim.land_by_player(), sampled);
}
