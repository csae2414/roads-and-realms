use rr_core::{Animal, MapCoord, MapData, MapObject, Resource};
use rr_formats::wld;

fn sample() -> MapData {
    let mut m = MapData::flat(16, 12, 8);
    m.name = "Testland".into();
    m.author = "rr".into();
    let g = m.grid;
    m.altitude[g.index(MapCoord::new(3, 3))] = 17;
    m.terrain_rsu[g.index(MapCoord::new(5, 5))] = 1;
    m.resources[g.index(MapCoord::new(5, 5))] = Resource::Gold(6);
    m.resources[g.index(MapCoord::new(6, 5))] = Resource::Water;
    m.objects[g.index(MapCoord::new(2, 8))] = MapObject::Tree(4);
    m.objects[g.index(MapCoord::new(9, 8))] = MapObject::Granite { kind: 1, size: 5 };
    m.animals[g.index(MapCoord::new(1, 1))] = Some(Animal::Deer);
    m.hq = vec![Some(MapCoord::new(4, 4)), Some(MapCoord::new(12, 8))];
    m
}

#[test]
fn write_then_read_roundtrips() {
    let m = sample();
    let bytes = wld::write(&m);
    let back = wld::read(&bytes).expect("parse");
    assert_eq!(back, m);
    assert_eq!(back.players(), 2);
}

#[test]
fn rejects_garbage() {
    assert!(wld::read(b"not a map").is_err());
    let mut bytes = wld::write(&sample());
    bytes.truncate(bytes.len() / 2);
    assert!(wld::read(&bytes).is_err());
}

/// Every free map shipped with Return to the Roots must parse (dev check; skipped if the
/// reference checkout is missing).
#[test]
fn reads_all_rttr_free_maps() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../reference/s25client/data/RTTR/MAPS");
    if !dir.exists() {
        eprintln!("skipped: {dir:?} missing");
        return;
    }
    let mut n = 0;
    for entry in walk(&dir) {
        let ext = entry.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        if ext != "swd" && ext != "wld" {
            continue;
        }
        let bytes = std::fs::read(&entry).unwrap();
        let m = wld::read(&bytes).unwrap_or_else(|e| panic!("{entry:?}: {e}"));
        assert!(m.players() >= 1, "{entry:?} has no HQ");
        assert!(m.terrain_rsu.iter().all(|&t| t < 0x40));
        n += 1;
    }
    assert!(n > 100, "only {n} maps found");
}

fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() { out.extend(walk(&p)) } else { out.push(p) }
    }
    out.sort();
    out
}
