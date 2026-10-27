//! New-player round 2, B: in a mission the rivals spare the newcomer for a while.
use rr_rules::Rules;
use rr_sim::{Cmd, GameSetup, PlayerSetup, Sim};
use std::sync::Arc;

/// Frame of the first attack of player 1 on player 0, if any, within `frames`.
fn first_attack(truce: Option<u32>, frames: u32) -> Option<u32> {
    let map =
        rr_mapgen::generate(&rr_mapgen::GenParams { width: 64, height: 64, players: 2, seed: 4, ..Default::default() });
    let setup = GameSetup { seed: 4, players: (0..2).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &map, &setup).unwrap();
    let mut ais = [rr_ai::Ai::with_level(0, rr_ai::Level::Easy), rr_ai::Ai::with_level(1, rr_ai::Level::Hard)];
    ais[1].truce = truce.map(|until| (0, until));
    for _ in 0..frames {
        let mut cmds = Vec::new();
        for ai in ais.iter_mut() {
            cmds.extend(ai.think(&sim));
        }
        for c in &cmds {
            if let (1, Cmd::Attack { target, .. }) = (c.player, &c.cmd)
                && sim.building_at(*target).is_some_and(|b| sim.building(b).player == 0)
            {
                return Some(sim.gf().0);
            }
        }
        sim.step(&cmds);
    }
    None
}

#[test]
fn a_truce_holds_off_attacks_on_the_spared_player() {
    let without = first_attack(None, 60_000).expect("a hard AI attacks an easy neighbour");
    let until = without + 15_000;
    if let Some(at) = first_attack(Some(until), until + 20_000) {
        assert!(at >= until, "attacked at {at}, truce until {until}");
    }
}

/// New-player round 3, #2: after the truce, the spared player's newly manned buildings get a
/// few minutes before an attack (the first soldier is alone in there).
#[test]
fn newly_manned_buildings_get_a_grace_time() {
    let map =
        rr_mapgen::generate(&rr_mapgen::GenParams { width: 64, height: 64, players: 2, seed: 4, ..Default::default() });
    let setup = GameSetup { seed: 4, players: (0..2).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &map, &setup).unwrap();
    let mut ais = [rr_ai::Ai::with_level(0, rr_ai::Level::Easy), rr_ai::Ai::with_level(1, rr_ai::Level::Hard)];
    // a truce that is over at once: only the grace time is left
    ais[1].truce = Some((0, 1));
    let mut attacks = 0;
    for _ in 0..60_000 {
        let mut cmds = Vec::new();
        for ai in ais.iter_mut() {
            cmds.extend(ai.think(&sim));
        }
        for c in &cmds {
            if let (1, Cmd::Attack { target, .. }) = (c.player, &c.cmd)
                && let Some(b) = sim.building_at(*target)
                && sim.building(b).player == 0
            {
                let since = sim.building(b).occupied_at;
                assert!(sim.gf().0 >= since + rr_ai::GRACE, "attacked at {} a building manned at {since}", sim.gf().0);
                attacks += 1;
            }
        }
        sim.step(&cmds);
    }
    assert!(attacks > 0, "no attacks at all");
}
