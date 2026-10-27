//! Deterministic event queue: events fire in (frame, insertion order).

use std::collections::BTreeMap;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Scheduler<E> {
    queue: BTreeMap<(u32, u64), E>,
    seq: u64,
}

impl<E> Default for Scheduler<E> {
    fn default() -> Self {
        Self { queue: BTreeMap::new(), seq: 0 }
    }
}

impl<E> Scheduler<E> {
    /// Schedule `ev` at frame `gf`. Returns a key that can cancel it.
    pub fn at(&mut self, gf: u32, ev: E) -> (u32, u64) {
        let key = (gf, self.seq);
        self.seq += 1;
        self.queue.insert(key, ev);
        key
    }

    pub fn cancel(&mut self, key: (u32, u64)) -> Option<E> {
        self.queue.remove(&key)
    }

    /// Pop the next event due at or before `gf`.
    pub fn pop_due(&mut self, gf: u32) -> Option<E> {
        let (&key, _) = self.queue.first_key_value()?;
        if key.0 > gf {
            return None;
        }
        self.queue.remove(&key)
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&(u32, u64), &E)> {
        self.queue.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fires_in_time_then_insertion_order() {
        let mut s = Scheduler::default();
        s.at(5, "b");
        s.at(3, "a");
        s.at(5, "c");
        let k = s.at(4, "x");
        s.cancel(k);
        assert_eq!(s.pop_due(2), None);
        assert_eq!(s.pop_due(5), Some("a"));
        assert_eq!(s.pop_due(5), Some("b"));
        assert_eq!(s.pop_due(5), Some("c"));
        assert!(s.is_empty());
    }
}
