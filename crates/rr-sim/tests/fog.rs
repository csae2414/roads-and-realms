mod common;
use common::*;
use rr_core::Direction::*;
use rr_rules::BuildingType as B;
use rr_sim::{Cmd, PlayerCommand, Visibility};

#[test]
fn hq_reveals_its_surroundings_rest_is_unexplored() {
    let mut sim = sim1();
    sim.step(&[]);
    assert_eq!(sim.visibility(0, c(20, 20)), Visibility::Visible);
    assert_eq!(sim.visibility(0, walk(&sim, c(20, 20), E, 12)), Visibility::Visible, "HQ sight 12");
    assert_eq!(sim.visibility(0, walk(&sim, c(20, 20), E, 14)), Visibility::Unexplored);
    assert_eq!(sim.visibility(0, c(50, 50)), Visibility::Unexplored);
}

#[test]
fn explored_land_stays_fogged_when_vision_is_lost() {
    let mut sim = sim1();
    let at = c(26, 20);
    sim.step(&[PlayerCommand { player: 0, cmd: Cmd::PlaceBuilding { at, kind: B::Guardhouse } }]);
    let a = walk(&sim, c(20, 20), SE, 1);
    sim.step(&[PlayerCommand { player: 0, cmd: Cmd::BuildRoad { from: a, dirs: vec![E, E, E, E, E, E] } }]);
    let b = sim.building_at(at).unwrap();
    run_until(&mut sim, 20_000, |s| s.building(b).is_ready() && !s.building(b).soldiers.is_empty()).unwrap();
    for _ in 0..60 {
        sim.step(&[]);
    }
    let far = walk(&sim, at, E, 11);
    assert_eq!(sim.visibility(0, far), Visibility::Visible, "guardhouse sight 12");
    sim.step(&[PlayerCommand { player: 0, cmd: Cmd::RemoveBuilding { at } }]);
    for _ in 0..60 {
        sim.step(&[]);
    }
    assert_eq!(sim.visibility(0, far), Visibility::Fog);
}

/// RTTR nofScout_Free: the scout makes 16 trips to unseen nodes up to 15 from its flag, so it
/// reveals land well beyond a fixed ring around the flag.
#[test]
fn the_scout_explores_unseen_land_far_out() {
    let mut sim = sim1();
    let a = walk(&sim, c(20, 20), SE, 1);
    sim.step(&[PlayerCommand { player: 0, cmd: Cmd::BuildRoad { from: a, dirs: vec![E, E, E, E] } }]);
    let flag = walk(&sim, a, E, 4);
    sim.step(&[PlayerCommand { player: 0, cmd: Cmd::SendScout { flag } }]);
    for _ in 0..12_000 {
        sim.step(&[]);
    }
    let g = sim.world().grid();
    let far = g
        .coords()
        .filter(|p| g.distance(*p, flag) >= 15 && sim.visibility(0, *p) != Visibility::Unexplored)
        .filter(|p| g.distance(*p, c(20, 20)) > 13)
        .count();
    assert!(far >= 40, "the scout revealed only {far} nodes 15+ from its flag");
}

/// RTTR nofScout_Free: once at the flag the scout walks 80-99 nodes (`rest_way`), then goes
/// back; it doesn't keep on for a set number of trips.
#[test]
fn the_scout_walks_80_to_99_nodes_then_goes_home() {
    let mut sim = sim1();
    let a = walk(&sim, c(20, 20), SE, 1);
    sim.step(&[PlayerCommand { player: 0, cmd: Cmd::BuildRoad { from: a, dirs: vec![E, E, E, E] } }]);
    let flag = walk(&sim, a, E, 4);
    sim.step(&[PlayerCommand { player: 0, cmd: Cmd::SendScout { flag } }]);
    let scout = |s: &rr_sim::Sim| {
        s.figures().iter().find(|(_, f)| f.job == rr_rules::Job::Scout && f.inside.is_none()).map(|(_, f)| f.pos)
    };
    let mut last = None;
    let mut steps = 0;
    let mut seen = false;
    for _ in 0..20_000 {
        sim.step(&[]);
        let now = scout(&sim);
        if now.is_some() {
            seen = true;
            if last.is_some() && now != last {
                steps += 1;
            }
        } else if seen {
            break;
        }
        last = now;
    }
    assert!(seen, "no scout went out");
    // the way out (about 6) and back (at most about 20) on top of 80-99
    assert!((86..=126).contains(&steps), "the scout walked {steps} nodes");
}

