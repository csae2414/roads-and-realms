//! Roast finding: save, load and continue must replay the same game when AIs play.
use rr_rules::Rules;
use rr_sim::{GameSetup, PlayerSetup, Sim};
use std::sync::Arc;

fn step(sim: &mut Sim, ais: &mut [rr_ai::Ai], frames: u32) {
    for _ in 0..frames {
        let mut cmds = Vec::new();
        for ai in ais.iter_mut() {
            cmds.extend(ai.think(sim));
        }
        sim.step(&cmds);
    }
}

#[test]
fn saved_game_with_ais_continues_identically() {
    let map =
        rr_mapgen::generate(&rr_mapgen::GenParams { width: 64, height: 64, players: 3, seed: 5, ..Default::default() });
    let setup = GameSetup { seed: 5, players: (0..3).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    let rules = Arc::new(Rules::classic());
    let mut sim = Sim::new(rules.clone(), &map, &setup).unwrap();
    // player 0 is the human; the AIs play 1 and 2 at different levels
    let mut ais = vec![rr_ai::Ai::with_level(1, rr_ai::Level::Hard), rr_ai::Ai::with_level(2, rr_ai::Level::Easy)];
    step(&mut sim, &mut ais, 6000);
    let bytes = rr_ai::save_game(&sim, &ais);
    step(&mut sim, &mut ais, 6000);
    let (mut back, mut back_ais) = rr_ai::load_game(rules, &bytes).expect("load");
    assert_eq!(
        back_ais.iter().map(|a| (a.player, a.level)).collect::<Vec<_>>(),
        vec![(1, rr_ai::Level::Hard), (2, rr_ai::Level::Easy)]
    );
    step(&mut back, &mut back_ais, 6000);
    assert_eq!(back.sync_hash(), sim.sync_hash(), "the loaded game took another course");
}

/// The save list shows the time played without loading the whole game.
#[test]
fn a_save_tells_its_game_time_cheaply() {
    let map =
        rr_mapgen::generate(&rr_mapgen::GenParams { width: 48, height: 48, players: 2, seed: 2, ..Default::default() });
    let setup = GameSetup { seed: 2, players: (0..2).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    let rules = Arc::new(Rules::classic());
    let mut sim = Sim::new(rules.clone(), &map, &setup).unwrap();
    let mut ais = vec![rr_ai::Ai::with_level(1, rr_ai::Level::Normal)];
    step(&mut sim, &mut ais, 1234);
    let bytes = rr_ai::save_game(&sim, &ais);
    assert_eq!(rr_ai::save_game_time(&bytes[..16]), Some(1234), "the header alone is enough");
    assert_eq!(rr_ai::save_game_time(b"junk"), None);
    // still loads
    assert_eq!(rr_ai::load_game(rules, &bytes).unwrap().0.gf().0, 1234);
}

/// New-player roast #9: the interface's own state (messages, tips shown) travels in the save.
#[test]
fn a_save_carries_extra_interface_state() {
    let map =
        rr_mapgen::generate(&rr_mapgen::GenParams { width: 48, height: 48, players: 2, seed: 2, ..Default::default() });
    let setup = GameSetup { seed: 2, players: (0..2).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    let rules = Arc::new(Rules::classic());
    let mut sim = Sim::new(rules.clone(), &map, &setup).unwrap();
    let mut ais = vec![rr_ai::Ai::with_level(1, rr_ai::Level::Normal)];
    step(&mut sim, &mut ais, 500);
    let bytes = rr_ai::save_game_with(&sim, &ais, b"messages");
    assert_eq!(rr_ai::save_game_time(&bytes), Some(500));
    let (back, back_ais, extra) = rr_ai::load_game_with(rules.clone(), &bytes).unwrap();
    assert_eq!((back.gf().0, back_ais.len(), extra.as_slice()), (500, 1, b"messages".as_slice()));
    // a save without any loads with nothing extra
    let plain = rr_ai::save_game(&sim, &ais);
    assert!(rr_ai::load_game_with(rules, &plain).unwrap().2.is_empty());
}

/// Performance roast #10: saves are packed, and the unpacked saves of before still load.
#[test]
fn saves_are_packed_and_old_ones_still_load() {
    let map =
        rr_mapgen::generate(&rr_mapgen::GenParams { width: 64, height: 64, players: 2, seed: 3, ..Default::default() });
    let setup = GameSetup { seed: 3, players: (0..2).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    let rules = Arc::new(Rules::classic());
    let mut sim = Sim::new(rules.clone(), &map, &setup).unwrap();
    let mut ais = vec![rr_ai::Ai::new(1)];
    step(&mut sim, &mut ais, 3000);
    let packed = rr_ai::save_game(&sim, &ais);
    assert_eq!(&packed[..4], b"RRG6");
    // the same game the way version 4 wrote it: magic, frame, plain postcard
    let mut plain = b"RRG4".to_vec();
    plain.extend(sim.gf().0.to_le_bytes());
    plain.extend(postcard::to_stdvec(&(sim.save(), &ais, Vec::<u8>::new())).unwrap());
    assert!(packed.len() * 3 < plain.len(), "{} vs {}", packed.len(), plain.len());
    for bytes in [&packed, &plain] {
        let (back, _) = rr_ai::load_game(rules.clone(), bytes).expect("load");
        assert_eq!(back.sync_hash(), sim.sync_hash());
        assert_eq!(rr_ai::save_game_time(&bytes[..8]), Some(sim.gf().0));
    }
    // a damaged packed save is refused, not a crash
    let mut broken = packed.clone();
    broken.truncate(packed.len() / 2);
    assert!(rr_ai::load_game(rules, &broken).is_err());
}

/// Robustness roast #10, #11: a save with an AI for a missing player, or with a flipped byte,
/// is refused; the reviewer's crash file too.
#[test]
fn damaged_saves_are_refused() {
    let map =
        rr_mapgen::generate(&rr_mapgen::GenParams { width: 64, height: 64, players: 2, seed: 3, ..Default::default() });
    let setup = GameSetup { seed: 3, players: (0..2).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    let rules = Arc::new(Rules::classic());
    let mut sim = Sim::new(rules.clone(), &map, &setup).unwrap();
    let mut ais = vec![rr_ai::Ai::new(1)];
    step(&mut sim, &mut ais, 2000);
    let ghost = rr_ai::save_game(&sim, &[rr_ai::Ai::new(1), rr_ai::Ai::new(9)]);
    assert!(rr_ai::load_game(rules.clone(), &ghost).is_err());
    let twice = rr_ai::save_game(&sim, &[rr_ai::Ai::new(1), rr_ai::Ai::new(1)]);
    assert!(rr_ai::load_game(rules.clone(), &twice).is_err());
    // flip bytes all over a good save: refused (checksum), never a crash
    let good = rr_ai::save_game(&sim, &ais);
    for i in (12..good.len()).step_by(97) {
        let mut bad = good.clone();
        bad[i] ^= 0x5a;
        assert!(rr_ai::load_game(rules.clone(), &bad).is_err(), "byte {i}");
    }
    // QA round 4, #5: a cut-off save gives a short reason, not the decompressor's output
    for cut in [good.len() / 3, good.len() / 2, good.len() - 10] {
        let e = rr_ai::load_game(rules.clone(), &good[..cut]).err().expect("refused");
        assert!(e.len() < 200, "{} characters: {}", e.len(), &e[..200.min(e.len())]);
    }
    let crash = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/crash.rrsave")).unwrap();
    assert!(rr_sim::Sim::load(rules, &crash).is_err());
}

/// Replays after the harbor and sea-war work (owner, 2026-10-27: "do replays work?"): a whole
/// recorded AI game on an island map, with expeditions, explorations, attacks from the sea and a
/// player's priority, plays back from its bytes to exactly the same state.
#[test]
#[ignore = "a whole game: tools/eval.sh runs it (cargo test -p rr-ai -- --ignored)"]
fn an_island_war_replays_exactly() {
    let map = rr_mapgen::generate(&rr_mapgen::GenParams {
        width: 128,
        height: 128,
        players: 4,
        seed: 3,
        islands: true,
        ..Default::default()
    });
    let rules = Arc::new(Rules::classic());
    let setup = GameSetup { seed: 3, players: (0..4).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    let mut sim = Sim::new(rules.clone(), &map, &setup).unwrap();
    let mut ais: Vec<rr_ai::Ai> = (0..4u8).map(rr_ai::Ai::new).collect();
    let mut rec = rr_sim::Replay::new(&rules, &map, &setup);
    let mut kinds = std::collections::BTreeSet::new();
    for gf in 0..200_000u32 {
        let mut cmds = Vec::new();
        for ai in ais.iter_mut() {
            cmds.extend(ai.think(&sim));
        }
        // a player's own orders too: the first site of player 0 gets priority
        if gf == 30_000
            && let Some((_, b)) = sim.buildings().iter().find(|(_, b)| b.player == 0 && !b.is_ready())
        {
            cmds.push(rr_sim::PlayerCommand { player: 0, cmd: rr_sim::Cmd::SetPriority { at: b.pos, on: true } });
        }
        for c in &cmds {
            kinds.insert(format!("{:?}", c.cmd).split([' ', '{']).next().unwrap_or("").to_string());
        }
        rec.record(sim.gf().0, &cmds);
        sim.step(&cmds);
    }
    rec.finish(&sim);
    // the game had what a replay must reproduce
    for k in ["SendExpedition", "StartExploration", "Attack", "SetPriority"] {
        assert!(kinds.contains(k), "no {k} in the game: {kinds:?}");
    }
    let sea_war = sim.players().iter().any(|p| p.stats.vanquished > 0);
    assert!(sea_war, "nobody took anything");
    let back = rr_sim::Replay::from_bytes(&rec.to_bytes()).unwrap();
    let replayed = back.play(rules).unwrap();
    assert!(back.verify(&replayed));
    assert_eq!(replayed.sync_hash(), sim.sync_hash());
    assert_eq!(replayed.dump_string(), sim.dump_string());
}
