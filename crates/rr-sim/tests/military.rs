mod common;
use common::*;
use rr_core::Direction::*;
use rr_core::MapCoord;
use rr_rules::{BuildingType as B, Job};
use rr_sim::{Cmd, Event, PlayerCommand, Sim};

fn ok(sim: &mut Sim, player: u8, cmd: Cmd) {
    let out = sim.step(&[PlayerCommand { player, cmd: cmd.clone() }]);
    assert!(out.events.iter().all(|e| !matches!(e, Event::CommandRejected { .. })), "{cmd:?}: {:?}", out.events);
}

fn build(sim: &mut Sim, player: u8, kind: B, at: MapCoord, hq: MapCoord) -> rr_sim::BuildingId {
    ok(sim, player, Cmd::PlaceBuilding { at, kind });
    let b = sim.building_at(at).unwrap();
    let f = sim.flag(sim.building(b).flag).pos;
    let hq_flag = walk(sim, hq, SE, 1);
    let dirs = sim.road_path(player, f, hq_flag, &[]).expect("road");
    ok(sim, player, Cmd::BuildRoad { from: f, dirs });
    b
}

#[test]
fn soldiers_occupy_and_attack_captures() {
    let (h0, h1) = (c(14, 20), c(46, 20));
    let mut sim = sim_with(&[h0, h1]);
    // full garrisons whatever the border distance
    for p in [0, 1] {
        ok(&mut sim, p, Cmd::SetMilitary { values: [10, 5, 5, 5, 8, 8, 8, 8] });
    }
    // player 0: generals; player 1: only privates (default)
    sim.debug_add_job(0, Job::General, 6);
    // the defender has few reserves (and can't recruit: no beer)
    sim.debug_take_job(1, Job::Private, 42);
    sim.debug_take_ware(1, rr_rules::Ware::Beer, 99);
    let g0 = build(&mut sim, 0, B::Guardhouse, c(21, 20), h0);
    let b1 = build(&mut sim, 1, B::Barracks, c(39, 20), h1);
    run_until(&mut sim, 20_000, |s| s.building(g0).soldiers.len() == 3 && s.building(b1).soldiers.len() == 2)
        .unwrap_or_else(|| panic!("garrisons:\n{}", sim.dump_string()));
    // the guardhouse got the strongest soldiers first (setting 0 = 10)
    let ranks: Vec<_> = sim.building(g0).soldiers.iter().map(|s| sim.figure(*s).job).collect();
    assert!(ranks.iter().all(|j| *j == Job::General), "{ranks:?}");
    // second guardhouse closer to the enemy so the barracks is in range
    let g1 = build(&mut sim, 0, B::Guardhouse, c(28, 20), h0);
    run_until(&mut sim, 20_000, |s| s.building(g1).soldiers.len() == 3).expect("second guardhouse manned");
    let target = sim.building(b1).pos;
    // veteran round 11, #6: the defender keeps its gold off there and holds a coin
    ok(&mut sim, 1, Cmd::ToggleProduction { at: target });
    sim.debug_set_stock(b1, 0, 1);
    // all but one soldier per building may attack
    ok(&mut sim, 0, Cmd::SetMilitary { values: [10, 5, 5, 5, 8, 8, 8, 8] });
    ok(&mut sim, 0, Cmd::Attack { target, soldiers: 4, strong: true });
    // soldiers out on the attack still count: no replacements are ordered meanwhile
    for _ in 0..300 {
        sim.step(&[]);
        assert_eq!(sim.building(g0).soldiers_coming + sim.building(g1).soldiers_coming, 0, "replacements ordered");
    }
    let mut captured = false;
    let mut coins = 0;
    for _ in 0..20_000 {
        coins = sim.building(b1).stock[0];
        let out = sim.step(&[]);
        captured |= out.events.iter().any(|e| matches!(e, Event::BuildingCaptured { by: 0, .. }));
        if captured {
            break;
        }
    }
    assert!(captured, "barracks captured:\n{}", sim.dump_string());
    let now = sim.building_at(target).unwrap();
    assert_eq!(sim.building(now).player, 0);
    // RTTR nobMilitary::Capture: the coins go to the new owner, the coin setting stays, the
    // troop limit is the building's most
    assert_eq!(sim.building(now).stock[0], coins, "the coins handed over");
    assert!(sim.building(now).coins_stopped, "the coin setting kept");
    assert_eq!(sim.building(now).garrison_wanted, 2, "a barracks' most");
    assert!(sim.wares().iter().all(|(_, w)| w.goal != Some(now) || w.player == 0), "old deliveries released");
    assert!(sim.figures().iter().all(|(_, f)| f.player == 0 || f.target_building() != Some(now)));
    sim.step(&[]);
    assert_eq!(sim.world().owner(target), Some(0), "territory follows the capture");
    // the other attackers waiting there move in (a barracks holds 2)
    run_until(&mut sim, 400, |s| s.building(now).soldiers.len() == 2)
        .unwrap_or_else(|| panic!("captured barracks garrison:\n{}", sim.dump_string()));
}

#[test]
fn losing_all_buildings_defeats_a_player() {
    let mut sim = sim_with(&[c(14, 20), c(46, 20)]);
    let mut defeated = false;
    let out = sim.step(&[PlayerCommand { player: 1, cmd: Cmd::Surrender }]);
    defeated |= out.events.iter().any(|e| matches!(e, Event::PlayerDefeated { player: 1 }));
    assert!(defeated);
    assert!(!sim.players()[1].alive);
}

#[test]
fn catapult_shoots_at_enemy_garrisons() {
    let (h0, h1) = (c(14, 20), c(46, 20));
    let mut sim = sim_with(&[h0, h1]);
    sim.debug_take_job(1, Job::Private, 40);
    sim.debug_take_ware(1, rr_rules::Ware::Beer, 99);
    let b1 = build(&mut sim, 1, B::Guardhouse, c(38, 20), h1);
    let g0 = build(&mut sim, 0, B::Guardhouse, c(21, 20), h0);
    run_until(&mut sim, 20_000, |s| !s.building(g0).soldiers.is_empty() && s.building(b1).soldiers.len() >= 2)
        .expect("garrisons");
    // a guardhouse closer by: the catapult only throws at what its owner can see
    let g1 = build(&mut sim, 0, B::Guardhouse, c(28, 20), h0);
    run_until(&mut sim, 20_000, |s| !s.building(g1).soldiers.is_empty()).expect("manned");
    let cat = build(&mut sim, 0, B::Catapult, c(26, 22), h0);
    run_until(&mut sim, 30_000, |s| s.building(cat).is_ready() && s.building(cat).worker.is_some())
        .expect("catapult runs");
    let before = sim.building(b1).soldiers.len() as i32;
    let mut killed = 0;
    for _ in 0..20_000 {
        let n = sim.buildings().get(b1).map_or(0, |b| b.soldiers.len()) as i32;
        if n < before {
            killed = before - n;
            break;
        }
        sim.step(&[]);
    }
    assert!(killed > 0, "the catapult hit nobody:\n{}", sim.dump_string());
}

