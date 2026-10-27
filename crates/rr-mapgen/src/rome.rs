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
    let hills: Vec<MapCoord> = HILLS.iter().map(|(_, x, y)| at(*x, *y)).collect();
    for c in grid.coords() {
        let i = grid.index(c);
        let (x, y) = fpos(c);
        // gentle meadows, never sea or mountain
        let mut e = 0.36 + noise.fbm(x, y, 3, 3, 11) * 0.22;
        // the hills: rounded, high but still meadow (a mountain meadow at the top)
        for hill in &hills {
            let d = grid.distance(*hill, c) as f32;
            e = e.max(0.67 - d * 0.035);
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
    hills[..p.players.min(2)].to_vec()
}

/// Jupiter's augur platform on the Capitol (docs/CAMPAIGN-ROME.md, chapter 1): a holy place
/// on its hill's top, the hill's ring around it cleared of trees and rocks and levelled, so
/// borders can close round it.
pub fn holy_places(m: &mut rr_core::MapData) {
    let g = m.grid;
    let (_, fx, fy) = HILLS[JUPITER];
    let at = MapCoord::new((fx * g.width as f32) as u16, ((fy * g.height as f32) as u16) & !1);
    let top = m.altitude[g.index(at)];
    for q in g.within(at, rr_world::HOLY_RING) {
        let i = g.index(q);
        m.objects[i] = rr_core::MapObject::None;
        m.animals[i] = None;
        m.altitude[i] = top;
    }
    m.objects[g.index(at)] = rr_core::MapObject::Decoration(rr_world::HOLY_BASE);
}

#[cfg(test)]
mod tests {
    use crate::{GenParams, T_WATER, generate};
    use rr_core::MapObject;

    /// The first chapter's map (docs/CAMPAIGN-ROME.md): both castles on their hills, high above
    /// the river, on one landmass with room to grow, and a river of water between the hills and
    /// the west.
    #[test]
    fn seven_hills_by_a_river() {
        for seed in 1..=4 {
            let m = generate(&GenParams {
                width: 64,
                height: 64,
                players: 2,
                seed,
                seven_hills: true,
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
                assert!(m.altitude[g.index(*h)] as u32 > mean + 3, "seed {seed}: castle {h:?} not on a hill");
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
            // Jupiter's holy place on the Capitol: neutral ground, outside both castles' first
            // land (radius 9 and its ring of 2), on a level top, nothing around it
            let mut m = m;
            super::holy_places(&mut m);
            let w = rr_world::World::new(&m);
            let holy = w.holy_places();
            assert_eq!(holy.len(), 1, "seed {seed}");
            let (at, god) = holy[0];
            assert_eq!(god, 0, "Jupiter");
            for h in &hq {
                assert!(g.distance(*h, at) >= 12, "seed {seed}: the Capitol is {} from a castle", g.distance(*h, at));
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
}
