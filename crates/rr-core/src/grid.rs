//! Map coordinates on the S2 node grid.
//!
//! Every odd row is shifted half a node to the right. The map wraps in both axes.

/// A node on the map.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default, serde::Serialize, serde::Deserialize)]
pub struct MapCoord {
    pub x: u16,
    pub y: u16,
}

impl MapCoord {
    pub const fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }
}

/// The six neighbor directions, in the original's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum Direction {
    W = 0,
    NW = 1,
    NE = 2,
    E = 3,
    SE = 4,
    SW = 5,
}

impl Direction {
    pub const ALL: [Direction; 6] = [Self::W, Self::NW, Self::NE, Self::E, Self::SE, Self::SW];

    pub const fn from_index(i: u8) -> Self {
        Self::ALL[(i % 6) as usize]
    }

    pub const fn index(self) -> u8 {
        self as u8
    }

    pub const fn opposite(self) -> Self {
        self.rotate_cw(3)
    }

    /// Rotate clockwise by `n` sixths of a turn.
    pub const fn rotate_cw(self, n: u8) -> Self {
        Self::from_index(self as u8 + n % 6)
    }
}

/// Dimensions of a wrapping map; all neighbor math goes through here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Grid {
    pub width: u16,
    pub height: u16,
}

impl Grid {
    pub fn new(width: u16, height: u16) -> Self {
        assert!(width > 0 && height > 0, "empty grid");
        assert!(height.is_multiple_of(2), "S2 maps have an even height");
        Self { width, height }
    }

    pub fn len(&self) -> usize {
        self.width as usize * self.height as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn index(&self, p: MapCoord) -> usize {
        p.y as usize * self.width as usize + p.x as usize
    }

    pub fn coord(&self, i: usize) -> MapCoord {
        MapCoord::new((i % self.width as usize) as u16, (i / self.width as usize) as u16)
    }

    pub fn contains(&self, p: MapCoord) -> bool {
        p.x < self.width && p.y < self.height
    }

    /// All nodes in index order (row by row).
    pub fn coords(&self) -> impl Iterator<Item = MapCoord> + use<> {
        let g = *self;
        (0..g.len()).map(move |i| g.coord(i))
    }

    fn left(&self, x: u16) -> u16 {
        if x == 0 { self.width - 1 } else { x - 1 }
    }

    fn right(&self, x: u16) -> u16 {
        if x + 1 == self.width { 0 } else { x + 1 }
    }

    fn up(&self, y: u16) -> u16 {
        if y == 0 { self.height - 1 } else { y - 1 }
    }

    fn down(&self, y: u16) -> u16 {
        if y + 1 == self.height { 0 } else { y + 1 }
    }

    pub fn neighbor(&self, p: MapCoord, d: Direction) -> MapCoord {
        let odd = p.y & 1 == 1;
        let (x, y) = match d {
            Direction::W => (self.left(p.x), p.y),
            Direction::E => (self.right(p.x), p.y),
            Direction::NW => (if odd { p.x } else { self.left(p.x) }, self.up(p.y)),
            Direction::NE => (if odd { self.right(p.x) } else { p.x }, self.up(p.y)),
            Direction::SW => (if odd { p.x } else { self.left(p.x) }, self.down(p.y)),
            Direction::SE => (if odd { self.right(p.x) } else { p.x }, self.down(p.y)),
        };
        MapCoord::new(x, y)
    }

    pub fn neighbors(&self, p: MapCoord) -> [MapCoord; 6] {
        Direction::ALL.map(|d| self.neighbor(p, d))
    }

    /// Walk `n` steps in one direction.
    pub fn walk(&self, mut p: MapCoord, d: Direction, n: u16) -> MapCoord {
        for _ in 0..n {
            p = self.neighbor(p, d);
        }
        p
    }

    /// Number of steps between two nodes, taking the wrap into account.
    pub fn distance(&self, a: MapCoord, b: MapCoord) -> u32 {
        // "odd-r" offset coordinates to axial: q = x - floor(y/2) (odd rows shifted right). On a
        // wrapping map the nearest copy of `b` is one of nine; the hex distance is at least the
        // rows apart and at least half the columns apart (|2dq + dr| / 2), so copies that can't
        // beat the best so far are skipped (the hottest function of the game: called millions
        // of times; same results as trying all nine).
        fn axial(x: i32, y: i32) -> (i32, i32) {
            (x - (y - (y & 1)) / 2, y)
        }
        let (w, h) = (self.width as i32, self.height as i32);
        let (aq, ar) = axial(a.x as i32, a.y as i32);
        let dy0 = b.y as i32 - a.y as i32;
        // the row offset with the fewest rows between first
        let mut dys = [0, -h, h];
        dys.sort_by_key(|dy| (dy0 + dy).abs());
        let mut best = u32::MAX;
        for dy in dys {
            if (dy0 + dy).unsigned_abs() >= best {
                continue;
            }
            for dx in [0, -w, w] {
                let (bq, br) = axial(b.x as i32 + dx, b.y as i32 + dy);
                let (dq, dr) = (bq - aq, br - ar);
                if (2 * dq + dr).unsigned_abs() / 2 >= best {
                    continue;
                }
                let d = ((dq.abs() + dr.abs() + (dq + dr).abs()) / 2) as u32;
                best = best.min(d);
            }
        }
        best
    }

    /// The plain nine-copy distance, for checking `distance`.
    #[cfg(test)]
    fn distance_all_nine(&self, a: MapCoord, b: MapCoord) -> u32 {
        fn axial(x: i32, y: i32) -> (i32, i32) {
            (x - (y - (y & 1)) / 2, y)
        }
        let (w, h) = (self.width as i32, self.height as i32);
        let (aq, ar) = axial(a.x as i32, a.y as i32);
        let mut best = u32::MAX;
        for dy in [-h, 0, h] {
            for dx in [-w, 0, w] {
                let (bq, br) = axial(b.x as i32 + dx, b.y as i32 + dy);
                let (dq, dr) = (bq - aq, br - ar);
                best = best.min(((dq.abs() + dr.abs() + (dq + dr).abs()) / 2) as u32);
            }
        }
        best
    }

    /// Nodes at exactly distance `r`, in a fixed order: starting `r` steps west, then clockwise.
    pub fn ring(&self, p: MapCoord, r: u16) -> impl Iterator<Item = MapCoord> + use<> {
        let g = *self;
        let start = g.walk(p, Direction::W, r);
        let steps = if r == 0 { 1 } else { 6 * r as usize };
        let mut cur = start;
        let mut emitted = 0usize;
        std::iter::from_fn(move || {
            if emitted == steps {
                return None;
            }
            let out = cur;
            if r > 0 {
                let side = emitted / r as usize;
                let dir =
                    [Direction::NE, Direction::E, Direction::SE, Direction::SW, Direction::W, Direction::NW][side];
                cur = g.neighbor(cur, dir);
            }
            emitted += 1;
            Some(out)
        })
    }

    /// Nodes within distance `r`, ring by ring outward from `p`.
    /// Call `f` with the index of every node within `r` of `p`, row by row: the same nodes as
    /// `within` (a node may come twice on maps smaller than the hex), much faster.
    pub fn for_each_within(&self, p: MapCoord, r: u16, mut f: impl FnMut(usize)) {
        let (w, h, r) = (self.width as i32, self.height as i32, r as i32);
        // odd-r offset to axial: q = x - floor(y / 2)
        let (cy, cq) = (p.y as i32, p.x as i32 - (p.y as i32 - (p.y as i32 & 1)) / 2);
        for dr in -r..=r {
            let y = cy + dr;
            let half = (y - (y & 1)) / 2;
            let (lo, hi) = ((-r).max(-dr - r), r.min(-dr + r));
            let row = y.rem_euclid(h) as usize * w as usize;
            for dq in lo..=hi {
                let x = (cq + dq + half).rem_euclid(w);
                f(row + x as usize);
            }
        }
    }

    pub fn within(&self, p: MapCoord, r: u16) -> impl Iterator<Item = MapCoord> + use<> {
        let g = *self;
        (0..=r).flat_map(move |i| g.ring(p, i))
    }
}

#[cfg(test)]
mod distance_tests {
    use super::{Grid, MapCoord};

