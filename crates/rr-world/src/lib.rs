//! The map as a set of node layers: terrain, altitude, objects, roads, ownership, and the
//! building-quality (BQ) rules. Knows nothing about economy or units.

pub mod terrain;

use rr_core::{Direction, Grid, Landscape, MapCoord, MapData, MapObject, Resource};
use terrain::TerrainBq;

/// Building quality of a node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Bq {
    Nothing,
    Flag,
    Hut,
    House,
    Castle,
    Mine,
    Harbor,
}

impl Bq {
    /// Can a building of size `want` be placed where this BQ was computed?
    pub fn allows(self, want: Bq) -> bool {
        use Bq::*;
        match want {
            Nothing => true,
            Flag => self != Nothing,
            Hut => matches!(self, Hut | House | Castle | Harbor),
            House => matches!(self, House | Castle | Harbor),
            Castle => matches!(self, Castle | Harbor),
            Mine => self == Mine,
            Harbor => self == Harbor,
        }
    }
}

/// One of the two triangles owned by a node: RSU points up (right of and below the node),
/// USD points down (below the node).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Tri {
    Rsu,
    Usd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub enum RoadKind {
    #[default]
    None,
    Normal,
    Donkey,
    Boat,
}

/// What occupies a node. Ids are opaque handles owned by the simulation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub enum NodeObj {
    #[default]
    None,
    Tree {
        species: u8,
        /// 0..=2 growing, 3 grown.
        stage: u8,
    },
    Granite {
        kind: u8,
        size: u8,
    },
    /// A decoration from the map file; some block (ruins, stalagmites), most don't (bushes).
    Decoration(u8),
    /// A grain field (stage 0..=3), blocks like a stone.
    Field(u8),
    Flag(u32),
    Building(u32),
    /// The extra nodes a large building occupies (W, NW, NE of the building node).
    Extension,
    /// Burning or burnt building; blocks like a stone.
    Fire,
    /// A geologist's sign (resource code), purely informative.
    Sign(u8),
    /// A ship being built on the shore (progress 0..9).
    ShipSite(u8),
}

/// How an object influences building around it (RTTR `BlockingManner`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blocking {
    None,
    Single,
    Building,
    Flag,
    Tree,
    FlagsAround,
}

impl NodeObj {
    pub fn blocking(self) -> Blocking {
        match self {
            NodeObj::None | NodeObj::Sign(_) => Blocking::None,
            NodeObj::Tree { .. } => Blocking::Tree,
            NodeObj::Granite { .. } | NodeObj::Field(_) | NodeObj::Fire => Blocking::FlagsAround,
            NodeObj::Decoration(id) => {
                if decoration_is_passable(id) {
                    Blocking::None
                } else {
                    Blocking::Single
                }
            }
            NodeObj::Flag(_) => Blocking::Flag,
            NodeObj::Building(_) => Blocking::Building,
            NodeObj::Extension | NodeObj::ShipSite(_) => Blocking::Single,
        }
    }
}

/// The Roman campaign's holy places (docs/CAMPAIGN-ROME.md) are map decorations from this id
/// on, one per god (`HOLY_PLACES`). They block like a rock, and nothing in the game removes
/// them: no fire, no border, no builder. (Kept as decorations, so maps and saves keep their
/// formats.)
pub const HOLY_BASE: u8 = 0xf0;

/// The holy places by god, in id order: their pictures' names (`assets/art/holy`).
pub const HOLY_PLACES: [&str; 9] =
    ["jupiter", "concordia", "juno_veii", "juno_capitol", "janus", "mars", "neptune", "cybele", "carthage_temple"];

/// The god of a holy place's decoration, if it is one (an index into `HOLY_PLACES`).
pub fn holy_god(o: NodeObj) -> Option<u8> {
    match o {
        NodeObj::Decoration(id) if id >= HOLY_BASE && ((id - HOLY_BASE) as usize) < HOLY_PLACES.len() => {
            Some(id - HOLY_BASE)
        }
        _ => None,
    }
}

/// Who may use a holy place: the one whose land lies fully around it (owner, 2026-10-27).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Holder {
    Nobody,
    /// The border runs through its ring: nobody may use it until one side pushes the other back.
    Contested,
    Player(u8),
}

/// How far around a holy place the land must be one player's (2: no border line can run
/// through it).
pub const HOLY_RING: u16 = 2;

