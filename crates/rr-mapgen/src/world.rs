//! The world campaign beyond Europe: more real regions built like `europe`.

use crate::region::{Range, Region};
use crate::{T_DESERT, T_FLOWERS, T_MEADOW, T_MOUNTAIN_MEADOW, T_SAVANNAH, T_STEPPE};
use rr_core::Landscape;

// ------------------------------------------------------------------ Mare Nostrum

const MED_RANGES: &[Range] = &[
    Range {
        name: "Atlas",
        pts: &[(-8.0, 31.0), (-4.0, 33.0), (0.0, 34.6), (5.0, 35.5), (9.0, 35.8)],
        width: 0.8,
        height: 0.8,
        gold: 0.2,
    },
    Range { name: "Pyrenees", pts: &[(-1.8, 43.1), (0.5, 42.7), (2.8, 42.5)], width: 0.5, height: 0.85, gold: 0.1 },
    Range { name: "Sierra Nevada", pts: &[(-5.0, 37.0), (-2.5, 37.2)], width: 0.45, height: 0.7, gold: 0.15 },
    Range {
        name: "Alps",
        pts: &[(5.6, 44.2), (6.9, 45.8), (8.5, 46.3), (10.5, 46.6)],
        width: 0.9,
        height: 1.0,
        gold: 0.35,
    },
    Range {
        name: "Apennines",
        pts: &[(8.6, 44.4), (11.0, 44.0), (13.0, 42.6), (14.6, 41.4), (16.0, 40.2), (16.2, 38.8)],
        width: 0.45,
        height: 0.6,
        gold: 0.1,
    },
    Range {
        name: "Dinaric Alps",
        pts: &[(14.5, 45.6), (16.5, 44.0), (18.5, 43.0), (20.0, 42.0), (21.0, 40.5), (21.8, 39.2)],
        width: 0.6,
        height: 0.65,
        gold: 0.15,
    },
    Range { name: "Balkan Mountains", pts: &[(22.6, 43.4), (25.6, 42.7)], width: 0.4, height: 0.5, gold: 0.15 },
    Range {
        name: "Taurus",
        pts: &[(29.0, 37.0), (33.0, 37.0), (36.0, 37.5), (39.0, 38.0)],
        width: 0.6,
        height: 0.8,
        gold: 0.25,
    },
    Range {
        name: "Pontic Mountains",
        pts: &[(33.0, 41.2), (37.0, 40.8), (41.0, 40.8)],
        width: 0.5,
        height: 0.6,
        gold: 0.15,
    },
    Range { name: "Lebanon", pts: &[(35.6, 33.2), (36.4, 34.6)], width: 0.35, height: 0.6, gold: 0.1 },
    Range { name: "Sinai", pts: &[(33.6, 28.6), (34.2, 29.6)], width: 0.4, height: 0.6, gold: 0.3 },
    Range { name: "Caucasus", pts: &[(39.8, 43.6), (42.0, 43.0)], width: 0.6, height: 1.0, gold: 0.25 },
];

const MED_BRIDGES: &[((f32, f32), (f32, f32))] = &[
    ((-5.6, 36.1), (-5.5, 35.8)), // Gibraltar
    ((15.5, 38.3), (15.7, 38.1)), // Messina
    ((28.9, 41.0), (29.1, 41.1)), // Bosporus
    ((26.4, 40.1), (26.7, 40.3)), // Dardanelles
];

pub const MED_STARTS: [(&str, f32, f32); 6] = [
    ("Carthago", 9.8, 36.2),
    ("Hispania", -4.0, 39.6),
    ("Roma", 12.6, 42.2),
    ("Graecia", 22.2, 39.6),
    ("Anatolia", 32.5, 39.2),
    ("Aegyptus", 31.0, 30.6),
];

fn med_climate(lon: f32, lat: f32, v: f32, v2: f32) -> u8 {
    // the Nile delta and valley are green in the middle of the desert
    let nile = (28.8..33.4).contains(&lon) && (lat > 29.9 || (30.2..32.0).contains(&lon));
    if nile {
        return if v > 0.6 { T_SAVANNAH } else { T_MEADOW[(v2 * 2.99) as usize] };
    }
    if lat < 31.5 && v > 0.2 || lat < 34.0 && v > 0.5 {
        T_DESERT
    } else if lat < 38.0 && v > 0.45 {
        T_SAVANNAH
    } else if v > 0.62 {
        T_STEPPE
    } else if v < 0.22 {
        T_FLOWERS
    } else {
        T_MEADOW[(v2 * 2.99) as usize]
    }
}

fn med_forest(lat: f32) -> (f32, bool) {
    (if lat > 41.0 { 0.45 } else { 0.3 }, false)
}

