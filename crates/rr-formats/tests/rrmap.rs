use rr_core::{MapCoord, MapData, MapObject, Resource};

#[test]
fn rrmap_round_trip() {
    let mut m = MapData::flat(32, 24, 8);
    m.name = "Test".into();
    m.hq = vec![Some(MapCoord::new(5, 6)), None];
    m.objects[3] = MapObject::Tree(2);
    m.resources[7] = Resource::Gold(5);
    let bytes = rr_formats::rrmap::write(&m);
    assert_eq!(rr_formats::rrmap::read(&bytes).unwrap(), m);
    assert_eq!(rr_formats::rrmap::read_any(&bytes).unwrap(), m);
    assert!(rr_formats::rrmap::read(b"RRMAP\0\x09\x00").is_err(), "other version refused");
    assert!(rr_formats::rrmap::read(b"nonsense").is_err());
}

/// A file can claim anything: broken maps are refused, never a panic later.
#[test]
fn rrmap_refuses_broken_maps() {
    let ok = MapData::flat(32, 24, 8);
    let check = |m: &MapData| rr_formats::rrmap::read(&rr_formats::rrmap::write(m));
    assert!(check(&ok).is_ok());
    let mut off = ok.clone();
    off.hq = vec![Some(MapCoord::new(40, 3))];
    assert!(check(&off).is_err(), "headquarters off the map");
    let mut harbor = ok.clone();
    harbor.harbors = vec![MapCoord::new(3, 99)];
    assert!(check(&harbor).is_err(), "harbor off the map");
    // a grid the game can't use (written by hand: Grid::new would refuse it)
    let mut odd = ok.clone();
    odd.grid = postcard::from_bytes::<rr_core::Grid>(&postcard::to_stdvec(&(32u16, 23u16)).unwrap()).unwrap();
    assert!(check(&odd).is_err(), "odd height");
}

/// A small map with one of everything the format holds.
fn sample() -> MapData {
    let mut m = MapData::flat(8, 8, 8);
    m.name = "Sample".into();
    m.author = "Roads & Realms".into();
    m.landscape = rr_core::Landscape::Winter;
    m.altitude[1] = 12;
    m.terrain_rsu[2] = 5;
    m.terrain_usd[2] = 6;
    m.resources[3] = Resource::Coal(1);
    m.resources[4] = Resource::Iron(2);
    m.resources[5] = Resource::Gold(3);
    m.resources[6] = Resource::Granite(4);
    m.resources[7] = Resource::Fish(5);
    m.resources[8] = Resource::Water;
    m.objects[9] = MapObject::Tree(8);
    m.objects[10] = MapObject::Granite { kind: 1, size: 6 };
    m.objects[11] = MapObject::Decoration(42);
    m.animals[12] = Some(rr_core::Animal::Duck);
    m.hq = vec![Some(MapCoord::new(2, 2)), None, Some(MapCoord::new(6, 6))];
    m.harbors = vec![MapCoord::new(4, 0)];
    m
}

/// Map-maker roast #14: the bytes of a version-1 file are pinned. When this fails, the map
/// format changed: raise `VERSION` in `rrmap.rs`, keep reading version 1 files, and describe
/// the change in docs/MAPFORMAT.md. (`RR_WRITE_SAMPLE=1` writes the file anew.)
#[test]
fn version_1_files_keep_loading() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/sample-v1.rrmap");
    if std::env::var_os("RR_WRITE_SAMPLE").is_some() {
        std::fs::write(path, rr_formats::rrmap::write(&sample())).unwrap();
    }
    let bytes = std::fs::read(path).unwrap();
    assert_eq!(rr_formats::rrmap::read(&bytes).unwrap(), sample());
    assert_eq!(rr_formats::rrmap::write(&sample()), bytes);
}
