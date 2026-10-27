//! The Roman campaign's first map: seven hills east of a river (the Tiber), meadows and woods
//! around them, Romulus's castle on the Palatine and Remus's on the Aventine
//! (docs/CAMPAIGN-ROME.md, chapter 1). The layout is fixed; the seed varies only the details.

use crate::{GenParams, Noise, fpos};
use rr_core::{Grid, MapCoord};

/// The hills, by their Latin names, as fractions of the map (x east, y south): the Palatine
/// in the middle, the Aventine south of it by the river, the Capitol by the river between
/// them, neutral ground (12-13 nodes from both castles: Jupiter's hill, `JUPITER`).
pub const HILLS: [(&str, f32, f32); 7] = [
    ("Palatine", 0.47, 0.50),
    ("Aventine", 0.40, 0.86),
    ("Capitoline", 0.30, 0.62),
    ("Quirinal", 0.50, 0.20),
    ("Viminal", 0.64, 0.28),
    ("Esquiline", 0.70, 0.46),
    ("Caelian", 0.62, 0.68),
];

/// The Alban hills, a small mountain range south-east of the city (ore and granite for the
/// first mine of the first-steps list), as a fraction of the map, and their radius in nodes.
const ALBAN: (f32, f32, f32) = (0.82, 0.70, 4.0);

/// The hill whose top holds Jupiter's augur platform (an index into `HILLS`).
pub const JUPITER: usize = 2;

/// The layouts of the seven hills, one per chapter that plays on them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RomeMap {
    /// Chapter I: Romulus on the Palatine, Remus on the Aventine, Jupiter's platform on the
    /// Capitol between them.
    Augury,
    /// Chapter II: Romulus on the Palatine, the Sabines on the Quirinal to the north, and
    /// Concordia's shrine in the valley between them.
    Sabines,
    /// Chapter III: hilly Etruscan country rich in ore (the random map, more mountains), and
    /// Juno's altar on the player's side of the way to Veii.
    Veii,
    /// Chapter IV: Rome on the Palatine, two Gaulish realms far to the north, and Juno's temple
    /// with the sacred geese on the Capitol, inside Rome's first land.
    Gauls,
    /// Chapter V: the random map with allied towns; Janus's gates a quarter of the way to the
    /// first of them.
    Appian,
    /// Chapter VI: the random map; the Field of Mars near Rome.
    Pyrrhus,
    /// Chapter VII: the islands; Neptune's shrine halfway across the sea, on whatever island lies
    /// there.
    Mylae,
    /// Chapter VIII: the random map with more mountains (the Alps); Cybele's temple in Rome's first
    /// land (Cannae's freed slaves and her stone come while it is held).
    Hannibal,
    /// Chapter IX: the random map (the siege was by land; the sea is chapter VII's); the Field of
    /// Mars near Rome and Carthage's temple near Carthage.
    Carthage,
    /// Chapter X: Europe (the world campaign's region, Rome in Italy); Janus's gates near Rome.
    Janus,
}

/// Where a layout's holy place stands: a point of the seven hills (fractions of the map), or a
/// share of the way from the player's castle to the first rival's (on the random maps).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Spot {
    Map(f32, f32),
    Way(f32),
    /// Inside the player's first land: 5-6 nodes from its castle, held from the start (a god
    /// whose deeds tell history, which must not wait for the player to expand).
    Home,
}

impl RomeMap {
    /// Whether the map is the seven hills (else the random map with this layout's holy place).
    pub fn hills(self) -> bool {
        matches!(self, RomeMap::Augury | RomeMap::Sabines | RomeMap::Gauls)
    }

    /// The random map's shape for this layout: (islands, mountains in percent).
    pub fn land(self) -> (bool, u32) {
        match self {
            RomeMap::Mylae => (true, 14),
            RomeMap::Veii | RomeMap::Hannibal => (false, 22),
            _ => (false, 14),
        }
    }

