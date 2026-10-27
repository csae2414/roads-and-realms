//! Our own Europe (Track B): coastlines from Natural Earth (public domain), mountain ranges from
//! real coordinates, climate by latitude. Not derived from the original game's map.

use crate::region::{Range, Region};
use crate::{T_DESERT, T_FLOWERS, T_MEADOW, T_SAVANNAH, T_STEPPE};
use rr_core::Landscape;
use rr_core::MapData;

pub const W: u16 = 200;
pub const H: u16 = 232;

const RANGES: &[Range] = &[
    Range {
        name: "Alps",
        pts: &[(5.6, 44.2), (6.9, 45.8), (8.5, 46.3), (10.5, 46.6), (12.5, 46.9), (14.5, 47.3), (16.0, 47.6)],
        width: 1.0,
        height: 1.0,
        gold: 0.35,
    },
    Range { name: "Pyrenees", pts: &[(-1.8, 43.1), (0.5, 42.7), (2.8, 42.5)], width: 0.5, height: 0.85, gold: 0.1 },
    Range { name: "Cantabrian", pts: &[(-7.0, 43.0), (-4.0, 43.1)], width: 0.45, height: 0.6, gold: 0.1 },
    Range { name: "Sistema Central", pts: &[(-6.5, 40.3), (-3.5, 40.7)], width: 0.4, height: 0.55, gold: 0.05 },
    Range { name: "Sierra Nevada", pts: &[(-5.0, 37.0), (-2.5, 37.2)], width: 0.45, height: 0.7, gold: 0.15 },
    Range {
        name: "Apennines",
        pts: &[(8.6, 44.4), (11.0, 44.0), (13.0, 42.6), (14.6, 41.4), (16.0, 40.2), (16.2, 38.8)],
        width: 0.45,
        height: 0.6,
        gold: 0.1,
    },
    Range { name: "Massif Central", pts: &[(2.2, 45.0), (3.4, 45.6)], width: 0.6, height: 0.45, gold: 0.05 },
    Range {
        name: "Carpathians",
        pts: &[(17.6, 48.8), (20.0, 49.3), (22.6, 49.1), (24.6, 47.8), (25.7, 46.0), (24.0, 45.4), (22.0, 45.1)],
        width: 0.6,
        height: 0.7,
        gold: 0.3,
    },
    Range {
        name: "Dinaric Alps",
        pts: &[(14.5, 45.6), (16.5, 44.0), (18.5, 43.0), (20.0, 42.0), (21.0, 40.5), (21.8, 39.2)],
        width: 0.6,
        height: 0.65,
        gold: 0.15,
    },
    Range { name: "Balkan Mountains", pts: &[(22.6, 43.4), (25.6, 42.7)], width: 0.4, height: 0.5, gold: 0.15 },
    Range { name: "Bohemian Forest", pts: &[(12.2, 50.3), (15.6, 50.8)], width: 0.4, height: 0.4, gold: 0.1 },
    Range {
        name: "Scandinavian Mountains",
        pts: &[(6.2, 58.6), (7.6, 61.0), (10.0, 63.0), (14.0, 66.0), (18.0, 68.5), (21.0, 69.6)],
        width: 1.0,
        height: 0.8,
        gold: 0.12,
    },
    Range { name: "Scottish Highlands", pts: &[(-5.6, 56.6), (-3.4, 57.4)], width: 0.5, height: 0.5, gold: 0.08 },
    Range { name: "Caucasus", pts: &[(39.8, 43.6), (43.0, 42.9), (46.0, 42.0)], width: 0.6, height: 1.0, gold: 0.25 },
    Range {
        name: "Atlas",
        pts: &[(-6.0, 34.3), (-2.0, 34.8), (3.0, 35.8), (8.0, 36.2)],
        width: 0.5,
        height: 0.6,
        gold: 0.1,
    },
    Range {
        name: "Anatolian Highlands",
        pts: &[(30.0, 38.0), (35.0, 38.5), (40.0, 39.5)],
        width: 1.0,
        height: 0.55,
        gold: 0.1,
    },
];

/// Land bridges over narrow straits so every region can be reached on foot (no seafaring):
/// (from, to) in (lon, lat). Dover (Doggerland), the Øresund and the Strait of Messina.
const BRIDGES: &[((f32, f32), (f32, f32))] =
    &[((1.0, 51.2), (2.2, 50.9)), ((12.4, 55.7), (13.2, 55.6)), ((15.5, 38.3), (15.7, 38.1))];

/// Start positions (lon, lat): a player per region.
pub const STARTS: [(&str, f32, f32); 6] = [
    ("Italy", 11.2, 44.8),
    ("Iberia", -4.0, 40.0),
    ("Britain", -1.6, 52.6),
    ("Scandinavia", 15.0, 59.8),
    ("Germania", 10.0, 51.2),
    ("Sarmatia", 32.0, 52.0),
];

fn climate(_lon: f32, lat: f32, v: f32, v2: f32) -> u8 {
    if lat < 37.5 && v > 0.55 {
        T_DESERT
    } else if lat < 41.0 && v > 0.45 {
        T_SAVANNAH
    } else if lat < 43.0 && v > 0.6 || lat > 64.0 && v > 0.55 {
        T_STEPPE
    } else if v < 0.22 {
        T_FLOWERS
    } else {
        T_MEADOW[(v2 * 2.99) as usize]
    }
}

fn forest(lat: f32) -> (f32, bool) {
    (
        if lat > 56.0 {
            0.62
        } else if lat > 45.0 {
            0.55
        } else {
            0.4
        },
        lat > 55.0,
    )
}

pub const EUROPE: Region = Region {
    name: "Europa",
    mask: include_str!("../../../data/maps/europe_mask.txt"),
    w: W,
    h: H,
    lon0: -11.0,
    lon1: 46.0,
    lat0: 71.5,
    lat1: 34.0,
    ranges: RANGES,
    bridges: BRIDGES,
    starts: &STARTS,
    climate,
    forest,
    landscape: Landscape::Greenland,
};

/// Names of the mountain ranges on the map.
pub fn range_names() -> Vec<&'static str> {
    EUROPE.range_names()
}

pub fn mask() -> Vec<u8> {
    crate::region::mask(&EUROPE)
}

pub fn generate(seed: u64) -> MapData {
    crate::region::generate(&EUROPE, seed)
}