pub const MEDITERRANEAN: Region = Region {
    name: "Mare Nostrum",
    mask: include_str!("../../../data/maps/mediterranean_mask.txt"),
    w: 200,
    h: 160,
    lon0: -10.0,
    lon1: 42.0,
    lat0: 46.0,
    lat1: 28.0,
    ranges: MED_RANGES,
    bridges: MED_BRIDGES,
    starts: &MED_STARTS,
    climate: med_climate,
    forest: med_forest,
    landscape: Landscape::Greenland,
};

// ------------------------------------------------------------------ North America

const NA_RANGES: &[Range] = &[
    Range {
        name: "Rocky Mountains",
        pts: &[(-118.0, 54.0), (-114.0, 49.0), (-111.0, 45.0), (-107.0, 40.0), (-106.0, 36.0), (-105.0, 32.0)],
        width: 2.0,
        height: 1.0,
        gold: 0.3,
    },
    Range { name: "Sierra Nevada", pts: &[(-121.5, 40.5), (-118.5, 36.0)], width: 0.7, height: 0.9, gold: 0.4 },
    Range { name: "Cascade Range", pts: &[(-121.5, 49.0), (-122.0, 43.0)], width: 0.6, height: 0.8, gold: 0.1 },
    Range {
        name: "Appalachians",
        pts: &[(-84.5, 34.0), (-80.0, 37.5), (-77.0, 40.5), (-73.5, 42.5), (-69.0, 45.5)],
        width: 1.0,
        height: 0.55,
        gold: 0.06,
    },
    Range { name: "Ozarks", pts: &[(-94.0, 36.0), (-91.0, 37.0)], width: 0.7, height: 0.35, gold: 0.05 },
    Range { name: "Black Hills", pts: &[(-103.9, 44.2), (-103.4, 43.6)], width: 0.4, height: 0.5, gold: 0.5 },
    Range { name: "Sierra Madre Oriental", pts: &[(-100.5, 24.0), (-99.0, 26.8)], width: 0.7, height: 0.6, gold: 0.2 },
];

pub const NA_STARTS: [(&str, f32, f32); 6] = [
    ("Virginia", -78.0, 37.6),
    ("New England", -71.6, 43.0),
    ("Great Lakes", -85.0, 42.2),
    ("Texas", -97.5, 31.0),
    ("California", -120.6, 37.6),
    ("Cascadia", -122.6, 46.6),
];

fn na_climate(lon: f32, lat: f32, v: f32, v2: f32) -> u8 {
    let southwest = lon < -103.0 && lat < 40.0;
    let plains = (-104.0..-96.0).contains(&lon);
    if southwest && v > 0.3 || lon < -112.0 && lat < 35.0 {
        T_DESERT
    } else if southwest || plains && v > 0.35 || lon < -100.0 && lat < 44.0 && v > 0.5 {
        T_STEPPE
    } else if lat < 31.0 && v > 0.5 {
        T_SAVANNAH
    } else if v < 0.22 {
        T_FLOWERS
    } else {
        T_MEADOW[(v2 * 2.99) as usize]
    }
}

fn na_forest(lat: f32) -> (f32, bool) {
    (if lat > 44.0 { 0.6 } else { 0.5 }, lat > 45.0)
}

pub const NORTH_AMERICA: Region = Region {
    name: "North America",
    mask: include_str!("../../../data/maps/northamerica_mask.txt"),
    w: 200,
    h: 176,
    lon0: -126.0,
    lon1: -64.0,
    lat0: 54.0,
    lat1: 24.0,
    ranges: NA_RANGES,
    bridges: &[],
    starts: &NA_STARTS,
    climate: na_climate,
    forest: na_forest,
    landscape: Landscape::Greenland,
};

// ------------------------------------------------------------------ Far East

const FE_RANGES: &[Range] = &[
    Range { name: "Hengduan Mountains", pts: &[(100.5, 26.5), (102.0, 32.0)], width: 1.2, height: 1.0, gold: 0.2 },
    Range { name: "Qinling", pts: &[(104.0, 34.0), (108.0, 33.8), (112.0, 33.5)], width: 0.8, height: 0.8, gold: 0.2 },
    Range { name: "Taihang", pts: &[(112.5, 35.0), (113.5, 37.5), (114.2, 39.8)], width: 0.5, height: 0.6, gold: 0.1 },
    Range {
        name: "Greater Khingan",
        pts: &[(118.5, 42.0), (120.5, 46.0), (122.0, 50.0)],
        width: 0.8,
        height: 0.6,
        gold: 0.3,
    },
    Range { name: "Changbai", pts: &[(126.0, 41.2), (129.0, 42.5)], width: 0.6, height: 0.7, gold: 0.3 },
    Range { name: "Taebaek", pts: &[(128.0, 38.5), (129.3, 35.5)], width: 0.4, height: 0.6, gold: 0.15 },
    Range {
        name: "Japanese Alps",
        pts: &[(136.5, 35.5), (137.5, 36.4), (138.2, 37.0)],
        width: 0.5,
        height: 0.9,
        gold: 0.3,
    },
    Range { name: "Nanling", pts: &[(110.0, 25.0), (116.0, 25.5)], width: 0.6, height: 0.5, gold: 0.15 },
    Range { name: "Wuyi", pts: &[(116.0, 25.5), (119.0, 28.0)], width: 0.6, height: 0.5, gold: 0.1 },
];

