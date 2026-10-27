use rr_core::Resource;
use rr_mapgen::europe;
use rr_world::{Bq, World};

#[test]
fn europe_has_six_buildable_starts_and_alpine_gold() {
    let m = europe::generate(1);
    assert_eq!(m.players(), 6);
    let w = World::new(&m);
    for (i, hq) in m.hq.iter().flatten().enumerate() {
        assert_eq!(w.bq(*hq), Bq::Castle, "start {} ({}) not buildable", i, europe::STARTS[i].0);
    }
    // the Alps hold gold
    let g = m.grid;
    let alps = m.hq[0].unwrap(); // Italy; the Alps are north of it
    let gold_near = g.within(alps, 40).filter(|p| matches!(m.resources[g.index(*p)], Resource::Gold(_))).count();
    assert!(gold_near > 20, "only {gold_near} gold nodes near Italy");
    let water = m.terrain_rsu.iter().filter(|t| **t == 5).count();
    assert!(water > g.len() / 3, "Europe is half sea");
}

#[test]
fn every_rival_capital_can_be_reached_on_foot_from_italy() {
    let m = europe::generate(1);
    let w = World::new(&m);
    let g = m.grid;
    let start = m.hq[0].unwrap();
    // flood fill over walkable nodes and edges
    let mut seen = vec![false; g.len()];
    let mut stack = vec![start];
    seen[g.index(start)] = true;
    while let Some(p) = stack.pop() {
        for d in rr_core::Direction::ALL {
            let q = g.neighbor(p, d);
            if !seen[g.index(q)] && w.edge_walkable(p, d) && w.node_walkable(q) {
                seen[g.index(q)] = true;
                stack.push(q);
            }
        }
    }
    for (i, hq) in m.hq.iter().flatten().enumerate() {
        assert!(seen[g.index(*hq)], "{} is not reachable from Italy", europe::STARTS[i].0);
    }
}

#[test]
fn coasts_have_harbor_spots() {
    let m = europe::generate(1);
    assert!(m.harbors.len() >= 15, "only {} harbor spots on Europe", m.harbors.len());
    let w = World::new(&m);
    let castles = m.harbors.iter().filter(|h| w.bq(**h) == Bq::Harbor).count();
    assert!(castles * 2 >= m.harbors.len(), "most harbor spots must be buildable: {castles}/{}", m.harbors.len());
    let r =
        rr_mapgen::generate(&rr_mapgen::GenParams { width: 96, height: 96, players: 3, seed: 2, ..Default::default() });
    assert!(r.harbors.len() >= 2, "random map: {} harbor spots", r.harbors.len());
}