/// Perf-2 #2: a walk bounded to `max` steps finds what the open search finds within it, and
/// gives up past it (attack reach asked a 6000-node search per post).
#[test]
fn a_bounded_walk_is_the_walk_within_bounds() {
    let mut m = rr_core::MapData::flat(64, 64, MEADOW);
    let g = m.grid;
    // a lake in the middle to walk around
    for p in g.within(c(32, 32), 7) {
        let i = g.index(p);
        m.terrain_rsu[i] = 0;
        m.terrain_usd[i] = 0;
    }
    m.hq = vec![Some(c(10, 10))];
    let sim = rr_sim::Sim::new(
        std::sync::Arc::new(rr_rules::Rules::classic()),
        &m,
        &rr_sim::GameSetup { players: vec![Default::default()], ..Default::default() },
    )
    .unwrap();
    let mut rng = rr_core::Pcg32::new(5, 1);
    let mut checked = 0;
    for _ in 0..400 {
        let a = c(rng.below(64) as u16, rng.below(64) as u16);
        let b = c(rng.below(64) as u16, rng.below(64) as u16);
        let open = sim.find_path(a, b, 6000).map(|p| p.len());
        let within = sim.path_len_within(a, b, 40);
        assert_eq!(within, open.filter(|l| *l <= 40), "{a:?} -> {b:?}");
        checked += usize::from(open.is_some());
    }
    assert!(checked > 100, "{checked}");
}

/// The Roman campaign's Jupiter (docs/CAMPAIGN-ROME.md, chapter 1): the player whose land lies
/// fully around his holy place sees far from it; a place outside the land gives nothing, and
/// nothing removes it.
#[test]
fn jupiters_holder_sees_far_from_the_hill() {
    let mut sim = sim1();
    let hill = walk(&sim, c(20, 20), E, 5);
    let far = walk(&sim, hill, E, 13);
    let outside = walk(&sim, c(20, 20), W, 15);
    sim.debug_set_obj(hill, rr_world::NodeObj::Decoration(rr_world::HOLY_BASE));
    sim.debug_set_obj(outside, rr_world::NodeObj::Decoration(rr_world::HOLY_BASE));
    for _ in 0..40 {
        sim.step(&[]);
    }
    assert_eq!(sim.world().holy_holder(hill), rr_world::Holder::Player(0));
    assert_eq!(sim.world().holy_holder(outside), rr_world::Holder::Nobody);
    assert_eq!(sim.visibility(0, far), Visibility::Visible, "Jupiter's sight from the hill");
    assert_eq!(sim.visibility(0, walk(&sim, outside, W, 13)), Visibility::Unexplored, "not from a place not held");
    assert_eq!(sim.world().holy_places().len(), 2);
}

/// A holy place where two realms' borders meet is contested: nobody may use it.
#[test]
fn a_holy_place_between_two_realms_is_contested() {
    let mut sim = sim_with(&[c(20, 20), c(34, 20)]);
    let mid = c(27, 20);
    sim.debug_set_obj(mid, rr_world::NodeObj::Decoration(rr_world::HOLY_BASE));
    sim.step(&[]);
    assert_eq!(sim.world().holy_holder(mid), rr_world::Holder::Contested);
}