#[test]
fn defenders_intercept_attackers_in_the_open() {
    let (h0, h1) = (c(14, 20), c(46, 20));
    let mut sim = sim_with(&[h0, h1]);
    sim.debug_take_ware(1, rr_rules::Ware::Beer, 99);
    ok(&mut sim, 1, Cmd::SetMilitary { values: [10, 3, 5, 3, 8, 8, 8, 8] });
    // the defender's watchtower stands next to the target, full of soldiers
    let target = build(&mut sim, 1, B::Barracks, c(38, 20), h1);
    let tower = build(&mut sim, 1, B::Watchtower, c(42, 24), h1);
    let g0 = build(&mut sim, 0, B::Guardhouse, c(21, 20), h0);
    run_until(&mut sim, 30_000, |s| {
        s.building(g0).soldiers.len() == 3
            && s.building(tower).soldiers.len() >= 4
            && s.building(target).soldiers.len() == 2
    })
    .expect("garrisons");
    // a second guardhouse closer by, so the target is in sight (unseen buildings can't be attacked)
    let g1 = build(&mut sim, 0, B::Guardhouse, c(28, 20), h0);
    run_until(&mut sim, 20_000, |s| s.building(g1).soldiers.len() == 3).expect("second guardhouse manned");
    ok(&mut sim, 0, Cmd::SetMilitary { values: [10, 5, 5, 5, 8, 8, 8, 8] });
    let at = sim.building(target).pos;
    ok(&mut sim, 0, Cmd::Attack { target: at, soldiers: 2, strong: true });
    let mut intercepted = false;
    for _ in 0..6000 {
        sim.step(&[]);
        intercepted |= sim
            .figures()
            .iter()
            .any(|(_, f)| matches!(f.kind, rr_sim::FigKind::Soldier(rr_sim::SoldierTask::Intercept { .. })));
        if intercepted {
            break;
        }
    }
    assert!(intercepted, "nobody came out to meet the attackers");
}

/// Roast finding: after the HQ falls, giving up the last military building ends the player.
#[test]
fn losing_the_last_military_building_defeats() {
    let (h0, h1) = (c(14, 20), c(46, 20));
    let mut sim = sim_with(&[h0, h1]);
    let g = build(&mut sim, 1, B::Guardhouse, c(40, 20), h1);
    run_until(&mut sim, 20_000, |s| !s.building(g).soldiers.is_empty()).expect("guardhouse manned");
    assert_eq!(sim.hq_pos(1), Some(h1));
    sim.debug_destroy_building(h1);
    // performance roast #1: the app asked for the lost HQ and crashed
    assert_eq!(sim.hq_pos(1), None);
    assert!(sim.players()[1].alive, "the guardhouse still stands");
    let out = sim.step(&[PlayerCommand { player: 1, cmd: Cmd::RemoveBuilding { at: c(40, 20) } }]);
    assert!(out.events.iter().any(|e| matches!(e, Event::PlayerDefeated { player: 1 })), "{:?}", out.events);
    assert!(!sim.players()[1].alive);
}

/// Roast finding: a smaller garrison sends its weakest soldiers home, not the oldest.
#[test]
fn reducing_a_garrison_keeps_the_strongest() {
    let mut sim = sim_with(&[c(14, 20), c(46, 20)]);
    sim.debug_add_job(0, Job::General, 1);
    ok(&mut sim, 0, Cmd::SetMilitary { values: [10, 3, 5, 3, 8, 8, 8, 8] });
    let g = build(&mut sim, 0, B::Guardhouse, c(21, 20), c(14, 20));
    run_until(&mut sim, 20_000, |s| s.building(g).soldiers.len() == 3).expect("manned");
    let at = sim.building(g).pos;
    ok(&mut sim, 0, Cmd::SetGarrisonWanted { at, wanted: 1 });
    run_until(&mut sim, 2000, |s| s.building(g).soldiers.len() == 1).expect("two sent home");
    let left = sim.figure(sim.building(g).soldiers[0]).job;
    assert_eq!(left, Job::General);
}

/// Roast finding: attacks can send the weakest soldiers (RTTR's attack window).
#[test]
fn weak_attack_sends_the_weakest() {
    let (h0, h1) = (c(14, 20), c(46, 20));
    let mut sim = sim_with(&[h0, h1]);
    sim.debug_add_job(0, Job::General, 4);
    ok(&mut sim, 0, Cmd::SetMilitary { values: [10, 3, 5, 5, 8, 8, 8, 8] });
    let g0 = build(&mut sim, 0, B::Guardhouse, c(21, 20), h0);
    run_until(&mut sim, 20_000, |s| s.building(g0).is_ready() && !s.building(g0).soldiers.is_empty()).expect("first");
    let g = build(&mut sim, 0, B::Guardhouse, c(28, 20), h0);
    let b1 = build(&mut sim, 1, B::Barracks, c(39, 20), h1);
    run_until(&mut sim, 20_000, |s| s.building(g).soldiers.len() == 3 && s.building(b1).is_ready()).expect("ready");
    let ranks: Vec<_> = sim.building(g).soldiers.iter().map(|s| sim.figure(*s).job).collect();
    assert!(ranks.contains(&Job::General) && ranks.contains(&Job::Private), "{ranks:?}");
    let target = sim.building(b1).pos;
    ok(&mut sim, 0, Cmd::Attack { target, soldiers: 1, strong: false });
    let att = sim
        .figures()
        .iter()
        .find(|(_, f)| matches!(f.kind, rr_sim::FigKind::Soldier(rr_sim::SoldierTask::Attack { .. })))
        .map(|(_, f)| f.job);
    assert_eq!(att, Some(Job::Private));
}

/// QA round 2: a building torn down during a duel releases both duellists.
#[test]
fn duellists_go_home_when_the_building_vanishes() {
    let (h0, h1) = (c(14, 20), c(46, 20));
    let mut sim = sim_with(&[h0, h1]);
    sim.debug_add_job(0, Job::General, 6);
    ok(&mut sim, 0, Cmd::SetMilitary { values: [10, 3, 5, 5, 8, 8, 8, 8] });
    let g0 = build(&mut sim, 0, B::Guardhouse, c(21, 20), h0);
    run_until(&mut sim, 20_000, |s| s.building(g0).soldiers.len() == 3).expect("first");
    let g1 = build(&mut sim, 0, B::Guardhouse, c(28, 20), h0);
    let b1 = build(&mut sim, 1, B::Barracks, c(39, 20), h1);
    run_until(&mut sim, 20_000, |s| s.building(g1).soldiers.len() == 3 && s.building(b1).soldiers.len() == 2)
        .expect("manned");
    let target = sim.building(b1).pos;
    ok(&mut sim, 0, Cmd::Attack { target, soldiers: 2, strong: true });
    let b1id = b1;
    run_until(&mut sim, 6000, |s| {
        s.figures().iter().any(|(_, f)| {
            f.kind == rr_sim::FigKind::Soldier(rr_sim::SoldierTask::Defend(b1id))
                && matches!(f.motion, rr_sim::Motion::Work { .. })
        })
    })
    .expect("a duel is fought");
    let duellists: Vec<_> = sim
        .figures()
        .iter()
        .filter(|(_, f)| {
            matches!(
                f.kind,
                rr_sim::FigKind::Soldier(rr_sim::SoldierTask::Defend(_) | rr_sim::SoldierTask::AtTarget { .. })
            )
        })
        .map(|(i, _)| i)
        .collect();
    ok(&mut sim, 1, Cmd::RemoveBuilding { at: target });
    for _ in 0..5000 {
        sim.step(&[]);
    }
    // everybody is home again (or gone)
    for f in duellists {
        if let Some(x) = sim.figures().get(f) {
            assert!(x.inside.is_some() || matches!(x.motion, rr_sim::Motion::Walk { .. }), "soldier frozen: {x:?}");
        }
    }
}

