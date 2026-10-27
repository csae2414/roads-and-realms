use rr_rules::{BuildingType, Rules};
use rr_sim::{Cmd, GameSetup, PlayerCommand, PlayerSetup, Sim};
use std::sync::Arc;

#[test]
fn site_near_italy_hq_survives() {
    let map = rr_mapgen::europe::generate(1);
    let players =
        (0..6).map(|i| PlayerSetup { team: Some(if i == 0 { 0 } else { 1 }), ..Default::default() }).collect();
    let mut sim =
        Sim::new(Arc::new(Rules::classic()), &map, &GameSetup { seed: 1, players, ..Default::default() }).unwrap();
    let mut ais: Vec<rr_ai::Ai> = (1..6).map(rr_ai::Ai::new).collect();
    // a hut spot near the HQ (the AIs build all around it)
    let hq = map.hq[0].unwrap();
    let g = sim.world().grid();
    let at = g
        .within(hq, 6)
        .filter(|p| g.distance(*p, hq) >= 3)
        .find(|p| sim.world().bq_for(*p, 0) == rr_world::Bq::Castle)
        .expect("a spot near the HQ");
    for i in 0..400 {
        let mut cmds = Vec::new();
        for ai in ais.iter_mut() {
            cmds.extend(ai.think(&sim));
        }
        if i == 64 {
            cmds.push(PlayerCommand { player: 0, cmd: Cmd::PlaceBuilding { at, kind: BuildingType::Woodcutter } });
        }
        let out = sim.step(&cmds);
        for e in out.events {
            if !matches!(e, rr_sim::Event::Sound { .. }) {
                eprintln!("gf {i}: {e:?}");
            }
        }
        if i > 64 && sim.building_at(at).is_none() {
            panic!("site vanished at gf {i}; obj {:?}", sim.world().obj(at));
        }
    }
}
