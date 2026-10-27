use rr_ai::Ai;
use rr_rules::Rules;
use rr_sim::{GameSetup, PlayerSetup, Sim};
use std::sync::Arc;

fn game(seed: u64, players: usize) -> Sim {
    let map = rr_mapgen::generate(&rr_mapgen::GenParams { width: 96, height: 96, players, seed, ..Default::default() });
    let setup =
        GameSetup { seed, players: (0..players).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    Sim::new(Arc::new(Rules::classic()), &map, &setup).unwrap()
}

#[test]
fn ai_builds_an_economy() {
    let mut sim = game(3, 1);
    let mut ai = Ai::new(0);
    for i in 0..20_000 {
        let cmds = ai.think(&sim);
        if i < 2000 && !cmds.is_empty() {
            eprintln!("gf {i}: {:?}", cmds[0].cmd);
        }
        let out = sim.step(&cmds);
        for e in out.events {
            if matches!(e, rr_sim::Event::CommandRejected { .. }) && i < 2000 {
                eprintln!("  -> {e:?}");
            }
        }
    }
    let ready = sim.buildings().iter().filter(|(_, b)| b.is_ready()).count();
    assert!(ready >= 6, "only {ready} buildings finished:\n{}", sim.dump_string());
}