/// QA round 2: pressing the garrison buttons doesn't heal soldiers faster.
#[test]
fn garrison_buttons_do_not_heal() {
    let mut sim = sim_with(&[c(14, 20), c(46, 20)]);
    sim.debug_add_job(0, Job::General, 1);
    ok(&mut sim, 0, Cmd::SetMilitary { values: [10, 3, 5, 3, 8, 8, 8, 8] });
    let g = build(&mut sim, 0, B::Guardhouse, c(21, 20), c(14, 20));
    run_until(&mut sim, 20_000, |s| !s.building(g).soldiers.is_empty()).expect("manned");
    let s = sim.building(g).soldiers[0];
    sim.debug_set_hp(s, 1);
    let at = sim.building(g).pos;
    for i in 0..300u32 {
        let wanted = 1 + (i % 3) as u8;
        sim.step(&[PlayerCommand { player: 0, cmd: Cmd::SetGarrisonWanted { at, wanted } }]);
    }
    assert!(sim.figure(s).hp <= 2, "healed {} hit points in 300 GF", sim.figure(s).hp - 1);
}

/// Veteran round 3, #1 (RTTR GameWorld::SetBuildingSite / IsMilitaryBuildingNearNode): no
/// military building within 4 nodes of an own military building, site, HQ or harbor.
#[test]
fn military_buildings_keep_their_distance() {
    let mut sim = sim1();
    let first = c(26, 20);
    let out = exec(&mut sim, 0, Cmd::PlaceBuilding { at: first, kind: B::Barracks });
    assert!(out.events.iter().all(|e| !matches!(e, Event::CommandRejected { .. })), "{:?}", out.events);
    let near = walk(&sim, first, rr_core::Direction::E, 3);
    assert!(sim.military_building_near(near, 0));
    let out = exec(&mut sim, 0, Cmd::PlaceBuilding { at: near, kind: B::Guardhouse });
    assert!(out.events.iter().any(|e| matches!(e, Event::CommandRejected { .. })), "3 nodes from a barracks site");
    // close to the HQ too
    assert!(sim.military_building_near(walk(&sim, c(20, 20), rr_core::Direction::W, 4), 0));
    // other buildings may stand there, and military ones further away
    let other = sim
        .world()
        .grid()
        .within(first, 4)
        .find(|p| sim.world().bq_for(*p, 0).allows(rr_world::Bq::Hut))
        .expect("a hut spot near the barracks");
    assert!(sim.military_building_near(other, 0));
    let out = exec(&mut sim, 0, Cmd::PlaceBuilding { at: other, kind: B::Woodcutter });
    assert!(out.events.iter().all(|e| !matches!(e, Event::CommandRejected { .. })), "{:?}", out.events);
    let far = sim
        .world()
        .grid()
        .within(c(20, 20), 8)
        .find(|p| sim.world().bq_for(*p, 0).allows(rr_world::Bq::Hut) && !sim.military_building_near(*p, 0))
        .expect("a spot away from military buildings");
    let out = exec(&mut sim, 0, Cmd::PlaceBuilding { at: far, kind: B::Guardhouse });
    assert!(out.events.iter().all(|e| !matches!(e, Event::CommandRejected { .. })), "{:?}", out.events);
}

/// Veteran round 3, #2 (RTTR: land follows `new_built`, not who is inside): a building whose
/// soldiers are all out defending keeps its land, even when the territory is worked out anew.
#[test]
fn a_building_keeps_its_land_while_its_soldiers_defend() {
    let h0 = c(20, 20);
    let mut sim = sim_with(&[h0]);
    let g = build(&mut sim, 0, B::Guardhouse, c(27, 20), h0);
    run_until(&mut sim, 20_000, |s| !s.building(g).soldiers.is_empty()).expect("manned");
    assert!(sim.building(g).occupied);
    let at = sim.building(g).pos;
    let edge = walk(&sim, at, E, 5);
    assert_eq!(sim.world().owner(edge), Some(0), "the guardhouse's land");
    sim.debug_send_defenders(at);
    assert!(sim.building(g).soldiers.is_empty());
    assert_eq!(sim.building_at(at), Some(g), "still standing");
    assert_eq!(sim.world().owner(edge), Some(0), "still its land");
}

/// Veteran round 3, #3 (RTTR nobMilitary::IsAttackable): a building nobody has moved into yet
/// can't be attacked.
#[test]
fn a_new_empty_building_cant_be_attacked() {
    let (h0, h1) = (c(14, 20), c(46, 20));
    let mut sim = sim_with(&[h0, h1]);
    let g0 = build(&mut sim, 0, B::Guardhouse, c(21, 20), h0);
    run_until(&mut sim, 20_000, |s| !s.building(g0).soldiers.is_empty()).expect("manned");
    let g0b = build(&mut sim, 0, B::Guardhouse, c(28, 20), h0);
    run_until(&mut sim, 20_000, |s| !s.building(g0b).soldiers.is_empty()).expect("manned");
    // player 1 has no soldiers to send: the barracks stays empty when finished
    sim.debug_take_job(1, Job::Private, 99);
    sim.debug_take_ware(1, rr_rules::Ware::Beer, 99);
    let b1 = build(&mut sim, 1, B::Barracks, c(39, 20), h1);
    run_until(&mut sim, 20_000, |s| s.building(b1).is_ready()).expect("built");
    assert!(sim.building(b1).soldiers.is_empty() && !sim.building(b1).occupied);
    let out = sim.step(&[PlayerCommand {
        player: 0,
        cmd: Cmd::Attack { target: sim.building(b1).pos, soldiers: 1, strong: true },
    }]);
    assert!(
        out.events.iter().any(|e| matches!(e, Event::CommandRejected { reason: rr_sim::RejectReason::NotAllowed, .. })),
        "{:?}",
        out.events
    );
}