    /// The castles' places (fractions of the map), the player's first: on the seven hills,
    /// or the Sabines' own hill far north of the Quirinal (so both realms must push out to
    /// meet at Concordia's shrine; on the Quirinal their land touched it from the start).
    pub fn castles(self) -> Vec<(f32, f32)> {
        let hill = |i: usize| (HILLS[i].1, HILLS[i].2);
        match self {
            RomeMap::Sabines => vec![hill(0), (0.53, 0.08)],
            // (the Senones beyond the hills, north-west and north-east)
            RomeMap::Gauls => vec![hill(0), (0.30, 0.06), (0.78, 0.10)],
            // (the random maps place their own castles)
            _ => vec![hill(0), hill(1)],
        }
    }

    /// The holy places: where, and whose god (`rr_world::HOLY_PLACES`).
    pub fn holy(self) -> Vec<(Spot, u8)> {
        match self {
            RomeMap::Augury => vec![(Spot::Map(HILLS[JUPITER].1, HILLS[JUPITER].2), 0)],
            RomeMap::Veii => vec![(Spot::Way(0.4), 2)],
            // (in the valley halfway between the two castles)
            RomeMap::Sabines => {
                let c = self.castles();
                vec![(Spot::Map((c[0].0 + c[1].0) / 2.0, (c[0].1 + c[1].1) / 2.0), 1)]
            }
            // (on the Capitol's slope by the Palatine, 6 nodes from the castle: Rome's own)
            RomeMap::Gauls => vec![(Spot::Map(0.40, 0.55), 3)],
            RomeMap::Appian => vec![(Spot::Way(0.25), 4)],
            RomeMap::Pyrrhus => vec![(Spot::Way(0.2), 5)],
            RomeMap::Mylae => vec![(Spot::Way(0.5), 6)],
            RomeMap::Hannibal => vec![(Spot::Home, 7)],
            RomeMap::Carthage => vec![(Spot::Way(0.2), 5), (Spot::Way(0.8), 8)],
            RomeMap::Janus => vec![(Spot::Way(0.15), 4)],
        }
    }
}

/// Where the river runs at row `y` (fraction of the width): a bend west of the hills that
/// wraps with the map.
fn river_x(y: f32, w: f32) -> f32 {
    w * (0.24 + 0.06 * (y * std::f32::consts::TAU).sin())
}

/// Shape the elevation (0..1, water below 0.3, mountains above 0.7) and return the castles:
/// the Palatine first, then the Aventine (the players' slots).
pub(crate) fn seven_hills(p: &GenParams, grid: &Grid, elev: &mut [f32]) -> Vec<MapCoord> {
    let noise = Noise::new(p.seed ^ 0x0753, p.width as u32, p.height as u32);
    let (w, h) = (p.width as f32, p.height as f32);
    let at = |fx: f32, fy: f32| {
        // (castles on even rows, as the generator's own starts)
        let y = ((fy * h) as u16) & !1;
        MapCoord::new((fx * w) as u16, y)
    };
    // the seven hills, and a hill under each castle (the Sabines' lies beyond them)
    let castles: Vec<MapCoord> =
        p.rome.unwrap_or(RomeMap::Augury).castles().into_iter().map(|(x, y)| at(x, y)).collect();
    let hills: Vec<MapCoord> = HILLS.iter().map(|(_, x, y)| at(*x, *y)).chain(castles.iter().copied()).collect();
    for c in grid.coords() {
        let i = grid.index(c);
        let (x, y) = fpos(c);
        // gentle meadows, never sea or mountain
        let mut e = 0.36 + noise.fbm(x, y, 3, 3, 11) * 0.22;
        // the hills: rounded, high but still meadow (a mountain meadow at the top); the
        // castles' a little higher (below 0.7, where the mountains begin)
        for (k, hill) in hills.iter().enumerate() {
            let top = if k >= HILLS.len() { 0.69 } else { 0.67 };
            e = e.max(top - grid.distance(*hill, c) as f32 * 0.035);
        }
        // the Alban hills: a mountain with a snowless top
        let alban = at(ALBAN.0, ALBAN.1);
        let d = grid.distance(alban, c) as f32;
        if d <= ALBAN.2 {
            e = e.max(0.86 - d * 0.03);
        }
        // the river: three nodes wide, wrapping with the map
        let rx = river_x(c.y as f32 / h, w);
        let dx = (c.x as f32 - rx).abs().min(w - (c.x as f32 - rx).abs());
        if dx < 1.6 {
            e = 0.15;
        }
        elev[i] = e;
    }
    castles[..p.players.min(castles.len())].to_vec()
}

