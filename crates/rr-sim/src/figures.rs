//! Figures (settlers): carriers, workers, builders, specialists. Each figure is a small state
//! machine driven by scheduled events: a walk step, a wait, or a work animation ending.

use crate::economy::{BUILD_STEP_GF, WareLoc};
use crate::military::SoldierTask;
use crate::{BState, BuildingId, Ev, FigId, FlagId, PlayerId, RoadId, Sim, SoundCue, WareId};
use rr_core::{Direction, MapCoord, Purpose, Resource};
use rr_rules::{BuildingType, Job, Ware};
use rr_world::{Blocking, NodeObj, RoadKind};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Walking one edge on flat ground takes 20 GF (RTTR `noFigure::StartWalking`).
pub const WALK_GF: u32 = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Motion {
    Idle,
    Walk { dir: Direction, start: u32, dur: u32 },
    Work { start: u32, dur: u32 },
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FigKind {
    Carrier {
        road: RoadId,
        pos: usize,
    },
    Worker {
        building: BuildingId,
    },
    Builder {
        site: BuildingId,
    },
    Planer {
        site: BuildingId,
    },
    Soldier(SoldierTask),
    Geologist {
        flag: FlagId,
        left: u8,
    },
    Scout {
        flag: FlagId,
        left: u8,
    },
    /// Walking to the nearest warehouse to be stored there.
    Home,
    /// No way home: wander a bit, then die.
    Lost {
        left: u16,
    },
    /// Wild animal (player 255).
    Animal {
        species: rr_core::Animal,
    },
}

/// What the figure does when its current motion ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Step {
    /// Follow `path`, then `arrive`.
    Go,
    /// Inside a building, waiting before work.
    Inside,
    /// Doing the job (in the building or outside).
    Working,
    /// Carrier states; `bool` = moving towards the road's `to` end.
    CIdle,
    CFetch(bool),
    CCarry(bool),
    CIntoBuilding(bool),
    CWaitRoom(bool),
    /// Worker carrying the product out to the flag.
    ToFlag,
    WaitRoom,
    /// Worker walking back into the building from the flag.
    BackIn,
    /// Outside job: walking to the target node.
    Out,
    /// Outside job: walking back home.
    Return,
    /// Builder/planer at the site.
    Site,
    /// Inside workers: the short pause after work before carrying the product out (RTTR wait2).
    Wait2,
    /// Inside workers: waiting for input wares; a delivery wakes them (RTTR
    /// WaitingForWaresOrProductionStopped). Last, so older saves keep their numbers.
    WaitWares,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Figure {
    pub player: PlayerId,
    pub job: Job,
    pub pos: MapCoord,
    pub motion: Motion,
    pub kind: FigKind,
    pub step: Step,
    /// Remaining directions, next one last (a stack).
    pub path: Vec<Direction>,
    /// The ware carried, by kind (for drawing); `ware` holds the item if it is a routed ware.
    pub carry: Option<Ware>,
    pub ware: Option<WareId>,
    /// Inside a building (not drawn).
    pub inside: Option<BuildingId>,
    /// Target node of an outside job.
    pub target: Option<MapCoord>,
    pub ev: Option<(u32, u64)>,
    pub hp: u8,
}

impl Figure {
    pub fn target_building(&self) -> Option<BuildingId> {
        match &self.kind {
            FigKind::Worker { building } => Some(*building),
            FigKind::Builder { site } | FigKind::Planer { site } => Some(*site),
            FigKind::Soldier(SoldierTask::ToBuilding(b)) => Some(*b),
            _ => None,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.inside.is_none()
    }
}

impl Sim {
    // ------------------------------------------------------------------ plumbing

    pub(crate) fn new_figure(&mut self, player: PlayerId, job: Job, pos: MapCoord, kind: FigKind) -> FigId {
        let hp = job.soldier_rank().map_or(0, |r| self.rules.military.hp[r as usize]);
        self.figures.insert(Figure {
            player,
            job,
            pos,
            motion: Motion::Idle,
            kind,
            step: Step::Go,
            path: Vec::new(),
            carry: None,
            ware: None,
            inside: None,
            target: None,
            ev: None,
            hp,
        })
    }

    /// Create a figure inside a warehouse; it leaves when the warehouse door is free.
    pub(crate) fn spawn_from_warehouse(&mut self, wh: BuildingId, job: Job, kind: FigKind) -> FigId {
        let (player, pos) = (self.buildings[wh].player, self.buildings[wh].pos);
        if let Some(inv) = self.buildings[wh].inventory.as_mut() {
            inv.jobs[job.index()] = inv.jobs[job.index()].saturating_sub(1);
        }
        let f = self.new_figure(player, job, pos, kind);
        self.figures[f].inside = Some(wh);
        self.buildings[wh].fig_queue.push(f);
        self.start_building_timer(wh, 1);
        f
    }

    fn set_event(&mut self, f: FigId, delay: u32) {
        if let Some(k) = self.figures[f].ev.take() {
            self.sched.cancel(k);
        }
        let key = self.schedule(delay, Ev::Figure(f));
        self.figures[f].ev = Some(key);
    }

    fn walk_duration(&self, from: MapCoord, d: Direction) -> u32 {
        let to = self.world.grid().neighbor(from, d);
        let diff = self.world.altitude(to) as i32 - self.world.altitude(from) as i32;
        match diff {
            1 => WALK_GF + WALK_GF / 2,
            2 | 3 => WALK_GF * 2,
            4.. => WALK_GF * 3,
            _ => WALK_GF,
        }
    }

    fn start_walk(&mut self, f: FigId, d: Direction) {
        let from = self.figures[f].pos;
        let dur = self.walk_duration(from, d);
        self.figures[f].motion = Motion::Walk { dir: d, start: self.gf, dur };
        self.set_event(f, dur);
    }

    fn start_work(&mut self, f: FigId, dur: u32) {
        self.figures[f].motion = Motion::Work { start: self.gf, dur };
        self.set_event(f, dur);
    }

    fn wait(&mut self, f: FigId, dur: u32) {
        self.figures[f].motion = Motion::Idle;
        self.set_event(f, dur);
    }

    /// Set a path (in walking order) and start walking it.
    fn go(&mut self, f: FigId, dirs: &[Direction], step: Step) {
        let fig = &mut self.figures[f];
        fig.path = dirs.iter().rev().copied().collect();
        fig.step = step;
        if fig.inside.is_some() && dirs.first() == Some(&Direction::SE) {
            fig.inside = None;
        }
        self.continue_path(f);
    }

    fn continue_path(&mut self, f: FigId) {
        match self.figures[f].path.pop() {
            Some(d) => self.start_walk(f, d),
            None => self.set_event(f, 1),
        }
    }

    /// Wake a figure that idles (carrier waiting for wares, builder waiting for material).
    /// An inside worker idling for wares: try again next frame.
    pub(crate) fn wake_worker(&mut self, f: FigId) {
        let Some(fig) = self.figures.get(f) else { return };
        // (only one waiting for wares: a rest between cycles runs its full time, QA round 4, #3)
        // (his wait1 first, then a look at the wares: RTTR TryToWork; fidelity #5)
        if fig.inside.is_some() && matches!(fig.motion, Motion::Idle) && fig.step == Step::WaitWares {
            let wait1 = self.rules.job(fig.job).wait1.max(1);
            self.figures[f].step = Step::Inside;
            self.wait(f, wait1);
        }
    }

    pub(crate) fn wake_figure(&mut self, f: FigId) {
        let Some(fig) = self.figures.get(f) else {
            return;
        };
        let idle = matches!(fig.motion, Motion::Idle) && fig.path.is_empty();
        let waiting = matches!(fig.step, Step::CIdle | Step::Site | Step::CWaitRoom(_) | Step::WaitRoom);
        if idle && waiting {
            self.set_event(f, 1);
        }
    }

    pub(crate) fn wake_carrier(&mut self, r: RoadId) {
        let Some(road) = self.roads.get(r) else { return };
        let (c, d) = (road.carrier, road.donkey);
        for f in [c, d].into_iter().flatten() {
            self.wake_figure(f);
        }
    }

    pub(crate) fn figure_leave_building(&mut self, f: FigId) {
        let Some(fig) = self.figures.get(f) else {
            return;
        };
        let Some(wh) = fig.inside else { return };
        let Some(whb) = self.buildings.get(wh) else {
            return;
        };
        let from_flag = whb.flag;
        let kind = fig.kind.clone();
        let path = match &kind {
            FigKind::Carrier { road, .. } => self.roads.get(*road).and_then(|r| {
                let mid = r.len() / 2;
                let mut p = self.road_walk(from_flag, r.from)?;
                p.extend_from_slice(&r.dirs[..mid]);
                Some(p)
            }),
            FigKind::Geologist { flag, .. } | FigKind::Scout { flag, .. } => {
                self.flags.contains(*flag).then(|| self.road_walk(from_flag, *flag)).flatten()
            }
            other => {
                let target = match other {
                    FigKind::Worker { building } => Some(*building),
                    FigKind::Builder { site } | FigKind::Planer { site } => Some(*site),
                    FigKind::Soldier(SoldierTask::ToBuilding(b)) => Some(*b),
                    // moving to another warehouse (e.g. a harbor that ships people over)
                    FigKind::Home => fig.target.and_then(|p| self.building_at(p)),
                    _ => None,
                };
                match target.and_then(|b| self.buildings.get(b)) {
                    Some(tb) => self.road_walk(from_flag, tb.flag).map(|mut p| {
                        p.push(Direction::NW);
                        p
                    }),
                    None => None,
                }
            }
        };
        let soldier_attack = matches!(kind, FigKind::Soldier(SoldierTask::Attack { .. } | SoldierTask::ToShip { .. }));
        if soldier_attack {
            self.soldier_leave_for_attack(f);
            return;
        }
        match path {
            Some(mut p) => {
                p.insert(0, Direction::SE);
                self.go(f, &p, Step::Go);
            }
            None => {
                // target unreachable now: go back in
                self.cancel_request_of(f);
                self.store_in_warehouse(f, wh);
            }
        }
    }

    /// Undo the "coming" marker of the figure's target.
    fn cancel_request_of(&mut self, f: FigId) {
        let kind = self.figures[f].kind.clone();
        match kind {
            FigKind::Carrier { road, .. } => {
                let donkey = self.figures[f].job == Job::PackDonkey;
                if let Some(r) = self.roads.get_mut(road) {
                    if donkey {
                        r.donkey_wanted = false;
                        if r.donkey == Some(f) {
                            r.donkey = None;
                        }
                    } else {
                        r.carrier_wanted = false;
                        if r.carrier == Some(f) {
                            r.carrier = None;
                        }
                    }
                }
            }
            FigKind::Worker { building } | FigKind::Builder { site: building } | FigKind::Planer { site: building } => {
                if let Some(b) = self.buildings.get_mut(building) {
                    b.worker_coming = false;
                    if b.worker == Some(f) {
                        b.worker = None;
                    }
                    if let BState::Site(s) = &mut b.state {
                        if s.builder == Some(f) {
                            s.builder = None;
                        }
                        if s.planer == Some(f) {
                            s.planer = None;
                        }
                    }
                }
            }
            FigKind::Soldier(SoldierTask::ToBuilding(b)) => {
                if let Some(b) = self.buildings.get_mut(b) {
                    b.soldiers_coming = b.soldiers_coming.saturating_sub(1);
                }
            }
            _ => {}
        }
    }

    /// The figure enters warehouse `wh` and becomes inventory.
    pub(crate) fn store_in_warehouse(&mut self, f: FigId, wh: BuildingId) {
        let Some(fig) = self.figures.remove(f) else {
            return;
        };
        if let Some(k) = fig.ev {
            self.sched.cancel(k);
        }
        if let Some(inv) = self.buildings.get_mut(wh).and_then(|b| b.inventory.as_mut()) {
            inv.jobs[fig.job.index()] += 1;
            if let Some(w) = fig.ware {
                if let Some(item) = self.wares.remove(w) {
                    inv.wares[item.kind.index()] += 1;
                }
            } else if let Some(c) = fig.carry {
                inv.wares[c.index()] += 1;
            }
        } else if let Some(w) = fig.ware {
            self.lose_ware(w);
        }
    }

    pub(crate) fn kill_figure(&mut self, f: FigId) {
        self.remove_figure(f, true);
    }

    /// A figure gone, screaming (killed) or not (a lost one that gave up: sound designer 2 #3,
    /// carriers dying alone in the wild were half the screams of a game).
    pub(crate) fn remove_figure(&mut self, f: FigId, scream: bool) {
        self.release_work_target(f);
        let Some(fig) = self.figures.remove(f) else {
            return;
        };
        if let Some(k) = fig.ev {
            self.sched.cancel(k);
        }
        if let Some(w) = fig.ware {
            self.lose_ware(w);
        }
        if scream {
            self.events.push(crate::Event::Sound { at: fig.pos, sound: SoundCue::Death });
        }
    }

    /// A worker on the way to a tree, stone, field or animal gives it up: others may take it,
    /// and a hunted animal walks on.
    fn release_work_target(&mut self, f: FigId) {
        let Some(fig) = self.figures.get(f) else {
            return;
        };
        let (FigKind::Worker { .. }, Some(q)) = (&fig.kind, fig.target) else {
            return;
        };
        let hunter = fig.job == Job::Hunter;
        self.figures[f].target = None;
        self.nature.reserved.remove(&q);
        if hunter {
            let animal = self
                .figures
                .iter()
                .find(|(_, a)| matches!(a.kind, FigKind::Animal { .. }) && a.pos == q && a.ev.is_none())
                .map(|(id, _)| id);
            if let Some(a) = animal {
                self.set_event(a, 1);
            }
        }
    }

    /// The figure's workplace/road vanished: walk to the nearest warehouse.
    pub(crate) fn figure_lost_home(&mut self, f: FigId) {
        let Some(fig) = self.figures.get(f) else {
            return;
        };
        if matches!(fig.kind, FigKind::Animal { .. } | FigKind::Home | FigKind::Lost { .. }) {
            if matches!(fig.kind, FigKind::Home) && matches!(fig.motion, Motion::Idle) && fig.ev.is_none() {
                self.set_event(f, 1);
            }
            return;
        }
        // still waiting in a warehouse's exit queue: straight back into its stock, for every
        // caller (net round 5, #1: a capture left the queue holding a figure that was stored
        // later, and saves of the next seconds didn't load; destroy_building did this already)
        let queued =
            self.figures[f].inside.filter(|wh| self.buildings.get(*wh).is_some_and(|w| w.fig_queue.contains(&f)));
        if let Some(wh) = queued
            && self.buildings[wh].inventory.is_some()
        {
            self.buildings[wh].fig_queue.retain(|x| *x != f);
            self.store_in_warehouse(f, wh);
            return;
        }
        // a boat carrier takes his boat home
        let boat = match self.figures[f].kind {
            FigKind::Carrier { road, .. } => {
                self.roads.get(road).is_some_and(|r| r.kind == RoadKind::Boat)
                    || self.figures[f].ware.is_none() && self.figures[f].carry == Some(Ware::Boat)
            }
            _ => false,
        };
        self.cancel_request_of(f);
        self.release_work_target(f);
        let fig = &self.figures[f];
        if let FigKind::Soldier(_) = fig.kind
            && let Some(b) = fig.inside
            && let Some(bd) = self.buildings.get_mut(b)
        {
            bd.soldiers.retain(|s| *s != f);
        }
        if let Some(w) = self.figures[f].ware.take() {
            self.lose_ware(w);
        }
        self.figures[f].carry = boat.then_some(Ware::Boat);
        // a figure inside a vanished building stands on its node now
        self.figures[f].inside = None;
        self.figures[f].kind = FigKind::Home;
        self.figures[f].path.clear();
        if matches!(self.figures[f].motion, Motion::Idle) {
            self.set_event(f, 1);
        }
        // a walking figure continues with `Home` at the next node
        self.figures[f].step = Step::Go;
    }

    fn go_home(&mut self, f: FigId) {
        let fig = &self.figures[f];
        let (player, pos) = (fig.player, fig.pos);
        // the warehouses that take this person in (warehouse settings), collecting ones first
        let homes = self.homes_for(player, fig.job);
        let collect: Vec<BuildingId> = homes.iter().filter(|h| h.2).map(|h| h.0).collect();
        let whs: Vec<(BuildingId, MapCoord)> = homes.iter().map(|h| (h.0, h.1)).collect();
        let g = self.world.grid();
        // at a flag of the road network (or in the building at it): home along the roads
        let at_flag = match self.flag_at(pos) {
            Some(fl) => Some((fl, None)),
            None => self.flag_at(g.neighbor(pos, Direction::SE)).map(|fl| (fl, Some(Direction::SE))),
        };
        if let Some((fl, step)) = at_flag
            && self.flags[fl].player == player
        {
            let routes = self.routes_to(fl);
            let best = whs
                .iter()
                .filter_map(|(id, p)| {
                    let wf = self.buildings[*id].flag;
                    let d = routes.get(wf.index()).map_or(u32::MAX, |r| r.0);
                    (d != u32::MAX).then_some((d, *id, *p, wf))
                })
                .min_by_key(|x| (!collect.contains(&x.1), x.0, x.1.index()));
            if let Some((_, _, wpos, wf)) = best
                && let Some(road) = self.road_walk(fl, wf)
            {
                let mut path: Vec<Direction> = step.into_iter().collect();
                path.extend(road);
                path.push(Direction::NW);
                self.figures[f].kind = FigKind::Home;
                self.figures[f].target = Some(wpos);
                self.go(f, &path, Step::Go);
                return;
            }
        }
        let mut order: Vec<(u32, BuildingId, MapCoord)> =
            whs.iter().map(|(id, p)| (g.distance(pos, *p), *id, *p)).collect();
        order.sort_by_key(|x| (!collect.contains(&x.1), x.0, x.1.index()));
        for (_, wh, wpos) in order.into_iter().take(3) {
            if pos == wpos {
                self.store_in_warehouse(f, wh);
                return;
            }
            // (not across the water: no path there, and each failed search walked 4000 nodes;
            // perf round 3, #3: 25-34 ms frames on the Mediterranean)
            if self.other_land(pos, wpos) {
                continue;
            }
            if let Some(path) = self.find_path(pos, wpos, 4000) {
                self.figures[f].kind = FigKind::Home;
                self.figures[f].target = Some(wpos);
                self.go(f, &path, Step::Go);
                return;
            }
        }
        // a lost figure stands on its node, never in a building: a carrier whose road went while
        // it stepped into a building arrived "inside" on the way home (the eval's audit, 44)
        self.figures[f].inside = None;
        self.figures[f].kind = FigKind::Lost { left: 20 };
        self.wander(f);
    }

    fn wander(&mut self, f: FigId) {
        let FigKind::Lost { left } = self.figures[f].kind else {
            return;
        };
        if left == 0 {
            self.remove_figure(f, false);
            return;
        }
        self.figures[f].kind = FigKind::Lost { left: left - 1 };
        let pos = self.figures[f].pos;
        let mut rng = self.rng_figure(f, Purpose::AnimalWalk);
        let dirs: Vec<Direction> = Direction::ALL.iter().copied().filter(|d| self.can_step(pos, *d)).collect();
        if dirs.is_empty() {
            self.remove_figure(f, false);
            return;
        }
        let d = dirs[rng.below(dirs.len() as u32) as usize];
        self.go(f, &[d], Step::Go);
    }

    fn can_step(&self, p: MapCoord, d: Direction) -> bool {
        let q = self.world.grid().neighbor(p, d);
        self.world.edge_walkable(p, d)
            && self.world.node_walkable(q)
            && matches!(self.world.obj(q).blocking(), Blocking::None | Blocking::Tree | Blocking::Flag)
    }

    /// A* over walkable nodes (trees and flags are passable). `to` may be a building node.
    pub fn find_path(&self, from: MapCoord, to: MapCoord, limit: usize) -> Option<Vec<Direction>> {
        self.find_path_bounded(from, to, limit, u32::MAX)
    }

    /// The length of the shortest walk from `from` to `to` if it is at most `max` steps: the
    /// search never looks past `max` (perf-2 #2: attack reach asked for a 6000-node search per
    /// post that couldn't get there). Exact: the hex distance never overestimates a walk.
    pub fn path_len_within(&self, from: MapCoord, to: MapCoord, max: u32) -> Option<usize> {
        self.find_path_bounded(from, to, 6000, max).map(|p| p.len())
    }

    fn find_path_bounded(&self, from: MapCoord, to: MapCoord, limit: usize, max: u32) -> Option<Vec<Direction>> {
        if from == to {
            return Some(Vec::new());
        }
        let g = self.world.grid();
        let n = g.len();
        // cost and the way back per node, valid only where stamped with this search's number:
        // nothing to clear between searches (a search can be several times a frame)
        let mut scratch = self.path_scratch.borrow_mut();
        let PathScratch { stamp, cost: costs, came: cames, search } = &mut *scratch;
        if stamp.len() != n {
            *stamp = vec![0; n];
            *costs = vec![0; n];
            *cames = vec![None; n];
            *search = 0;
        }
        *search = search.wrapping_add(1);
        if *search == 0 {
            stamp.fill(0);
            *search = 1;
        }
        let now = *search;
        let cost_of = |i: usize, stamp: &[u32], costs: &[u32]| if stamp[i] == now { costs[i] } else { u32::MAX };
        let mut heap = BinaryHeap::new();
        let fi = g.index(from);
        stamp[fi] = now;
        costs[fi] = 0;
        cames[fi] = None;
        heap.push(Reverse((g.distance(from, to), 0u32, fi)));
        let ti = g.index(to);
        let mut explored = 0;
        while let Some(Reverse((_, c, i))) = heap.pop() {
            if i == ti {
                let mut dirs = Vec::new();
                let mut cur = ti;
                while let Some((prev, d)) = cames[cur] {
                    dirs.push(d);
                    cur = prev;
                }
                dirs.reverse();
                return Some(dirs);
            }
            if c > cost_of(i, stamp, costs) {
                continue;
            }
            explored += 1;
            if explored > limit {
                return None;
            }
            let p = g.coord(i);
            for d in Direction::ALL {
                let q = g.neighbor(p, d);
                let qi = g.index(q);
                let ok = if qi == ti {
                    self.world.edge_walkable(p, d)
                } else {
                    self.can_step(p, d)
                        || self.world.is_on_road(q) && self.world.edge_walkable(p, d) && self.world.node_walkable(q)
                };
                if !ok {
                    continue;
                }
                let nc = c + 1;
                let h = g.distance(q, to);
                if nc.saturating_add(h) > max {
                    continue;
                }
                if nc < cost_of(qi, stamp, costs) {
                    stamp[qi] = now;
                    costs[qi] = nc;
                    cames[qi] = Some((i, d));
                    heap.push(Reverse((nc + h, nc, qi)));
                }
            }
        }
        None
    }

    // ------------------------------------------------------------------ the event

    pub(crate) fn figure_event(&mut self, f: FigId) {
        let Some(fig) = self.figures.get_mut(f) else {
            return;
        };
        fig.ev = None;
        let motion = fig.motion;
        fig.motion = Motion::Idle;
        if let Motion::Walk { dir, .. } = motion {
            let to = self.world.grid().neighbor(self.figures[f].pos, dir);
            self.figures[f].pos = to;
            if let FigKind::Carrier { road, pos } = self.figures[f].kind
                && let Some(r) = self.roads.get(road)
            {
                let nodes = r.nodes(self.world.grid());
                if let Some(i) = nodes.iter().position(|q| *q == to) {
                    self.figures[f].kind = FigKind::Carrier { road, pos: i };
                } else {
                    self.figures[f].kind = FigKind::Carrier { road, pos };
                }
            }
            // entering a building (the last step of a path) hides the figure; walking
            // across a building node placed after the path was planned does not
            if dir == Direction::NW
                && self.figures[f].path.is_empty()
                && !matches!(self.figures[f].kind, FigKind::Animal { .. } | FigKind::Lost { .. })
                && let Some(b) = self.building_at(to)
            {
                self.figures[f].inside = Some(b);
            }
            if !self.figures[f].path.is_empty() {
                // an attacker on its way may be met by a defender
                if let FigKind::Soldier(SoldierTask::Attack { target, .. }) = self.figures[f].kind
                    && self.try_hunt(f, target)
                {
                    return;
                }
                // the workplace may have vanished meanwhile: `Home` figures replan on each node
                self.continue_path(f);
                return;
            }
        }
        // a path set without starting to walk (soldiers leaving for an attack)
        if !matches!(motion, Motion::Walk { .. }) && !self.figures[f].path.is_empty() {
            self.continue_path(f);
            return;
        }
        let fig = &self.figures[f];
        if fig.kind == FigKind::Home && fig.path.is_empty() {
            // arrived home or needs a path
            if let Some(t) = fig.target
                && t == fig.pos
                && let Some(wh) = self.building_at(t)
                && self.buildings[wh].inventory.is_some()
            {
                self.store_in_warehouse(f, wh);
                return;
            }
            self.go_home(f);
            return;
        }
        match fig.kind.clone() {
            FigKind::Lost { .. } => self.wander(f),
            FigKind::Carrier { road, pos } => self.carrier_act(f, road, pos),
            FigKind::Worker { building } => self.worker_act(f, building),
            FigKind::Builder { site } => self.builder_act(f, site),
            FigKind::Planer { site } => self.planer_act(f, site),
            FigKind::Soldier(task) => self.soldier_act(f, task),
            FigKind::Geologist { flag, left } => self.geologist_act(f, flag, left),
            FigKind::Scout { flag, left } => self.scout_act(f, flag, left),
            FigKind::Animal { .. } => self.animal_act(f),
            FigKind::Home => self.go_home(f),
        }
    }

    // ------------------------------------------------------------------ carriers

    pub(crate) fn on_road_created(&mut self, _r: RoadId) {}

    pub(crate) fn on_road_split(&mut self, r: RoadId, r2: RoadId) {
        // the carrier (and the donkey) stays on whichever part it stands on; the other part
        // gets a new one
        for donkey in [false, true] {
            let holder = if donkey { self.roads[r].donkey } else { self.roads[r].carrier };
            let Some(c) = holder else { continue };
            let Some(pos) = self.figures.get(c).map(|f| f.pos) else { continue };
            let on_second = self.roads[r2].nodes(self.world.grid())[1..].contains(&pos)
                && !self.roads[r].nodes(self.world.grid()).contains(&pos);
            if on_second {
                if donkey {
                    self.roads[r].donkey = None;
                    self.roads[r].donkey_wanted = false;
                    self.roads[r2].donkey = Some(c);
                } else {
                    self.roads[r].carrier = None;
                    self.roads[r].carrier_wanted = false;
                    self.roads[r2].carrier = Some(c);
                }
                self.figures[c].kind = FigKind::Carrier { road: r2, pos: 0 };
            }
            // one carrying a ware towards the far end delivers it to the new middle flag
            self.wake_figure(c);
        }
    }

    pub(crate) fn on_road_destroyed(&mut self, road: &crate::Road) {
        // carriers of this road (or on their way to it) go home
        let ids: Vec<FigId> = self
            .figures
            .iter()
            .filter(|(_, f)| matches!(f.kind, FigKind::Carrier { road: r, .. } if self.roads.get(r).is_none()))
            .map(|(id, _)| id)
            .collect();
        let boat = road.kind == RoadKind::Boat;
        for f in ids {
            if boat {
                // a ware in the boat is lost; the boat comes home
                if let Some(w) = self.figures[f].ware.take() {
                    self.lose_ware(w);
                }
                self.figures[f].carry = Some(Ware::Boat);
            }
            let queued =
                self.figures[f].inside.filter(|b| self.buildings.get(*b).is_some_and(|b| b.inventory.is_some()));
            if let Some(wh) = queued {
                // still in the warehouse queue: drop the request
                if let Some(b) = self.buildings.get_mut(wh) {
                    b.fig_queue.retain(|x| *x != f);
                }
                self.store_in_warehouse(f, wh);
            } else {
                self.figure_lost_home(f);
                if boat {
                    self.row_ashore(f, road);
                }
            }
        }
    }

    /// A boat carrier on a removed boat road first rows to its nearer end, then walks home.
    fn row_ashore(&mut self, f: FigId, road: &crate::Road) {
        let g = self.world.grid();
        let fig = &self.figures[f];
        let at = match fig.motion {
            Motion::Walk { dir, .. } => g.neighbor(fig.pos, dir),
            _ => fig.pos,
        };
        let nodes = road.nodes(g);
        let Some(i) = nodes.iter().position(|q| *q == at) else {
            return;
        };
        let dirs: Vec<Direction> = if i <= road.dirs.len() / 2 {
            road.dirs[..i].iter().rev().map(|d| d.opposite()).collect()
        } else {
            road.dirs[i..].to_vec()
        };
        self.figures[f].path = dirs.into_iter().rev().collect();
    }

    /// Is there a ware at flag `fl` that wants to travel along road `r`?
    fn ware_for_road(&self, fl: FlagId, r: RoadId) -> Option<WareId> {
        let road = &self.roads[r];
        let dir = road.dir_at(fl);
        let order = &self.players[road.player as usize].settings.transport_order;
        self.flags[fl]
            .wares
            .iter()
            .copied()
            .filter(|w| self.wares[*w].next == Some(dir))
            .min_by_key(|w| (order.iter().position(|k| *k == self.wares[*w].kind).unwrap_or(99), w.index()))
    }

    fn carrier_act(&mut self, f: FigId, road: RoadId, pos: usize) {
        let Some(r) = self.roads.get(road).cloned() else {
            self.figure_lost_home(f);
            return;
        };
        let step = self.figures[f].step;
        let len = r.len();
        // A hand-made save can carry a carrier index past the end of a road that was shortened
        // since (splitting a road truncates the first half). The steps below index `dirs[pos]`
        // or `dirs[pos - 1]`, so the index is kept inside the road and the figure is sent to the
        // repair below, which puts it back on the node it really stands on.
        let indexed = pos <= len;
        let pos = pos.min(len);
        let g = self.world.grid();
        let nodes = r.nodes(g);
        let here = self.figures[f].pos;
        if nodes.get(pos) != Some(&here) && !matches!(step, Step::CIntoBuilding(_)) {
            // not on the road (still arriving) - should not happen after `Go`; walk to the road
            if indexed && step == Step::Go && here == nodes[len / 2] {
                // fallthrough below
            } else if let Some(i) = nodes.iter().position(|q| *q == here) {
                self.figures[f].kind = FigKind::Carrier { road, pos: i };
                return self.carrier_act(f, road, i);
            } else {
                // the road was split while we walked: serve the part we stand on if it is free
                let player = self.figures[f].player;
                let donkey = self.figures[f].job == Job::PackDonkey;
                let other = self.road_at(here).filter(|_| !donkey).filter(|o| {
                    let ro = &self.roads[*o];
                    ro.player == player && (ro.carrier.is_none() || ro.carrier == Some(f))
                });
                if let Some(o) = other {
                    if self.roads[road].carrier == Some(f) {
                        self.roads[road].carrier = None;
                    }
                    self.roads[road].carrier_wanted = false;
                    // (a road that doesn't pass here can only come from a damaged save: give up
                    // rather than try again for ever)
                    let Some(i) = self.roads[o].nodes(g).iter().position(|q| *q == here) else {
                        self.figure_lost_home(f);
                        return;
                    };
                    // take the road over now (a carrier already on its way there turns back)
                    self.roads[o].carrier = Some(f);
                    self.roads[o].carrier_wanted = false;
                    self.figures[f].kind = FigKind::Carrier { road: o, pos: i };
                    return self.carrier_act(f, o, i);
                }
                self.figure_lost_home(f);
                return;
            }
        }
        match step {
            Step::Go => {
                // arrived on the road; if someone else got there first, go home again
                let donkey = self.figures[f].job == Job::PackDonkey;
                let holder = if donkey { r.donkey } else { r.carrier };
                if holder.is_some_and(|c| c != f && self.figures.contains(c)) {
                    self.figure_lost_home(f);
                    return;
                }
                if donkey {
                    self.roads[road].donkey = Some(f);
                    self.roads[road].donkey_wanted = false;
                } else {
                    self.roads[road].carrier = Some(f);
                    self.roads[road].carrier_wanted = false;
                    // a boat carrier puts his boat in the water
                    if self.figures[f].ware.is_none() {
                        self.figures[f].carry = None;
                    }
                }
                self.players[r.player as usize].road_epoch += 1;
                self.route_cache.borrow_mut().clear();
                self.figures[f].step = Step::CIdle;
                self.carrier_idle(f, road, pos);
            }
            Step::CIdle => self.carrier_idle(f, road, pos),
            Step::CFetch(to_end) => {
                let at_end = if to_end { pos == len } else { pos == 0 };
                if !at_end {
                    self.carrier_step_along(f, &r, pos, to_end);
                    return;
                }
                let fl = if to_end { r.to } else { r.from };
                match self.ware_for_road(fl, road) {
                    Some(w) => {
                        self.flags[fl].wares.retain(|x| *x != w);
                        self.wares[w].loc = WareLoc::Carried(f);
                        self.figures[f].ware = Some(w);
                        self.figures[f].carry = Some(self.wares[w].kind);
                        self.figures[f].step = Step::CCarry(!to_end);
                        self.carrier_step_along(f, &r, pos, !to_end);
                    }
                    None => {
                        self.figures[f].step = Step::CIdle;
                        self.carrier_idle(f, road, pos);
                    }
                }
            }
            Step::CCarry(to_end) => {
                let at_end = if to_end { pos == len } else { pos == 0 };
                if !at_end {
                    self.carrier_step_along(f, &r, pos, to_end);
                    return;
                }
                let fl = if to_end { r.to } else { r.from };
                self.carrier_drop(f, road, fl, to_end);
            }
            Step::CWaitRoom(to_end) => {
                let fl = if to_end { r.to } else { r.from };
                self.carrier_drop(f, road, fl, to_end);
            }
            Step::CIntoBuilding(to_end) => {
                // inside the goal building: deliver, then walk out again
                if self.figures[f].inside.is_some() {
                    if let Some(w) = self.figures[f].ware.take() {
                        let goal = self.wares.get(w).and_then(|x| x.goal);
                        self.figures[f].carry = None;
                        match goal {
                            Some(b) if self.buildings.contains(b) => self.deliver_ware(w, b),
                            _ => self.lose_ware(w),
                        }
                    }
                    self.figures[f].inside = None;
                    self.go(f, &[Direction::SE], Step::CIntoBuilding(to_end));
                    return;
                }
                let flag_pos = nodes[if to_end { len } else { 0 }];
                if here != flag_pos {
                    // the building burnt while we walked in: the ware is lost with it
                    if let Some(w) = self.figures[f].ware.take() {
                        self.lose_ware(w);
                    }
                    self.figures[f].carry = None;
                    self.go(f, &[Direction::SE], Step::CIntoBuilding(to_end));
                    return;
                }
                // back at the flag
                self.figures[f].kind = FigKind::Carrier { road, pos: if to_end { len } else { 0 } };
                self.figures[f].step = Step::CIdle;
                self.carrier_idle(f, road, if to_end { len } else { 0 });
            }
            _ => {
                self.figures[f].step = Step::CIdle;
                self.carrier_idle(f, road, pos);
            }
        }
    }

    fn carrier_step_along(&mut self, f: FigId, r: &crate::Road, pos: usize, to_end: bool) {
        let d = if to_end { r.dirs[pos] } else { r.dirs[pos - 1].opposite() };
        self.start_walk(f, d);
    }

    fn carrier_idle(&mut self, f: FigId, road: RoadId, pos: usize) {
        let r = self.roads[road].clone();
        let len = r.len();
        let at_from = self.ware_for_road(r.from, road).is_some();
        let at_to = self.ware_for_road(r.to, road).is_some();
        let target = match (at_from, at_to) {
            (true, true) => Some(pos * 2 > len),
            (true, false) => Some(false),
            (false, true) => Some(true),
            (false, false) => None,
        };
        match target {
            Some(to_end) => {
                self.figures[f].step = Step::CFetch(to_end);
                let at_end = if to_end { pos == len } else { pos == 0 };
                if at_end {
                    self.carrier_act(f, road, pos);
                } else {
                    self.carrier_step_along(f, &r, pos, to_end);
                }
            }
            None => {
                // walk back to the middle and wait there
                let mid = len / 2;
                self.figures[f].step = Step::CIdle;
                if pos == mid {
                    self.figures[f].motion = Motion::Idle;
                    // idle: woken by `wake_carrier`
                } else {
                    let d = if pos < mid { r.dirs[pos] } else { r.dirs[pos - 1].opposite() };
                    self.start_walk(f, d);
                }
            }
        }
    }

    /// Arrived at flag `fl` with a ware.
    fn carrier_drop(&mut self, f: FigId, road: RoadId, fl: FlagId, to_end: bool) {
        let Some(w) = self.figures[f].ware else {
            self.figures[f].step = Step::CIdle;
            let pos = if to_end { self.roads[road].len() } else { 0 };
            self.carrier_idle(f, road, pos);
            return;
        };
        let goal = self.wares.get(w).and_then(|x| x.goal);
        let goal_here = goal.and_then(|b| self.buildings.get(b)).is_some_and(|b| b.flag == fl);
        if goal_here {
            self.figures[f].step = Step::CIntoBuilding(to_end);
            self.go(f, &[Direction::NW], Step::CIntoBuilding(to_end));
            return;
        }
        let swap = self.ware_for_road(fl, road);
        if self.flag_has_room(fl) || swap.is_some() {
            self.figures[f].ware = None;
            self.figures[f].carry = None;
            if let Some(s) = swap {
                // take the waiting ware first so the drop can't fill the last slot
                self.flags[fl].wares.retain(|x| *x != s);
                self.wares[s].loc = WareLoc::Carried(f);
                self.put_ware_at_flag(w, fl);
                self.figures[f].ware = Some(s);
                self.figures[f].carry = Some(self.wares[s].kind);
                self.figures[f].step = Step::CCarry(!to_end);
                let r = self.roads[road].clone();
                let pos = if to_end { r.len() } else { 0 };
                self.carrier_step_along(f, &r, pos, !to_end);
            } else {
                self.put_ware_at_flag(w, fl);
                self.figures[f].step = Step::CIdle;
                let pos = if to_end { self.roads[road].len() } else { 0 };
                self.carrier_idle(f, road, pos);
            }
        } else {
            self.figures[f].step = Step::CWaitRoom(to_end);
            self.wait(f, WALK_GF);
        }
    }

    // ------------------------------------------------------------------ production workers

    fn worker_act(&mut self, f: FigId, b: BuildingId) {
        let Some(bd) = self.buildings.get(b) else {
            self.figure_lost_home(f);
            return;
        };
        let step = self.figures[f].step;
        let kind = bd.kind;
        let job = self.figures[f].job;
        let jd = self.rules.job(job).clone();
        match step {
            Step::Go => {
                // arrived at the workplace
                self.buildings[b].worker = Some(f);
                self.buildings[b].worker_coming = false;
                self.figures[f].inside = Some(b);
                self.figures[f].step = Step::Inside;
                self.wait(f, jd.wait1.max(1));
            }
            Step::Inside | Step::WaitWares => self.worker_try_work(f, b),
            Step::Working => self.worker_work_done(f, b),
            Step::Wait2 => self.worker_carry_out(f, b),
            Step::ToFlag => {
                let flag = self.buildings[b].flag;
                self.worker_put_down(f, b, flag);
            }
            Step::WaitRoom => {
                let flag = self.buildings[b].flag;
                self.worker_put_down(f, b, flag);
            }
            Step::BackIn => {
                self.figures[f].inside = Some(b);
                self.figures[f].step = Step::Inside;
                let wait = if kind == BuildingType::Catapult { 50 } else { jd.wait1.max(1) };
                self.wait(f, wait);
            }
            Step::Out => {
                // at the target node: do the outside work
                self.figures[f].step = Step::Working;
                let dur = match job {
                    Job::Hunter => 100,
                    // a sea ship's step is short work on the shore (RTTR nofShipWright
                    // WORKING_TIME_SHIPS; fidelity #3: it took a boat's 1 250 GF, ships 4x slower)
                    Job::Shipwright if self.ship_mode(b) => crate::seafaring::SHIP_STEP_WORK,
                    Job::Farmer | Job::Woodcutter | Job::Stonemason | Job::Fisher | Job::Forester => jd.work.max(20),
                    _ => jd.work.max(20),
                };
                if let Some(sound) = crate::work_cue(job, true) {
                    self.events.push(crate::Event::Sound { at: self.figures[f].pos, sound });
                }
                self.start_work(f, dur);
            }
            Step::Return => {
                // back inside the building with (maybe) a product
                self.figures[f].inside = Some(b);
                if self.figures[f].carry.is_some() {
                    self.worker_carry_out(f, b);
                } else {
                    self.figures[f].step = Step::Inside;
                    self.wait(f, jd.wait1.max(1));
                }
            }
            _ => {
                self.figures[f].step = Step::Inside;
                self.wait(f, jd.wait1.max(1));
            }
        }
    }

    fn is_outside_job(job: Job) -> bool {
        matches!(job, Job::Woodcutter | Job::Forester | Job::Stonemason | Job::Fisher | Job::Hunter | Job::Farmer)
    }

    /// Outside jobs, plus the shipwright once his owner has a harbor (he then builds ships on the
    /// shore instead of boats in the yard, like RTTR's ship mode).
    fn works_outside(&self, job: Job, b: BuildingId) -> bool {
        Self::is_outside_job(job) || job == Job::Shipwright && self.ship_mode(b)
    }

    fn ship_mode(&self, b: BuildingId) -> bool {
        let bd = &self.buildings[b];
        // the player's choice; ships also need a harbor on the same sea
        bd.build_ships && self.ships_from(bd.player, self.flags[bd.flag].pos)
    }

    /// Would a shipyard whose flag stands at `flag` build ships for `player`: is there sea within
    /// the shipwright's reach that one of the player's harbors is on?
    pub fn ships_from(&self, player: PlayerId, flag: MapCoord) -> bool {
        let g = self.world.grid();
        let near_sea: Vec<u16> = g
            .within(flag, crate::seafaring::SHIPWRIGHT_RADIUS + 2)
            .map(|q| self.sea_at_pub(q))
            .filter(|s| *s != 0)
            .collect();
        !near_sea.is_empty()
            && self.buildings.iter().any(|(_, h)| {
                h.player == player
                    && h.kind == rr_rules::BuildingType::Harbor
                    && h.is_ready()
                    && self.harbor_sea(h.pos).is_some_and(|s| near_sea.contains(&s))
            })
    }

    fn record_attempt(&mut self, b: BuildingId, ok: bool) {
        let bd = &mut self.buildings[b];
        bd.history = (bd.history << 1) | ok as u16;
        bd.attempts = next_attempt(bd.attempts);
        // (a mine's ore only dwindles: told it runs low, it stays told)
        if ok && mine_resource(bd.kind).is_none() {
            bd.exhausted = false;
        }
    }

    /// Take one ware of input `i`; running low (under 2), order more at once (RTTR nobUsual).
    fn use_input(&mut self, b: BuildingId, i: usize) {
        let bd = &mut self.buildings[b];
        bd.stock[i] -= 1;
        if bd.stock[i] < 2 {
            bd.order_input = i as u8;
            bd.order_at = self.gf;
        }
    }

    fn worker_try_work(&mut self, f: FigId, b: BuildingId) {
        let job = self.figures[f].job;
        let jd = self.rules.job(job).clone();
        let bd = &self.buildings[b];
        let def = self.rules.building(bd.kind).clone();
        if bd.stopped {
            self.wait(f, 200);
            return;
        }
        if bd.kind == BuildingType::LookoutTower {
            // the scout keeps watch from up there (the tower's sight): no work to do or count
            // (QA round 6, #5: an empty cycle for ever, and the tower at 0 %)
            self.figures[f].motion = Motion::Idle;
            return;
        }
        if self.works_outside(job, b) {
            if job == Job::Shipwright && self.buildings[b].stock[0] == 0 {
                // no board for the next build step
                self.wait(f, 200);
                return;
            }
            match self.find_outside_target(f, b) {
                Some((target, path)) => {
                    if job == Job::Shipwright {
                        self.buildings[b].stock[0] -= 1;
                    }
                    self.figures[f].target = Some(target);
                    self.figures[f].inside = None;
                    self.go(f, &path, Step::Out);
                }
                None => {
                    self.record_attempt(b, false);
                    // eight misses in a row: tell the player once (until there is work again)
                    let bd = &mut self.buildings[b];
                    if bd.attempts >= 8 && bd.history & 0xff == 0 && !bd.exhausted {
                        bd.exhausted = true;
                        let (player, at, kind) = (bd.player, bd.pos, bd.kind);
                        self.events.push(crate::Event::NothingInReach { player, at, kind });
                    }
                    self.wait(f, jd.wait1.max(100));
                }
            }
            return;
        }
        if def.kind_is_catapult() {
            self.catapult_try(f, b);
            return;
        }
        // mines need the resource in the mountain; an exhausted mine eats nothing
        if let Some(res) = mine_resource(self.buildings[b].kind) {
            let pos = self.buildings[b].pos;
            if !self.can_dig(pos, res) {
                self.record_attempt(b, false);
                if !self.buildings[b].exhausted {
                    self.buildings[b].exhausted = true;
                    let player = self.buildings[b].player;
                    self.events.push(crate::Event::MineExhausted { player, at: pos });
                }
                self.wait(f, jd.wait1.max(100));
                return;
            }
        }
        let bd = &self.buildings[b];
        // inside workers: inputs
        let n = def.inputs.len();
        if n > 0 {
            if def.use_one_each {
                if (0..n).any(|i| bd.stock[i] == 0) {
                    // waiting for wares is time not worked: a miss per work cycle (RTTR measures
                    // productivity by time); a delivery wakes the worker at once
                    self.record_attempt(b, false);
                    self.figures[f].step = Step::WaitWares;
                    self.wait(f, jd.work.max(100));
                    return;
                }
                for i in 0..n {
                    self.use_input(b, i);
                }
            } else {
                // use the input we have most of
                let Some(i) = (0..n).filter(|i| bd.stock[*i] > 0).max_by_key(|i| (bd.stock[*i], Reverse(*i))) else {
                    // waiting for wares is time not worked: a miss per work cycle (RTTR measures
                    // productivity by time); a delivery wakes the worker at once
                    self.record_attempt(b, false);
                    self.figures[f].step = Step::WaitWares;
                    self.wait(f, jd.work.max(100));
                    return;
                };
                self.use_input(b, i);
            }
        }
        self.figures[f].step = Step::Working;
        if let Some(sound) = crate::work_cue(job, false) {
            self.events.push(crate::Event::Sound { at: self.buildings[b].pos, sound });
        }
        self.start_work(f, jd.work.max(1));
    }

    fn worker_work_done(&mut self, f: FigId, b: BuildingId) {
        let job = self.figures[f].job;
        let kind = self.buildings[b].kind;
        let def = self.rules.building(kind).clone();
        if self.works_outside(job, b) {
            let target = self.figures[f].target.unwrap_or(self.figures[f].pos);
            let product = self.outside_work_result(f, b, target);
            self.figures[f].target = None;
            self.figures[f].carry = product;
            self.record_attempt(b, product.is_some() || matches!(job, Job::Forester | Job::Shipwright));
            let home = self.buildings[b].pos;
            let pos = self.figures[f].pos;
            match self.find_path(pos, home, 4000) {
                Some(p) => self.go(f, &p, Step::Return),
                None => self.figure_lost_home(f),
            }
            return;
        }
        let output = if let Some(res) = mine_resource(kind) {
            self.mine_yield(f, b, res).then_some(def.output).flatten()
        } else if kind == BuildingType::Metalworks {
            self.choose_tool(self.buildings[b].player, f)
        } else if kind == BuildingType::Armory {
            // swords and shields in pairs (RTTR nofArmorer alternates): whichever the player has
            // made fewer of (counting idle waits as turns left one weapon in ten unpaired;
            // veteran round 8, #5)
            let made = &self.players[self.buildings[b].player as usize].stats.produced_by_ware;
            Some(if made[Ware::Sword.index()] <= made[Ware::Shield.index()] { Ware::Sword } else { Ware::Shield })
        } else if kind == BuildingType::DonkeyBreeder {
            // a new donkey leaves the breeder and walks to a warehouse along the roads
            let (player, pos) = (self.buildings[b].player, self.buildings[b].pos);
            let d = self.new_figure(player, Job::PackDonkey, pos, FigKind::Home);
            self.set_event(d, 1);
            None
        } else {
            def.output
        };
        self.record_attempt(b, output.is_some() || kind == BuildingType::DonkeyBreeder);
        self.figures[f].carry = output;
        if output.is_some() {
            self.figures[f].step = Step::Wait2;
            let w = self.rules.job(job).wait2.max(1);
            self.wait(f, w);
        } else {
            self.figures[f].step = Step::Inside;
            // (a donkey bred: the breeder still has his wait2 before the next wait1, as RTTR's
            // workman after every work; fidelity #8: 648 GF a donkey instead of 853)
            let jd = self.rules.job(job);
            let w = if kind == BuildingType::DonkeyBreeder { jd.wait2 + jd.wait1 } else { jd.wait1 };
            self.wait(f, w.max(1));
        }
    }

    fn worker_carry_out(&mut self, f: FigId, _b: BuildingId) {
        self.figures[f].inside = None;
        self.go(f, &[Direction::SE], Step::ToFlag);
    }

    fn worker_put_down(&mut self, f: FigId, b: BuildingId, flag: FlagId) {
        if !self.flag_has_room(flag) {
            self.figures[f].step = Step::WaitRoom;
            self.wait(f, WALK_GF * 2);
            return;
        }
        if let Some(kind) = self.figures[f].carry.take() {
            let player = self.buildings[b].player;
            let w =
                self.wares.insert(crate::WareItem { kind, player, loc: WareLoc::Building(b), goal: None, next: None });
            let st = &mut self.players[player as usize].stats;
            st.produced += 1;
            st.produced_by_ware[kind.index()] += 1;
            self.put_ware_at_flag(w, flag);
        }
        self.go(f, &[Direction::NW], Step::BackIn);
    }

    fn choose_tool(&self, player: PlayerId, f: FigId) -> Option<Ware> {
        let s = &self.players[player as usize].settings.tools;
        let total: u32 = s.iter().map(|t| t.1 as u32).sum();
        let mut rng = self.rng_figure(f, Purpose::Production);
        // all sliders at 0: a random tool, as the original (not inputs used up for nothing)
        if total == 0 {
            return (!s.is_empty()).then(|| s[rng.below(s.len() as u32) as usize].0);
        }
        let mut pick = rng.below(total);
        for (w, v) in s {
            if pick < *v as u32 {
                return Some(*w);
            }
            pick -= *v as u32;
        }
        None
    }

    fn can_dig(&self, pos: MapCoord, res: fn(Resource) -> Option<u8>) -> bool {
        let g = self.world.grid();
        g.within(pos, 2).any(|q| res(self.world.resources[g.index(q)]).is_some_and(|a| a > 0))
    }

    /// The ore left in a mine's reach.
    fn mine_left(&self, pos: MapCoord, res: fn(Resource) -> Option<u8>) -> u32 {
        let g = self.world.grid();
        g.within(pos, 2).filter_map(|q| res(self.world.resources[g.index(q)])).map(u32::from).sum()
    }

    /// A mine's work round ends (its food is eaten): does it bring something up? Mines dwindle
    /// instead of stopping (RTTR's "S4-like exhaustion" option; owner's playtest 2026-10-27):
    /// the chance is the ore left in reach × 5 %, at most 100 %, and each find takes one unit,
    /// but every node keeps its last. Under a third, the player is told once that it runs low.
    fn mine_yield(&mut self, f: FigId, b: BuildingId, res: fn(Resource) -> Option<u8>) -> bool {
        let pos = self.buildings[b].pos;
        let chance = (self.mine_left(pos, res) * MINE_CHANCE_PER_UNIT).min(100);
        if chance < MINE_LOW_PERCENT && !self.buildings[b].exhausted {
            self.buildings[b].exhausted = true;
            let player = self.buildings[b].player;
            self.events.push(crate::Event::MineRunningLow { player, at: pos });
        }
        if self.rng_figure(f, Purpose::Production).below(100) >= chance {
            return false;
        }
        let g = self.world.grid();
        for q in g.within(pos, 2) {
            let i = g.index(q);
            if let Some(a) = res(self.world.resources[i])
                && a > MINE_KEEPS
            {
                self.world.resources[i] = with_amount(self.world.resources[i], a - 1);
                break;
            }
        }
        true
    }

    /// How far an outside worker of `job` goes from home.
    fn job_radius(&self, job: Job) -> Option<u16> {
        let e = &self.rules.economy;
        Some(match job {
            Job::Woodcutter => e.woodcutter_radius,
            Job::Forester => e.forester_radius,
            Job::Stonemason => e.stonemason_radius,
            Job::Fisher => e.fisher_radius,
            Job::Hunter => e.hunter_radius,
            Job::Farmer => e.farmer_radius,
            Job::Shipwright => crate::seafaring::SHIPWRIGHT_RADIUS,
            _ => return None,
        })
    }

    /// The area a building's worker works in, for those who work outside (shown when placing).
    pub fn work_radius(&self, kind: BuildingType) -> Option<u16> {
        if kind == BuildingType::Catapult {
            return Some(crate::military::CATAPULT_RANGE);
        }
        self.job_radius(self.rules.building(kind).worker?)
    }

    /// Where an outside worker goes next, and the path there.
    fn find_outside_target(&mut self, f: FigId, b: BuildingId) -> Option<(MapCoord, Vec<Direction>)> {
        let job = self.figures[f].job;
        let home = self.buildings[b].pos;
        let flag_pos = self.flags[self.buildings[b].flag].pos;
        let g = self.world.grid();
        let radius = self.job_radius(job).unwrap_or(4);
        // only the game in reach (a map holds hundreds of animals)
        let animals: Vec<MapCoord> = if job == Job::Hunter {
            self.figures
                .iter()
                .filter(|(_, a)| matches!(a.kind, FigKind::Animal { .. }) && g.distance(a.pos, home) <= radius as u32)
                .map(|(_, a)| a.pos)
                .collect()
        } else {
            Vec::new()
        };
        let near_warehouse: Vec<MapCoord> = if job == Job::Forester {
            self.buildings.iter().filter(|(_, b)| b.kind.is_warehouse()).map(|(_, b)| b.pos).collect()
        } else {
            Vec::new()
        };
        let mut cands: Vec<MapCoord> = Vec::new();
        let mut harvest_field = false;
        for q in g.within(home, radius) {
            if self.nature.reserved.contains(&q) {
                continue;
            }
            let ok = match job {
                Job::Woodcutter => matches!(self.world.obj(q), NodeObj::Tree { stage: 3, .. }),
                Job::Stonemason => matches!(self.world.obj(q), NodeObj::Granite { .. }),
                // (not right round a warehouse: the castle's building land stays free; new-player
                // round 7, #8)
                Job::Forester => self.can_plant_tree(q) && !near_warehouse.iter().any(|w| g.distance(*w, q) <= 3),
                Job::Fisher => self.fishing_spot(q),
                Job::Hunter => animals.contains(&q),
                Job::Farmer => {
                    if matches!(self.world.obj(q), NodeObj::Field(3)) {
                        harvest_field = true;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            };
            if ok {
                cands.push(q);
            }
        }
        if job == Job::Shipwright {
            // continue a ship on the stocks, or start one on the shore
            let around: Vec<MapCoord> = g.within(flag_pos, radius).collect();
            let sites: Vec<MapCoord> =
                around.iter().copied().filter(|q| matches!(self.world.obj(*q), NodeObj::ShipSite(_))).collect();
            cands = if sites.is_empty() {
                around
                    .into_iter()
                    .filter(|q| {
                        self.world.obj(*q) == NodeObj::None
                            && !self.world.is_on_road(*q)
                            && self.world.node_walkable(*q)
                            && g.within(*q, 2).any(|n| self.sea_at_pub(n) != 0)
                    })
                    .collect()
            } else {
                sites
            };
        }
        if job == Job::Farmer {
            if harvest_field {
                cands.retain(|q| matches!(self.world.obj(*q), NodeObj::Field(3)));
            } else {
                cands =
                    g.within(home, radius).filter(|q| self.can_sow(*q) && !self.nature.reserved.contains(q)).collect();
            }
        }
        if cands.is_empty() {
            return None;
        }
        // RTTR nofFarmhand: look ring by ring outward and stop one ring after the first with a
        // spot (the stonemason at the first), so work stays close to home and woods grow outward
        if matches!(job, Job::Woodcutter | Job::Forester | Job::Stonemason | Job::Fisher | Job::Farmer) {
            let first = cands.iter().map(|q| g.distance(home, *q)).min().unwrap_or(0);
            let extra = if job == Job::Stonemason { 0 } else { 1 };
            // the near spots first (in their order), the rest stay for when none can be reached
            cands.sort_by_key(|q| g.distance(home, *q) > first + extra);
            let n_near = cands.iter().filter(|q| g.distance(home, **q) <= first + extra).count();
            let mut rng = self.rng_figure(f, Purpose::Production);
            for _ in 0..6 {
                let q = cands[rng.below(n_near as u32) as usize];
                if let Some(found) = self.go_to_work_spot(f, job, q, flag_pos) {
                    return Some(found);
                }
            }
        }
        let mut rng = self.rng_figure(f, Purpose::Production);
        // try a few random candidates; the worker must be able to reach them
        for _ in 0..6 {
            let q = cands[rng.below(cands.len() as u32) as usize];
            if let Some(found) = self.go_to_work_spot(f, job, q, flag_pos) {
                return Some(found);
            }
        }
        None
    }

    /// Walk from the flag to work spot `q` (or next to it, for stones and fish), if reachable:
    /// reserves the spot and returns it with the path.
    fn go_to_work_spot(
        &mut self,
        f: FigId,
        job: Job,
        q: MapCoord,
        flag_pos: MapCoord,
    ) -> Option<(MapCoord, Vec<Direction>)> {
        let g = self.world.grid();
        // stand next to objects that block (stone), on the node otherwise
        let stand = if matches!(job, Job::Stonemason | Job::Fisher) {
            Direction::ALL
                .iter()
                .map(|d| g.neighbor(q, *d))
                .find(|n| self.can_stand(*n) && (job != Job::Fisher || self.world.node_walkable(*n)))
        } else {
            Some(q)
        };
        let stand = stand?;
        let start = flag_pos;
        // RTTR nofFarmhand: a spot within a 20-step walk (the hunter's 50, MAX_HUNTING_DISTANCE),
        // not one the search finds round a lake after 3000 nodes (perf-2 #9)
        let reach = if job == Job::Hunter { 50 } else { 20 };
        if let Some(mut p) = self.find_path_bounded(start, stand, 3000, reach) {
            p.insert(0, Direction::SE);
            self.nature.reserved.insert(q);
            self.figures[f].target = Some(q);
            if job == Job::Hunter {
                // the hunted animal stops and waits (RTTR: the animal is "hunted")
                let animal = self
                    .figures
                    .iter()
                    .find(|(_, a)| {
                        matches!(a.kind, FigKind::Animal { .. }) && a.pos == q && matches!(a.motion, Motion::Idle)
                    })
                    .map(|(id, _)| id);
                if let Some(a) = animal
                    && let Some(k) = self.figures[a].ev.take()
                {
                    self.sched.cancel(k);
                }
            }
            return Some((q, p));
        }
        None
    }

    fn can_stand(&self, q: MapCoord) -> bool {
        self.world.node_walkable(q)
            && matches!(self.world.obj(q).blocking(), Blocking::None | Blocking::Tree | Blocking::Flag)
    }

    /// RTTR nofForester::GetPointQuality: free (decorations allowed), not on a road, no building
    /// next to it, all surrounding terrain "vital" (land and buildable).
    fn can_plant_tree(&self, q: MapCoord) -> bool {
        matches!(self.world.obj(q), NodeObj::None | NodeObj::Decoration(_) | NodeObj::Sign(_))
            && self.world.obj(q).blocking() == Blocking::None
            && !self.world.is_on_road(q)
            && self.world.terrains_around(q).iter().all(|t| rr_world::terrain::is_vital(*t))
            && self.world.grid().neighbors(q).iter().all(|n| !matches!(self.world.obj(*n), NodeObj::Building(_)))
    }

    /// RTTR nofFarmer::GetNewFieldPointQuality: as for trees, and no field next to it.
    fn can_sow(&self, q: MapCoord) -> bool {
        self.can_plant_tree(q)
            && self.world.grid().neighbors(q).iter().all(|n| !matches!(self.world.obj(*n), NodeObj::Field(_)))
    }

    fn fishing_spot(&self, q: MapCoord) -> bool {
        matches!(self.world.resources[self.world.grid().index(q)], Resource::Fish(a) if a > 0)
    }

    fn outside_work_result(&mut self, _f: FigId, _b: BuildingId, target: MapCoord) -> Option<Ware> {
        self.nature.reserved.remove(&target);
        let job = self.figures[_f].job;
        let g = self.world.grid();
        match job {
            Job::Woodcutter => {
                if matches!(self.world.obj(target), NodeObj::Tree { stage: 3, .. }) {
                    self.world.set_obj(target, NodeObj::None);
                    self.events.push(crate::Event::Sound { at: target, sound: SoundCue::TreeFall });
                    Some(Ware::Wood)
                } else {
                    None
                }
            }
            Job::Stonemason => {
                if let NodeObj::Granite { kind, size } = self.world.obj(target) {
                    self.world.set_obj(
                        target,
                        if size > 1 { NodeObj::Granite { kind, size: size - 1 } } else { NodeObj::None },
                    );
                    Some(Ware::Stones)
                } else {
                    None
                }
            }
            Job::Forester => {
                if self.can_plant_tree(target) {
                    let mut rng = self.rng_node(target, Purpose::TreeGrowth);
                    let species =
                        rng.below(if self.world.landscape == rr_core::Landscape::Greenland { 6 } else { 9 }) as u8;
                    self.world.set_obj(target, NodeObj::Tree { species, stage: 0 });
                    self.nature_schedule_growth(target);
                }
                None
            }
            Job::Fisher => {
                let i = g.index(target);
                // RTTR nofFisher: the catch succeeds with 40 + 10 * amount percent
                let mut rng = self.rng_node(target, Purpose::Production);
                if let Resource::Fish(a) = self.world.resources[i]
                    && a > 0
                    && rng.chance(40 + 10 * a as u32)
                {
                    self.world.resources[i] = if a > 1 { Resource::Fish(a - 1) } else { Resource::None };
                    Some(Ware::Fish)
                } else {
                    None
                }
            }
            Job::Hunter => {
                let animal = self
                    .figures
                    .iter()
                    .find(|(_, a)| matches!(a.kind, FigKind::Animal { .. }) && a.pos == target)
                    .map(|(i, _)| i);
                animal.map(|a| {
                    self.figures.remove(a);
                    Ware::Meat
                })
            }
            Job::Shipwright => {
                let step = match self.world.obj(target) {
                    NodeObj::ShipSite(p) => p + 1,
                    NodeObj::None => 1,
                    _ => return None,
                };
                if step >= crate::seafaring::SHIP_STEPS {
                    self.world.set_obj(target, NodeObj::None);
                    let water = g.within(target, 3).find(|n| self.sea_at_pub(*n) != 0);
                    if let Some(w) = water {
                        let player = self.figures[_f].player;
                        self.spawn_ship(player, w);
                        self.events.push(crate::Event::Sound { at: target, sound: SoundCue::Launch });
                    }
                } else {
                    self.world.set_obj(target, NodeObj::ShipSite(step));
                }
                None
            }
            Job::Farmer => match self.world.obj(target) {
                NodeObj::Field(3) => {
                    self.world.set_obj(target, NodeObj::None);
                    Some(Ware::Grain)
                }
                NodeObj::None if self.can_sow(target) => {
                    self.world.set_obj(target, NodeObj::Field(0));
                    self.nature_schedule_growth(target);
                    None
                }
                _ => None,
            },
            _ => None,
        }
    }

    // ------------------------------------------------------------------ builders and planers

    fn builder_act(&mut self, f: FigId, site: BuildingId) {
        let Some(bd) = self.buildings.get(site) else {
            self.figure_lost_home(f);
            return;
        };
        let BState::Site(_) = bd.state else {
            self.figure_lost_home(f);
            return;
        };
        match self.figures[f].step {
            Step::Go => {
                if let BState::Site(s) = &mut self.buildings[site].state {
                    s.builder = Some(f);
                }
                self.buildings[site].worker_coming = false;
                // the builder works in front of the site, not inside
                self.figures[f].inside = None;
                self.figures[f].step = Step::Site;
                self.builder_next(f, site);
            }
            Step::Working => {
                if self.site_build_step(site) {
                    self.finish_building(site);
                    let kind = self.buildings[site].kind;
                    if let BState::Ready = self.buildings[site].state
                        && kind.is_warehouse()
                    {
                        self.store_in_warehouse(f, site);
                        return;
                    }
                    self.figure_lost_home(f);
                    return;
                }
                self.figures[f].step = Step::Site;
                self.builder_next(f, site);
            }
            _ => self.builder_next(f, site),
        }
    }

    fn builder_next(&mut self, f: FigId, site: BuildingId) {
        let BState::Site(s) = &self.buildings[site].state else {
            return;
        };
        let cost = self.rules.building(self.buildings[site].kind).cost();
        let done = s.progress;
        let steps_avail = (s.used_boards as u16 + s.used_stones as u16) * crate::economy::STEPS_PER_MATERIAL;
        if done < steps_avail {
            self.figures[f].step = Step::Working;
            self.events.push(crate::Event::Sound { at: self.buildings[site].pos, sound: SoundCue::Hammer });
            self.start_work(f, BUILD_STEP_GF);
            return;
        }
        if done >= crate::Site::total_steps(cost) {
            self.figures[f].step = Step::Working;
            self.set_event(f, 1);
            return;
        }
        if self.site_take_material(site) {
            self.builder_next(f, site);
        } else {
            // wait for material
            self.figures[f].step = Step::Site;
            self.wait(f, 200);
        }
    }

    fn planer_act(&mut self, f: FigId, site: BuildingId) {
        let Some(bd) = self.buildings.get(site) else {
            self.figure_lost_home(f);
            return;
        };
        let BState::Site(s) = &bd.state else {
            self.figure_lost_home(f);
            return;
        };
        let alt = self.world.altitude(bd.pos);
        let next = s.to_level.first().copied();
        match self.figures[f].step {
            Step::Go => {
                if let BState::Site(s) = &mut self.buildings[site].state {
                    s.planer = Some(f);
                }
                self.buildings[site].worker_coming = false;
                self.figures[f].inside = None;
                self.figures[f].step = Step::Working;
                self.start_work(f, self.rules.job(Job::Planer).work);
            }
            Step::Working => {
                if let Some(q) = next {
                    self.world.set_altitude(q, alt);
                    if let BState::Site(s) = &mut self.buildings[site].state {
                        s.to_level.remove(0);
                    }
                }
                let more = matches!(&self.buildings[site].state, BState::Site(s) if !s.to_level.is_empty());
                if more {
                    self.start_work(f, self.rules.job(Job::Planer).work);
                } else {
                    if let BState::Site(s) = &mut self.buildings[site].state {
                        s.planing = false;
                        s.planer = None;
                    }
                    self.figure_lost_home(f);
                }
            }
            _ => self.figure_lost_home(f),
        }
    }

    // ------------------------------------------------------------------ specialists

    pub(crate) fn cmd_send_specialist(
        &mut self,
        player: PlayerId,
        flag: MapCoord,
        job: Job,
    ) -> Result<(), crate::RejectReason> {
        let fl = self.flag_at(flag).ok_or(crate::RejectReason::NoFlag)?;
        if self.flags[fl].player != player {
            return Err(crate::RejectReason::NotAllowed);
        }
        let routes = self.routes_to(fl);
        let mut best: Option<(u32, BuildingId)> = None;
        for (id, b) in self.buildings.iter() {
            if b.player == player
                && b.is_ready()
                && let Some(inv) = &b.inventory
                && inv.can_supply(job, &self.rules)
            {
                let d = routes.get(b.flag.index()).map_or(u32::MAX, |r| r.0);
                if d != u32::MAX && best.is_none_or(|x| d < x.0) {
                    best = Some((d, id));
                }
            }
        }
        let (_, wh) = best.ok_or(crate::RejectReason::NotFound)?;
        let rules = self.rules.clone();
        let inv = self.buildings[wh].inventory.as_mut().expect("warehouse");
        if inv.job(job) == 0 && inv.take_person(job, &rules) {
            inv.jobs[job.index()] += 1;
        }
        let kind = if job == Job::Geologist {
            FigKind::Geologist { flag: fl, left: self.rules.economy.geologist_flags as u8 }
        } else {
            FigKind::Scout { flag: fl, left: SCOUT_AT_FLAG }
        };
        self.spawn_from_warehouse(wh, job, kind);
        Ok(())
    }

    fn geologist_act(&mut self, f: FigId, flag: FlagId, left: u8) {
        let Some(fl) = self.flags.get(flag) else {
            self.figure_lost_home(f);
            return;
        };
        let center = fl.pos;
        let pos = self.figures[f].pos;
        if self.figures[f].step == Step::Out {
            self.figures[f].step = Step::Working;
            self.events.push(crate::Event::Sound { at: pos, sound: SoundCue::Hammer });
            self.start_work(f, 100);
            return;
        }
        if self.figures[f].step == Step::Working {
            // put up a sign with what was found
            let res = self.world.resources[self.world.grid().index(pos)];
            let mountain = self.world.terrains_around(pos).iter().all(|t| rr_world::terrain::is_mountain(*t));
            let shown = match res {
                Resource::Water | Resource::Fish(_) if mountain => Resource::None,
                Resource::Fish(_) => Resource::None,
                r => r,
            };
            if matches!(self.world.obj(pos), NodeObj::None) {
                self.world.set_obj(pos, NodeObj::Sign(sign_code(shown)));
                self.nature_schedule_sign_end(pos);
            }
            if shown != Resource::None && shown != Resource::Water {
                let player = self.figures[f].player;
                self.events.push(crate::Event::GeologistFound { player, at: pos, what: shown });
            }
            self.figures[f].kind = FigKind::Geologist { flag, left: left.saturating_sub(1) };
        }
        let left = match self.figures[f].kind {
            FigKind::Geologist { left, .. } => left,
            _ => 0,
        };
        if left == 0 {
            self.figure_lost_home(f);
            return;
        }
        // pick a free node near the flag
        let g = self.world.grid();
        let mut rng = self.rng_figure(f, Purpose::Geologist);
        // RTTR nofGeologist::LookForNewNodes: up to 15 rings out (`geologist_radius`); from the first ring with a free
        // node take two more rings (at most ring 10). No signs, flags or trees.
        let good = |q: &MapCoord| {
            *q != pos
                && matches!(self.world.obj(*q).blocking(), Blocking::None)
                && !matches!(self.world.obj(*q), NodeObj::Sign(_))
                && self.world.node_walkable(*q)
                && !self.nature.reserved.contains(q)
        };
        let mut cands: Vec<MapCoord> = Vec::new();
        let reach = self.rules.economy.geologist_radius;
        let mut max_r = reach;
        for r in 1..=reach {
            if r > max_r {
                break;
            }
            let ring: Vec<MapCoord> = g.ring(center, r).filter(good).collect();
            if !ring.is_empty() && cands.is_empty() {
                max_r = (r + 2).min(10).max(r).min(reach);
            }
            cands.extend(ring);
        }
        for _ in 0..6 {
            if cands.is_empty() {
                break;
            }
            let i = rng.below(cands.len() as u32) as usize;
            let q = cands.swap_remove(i);
            // humans walk at most 20 nodes to a spot
            if let Some(p) = self.find_path(pos, q, 800).filter(|p| p.len() <= 20) {
                self.figures[f].step = Step::Out;
                self.go_then_work(f, &p, 100);
                return;
            }
        }
        self.figure_lost_home(f);
    }

    fn go_then_work(&mut self, f: FigId, path: &[Direction], _work: u32) {
        self.go(f, path, Step::Out);
    }

    fn scout_act(&mut self, f: FigId, flag: FlagId, left: u8) {
        let Some(fl) = self.flags.get(flag) else {
            self.figure_lost_home(f);
            return;
        };
        let center = fl.pos;
        let pos = self.figures[f].pos;
        let mut rng = self.rng_figure(f, Purpose::Misc);
        // RTTR nofScout_Free: at the flag the scout gets 80-99 nodes to walk, then goes back
        let left = if left == SCOUT_AT_FLAG { SCOUT_STEPS + rng.below(SCOUT_STEPS_RAND) as u8 } else { left };
        if left == 0 {
            self.figure_lost_home(f);
            return;
        }
        // a random node up to 16 from the flag that the player can't see now, reachable within
        // 32 steps and within 20 of the flag (no long ways round mountains); none left: back
        let g = self.world.grid();
        let player = self.figures[f].player;
        let mut cands: Vec<MapCoord> = g
            .within(center, SCOUT_RANGE)
            .filter(|q| *q != pos && self.can_stand(*q) && self.visibility(player, *q) != crate::Visibility::Visible)
            .collect();
        for _ in 0..8 {
            if cands.is_empty() {
                break;
            }
            let q = cands.swap_remove(rng.below(cands.len() as u32) as usize);
            let Some(mut p) = self.find_path(pos, q, 3000) else { continue };
            if p.len() > 2 * SCOUT_RANGE as usize
                || self.find_path(center, q, 3000).is_none_or(|b| b.len() > (SCOUT_RANGE + SCOUT_RANGE / 4) as usize)
            {
                continue;
            }
            let walk = p.len().min(left as usize);
            p.truncate(walk);
            self.figures[f].kind = FigKind::Scout { flag, left: left - walk as u8 };
            self.go(f, &p, Step::Go);
            return;
        }
        self.figure_lost_home(f);
    }
}

/// RTTR nofScout_Free: 80 + rand(20) nodes, up to 16 from the flag.
const SCOUT_STEPS: u8 = 80;
const SCOUT_STEPS_RAND: u32 = 20;
const SCOUT_RANGE: u16 = 16;
/// `left` of a scout on the way to its flag: the walk is counted from there.
const SCOUT_AT_FLAG: u8 = u8::MAX;

/// Dwindling mines (RTTR S4LIKE_FULL_PRODUCTIVITY_RESOURCE_AMOUNT 20: full at 20 units left,
/// 5 % a unit below), the unit each node keeps (S4LIKE_MIN_RESOURCE_AMOUNT), and below what
/// chance the player hears the mine runs low.
const MINE_CHANCE_PER_UNIT: u32 = 5;
const MINE_KEEPS: u8 = 1;
const MINE_LOW_PERCENT: u32 = 30;

fn mine_resource(kind: BuildingType) -> Option<fn(Resource) -> Option<u8>> {
    match kind {
        BuildingType::CoalMine => Some(|r| if let Resource::Coal(a) = r { Some(a) } else { None }),
        BuildingType::IronMine => Some(|r| if let Resource::Iron(a) = r { Some(a) } else { None }),
        BuildingType::GoldMine => Some(|r| if let Resource::Gold(a) = r { Some(a) } else { None }),
        BuildingType::GraniteMine => Some(|r| if let Resource::Granite(a) = r { Some(a) } else { None }),
        _ => None,
    }
}

fn with_amount(r: Resource, a: u8) -> Resource {
    if a == 0 {
        return Resource::None;
    }
    match r {
        Resource::Coal(_) => Resource::Coal(a),
        Resource::Iron(_) => Resource::Iron(a),
        Resource::Gold(_) => Resource::Gold(a),
        Resource::Granite(_) => Resource::Granite(a),
        Resource::Fish(_) => Resource::Fish(a),
        other => other,
    }
}

/// Sign code: high nibble kind (0 none, 1 coal, 2 iron, 3 gold, 4 granite, 5 water), low nibble amount.
pub fn sign_code(r: Resource) -> u8 {
    match r {
        Resource::None | Resource::Fish(_) => 0,
        Resource::Coal(a) => 0x10 | a,
        Resource::Iron(a) => 0x20 | a,
        Resource::Gold(a) => 0x30 | a,
        Resource::Granite(a) => 0x40 | a,
        Resource::Water => 0x50,
    }
}

trait CatapultKind {
    fn kind_is_catapult(&self) -> bool;
}

impl CatapultKind for rr_rules::BuildingDef {
    fn kind_is_catapult(&self) -> bool {
        self.worker == Some(Job::Helper) && self.inputs == [Ware::Stones]
    }
}

/// The attempt counter counts up to 255 and then cycles through 16..=255: the productivity
/// display only needs "at least 16", and the armory's sword/shield parity keeps alternating.
fn next_attempt(n: u8) -> u8 {
    if n == u8::MAX { 16 } else { n + 1 }
}

#[cfg(test)]
mod tests {
    #[test]
    fn attempt_parity_keeps_alternating() {
        let mut n = 0u8;
        for i in 0..1000u32 {
            assert_eq!(n % 2, (i % 2) as u8, "at attempt {i}");
            n = super::next_attempt(n);
            assert!(n > 0);
        }
    }
}

/// The path search's working memory: per node the number of the search that last touched it,
/// and that search's cost and way back.
#[derive(Default)]
pub(crate) struct PathScratch {
    stamp: Vec<u32>,
    cost: Vec<u32>,
    came: Vec<Option<(usize, Direction)>>,
    search: u32,
}

#[cfg(test)]
mod carrier_index_tests {
    use super::*;
    use crate::{Cmd, GameSetup, PlayerCommand, PlayerSetup};
    use rr_core::MapData;
    use rr_rules::Rules;
    use std::sync::Arc;

    /// A flat 64x64 meadow, one HQ, and a four-step road with its carrier waiting in the middle.
    fn road_with_carrier() -> (Sim, RoadId) {
        let mut m = MapData::flat(64, 64, 8); // meadow
        m.hq = vec![Some(MapCoord::new(20, 20))];
        let setup = GameSetup { seed: 42, players: vec![PlayerSetup::default()], ..Default::default() };
        let mut sim = Sim::new(Arc::new(Rules::classic()), &m, &setup).expect("setup");
        let a = sim.world.grid().walk(MapCoord::new(20, 20), Direction::SE, 1);
        let dirs = vec![Direction::E, Direction::E, Direction::E, Direction::E];
        sim.step(&[PlayerCommand { player: 0, cmd: Cmd::BuildRoad { from: a, dirs } }]);
        let end = sim.world.grid().walk(a, Direction::E, 4);
        let r = sim.road_between(a, end).expect("the road was not built");
        for _ in 0..1000 {
            if sim.roads[r].carrier.is_some() {
                return (sim, r);
            }
            sim.step(&[]);
        }
        panic!("no carrier came to the road");
    }

    /// A hand-made save can carry a carrier index past the end of a road that was shortened
    /// since (splitting a road truncates the first half). A figure standing on the middle node
    /// with such an index walked to `dirs[pos - 1]`, out of bounds, and the game died.
    #[test]
    fn a_carrier_index_past_the_roads_end_cannot_crash_the_game() {
        let (mut sim, r) = road_with_carrier();
        let f = sim.roads[r].carrier.expect("carrier");
        let len = sim.roads[r].len();
        let mid = sim.roads[r].nodes(sim.world.grid())[len / 2];
        assert_eq!(sim.figures[f].pos, mid, "the carrier waits in the middle");
        sim.figures[f].step = Step::Go;
        sim.figures[f].kind = FigKind::Carrier { road: r, pos: 500 };
        sim.figure_event(f); // used to panic here
        if let FigKind::Carrier { road, pos } = sim.figures[f].kind {
            assert!(pos <= sim.roads[road].len(), "the index stayed past the road's end: {pos}");
        }
        assert!(sim.validate().is_ok(), "the game no longer validates");
    }
}