/// Veteran round 3, #5 (RTTR GameWorld::Attack): attackers are picked across all buildings,
/// by rank first (strong or weak), then by distance, not building by building.
#[test]
fn attackers_are_picked_by_rank_across_buildings() {
    let (h0, h1) = (c(14, 20), c(46, 20));
    for (strong, want) in [(true, Job::General), (false, Job::Private)] {
        let mut sim = sim_with(&[h0, h1]);
        sim.debug_take_ware(1, rr_rules::Ware::Beer, 99);
        sim.debug_take_ware(0, rr_rules::Ware::Beer, 99);
        // full garrisons everywhere; the far guardhouse gets generals, the near one privates
        ok(&mut sim, 0, Cmd::SetMilitary { values: [10, 5, 5, 10, 8, 8, 8, 8] });
        sim.debug_take_job(0, Job::Private, 99);
        sim.debug_add_job(0, Job::General, 4);
        let far = build(&mut sim, 0, B::Guardhouse, c(21, 20), h0);
        run_until(&mut sim, 20_000, |s| s.building(far).soldiers.len() == 3).unwrap_or_else(|| {
            panic!("far: {:?}", sim.building(far).soldiers.iter().map(|f| sim.figure(*f).job).collect::<Vec<_>>())
        });
        sim.debug_take_job(0, Job::General, 99);
        sim.debug_add_job(0, Job::Private, 4);
        let near = build(&mut sim, 0, B::Guardhouse, c(28, 20), h0);
        run_until(&mut sim, 20_000, |s| s.building(near).soldiers.len() == 3).expect("near manned");
        assert!(sim.building(near).soldiers.iter().all(|f| sim.figure(*f).job == Job::Private));
        assert!(sim.building(far).soldiers.iter().all(|f| sim.figure(*f).job == Job::General));
        let b1 = build(&mut sim, 1, B::Barracks, c(39, 20), h1);
        run_until(&mut sim, 20_000, |s| !s.building(b1).soldiers.is_empty()).expect("target manned");
        ok(&mut sim, 0, Cmd::SetMilitary { values: [10, 5, 5, 10, 8, 8, 8, 8] });
        assert!(sim.attackers_available(far, 18) > 0, "the far guardhouse may send");
        let target = sim.building(b1).pos;
        ok(&mut sim, 0, Cmd::Attack { target, soldiers: 1, strong });
        let sent: Vec<Job> = sim
            .figures()
            .iter()
            .filter(|(_, f)| matches!(f.kind, rr_sim::FigKind::Soldier(rr_sim::SoldierTask::Attack { .. })))
            .map(|(_, f)| f.job)
            .collect();
        assert_eq!(sent, vec![want], "strong: {strong}");
    }
}

/// Veteran round 3, #6 (RTTR nobMilitary::ChooseSoldier): the defense setting picks which rank
/// steps out to defend: of the ranks inside, the one at (ranks - 1) * setting / 5.
#[test]
fn the_defense_setting_picks_the_defenders_rank() {
    let h0 = c(20, 20);
    let mut sim = sim_with(&[h0]);
    ok(&mut sim, 0, Cmd::SetMilitary { values: [10, 5, 5, 10, 8, 8, 8, 8] });
    sim.debug_take_job(0, Job::Private, 99);
    for j in [Job::Private, Job::Sergeant, Job::General] {
        sim.debug_add_job(0, j, 2);
    }
    sim.debug_add_job(0, Job::General, 1);
    let t = build(&mut sim, 0, B::Watchtower, c(27, 20), h0);
    run_until(&mut sim, 30_000, |s| s.building(t).soldiers.len() == 6).expect("watchtower full");
    let rank = |sim: &Sim| sim.defender_choice(t).map(|f| sim.figure(f).job);
    for (setting, want) in [(0, Job::Private), (2, Job::Private), (3, Job::Sergeant), (5, Job::General)] {
        ok(&mut sim, 0, Cmd::SetMilitary { values: [10, setting, 5, 10, 8, 8, 8, 8] });
        assert_eq!(rank(&sim), Some(want), "setting {setting}");
    }
}

/// Veteran round 3, #11 (RTTR nofCatapultMan / nobMilitary::HitOfCatapultStone): a catapult
/// throws at visible, occupied military buildings within 14, kills the weakest soldier inside,
/// and the owner is told.
#[test]
fn a_catapult_hits_the_weakest_and_tells_the_owner() {
    let (h0, h1) = (c(14, 20), c(46, 20));
    let mut sim = sim_with(&[h0, h1]);
    for p in [0, 1] {
        ok(&mut sim, p, Cmd::SetMilitary { values: [10, 5, 5, 5, 8, 8, 8, 8] });
    }
    sim.debug_take_ware(1, rr_rules::Ware::Beer, 99);
    // player 1's barracks: a general and a private
    sim.debug_take_job(1, Job::Private, 99);
    sim.debug_add_job(1, Job::General, 2);
    sim.debug_add_job(1, Job::Private, 1);
    let b1 = build(&mut sim, 1, B::Barracks, c(39, 20), h1);
    run_until(&mut sim, 20_000, |s| s.building(b1).soldiers.len() == 2).expect("barracks manned");
    let g0 = build(&mut sim, 0, B::Guardhouse, c(21, 20), h0);
    run_until(&mut sim, 20_000, |s| !s.building(g0).soldiers.is_empty()).expect("manned");
    let g1 = build(&mut sim, 0, B::Guardhouse, c(28, 20), h0);
    run_until(&mut sim, 20_000, |s| !s.building(g1).soldiers.is_empty()).expect("manned");
    sim.debug_add_ware(0, rr_rules::Ware::Stones, 20);
    let cat = build(&mut sim, 0, B::Catapult, c(31, 22), h0);
    run_until(&mut sim, 20_000, |s| s.building(cat).is_ready() && s.building(cat).worker.is_some())
        .expect("catapult working");
    let mut told = false;
    let done = run_until_events(&mut sim, 30_000, |s, ev| {
        told |= ev.iter().any(|e| matches!(e, Event::CatapultHit { player: 1, .. }));
        s.building(b1).soldiers.len() < 2
    });
    assert!(done, "no hit");
    assert!(told, "the owner hears of it");
    let left: Vec<Job> = sim.building(b1).soldiers.iter().map(|f| sim.figure(*f).job).collect();
    assert_eq!(left, vec![Job::General], "the private was hit first");
}

/// Step until `f` holds, giving it each frame's events.
fn run_until_events(sim: &mut Sim, max: u32, mut f: impl FnMut(&Sim, &[Event]) -> bool) -> bool {
    for _ in 0..max {
        let out = sim.step(&[]);
        if f(sim, &out.events) {
            return true;
        }
    }
    false
}

