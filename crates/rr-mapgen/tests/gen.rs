use rr_core::{MapObject, Resource};
use rr_mapgen::{GenParams, generate};
use rr_world::{Bq, World, terrain};

#[test]
fn generated_map_is_playable() {
    for seed in [1u64, 2, 3, 99] {
        let m = generate(&GenParams { width: 96, height: 96, players: 4, seed, ..Default::default() });
        assert_eq!(m.players(), 4, "seed {seed}");
        let w = World::new(&m);
        let g = m.grid;
        for hq in m.hq.iter().flatten() {
            assert_eq!(w.bq(*hq), Bq::Castle, "seed {seed}: HQ at {hq:?} must be castle-buildable");
            // enough buildable land around each HQ
            let room = g.within(*hq, 8).filter(|p| w.bq(*p) != Bq::Nothing).count();
            assert!(room > 120, "seed {seed}: only {room} usable nodes near {hq:?}");
        }
        // HQs are far apart
        let hqs: Vec<_> = m.hq.iter().flatten().copied().collect();
        for i in 0..hqs.len() {
            for j in i + 1..hqs.len() {
                assert!(g.distance(hqs[i], hqs[j]) >= 24, "seed {seed}: HQs too close");
            }
        }
        let water = m.terrain_rsu.iter().filter(|t| terrain::is_water(**t)).count();
        let mountain = m.terrain_rsu.iter().filter(|t| terrain::is_mountain(**t)).count();
        assert!(water > 200 && mountain > 200, "seed {seed}: water {water}, mountain {mountain}");
        let gold = m.resources.iter().filter(|r| matches!(r, Resource::Gold(_))).count();
        let coal = m.resources.iter().filter(|r| matches!(r, Resource::Coal(_))).count();
        assert!(gold > 10 && coal > 30, "seed {seed}: gold {gold} coal {coal}");
        let trees = m.objects.iter().filter(|o| matches!(o, MapObject::Tree(_))).count();
        let stones = m.objects.iter().filter(|o| matches!(o, MapObject::Granite { .. })).count();
        assert!(trees > 300 && stones > 40, "seed {seed}: trees {trees}, stones {stones}");
    }
}

#[test]
fn generation_is_deterministic() {
    let p = GenParams { width: 64, height: 64, players: 2, seed: 7, ..Default::default() };
    assert_eq!(generate(&p), generate(&p));
}

/// New-player roast #2: every start has a real forest and stone in reach (the original's starts
/// sit next to woods), so the first woodcutter and quarry don't run dry within minutes.
pub fn assert_start_resources(m: &rr_core::MapData, what: &str) {
    let g = m.grid;
    for hq in m.hq.iter().flatten() {
        let near: Vec<_> = g.within(*hq, 10).filter(|p| g.distance(*hq, *p) >= 3).collect();
        let trees = near.iter().filter(|p| matches!(m.objects[g.index(**p)], MapObject::Tree(_))).count();
        let stone: u32 = near
            .iter()
            .map(|p| match m.objects[g.index(*p)] {
                MapObject::Granite { size, .. } => size as u32,
                _ => 0,
            })
            .sum();
        assert!(trees >= 25, "{what}: only {trees} trees near {hq:?}");
        assert!(stone >= 30, "{what}: only {stone} stone near {hq:?}");
    }
}

#[test]
fn every_start_has_a_forest_and_stone() {
    for seed in [1u64, 2, 3, 99] {
        for landscape in [rr_core::Landscape::Greenland, rr_core::Landscape::Wasteland, rr_core::Landscape::Winter] {
            let m = generate(&GenParams { width: 96, height: 96, players: 4, seed, landscape, ..Default::default() });
            assert_start_resources(&m, &format!("seed {seed} {landscape:?}"));
        }
    }
    for key in ["europe", "mediterranean", "northamerica", "fareast", "north"] {
        let m = rr_mapgen::region::generate(rr_mapgen::world::region(key).unwrap(), 1);
        assert_start_resources(&m, key);
    }
}
