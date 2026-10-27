use rr_mapgen::world;
use rr_world::{Bq, World};

#[test]
fn every_region_is_playable() {
    for key in ["europe", "mediterranean", "northamerica", "fareast", "north"] {
        let r = world::region(key).unwrap();
        let m = rr_mapgen::region::generate(r, 1);
        assert_eq!(m.players(), r.starts.len(), "{key}");
        let w = World::new(&m);
        let g = m.grid;
        for (i, hq) in m.hq.iter().flatten().enumerate() {
            assert_eq!(w.bq(*hq), Bq::Castle, "{key}: start {} not buildable", r.starts[i].0);
        }
        // every rival reachable on foot from the first start
        let seen = reach(&w, m.hq[0].unwrap());
        for (i, hq) in m.hq.iter().flatten().enumerate() {
            if world::island_starts(key).contains(&r.starts[i].0) {
                // the island must have a harbor spot of its own, and so must the mainland
                let island = reach(&w, *hq);
                assert!(m.harbors.iter().any(|h| island[g.index(*h)]), "{key}: no harbor on {}", r.starts[i].0);
                assert!(m.harbors.iter().any(|h| seen[g.index(*h)]), "{key}: no mainland harbor");
                continue;
            }
            assert!(seen[g.index(*hq)], "{key}: {} not reachable from {}", r.starts[i].0, r.starts[0].0);
        }
        // every start has room to grow (the original's starts are all generous); trees count as
        // room, as woodcutters clear them (each start has a forest of its own)
        let mut cleared = m.clone();
        for o in cleared.objects.iter_mut() {
            if matches!(o, rr_core::MapObject::Tree(_)) {
                *o = rr_core::MapObject::None;
            }
        }
        let wc = World::new(&cleared);
        for (i, hq) in m.hq.iter().flatten().enumerate() {
            let room = g.within(*hq, 22).filter(|p| matches!(wc.bq(*p), Bq::Hut | Bq::House | Bq::Castle)).count();
            if std::env::var_os("RR_ROOM").is_some() {
                eprintln!("{key}: {} room {room}", r.starts[i].0);
            }
            assert!(room >= 200, "{key}: {} has only {room} building spots", r.starts[i].0);
        }
        let water = m.terrain_rsu.iter().filter(|t| **t == 5).count();
        assert!(water > g.len() / 10, "{key}: too little sea");
    }
}

fn reach(w: &World, from: rr_core::MapCoord) -> Vec<bool> {
    let g = w.grid();
    let mut seen = vec![false; g.len()];
    let mut stack = vec![from];
    seen[g.index(from)] = true;
    while let Some(p) = stack.pop() {
        for d in rr_core::Direction::ALL {
            let q = g.neighbor(p, d);
            if !seen[g.index(q)] && w.edge_walkable(p, d) && w.node_walkable(q) {
                seen[g.index(q)] = true;
                stack.push(q);
            }
        }
    }
    seen
}
