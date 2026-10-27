//! Neutral map description: produced by loaders and generators, consumed by `World::new`.

use crate::{Grid, MapCoord};

/// The three original landscapes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub enum Landscape {
    #[default]
    Greenland,
    Wasteland,
    Winter,
}

/// Resource stored in a node (amounts 1..=7 like the original).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub enum Resource {
    #[default]
    None,
    Water,
    Fish(u8),
    Coal(u8),
    Iron(u8),
    Gold(u8),
    Granite(u8),
}

/// Map objects as stored in the map file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub enum MapObject {
    #[default]
    None,
    /// Tree species 0..=8, fully grown.
    Tree(u8),
    /// Granite pile, type 0/1, size 1..=6.
    Granite { kind: u8, size: u8 },
    /// Decoration that blocks building (bush, stone, ruin...). The id is the original file's.
    Decoration(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Animal {
    Rabbit,
    Fox,
    Stag,
    Deer,
    Duck,
    Sheep,
    /// The winter lands' bear (RTTR Species::PolarBear; not in the original's map files).
    PolarBear,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MapData {
    pub name: String,
    pub author: String,
    pub grid: Grid,
    pub landscape: Landscape,
    /// Altitude per node, 0..=60 in practice.
    pub altitude: Vec<u8>,
    /// Original terrain id (lower 6 bits) of the triangle pointing down-right ("RSU") of each node.
    pub terrain_rsu: Vec<u8>,
    /// Original terrain id of the triangle pointing down ("USD") of each node.
    pub terrain_usd: Vec<u8>,
    pub resources: Vec<Resource>,
    pub objects: Vec<MapObject>,
    pub animals: Vec<Option<Animal>>,
    /// Headquarter position per player slot (up to 7).
    pub hq: Vec<Option<MapCoord>>,
    pub harbors: Vec<MapCoord>,
}

/// The biggest map side a game is set up on.
pub const MAX_SIDE: u16 = 1024;

impl MapData {
    /// What every game relies on, whatever made the map (a file, a replay, a generator): a sane
    /// size, layers of that size, headquarters and harbors on the map.
    pub fn validate(&self) -> Result<(), String> {
        let g = self.grid;
        if g.width == 0 || g.height == 0 || !g.height.is_multiple_of(2) || g.width > MAX_SIDE || g.height > MAX_SIDE {
            return Err(format!("a map of {}×{} nodes can't be played", g.width, g.height));
        }
        if self.hq.len() > 7 || self.hq.iter().flatten().chain(self.harbors.iter()).any(|p| !g.contains(*p)) {
            return Err("a headquarters or harbor lies outside the map".into());
        }
        let n = g.len();
        let sizes = [
            self.altitude.len(),
            self.terrain_rsu.len(),
            self.terrain_usd.len(),
            self.resources.len(),
            self.objects.len(),
            self.animals.len(),
        ];
        if sizes.iter().any(|s| *s != n) {
            return Err("map layers don't match the map size".into());
        }
        // the documented ranges, the first bad node named (map maker round 1, #6: altitude 250
        // loaded and drew the map off the screen; an unknown terrain id became lava)
        for p in g.coords() {
            let i = g.index(p);
            let at = format!("at {},{}", p.x, p.y);
            if self.altitude[i] > 100 {
                return Err(format!("altitude {} {at} (0..=60; up to 100 is accepted)", self.altitude[i]));
            }
            for t in [self.terrain_rsu[i], self.terrain_usd[i]] {
                if t > 34 {
                    return Err(format!("unknown terrain id {t} {at} (0..=34)"));
                }
            }
            match self.objects[i] {
                MapObject::Tree(s) if s > 8 => return Err(format!("tree species {s} {at} (0..=8)")),
                MapObject::Granite { size, .. } if !(1..=6).contains(&size) => {
                    return Err(format!("granite size {size} {at} (1..=6)"));
                }
                _ => {}
            }
            if let Resource::Fish(n) | Resource::Coal(n) | Resource::Iron(n) | Resource::Gold(n) | Resource::Granite(n) =
                self.resources[i]
                && n > 7
            {
                return Err(format!("resource amount {n} {at} (1..=7)"));
            }
        }
        Ok(())
    }

    /// A flat meadow map, handy for tests and generators.
    pub fn flat(width: u16, height: u16, terrain: u8) -> Self {
        let grid = Grid::new(width, height);
        let n = grid.len();
        Self {
            name: String::new(),
            author: String::new(),
            grid,
            landscape: Landscape::Greenland,
            altitude: vec![10; n],
            terrain_rsu: vec![terrain; n],
            terrain_usd: vec![terrain; n],
            resources: vec![Resource::None; n],
            objects: vec![MapObject::None; n],
            animals: vec![None; n],
            hq: Vec::new(),
            harbors: Vec::new(),
        }
    }

    pub fn players(&self) -> usize {
        self.hq.iter().filter(|h| h.is_some()).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Map maker round 1, #11: every animal kind is in MAPFORMAT's table.
    #[test]
    fn mapformat_names_every_animal() {
        let doc = include_str!("../../../docs/MAPFORMAT.md");
        let line = doc.lines().find(|l| l.starts_with("| animals |")).expect("the animals row");
        for (i, name) in ["rabbit", "fox", "stag", "deer", "duck", "sheep", "polar bear"].iter().enumerate() {
            assert!(line.contains(&format!("{i} {name}")), "{i} {name}: {line}");
        }
        // (and the list above is the enum's, in order)
        let all =
            [Animal::Rabbit, Animal::Fox, Animal::Stag, Animal::Deer, Animal::Duck, Animal::Sheep, Animal::PolarBear];
        assert_eq!(all.iter().map(|a| *a as usize).collect::<Vec<_>>(), (0..7).collect::<Vec<_>>());
        match Animal::Rabbit {
            Animal::Rabbit
            | Animal::Fox
            | Animal::Stag
            | Animal::Deer
            | Animal::Duck
            | Animal::Sheep
            | Animal::PolarBear => {}
        }
    }

    /// Map maker round 1, #6: values outside the documented ranges are refused, naming the field
    /// and the node (altitude 250 loaded and drew the map off the screen).
    #[test]
    fn out_of_range_values_are_named() {
        let ok = MapData::flat(16, 16, 8);
        assert!(ok.validate().is_ok());
        let at = |m: &MapData| m.grid.index(MapCoord::new(3, 4));
        let mut m = ok.clone();
        let i = at(&m);
        m.altitude[i] = 250;
        assert_eq!(m.validate().unwrap_err(), "altitude 250 at 3,4 (0..=60; up to 100 is accepted)");
        let mut m = ok.clone();
        m.terrain_usd[i] = 200;
        assert_eq!(m.validate().unwrap_err(), "unknown terrain id 200 at 3,4 (0..=34)");
        let mut m = ok.clone();
        m.objects[i] = MapObject::Granite { kind: 0, size: 9 };
        assert!(m.validate().unwrap_err().starts_with("granite size 9 at 3,4"));
        let mut m = ok.clone();
        m.objects[i] = MapObject::Tree(12);
        assert!(m.validate().unwrap_err().starts_with("tree species 12 at 3,4"));
        let mut m = ok;
        m.resources[i] = Resource::Gold(20);
        assert!(m.validate().unwrap_err().starts_with("resource amount 20 at 3,4"));
    }
}
