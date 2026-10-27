//! Generational arena: dense storage, deterministic slot reuse, stale ids never resolve.

use std::fmt;
use std::marker::PhantomData;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(bound = "")]
pub struct Id<T> {
    idx: u32,
    generation: u32,
    _t: PhantomData<fn() -> T>,
}

impl<T> Id<T> {
    pub const fn from_raw(idx: u32, generation: u32) -> Self {
        Self { idx, generation, _t: PhantomData }
    }

    pub const fn index(self) -> usize {
        self.idx as usize
    }

    pub const fn generation(self) -> u32 {
        self.generation
    }
}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Id<T> {}
impl<T> PartialEq for Id<T> {
    fn eq(&self, o: &Self) -> bool {
        self.idx == o.idx && self.generation == o.generation
    }
}
impl<T> Eq for Id<T> {}
impl<T> PartialOrd for Id<T> {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl<T> Ord for Id<T> {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        (self.idx, self.generation).cmp(&(o.idx, o.generation))
    }
}
impl<T> std::hash::Hash for Id<T> {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        (self.idx, self.generation).hash(h)
    }
}
impl<T> fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}v{}", self.idx, self.generation)
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Slot<T> {
    generation: u32,
    value: Option<T>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Arena<T> {
    slots: Vec<Slot<T>>,
    free: Vec<u32>,
    len: usize,
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Arena<T> {
    pub const fn new() -> Self {
        Self { slots: Vec::new(), free: Vec::new(), len: 0 }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    /// Number of slots (live or free): every id's index is below this.
    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn insert(&mut self, value: T) -> Id<T> {
        self.len += 1;
        if let Some(idx) = self.free.pop() {
            let s = &mut self.slots[idx as usize];
            s.value = Some(value);
            Id::from_raw(idx, s.generation)
        } else {
            self.slots.push(Slot { generation: 0, value: Some(value) });
            Id::from_raw(self.slots.len() as u32 - 1, 0)
        }
    }

    /// Insert with access to the id the value will get.
    pub fn insert_with(&mut self, f: impl FnOnce(Id<T>) -> T) -> Id<T> {
        let id = self.next_id();
        let got = self.insert(f(id));
        debug_assert_eq!(id, got);
        got
    }

    /// The id the next `insert` will return.
    pub fn next_id(&self) -> Id<T> {
        match self.free.last() {
            Some(&idx) => Id::from_raw(idx, self.slots[idx as usize].generation),
            None => Id::from_raw(self.slots.len() as u32, 0),
        }
    }

    pub fn remove(&mut self, id: Id<T>) -> Option<T> {
        let s = self.slots.get_mut(id.index())?;
        if s.generation != id.generation || s.value.is_none() {
            return None;
        }
        s.generation = s.generation.wrapping_add(1);
        self.free.push(id.idx);
        self.len -= 1;
        s.value.take()
    }

    /// The live id at a slot index, if any (for handles stored as plain indices).
    pub fn id_at(&self, index: usize) -> Option<Id<T>> {
        let s = self.slots.get(index)?;
        s.value.as_ref().map(|_| Id::from_raw(index as u32, s.generation))
    }

    pub fn contains(&self, id: Id<T>) -> bool {
        self.get(id).is_some()
    }

    pub fn get(&self, id: Id<T>) -> Option<&T> {
        let s = self.slots.get(id.index())?;
        if s.generation == id.generation { s.value.as_ref() } else { None }
    }

    pub fn get_mut(&mut self, id: Id<T>) -> Option<&mut T> {
        let s = self.slots.get_mut(id.index())?;
        if s.generation == id.generation { s.value.as_mut() } else { None }
    }

    pub fn iter(&self) -> impl Iterator<Item = (Id<T>, &T)> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.value.as_ref().map(|v| (Id::from_raw(i as u32, s.generation), v)))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Id<T>, &mut T)> {
        self.slots
            .iter_mut()
            .enumerate()
            .filter_map(|(i, s)| s.value.as_mut().map(|v| (Id::from_raw(i as u32, s.generation), v)))
    }

    pub fn ids(&self) -> Vec<Id<T>> {
        self.iter().map(|(id, _)| id).collect()
    }
}

impl<T> std::ops::Index<Id<T>> for Arena<T> {
    type Output = T;
    fn index(&self, id: Id<T>) -> &T {
        self.get(id).expect("stale or invalid id")
    }
}

impl<T> std::ops::IndexMut<Id<T>> for Arena<T> {
    fn index_mut(&mut self, id: Id<T>) -> &mut T {
        self.get_mut(id).expect("stale or invalid id")
    }
}
