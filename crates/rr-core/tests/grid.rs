use rr_core::{Direction, Grid, MapCoord};

fn c(x: u16, y: u16) -> MapCoord {
    MapCoord::new(x, y)
}

#[test]
fn direction_order_matches_s2() {
    use Direction::*;
    assert_eq!(Direction::ALL, [W, NW, NE, E, SE, SW]);
    assert_eq!(W.opposite(), E);
    assert_eq!(NW.opposite(), SE);
    assert_eq!(NE.opposite(), SW);
    assert_eq!(NE.rotate_cw(1), E);
    assert_eq!(W.rotate_cw(5), SW);
}

#[test]
fn neighbors_even_row() {
    let g = Grid::new(10, 10);
    let p = c(4, 4);
    assert_eq!(g.neighbor(p, Direction::W), c(3, 4));
    assert_eq!(g.neighbor(p, Direction::NW), c(3, 3));
    assert_eq!(g.neighbor(p, Direction::NE), c(4, 3));
    assert_eq!(g.neighbor(p, Direction::E), c(5, 4));
    assert_eq!(g.neighbor(p, Direction::SE), c(4, 5));
    assert_eq!(g.neighbor(p, Direction::SW), c(3, 5));
}

#[test]
fn neighbors_odd_row() {
    let g = Grid::new(10, 10);
    let p = c(4, 5);
    assert_eq!(g.neighbor(p, Direction::W), c(3, 5));
    assert_eq!(g.neighbor(p, Direction::NW), c(4, 4));
    assert_eq!(g.neighbor(p, Direction::NE), c(5, 4));
    assert_eq!(g.neighbor(p, Direction::E), c(5, 5));
    assert_eq!(g.neighbor(p, Direction::SE), c(5, 6));
    assert_eq!(g.neighbor(p, Direction::SW), c(4, 6));
}

#[test]
fn neighbors_wrap_at_edges() {
    let g = Grid::new(8, 6);
    assert_eq!(g.neighbor(c(0, 0), Direction::W), c(7, 0));
    assert_eq!(g.neighbor(c(0, 0), Direction::NW), c(7, 5));
    assert_eq!(g.neighbor(c(7, 5), Direction::E), c(0, 5));
    assert_eq!(g.neighbor(c(7, 5), Direction::SE), c(0, 0));
}

#[test]
fn every_neighbor_relation_is_symmetric() {
    let g = Grid::new(8, 6);
    for p in g.coords() {
        for d in Direction::ALL {
            assert_eq!(g.neighbor(g.neighbor(p, d), d.opposite()), p, "{p:?} {d:?}");
        }
    }
}

#[test]
fn index_roundtrip() {
    let g = Grid::new(8, 6);
    assert_eq!(g.len(), 48);
    for (i, p) in g.coords().enumerate() {
        assert_eq!(g.index(p), i);
        assert_eq!(g.coord(i), p);
    }
}

#[test]
fn distance_is_hex_distance_with_wrap() {
    let g = Grid::new(20, 20);
    let p = c(5, 5);
    assert_eq!(g.distance(p, p), 0);
    for d in Direction::ALL {
        assert_eq!(g.distance(p, g.neighbor(p, d)), 1);
    }
    let mut q = p;
    for _ in 0..3 {
        q = g.neighbor(q, Direction::E);
    }
    q = g.neighbor(q, Direction::SE);
    assert_eq!(g.distance(p, q), 4);
    // wrap: 0 and 19 are neighbours
    assert_eq!(g.distance(c(0, 4), c(19, 4)), 1);
    assert_eq!(g.distance(c(3, 0), c(3, 19)), 1);
}

#[test]
fn ring_has_6r_nodes_at_exact_distance() {
    let g = Grid::new(30, 30);
    let p = c(10, 11);
    assert_eq!(g.ring(p, 0).collect::<Vec<_>>(), vec![p]);
    for r in 1..6u16 {
        let ring: Vec<_> = g.ring(p, r).collect();
        assert_eq!(ring.len(), 6 * r as usize);
        for q in &ring {
            assert_eq!(g.distance(p, *q), r as u32);
        }
        let mut sorted = ring.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), ring.len());
    }
}

#[test]
fn radius_iterates_rings_in_order() {
    let g = Grid::new(30, 30);
    let p = c(10, 10);
    let all: Vec<_> = g.within(p, 3).collect();
    assert_eq!(all.len(), 1 + 6 + 12 + 18);
    assert_eq!(all[0], p);
    let dists: Vec<u32> = all.iter().map(|q| g.distance(p, *q)).collect();
    assert!(dists.windows(2).all(|w| w[0] <= w[1]));
}
