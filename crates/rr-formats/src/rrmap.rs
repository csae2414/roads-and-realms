//! Our own map files (`.rrmap`), written by the map editor: the map data as postcard after a
//! magic and a version, so a map made today still loads tomorrow or is refused clearly.

use rr_core::MapData;

const MAGIC: &[u8; 6] = b"RRMAP\0";
const VERSION: u16 = 1;
/// The biggest map side accepted (also for `.wld`/`.swd`).
pub const MAX_SIDE: u16 = rr_core::MAX_SIDE;

pub fn write(map: &MapData) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    out.extend(VERSION.to_le_bytes());
    out.extend(postcard::to_stdvec(map).expect("serializable"));
    out
}

pub fn read(bytes: &[u8]) -> Result<MapData, String> {
    let body = bytes.strip_prefix(MAGIC.as_slice()).ok_or("not a .rrmap file")?;
    let (v, body) = body.split_at_checked(2).ok_or("truncated .rrmap file")?;
    let v = u16::from_le_bytes([v[0], v[1]]);
    if v != VERSION {
        return Err(format!("map file version {v}, this game reads version {VERSION}"));
    }
    let map: MapData = postcard::from_bytes(body).map_err(|e| e.to_string())?;
    validate(&map)?;
    Ok(map)
}

/// A file can say anything: check what the game relies on (see `MapData::validate`).
pub fn validate(map: &MapData) -> Result<(), String> {
    map.validate()
}

/// Read any map file this game knows: our `.rrmap`, or the original's `.wld`/`.swd`.
pub fn read_any(bytes: &[u8]) -> Result<MapData, String> {
    if bytes.starts_with(MAGIC) {
        read(bytes)
    } else {
        // (a garbage .rrmap was "not a WORLD_V1.0 map": say both; map maker round 1, #6)
        crate::wld::read(bytes).map_err(|e| match e {
            crate::wld::Error::BadMagic => "neither a .rrmap map nor an original WORLD_V1.0 (.wld/.swd) map".into(),
            e => e.to_string(),
        })
    }
}
