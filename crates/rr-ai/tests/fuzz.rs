//! Fuzz: random commands (mostly plausible, some invalid) from all players on a generated map.
//! The simulation must never panic, stay deterministic, and keep its invariants.

use rr_core::{Direction, MapCoord, Pcg32};
use rr_rules::{BuildingType, Rules, Ware};
use rr_sim::{Cmd, GameSetup, PlayerCommand, PlayerSetup, Sim, StockPolicy};
use std::sync::Arc;

fn random_cmd(sim: &Sim, rng: &mut Pcg32, player: u8) -> Cmd {
    let g = sim.world().grid();
    let hq = sim.players()[player as usize]
        .hq
        .and_then(|h| sim.buildings().get(h))
        .map(|b| b.pos)
        .unwrap_or(MapCoord::new(0, 0));
    let near = |rng: &mut Pcg32| {
        let r = rng.below(14) as u16;
        let ring: Vec<MapCoord> = g.ring(hq, r).collect();
        ring[rng.below(ring.len() as u32) as usize]
    };
    let flags: Vec<MapCoord> = sim.flags().iter().filter(|(_, f)| f.player == player).map(|(_, f)| f.pos).collect();
    let any_flag = |rng: &mut Pcg32| flags.get(rng.below(flags.len().max(1) as u32) as usize).copied().unwrap_or(hq);
    let kinds = BuildingType::ALL;
    match rng.below(102) {
        0..=24 => Cmd::PlaceBuilding { at: near(rng), kind: kinds[rng.below(kinds.len() as u32) as usize] },
        25..=34 => Cmd::PlaceFlag { at: near(rng) },
        35..=54 => {
            let from = any_flag(rng);
            let to = near(rng);
            match sim.road_path(player, from, to, &[]) {
                Some(dirs) => Cmd::BuildRoad { from, dirs },
                None => Cmd::BuildRoad {
                    from,
                    dirs: (0..rng.below(6)).map(|_| Direction::from_index(rng.below(6) as u8)).collect(),
                },
            }
        }
        55..=59 => Cmd::RemoveFlag { at: any_flag(rng) },
        60..=63 => Cmd::RemoveBuilding { at: near(rng) },
        64..=66 => Cmd::RemoveRoad { flag: any_flag(rng), dir: Direction::from_index(rng.below(6) as u8) },
        67..=70 => Cmd::ToggleProduction { at: near(rng) },
        71..=73 => Cmd::SendGeologist { flag: any_flag(rng) },
        74..=75 => Cmd::SendScout { flag: any_flag(rng) },
        76..=84 => {
            // attack anything of someone else
            let targets: Vec<MapCoord> =
                sim.buildings().iter().filter(|(_, b)| b.player != player).map(|(_, b)| b.pos).collect();
            let at = targets.get(rng.below(targets.len().max(1) as u32) as usize).copied().unwrap_or(hq);
            Cmd::Attack { target: at, soldiers: rng.below(12) as u8, strong: rng.chance(50) }
        }
        85..=87 => Cmd::SetGarrisonWanted { at: near(rng), wanted: rng.below(12) as u8 },
        88..=90 => {
            Cmd::SetDistribution { ware: Ware::Fish, building: BuildingType::CoalMine, weight: rng.below(12) as u8 }
        }
        91..=92 => Cmd::SetMilitary { values: [0; 8].map(|_| rng.below(12) as u8) },
        93..=94 => Cmd::SetTransportOrder { order: vec![Ware::ALL[rng.below(31) as usize]] },
        95 => Cmd::SetTools { values: vec![(Ware::Axe, rng.below(11) as u8)] },
        96 => {
            // warehouse settings at a random warehouse
            let whs: Vec<MapCoord> = sim
                .buildings()
                .iter()
                .filter(|(_, b)| b.player == player && b.inventory.is_some())
                .map(|(_, b)| b.pos)
                .collect();
            let policies = [StockPolicy::Normal, StockPolicy::Collect, StockPolicy::Stop, StockPolicy::MoveOut];
            match whs.get(rng.below(whs.len().max(1) as u32) as usize) {
                Some(at) if rng.chance(50) => Cmd::SetStockPolicy {
                    at: *at,
                    ware: Ware::ALL[rng.below(31) as usize],
                    policy: policies[rng.below(4) as usize],
                },
                Some(at) => Cmd::SetPeoplePolicy {
                    at: *at,
                    job: rr_rules::Job::ALL[rng.below(rr_rules::Job::COUNT as u32) as usize],
                    policy: policies[rng.below(4) as usize],
                },
                None => Cmd::UpgradeRoad { flag: any_flag(rng), dir: Direction::E },
            }
        }
        97 => Cmd::StartExpedition { harbor: near(rng) },
        98 => Cmd::SendExpedition { harbor: near(rng), to: near(rng) },
        99 => Cmd::SetShipyardMode { at: near(rng), ships: rng.chance(50) },
        // ranks and counts beyond what is allowed too
        100 => Cmd::SetReserve {
            at: if rng.chance(50) { hq } else { near(rng) },
            rank: rng.below(6) as u8,
            count: rng.below(30) as u8,
        },
        // out of bounds / nonsense
        _ => Cmd::PlaceFlag { at: MapCoord::new(9999, 9999) },
    }
}

