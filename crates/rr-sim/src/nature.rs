//! Trees, grain fields, fire, geologist signs and wild animals.

use crate::figures::{FigKind, Step};
use crate::{Ev, FigId, Sim};
use rr_core::{Direction, MapCoord, MapData, Purpose};
use rr_rules::Job;
use rr_world::NodeObj;
use std::collections::{BTreeMap, BTreeSet};

/// RTTR `noTree::WAIT_LENGTH`: frames between growth stages.
const TREE_STAGE_GF: u32 = 835;
/// RTTR `noGrainfield`: frames between field stages, and until a ripe field withers.
const FIELD_STAGE_GF: u32 = 1100;
const FIELD_WITHER_GF: u32 = 3000;
/// RTTR noFire, noGrainfield (a withered field's last stage) and noSign (fidelity #9).
const FIRE_GF: u32 = 3700;
const FIELD_WITHERED_GF: u32 = 20;
const SIGN_GF: u32 = 8500;
const SIGN_RAND: u32 = 500;

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Nature {
    /// Nodes a worker is heading to (tree to fell, stone to cut, spot to plant).
    pub reserved: BTreeSet<MapCoord>,
    /// The frame a node's pending nature event is due; stale events are ignored.
    pub due: BTreeMap<MapCoord, u32>,
}

impl Sim {
    pub(crate) fn nature_init(&mut self, map: &MapData) {
        for (i, a) in map.animals.iter().enumerate() {
            if let Some(species) = a {
                let p = map.grid.coord(i);
                let f = self.new_figure(255, Job::Helper, p, FigKind::Animal { species: *species });
                self.figures[f].step = Step::Go;
                let mut rng = self.rng_figure(f, Purpose::AnimalWalk);
                let d = 50 + rng.below(300);
                let key = self.schedule(d, Ev::Figure(f));
                self.figures[f].ev = Some(key);
            }
        }
    }

    fn nature_at(&mut self, p: MapCoord, delay: u32) {
        let key = self.schedule(delay, Ev::Nature(p));
        self.nature.due.insert(p, key.0);
    }

    pub(crate) fn nature_schedule_growth(&mut self, p: MapCoord) {
        let d = match self.world.obj(p) {
            NodeObj::Field(_) => FIELD_STAGE_GF,
            _ => TREE_STAGE_GF,
        };
        self.nature_at(p, d);
    }

    pub(crate) fn nature_schedule_fire_end(&mut self, p: MapCoord) {
        self.nature_at(p, FIRE_GF);
    }

    pub(crate) fn nature_schedule_sign_end(&mut self, p: MapCoord) {
        let mut rng = self.rng_node(p, Purpose::Nature);
        let d = SIGN_GF + rng.below(SIGN_RAND + 1);
        self.nature_at(p, d);
    }

    pub(crate) fn nature_event(&mut self, p: MapCoord) {
        if self.nature.due.get(&p) != Some(&self.gf) {
            return;
        }
        self.nature.due.remove(&p);
        match self.world.obj(p) {
            NodeObj::Tree { species, stage } if stage < 3 => {
                self.world.set_obj(p, NodeObj::Tree { species, stage: stage + 1 });
                if stage + 1 < 3 {
                    self.nature_at(p, TREE_STAGE_GF);
                }
            }
            NodeObj::Field(s) if s < 3 => {
                self.world.set_obj(p, NodeObj::Field(s + 1));
                let d = if s + 1 == 3 {
                    let mut rng = self.rng_node(p, Purpose::Nature);
                    FIELD_WITHER_GF + rng.below(1000)
                } else {
                    FIELD_STAGE_GF
                };
                self.nature_at(p, d);
            }
            NodeObj::Field(3) => {
                self.world.set_obj(p, NodeObj::Field(4));
                self.nature_at(p, FIELD_WITHERED_GF);
            }
            NodeObj::Field(_) | NodeObj::Fire | NodeObj::Sign(_) => self.world.set_obj(p, NodeObj::None),
            _ => {}
        }
    }

    /// Forests breed game (RTTR noTree::produce_animal_event: one tree in 20 makes an animal every
    /// 6000-8000 GF, so trees / 280 per 500 GF). Every 500 GF a sample of len / 100 nodes is tried;
    /// a grown tree in it spawns with 357 in 1000, which gives the same rate. No map-wide cap: a
    /// forest stops breeding once it holds `LOCAL_MAX` animals within `LOCAL_R`.
    pub(crate) fn nature_tick(&mut self) {
        const EVERY: u32 = 500;
        const LOCAL_R: u32 = 8;
        const LOCAL_MAX: usize = 6;
        if !self.gf.is_multiple_of(EVERY) || self.gf == 0 {
            return;
        }
        let g = self.world.grid();
        let mut animals: Vec<MapCoord> =
            self.figures.iter().filter(|(_, a)| matches!(a.kind, FigKind::Animal { .. })).map(|(_, a)| a.pos).collect();
        // a safety net far above what forests reach
        let cap = g.len() / 16;
        let mut rng = self.rng(0xa417, Purpose::Nature);
        let tries = (g.len() / 100).max(8);
        for _ in 0..tries {
            if animals.len() >= cap {
                break;
            }
            let p = g.coord(rng.below(g.len() as u32) as usize);
            if !matches!(self.world.obj(p), NodeObj::Tree { stage: 3, .. }) {
                continue;
            }
            if rng.below(1000) >= 357 {
                continue;
            }
            if animals.iter().filter(|a| g.distance(**a, p) <= LOCAL_R).count() >= LOCAL_MAX {
                continue;
            }
            let d = Direction::from_index(rng.below(6) as u8);
            let q = g.neighbor(p, d);
            if !self.world.node_walkable(q) || !matches!(self.world.obj(q), NodeObj::None) {
                continue;
            }
            let species = match self.world.landscape {
                rr_core::Landscape::Winter => rr_core::Animal::Stag,
                _ => [rr_core::Animal::Rabbit, rr_core::Animal::Fox, rr_core::Animal::Deer, rr_core::Animal::Stag]
                    [rng.below(4) as usize],
            };
            let f = self.new_figure(255, Job::Helper, q, FigKind::Animal { species });
            let key = self.schedule(50 + rng.below(200), Ev::Figure(f));
            self.figures[f].ev = Some(key);
            animals.push(q);
        }
    }

    pub(crate) fn animal_act(&mut self, f: FigId) {
        let pos = self.figures[f].pos;
        let mut rng = self.rng_figure(f, Purpose::AnimalWalk);
        let dirs: Vec<Direction> = Direction::ALL
            .iter()
            .copied()
            .filter(|d| {
                let q = self.world.grid().neighbor(pos, *d);
                self.world.edge_walkable(pos, *d)
                    && self.world.node_walkable(q)
                    && matches!(self.world.obj(q), NodeObj::None | NodeObj::Tree { .. })
                    && !self.world.is_on_road(q)
            })
            .collect();
        if !dirs.is_empty() && rng.chance(60) {
            let d = dirs[rng.below(dirs.len() as u32) as usize];
            self.figures[f].path = vec![d];
            self.figures[f].step = Step::Go;
            let dur = 30;
            self.figures[f].motion = crate::Motion::Walk { dir: d, start: self.gf, dur };
            self.figures[f].path.clear();
            let key = self.schedule(dur, Ev::Figure(f));
            self.figures[f].ev = Some(key);
        } else {
            let wait = 100 + rng.below(400);
            self.figures[f].motion = crate::Motion::Idle;
            let key = self.schedule(wait, Ev::Figure(f));
            self.figures[f].ev = Some(key);
        }
    }
}