pub const FE_STARTS: [(&str, f32, f32); 6] = [
    ("Zhongyuan", 113.5, 34.6),
    ("Manchuria", 125.5, 45.5),
    ("Korea", 127.3, 37.2),
    ("Yamato", 137.0, 35.0),
    ("Jiangnan", 119.5, 31.0),
    ("Lingnan", 113.2, 23.4),
];

fn fe_climate(lon: f32, lat: f32, v: f32, v2: f32) -> u8 {
    let gobi = lat > 39.5 && lon < 114.0;
    if gobi && v > 0.3 {
        T_DESERT
    } else if gobi || lat > 41.0 && lon < 118.0 && v > 0.45 {
        T_STEPPE
    } else if lat < 25.5 && v > 0.5 {
        T_SAVANNAH
    } else if v < 0.22 {
        T_FLOWERS
    } else {
        T_MEADOW[(v2 * 2.99) as usize]
    }
}

fn fe_forest(lat: f32) -> (f32, bool) {
    (if lat > 40.0 { 0.6 } else { 0.5 }, lat > 42.0)
}

/// Japan stays an island: reach it with ships.
pub const FAR_EAST: Region = Region {
    name: "Far East",
    mask: include_str!("../../../data/maps/fareast_mask.txt"),
    w: 200,
    h: 176,
    lon0: 100.0,
    lon1: 146.0,
    lat0: 50.0,
    lat1: 20.0,
    ranges: FE_RANGES,
    bridges: &[],
    starts: &FE_STARTS,
    climate: fe_climate,
    forest: fe_forest,
    landscape: Landscape::Greenland,
};

// ------------------------------------------------------------------ The North (winter)

const NO_RANGES: &[Range] = &[
    Range {
        name: "Scandes",
        pts: &[(6.0, 59.0), (8.0, 61.0), (11.5, 62.8), (14.0, 65.0), (16.5, 67.5), (19.5, 69.0)],
        width: 1.2,
        height: 0.9,
        gold: 0.3,
    },
    Range { name: "Khibiny", pts: &[(33.0, 67.6), (34.5, 67.8)], width: 0.6, height: 0.7, gold: 0.25 },
    Range { name: "Suomenselkä", pts: &[(23.0, 62.8), (27.0, 63.6)], width: 0.5, height: 0.35, gold: 0.1 },
];

pub const NO_STARTS: [(&str, f32, f32); 6] = [
    ("Svealand", 16.2, 59.9),
    ("Norvegr", 10.4, 60.4),
    ("Suomi", 24.5, 61.6),
    ("Livonia", 25.4, 57.4),
    ("Novgorod", 31.6, 58.7),
    ("Sapmi", 26.0, 67.2),
];

fn no_climate(_lon: f32, lat: f32, v: f32, v2: f32) -> u8 {
    if lat > 66.5 && v > 0.45 {
        // open tundra
        T_MOUNTAIN_MEADOW
    } else if v < 0.2 {
        T_FLOWERS
    } else if v2 < 0.25 {
        // green tundra only in patches; mostly frost-grey and icy meadows
        T_MEADOW[1]
    } else if v2 < 0.6 {
        T_MEADOW[0]
    } else {
        T_MEADOW[2]
    }
}

fn no_forest(lat: f32) -> (f32, bool) {
    (if lat > 67.0 { 0.35 } else { 0.65 }, true)
}

/// Winter landscape: taiga, frozen lakes, the Scandes.
pub const NORTH: Region = Region {
    name: "The North",
    mask: include_str!("../../../data/maps/north_mask.txt"),
    w: 200,
    h: 184,
    lon0: 4.0,
    lon1: 42.0,
    lat0: 71.0,
    lat1: 53.0,
    ranges: NO_RANGES,
    bridges: &[],
    starts: &NO_STARTS,
    climate: no_climate,
    forest: no_forest,
    landscape: Landscape::Winter,
};

/// Starts that are only reachable by ship.
pub fn island_starts(key: &str) -> &'static [&'static str] {
    match key {
        "fareast" => &["Yamato"],
        _ => &[],
    }
}

/// The campaign's regions, by key.
pub fn region(key: &str) -> Option<&'static Region> {
    match key {
        "europe" => Some(&crate::europe::EUROPE),
        "mediterranean" => Some(&MEDITERRANEAN),
        "northamerica" => Some(&NORTH_AMERICA),
        "fareast" => Some(&FAR_EAST),
        "north" => Some(&NORTH),
        _ => None,
    }
}
