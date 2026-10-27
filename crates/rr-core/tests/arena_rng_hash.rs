use rr_core::{Arena, Pcg32, Purpose, StableHash, StableHasher, rng_for};

#[test]
fn arena_insert_get_remove_with_generations() {
    let mut a: Arena<&str> = Arena::new();
    let x = a.insert("x");
    let y = a.insert("y");
    assert_eq!(a.get(x), Some(&"x"));
    assert_eq!(a.len(), 2);
    assert_eq!(a.remove(x), Some("x"));
    assert_eq!(a.get(x), None, "stale id must not resolve");
    let z = a.insert("z");
    assert_eq!(z.index(), x.index(), "free slot is reused");
    assert_ne!(z, x, "but with a new generation");
    assert_eq!(a.get(x), None);
    assert_eq!(a.get(z), Some(&"z"));
    *a.get_mut(y).unwrap() = "Y";
    let items: Vec<_> = a.iter().map(|(_, v)| *v).collect();
    assert_eq!(items, vec!["z", "Y"], "iteration is in index order");
}

#[test]
fn arena_free_list_is_lifo_and_deterministic() {
    let mut a: Arena<u32> = Arena::new();
    let ids: Vec<_> = (0..5).map(|i| a.insert(i)).collect();
    a.remove(ids[1]);
    a.remove(ids[3]);
    assert_eq!(a.insert(10).index(), 3);
    assert_eq!(a.insert(11).index(), 1);
    assert_eq!(a.insert(12).index(), 5);
}

#[test]
fn pcg32_known_sequence() {
    // Reference values of the PCG32 paper's demo (seed 42, stream 54).
    let mut r = Pcg32::new(42, 54);
    let got: Vec<u32> = (0..6).map(|_| r.next_u32()).collect();
    assert_eq!(got, vec![0xa15c02b7, 0x7b47f409, 0xba1d3330, 0x83d2f293, 0xbfa4784b, 0xcbed606e]);
}

#[test]
fn range_is_unbiased_and_in_bounds() {
    let mut r = Pcg32::new(1, 1);
    let mut counts = [0u32; 6];
    for _ in 0..60_000 {
        counts[r.below(6) as usize] += 1;
    }
    for c in counts {
        assert!((9_000..11_000).contains(&c), "{counts:?}");
    }
}

#[test]
fn derived_rng_is_independent_per_use() {
    let a1 = rng_for(7, 100, 3, Purpose::Fight).next_u32();
    let a2 = rng_for(7, 100, 3, Purpose::Fight).next_u32();
    assert_eq!(a1, a2);
    assert_ne!(a1, rng_for(7, 101, 3, Purpose::Fight).next_u32());
    assert_ne!(a1, rng_for(7, 100, 4, Purpose::Fight).next_u32());
    assert_ne!(a1, rng_for(7, 100, 3, Purpose::AnimalWalk).next_u32());
    assert_ne!(a1, rng_for(8, 100, 3, Purpose::Fight).next_u32());
}

const PINNED_ABC: u64 = 14900536896028786145;

#[test]
fn stable_hash_is_stable_and_order_sensitive() {
    fn h<T: StableHash + ?Sized>(v: &T) -> u64 {
        let mut s = StableHasher::new();
        v.stable_hash(&mut s);
        s.finish()
    }
    // pinned value: changing the hasher breaks old replays, so this must never change silently
    assert_eq!(h(&[1u32, 2, 3][..]), h(&vec![1u32, 2, 3]));
    assert_ne!(h(&[1u32, 2, 3][..]), h(&[3u32, 2, 1][..]));
    assert_ne!(h(&(1u8, 0u8)), h(&(0u8, 1u8)));
    assert_ne!(h(&Some(0u32)), h(&None::<u32>));
    assert_eq!(h("abc"), PINNED_ABC);
}
