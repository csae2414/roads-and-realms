//! Procedural maps: continents with meadows, forests, mountains (with resources) and lakes,
//! and fair start positions. Fully deterministic from the seed.

use rr_core::{Animal, Direction, Grid, Landscape, MapCoord, MapData, MapObject, Pcg32, Resource};

pub mod europe;
pub mod region;
pub mod rome;
pub mod world;

#[derive(Clone, Debug)]
pub struct GenParams {
    pub width: u16,
    pub height: u16,
    pub players: usize,
    pub seed: u64,
    /// Share of the map that is water, in percent.
    pub water: u32,
    /// Share of mountains, in percent.
    pub mountains: u32,
    pub landscape: Landscape,
    /// An archipelago: every realm on its own island, uninhabited islands with ore and forest
    /// between them, open sea around (owner, 2026-10-27: "make harbors meaningful").
    pub islands: bool,
    /// A map of the Roman campaign: the seven hills by a river, two castles on fixed hills
    /// (`rome::seven_hills`; the size and the player count stay the caller's).
    pub rome: Option<rome::RomeMap>,
}

impl Default for GenParams {
    fn default() -> Self {
        Self {
            width: 128,
            height: 128,
            players: 2,
            seed: 1,
            water: 22,
            mountains: 14,
            landscape: Landscape::Greenland,
            islands: false,
            rome: None,
        }
    }
}

pub const T_MEADOW: [u8; 3] = [8, 9, 10];
pub const T_STEPPE: u8 = 14;
pub const T_FLOWERS: u8 = 15;
pub const T_SAVANNAH: u8 = 0;
pub const T_MOUNTAIN: [u8; 4] = [1, 11, 12, 13];
pub const T_MOUNTAIN_MEADOW: u8 = 18;
pub const T_WATER: u8 = 5;
pub const T_SNOW: u8 = 2;
pub const T_DESERT: u8 = 4;

/// Periodic value noise on the torus, so the wrapping map has no seams.
pub struct Noise {
    seed: u64,
    w: u32,
    h: u32,
}