/// A story's hand-over (`Cmd::HandOver`, the Roman campaign's Tarpeia): the owner's military
/// building goes over to the other player with its land, its soldiers walk home; nobody else
/// can give it away, and a warehouse can't be given.
#[test]
fn a_building_is_handed_over() {
    let mut sim = sim_with(&[c(20, 20), c(40, 20)]);
    let at = c(26, 20);
    sim.step(&[PlayerCommand { player: 0, cmd: Cmd::PlaceBuilding { at, kind: B::Guardhouse } }]);
    let a = walk(&sim, c(20, 20), SE, 1);
    sim.step(&[PlayerCommand { player: 0, cmd: Cmd::BuildRoad { from: a, dirs: vec![E, E, E, E, E, E] } }]);
    let b = sim.building_at(at).unwrap();
    run_until(&mut sim, 20_000, |s| s.building(b).is_ready() && !s.building(b).soldiers.is_empty()).unwrap();
    let give = |to| PlayerCommand { player: 0, cmd: Cmd::HandOver { at, to } };
    // only the owner may give it, and not a castle
    sim.step(&[PlayerCommand { player: 1, cmd: Cmd::HandOver { at, to: 1 } }]);
    assert_eq!(sim.building(b).player, 0);
    sim.step(&[PlayerCommand { player: 0, cmd: Cmd::HandOver { at: c(20, 20), to: 1 } }]);
    assert_eq!(sim.building(sim.building_at(c(20, 20)).unwrap()).player, 0);
    let soldiers_before = sim.player_inventory(0).soldiers();
    sim.step(&[give(1)]);
    assert_eq!(sim.building(b).player, 1, "handed over");
    assert!(sim.building(b).soldiers.is_empty(), "its soldiers left");
    run_until(&mut sim, 4000, |s| s.player_inventory(0).soldiers() > soldiers_before).expect("the soldier came home");
    for _ in 0..60 {
        sim.step(&[]);
    }
    assert_eq!(sim.world().owner(at), Some(1), "the land goes with it");
}

/// Offerings at a holy place (the Roman campaign's gods, `Cmd::Offer`) leave the warehouses,
/// only while the player holds the place and has the wares; a god's answer (`Cmd::Bless`)
/// promotes privates in the warehouses.
#[test]
fn offerings_and_a_gods_answer() {
    use rr_rules::{Job, Ware};
    let mut sim = sim1();
    let altar = walk(&sim, c(20, 20), E, 5);
    let far = walk(&sim, c(20, 20), W, 15);
    for p in [altar, far] {
        sim.debug_set_obj(p, rr_world::NodeObj::Decoration(rr_world::HOLY_BASE + 2));
    }
    sim.step(&[]);
    let boards = sim.player_inventory(0).ware(Ware::Boards);
    let offer = |at, n| PlayerCommand { player: 0, cmd: Cmd::Offer { at, ware: Ware::Boards, n } };
    sim.step(&[offer(altar, 3)]);
    assert_eq!(sim.player_inventory(0).ware(Ware::Boards), boards - 3);
    sim.step(&[offer(far, 3), offer(altar, boards + 1)]);
    assert_eq!(sim.player_inventory(0).ware(Ware::Boards), boards - 3, "not held, or too many: nothing");
    let (pvt, pfc) = (sim.player_inventory(0).job(Job::Private), sim.player_inventory(0).job(Job::PrivateFirstClass));
    sim.step(&[PlayerCommand { player: 0, cmd: Cmd::Bless { at: altar, n: 3, recruit: false } }]);
    assert_eq!(sim.player_inventory(0).job(Job::Private), pvt - 3);
    assert_eq!(sim.player_inventory(0).job(Job::PrivateFirstClass), pfc + 3);
    sim.step(&[PlayerCommand { player: 0, cmd: Cmd::Bless { at: far, n: 3, recruit: false } }]);
    assert_eq!(sim.player_inventory(0).job(Job::PrivateFirstClass), pfc + 3, "not from a place not held");
    // the recruiting answer: new privates in the castle
    sim.step(&[PlayerCommand { player: 0, cmd: Cmd::Bless { at: altar, n: 5, recruit: true } }]);
    assert_eq!(sim.player_inventory(0).job(Job::Private), pvt - 3 + 5);
}
