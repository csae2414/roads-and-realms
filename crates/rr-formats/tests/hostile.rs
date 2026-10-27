//! Robustness roast #1, #2: damaged or hostile map files are refused, never a crash or a huge
//! allocation; what is accepted can be built into a world.
use rr_core::{MapCoord, MapData};

fn data(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

#[test]
fn the_roast_files_are_refused() {
    // claims 65534×65534 nodes in 2.6 KB
    assert!(rr_formats::rrmap::read_any(&data("huge.swd")).is_err());
    // a 1×2 map with the "Das Dreieck" extra word
    assert!(rr_formats::rrmap::read_any(&data("tiny_extra.wld")).is_err());
}

#[test]
fn damaged_maps_never_crash() {
    let mut m = MapData::flat(32, 32, 8);
    m.hq = vec![Some(MapCoord::new(8, 8)), Some(MapCoord::new(24, 24))];
    m.harbors = vec![MapCoord::new(3, 3)];
    let originals = [rr_formats::wld::write(&m), rr_formats::rrmap::write(&m)];
    let mut seed = 99u64;
    let mut rnd = |n: usize| {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((seed >> 33) as usize) % n.max(1)
    };
    for original in &originals {
        for round in 0..3000 {
            let mut b = original.clone();
            for _ in 0..1 + rnd(8) {
                let i = rnd(b.len());
                b[i] = rnd(256) as u8;
            }
            if round % 5 == 0 {
                b.truncate(rnd(b.len()));
            }
            if let Ok(map) = rr_formats::rrmap::read_any(&b) {
                let _ = rr_world::World::new(&map);
            }
        }
    }
}