fn hash3(seed: u64, a: u32, b: u32, c: u32) -> u32 {
    let mut z = seed
        ^ (a as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ ((b as u64) << 32 | c as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    (z ^ (z >> 31)) as u32
}

impl Noise {
    pub fn new(seed: u64, w: u32, h: u32) -> Self {
        Self { seed, w, h }
    }

    fn lattice(&self, cells_x: u32, cells_y: u32, ix: i64, iy: i64, oct: u32) -> f32 {
        let x = ix.rem_euclid(cells_x as i64) as u32;
        let y = iy.rem_euclid(cells_y as i64) as u32;
        (hash3(self.seed, x, y, oct) & 0xffff) as f32 / 65535.0
    }

    /// Value in 0..1 at map position (x, y); `cells` lattice cells across the map.
    fn octave(&self, x: f32, y: f32, cells: u32, oct: u32) -> f32 {
        let cx = cells.max(1);
        let cy = ((cells as f32 * self.h as f32 / self.w as f32).round() as u32).max(1);
        let fx = x / self.w as f32 * cx as f32;
        let fy = y / self.h as f32 * cy as f32;
        let (ix, iy) = (fx.floor() as i64, fy.floor() as i64);
        let (tx, ty) = (fx - ix as f32, fy - iy as f32);
        let s = |t: f32| t * t * (3.0 - 2.0 * t);
        let (sx, sy) = (s(tx), s(ty));
        let v00 = self.lattice(cx, cy, ix, iy, oct);
        let v10 = self.lattice(cx, cy, ix + 1, iy, oct);
        let v01 = self.lattice(cx, cy, ix, iy + 1, oct);
        let v11 = self.lattice(cx, cy, ix + 1, iy + 1, oct);
        let a = v00 + (v10 - v00) * sx;
        let b = v01 + (v11 - v01) * sx;
        a + (b - a) * sy
    }

    /// Fractal sum, normalized to 0..1.
    pub fn fbm(&self, x: f32, y: f32, base_cells: u32, octaves: u32, salt: u32) -> f32 {
        let mut sum = 0.0;
        let mut amp = 1.0;
        let mut norm = 0.0;
        let mut cells = base_cells;
        for o in 0..octaves {
            sum += self.octave(x, y, cells, o * 31 + salt) * amp;
            norm += amp;
            amp *= 0.5;
            cells *= 2;
        }
        sum / norm
    }
}

/// Screen-ish position of a node (odd rows shifted half a node right).
pub(crate) fn fpos(p: MapCoord) -> (f32, f32) {
    (p.x as f32 + if p.y & 1 == 1 { 0.5 } else { 0.0 }, p.y as f32)
}

#[allow(clippy::too_many_lines)] // to split (architect round 2, #8: no new long ones)
pub fn generate(p: &GenParams) -> MapData {
    let grid = Grid::new(p.width, p.height);
    let n = grid.len();
    let noise = Noise::new(p.seed, p.width as u32, p.height as u32);
    let mut rng = Pcg32::new(p.seed, 0x6d61_7067);

    // 1. elevation 0..1
    let cells = (p.width as u32 / 24).max(2);
    let mut elev: Vec<f32> = (0..n)
        .map(|i| {
            let (x, y) = fpos(grid.coord(i));
            noise.fbm(x, y, cells, 5, 1)
        })
        .collect();
    // normalize by rank so the water/mountain shares hold for any seed
    let mut sorted = elev.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let q = |pct: u32| sorted[((n as u64 * pct as u64 / 100) as usize).min(n - 1)];
    let sea = q(p.water);
    let mount = q(100 - p.mountains);
    for e in elev.iter_mut() {
        *e = if *e < sea {
            (*e - sorted[0]) / (sea - sorted[0]).max(1e-6) * 0.3
        } else if *e < mount {
            0.3 + (*e - sea) / (mount - sea).max(1e-6) * 0.4
        } else {
            0.7 + (*e - mount) / (sorted[n - 1] - mount).max(1e-6) * 0.3
        };
    }

    // 2. start positions: land, not mountain, far apart (or the seven hills' own)
    let hq = start_positions(p, &grid, &mut elev, &mut rng);
    islands_mask(p, &grid, &mut elev, &hq, &mut rng);
    // 3. stamp a fair start area per player: flat meadow, a mountain, a lake
    for (i, h) in hq.iter().enumerate() {
        // (a castle on one of the seven hills keeps its hill: a small plateau at its top)
        let (base, r) = if p.rome.is_some() { (elev[grid.index(*h)], 4.0) } else { (0.45, 9.0) };
        for c in grid.within(*h, r as u16) {
            let d = grid.distance(*h, c) as f32;
            let k = grid.index(c);
            let t = (d / r).clamp(0.0, 1.0);
            elev[k] = base + (elev[k] - base) * t * t;
        }
        // (the seven hills have no start mountain or lake: their layout is fixed)
        if p.rome.is_some() {
            continue;
        }
        let dir = Direction::from_index((i as u8 * 2 + rng.below(6) as u8) % 6);
        let m = grid.walk(*h, dir, if p.islands { 7 } else { 13 });
        for c in grid.within(m, 4) {
            let d = grid.distance(m, c) as f32;
            let k = grid.index(c);
            elev[k] = elev[k].max(0.76 - d * 0.015);
        }
        let lake = grid.walk(*h, dir.rotate_cw(3), 12);
        for c in grid.within(lake, 2) {
            let k = grid.index(c);
            elev[k] = elev[k].min(0.2);
        }
    }

    // 4. terrain per triangle from the mean elevation of its corners
    let vary = |c: MapCoord, salt: u32| {
        let (x, y) = fpos(c);
        noise.fbm(x, y, cells * 3, 2, salt)
    };
    let tri_terrain = |a: MapCoord, b: MapCoord, c: MapCoord| -> u8 {
        let e = (elev[grid.index(a)] + elev[grid.index(b)] + elev[grid.index(c)]) / 3.0;
        let v = vary(a, 7);
        if e < 0.3 {
            T_WATER
        } else if e < 0.7 {
            let dry = vary(a, 13);
            if e < 0.32 && dry > 0.6 {
                T_DESERT
            } else if dry > 0.68 && p.landscape == Landscape::Greenland {
                // (terrain 14 is a lush green in the wasteland and winter sets: greenland only)
                T_STEPPE
            } else if dry < 0.25 {
                T_FLOWERS
            } else if e > 0.66 {
                T_MOUNTAIN_MEADOW
            } else {
                T_MEADOW[(v * 2.99) as usize]
            }
        } else if e > 0.93 {
            T_SNOW
        } else {
            T_MOUNTAIN[((e - 0.7) / 0.23 * 3.99).min(3.0) as usize]
        }
    };
    let mut terrain_rsu = vec![0u8; n];
    let mut terrain_usd = vec![0u8; n];
    for c in grid.coords() {
        let i = grid.index(c);
        let sw = grid.neighbor(c, Direction::SW);
        let se = grid.neighbor(c, Direction::SE);
        let e = grid.neighbor(c, Direction::E);
        terrain_rsu[i] = tri_terrain(c, sw, se);
        terrain_usd[i] = tri_terrain(c, e, se);
    }

    // 5. altitude: smooth, flat around HQs
    let mut altitude: Vec<u8> = (0..n)
        .map(|i| {
            let e = elev[i];
            // rolling hills on land: fine noise on top of the large-scale elevation
            let (x, y) = fpos(grid.coord(i));
            let hills = (noise.fbm(x, y, cells * 6, 3, 77) - 0.5) * 9.0;
            let a = if e < 0.3 {
                8.0
            } else if e < 0.7 {
                10.0 + (e - 0.3) * 30.0 + hills
            } else {
                22.0 + (e - 0.7) * 90.0 + hills * 0.6
            };
            a.round().max(8.0) as u8
        })
        .collect();
    for _ in 0..2 {
        let prev = altitude.clone();
        for c in grid.coords() {
            let s: u32 = grid.neighbors(c).iter().map(|q| prev[grid.index(*q)] as u32).sum::<u32>()
                + prev[grid.index(c)] as u32 * 2;
            altitude[grid.index(c)] = (s / 8) as u8;
        }
    }
    for h in &hq {
        let a = altitude[grid.index(*h)];
        for c in grid.within(*h, 3) {
            altitude[grid.index(c)] = a;
        }
    }

    // 6. resources, objects, animals
    let mut resources = vec![Resource::None; n];
    let mut objects = vec![MapObject::None; n];
    let mut animals = vec![None; n];
    let all_of = |c: MapCoord, f: &dyn Fn(u8) -> bool| {
        let nw = grid.neighbor(c, Direction::NW);
        let ne = grid.neighbor(c, Direction::NE);
        let w = grid.neighbor(c, Direction::W);
        [
            terrain_rsu[grid.index(nw)],
            terrain_usd[grid.index(nw)],
            terrain_rsu[grid.index(ne)],
            terrain_usd[grid.index(c)],
            terrain_rsu[grid.index(c)],
            terrain_usd[grid.index(w)],
        ]
        .iter()
        .all(|t| f(*t))
    };
    let is_mtn = |t: u8| T_MOUNTAIN.contains(&t);
    let is_land = |t: u8| T_MEADOW.contains(&t) || [T_STEPPE, T_FLOWERS, T_SAVANNAH, T_MOUNTAIN_MEADOW].contains(&t);
    let is_water = |t: u8| t == T_WATER;
    for c in grid.coords() {
        let i = grid.index(c);
        if all_of(c, &is_mtn) {
            let kind = vary(c, 101);
            let amount = 1 + (vary(c, 202) * 7.0).min(6.0) as u8;
            resources[i] = if kind < 0.42 {
                Resource::Coal(amount)
            } else if kind < 0.68 {
                Resource::Iron(amount)
            } else if kind < 0.84 {
                Resource::Gold(amount)
            } else {
                Resource::Granite(amount)
            };
        } else if all_of(c, &is_water) {
            let shore = grid.within(c, 2).any(|q| !all_of(q, &is_water));
            if shore {
                resources[i] = Resource::Fish(4);
            }
        } else if all_of(c, &is_land) {
            resources[i] = Resource::Water;
            let near_hq = hq.iter().any(|h| grid.distance(*h, c) <= 4);
            if near_hq {
                continue;
            }
            let forest = vary(c, 303);
            let r = rng.below(1000);
            if forest > 0.62 && r < 550 || r < 25 {
                let species = rng.below(if p.landscape == Landscape::Greenland { 6 } else { 9 }) as u8;
                objects[i] = MapObject::Tree(species);
            } else if vary(c, 404) > 0.72 && r < 400 || r < 12 {
                objects[i] = MapObject::Granite { kind: (r % 2) as u8, size: 2 + (r % 5) as u8 };
            } else if r > 994 {
                animals[i] =
                    Some([Animal::Rabbit, Animal::Fox, Animal::Stag, Animal::Deer, Animal::Sheep][(r % 5) as usize]);
            } else if r > 990 {
                objects[i] = MapObject::Decoration((r % 10) as u8);
            }
        }
    }
    let mut map = MapData {
        name: format!("Generated {}", p.seed),
        author: "rr-mapgen".into(),
        grid,
        landscape: p.landscape,
        altitude,
        terrain_rsu,
        terrain_usd,
        resources,
        objects,
        animals,
        hq: hq.into_iter().map(Some).collect(),
        harbors: Vec::new(),
    };
    place_harbors(&mut map);
    start_resources(&mut map);
    map
}

/// The castles: on land, not mountain, far apart; the first at random (or the seven hills').
fn start_positions(p: &GenParams, grid: &Grid, elev: &mut [f32], rng: &mut Pcg32) -> Vec<MapCoord> {
    // (the seven hills shape the land and place the castles themselves)
    if p.rome.is_some() {
        return rome::seven_hills(p, grid, elev);
    }
    let mut hq: Vec<MapCoord> = Vec::new();
    let cands: Vec<MapCoord> =
        grid.coords().filter(|c| (0.38..0.6).contains(&elev[grid.index(*c)]) && c.y % 2 == 0).collect();
    for i in 0..p.players {
        let pick = if i == 0 {
            cands[rng.below(cands.len() as u32) as usize]
        } else {
            *cands
                .iter()
                .max_by_key(|c| (hq.iter().map(|h| grid.distance(**c, *h)).min().unwrap_or(0), c.y, c.x))
                .unwrap()
        };
        hq.push(pick);
    }
    hq
}

/// Home islands' radius: a third of the castles' spacing, 11 to 20 nodes.
fn island_radius(grid: &Grid, hq: &[MapCoord]) -> f32 {
    let spacing = hq
        .iter()
        .enumerate()
        .flat_map(|(i, a)| hq[i + 1..].iter().map(move |b| grid.distance(*a, *b)))
        .min()
        .unwrap_or(grid.width as u32 / 2);
    (spacing as f32 * 0.33).clamp(11.0, 20.0)
}

/// For [`GenParams::islands`] (then each castle's mountain stands 7 nodes out, not 13, which
/// would lie in the sea): land only on a home island round each castle and on a few
/// uninhabited islands in the open sea between them (every other one with a mountain core, for
/// the ore), sea everywhere else; the coasts frayed by noise.
fn islands_mask(p: &GenParams, grid: &Grid, elev: &mut [f32], hq: &[MapCoord], rng: &mut Pcg32) {
    if !p.islands {
        return;
    }
    let noise = Noise::new(p.seed ^ 0x1515, p.width as u32, p.height as u32);
    let r_home = island_radius(grid, hq);
    // uninhabited islands: far from every castle and from each other
    let mut isles: Vec<(MapCoord, f32)> = Vec::new();
    for _ in 0..400 {
        if isles.len() > p.players + 1 {
            break;
        }
        let c = grid.coord(rng.below(grid.len() as u32) as usize);
        let far = |q: MapCoord, d: f32| grid.distance(q, c) as f32 >= d;
        if hq.iter().all(|h| far(*h, r_home + 9.0)) && isles.iter().all(|(q, r)| far(*q, r + 11.0)) {
            isles.push((c, 6.0 + rng.below(4) as f32));
        }
    }
    for c in grid.coords() {
        let i = grid.index(c);
        let (x, y) = fpos(c);
        let fray = (noise.fbm(x, y, (p.width as u32 / 8).max(4), 3, 555) - 0.5) * 14.0;
        let home = hq.iter().map(|h| grid.distance(*h, c) as f32).fold(f32::MAX, f32::min);
        let isle = isles
            .iter()
            .enumerate()
            .map(|(k, (q, r))| (grid.distance(*q, c) as f32 / r, k))
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if home + fray <= r_home {
            elev[i] = elev[i].clamp(0.36, 0.69);
        } else if let Some((d, k)) = isle
            && d + fray / 8.0 <= 1.0
        {
            // every other island has a mountain at its heart
            elev[i] = if k % 2 == 0 && d < 0.5 { elev[i].max(0.8) } else { elev[i].clamp(0.36, 0.69) };
        } else {
            elev[i] = elev[i].min(0.22);
        }
    }
}

/// A start forest and quarry stone for every HQ (the original's starts sit next to woods): trees
/// on most free land within 3 nodes of a spot 7 nodes out on the side with the most free land,
/// and big stone piles 6 nodes out on another side. Land nodes are those with ground water.
/// Runs after [`place_harbors`] and keeps the harbor spots clear.
pub fn start_resources(m: &mut MapData) {
    let grid = m.grid;
    let hq: Vec<MapCoord> = m.hq.iter().flatten().copied().collect();
    let harbors = m.harbors.clone();
    let resources = &m.resources;
    let objects = &mut m.objects;
    let free = |objects: &[MapObject], c: MapCoord| {
        let i = grid.index(c);
        resources[i] == Resource::Water
            && objects[i] == MapObject::None
            && hq.iter().all(|h| grid.distance(*h, c) >= 4)
            && harbors.iter().all(|h| grid.distance(*h, c) > 2)
    };
    for h in &hq {
        let room = |objects: &[MapObject], d: Direction, out: u16, r: u16| {
            grid.within(grid.walk(*h, d, out), r).filter(|c| free(objects, *c)).count()
        };
        let best = |objects: &[MapObject], dirs: &[Direction], out: u16, r: u16| {
            dirs.iter().copied().max_by_key(|d| (room(objects, *d, out, r), std::cmp::Reverse(*d as u8))).unwrap()
        };
        let forest = best(objects, &Direction::ALL, 7, 3);
        // up to 30 trees in reach, counting those already there (trees take building room)
        let mut planted = grid
            .within(*h, 10)
            .filter(|c| grid.distance(*h, *c) >= 3 && matches!(objects[grid.index(*c)], MapObject::Tree(_)))
            .count();
        for r in 3..=4u16 {
            let centre = grid.walk(*h, forest, 7);
            for (k, c) in grid.within(centre, r).enumerate() {
                if planted >= 30 || grid.distance(*h, c) > 10 || !free(objects, c) {
                    continue;
                }
                // dense: a clump blocks less building land than scattered trees
                objects[grid.index(c)] = MapObject::Tree((k % 6) as u8);
                planted += 1;
            }
        }
        let others: Vec<Direction> = Direction::ALL
            .into_iter()
            .filter(|d| (*d as i32 - forest as i32).rem_euclid(6).min((forest as i32 - *d as i32).rem_euclid(6)) >= 2)
            .collect();
        // another side if it has room, else anywhere near (coasts, mountains)
        let (quarry, out) =
            [(&others[..], 6), (&others[..], 5), (&others[..], 8), (&Direction::ALL[..], 5), (&Direction::ALL[..], 8)]
                .into_iter()
                .map(|(dirs, out)| (best(objects, dirs, out, 2), out))
                .find(|(d, out)| room(objects, *d, *out, 2) >= 4)
                .unwrap_or((forest, 9));
        let mut piles = 0;
        for r in 1..=2u16 {
            for c in grid.within(grid.walk(*h, quarry, out), r).collect::<Vec<_>>() {
                if piles < 6 && free(objects, c) {
                    objects[grid.index(c)] = MapObject::Granite { kind: (piles % 2) as u8, size: 6 };
                    piles += 1;
                }
            }
        }
    }
}

/// Mark harbor spots on coasts: buildable land with a real sea (40+ water nodes) within three
/// nodes, at least ten nodes apart. Clears and levels the spot so a harbor fits.
pub fn place_harbors(m: &mut MapData) {
    let g = m.grid;
    let n = g.len();
    let tris = |m: &MapData, c: MapCoord| {
        let nw = g.neighbor(c, Direction::NW);
        let ne = g.neighbor(c, Direction::NE);
        let w = g.neighbor(c, Direction::W);
        [
            m.terrain_rsu[g.index(nw)],
            m.terrain_usd[g.index(nw)],
            m.terrain_rsu[g.index(ne)],
            m.terrain_usd[g.index(c)],
            m.terrain_rsu[g.index(c)],
            m.terrain_usd[g.index(w)],
        ]
    };
    let water = |t: u8| matches!(t, 5 | 6 | 19);
    let land = |t: u8| matches!(t, 0 | 8 | 9 | 10 | 14 | 15);
    // seas and their sizes
    let mut sea = vec![0u32; n];
    let mut sizes = vec![0u32];
    for s in g.coords() {
        if sea[g.index(s)] != 0 || !tris(m, s).iter().all(|t| water(*t)) {
            continue;
        }
        let id = sizes.len() as u32;
        let mut stack = vec![s];
        sea[g.index(s)] = id;
        let mut count = 0;
        while let Some(p) = stack.pop() {
            count += 1;
            for q in g.neighbors(p) {
                if sea[g.index(q)] == 0 && tris(m, q).iter().all(|t| water(*t)) {
                    sea[g.index(q)] = id;
                    stack.push(q);
                }
            }
        }
        sizes.push(count);
    }
    let mut spots: Vec<MapCoord> = Vec::new();
    for c in g.coords() {
        if c.y % 2 == 1 || !tris(m, c).iter().all(|t| land(*t)) {
            continue;
        }
        // the flag (SE) and the large building's extensions must be land too
        let se = g.neighbor(c, Direction::SE);
        if !tris(m, se).iter().all(|t| land(*t) || water(*t)) {
            continue;
        }
        let big_sea = g.within(c, 3).any(|q| {
            let s = sea[g.index(q)];
            s != 0 && sizes[s as usize] >= 40
        });
        if !big_sea
            || spots.iter().any(|s| g.distance(*s, c) < 10)
            || m.hq.iter().flatten().any(|h| g.distance(*h, c) < 5)
        {
            continue;
        }
        spots.push(c);
    }
    for s in &spots {
        let a = m.altitude[g.index(*s)];
        for q in g.within(*s, 2) {
            let i = g.index(q);
            m.altitude[i] = a;
            if tris(m, q).iter().all(|t| land(*t)) {
                m.objects[i] = MapObject::None;
            }
        }
    }
    // keep the spots where a large building really fits
    let w = rr_world::World::new(m);
    spots.retain(|s| w.bq(*s) == rr_world::Bq::Castle);
    m.harbors = spots;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The land a walker reaches from `from` (land: a node not all water around).
    fn landmass(m: &MapData, from: MapCoord) -> Vec<bool> {
        let w = rr_world::World::new(m);
        let g = m.grid;
        let mut seen = vec![false; g.len()];
        let mut stack = vec![from];
        seen[g.index(from)] = true;
        while let Some(p) = stack.pop() {
            for q in g.neighbors(p) {
                if !seen[g.index(q)] && w.node_walkable(q) {
                    seen[g.index(q)] = true;
                    stack.push(q);
                }
            }
        }
        seen
    }

    /// An archipelago (owner, 2026-10-27, "make harbors meaningful"): each realm on its own
    /// island with a harbor spot, and land with ore no realm starts on, so the sea must be crossed.
    #[test]
    fn an_archipelago_parts_the_realms_by_sea() {
        for seed in 1..=4 {
            let m =
                generate(&GenParams { width: 128, height: 128, players: 4, seed, islands: true, ..Default::default() });
            let hq: Vec<MapCoord> = m.hq.iter().flatten().copied().collect();
            let lands: Vec<Vec<bool>> = hq.iter().map(|h| landmass(&m, *h)).collect();
            for (i, land) in lands.iter().enumerate() {
                for (j, h) in hq.iter().enumerate() {
                    assert_eq!(land[m.grid.index(*h)], i == j, "seed {seed}: castles {i} and {j} share land");
                }
                assert!(m.harbors.iter().any(|s| land[m.grid.index(*s)]), "seed {seed}: island {i} has no harbor spot");
            }
            // ore on land no realm starts on
            let homeless = m.grid.coords().any(|c| {
                matches!(m.resources[m.grid.index(c)], Resource::Gold(_) | Resource::Iron(_) | Resource::Coal(_))
                    && lands.iter().all(|l| !l[m.grid.index(c)])
            });
            assert!(homeless, "seed {seed}: no ore on an uninhabited island");
            // RR_MAP_PPM=f.ppm: a picture of the first map (sea, land, forest, mountains, castles
            // in red, harbor spots in yellow)
            if let Ok(path) = std::env::var("RR_MAP_PPM")
                && seed == 1
            {
                let g = m.grid;
                let mut out = format!("P3 {} {} 255\n", g.width, g.height);
                for c in g.coords() {
                    let i = g.index(c);
                    let near = |v: &[MapCoord], r: u32| v.iter().any(|q| g.distance(*q, c) <= r);
                    out += if near(&hq, 1) {
                        "220 30 30"
                    } else if near(&m.harbors, 0) {
                        "250 220 40"
                    } else if m.terrain_rsu[i] == T_WATER {
                        "30 60 140"
                    } else if T_MOUNTAIN.contains(&m.terrain_rsu[i]) {
                        "140 130 120"
                    } else if matches!(m.objects[i], MapObject::Tree(_)) {
                        "40 100 40"
                    } else {
                        "110 170 70"
                    };
                    out += "\n";
                }
                std::fs::write(path, out).unwrap();
            }
        }
    }
}
