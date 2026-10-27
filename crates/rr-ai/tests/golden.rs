//! Architect round 2, #2: a forgotten version bump fails here. A short fixed game's sync hash is
//! kept with the version numbers in `golden.txt` (tracked). If the game plays differently and
//! the versions are the same, someone changed the rules of play without raising SIM_VERSION or
//! AI_VERSION. After a deliberate change: raise the version, then `RR_BLESS=1 cargo test -p
//! rr-ai --test golden` writes the new file.
use rr_sim::{GameSetup, PlayerSetup, Sim};
use std::sync::Arc;

fn now() -> String {
    let map = rr_mapgen::generate(&rr_mapgen::GenParams {
        width: 128,
        height: 128,
        players: 4,
        seed: 11,
        ..Default::default()
    });
    let setup = GameSetup { seed: 11, players: vec![PlayerSetup::default(); 4], ..Default::default() };
    let mut sim = Sim::new(Arc::new(rr_rules::Rules::classic()), &map, &setup).unwrap();
    let mut ais: Vec<rr_ai::Ai> = (0..4).map(rr_ai::Ai::new).collect();
    for _ in 0..30_000 {
        let mut cmds = Vec::new();
        for ai in ais.iter_mut() {
            cmds.extend(ai.think(&sim));
        }
        sim.step(&cmds);
    }
    format!(
        "save {}\nsim {}\nai {}\nhash {:016x}\nhash campaign {:016x}\n",
        rr_sim::SAVE_VERSION,
        rr_sim::SIM_VERSION,
        rr_ai::AI_VERSION,
        sim.sync_hash(),
        campaign()
    )
}

/// Architect round 3, #3: the free-for-all above never reaches a truce, peace time, an invader,
/// a harbor or a capture. Mare Nostrum (seed 1) does: the invader armed, the campaign truce on
/// player 0, five minutes of peace for all, and war after the truce.
fn campaign() -> u64 {
    let region = rr_mapgen::world::region("mediterranean").unwrap();
    let map = rr_mapgen::region::generate(region, 1);
    let n = map.players();
    let setup = GameSetup { seed: 1, players: vec![PlayerSetup::default(); n], ..Default::default() };
    let mut sim = Sim::new(Arc::new(rr_rules::Rules::classic()), &map, &setup).unwrap();
    let mut ais: Vec<rr_ai::Ai> = (0..n as u8).map(rr_ai::Ai::new).collect();
    const PEACE: u32 = 5 * 60 * 20;
    for ai in &mut ais {
        ai.peace_until = PEACE;
    }
    for ai in &mut ais[1..] {
        ai.truce = Some((0, rr_ai::MISSION_TRUCE));
    }
    ais[n - 1].invade = Some(0);
    let mut seen = [0u32; 4];
    // (to minute 50: 20 attacks, 13 captures and 12 ships launched by then)
    while sim.gf().0 < 60_000 {
        let mut cmds = Vec::new();
        for ai in ais.iter_mut() {
            cmds.extend(ai.think(&sim));
        }
        // (peace time and the truce, as the game keeps them)
        let gf = sim.gf().0;
        cmds.retain(|c| {
            !matches!(c.cmd, rr_sim::Cmd::Attack { .. }) || gf >= PEACE && (c.player != 0 || gf >= rr_ai::MISSION_TRUCE)
        });
        seen[0] += cmds.iter().filter(|c| matches!(c.cmd, rr_sim::Cmd::Attack { .. })).count() as u32;
        for e in sim.step(&cmds).events {
            match e {
                rr_sim::Event::BuildingCaptured { .. } => seen[1] += 1,
                rr_sim::Event::Sound { sound: rr_sim::SoundCue::Launch, .. } => seen[2] += 1,
                _ => {}
            }
        }
    }
    eprintln!("campaign golden: attacks {}, captures {}, ships {}", seen[0], seen[1], seen[2]);
    sim.sync_hash()
}

#[test]
fn a_changed_game_has_a_new_version() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/golden.txt");
    let got = now();
    if std::env::var_os("RR_BLESS").is_some() {
        std::fs::write(path, &got).unwrap();
        return;
    }
    let want = std::fs::read_to_string(path).unwrap_or_default();
    if got == want {
        return;
    }
    let versions = |s: &str| s.lines().filter(|l| !l.starts_with("hash")).collect::<Vec<_>>().join("\n");
    assert_ne!(
        versions(&got),
        versions(&want),
        "the game plays differently with the same versions: raise SIM_VERSION or AI_VERSION\nwas:\n{want}now:\n{got}"
    );
    panic!(
        "new versions: bless the golden file (RR_BLESS=1 cargo test -p rr-ai --test golden)\nwas:\n{want}now:\n{got}"
    );
}