/// Veteran round 3 (RTTR nobBaseWarehouse reserves): the headquarters sends all its soldiers
/// unless the player keeps some in reserve, rank by rank.
#[test]
fn the_hq_keeps_only_the_soldiers_held_in_reserve() {
    for (reserve, left) in [(0u8, 0u32), (1, 1)] {
        let h0 = c(20, 20);
        let mut sim = sim_with(&[h0]);
        ok(&mut sim, 0, Cmd::SetMilitary { values: [10, 5, 5, 5, 8, 8, 8, 8] });
        sim.debug_take_ware(0, rr_rules::Ware::Beer, 99);
        sim.debug_take_job(0, Job::Private, 99);
        sim.debug_add_job(0, Job::Private, 1);
        ok(&mut sim, 0, Cmd::SetReserve { at: h0, rank: 0, count: reserve });
        let g = build(&mut sim, 0, B::Guardhouse, c(27, 20), h0);
        run_until(&mut sim, 20_000, |s| s.building(g).is_ready()).expect("built");
        for _ in 0..3000 {
            sim.step(&[]);
        }
        let hq = sim.building_at(h0).unwrap();
        assert_eq!(sim.building(hq).inventory.as_ref().unwrap().job(Job::Private), left, "reserve {reserve}");
    }
}

/// QA round 3, #7 (RTTR LookForMilitaryBuildings sees buildings, not sites): an enemy
/// construction site doesn't change how close to the border a building counts.
#[test]
fn enemy_sites_dont_count_for_the_border() {
    let (h0, h1) = (c(14, 20), c(46, 20));
    let mut sim = sim_with(&[h0, h1]);
    let g0 = build(&mut sim, 0, B::Guardhouse, c(21, 20), h0);
    run_until(&mut sim, 20_000, |s| !s.building(g0).soldiers.is_empty()).expect("manned");
    let before = sim.frontier_distance(g0);
    // a barracks site of player 1 close by, never built (no road)
    ok(&mut sim, 1, Cmd::PlaceBuilding { at: c(38, 20), kind: B::Barracks });
    assert!(sim.building_at(c(38, 20)).is_some_and(|b| !sim.building(b).is_ready()));
    assert_eq!(sim.frontier_distance(g0), before);
}

/// New-player round 3, #1: the game can tell when a player has no spot left for a military
/// building (all free land within 4 nodes of the HQ or another military building).
#[test]
fn no_room_for_military_buildings_is_noticed() {
    assert!(sim1().military_spot_exists(0), "a fresh start has room");
    // an island just big enough for the headquarters
    let mut m = rr_core::MapData::flat(40, 40, 5);
    let hq = c(20, 20);
    for p in m.grid.within(hq, 4).collect::<Vec<_>>() {
        let i = m.grid.index(p);
        m.terrain_rsu[i] = MEADOW;
        m.terrain_usd[i] = MEADOW;
    }
    m.hq = vec![Some(hq)];
    let setup = rr_sim::GameSetup { seed: 1, players: vec![Default::default()], ..Default::default() };
    let sim = Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &m, &setup).unwrap();
    assert!(!sim.military_spot_exists(0));
}

/// Veteran round 4, #3 (RTTR nofPassiveSoldier: each soldier heals on his own timer): three
/// wounded soldiers heal together, not one after the other.
#[test]
fn every_soldier_heals_on_his_own() {
    let mut sim = sim_with(&[c(14, 20), c(46, 20)]);
    ok(&mut sim, 0, Cmd::SetMilitary { values: [10, 3, 5, 3, 8, 8, 8, 8] });
    let g = build(&mut sim, 0, B::Guardhouse, c(21, 20), c(14, 20));
    run_until(&mut sim, 40_000, |s| s.building(g).soldiers.len() >= 3).expect("manned");
    let soldiers: Vec<_> = sim.building(g).soldiers.iter().take(3).copied().collect();
    for s in &soldiers {
        sim.debug_set_hp(*s, 1);
    }
    // a private has 3 hit points: two heals each, 500-1000 GF apiece
    for _ in 0..2200 {
        sim.step(&[]);
    }
    for s in &soldiers {
        assert_eq!(sim.figure(*s).hp, 3, "soldier {s:?} still wounded");
    }
}

/// RTTR: a warehouse picks its defender by the defense setting like a military building does
/// (weakest at 0, strongest at full), not always the strongest.
#[test]
fn warehouse_defender_follows_the_defense_setting() {
    let mut sim = sim1();
    sim.debug_add_job(0, Job::General, 2);
    let hq = sim.building_at(c(20, 20)).unwrap();
    ok(&mut sim, 0, Cmd::SetMilitary { values: [10, 0, 5, 5, 8, 8, 8, 8] });
    assert_eq!(sim.warehouse_defender(hq), Some(Job::Private));
    ok(&mut sim, 0, Cmd::SetMilitary { values: [10, 5, 5, 5, 8, 8, 8, 8] });
    assert_eq!(sim.warehouse_defender(hq), Some(Job::General));
}

/// RTTR nobBaseWarehouse: strong soldiers go first from a setting of half the scale (2 of 5) up.
#[test]
fn strong_soldiers_go_first_from_setting_two() {
    let mut sim = sim1();
    sim.debug_add_job(0, Job::General, 3);
    ok(&mut sim, 0, Cmd::SetMilitary { values: [10, 2, 5, 5, 8, 8, 8, 8] });
    let g = build(&mut sim, 0, B::Guardhouse, c(26, 20), c(20, 20));
    run_until(&mut sim, 20_000, |s| !s.building(g).soldiers.is_empty()).expect("manned");
    let s = sim.building(g).soldiers[0];
    assert_eq!(sim.figure(s).job, Job::General);
}

/// RTTR nobMilitary::SetCoinsAllowed(false): coins already ordered turn back to a warehouse.
#[test]
fn gold_off_sends_ordered_coins_back() {
    let mut sim = sim1();
    let g = build(&mut sim, 0, B::Guardhouse, c(27, 20), c(20, 20));
    run_until(&mut sim, 20_000, |s| !s.building(g).soldiers.is_empty()).expect("manned");
    sim.debug_add_ware(0, rr_rules::Ware::Coins, 4);
    run_until(&mut sim, 2_000, |s| s.building(g).incoming[0] > 0).expect("coins ordered");
    let at = sim.building(g).pos;
    ok(&mut sim, 0, Cmd::ToggleProduction { at });
    assert_eq!(sim.building(g).incoming[0], 0, "nothing expected any more");
    for _ in 0..6_000 {
        sim.step(&[]);
    }
    assert_eq!(sim.building(g).stock[0], 0, "no coin delivered after gold off");
    assert_eq!(sim.player_inventory(0).ware(rr_rules::Ware::Coins), 4, "all coins back in store");
}

/// RTTR nobBaseWarehouse::AreRecruitingConditionsComply: a warehouse recruits only while it keeps
/// at least max(1, 100 - 10 x recruit setting) helpers; a low setting saves carriers.
#[test]
fn recruiting_keeps_the_helper_reserve() {
    use rr_rules::Ware;
    let mut sim = sim1();
    ok(&mut sim, 0, Cmd::SetMilitary { values: [5, 5, 5, 5, 8, 8, 8, 8] });
    let helpers = sim.player_inventory(0).job(Job::Helper);
    sim.debug_take_job(0, Job::Helper, helpers);
    sim.debug_add_job(0, Job::Helper, 40);
    for w in [Ware::Sword, Ware::Shield, Ware::Beer] {
        sim.debug_add_ware(0, w, 10);
    }
    let privates = sim.player_inventory(0).job(Job::Private);
    for _ in 0..1_500 {
        sim.step(&[]);
    }
    assert_eq!(sim.player_inventory(0).job(Job::Private), privates, "40 helpers < 50 reserve: no recruits");
    sim.debug_add_job(0, Job::Helper, 30);
    run_until(&mut sim, 1_500, |s| s.player_inventory(0).job(Job::Private) > privates).expect("70 helpers: recruits");
}

