//! PCG32 (XSH-RR), implemented here so the algorithm can never change underneath us.

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Pcg32 {
    state: u64,
    inc: u64,
}

const MUL: u64 = 6364136223846793005;

impl Pcg32 {
    pub fn new(seed: u64, stream: u64) -> Self {
        let mut r = Self { state: 0, inc: (stream << 1) | 1 };
        r.next_u32();
        r.state = r.state.wrapping_add(seed);
        r.next_u32();
        r
    }

    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old.wrapping_mul(MUL).wrapping_add(self.inc);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// Uniform value in `0..n` without modulo bias. `n` must be > 0.
    pub fn below(&mut self, n: u32) -> u32 {
        assert!(n > 0);
        let threshold = n.wrapping_neg() % n;
        loop {
            let r = self.next_u32();
            if r >= threshold {
                return r % n;
            }
        }
    }

    /// Uniform value in `lo..=hi`.
    pub fn range(&mut self, lo: u32, hi: u32) -> u32 {
        lo + self.below(hi - lo + 1)
    }

    /// True with probability `percent` / 100.
    pub fn chance(&mut self, percent: u32) -> bool {
        self.below(100) < percent
    }
}

/// What a random draw is for. Part of the derivation key, so new uses never shift old ones.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum Purpose {
    Fight = 1,
    AnimalWalk = 2,
    TreeGrowth = 3,
    Geologist = 4,
    Production = 5,
    Nature = 6,
    Military = 7,
    Ai = 8,
    MapGen = 9,
    Misc = 10,
}

fn mix(mut z: u64) -> u64 {
    // splitmix64 finalizer
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

/// A fresh generator for one use: (game seed, frame, entity, purpose).
pub fn rng_for(seed: u64, gf: u32, entity: u64, purpose: Purpose) -> Pcg32 {
    let a = mix(seed ^ mix(gf as u64 ^ ((purpose as u64) << 40)));
    let b = mix(entity.wrapping_add(0x9e3779b97f4a7c15) ^ a);
    Pcg32::new(a, b)
}
