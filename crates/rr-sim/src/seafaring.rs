//! Seafaring: seas, ships, harbors, sea transport and expeditions (after RTTR `noShip`,
//! `nobHarborBuilding`, `nofShipWright`).
//!
//! - A *ship node* is a node whose six triangles are all water; connected ship nodes form a sea.
//! - Harbor spots are map nodes with a ship node within two steps (their coast point).
//! - Shipyards build ships when their owner has a harbor; ships dock at the nearest harbor.
//! - Requests a player's road network can't serve are served from a harbor on another coast of
//!   the same sea: the goods are queued there and shipped to the requester's harbor.
//! - An expedition (the harbor's own cost plus a builder, and our starter kit) sails to a free
//!   harbor spot and founds a colony: a harbor site that holds territory at once; built, its
//!   store holds the kit.

use crate::economy::{BState, Site};
use crate::figures::{FigKind, Motion, Step};
use crate::military::SoldierTask;
use crate::{BuildingId, Ev, Event, PlayerId, RejectReason, ShipId, Sim};
use rr_core::{Direction, MapCoord};
use rr_rules::{BuildingType, Job, Ware};
use std::collections::VecDeque;

/// A colony's starter kit beyond the harbor's own cost: boards and stones (`expedition_load`).
pub const COLONY_KIT: (u8, u8) = (10, 6);
/// How far from a harbor spot ships may anchor.
pub const HARBOR_COAST_RANGE: u16 = 4;
/// Frames a ship needs per node.
pub const SHIP_GF: u32 = 20;
/// Goods and people a ship carries at once.
pub const SHIP_CAPACITY: usize = 40;
/// Build steps of a ship (RTTR noShipBuildingSite PROGRESS_PARTS 4 + 2 + 3), one board each.
pub const SHIP_STEPS: u8 = 9;
/// How far the shipwright looks for a building spot on the shore.
pub const SHIPWRIGHT_RADIUS: u16 = 8;
/// A shipwright's work per step of a sea ship (RTTR WORKING_TIME_SHIPS).
pub(crate) const SHIP_STEP_WORK: u32 = 70;
/// A ship's time to load before it leaves and to unload when it arrives (RTTR noShip: 200 each).
pub(crate) const SHIP_LOAD_GF: u32 = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Cargo {
    Ware(Ware),
    Person(Job),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Expedition {
    pub boards: u8,
    pub stones: u8,
    pub builder: bool,
    /// The ship that holds the expedition, once loaded.
    pub ship: Option<ShipId>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ShipTask {
    /// Waiting (docked or at sea).
    Idle,
    /// Sailing to a harbor to dock.
    ToHarbor(BuildingId),
    /// Carrying goods and people to a harbor.
    Cargo { to: BuildingId, load: Vec<Cargo> },
    /// Carrying an expedition; `target` is the harbor spot once chosen.
    Expedition { target: Option<MapCoord> },
    /// An attack by sea (RTTR sea attacks): soldiers board at `harbor`, land at the harbor spot
    /// `landing` near `target`, and come back aboard when the fight is over. `phase`: 0 taking
    /// them on, 1 sailing there, 2 waiting at the landing, 3 sailing home.
    SeaAttack {
        harbor: BuildingId,
        landing: MapCoord,
        target: BuildingId,
        aboard: Vec<crate::FigId>,
        out: Vec<crate::FigId>,
        coming: u8,
        phase: u8,
        since: u32,
    },
    /// Exploring the coasts (RTTR exploration expedition): `stops` unexplored harbor spots
    /// visited so far, then back to `home`.
    Explore { home: BuildingId, stops: u8 },
}

/// How many coasts an exploring ship visits, and how far it sees (RTTR
/// VISUALRANGE_EXPLORATION_SHIP).
const EXPLORE_STOPS: u8 = 3;
pub const EXPLORE_SIGHT: u16 = 12;

/// How an attack by sea would go: from our `harbor`, by `ship`, landing at `landing`, with
/// soldiers from the posts `sources` (distance to the harbor, post).
#[derive(Clone, Debug)]
pub struct SeaAttackPlan {
    pub harbor: BuildingId,
    pub landing: MapCoord,
    pub ship: ShipId,
    pub sources: Vec<(u32, BuildingId)>,
}

/// RTTR SEAATTACK_DISTANCE: the target within this of the landing (by a short walk), the posts
/// within this of our harbor.
pub const SEA_ATTACK_DISTANCE: u32 = 15;
/// How long a ship waits for the last soldiers to come aboard before it sails with those it has.
const SEA_ATTACK_GATHER_GF: u32 = 3000;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Ship {
    pub player: PlayerId,
    pub pos: MapCoord,
    pub sea: u16,
    pub motion: Motion,
    /// Remaining directions, next one last.
    pub path: Vec<Direction>,
    pub docked: Option<BuildingId>,
    pub task: ShipTask,
    pub ev: Option<(u32, u64)>,
}

pub(crate) fn cargo_slot(c: Cargo) -> usize {
    idx(c)
}

fn idx(c: Cargo) -> usize {
    match c {
        Cargo::Ware(w) => w.index(),
        Cargo::Person(j) => Ware::COUNT + j.index(),
    }
}

impl Sim {
    pub fn ships(&self) -> &rr_core::Arena<Ship> {
        &self.ships
    }

    pub fn ship(&self, id: ShipId) -> &Ship {
        &self.ships[id]
    }

    fn is_ship_node(&self, p: MapCoord) -> bool {
        self.world.terrains_around(p).iter().all(|t| rr_world::terrain::is_water(*t))
    }

    pub(crate) fn compute_seas(&mut self) {
        let g = self.world.grid();
        let mut sea = vec![0u16; g.len()];
        let mut next = 1u16;
        for start in g.coords() {
            if sea[g.index(start)] != 0 || !self.is_ship_node(start) {
                continue;
            }
            let mut q = VecDeque::from([start]);
            sea[g.index(start)] = next;
            while let Some(p) = q.pop_front() {
                for n in g.neighbors(p) {
                    let i = g.index(n);
                    if sea[i] == 0 && self.is_ship_node(n) {
                        sea[i] = next;
                        q.push_back(n);
                    }
                }
            }
            next = next.saturating_add(1);
        }
        self.sea = sea;
    }

    /// Number the land masses: nodes people can walk between, by terrain alone.
    pub(crate) fn compute_land(&mut self) {
        let g = self.world.grid();
        let mut land = vec![0u16; g.len()];
        let mut next = 1u16;
        for start in g.coords() {
            if land[g.index(start)] != 0 || !self.world.node_walkable(start) {
                continue;
            }
            let mut q = VecDeque::from([start]);
            land[g.index(start)] = next;
            while let Some(p) = q.pop_front() {
                for d in Direction::ALL {
                    let n = g.neighbor(p, d);
                    let i = g.index(n);
                    if land[i] == 0 && self.world.edge_walkable(p, d) && self.world.node_walkable(n) {
                        land[i] = next;
                        q.push_back(n);
                    }
                }
            }
            next = next.saturating_add(1);
        }
        self.land = land;
    }

    /// Can people walk from `a` to `b` at all (same land mass)?
    pub fn same_land(&self, a: MapCoord, b: MapCoord) -> bool {
        let g = self.world.grid();
        let (la, lb) = (self.land[g.index(a)], self.land[g.index(b)]);
        la != 0 && la == lb
    }

    /// Both nodes lie on known land masses, and not the same: no path on foot between them.
    /// Are `a` and `b` on different lands (a sea between them)?
    pub fn other_land_pub(&self, a: MapCoord, b: MapCoord) -> bool {
        self.other_land(a, b)
    }

    pub(crate) fn other_land(&self, a: MapCoord, b: MapCoord) -> bool {
        let g = self.world.grid();
        let (la, lb) = (self.land[g.index(a)], self.land[g.index(b)]);
        la != 0 && lb != 0 && la != lb
    }

    pub(crate) fn sea_at_pub(&self, p: MapCoord) -> u16 {
        self.sea_at(p)
    }

    fn sea_at(&self, p: MapCoord) -> u16 {
        self.sea.get(self.world.grid().index(p)).copied().unwrap_or(0)
    }

    /// The water node where ships anchor for a harbor spot.
    pub fn harbor_coast(&self, spot: MapCoord) -> Option<MapCoord> {
        self.world.grid().within(spot, HARBOR_COAST_RANGE).find(|q| self.sea_at(*q) != 0)
    }

    /// The sea a harbor spot lies at.
    pub fn harbor_sea(&self, spot: MapCoord) -> Option<u16> {
        // (a coast search per harbor, several times per understaffed post and pass: perf-2 #5)
        if let Some(s) = self.harbor_seas.borrow().get(&spot) {
            return *s;
        }
        let s = self.harbor_coast(spot).map(|c| self.sea_at(c));
        self.harbor_seas.borrow_mut().insert(spot, s);
        s
    }

    fn sea_path(&self, from: MapCoord, to: MapCoord) -> Option<Vec<Direction>> {
        if from == to {
            return Some(Vec::new());
        }
        // (kept: the sea never changes, and ships sail the same few coasts again and again; each
        // search walked the whole sea, 8.3 % of late time on the Mediterranean: perf round 3, #9)
        if let Some(p) = self.sea_paths.borrow().get(&(from, to)) {
            return p.clone();
        }
        let g = self.world.grid();
        let path = crate::transport::bfs_path(g, from, to, usize::MAX, |p, d, _| self.sea_at(g.neighbor(p, d)) != 0);
        let mut kept = self.sea_paths.borrow_mut();
        if kept.len() >= 4096 {
            kept.clear();
        }
        kept.insert((from, to), path.clone());
        path
    }

    /// Ready harbors of `player` at sea `sea`.
    fn harbors_at(&self, player: PlayerId, sea: u16) -> Vec<BuildingId> {
        self.buildings
            .iter()
            .filter(|(_, b)| b.player == player && b.kind == BuildingType::Harbor && b.is_ready())
            .filter(|(_, b)| self.harbor_sea(b.pos) == Some(sea))
            .map(|(id, _)| id)
            .collect()
    }

    pub(crate) fn spawn_ship(&mut self, player: PlayerId, at: MapCoord) {
        let sea = self.sea_at(at);
        let id = self.ships.insert(Ship {
            player,
            pos: at,
            sea,
            motion: Motion::Idle,
            path: Vec::new(),
            docked: None,
            task: ShipTask::Idle,
            ev: None,
        });
        self.ship_wake(id, 1);
    }

    fn ship_wake(&mut self, s: ShipId, delay: u32) {
        if let Some(k) = self.ships[s].ev.take() {
            self.sched.cancel(k);
        }
        let key = self.schedule(delay, Ev::Ship(s));
        self.ships[s].ev = Some(key);
    }

    fn ship_sail(&mut self, s: ShipId, to: MapCoord) -> bool {
        // a ship under way finishes its step first and plans from the node it is sailing to
        let ship = &self.ships[s];
        let (from, moving) = match ship.motion {
            Motion::Walk { dir, .. } => (self.world.grid().neighbor(ship.pos, dir), true),
            _ => (ship.pos, false),
        };
        match self.sea_path(from, to) {
            Some(p) => {
                self.ships[s].docked = None;
                self.ships[s].path = p.into_iter().rev().collect();
                if !moving {
                    self.ship_wake(s, 1);
                }
                true
            }
            None => false,
        }
    }

    pub(crate) fn ship_event(&mut self, s: ShipId) {
        let Some(ship) = self.ships.get_mut(s) else { return };
        ship.ev = None;
        if let Motion::Walk { dir, .. } = ship.motion {
            ship.pos = self.world.grid().neighbor(ship.pos, dir);
            ship.motion = Motion::Idle;
        }
        if let Some(d) = self.ships[s].path.pop() {
            self.ships[s].motion = Motion::Walk { dir: d, start: self.gf, dur: SHIP_GF };
            let key = self.schedule(SHIP_GF, Ev::Ship(s));
            self.ships[s].ev = Some(key);
            return;
        }
        // arrived
        match self.ships[s].task.clone() {
            ShipTask::Idle => self.ship_find_harbor(s),
            ShipTask::ToHarbor(h) => {
                if self.buildings.contains(h) {
                    self.ships[s].docked = Some(h);
                    self.ships[s].task = ShipTask::Idle;
                    self.events.push(crate::Event::Sound { at: self.ships[s].pos, sound: crate::SoundCue::Dock });
                    self.start_building_timer(h, 1);
                } else {
                    self.ships[s].task = ShipTask::Idle;
                    self.ship_find_harbor(s);
                }
            }
            ShipTask::Cargo { to, load } => {
                // first alongside: unloading takes its time (RTTR noShip; fidelity #6)
                if self.ships[s].docked != Some(to) && self.buildings.contains(to) {
                    self.ships[s].docked = Some(to);
                    self.events.push(crate::Event::Sound { at: self.ships[s].pos, sound: crate::SoundCue::Dock });
                    self.ship_wake(s, SHIP_LOAD_GF);
                    return;
                }
                if let Some(inv) = self.buildings.get_mut(to).and_then(|b| b.inventory.as_mut()) {
                    for c in &load {
                        match c {
                            Cargo::Ware(w) => inv.wares[w.index()] += 1,
                            Cargo::Person(j) => inv.jobs[j.index()] += 1,
                        }
                    }
                    let bd = &mut self.buildings[to];
                    for c in &load {
                        let i = idx(*c);
                        bd.sea_incoming[i] = bd.sea_incoming[i].saturating_sub(1);
                    }
                    self.ships[s].docked = Some(to);
                    self.start_building_timer(to, 1);
                }
                self.ships[s].task = ShipTask::Idle;
            }
            ShipTask::SeaAttack { .. } => self.sea_attack_step(s),
            ShipTask::Explore { home, stops } => {
                self.ships[s].task = ShipTask::Explore { home, stops: stops + 1 };
                self.explore_next(s);
            }
            ShipTask::Expedition { target: Some(spot) } => self.found_colony(s, spot),
            ShipTask::Expedition { target: None } => {
                if !self.buildings.iter().any(|(_, b)| b.expedition.as_ref().is_some_and(|e| e.ship == Some(s))) {
                    self.expedition_return(s);
                }
            }
        }
    }

    /// The harbor spots on `sea` that `player` has never seen, nearest to `from` first.
    pub fn unexplored_spots(&self, player: PlayerId, sea: u16, from: MapCoord) -> Vec<MapCoord> {
        let g = self.world.grid();
        let mut v: Vec<MapCoord> = self
            .world
            .harbor_spots()
            .into_iter()
            .filter(|p| {
                self.harbor_sea(*p) == Some(sea) && self.visibility(player, *p) == crate::Visibility::Unexplored
            })
            .collect();
        v.sort_by_key(|p| (g.distance(from, *p), p.y, p.x));
        v
    }

    /// An idle ship of `player` on the sea of harbor `h`, the nearest to it.
    pub fn idle_ship_for(&self, player: PlayerId, h: BuildingId) -> Option<ShipId> {
        let pos = self.buildings.get(h)?.pos;
        let sea = self.harbor_sea(pos)?;
        let g = self.world.grid();
        self.ships
            .iter()
            .filter(|(_, s)| s.player == player && s.sea == sea && s.task == ShipTask::Idle)
            .min_by_key(|(id, s)| (g.distance(s.pos, pos), id.index()))
            .map(|(id, _)| id)
    }

    pub(crate) fn cmd_start_exploration(&mut self, player: PlayerId, at: MapCoord) -> Result<(), RejectReason> {
        let h = self.building_at(at).ok_or(RejectReason::NotFound)?;
        let b = &self.buildings[h];
        if b.player != player || b.kind != BuildingType::Harbor || !b.is_ready() {
            return Err(RejectReason::NotAllowed);
        }
        let sea = self.harbor_sea(at).ok_or(RejectReason::NotFound)?;
        if self.unexplored_spots(player, sea, at).is_empty() {
            return Err(RejectReason::NotFound);
        }
        let s = self.idle_ship_for(player, h).ok_or(RejectReason::NotFound)?;
        self.ships[s].task = ShipTask::Explore { home: h, stops: 0 };
        self.explore_next(s);
        Ok(())
    }

    /// An exploring ship sails on to the nearest coast still unseen, or home.
    fn explore_next(&mut self, s: ShipId) {
        let ShipTask::Explore { home, stops } = self.ships[s].task.clone() else { return };
        let (player, sea, pos) = (self.ships[s].player, self.ships[s].sea, self.ships[s].pos);
        if stops < EXPLORE_STOPS {
            for spot in self.unexplored_spots(player, sea, pos) {
                if let Some(c) = self.harbor_coast(spot)
                    && c != pos
                    && self.ship_sail(s, c)
                {
                    return;
                }
            }
        }
        self.ships[s].task = ShipTask::Idle;
        match self.buildings.get(home).and_then(|h| self.harbor_coast(h.pos)) {
            Some(c) if c != pos && self.ship_sail(s, c) => self.ships[s].task = ShipTask::ToHarbor(home),
            _ => self.ship_wake(s, 1),
        }
    }

    /// How `player` could attack `t` by sea, if at all (RTTR GetSoldiersForSeaAttack): a harbor
    /// spot within `SEA_ATTACK_DISTANCE` of the target and a short walk from it, one of our
    /// harbors on the same sea with an idle ship there, and our posts near that harbor.
    pub fn sea_attack_plan(&self, player: PlayerId, t: BuildingId) -> Option<SeaAttackPlan> {
        let g = self.world.grid();
        let tb = self.buildings.get(t)?;
        let goal = self.flags[tb.flag].pos;
        let mut landings: Vec<(u32, MapCoord)> = self
            .world
            .harbor_spots()
            .iter()
            .copied()
            .filter(|l| g.distance(*l, tb.pos) <= SEA_ATTACK_DISTANCE && self.harbor_sea(*l).is_some())
            .filter(|l| self.same_land(*l, goal))
            .filter(|l| self.path_len_within(*l, goal, SEA_ATTACK_DISTANCE).is_some() || *l == tb.pos)
            .map(|l| (g.distance(l, tb.pos), l))
            .collect();
        landings.sort_by_key(|(d, l)| (*d, l.y, l.x));
        for (_, landing) in landings {
            let sea = self.harbor_sea(landing)?;
            let mut harbors: Vec<(u32, BuildingId)> = self
                .harbors_at(player, sea)
                .into_iter()
                .filter(|h| !self.same_land(self.buildings[*h].pos, goal))
                .map(|h| (g.distance(self.buildings[h].pos, landing), h))
                .collect();
            harbors.sort_by_key(|(d, h)| (*d, h.index()));
            for (_, h) in harbors {
                let hpos = self.buildings[h].pos;
                let ship = self
                    .ships
                    .iter()
                    .filter(|(_, s)| s.player == player && s.sea == sea && s.task == ShipTask::Idle)
                    .min_by_key(|(id, s)| (g.distance(s.pos, hpos), id.index()))
                    .map(|(id, _)| id);
                let Some(ship) = ship else { continue };
                let hflag = self.flags[self.buildings[h].flag].pos;
                let mut sources: Vec<(u32, BuildingId)> = self
                    .buildings
                    .iter()
                    .filter(|(_, b)| b.player == player && b.is_ready() && crate::is_frontier_military(b.kind))
                    .map(|(id, b)| (g.distance(b.pos, hpos), id))
                    .filter(|(d, _)| *d <= SEA_ATTACK_DISTANCE)
                    .filter(|(_, id)| {
                        let from = self.flags[self.buildings[*id].flag].pos;
                        self.same_land(from, hflag)
                            && self.path_len_within(from, hflag, 2 * SEA_ATTACK_DISTANCE).is_some()
                    })
                    .collect();
                sources.sort();
                if !sources.is_empty() {
                    return Some(SeaAttackPlan { harbor: h, landing, ship, sources });
                }
            }
        }
        None
    }

    /// `sent` soldiers are on their way to board for an attack on `t`.
    pub(crate) fn start_sea_attack(&mut self, plan: &SeaAttackPlan, t: BuildingId, sent: u8) {
        let s = plan.ship;
        self.ships[s].task = ShipTask::SeaAttack {
            harbor: plan.harbor,
            landing: plan.landing,
            target: t,
            aboard: Vec::new(),
            out: Vec::new(),
            coming: sent,
            phase: 0,
            since: self.gf,
        };
        match self.harbor_coast(self.buildings[plan.harbor].pos) {
            Some(c) if c != self.ships[s].pos => {
                if !self.ship_sail(s, c) {
                    self.ship_wake(s, 50);
                }
            }
            _ => self.ship_wake(s, 50),
        }
    }

    /// A soldier at the boarding place of `ship` goes aboard: at our harbor before it sails, at the
    /// landing while it waits there. False if that ship isn't taking anybody on there.
    pub(crate) fn board(&mut self, f: crate::FigId, ship: ShipId) -> bool {
        let pos = self.figures[f].pos;
        let Some(ShipTask::SeaAttack { harbor, landing, aboard, coming, phase, .. }) =
            self.ships.get_mut(ship).map(|s| &mut s.task)
        else {
            return false;
        };
        let at = match *phase {
            0 => self.buildings.get(*harbor).map(|h| h.pos),
            2 => Some(*landing),
            _ => None,
        };
        if at != Some(pos) {
            return false;
        }
        aboard.push(f);
        *coming = coming.saturating_sub(1);
        // (out of sight while aboard: in the harbor's keeping, as figures inside a building are)
        let harbor = *harbor;
        self.figures[f].inside = Some(harbor);
        self.figures[f].motion = Motion::Idle;
        self.figures[f].path.clear();
        true
    }

    /// The ship of a sea attack waiting for soldier `f` at its landing, and where that is.
    pub(crate) fn sea_attack_ship_of(&self, f: crate::FigId) -> Option<(ShipId, MapCoord)> {
        self.ships.iter().find_map(|(id, s)| match &s.task {
            ShipTask::SeaAttack { landing, out, phase: 2, .. } if out.contains(&f) => Some((id, *landing)),
            _ => None,
        })
    }

    /// A sea attack's ship arrived or woke: take soldiers on, sail, land them, wait, bring them home.
    fn sea_attack_step(&mut self, s: ShipId) {
        let ShipTask::SeaAttack { harbor, landing, target, aboard, out, coming, phase, since } =
            self.ships[s].task.clone()
        else {
            return;
        };
        let home_coast = self.buildings.get(harbor).and_then(|h| self.harbor_coast(h.pos));
        let set = |sim: &mut Self, aboard: Vec<crate::FigId>, out: Vec<crate::FigId>, phase: u8| {
            sim.ships[s].task = ShipTask::SeaAttack { harbor, landing, target, aboard, out, coming, phase, since };
        };
        match phase {
            0 => {
                if home_coast.is_none() {
                    // the harbor is gone: whoever is aboard steps ashore and goes home
                    self.sea_attack_disembark(s, &aboard, None);
                    return;
                }
                let waited = self.gf >= since + SEA_ATTACK_GATHER_GF;
                if coming > 0 && !waited {
                    self.ship_wake(s, 50);
                    return;
                }
                if aboard.is_empty() {
                    self.ships[s].task = ShipTask::Idle;
                    self.ship_wake(s, 1);
                    return;
                }
                let to = self.harbor_coast(landing);
                set(self, aboard, out, 1);
                if !to.is_some_and(|c| self.ship_sail(s, c)) {
                    set(self, Vec::new(), Vec::new(), 3);
                    let all = self.ships_aboard(s);
                    self.sea_attack_disembark(s, &all, None);
                }
                self.ships[s].docked = None;
            }
            1 => {
                // landed: every soldier aboard goes for the target
                let goal = self.buildings.get(target).map(|t| self.flags[t.flag].pos);
                for f in &aboard {
                    let Some(fig) = self.figures.get_mut(*f) else { continue };
                    fig.inside = None;
                    fig.pos = landing;
                    let FigKind::Soldier(SoldierTask::ToShip { home, .. }) = fig.kind.clone() else { continue };
                    fig.kind = FigKind::Soldier(SoldierTask::Attack { target, home });
                    match goal.and_then(|g| self.find_path(landing, g, 6000)) {
                        Some(p) => {
                            self.figures[*f].path = p.into_iter().rev().collect();
                            self.figures[*f].step = Step::Go;
                        }
                        None => self.figures[*f].path.clear(),
                    }
                    self.figure_continue(*f);
                }
                self.events.push(Event::Sound { at: landing, sound: crate::SoundCue::Dock });
                set(self, Vec::new(), aboard, 2);
                self.ship_wake(s, 100);
            }
            2 => {
                // waiting at the landing until nobody of ours is out there any more
                let still: Vec<crate::FigId> = out
                    .iter()
                    .copied()
                    .filter(|f| {
                        self.figures.get(*f).is_some_and(|x| {
                            matches!(
                                x.kind,
                                FigKind::Soldier(
                                    SoldierTask::Attack { .. }
                                        | SoldierTask::AtTarget { .. }
                                        | SoldierTask::Return(_)
                                        | SoldierTask::ToShip { .. }
                                )
                            ) && !aboard.contains(f)
                        })
                    })
                    .collect();
                if !still.is_empty() {
                    set(self, aboard, still, 2);
                    self.ship_wake(s, 100);
                    return;
                }
                set(self, aboard, Vec::new(), 3);
                match home_coast {
                    Some(c) if self.ship_sail(s, c) => {}
                    _ => {
                        let all = self.ships_aboard(s);
                        self.sea_attack_disembark(s, &all, Some(landing));
                    }
                }
            }
            _ => {
                // home: everybody ashore, back to their posts
                let at = self.buildings.get(harbor).map(|h| h.pos);
                self.sea_attack_disembark(s, &aboard, at);
            }
        }
    }

    fn ships_aboard(&self, s: ShipId) -> Vec<crate::FigId> {
        match &self.ships[s].task {
            ShipTask::SeaAttack { aboard, .. } => aboard.clone(),
            _ => Vec::new(),
        }
    }

    /// Soldiers aboard step ashore at `at` (or where they boarded) and walk back to their posts;
    /// the ship is free again.
    fn sea_attack_disembark(&mut self, s: ShipId, aboard: &[crate::FigId], at: Option<MapCoord>) {
        self.ships[s].task = ShipTask::Idle;
        for f in aboard {
            let Some(fig) = self.figures.get_mut(*f) else { continue };
            if let Some(p) = at {
                fig.pos = p;
            }
            fig.inside = None;
            let home = match fig.kind {
                FigKind::Soldier(SoldierTask::ToShip { home, .. }) => home,
                _ => continue,
            };
            fig.kind = FigKind::Soldier(SoldierTask::Return(home));
            self.soldier_return_pub(*f, home);
        }
        self.ship_wake(s, 1);
    }

    /// An idle ship looks for the nearest harbor of its owner on its sea.
    fn ship_find_harbor(&mut self, s: ShipId) {
        let (player, sea, pos) = (self.ships[s].player, self.ships[s].sea, self.ships[s].pos);
        let g = self.world.grid();
        let mut hs: Vec<(u32, BuildingId, MapCoord)> = self
            .harbors_at(player, sea)
            .into_iter()
            .filter_map(|h| self.harbor_coast(self.buildings[h].pos).map(|c| (g.distance(pos, c), h, c)))
            .collect();
        hs.sort_by_key(|x| (x.0, x.1.index()));
        if let Some((_, h, c)) = hs.first().copied() {
            if c == pos {
                self.ships[s].docked = Some(h);
                self.start_building_timer(h, 1);
            } else {
                self.ships[s].task = ShipTask::ToHarbor(h);
                self.ship_sail(s, c);
            }
        } else {
            // no harbor yet: wait and look again later
            self.ship_wake(s, 500);
        }
    }

    /// What an expedition takes along: the harbor's boards and stones, and the colony's starter
    /// kit (ours, not RTTR's: owner, 2026-10-27, "make harbors meaningful"), enough for a
    /// guardhouse, woodcutter, forester, quarry and sawmill, so the colony can grow on its own.
    pub fn expedition_load(&self) -> (u8, u8) {
        let cost = self.rules.building(BuildingType::Harbor).cost();
        (cost.0 + COLONY_KIT.0, cost.1 + COLONY_KIT.1)
    }

    /// Harbor timer: prepare the expedition and load docked ships.
    pub(crate) fn harbor_tick(&mut self, h: BuildingId) {
        // collect expedition materials from the stock
        if let Some(mut e) = self.buildings[h].expedition.clone() {
            let cost = self.expedition_load();
            let inv = self.buildings[h].inventory.as_mut().expect("harbor stock");
            while e.boards < cost.0 && inv.wares[Ware::Boards.index()] > 0 {
                inv.wares[Ware::Boards.index()] -= 1;
                e.boards += 1;
            }
            while e.stones < cost.1 && inv.wares[Ware::Stones.index()] > 0 {
                inv.wares[Ware::Stones.index()] -= 1;
                e.stones += 1;
            }
            if !e.builder {
                if inv.jobs[Job::Builder.index()] > 0 {
                    inv.jobs[Job::Builder.index()] -= 1;
                    e.builder = true;
                } else if inv.jobs[Job::Helper.index()] > 0 && inv.wares[Ware::Hammer.index()] > 0 {
                    inv.jobs[Job::Helper.index()] -= 1;
                    inv.wares[Ware::Hammer.index()] -= 1;
                    e.builder = true;
                }
            }
            let complete = e.boards == cost.0 && e.stones == cost.1 && e.builder;
            let mut call = false;
            if complete && e.ship.is_none() {
                let ship =
                    self.ships.iter().find(|(_, s)| s.docked == Some(h) && s.task == ShipTask::Idle).map(|(id, _)| id);
                if let Some(s) = ship {
                    e.ship = Some(s);
                    self.ships[s].task = ShipTask::Expedition { target: None };
                } else {
                    // none here: one is called (RTTR nobHarborBuilding::OrderShip; Egypt's
                    // expedition waited for ever while its ships lay at its other harbor)
                    call = true;
                }
            }
            self.buildings[h].expedition = Some(e);
            if call {
                self.call_ship(h);
            }
        }
        // ship goods: one destination per trip
        if self.buildings[h].ship_queue.is_empty() {
            return;
        }
        let ship = self.ships.iter().find(|(_, s)| s.docked == Some(h) && s.task == ShipTask::Idle).map(|(id, _)| id);
        let Some(s) = ship else {
            self.call_ship(h);
            return;
        };
        let dest = self.buildings[h].ship_queue[0].1;
        let mut load = Vec::new();
        let mut rest = Vec::new();
        for (c, d) in std::mem::take(&mut self.buildings[h].ship_queue) {
            if d == dest && load.len() < SHIP_CAPACITY {
                load.push(c);
            } else {
                rest.push((c, d));
            }
        }
        self.buildings[h].ship_queue = rest;
        let coast = self.buildings.get(dest).and_then(|b| self.harbor_coast(b.pos));
        match coast {
            Some(c) => {
                self.ships[s].task = ShipTask::Cargo { to: dest, load };
                if self.ship_sail(s, c) {
                    // loading first (RTTR noShip: 200 GF)
                    self.ship_wake(s, SHIP_LOAD_GF);
                } else {
                    self.ships[s].task = ShipTask::Idle;
                }
            }
            None => {
                // destination gone: back into the stock
                if let Some(inv) = self.buildings[h].inventory.as_mut() {
                    for c in load {
                        match c {
                            Cargo::Ware(w) => inv.wares[w.index()] += 1,
                            Cargo::Person(j) => inv.jobs[j.index()] += 1,
                        }
                    }
                }
            }
        }
    }

    /// Call an idle ship of the harbor's owner from elsewhere on its sea, unless one is on its
    /// way there already.
    fn call_ship(&mut self, h: BuildingId) {
        let player = self.buildings[h].player;
        let Some(sea) = self.harbor_sea(self.buildings[h].pos) else { return };
        if self.ships.iter().any(|(_, x)| x.player == player && x.task == ShipTask::ToHarbor(h)) {
            return;
        }
        // the nearest idle ship, not the first in the list (veteran round 11, #7: 81 of 923
        // calls sent one about 24 nodes farther than needed); the coast is searched only if
        // there is one (perf round 3, #9)
        let idle_here = |x: &Ship| x.player == player && x.sea == sea && x.task == ShipTask::Idle && x.path.is_empty();
        if !self.ships.iter().any(|(_, x)| idle_here(x)) {
            return;
        }
        let Some(c) = self.harbor_coast(self.buildings[h].pos) else { return };
        let g = self.world.grid();
        let idle = self
            .ships
            .iter()
            .filter(|(_, x)| idle_here(x))
            .min_by_key(|(id, x)| (g.distance(x.pos, c), id.index()))
            .map(|(id, _)| id);
        if let Some(s) = idle {
            self.ships[s].task = ShipTask::ToHarbor(h);
            self.ship_sail(s, c);
        }
    }

    /// Supply an unserved request of building `b` from a harbor on another coast of the same sea.
    /// Returns true if something was set in motion.
    pub(crate) fn overseas_supply(&mut self, player: PlayerId, bflag: crate::FlagId, what: Cargo, count: u8) -> bool {
        match self.overseas_route(player, bflag) {
            Some(route) => self.overseas_supply_by(player, &route, what, count),
            None => false,
        }
    }

    /// Where an overseas request from flag `bflag` goes: the requester's harbor (the first on
    /// its roads) and the harbors of that sea not on its roads, in building order. Worked out
    /// once for all five ranks of soldiers, in one scan (perf round 3, #2: two scans of every
    /// building per rank, 20 % of late time).
    pub(crate) fn overseas_route(&self, player: PlayerId, bflag: crate::FlagId) -> Option<OverseasRoute> {
        let routes = self.routes_to(bflag);
        let on_roads = |h: &crate::Building| routes.get(h.flag.index()).is_some_and(|r| r.0 != u32::MAX);
        let harbors: Vec<(BuildingId, bool)> = self
            .buildings
            .iter()
            .filter(|(_, h)| h.player == player && h.kind == BuildingType::Harbor && h.is_ready())
            .map(|(id, h)| (id, on_roads(h)))
            .collect();
        let home = harbors.iter().find(|h| h.1)?.0;
        let sea = self.harbor_sea(self.buildings[home].pos)?;
        let sources = harbors
            .iter()
            .filter(|(id, roads)| *id != home && !roads && self.harbor_sea(self.buildings[*id].pos) == Some(sea))
            .map(|h| h.0)
            .collect();
        Some(OverseasRoute { home, sources })
    }

    /// `overseas_supply` along a route worked out before.
    pub(crate) fn overseas_supply_by(
        &mut self,
        player: PlayerId,
        route: &OverseasRoute,
        what: Cargo,
        count: u8,
    ) -> bool {
        let home = route.home;
        let i = idx(what);
        // what all requesters at this harbor asked for in this pass
        let asked = self.sea_asked.entry((home, i)).or_insert(0);
        *asked += count as u32;
        let asked = *asked;
        if self.buildings[home].sea_incoming[i] as u32 >= asked {
            return true; // already on its way
        }
        // a harbor elsewhere that has it (or can make the person)
        for &src in &route.sources {
            let rules = self.rules.clone();
            let inv = self.buildings[src].inventory.as_mut().expect("harbor stock");
            let have = match what {
                Cargo::Ware(w) => inv.wares[w.index()] > 0,
                Cargo::Person(j) => inv.can_supply(j, &rules),
            };
            if have {
                match what {
                    Cargo::Ware(w) => inv.wares[w.index()] -= 1,
                    Cargo::Person(j) => {
                        inv.take_person(j, &rules);
                    }
                }
                self.buildings[src].ship_queue.push((what, home));
                self.buildings[home].sea_incoming[i] += 1;
                self.start_building_timer(src, 1);
                return true;
            }
            // not there yet: the harbor asks its own warehouses for it
            match what {
                Cargo::Ware(w) => {
                    let d = &mut self.buildings[src].sea_demand[w.index()];
                    *d = (*d).max(1);
                }
                Cargo::Person(j) => {
                    if self.send_person_to_harbor(player, j, src, count) {
                        return true;
                    }
                }
            }
        }
        false
    }
}

/// A request's harbor and the harbors across its sea (`Sim::overseas_route`).
pub(crate) struct OverseasRoute {
    home: BuildingId,
    sources: Vec<BuildingId>,
}

impl Sim {
    /// A `job` from a warehouse on the same coast walks to harbor `h` (to be shipped from
    /// there), unless `count` of them are already on their way. True if one is coming.
    fn send_person_to_harbor(&mut self, player: PlayerId, job: Job, h: BuildingId, count: u8) -> bool {
        let hpos = self.buildings[h].pos;
        // (counted once a frame, not a scan of every figure per rank and post: perf-2 #5)
        if self.harbor_walkers.as_ref().is_none_or(|(gf, _)| *gf != self.gf) {
            let mut walkers = std::collections::BTreeMap::new();
            for (_, f) in self.figures.iter() {
                if f.kind == FigKind::Home
                    && let Some(t) = f.target
                {
                    *walkers.entry((f.player, f.job, t)).or_insert(0) += 1;
                }
            }
            self.harbor_walkers = Some((self.gf, walkers));
        }
        let coming = self.harbor_walkers.as_ref().and_then(|(_, w)| w.get(&(player, job, hpos)).copied()).unwrap_or(0);
        if coming >= count as usize {
            return coming > 0;
        }
        let hflag = self.buildings[h].flag;
        let routes = self.routes_to(hflag);
        let rules = self.rules.clone();
        let src = self
            .buildings
            .iter()
            .filter(|(id, b)| *id != h && b.player == player && b.is_ready())
            .filter(|(_, b)| b.inventory.as_ref().is_some_and(|i| i.job(job) > 0 || i.can_make(job, &rules)))
            .filter_map(|(id, b)| {
                let d = routes.get(b.flag.index()).map_or(u32::MAX, |r| r.0);
                (d != u32::MAX).then_some((d, id, b.flag))
            })
            .min_by_key(|x| (x.0, x.1.index()));
        let Some((_, wh, _)) = src else { return false };
        let inv = self.buildings[wh].inventory.as_mut().expect("warehouse");
        if inv.job(job) == 0 && inv.take_person(job, &rules) {
            inv.jobs[job.index()] += 1;
        }
        // it leaves the warehouse towards the harbor (see `figure_leave_building`)
        let f = self.spawn_from_warehouse(wh, job, FigKind::Home);
        self.figures[f].target = Some(hpos);
        if let Some((_, w)) = self.harbor_walkers.as_mut() {
            *w.entry((player, job, hpos)).or_insert(0) += 1;
        }
        true
    }

    pub(crate) fn cmd_start_expedition(&mut self, player: PlayerId, at: MapCoord) -> Result<(), RejectReason> {
        let h = self.building_at(at).ok_or(RejectReason::NotFound)?;
        let b = &self.buildings[h];
        if b.player != player || b.kind != BuildingType::Harbor || !b.is_ready() || b.expedition.is_some() {
            return Err(RejectReason::NotAllowed);
        }
        self.buildings[h].expedition = Some(Expedition::default());
        self.start_building_timer(h, 1);
        Ok(())
    }

    /// Is the expedition of harbor `h` loaded on a ship and waiting for a target?
    pub fn expedition_ready(&self, h: BuildingId) -> bool {
        self.buildings
            .get(h)
            .and_then(|b| b.expedition.as_ref())
            .and_then(|e| e.ship)
            .is_some_and(|s| self.ships.get(s).is_some_and(|x| x.task == ShipTask::Expedition { target: None }))
    }

    /// Harbor spots `player`'s ship at `sea` could found a colony on.
    pub fn free_harbor_spots(&self, player: PlayerId, sea: u16) -> Vec<MapCoord> {
        self.world
            .grid()
            .coords()
            .filter(|p| self.world.is_harbor_spot(*p) && self.harbor_sea(*p) == Some(sea))
            .filter(|p| self.world.owner(*p).is_none() && self.colony_spot_free(player, *p))
            .collect()
    }

    /// Could `player`'s expedition land at `spot` now: the spot free land or ours, and its flag
    /// node too (one check for the AI, the button and the landing; QA round 5, #2: where a
    /// rival owned the flag node an expedition was sent there 203 times).
    pub fn colony_spot_free(&self, player: PlayerId, spot: MapCoord) -> bool {
        let se = self.world.grid().neighbor(spot, Direction::SE);
        let se_ok = match self.flag_at(se) {
            Some(fl) => self.flags[fl].player == player,
            None => {
                self.world.owner(se).is_none_or(|o| o == player)
                    && !matches!(
                        self.world.obj(se),
                        rr_world::NodeObj::Building(_) | rr_world::NodeObj::Extension | rr_world::NodeObj::Flag(_)
                    )
            }
        };
        self.world.owner(spot).is_none_or(|o| o == player)
            && self.world.obj(spot).blocking() == rr_world::Blocking::None
            && self.building_at(spot).is_none()
            && se_ok
    }

    pub(crate) fn cmd_send_expedition(
        &mut self,
        player: PlayerId,
        at: MapCoord,
        to: MapCoord,
    ) -> Result<(), RejectReason> {
        let h = self.building_at(at).ok_or(RejectReason::NotFound)?;
        if self.buildings[h].player != player || !self.expedition_ready(h) {
            return Err(RejectReason::NotAllowed);
        }
        let s = self.buildings[h].expedition.as_ref().and_then(|e| e.ship).expect("ready");
        if !self.world.is_harbor_spot(to) || self.harbor_sea(to) != Some(self.ships[s].sea) {
            return Err(RejectReason::NotBuildable);
        }
        let coast = self.harbor_coast(to).ok_or(RejectReason::NotBuildable)?;
        self.ships[s].task = ShipTask::Expedition { target: Some(to) };
        if !self.ship_sail(s, coast) {
            self.ships[s].task = ShipTask::Expedition { target: None };
            return Err(RejectReason::NotFound);
        }
        // the expedition leaves the harbor
        if let Some(e) = self.buildings[h].expedition.as_mut() {
            e.ship = Some(s);
        }
        let e = self.buildings[h].expedition.take();
        self.buildings[h].expedition = None;
        self.ships[s].docked = None;
        let _ = e;
        Ok(())
    }

    /// An expedition ship without a harbor (its landing failed, or its harbor burnt) sails to the
    /// nearest own harbor on its sea that has no loaded expedition and becomes that harbor's
    /// expedition, ready for a new target. With no such harbor it looks again later.
    pub(crate) fn expedition_return(&mut self, s: ShipId) {
        let (player, sea, pos) = (self.ships[s].player, self.ships[s].sea, self.ships[s].pos);
        self.ships[s].task = ShipTask::Expedition { target: None };
        let g = self.world.grid();
        let mut hs: Vec<(u32, BuildingId, MapCoord)> = self
            .harbors_at(player, sea)
            .into_iter()
            .filter(|h| self.buildings[*h].expedition.as_ref().is_none_or(|e| e.ship.is_none()))
            .filter_map(|h| self.harbor_coast(self.buildings[h].pos).map(|c| (g.distance(pos, c), h, c)))
            .collect();
        hs.sort_by_key(|x| (x.0, x.1.index()));
        let Some((_, h, coast)) = hs.first().copied() else {
            self.ship_wake(s, 500);
            return;
        };
        // materials being gathered there go back into its stock: the ship brings a full set
        if let Some(old) = self.buildings[h].expedition.take()
            && let Some(inv) = self.buildings[h].inventory.as_mut()
        {
            inv.wares[Ware::Boards.index()] += old.boards as u32;
            inv.wares[Ware::Stones.index()] += old.stones as u32;
            if old.builder {
                inv.jobs[Job::Builder.index()] += 1;
            }
        }
        let load = self.expedition_load();
        self.buildings[h].expedition =
            Some(Expedition { boards: load.0, stones: load.1, builder: true, ship: Some(s) });
        if coast != pos {
            self.ship_sail(s, coast);
        }
    }

    /// The expedition lands: a harbor site with all materials and a builder (RTTR FoundColony).
    fn found_colony(&mut self, s: ShipId, spot: MapCoord) {
        let player = self.ships[s].player;
        let se = self.world.grid().neighbor(spot, Direction::SE);
        if !self.colony_spot_free(player, spot) {
            // spot taken meanwhile: back to a harbor, to wait for a new target there
            self.events.push(Event::NoResources { player, at: spot });
            self.expedition_return(s);
            return;
        }
        if self.world.obj(se).blocking() != rr_world::Blocking::None && self.flag_at(se).is_none() {
            self.world.set_obj(se, rr_world::NodeObj::None);
        }
        let flag = match self.flag_at(se) {
            Some(f) => f,
            None => self.create_flag(player, se),
        };
        let cost = self.rules.building(BuildingType::Harbor).cost();
        let site = Site { boards: cost.0, stones: cost.1, from_sea: true, ..Default::default() };
        let b = self.insert_site_from_sea(player, spot, flag, site);
        // the builder steps ashore
        let f = self.new_figure(player, Job::Builder, spot, FigKind::Builder { site: b });
        self.buildings[b].worker_coming = true;
        let key = self.schedule(5, Ev::Figure(f));
        self.figures[f].ev = Some(key);
        self.territory_dirty = true;
        self.ships[s].task = ShipTask::Idle;
        self.ship_wake(s, 100);
        self.events.push(Event::BuildingFinished { player, at: spot, kind: BuildingType::Harbor });
    }

    fn insert_site_from_sea(
        &mut self,
        player: PlayerId,
        spot: MapCoord,
        flag: crate::FlagId,
        site: Site,
    ) -> BuildingId {
        self.insert_building_pub(player, BuildingType::Harbor, spot, flag, BState::Site(site))
    }
}
