use rr_core::{Direction, MapCoord, MapData};
use rr_world::{Bq, NodeObj, RoadKind, World, terrain};

const MEADOW: u8 = 8;
const MOUNTAIN: u8 = 1;
const WATER: u8 = 5;
const SNOW: u8 = 2;

fn c(x: u16, y: u16) -> MapCoord {
    MapCoord::new(x, y)
}

fn world(t: u8) -> World {
    World::new(&MapData::flat(32, 32, t))
}

/// Set all six triangles around `p` to terrain `t`.
fn paint(w: &mut World, p: MapCoord, t: u8) {
    for (q, tri) in w.triangles_around(p) {
        w.set_terrain(q, tri, t);
    }
}

#[test]
fn terrain_table_matches_rttr() {
    use terrain::TerrainBq::*;
    assert_eq!(terrain::bq(MEADOW), Castle);
    assert_eq!(terrain::bq(MOUNTAIN), Mine);
    assert_eq!(terrain::bq(4), Flag); // desert: walkable only
    assert_eq!(terrain::bq(3), Nothing); // swamp
    assert_eq!(terrain::bq(WATER), Nothing);
    assert_eq!(terrain::bq(SNOW), Danger);
    assert_eq!(terrain::bq(16), Danger); // lava
    assert_eq!(terrain::bq(34), Castle); // buildable mountain
    assert!(terrain::is_water(WATER));
    assert!(terrain::is_mountain(13));
}

#[test]
fn flat_meadow_is_castle_mountain_is_mine_water_nothing() {
    assert_eq!(world(MEADOW).bq(c(10, 10)), Bq::Castle);
    assert_eq!(world(MOUNTAIN).bq(c(10, 10)), Bq::Mine);
    assert_eq!(world(WATER).bq(c(10, 10)), Bq::Nothing);
    assert_eq!(world(SNOW).bq(c(10, 10)), Bq::Nothing);
}

#[test]
fn mixed_terrain_gives_flag_and_danger_gives_nothing() {
    let mut w = world(MEADOW);
    let p = c(10, 10);
    let (q, tri) = w.triangles_around(p)[0];
    w.set_terrain(q, tri, WATER);
    assert_eq!(w.bq(p), Bq::Flag);
    w.set_terrain(q, tri, SNOW);
    assert_eq!(w.bq(p), Bq::Nothing);
}

#[test]
fn altitude_rules() {
    let p = c(10, 10);
    // flag point (SE) more than 1 higher -> flag
    let mut w = world(MEADOW);
    let se = w.grid().neighbor(p, Direction::SE);
    w.set_altitude(se, 12);
    assert_eq!(w.bq(p), Bq::Flag);
    // direct neighbor differs by > 3 -> flag
    let mut w = world(MEADOW);
    w.set_altitude(w.grid().neighbor(p, Direction::W), 14);
    assert_eq!(w.bq(p), Bq::Flag);
    // radius-2 node differs by > 2 -> hut
    let mut w = world(MEADOW);
    let far = w.grid().walk(p, Direction::W, 2);
    w.set_altitude(far, 13);
    assert_eq!(w.bq(p), Bq::Hut);
}

#[test]
fn trees_allow_huts_stones_allow_flags() {
    let p = c(10, 10);
    let mut w = world(MEADOW);
    w.set_obj(w.grid().neighbor(p, Direction::NE), NodeObj::Tree { species: 0, stage: 3 });
    assert_eq!(w.bq(p), Bq::Hut);
    let mut w = world(MEADOW);
    w.set_obj(w.grid().neighbor(p, Direction::NE), NodeObj::Granite { kind: 0, size: 4 });
    assert_eq!(w.bq(p), Bq::Flag);
    // and the node with the object itself is unusable
    assert_eq!(w.bq(w.grid().neighbor(p, Direction::NE)), Bq::Nothing);
}

#[test]
fn flags_block_neighbors() {
    let p = c(10, 10);
    let mut w = world(MEADOW);
    w.set_obj(w.grid().neighbor(p, Direction::E), NodeObj::Flag(1));
    assert_eq!(w.bq(p), Bq::Nothing, "flag east blocks everything");
    let mut w = world(MEADOW);
    w.set_obj(w.grid().neighbor(p, Direction::W), NodeObj::Flag(1));
    // a building at p would have its flag at SE, which is fine, but castles need W/NW/NE free
    assert_eq!(w.bq(p), Bq::House);
    // the SE neighbor of a flag's west node...
    let mut w = world(MEADOW);
    w.set_obj(w.grid().neighbor(p, Direction::SE), NodeObj::Flag(1));
    assert_eq!(w.bq(p), Bq::Castle, "existing flag at the flag spot is fine");
}

#[test]
fn building_within_two_reduces_castle_to_house() {
    let p = c(10, 10);
    let mut w = world(MEADOW);
    w.set_obj(w.grid().walk(p, Direction::E, 2), NodeObj::Building(7));
    assert_eq!(w.bq(p), Bq::House);
}

#[test]
fn roads_reduce_bq() {
    let p = c(10, 10);
    let mut w = world(MEADOW);
    w.set_road(p, Direction::E, RoadKind::Normal);
    assert_eq!(w.road(w.grid().neighbor(p, Direction::E), Direction::W), RoadKind::Normal);
    assert!(w.is_on_road(p));
    assert_eq!(w.bq(p), Bq::Flag);
    // road at a castle attachment node (W, NW, NE) -> house
    let mut w = world(MEADOW);
    let nw = w.grid().neighbor(p, Direction::NW);
    w.set_road(nw, Direction::NW, RoadKind::Normal);
    assert_eq!(w.bq(p), Bq::House);
}

#[test]
fn territory_adjusts_bq_for_player() {
    let p = c(10, 10);
    let mut w = world(MEADOW);
    assert_eq!(w.bq_for(p, 0), Bq::Nothing, "no territory");
    for q in w.grid().within(p, 5) {
        w.set_owner(q, Some(0));
    }
    assert_eq!(w.bq_for(p, 0), Bq::Castle);
    assert_eq!(w.bq_for(p, 1), Bq::Nothing);
    // border node: neighbors not all owned
    let edge = w.grid().walk(p, Direction::W, 5);
    assert_eq!(w.bq_for(edge, 0), Bq::Nothing);
    // building flag point at the border -> flag only
    let q = w.grid().walk(p, Direction::E, 3);
    let q = w.grid().walk(q, Direction::SE, 1);
    assert_eq!(w.bq_for(q, 0), Bq::Flag);
}

#[test]
fn walkability() {
    let mut w = world(MEADOW);
    let p = c(10, 10);
    assert!(w.node_walkable(p));
    assert!(w.edge_walkable(p, Direction::E));
    paint(&mut w, p, WATER);
    assert!(!w.node_walkable(p));
    let mut w = world(MEADOW);
    let (q, tri) = w.triangles_around(p)[3];
    w.set_terrain(q, tri, SNOW);
    assert!(!w.node_walkable(p), "danger next to the node");
}

#[test]
fn harbor_spot_upgrades_castle() {
    let mut m = MapData::flat(32, 32, MEADOW);
    m.harbors.push(c(10, 10));
    let w = World::new(&m);
    assert_eq!(w.bq(c(10, 10)), Bq::Harbor);
}