fn rate() -> u32 {
    std::env::var("RR_FUZZ_RATE").ok().and_then(|v| v.parse().ok()).unwrap_or(8)
}

fn run(seed: u64, frames: u32, rate: u32) -> (u64, Sim) {
    let map =
        rr_mapgen::generate(&rr_mapgen::GenParams { width: 64, height: 64, players: 3, seed, ..Default::default() });
    let setup = GameSetup { seed, players: (0..3).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    let mut sim = Sim::new(Arc::new(Rules::classic()), &map, &setup).unwrap();
    let mut rng = Pcg32::new(seed, 99);
    let mut ais: Vec<rr_ai::Ai> = (0..3).map(rr_ai::Ai::new).collect();
    for gf in 0..frames {
        let mut cmds = Vec::new();
        // AIs build a real economy; random commands disturb it
        for ai in ais.iter_mut() {
            cmds.extend(ai.think(&sim));
        }
        if rng.chance(rate) {
            let p = rng.below(3) as u8;
            cmds.push(PlayerCommand { player: p, cmd: random_cmd(&sim, &mut rng, p) });
        }
        sim.step(&cmds);
        // (every 10 s of game time: a bad window after a capture lasted a few seconds)
        if gf % 200 == 0 {
            check_invariants(&sim);
        }
    }
    check_invariants(&sim);
    (sim.sync_hash(), sim)
}

fn check_invariants(sim: &Sim) {
    // (the audit itself lives in rr-sim now)
    if let Err(e) = sim.audit() {
        panic!("{e}");
    }
    // a save loads back to the same game at every audit point, not only at the end
    // (architect round 3, #3: item 540's unloadable saves lasted a few seconds after a capture)
    let gf = sim.gf().0;
    let back = Sim::load(Arc::new(Rules::classic()), &sim.save()).unwrap_or_else(|e| panic!("load at gf {gf}: {e}"));
    assert_eq!(back.sync_hash(), sim.sync_hash(), "loaded at gf {gf}");
}

#[test]
fn fuzz_no_panic_and_deterministic() {
    let seeds: u64 = std::env::var("RR_FUZZ_SEEDS").ok().and_then(|v| v.parse().ok()).unwrap_or(3);
    let frames: u32 = std::env::var("RR_FUZZ_FRAMES").ok().and_then(|v| v.parse().ok()).unwrap_or(20_000);
    for seed in 1..=seeds {
        let (a, sim) = run(seed, frames, rate());
        let (b, _) = run(seed, frames, rate());
        assert_eq!(a, b, "seed {seed} not deterministic");
        // save/load round trip at the end
        let back = Sim::load(Arc::new(Rules::classic()), &sim.save()).expect("load");
        assert_eq!(back.sync_hash(), sim.sync_hash());
    }
}

/// QA round 9, #1: a Helper walking home from a guardhouse whose flag was then removed kept
/// "inside" the gone building, and no save loaded for 151 frames (gf 18 287-18 437 here).
#[test]
fn a_removed_building_keeps_nobody_inside() {
    run(5, 18_500, 100);
}