/// Eval 28 fuzz find: gold off on a military building site kept its boards and stones coming.
#[test]
fn gold_off_on_a_site_keeps_its_building_wares() {
    let mut sim = sim1();
    let g = build(&mut sim, 0, B::Guardhouse, c(27, 20), c(20, 20));
    run_until(&mut sim, 2_000, |s| s.building(g).site_incoming.0 > 0).expect("boards ordered");
    let before = sim.building(g).site_incoming;
    let at = sim.building(g).pos;
    ok(&mut sim, 0, Cmd::ToggleProduction { at });
    assert_eq!(sim.building(g).site_incoming, before);
    ready_or_panic(&mut sim, g);
}

fn ready_or_panic(sim: &mut Sim, b: rr_sim::BuildingId) {
    run_until(sim, 20_000, |s| s.building(b).is_ready()).expect("the site is finished");
}

/// New-player round 6, #8: the interface can tell "no military building of yours is in reach"
/// (the headquarters only defends) from "they keep their last soldier".
#[test]
fn attack_posts_in_reach_are_counted() {
    let mut sim = sim_with(&[c(14, 20), c(40, 20)]);
    sim.step(&[]);
    assert_eq!(sim.attack_posts_near(0, c(40, 20)), 0, "a headquarters sends no attackers");
    let g = build(&mut sim, 0, B::Guardhouse, c(21, 20), c(14, 20));
    run_until(&mut sim, 20_000, |s| !s.building(g).soldiers.is_empty()).expect("manned");
    assert_eq!(sim.attack_posts_near(0, c(40, 20)), 1);
}

/// The strength an attack would have: the soldiers the posts that can walk there may send,
/// strongest first (each post keeps one).
#[test]
fn attack_strength_counts_who_can_go() {
    let mut sim = sim_with(&[c(14, 20), c(40, 20)]);
    ok(&mut sim, 0, Cmd::SetMilitary { values: [10, 5, 5, 5, 8, 8, 8, 8] });
    sim.debug_add_job(0, Job::General, 3);
    assert!(sim.attack_strength(0, c(40, 20)).is_empty());
    let g = build(&mut sim, 0, B::Guardhouse, c(21, 20), c(14, 20));
    run_until(&mut sim, 20_000, |s| s.building(g).soldiers.len() == 3).expect("manned");
    let hp = sim.attack_strength(0, c(40, 20));
    let general = sim.rules().military.hp[4];
    assert_eq!(hp, vec![general, general], "two of three generals go");
    assert_eq!(sim.attack_strength(0, c(30, 30)), Vec::<u8>::new(), "no building there");
}

/// QA round 4, #4: a coin starts the promotion clock (RTTR: 100 + rand(300) GF later), so the
/// garrison buttons can't make promotions instant.
#[test]
fn a_coin_starts_the_promotion_clock() {
    let mut sim = sim1();
    let g = build(&mut sim, 0, B::Guardhouse, c(27, 20), c(20, 20));
    run_until(&mut sim, 20_000, |s| !s.building(g).soldiers.is_empty()).expect("manned");
    sim.debug_add_ware(0, rr_rules::Ware::Coins, 2);
    run_until(&mut sim, 4_000, |s| s.building(g).stock[0] > 0 || s.building(g).promote_at.is_some())
        .expect("a coin arrives");
    let gf = sim.gf().0;
    let at = sim.building(g).promote_at.expect("the promotion is timed");
    assert!((gf + 99..=gf + 401).contains(&at), "promotion at {at}, coin at {gf}");
}

/// Item 307 (RTTR MilUpgradeOptim): a building sends its strongest soldier home and keeps its
/// smaller garrison, so the warehouses can send that soldier on to a front.
#[test]
fn send_home_lets_the_strongest_go() {
    let mut sim = sim_with(&[c(14, 20)]);
    ok(&mut sim, 0, Cmd::SetMilitary { values: [10, 5, 5, 5, 8, 8, 8, 8] });
    // one general among the privates
    sim.debug_add_job(0, Job::General, 1);
    let g = build(&mut sim, 0, B::Guardhouse, c(21, 20), c(14, 20));
    run_until(&mut sim, 20_000, |s| s.building(g).soldiers.len() == 3).expect("manned");
    let general = |s: &Sim| s.building(g).soldiers.iter().any(|f| s.figure(*f).job == Job::General);
    assert!(general(&sim), "the general went to the guardhouse (strongest first)");
    ok(&mut sim, 0, Cmd::SendHome { at: c(21, 20) });
    for _ in 0..3_000 {
        sim.step(&[]);
    }
    assert!(!general(&sim), "the general left");
    assert_eq!(sim.building(g).soldiers.len(), 2, "and nobody came in its place");
    assert!(sim.player_inventory(0).job(Job::General) >= 1, "the general is home");
}

/// Fidelity audit #1 (RTTR nobMilitary PrepareUpgrading): with coins in stock the next
/// promotion follows 100 + rand(300) GF after the last, not on the healing clock (gaps of
/// 377-803 GF were measured).
#[test]
fn promotions_follow_each_other_quickly() {
    let mut sim = sim1();
    ok(&mut sim, 0, Cmd::SetMilitary { values: [10, 5, 5, 5, 8, 8, 8, 8] });
    let f = build(&mut sim, 0, B::Fortress, c(27, 20), c(20, 20));
    run_until(&mut sim, 30_000, |s| s.building(f).soldiers.len() == 9).expect("manned");
    sim.debug_add_ware(0, rr_rules::Ware::Coins, 6);
    // promotions: a coin spent (each promotes one soldier of every rank present)
    let mut last_coins = None;
    let mut times = Vec::new();
    for _ in 0..8_000 {
        sim.step(&[]);
        let b = sim.building(f);
        let have = b.stock[0] + b.incoming[0];
        if let Some(l) = last_coins
            && have < l
        {
            // (with a coin left waiting in the building the next gap is the promotion's own)
            times.push((sim.gf().0, b.stock[0] > 0));
        }
        last_coins = Some(have);
        if times.len() == 6 {
            break;
        }
    }
    assert_eq!(times.len(), 6, "promotions at {times:?}");
    let gaps: Vec<u32> = times.windows(2).filter(|w| w[0].1).map(|w| w[1].0 - w[0].0).collect();
    assert!(gaps.len() >= 3, "{times:?}");
    assert!(gaps.iter().all(|g| *g <= 420), "gaps {gaps:?}");
}

