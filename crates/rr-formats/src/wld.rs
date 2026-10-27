//! Original map files (`.WLD` / `.SWD`, both "WORLD_V1.0").
//!
//! Layout as documented by libsiedler2 (`ArchivItem_Map*.cpp`): a fixed header, then 14 layer
//! blocks of `width * height` bytes, then a list of extra animal records terminated by 0xFF.

use rr_core::{Animal, Grid, Landscape, MapCoord, MapData, MapObject, Resource};
use std::fmt;

const MAGIC: &[u8; 10] = b"WORLD_V1.0";
const MAX_PLAYERS: usize = 7;
const HEADER_SIG: u16 = 0x2711;
const BLOCK_ID: u16 = 0x2710;
const NUM_LAYERS: usize = 14;
const HARBOR_MASK: u8 = 0x40;

#[derive(Clone, Copy)]
#[repr(usize)]
enum Layer {
    Altitude = 0,
    Terrain1 = 1,
    Terrain2 = 2,
    ObjectIndex = 4,
    ObjectType = 5,
    Animals = 6,
    Resources = 11,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    BadMagic,
    BadHeader(&'static str),
    BadBlock(usize),
    Eof,
    Invalid(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BadMagic => write!(f, "not a WORLD_V1.0 map"),
            Error::BadHeader(why) => write!(f, "bad header: {why}"),
            Error::BadBlock(i) => write!(f, "bad layer block {i}"),
            Error::Eof => write!(f, "unexpected end of file"),
            Error::Invalid(why) => write!(f, "{why}"),
        }
    }
}