/// The layout's holy place (docs/CAMPAIGN-ROME.md): Jupiter's augur platform on the Capitol,
/// Concordia's shrine in the valley; its ring cleared of trees and rocks and levelled, so
/// borders can close round it.
pub fn holy_places(m: &mut rr_core::MapData, layout: RomeMap) {
    for (spot, god) in layout.holy() {
        holy_place(m, spot, god);
    }
}

fn holy_place(m: &mut rr_core::MapData, spot: Spot, god: u8) {
    let g = m.grid;
    let at = match spot {
        Spot::Map(fx, fy) => MapCoord::new((fx * g.width as f32) as u16, ((fy * g.height as f32) as u16) & !1),
        // (that share of the way, else a little nearer or farther: a lake or a mountain may lie
        // there)
        Spot::Home => {
            let Some(at) = near_home(m) else { return };
            at
        }
        Spot::Way(f) => {
            let tries = [f, f - 0.1, f + 0.1, f - 0.2, f + 0.2];
            let Some(at) = tries.into_iter().filter(|f| *f > 0.05 && *f < 0.95).find_map(|f| between_castles(m, f))
            else {
                return;
            };
            at
        }
    };
    let top = m.altitude[g.index(at)];
    for q in g.within(at, rr_world::HOLY_RING) {
        let i = g.index(q);
        m.objects[i] = rr_core::MapObject::None;
        m.animals[i] = None;
        m.altitude[i] = top;
    }
    m.objects[g.index(at)] = rr_core::MapObject::Decoration(rr_world::HOLY_BASE + god);
}

/// A spot 5-6 nodes from the player's castle with buildable land all round its ring: inside the
/// castle's first land (radius 9), ring and all.
fn near_home(m: &rr_core::MapData) -> Option<MapCoord> {
    let g = m.grid;
    let home = m.hq.first().copied().flatten()?;
    let soil = |t: u8| {
        crate::T_MEADOW.contains(&t)
            || [crate::T_STEPPE, crate::T_FLOWERS, crate::T_SAVANNAH, crate::T_MOUNTAIN_MEADOW].contains(&t)
    };
    let land = |q: MapCoord| soil(m.terrain_rsu[g.index(q)]) && soil(m.terrain_usd[g.index(q)]);
    let mut spots: Vec<MapCoord> = g
        .within(home, 6)
        .filter(|q| g.distance(home, *q) >= 5)
        .filter(|q| g.within(*q, rr_world::HOLY_RING + 1).all(land))
        .collect();
    spots.sort_by_key(|q| (q.y, q.x));
    spots.first().copied()
}