    /// Performance roast #7: row-by-row stamping covers exactly the nodes of `within`.
    #[test]
    fn for_each_within_equals_within() {
        for (w, h) in [(8, 8), (32, 24), (97, 64), (192, 192)] {
            let g = Grid::new(w, h);
            for (x, y, r) in [(0, 0, 3), (5, 7, 12), (w - 1, h - 1, 9), (w / 2, 1, 20), (3, h - 2, 2), (1, 1, 0)] {
                let p = MapCoord::new(x % w, y % h);
                let mut a: Vec<usize> = g.within(p, r).map(|q| g.index(q)).collect();
                let mut b = Vec::new();
                g.for_each_within(p, r, |i| b.push(i));
                a.sort();
                a.dedup();
                b.sort();
                b.dedup();
                assert_eq!(a, b, "{w}x{h} {p:?} r{r}");
            }
        }
    }

    /// Performance roast #6: the pruned distance gives exactly what trying all nine copies gives.
    #[test]
    fn pruned_distance_equals_all_nine() {
        let mut seed = 12345u64;
        let mut rnd = |n: u16| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((seed >> 33) % n as u64) as u16
        };
        for (w, h) in [(8, 8), (32, 24), (96, 96), (97, 64), (192, 192), (40, 200)] {
            let g = Grid::new(w, h);
            for _ in 0..20_000 {
                let a = MapCoord::new(rnd(w), rnd(h));
                let b = MapCoord::new(rnd(w), rnd(h));
                assert_eq!(g.distance(a, b), g.distance_all_nine(a, b), "{w}x{h} {a:?} {b:?}");
            }
        }
    }
}