/// Fidelity audit #2 (RTTR nofCatapultMan: CATAPULT_WAIT1_LENGTH 1 300 before every throw, plus
/// aiming): about 14 stones in 20 000 GF, not 45.
#[test]
fn the_catapult_throws_at_rttrs_pace() {
    let (h0, h1) = (c(14, 20), c(46, 20));
    let mut sim = sim_with(&[h0, h1]);
    sim.debug_add_ware(0, rr_rules::Ware::Stones, 60);
    let b1 = build(&mut sim, 1, B::Guardhouse, c(38, 20), h1);
    let g0 = build(&mut sim, 0, B::Guardhouse, c(21, 20), h0);
    run_until(&mut sim, 20_000, |s| !s.building(g0).soldiers.is_empty() && s.building(b1).soldiers.len() >= 2)
        .expect("garrisons");
    let g1 = build(&mut sim, 0, B::Guardhouse, c(28, 20), h0);
    run_until(&mut sim, 20_000, |s| !s.building(g1).soldiers.is_empty()).expect("manned");
    let cat = build(&mut sim, 0, B::Catapult, c(26, 22), h0);
    run_until(&mut sim, 30_000, |s| s.building(cat).is_ready() && s.building(cat).worker.is_some())
        .expect("catapult runs");
    run_until(&mut sim, 6_000, |s| s.building(cat).stock[0] > 0).expect("stones in");
    // stones thrown: each drop of the catapult's stock (deliveries only add)
    let mut thrown = 0;
    let mut last = sim.building(cat).stock[0];
    for _ in 0..20_000 {
        sim.step(&[]);
        let now = sim.building(cat).stock[0];
        if now < last {
            thrown += last - now;
        }
        last = now;
    }
    assert!((11..=16).contains(&thrown), "{thrown} stones in 20 000 GF");
}

/// Fidelity audit #7 (RTTR HitOfCatapultStone): when a stone kills the last soldier inside,
/// the building is destroyed, not left standing empty.
#[test]
fn a_catapult_hit_on_the_last_soldier_destroys_the_building() {
    let (h0, h1) = (c(14, 20), c(46, 20));
    let mut sim = sim_with(&[h0, h1]);
    sim.debug_add_ware(0, rr_rules::Ware::Stones, 60);
    // player 1: no soldiers to send after the first garrison
    let b1 = build(&mut sim, 1, B::Guardhouse, c(38, 20), h1);
    let g0 = build(&mut sim, 0, B::Guardhouse, c(21, 20), h0);
    run_until(&mut sim, 20_000, |s| !s.building(g0).soldiers.is_empty() && s.building(b1).soldiers.len() >= 2)
        .expect("garrisons");
    for j in Job::SOLDIERS {
        let n = sim.player_inventory(1).job(j);
        sim.debug_take_job(1, j, n);
    }
    sim.debug_take_ware(1, rr_rules::Ware::Beer, 99);
    let g1 = build(&mut sim, 0, B::Guardhouse, c(28, 20), h0);
    run_until(&mut sim, 20_000, |s| !s.building(g1).soldiers.is_empty()).expect("manned");
    let cat = build(&mut sim, 0, B::Catapult, c(26, 22), h0);
    run_until(&mut sim, 30_000, |s| s.building(cat).is_ready() && s.building(cat).worker.is_some())
        .expect("catapult runs");
    run_until(&mut sim, 60_000, |s| s.buildings().get(b1).is_none())
        .unwrap_or_else(|| panic!("still standing with {} soldiers", sim.building(b1).soldiers.len()));
}

/// Fidelity audit #10 (RTTR nobMilitary: soldiers leave one by one every 30 + rand(10) GF; duel
/// rounds of 15 GF): the attackers of one building step out over a while, not in a burst.
#[test]
fn soldiers_leave_one_by_one_at_rttrs_pace() {
    let (h0, h1) = (c(14, 20), c(46, 20));
    let mut sim = sim_with(&[h0, h1]);
    ok(&mut sim, 0, Cmd::SetMilitary { values: [10, 5, 5, 5, 8, 8, 8, 8] });
    let fort = build(&mut sim, 0, B::Fortress, c(21, 20), h0);
    let b1 = build(&mut sim, 1, B::Barracks, c(38, 20), h1);
    run_until(&mut sim, 40_000, |s| s.building(fort).soldiers.len() == 9 && !s.building(b1).soldiers.is_empty())
        .expect("garrisons");
    // a guardhouse nearer by, so the target is in sight
    let g1 = build(&mut sim, 0, B::Guardhouse, c(28, 20), h0);
    run_until(&mut sim, 20_000, |s| !s.building(g1).soldiers.is_empty()).expect("manned");
    let target = sim.building(b1).pos;
    run_until(&mut sim, 2_000, |s| s.is_attackable(0, b1)).expect("the barracks in sight");
    ok(&mut sim, 0, Cmd::Attack { target, soldiers: 5, strong: true });
    // frames at which each attacker steps out of the fortress
    let mut out = Vec::new();
    let mut inside = sim.building(fort).soldiers.len() + sim.building(fort).fig_queue.len();
    for _ in 0..600 {
        sim.step(&[]);
        let now = sim.figures().iter().filter(|(_, f)| f.inside == Some(fort)).count();
        if now < inside {
            out.push(sim.gf().0);
        }
        inside = now;
    }
    let gaps: Vec<u32> = out.windows(2).map(|w| w[1] - w[0]).collect();
    assert!(gaps.len() >= 3 && gaps.iter().all(|g| (30..=40).contains(g)), "left at {out:?}");
}

/// Sound designer 2 #3: a kill screams once (15-20 % of deaths screamed twice), and every
/// strike of a duel clangs (one clang per 5-6 s duel).
#[test]
fn a_duel_clangs_every_strike_and_screams_once() {
    let (h0, h1) = (c(14, 20), c(46, 20));
    let mut sim = sim_with(&[h0, h1]);
    for p in [0, 1] {
        ok(&mut sim, p, Cmd::SetMilitary { values: [10, 5, 5, 5, 8, 8, 8, 8] });
    }
    sim.debug_take_job(1, Job::Private, 42);
    sim.debug_take_ware(1, rr_rules::Ware::Beer, 99);
    let g0 = build(&mut sim, 0, B::Guardhouse, c(21, 20), h0);
    let b1 = build(&mut sim, 1, B::Barracks, c(39, 20), h1);
    run_until(&mut sim, 20_000, |s| s.building(g0).soldiers.len() == 3 && s.building(b1).soldiers.len() == 2)
        .expect("garrisons");
    let g1 = build(&mut sim, 0, B::Guardhouse, c(28, 20), h0);
    run_until(&mut sim, 20_000, |s| s.building(g1).soldiers.len() == 3).expect("second guardhouse");
    let target = sim.building(b1).pos;
    ok(&mut sim, 0, Cmd::Attack { target, soldiers: 2, strong: true });
    let (mut swords, mut deaths) = (0, Vec::new());
    for _ in 0..20_000 {
        let gf = sim.gf().0;
        let out = sim.step(&[]);
        for e in &out.events {
            match e {
                Event::Sound { sound: rr_sim::SoundCue::Sword, .. } => swords += 1,
                Event::Sound { at, sound: rr_sim::SoundCue::Death } => deaths.push((gf, *at)),
                _ => {}
            }
        }
        if out.events.iter().any(|e| matches!(e, Event::BuildingCaptured { .. })) {
            break;
        }
    }
    assert!(!deaths.is_empty(), "nobody died");
    let mut once = deaths.clone();
    once.dedup();
    assert_eq!(once.len(), deaths.len(), "a death screamed twice: {deaths:?}");
    // a strike every 15 gf; a kill takes several
    assert!(swords >= 4 * deaths.len(), "{swords} clangs for {} deaths", deaths.len());
}

