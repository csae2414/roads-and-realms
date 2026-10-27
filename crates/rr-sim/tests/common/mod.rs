#![allow(dead_code)]
use rr_core::{Direction, MapCoord, MapData};
use rr_rules::Rules;
use rr_sim::{Cmd, GameSetup, PlayerSetup, Sim};
use std::sync::Arc;

pub const MEADOW: u8 = 8;

pub fn c(x: u16, y: u16) -> MapCoord {
    MapCoord::new(x, y)
}

/// A flat 64x64 meadow with one HQ at (20, 20), or two HQs.
pub fn sim_with(players: &[MapCoord]) -> Sim {
    let mut m = MapData::flat(64, 64, MEADOW);
    m.hq = players.iter().map(|p| Some(*p)).collect();
    let setup =
        GameSetup { seed: 42, players: players.iter().map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    Sim::new(Arc::new(Rules::classic()), &m, &setup).expect("setup")
}

pub fn sim1() -> Sim {
    sim_with(&[c(20, 20)])
}

pub fn walk(sim: &Sim, p: MapCoord, d: Direction, n: u16) -> MapCoord {
    sim.world().grid().walk(p, d, n)
}

/// Step until `f` is true or `max` frames pass; returns frames used.
pub fn run_until(sim: &mut Sim, max: u32, mut f: impl FnMut(&Sim) -> bool) -> Option<u32> {
    for i in 0..max {
        if f(sim) {
            return Some(i);
        }
        sim.step(&[]);
        // the bookkeeping holds all along (architect round 2, #3)
        if i % 500 == 499
            && let Err(e) = sim.audit()
        {
            panic!("audit at gf {}: {e}", sim.gf().0);
        }
    }
    if f(sim) { Some(max) } else { None }
}

pub fn exec(sim: &mut Sim, player: u8, cmd: Cmd) -> rr_sim::StepOutput {
    sim.step(&[rr_sim::PlayerCommand { player, cmd }])
}
