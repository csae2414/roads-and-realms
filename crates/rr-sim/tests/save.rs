mod common;
use common::*;
use rr_core::Direction::*;
use rr_rules::{BuildingType as B, Rules};
use rr_sim::{Cmd, PlayerCommand, Sim};
use rr_world::NodeObj;
use std::sync::Arc;

fn scenario() -> Sim {
    let mut sim = sim_with(&[c(20, 20), c(44, 40)]);
    for p in [c(26, 17), c(27, 18), c(25, 16), c(28, 19)] {
        sim.debug_set_obj(p, NodeObj::Tree { species: 2, stage: 3 });
    }
    let a = walk(&sim, c(20, 20), SE, 1);
    let cmds = [
        Cmd::PlaceBuilding { at: c(24, 20), kind: B::Woodcutter },
        Cmd::BuildRoad { from: a, dirs: vec![E, E, E, E] },
        Cmd::PlaceBuilding { at: c(20, 24), kind: B::Forester },
    ];
    for cmd in cmds {
        sim.step(&[PlayerCommand { player: 0, cmd }]);
    }
    let hf = walk(&sim, c(20, 20), SE, 1);
    sim.step(&[PlayerCommand { player: 0, cmd: Cmd::BuildRoad { from: hf, dirs: vec![SE, SW, SE, SW] } }]);
    sim
}

#[test]
fn save_load_continue_is_identical() {
    let mut straight = scenario();
    let mut saved = scenario();
    for _ in 0..3000 {
        straight.step(&[]);
        saved.step(&[]);
    }
    let bytes = saved.save();
    assert!(bytes.len() > 1000);
    let mut loaded = Sim::load(Arc::new(Rules::classic()), &bytes).expect("load");
    assert_eq!(loaded.sync_hash(), straight.sync_hash());
    for i in 0..4000 {
        straight.step(&[]);
        loaded.step(&[]);
        if i % 500 == 0 {
            assert_eq!(loaded.sync_hash(), straight.sync_hash(), "diverged at +{i}");
        }
    }
    assert_eq!(loaded.dump_string(), straight.dump_string());
}

#[test]
fn load_rejects_garbage_and_other_rules() {
    let rules = Arc::new(Rules::classic());
    assert!(Sim::load(rules.clone(), b"hello").is_err());
    let bytes = scenario().save();
    assert!(Sim::load(rules, &bytes[..bytes.len() / 2]).is_err());
    let mut other = Rules::classic();
    other.military.hp[0] = 4;
    assert!(matches!(Sim::load(Arc::new(other), &bytes), Err(rr_sim::LoadError::RulesMismatch)));
}

#[test]
fn replay_reproduces_the_game() {
    let map = {
        let mut m = rr_core::MapData::flat(64, 64, MEADOW);
        m.hq = vec![Some(c(20, 20))];
        m
    };
    let rules = Arc::new(Rules::classic());
    let setup = rr_sim::GameSetup { seed: 9, players: vec![Default::default()], ..Default::default() };
    let mut sim = Sim::new(rules.clone(), &map, &setup).unwrap();
    let mut rec = rr_sim::Replay::new(&rules, &map, &setup);
    let a = walk(&sim, c(20, 20), SE, 1);
    for i in 0..3000u32 {
        let cmds = match i {
            5 => vec![PlayerCommand { player: 0, cmd: Cmd::PlaceBuilding { at: c(24, 20), kind: B::Sawmill } }],
            6 => vec![PlayerCommand { player: 0, cmd: Cmd::BuildRoad { from: a, dirs: vec![E, E, E, E] } }],
            _ => vec![],
        };
        rec.record(sim.gf().0, &cmds);
        sim.step(&cmds);
    }
    rec.finish(&sim);
    let bytes = rec.to_bytes();
    let back = rr_sim::Replay::from_bytes(&bytes).unwrap();
    let replayed = back.play(rules).unwrap();
    assert_eq!(replayed.sync_hash(), back.final_hash);
    assert_eq!(replayed.dump_string(), sim.dump_string());
    // other rules: refused instead of silently playing another game
    let mut other = rr_sim::Replay::from_bytes(&bytes).unwrap();
    other.rules_hash ^= 1;
    assert!(matches!(other.play(Arc::new(Rules::classic())), Err(rr_sim::SetupError::ReplayMismatch)));
    // QA round 4, #6: recorded with an older simulation (same save format): refused too
    let mut older = rr_sim::Replay::from_bytes(&bytes).unwrap();
    older.version -= 1;
    assert!(matches!(older.play(Arc::new(Rules::classic())), Err(rr_sim::SetupError::ReplayMismatch)));
    // and a replay that ends somewhere else than recorded says so
    assert!(back.verify(&replayed));
    let mut wrong = rr_sim::Replay::from_bytes(&bytes).unwrap();
    wrong.final_hash ^= 1;
    assert!(!wrong.verify(&replayed));
}

/// QA round 3, #6: a save from another version says so, even when the rest can't be read.
#[test]
fn an_old_save_says_its_version() {
    let sim = common::sim1();
    let mut bytes = sim.save();
    // the version (a small number, one byte) follows the 8-byte magic
    assert_eq!(bytes[8] as u32, rr_sim::SAVE_VERSION);
    bytes[8] = 12;
    bytes.truncate(40);
    match Sim::load(std::sync::Arc::new(rr_rules::Rules::classic()), &bytes) {
        Err(rr_sim::LoadError::Version(12)) => {}
        Err(e) => panic!("{e}"),
        Ok(_) => panic!("loaded"),
    }
}
