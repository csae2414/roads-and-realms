//! Stable hashing for desync detection. Never derived from serialized bytes or `DefaultHasher`.

/// 64-bit hasher with a fixed, documented algorithm (word-wise multiply-xorshift).
#[derive(Clone, Debug)]
pub struct StableHasher {
    state: u64,
}

impl Default for StableHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl StableHasher {
    pub const fn new() -> Self {
        Self { state: 0x243f_6a88_85a3_08d3 }
    }

    pub fn write_u64(&mut self, v: u64) {
        let x = (self.state ^ v).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        self.state = x ^ (x >> 29) ^ (x << 7);
    }

    pub fn finish(&self) -> u64 {
        let mut z = self.state;
        z = (z ^ (z >> 33)).wrapping_mul(0xff51_afd7_ed55_8ccd);
        z ^ (z >> 33)
    }
}

pub trait StableHash {
    fn stable_hash(&self, h: &mut StableHasher);
}

macro_rules! int_hash {
    ($($t:ty),*) => {$(
        impl StableHash for $t {
            fn stable_hash(&self, h: &mut StableHasher) { h.write_u64(*self as u64) }
        }
    )*};
}
int_hash!(u8, u16, u32, u64, usize, i8, i16, i32, i64, bool);

impl StableHash for str {
    fn stable_hash(&self, h: &mut StableHasher) {
        self.as_bytes().stable_hash(h)
    }
}
impl StableHash for String {
    fn stable_hash(&self, h: &mut StableHasher) {
        self.as_str().stable_hash(h)
    }
}

impl<T: StableHash> StableHash for [T] {
    fn stable_hash(&self, h: &mut StableHasher) {
        h.write_u64(self.len() as u64);
        for v in self {
            v.stable_hash(h);
        }
    }
}
impl<T: StableHash> StableHash for Vec<T> {
    fn stable_hash(&self, h: &mut StableHasher) {
        self.as_slice().stable_hash(h)
    }
}
impl<T: StableHash, const N: usize> StableHash for [T; N] {
    fn stable_hash(&self, h: &mut StableHasher) {
        self.as_slice().stable_hash(h)
    }
}
impl<T: StableHash> StableHash for Option<T> {
    fn stable_hash(&self, h: &mut StableHasher) {
        match self {
            None => h.write_u64(0),
            Some(v) => {
                h.write_u64(1);
                v.stable_hash(h);
            }
        }
    }
}
impl<T: StableHash + ?Sized> StableHash for &T {
    fn stable_hash(&self, h: &mut StableHasher) {
        (**self).stable_hash(h)
    }
}
impl<A: StableHash, B: StableHash> StableHash for (A, B) {
    fn stable_hash(&self, h: &mut StableHasher) {
        self.0.stable_hash(h);
        self.1.stable_hash(h);
    }
}
impl<A: StableHash, B: StableHash, C: StableHash> StableHash for (A, B, C) {
    fn stable_hash(&self, h: &mut StableHasher) {
        self.0.stable_hash(h);
        self.1.stable_hash(h);
        self.2.stable_hash(h);
    }
}
impl StableHash for crate::MapCoord {
    fn stable_hash(&self, h: &mut StableHasher) {
        h.write_u64(((self.x as u64) << 16) | self.y as u64)
    }
}
impl StableHash for crate::Gf {
    fn stable_hash(&self, h: &mut StableHasher) {
        self.0.stable_hash(h)
    }
}
impl<T> StableHash for crate::Id<T> {
    fn stable_hash(&self, h: &mut StableHasher) {
        h.write_u64(((self.index() as u64) << 32) | self.generation() as u64)
    }
}
impl<T: StableHash> StableHash for crate::Arena<T> {
    fn stable_hash(&self, h: &mut StableHasher) {
        h.write_u64(self.len() as u64);
        for (id, v) in self.iter() {
            id.stable_hash(h);
            v.stable_hash(h);
        }
    }
}
