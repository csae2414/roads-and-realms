//! Shared primitives for Roads & Realms: coordinates, ids, deterministic RNG, hashing.

mod arena;
mod grid;
mod hash;
mod map;
mod rng;

pub use arena::{Arena, Id};
pub use grid::{Direction, Grid, MapCoord};
pub use hash::{StableHash, StableHasher};
pub use map::{Animal, Landscape, MAX_SIDE, MapData, MapObject, Resource};
pub use rng::{Pcg32, Purpose, rng_for};

/// One game frame (GF). At normal speed one GF is 50 ms.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default, serde::Serialize, serde::Deserialize)]
pub struct Gf(pub u32);

impl Gf {
    pub const MS: u32 = 50;

    pub fn plus(self, n: u32) -> Gf {
        Gf(self.0 + n)
    }
}

/// `a * pct / 100`, rounded half up.
pub fn percent_of(a: u32, pct: u32) -> u32 {
    div_round_half_up(a as u64 * pct as u64, 100) as u32
}

pub fn div_round_half_up(a: u64, b: u64) -> u64 {
    (a + b / 2) / b
}
