//! Terrain properties by original terrain id (lower 6 bits). Same semantics in all three
//! landscapes; only the look differs. Source: RTTR `gamedata/world/*.lua` + `TerrainDesc.cpp`.

use rr_core::Landscape;

/// What a triangle allows around a node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerrainBq {
    /// Not walkable (water, swamp).
    Nothing,
    /// Walkable only (desert).
    Flag,
    Mine,
    Castle,
    /// Snow, lava: nothing may even stand next to it.
    Danger,
}

pub fn bq(t: u8) -> TerrainBq {
    use TerrainBq::*;
    match t {
        0 | 6 | 8 | 9 | 10 | 14 | 15 | 18 | 34 => Castle,
        1 | 11 | 12 | 13 => Mine,
        4 | 7 => Flag,
        3 | 5 | 19 => Nothing,
        _ => Danger,
    }
}

pub fn is_walkable(t: u8) -> bool {
    matches!(bq(t), TerrainBq::Castle | TerrainBq::Mine | TerrainBq::Flag)
}

pub fn is_danger(t: u8) -> bool {
    bq(t) == TerrainBq::Danger
}

/// Land that is buildable (RTTR `TerrainDesc::IsVital`): trees and fields grow here.
pub fn is_vital(t: u8) -> bool {
    matches!(t, 0 | 8 | 9 | 10 | 14 | 15)
}

/// Ships sail and fish live here.
pub fn is_water(t: u8) -> bool {
    matches!(t, 5 | 6 | 19)
}

/// Mountain terrain holds coal, iron, gold and granite.
pub fn is_mountain(t: u8) -> bool {
    matches!(t, 1 | 11 | 12 | 13)
}

/// Humidity in percent; drives tree growth and farms.
pub fn humidity(t: u8) -> u8 {
    match t {
        0 => 60,
        8 => 95,
        14 => 30,
        4 | 7 => 0,
        1 | 11 | 12 | 13 | 34 => 0,
        _ if is_walkable(t) || is_water(t) => 100,
        _ => 0,
    }
}

/// Minimap/placeholder color (0xRRGGBB).
pub fn color(landscape: Landscape, t: u8) -> u32 {
    let (gl, wl, ww): (u32, u32, u32) = match t {
        0 => (0x8a9440, 0x5e5648, 0xdce4ec),
        // pale grey-ochre stone: mountains are the brightest land after snow (art director 2 #4)
        1 | 11 | 12 => (0xa89a84, 0x706454, 0x6c727a),
        13 => (0x9c8c74, 0x684c24, 0x666c76),
        2 => (0xe8edf2, 0x860000, 0x00286c),
        3 => (0x56693a, 0x001820, 0x00286c),
        // (winter: pale ice, not a saturated blue that read as water wedges)
        4 | 7 => (0xc09c7c, 0x9c7c64, 0xb4c6d6),
        5 | 6 | 19 => (0x1038a4, 0x454520, 0x1038a4),
        // calm mid-tones for the meadows, so buildings and people stand out (art director #2)
        8 | 15 => (0x648a3a, 0x7a6a4c, 0xd4dce6),
        9 => (0x6e8f3a, 0x6c5f48, 0xdce4ec),
        10 => (0x5a7e32, 0x74664a, 0xd0d9e4),
        14 => (0x95a548, 0x88b028, 0x88b15e),
        18 => (0x8c9c58, 0x5c5838, 0x94a0c0),
        34 => (0xa89a84, 0x706454, 0x6c727a),
        _ => (0xc02020, 0xc32020, 0xc02020),
    };
    match landscape {
        Landscape::Greenland => gl,
        Landscape::Wasteland => wl,
        Landscape::Winter => ww,
    }
}