/// A spot for a holy place `frac` of the way from the first castle to the second (across the
/// map's wrap the short way): the nearest node there with buildable land all round its ring
/// (not mountain, not water), not by a castle.
fn between_castles(m: &rr_core::MapData, frac: f32) -> Option<MapCoord> {
    let g = m.grid;
    let (a, b) = (m.hq.first().copied().flatten()?, m.hq.get(1).copied().flatten()?);
    let (w, h) = (g.width as f32, g.height as f32);
    let wrap = |d: f32, size: f32| {
        if d > size / 2.0 {
            d - size
        } else if d < -size / 2.0 {
            d + size
        } else {
            d
        }
    };
    let dx = wrap(b.x as f32 - a.x as f32, w);
    let dy = wrap(b.y as f32 - a.y as f32, h);
    let x = (a.x as f32 + dx * frac).rem_euclid(w) as u16;
    let y = (a.y as f32 + dy * frac).rem_euclid(h) as u16 & !1;
    let aim = MapCoord::new(x, y);
    // (buildable land: the meadows, steppe, flowers, savannah, the mountains' meadow)
    let soil = |t: u8| {
        crate::T_MEADOW.contains(&t)
            || [crate::T_STEPPE, crate::T_FLOWERS, crate::T_SAVANNAH, crate::T_MOUNTAIN_MEADOW].contains(&t)
    };
    let land = |q: MapCoord| soil(m.terrain_rsu[g.index(q)]) && soil(m.terrain_usd[g.index(q)]);
    let mut spots: Vec<MapCoord> = g
        .within(aim, 12)
        .filter(|q| g.within(*q, rr_world::HOLY_RING + 1).all(land))
        .filter(|q| m.hq.iter().flatten().all(|c| g.distance(*c, *q) > 6))
        .collect();
    spots.sort_by_key(|q| (g.distance(aim, *q), q.y, q.x));
    spots.first().copied()
}

#[cfg(test)]
mod tests {
    use super::RomeMap;
    use crate::{GenParams, T_WATER, generate};
    use rr_core::MapObject;

    /// The chapters' maps (docs/CAMPAIGN-ROME.md): both castles on their hills, high above
    /// the river, on one landmass with room to grow, and a river of water between the hills and
    /// the west; the layout's holy place on neutral ground.
    #[test]
    fn seven_hills_by_a_river() {
        for (layout, seed) in [RomeMap::Augury, RomeMap::Sabines].into_iter().flat_map(|l| (1..=4).map(move |s| (l, s)))
        {
            let m = generate(&GenParams {
                width: 64,
                height: 64,
                players: 2,
                seed,
                rome: Some(layout),
                ..Default::default()
            });
            let g = m.grid;
            let hq: Vec<_> = m.hq.iter().flatten().copied().collect();
            assert_eq!(hq.len(), 2, "seed {seed}");
            let w = rr_world::World::new(&m);
            for h in &hq {
                assert_eq!(w.bq(*h), rr_world::Bq::Castle, "seed {seed}: no room for the castle at {h:?}");
            }
            // the castles stand on hills: higher than the land around on average
            let mean = m.altitude.iter().map(|a| *a as u32).sum::<u32>() / g.len() as u32;
            for h in &hq {
                assert!(
                    m.altitude[g.index(*h)] as u32 > mean + 3,
                    "{layout:?} seed {seed}: castle {h:?} not on a hill: {} vs mean {mean}",
                    m.altitude[g.index(*h)]
                );
            }
            assert!(g.distance(hq[0], hq[1]) >= 16, "seed {seed}: the castles are too close");
            let water = m.terrain_rsu.iter().filter(|t| **t == T_WATER).count();
            assert!(water * 100 / g.len() >= 3, "seed {seed}: no river");
            // one landmass: Remus's castle can be reached from Romulus's
            let mut seen = vec![false; g.len()];
            let mut stack = vec![hq[0]];
            seen[g.index(hq[0])] = true;
            while let Some(p) = stack.pop() {
                for q in g.neighbors(p) {
                    if !seen[g.index(q)] && w.node_walkable(q) {
                        seen[g.index(q)] = true;
                        stack.push(q);
                    }
                }
            }
            assert!(seen[g.index(hq[1])], "seed {seed}: the castles are cut apart");
            // the Alban hills hold ore: a mine for the first-steps list
            assert!(
                m.resources.iter().any(|r| matches!(
                    r,
                    rr_core::Resource::Coal(_)
                        | rr_core::Resource::Iron(_)
                        | rr_core::Resource::Gold(_)
                        | rr_core::Resource::Granite(_)
                )),
                "seed {seed}: no ore"
            );
            let trees = m.objects.iter().filter(|o| matches!(o, MapObject::Tree(_))).count();
            assert!(trees > 200, "seed {seed}: only {trees} trees");
            // the holy place (Jupiter's on the Capitol, Concordia's in the valley): neutral
            // ground, outside both castles' first land (radius 9), nothing around it
            let mut m = m;
            super::holy_places(&mut m, layout);
            let w = rr_world::World::new(&m);
            let holy = w.holy_places();
            assert_eq!(holy.len(), 1, "seed {seed}");
            let (at, god) = holy[0];
            assert_eq!(god, layout.holy()[0].1, "{layout:?}");
            for h in &hq {
                assert!(
                    g.distance(*h, at) >= 10,
                    "{layout:?} {seed}: the holy place is {} from a castle",
                    g.distance(*h, at)
                );
            }
            assert!(
                g.within(at, rr_world::HOLY_RING).all(|q| q == at || matches!(m.objects[g.index(q)], MapObject::None))
            );
            assert!(
                !g.within(at, rr_world::HOLY_RING).any(|q| m.terrain_rsu[g.index(q)] == T_WATER),
                "seed {seed}: in the river"
            );
            // RR_MAP_PPM=f.ppm: a picture of the first map (water, meadow, hills lighter,
            // mountains grey, forest, castles red)
            if let Ok(path) = std::env::var("RR_MAP_PPM")
                && seed == 1
                && layout == RomeMap::Sabines
            {
                let mut out = format!("P3 {} {} 255\n", g.width, g.height);
                for c in g.coords() {
                    let i = g.index(c);
                    let a = m.altitude[i] as u32;
                    let px = if hq.iter().any(|h| g.distance(*h, c) <= 1) {
                        "220 30 30".to_string()
                    } else if m.terrain_rsu[i] == T_WATER {
                        "30 60 140".to_string()
                    } else if crate::T_MOUNTAIN.contains(&m.terrain_rsu[i]) {
                        "140 130 120".to_string()
                    } else if matches!(m.objects[i], MapObject::Tree(_)) {
                        "40 100 40".to_string()
                    } else {
                        format!("{} {} 60", 60 + a * 5, 120 + a * 4)
                    };
                    out += &px;
                    out += "\n";
                }
                std::fs::write(path, out).unwrap();
            }
        }
    }