/// Sound designer 2 #6: the catapult's throw is heard where it stands, the stone where it lands.
#[test]
fn a_catapult_is_heard_throwing_and_landing() {
    let (h0, h1) = (c(14, 20), c(46, 20));
    let mut sim = sim_with(&[h0, h1]);
    sim.debug_take_job(1, Job::Private, 40);
    sim.debug_take_ware(1, rr_rules::Ware::Beer, 99);
    let b1 = build(&mut sim, 1, B::Guardhouse, c(38, 20), h1);
    let g0 = build(&mut sim, 0, B::Guardhouse, c(21, 20), h0);
    run_until(&mut sim, 20_000, |s| !s.building(g0).soldiers.is_empty() && s.building(b1).soldiers.len() >= 2)
        .expect("garrisons");
    let g1 = build(&mut sim, 0, B::Guardhouse, c(28, 20), h0);
    run_until(&mut sim, 20_000, |s| !s.building(g1).soldiers.is_empty()).expect("manned");
    let cat = build(&mut sim, 0, B::Catapult, c(26, 22), h0);
    run_until(&mut sim, 30_000, |s| s.building(cat).is_ready() && s.building(cat).worker.is_some())
        .expect("catapult runs");
    let (cat_at, target) = (sim.building(cat).pos, sim.building(b1).pos);
    // what the map rings in red when the catapult's window is open: the one post in reach
    assert_eq!(sim.catapult_targets(cat), vec![b1]);
    assert_eq!(sim.work_radius(B::Catapult), Some(rr_sim::CATAPULT_RANGE));
    let (mut thrown, mut landed) = (false, false);
    for _ in 0..20_000 {
        let out = sim.step(&[]);
        for e in &out.events {
            match e {
                Event::Sound { at, sound: rr_sim::SoundCue::Catapult } if *at == cat_at => thrown = true,
                Event::Sound { at, sound: rr_sim::SoundCue::Thump } if *at == target => landed = true,
                // (the stone's flight, for the map to draw)
                Event::CatapultThrow { from, to, .. } => assert_eq!((*from, *to), (cat_at, target)),
                _ => {}
            }
        }
        if thrown && landed {
            break;
        }
    }
    assert!(thrown && landed, "thrown {thrown}, landed {landed}");
}

/// Sound designer 2 #6 (rest): a promotion is heard at its building.
#[test]
fn a_promotion_is_heard() {
    let mut sim = sim1();
    let g = build(&mut sim, 0, B::Guardhouse, c(27, 20), c(20, 20));
    run_until(&mut sim, 20_000, |s| !s.building(g).soldiers.is_empty()).expect("manned");
    sim.debug_add_ware(0, rr_rules::Ware::Coins, 2);
    let at = sim.building(g).pos;
    let heard = run_until_events(&mut sim, 20_000, |_, ev| {
        ev.iter().any(|e| matches!(e, Event::Sound { at: a, sound: rr_sim::SoundCue::Promote } if *a == at))
    });
    assert!(heard, "no promotion heard");
}

/// Perf-2 #3: the frontier classes of many buildings at once are each building's own (the AI
/// rebuilt the list of every post on the map per building).
#[test]
fn frontier_classes_in_one_go() {
    let (h0, h1) = (c(14, 20), c(46, 20));
    let mut sim = sim_with(&[h0, h1]);
    let a = build(&mut sim, 0, B::Guardhouse, c(21, 20), h0);
    let b = build(&mut sim, 1, B::Barracks, c(39, 20), h1);
    run_until(&mut sim, 20_000, |s| s.building(a).is_ready() && s.building(b).is_ready()).expect("built");
    let ids: Vec<rr_sim::BuildingId> = sim.buildings().iter().map(|(id, _)| id).collect();
    let one_by_one: Vec<usize> = ids.iter().map(|id| sim.frontier_distance(*id)).collect();
    assert_eq!(sim.frontier_distances(&ids), one_by_one);
}

/// RTTR DestroyPlayerRests: a border pushed up to a flag destroys it (a flag may not stand on
/// the border), and the building at the flag burns with it; a road across the new border line
/// is cut. Owner's playtest 2026-10-27: flags and roads stayed on the border line, and a
/// building lost with its flag didn't burn.
#[test]
fn a_border_pushed_up_to_a_flag_takes_it() {
    let (h0, h1) = (c(14, 20), c(46, 20));
    let mut sim = sim_with(&[h0, h1]);
    // the easternmost hut spot on our side
    let spot = (17..24)
        .rev()
        .flat_map(|x| [c(x, 19), c(x, 21)])
        .find(|p| matches!(sim.world().bq_for(*p, 0), rr_world::Bq::Hut | rr_world::Bq::House | rr_world::Bq::Castle))
        .expect("a hut spot");
    let hut = build(&mut sim, 0, B::Woodcutter, spot, h0);
    let (hut_at, flag_at) = (sim.building(hut).pos, sim.flag(sim.building(hut).flag).pos);
    let g = sim.world().grid();
    let b1 = build(&mut sim, 1, B::Guardhouse, c(38, 20), h1);
    run_until(&mut sim, 20_000, |s| !s.building(b1).soldiers.is_empty()).expect("manned");
    let b2 = build(&mut sim, 1, B::Barracks, c(31, 20), h1);
    run_until(&mut sim, 30_000, |s| !s.building(b2).soldiers.is_empty()).expect("manned");
    let w = sim.world();
    assert_eq!(w.owner(flag_at), Some(0), "the flag's node is still ours");
    let border = |p: MapCoord| !w.is_inner_territory(p, 0) && w.owner(p) == Some(0);
    assert!(
        border(flag_at) || g.neighbors(flag_at).iter().any(|q| w.owner(*q) != Some(0)),
        "the test needs the border at the flag"
    );
    assert!(sim.flag_at(flag_at).is_none(), "a flag on the border goes");
    assert!(sim.building_at(hut_at).is_none());
    assert_eq!(sim.world().obj(hut_at), rr_world::NodeObj::Fire, "the hut burns with its flag");
    // no road of ours runs through a border node
    for p in (0..g.len()).map(|i| g.coord(i)) {
        if let Some(r) = sim.road_at(p)
            && sim.road(r).player == 0
        {
            assert!(sim.world().is_inner_territory(p, 0), "a road on the border at {p:?}");
        }
    }
}