/// Bushes, flowers and small stones from the original object lists don't block.
fn decoration_is_passable(id: u8) -> bool {
    (id <= 0x0f && id != 0x0b) || (0x10..=0x14).contains(&id) || (0x22..=0x2b).contains(&id)
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct World {
    grid: Grid,
    pub landscape: Landscape,
    altitude: Vec<u8>,
    t_rsu: Vec<u8>,
    t_usd: Vec<u8>,
    pub resources: Vec<Resource>,
    obj: Vec<NodeObj>,
    /// Roads leaving a node towards E, SE, SW (the other three are stored at the neighbor).
    roads: Vec<[RoadKind; 3]>,
    /// Owning player + 1, or 0.
    owner: Vec<u8>,
    harbor: Vec<bool>,
}

impl World {
    /// Do all layers fit the grid? (A loaded save is checked with this.)
    pub fn layers_ok(&self) -> bool {
        let n = self.grid.len();
        self.grid.width > 0
            && self.grid.height.is_multiple_of(2)
            && [self.altitude.len(), self.t_rsu.len(), self.t_usd.len(), self.resources.len(), self.obj.len()]
                .iter()
                .chain([self.roads.len(), self.owner.len(), self.harbor.len()].iter())
                .all(|l| *l == n)
    }

    pub fn new(m: &MapData) -> Self {
        let n = m.grid.len();
        let mut harbor = vec![false; n];
        for p in &m.harbors {
            harbor[m.grid.index(*p)] = true;
        }
        let obj = m
            .objects
            .iter()
            .map(|o| match *o {
                MapObject::None => NodeObj::None,
                MapObject::Tree(species) => NodeObj::Tree { species, stage: 3 },
                MapObject::Granite { kind, size } => NodeObj::Granite { kind, size },
                MapObject::Decoration(id) => NodeObj::Decoration(id),
            })
            .collect();
        Self {
            grid: m.grid,
            landscape: m.landscape,
            altitude: m.altitude.clone(),
            t_rsu: m.terrain_rsu.clone(),
            t_usd: m.terrain_usd.clone(),
            resources: m.resources.clone(),
            obj,
            roads: vec![[RoadKind::None; 3]; n],
            owner: vec![0; n],
            harbor,
        }
    }

    pub fn grid(&self) -> Grid {
        self.grid
    }

    fn i(&self, p: MapCoord) -> usize {
        self.grid.index(p)
    }

    pub fn altitude(&self, p: MapCoord) -> u8 {
        self.altitude[self.i(p)]
    }

    pub fn set_altitude(&mut self, p: MapCoord, a: u8) {
        let i = self.i(p);
        self.altitude[i] = a;
    }

    pub fn terrain(&self, p: MapCoord, t: Tri) -> u8 {
        match t {
            Tri::Rsu => self.t_rsu[self.i(p)],
            Tri::Usd => self.t_usd[self.i(p)],
        }
    }

    pub fn set_terrain(&mut self, p: MapCoord, t: Tri, v: u8) {
        let i = self.i(p);
        match t {
            Tri::Rsu => self.t_rsu[i] = v,
            Tri::Usd => self.t_usd[i] = v,
        }
    }

    /// The six triangles touching a node.
    pub fn triangles_around(&self, p: MapCoord) -> [(MapCoord, Tri); 6] {
        let g = self.grid;
        let nw = g.neighbor(p, Direction::NW);
        let ne = g.neighbor(p, Direction::NE);
        let w = g.neighbor(p, Direction::W);
        [(nw, Tri::Rsu), (nw, Tri::Usd), (ne, Tri::Rsu), (p, Tri::Usd), (p, Tri::Rsu), (w, Tri::Usd)]
    }

    pub fn terrains_around(&self, p: MapCoord) -> [u8; 6] {
        self.triangles_around(p).map(|(q, t)| self.terrain(q, t))
    }

    /// The two triangles left and right of the edge from `p` in direction `d`.
    pub fn edge_terrains(&self, p: MapCoord, d: Direction) -> [u8; 2] {
        let g = self.grid;
        let n = |d| g.neighbor(p, d);
        use Tri::*;
        let pair = match d {
            Direction::W => [(n(Direction::W), Usd), (n(Direction::NW), Rsu)],
            Direction::NW => [(n(Direction::NW), Rsu), (n(Direction::NW), Usd)],
            Direction::NE => [(n(Direction::NW), Usd), (n(Direction::NE), Rsu)],
            Direction::E => [(n(Direction::NE), Rsu), (p, Usd)],
            Direction::SE => [(p, Usd), (p, Rsu)],
            Direction::SW => [(p, Rsu), (n(Direction::W), Usd)],
        };
        pair.map(|(q, t)| self.terrain(q, t))
    }

    /// Can a figure stand on this node?
    pub fn node_walkable(&self, p: MapCoord) -> bool {
        let ts = self.terrains_around(p);
        !ts.iter().any(|&t| terrain::is_danger(t)) && ts.iter().any(|&t| terrain::is_walkable(t))
    }

    /// Can a figure walk along this edge (terrain only)?
    pub fn edge_walkable(&self, p: MapCoord, d: Direction) -> bool {
        let [a, b] = self.edge_terrains(p, d);
        !terrain::is_danger(a) && !terrain::is_danger(b) && (terrain::is_walkable(a) || terrain::is_walkable(b))
    }

    pub fn obj(&self, p: MapCoord) -> NodeObj {
        self.obj[self.i(p)]
    }

    pub fn set_obj(&mut self, p: MapCoord, o: NodeObj) {
        let i = self.i(p);
        self.obj[i] = o;
    }

    pub fn is_harbor_spot(&self, p: MapCoord) -> bool {
        self.harbor[self.i(p)]
    }

    /// Every harbor spot of the map.
    pub fn harbor_spots(&self) -> Vec<MapCoord> {
        (0..self.harbor.len()).filter(|i| self.harbor[*i]).map(|i| self.grid.coord(i)).collect()
    }

    pub fn road(&self, p: MapCoord, d: Direction) -> RoadKind {
        match d {
            Direction::E => self.roads[self.i(p)][0],
            Direction::SE => self.roads[self.i(p)][1],
            Direction::SW => self.roads[self.i(p)][2],
            _ => {
                let q = self.grid.neighbor(p, d);
                self.road(q, d.opposite())
            }
        }
    }

    pub fn set_road(&mut self, p: MapCoord, d: Direction, k: RoadKind) {
        let i = self.i(p);
        match d {
            Direction::E => self.roads[i][0] = k,
            Direction::SE => self.roads[i][1] = k,
            Direction::SW => self.roads[i][2] = k,
            _ => {
                let q = self.grid.neighbor(p, d);
                self.set_road(q, d.opposite(), k)
            }
        }
    }

    pub fn is_on_road(&self, p: MapCoord) -> bool {
        Direction::ALL.iter().any(|&d| self.road(p, d) != RoadKind::None)
    }

    pub fn owner(&self, p: MapCoord) -> Option<u8> {
        self.owner[self.i(p)].checked_sub(1)
    }

    /// Is every node owned by a player below `count` (or by nobody)? The layer stores player + 1,
    /// so a loaded save has to pass this before its land is counted per player.
    pub fn owners_below(&self, count: usize) -> bool {
        self.owner.iter().all(|&o| o as usize <= count)
    }

    pub fn set_owner(&mut self, p: MapCoord, player: Option<u8>) {
        let i = self.i(p);
        self.owner[i] = player.map_or(0, |pl| pl + 1);
    }

    /// The node and all its neighbors belong to `player` (so border nodes are excluded).
    pub fn is_inner_territory(&self, p: MapCoord, player: u8) -> bool {
        self.owner(p) == Some(player) && self.grid.neighbors(p).iter().all(|&q| self.owner(q) == Some(player))
    }

    fn uniform_owner(&self, p: MapCoord) -> bool {
        let o = self.owner(p);
        self.grid.neighbors(p).iter().all(|&q| self.owner(q) == o)
    }

    /// BQ of a node ignoring ownership.
    pub fn bq(&self, p: MapCoord) -> Bq {
        self.bq_impl(p, false)
    }

    /// Who holds the holy place at `p`: the player who owns `p` and every node within
    /// `HOLY_RING`, else contested where two own parts of it, else nobody.
    pub fn holy_holder(&self, p: MapCoord) -> Holder {
        let mut owner = None;
        let mut whole = true;
        for q in self.grid.within(p, HOLY_RING) {
            match (self.owner(q), owner) {
                (None, _) => whole = false,
                (Some(o), None) => owner = Some(o),
                (Some(o), Some(first)) if o != first => return Holder::Contested,
                _ => {}
            }
        }
        match owner {
            Some(o) if whole => Holder::Player(o),
            _ => Holder::Nobody,
        }
    }

    /// The holy places on the map: where, and whose god (an index into `HOLY_PLACES`).
    pub fn holy_places(&self) -> Vec<(MapCoord, u8)> {
        self.grid.coords().filter_map(|p| holy_god(self.obj(p)).map(|g| (p, g))).collect()
    }

    /// BQ for a player: only on own inner territory; a building whose flag would be on the
    /// border degrades to a flag.
    pub fn bq_for(&self, p: MapCoord, player: u8) -> Bq {
        let bq = self.bq(p);
        if bq == Bq::Nothing || !self.is_inner_territory(p, player) {
            return Bq::Nothing;
        }
        let se = self.grid.neighbor(p, Direction::SE);
        if bq != Bq::Flag && !self.uniform_owner(se) {
            for d in [Direction::W, Direction::NW, Direction::NE] {
                if self.obj(self.grid.neighbor(p, d)).blocking() == Blocking::Flag {
                    return Bq::Nothing;
                }
            }
            return Bq::Flag;
        }
        bq
    }

    /// Port of RTTR `BQCalculator` (itself modeled on the original).
    fn bq_impl(&self, p: MapCoord, flag_only: bool) -> Bq {
        if self.obj(p).blocking() != Blocking::None {
            return Bq::Nothing;
        }
        let g = self.grid;
        // 1. terrain
        let mut cur;
        let ts = self.terrains_around(p);
        if flag_only {
            if ts.iter().any(|&t| terrain::bq(t) == TerrainBq::Danger) {
                return Bq::Nothing;
            }
            if !ts.iter().any(|&t| terrain::bq(t) != TerrainBq::Nothing) {
                return Bq::Nothing;
            }
            cur = Bq::Flag;
        } else {
            let (mut build, mut mine, mut flag) = (0, 0, 0);
            for &t in &ts {
                match terrain::bq(t) {
                    TerrainBq::Castle => build += 1,
                    TerrainBq::Mine => mine += 1,
                    TerrainBq::Flag => flag += 1,
                    TerrainBq::Danger => return Bq::Nothing,
                    TerrainBq::Nothing => {}
                }
            }
            cur = if mine == 6 {
                Bq::Mine
            } else if build == 6 {
                Bq::Castle
            } else if flag + mine + build > 0 {
                Bq::Flag
            } else {
                return Bq::Nothing;
            };
        }
        let nb = g.neighbors(p);
        let alt = self.altitude(p) as i32;
        let alt_of = |q: MapCoord| self.altitude(q) as i32;
        let se = nb[Direction::SE.index() as usize];

        // 2. altitude
        if cur == Bq::Castle {
            if alt_of(se) > alt + 1 || nb.iter().any(|&q| (alt_of(q) - alt).abs() > 3) {
                cur = Bq::Flag;
            } else if g.ring(p, 2).any(|q| (alt_of(q) - alt).abs() > 2) {
                cur = Bq::Hut;
            }
        } else if cur == Bq::Mine && alt_of(se) > alt + 3 {
            cur = Bq::Flag;
        }

        // 3. neighboring objects
        let blocks = nb.map(|q| self.obj(q).blocking());
        let has = |b: Blocking| blocks.contains(&b);
        if flag_only {
            return if has(Blocking::Flag) { Bq::Nothing } else { Bq::Flag };
        }
        if blocks[Direction::E.index() as usize] == Blocking::Flag
            || blocks[Direction::SW.index() as usize] == Blocking::Flag
        {
            return Bq::Nothing;
        }

        // 4. nearby objects reduce
        let ext = [Direction::W, Direction::NW, Direction::NE];
        if has(Blocking::FlagsAround) {
            cur = Bq::Flag;
        } else if matches!(cur, Bq::House | Bq::Castle) && has(Blocking::Tree) {
            cur = Bq::Hut;
        } else if cur == Bq::Castle && ext.iter().any(|d| blocks[d.index() as usize] != Blocking::None) {
            cur = Bq::House;
        }
        if cur == Bq::Castle && g.ring(p, 2).any(|q| self.obj(q).blocking() == Blocking::Building) {
            cur = Bq::House;
        }
        if cur == Bq::Castle && ext.iter().any(|&d| self.is_on_road(nb[d.index() as usize])) {
            cur = Bq::House;
        }
        if cur != Bq::Flag && self.is_on_road(p) {
            cur = Bq::Flag;
        }
        if cur == Bq::Flag {
            return if has(Blocking::Flag) { Bq::Nothing } else { Bq::Flag };
        }
        if cur == Bq::Castle && self.is_harbor_spot(p) {
            cur = Bq::Harbor;
        }
        // a building needs its flag at SE
        if blocks[Direction::SE.index() as usize] == Blocking::Flag || self.bq_impl(se, true) != Bq::Nothing {
            return cur;
        }
        if has(Blocking::Flag) { Bq::Nothing } else { Bq::Flag }
    }
}