    /// Chapter IV: Rome and two Gaulish castles far to the north, and Juno's temple of the geese
    /// inside Rome's first land, away from the river and with its ring clear.
    #[test]
    fn the_gauls_come_from_the_north() {
        for seed in 1..=4 {
            let mut m = generate(&GenParams {
                width: 64,
                height: 64,
                players: 3,
                seed,
                rome: Some(RomeMap::Gauls),
                ..Default::default()
            });
            super::holy_places(&mut m, RomeMap::Gauls);
            let g = m.grid;
            let hq: Vec<_> = m.hq.iter().flatten().copied().collect();
            assert_eq!(hq.len(), 3, "seed {seed}");
            for gaul in &hq[1..] {
                assert!(
                    g.distance(hq[0], *gaul) >= 22,
                    "seed {seed}: a Gaulish castle {} from Rome",
                    g.distance(hq[0], *gaul)
                );
            }
            let w = rr_world::World::new(&m);
            let holy = w.holy_places();
            assert_eq!(holy.iter().map(|h| h.1).collect::<Vec<_>>(), vec![3], "seed {seed}");
            let at = holy[0].0;
            assert!(
                g.distance(hq[0], at) + rr_world::HOLY_RING as u32 <= 9,
                "seed {seed}: the temple outside Rome's first land"
            );
            assert!(g.within(at, rr_world::HOLY_RING).all(|q| w.node_walkable(q)), "seed {seed}");
        }
    }
}