impl std::error::Error for Error {}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn bytes(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self.pos.checked_add(n).ok_or(Error::Eof)?;
        let s = self.data.get(self.pos..end).ok_or(Error::Eof)?;
        self.pos = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.bytes(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, Error> {
        let b = self.bytes(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Result<u32, Error> {
        let b = self.bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
}

/// Decode an OEM (code page 437) string up to the first NUL. Non-ASCII maps to the CP437 glyph
/// for the umlauts the German maps use, and to '?' otherwise.
fn oem_string(b: &[u8]) -> String {
    b.iter()
        .take_while(|&&c| c != 0)
        .map(|&c| match c {
            0x20..=0x7e => c as char,
            0x81 => 'ü',
            0x84 => 'ä',
            0x8e => 'Ä',
            0x94 => 'ö',
            0x99 => 'Ö',
            0x9a => 'Ü',
            0xe1 => 'ß',
            _ => '?',
        })
        .collect::<String>()
        .trim_end()
        .to_string()
}

fn oem_bytes(s: &str, len: usize) -> Vec<u8> {
    let mut out: Vec<u8> = s
        .chars()
        .map(|c| match c {
            ' '..='~' => c as u8,
            'ü' => 0x81,
            'ä' => 0x84,
            'Ä' => 0x8e,
            'ö' => 0x94,
            'Ö' => 0x99,
            'Ü' => 0x9a,
            'ß' => 0xe1,
            _ => b'?',
        })
        .take(len - 1)
        .collect();
    out.resize(len, 0);
    out
}

#[allow(clippy::too_many_lines)] // to split (architect round 2, #8: no new long ones)
pub fn read(data: &[u8]) -> Result<MapData, Error> {
    let mut r = Reader { data, pos: 0 };
    if r.bytes(10)? != MAGIC {
        return Err(Error::BadMagic);
    }
    let name_pos = r.pos;
    r.bytes(20)?;
    let (pos_w, pos_h) = (r.u16()?, r.u16()?);
    let gfxset = r.u8()?;
    let _num_players = r.u8()?;
    let author = oem_string(r.bytes(20)?);
    let mut hq_x = [0u16; MAX_PLAYERS];
    let mut hq_y = [0u16; MAX_PLAYERS];
    for x in &mut hq_x {
        *x = r.u16()?;
    }
    for y in &mut hq_y {
        *y = r.u16()?;
    }
    let _invalid = r.u8()?;
    r.bytes(MAX_PLAYERS)?; // player faces
    r.bytes(250 * 9)?; // area infos
    let mut sig = r.u16()?;
    let extra_word = sig == 0;
    if extra_word {
        // bug in some maps ("Das Dreieck"): two extra bytes that belong to the first block
        sig = r.u16()?;
    }
    if sig != HEADER_SIG {
        return Err(Error::BadHeader("signature"));
    }
    r.u32()?;
    let (w, h) = (r.u16()?, r.u16()?);
    // (a 2.6 KB file claiming 65534×65534 nodes asked for 60 GB)
    let max = crate::rrmap::MAX_SIDE;
    if w < 4 || h < 4 || h % 2 == 1 || w > max || h > max {
        return Err(Error::BadHeader("size"));
    }
    let name_len = if (w, h) == (pos_w, pos_h) { 20 } else { 24 };
    let name = oem_string(&data[name_pos..name_pos + name_len]);
    let n = w as usize * h as usize;

    let mut layers: Vec<Vec<u8>> = Vec::with_capacity(NUM_LAYERS);
    for i in 0..NUM_LAYERS {
        let (id, unknown, bw, bh, mult, len) = (r.u16()?, r.u32()?, r.u16()?, r.u16()?, r.u16()?, r.u32()?);
        if id != BLOCK_ID || unknown != 0 {
            return Err(Error::BadBlock(i));
        }
        if mult == 0 {
            layers.push(vec![0; n]);
            continue;
        }
        if len as usize != n || bw != w || bh != h {
            return Err(Error::BadBlock(i));
        }
        let mut layer = r.bytes(n)?.to_vec();
        if i == 0 && extra_word {
            r.pos -= 2;
            let v = layer[n - 3];
            layer[n - 1] = v;
            layer[n - 2] = v;
        }
        layers.push(layer);
    }
    // Extra animal records; tolerate files that simply end here.
    while let Ok(id) = r.u8() {
        if id == 0xff {
            break;
        }
        r.bytes(4)?;
    }

    let grid = Grid::new(w, h);
    let l = |layer: Layer| &layers[layer as usize];
    let mut harbors = Vec::new();
    let mut hq: Vec<Option<MapCoord>> = Vec::new();
    let mut objects = vec![MapObject::None; n];
    let mut resources = vec![Resource::None; n];
    let mut animals = vec![None; n];
    for i in 0..n {
        let p = grid.coord(i);
        if l(Layer::Terrain1)[i] & HARBOR_MASK != 0 {
            harbors.push(p);
        }
        let lc = l(Layer::ObjectIndex)[i];
        objects[i] = match l(Layer::ObjectType)[i] {
            0x80 => {
                let slot = lc as usize;
                if slot < MAX_PLAYERS {
                    if hq.len() <= slot {
                        hq.resize(slot + 1, None);
                    }
                    hq[slot] = Some(p);
                }
                MapObject::None
            }
            t @ 0xc4..=0xc6 => {
                let row = match lc {
                    0x30..=0x3d => Some(0),
                    0x70..=0x7d => Some(1),
                    0xb0..=0xbd => Some(2),
                    0xf0..=0xfd => Some(3),
                    _ => None,
                };
                match row {
                    Some(r) if t != 0xc6 || r == 0 => MapObject::Tree((t - 0xc4) * 4 + r),
                    _ => MapObject::None,
                }
            }
            0xc8 | 0xc9 => MapObject::Decoration(lc),
            t @ (0xcc | 0xcd) if (1..=6).contains(&lc) => MapObject::Granite { kind: t - 0xcc, size: lc },
            _ => MapObject::None,
        };
        let res = l(Layer::Resources)[i];
        resources[i] = match res {
            0x20 | 0x21 => Resource::Water,
            0x41..=0x47 => Resource::Coal(res - 0x40),
            0x49..=0x4f => Resource::Iron(res - 0x48),
            0x51..=0x57 => Resource::Gold(res - 0x50),
            0x59..=0x5f => Resource::Granite(res - 0x58),
            0x81..=0x8f => Resource::Fish(4),
            _ => Resource::None,
        };
        animals[i] = match l(Layer::Animals)[i] {
            1 => Some(Animal::Rabbit),
            2 => Some(Animal::Fox),
            3 => Some(Animal::Stag),
            4 => Some(Animal::Deer),
            5 => Some(Animal::Duck),
            6 => Some(Animal::Sheep),
            _ => None,
        };
    }
    // Fall back to the header positions when the object layer has no HQ markers.
    if hq.iter().all(Option::is_none) {
        hq = (0..MAX_PLAYERS).map(|i| (hq_x[i] < w && hq_y[i] < h).then(|| MapCoord::new(hq_x[i], hq_y[i]))).collect();
        while hq.last() == Some(&None) {
            hq.pop();
        }
    }

    let map = MapData {
        name,
        author,
        grid,
        landscape: match gfxset {
            1 => Landscape::Wasteland,
            2 => Landscape::Winter,
            _ => Landscape::Greenland,
        },
        altitude: l(Layer::Altitude).clone(),
        terrain_rsu: l(Layer::Terrain1).iter().map(|t| t & 0x3f).collect(),
        terrain_usd: l(Layer::Terrain2).iter().map(|t| t & 0x3f).collect(),
        resources,
        objects,
        animals,
        hq,
        harbors,
    };
    crate::rrmap::validate(&map).map_err(Error::Invalid)?;
    Ok(map)
}

pub fn write(m: &MapData) -> Vec<u8> {
    let (w, h) = (m.grid.width, m.grid.height);
    let n = m.grid.len();
    let mut o = Vec::with_capacity(4000 + n * 16);
    let u16 = |o: &mut Vec<u8>, v: u16| o.extend_from_slice(&v.to_le_bytes());
    o.extend_from_slice(MAGIC);
    o.extend_from_slice(&oem_bytes(&m.name, 20));
    u16(&mut o, w);
    u16(&mut o, h);
    o.push(match m.landscape {
        Landscape::Greenland => 0,
        Landscape::Wasteland => 1,
        Landscape::Winter => 2,
    });
    o.push(m.players() as u8);
    o.extend_from_slice(&oem_bytes(&m.author, 20));
    let hq = |i: usize| m.hq.get(i).copied().flatten();
    for i in 0..MAX_PLAYERS {
        u16(&mut o, hq(i).map_or(0xffff, |p| p.x));
    }
    for i in 0..MAX_PLAYERS {
        u16(&mut o, hq(i).map_or(0xffff, |p| p.y));
    }
    o.push(0);
    o.extend_from_slice(&[0; MAX_PLAYERS]);
    o.extend_from_slice(&[0; 250 * 9]);
    u16(&mut o, HEADER_SIG);
    o.extend_from_slice(&0u32.to_le_bytes());
    u16(&mut o, w);
    u16(&mut o, h);

    let mut layers = vec![vec![0u8; n]; NUM_LAYERS];
    layers[Layer::Altitude as usize].clone_from(&m.altitude);
    layers[Layer::Terrain1 as usize].clone_from(&m.terrain_rsu);
    layers[Layer::Terrain2 as usize].clone_from(&m.terrain_usd);
    for p in &m.harbors {
        layers[Layer::Terrain1 as usize][m.grid.index(*p)] |= HARBOR_MASK;
    }
    for i in 0..n {
        let (t, idx) = match m.objects[i] {
            MapObject::None => (0, 0),
            MapObject::Tree(s) => (0xc4 + s / 4, 0x30 + (s % 4) * 0x40),
            MapObject::Granite { kind, size } => (0xcc + kind, size),
            MapObject::Decoration(id) => (0xc8, id),
        };
        layers[Layer::ObjectType as usize][i] = t;
        layers[Layer::ObjectIndex as usize][i] = idx;
        layers[Layer::Resources as usize][i] = match m.resources[i] {
            Resource::None => 0,
            Resource::Water => 0x21,
            Resource::Fish(_) => 0x87,
            Resource::Coal(a) => 0x40 + a,
            Resource::Iron(a) => 0x48 + a,
            Resource::Gold(a) => 0x50 + a,
            Resource::Granite(a) => 0x58 + a,
        };
        layers[Layer::Animals as usize][i] = match m.animals[i] {
            None => 0,
            Some(Animal::Rabbit) => 1,
            Some(Animal::Fox) => 2,
            Some(Animal::Stag) => 3,
            Some(Animal::Deer) => 4,
            Some(Animal::Duck) => 5,
            Some(Animal::Sheep) => 6,
            // (the original's files know no polar bear: the nearest big game)
            Some(Animal::PolarBear) => 3,
        };
    }
    for (slot, p) in m.hq.iter().enumerate() {
        if let Some(p) = p {
            let i = m.grid.index(*p);
            layers[Layer::ObjectType as usize][i] = 0x80;
            layers[Layer::ObjectIndex as usize][i] = slot as u8;
        }
    }
    for layer in &layers {
        u16(&mut o, BLOCK_ID);
        o.extend_from_slice(&0u32.to_le_bytes());
        u16(&mut o, w);
        u16(&mut o, h);
        u16(&mut o, 1);
        o.extend_from_slice(&(n as u32).to_le_bytes());
        o.extend_from_slice(layer);
    }
    o.push(0xff);
    o
}
