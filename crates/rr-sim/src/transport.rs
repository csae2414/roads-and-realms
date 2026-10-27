//! Flags, roads and the route finder over the flag graph.

use crate::{BuildingId, FigId, FlagId, PlayerId, RejectReason, RoadId, Sim, WareId};
use rr_core::{Direction, MapCoord};
use rr_world::{Bq, NodeObj, RoadKind};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Longest boat road in edges (RTTR AddonMaxWaterwayLength default).
pub const MAX_WATERWAY: usize = 5;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Flag {
    pub pos: MapCoord,
    pub player: PlayerId,
    /// Wares lying at the flag (at most `flag_capacity`).
    pub wares: Vec<WareId>,
    /// Road leaving in each direction (indexed by `Direction::index`). NW leads to the building.
    pub roads: [Option<RoadId>; 6],
    /// Building at the NW neighbor, if any.
    pub building: Option<BuildingId>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Road {
    pub player: PlayerId,
    pub from: FlagId,
    pub to: FlagId,
    pub start: MapCoord,
    pub dirs: Vec<Direction>,
    pub carrier: Option<FigId>,
    pub donkey: Option<FigId>,
    pub kind: RoadKind,
    /// A carrier has been requested from a warehouse but not yet assigned.
    pub carrier_wanted: bool,
    /// Frames the carrier was busy in the current window (for donkey upgrades).
    pub busy: u32,
    /// A pack donkey has been requested.
    pub donkey_wanted: bool,
}

impl Road {
    pub fn len(&self) -> usize {
        self.dirs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.dirs.is_empty()
    }

    /// All nodes from the start flag to the end flag.
    pub fn nodes(&self, g: rr_core::Grid) -> Vec<MapCoord> {
        let mut out = Vec::with_capacity(self.dirs.len() + 1);
        let mut p = self.start;
        out.push(p);
        for d in &self.dirs {
            p = g.neighbor(p, *d);
            out.push(p);
        }
        out
    }

    /// The flag at the other end.
    pub fn other(&self, f: FlagId) -> FlagId {
        if f == self.from { self.to } else { self.from }
    }

    /// Direction in which this road leaves flag `f`.
    pub fn dir_at(&self, f: FlagId) -> Direction {
        if f == self.from { self.dirs[0] } else { self.dirs[self.dirs.len() - 1].opposite() }
    }
}

/// Breadth-first search from `from` to `to` over steps that `step_ok(node, dir, reaches_goal)`
/// allows, at most `max_len` steps: the directions of a shortest path. Ties go the same way
/// every time (directions in their fixed order).
/// Why a road can't be built (see [`Sim::road_trouble`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoadTrouble {
    NotYourLand,
    /// A building, tree, stone or water is in the way at the goal.
    Blocked,
    TooFar,
    /// Another road is in the way; a flag on it at this node would let the roads meet.
    CrossesRoad(MapCoord),
    NoWay,
}

pub(crate) fn bfs_path(
    g: rr_core::Grid,
    from: MapCoord,
    to: MapCoord,
    max_len: usize,
    step_ok: impl FnMut(MapCoord, Direction, bool) -> bool,
) -> Option<Vec<Direction>> {
    bfs_until(g, from, |q| q == to, max_len, step_ok).map(|(_, dirs)| dirs)
}

/// Breadth-first search to the nearest node that `is_goal`; returns it and the way there.
pub(crate) fn bfs_until(
    g: rr_core::Grid,
    from: MapCoord,
    is_goal: impl Fn(MapCoord) -> bool,
    max_len: usize,
    mut step_ok: impl FnMut(MapCoord, Direction, bool) -> bool,
) -> Option<(MapCoord, Vec<Direction>)> {
    let mut prev: std::collections::BTreeMap<MapCoord, (MapCoord, Direction)> = std::collections::BTreeMap::new();
    let mut queue = std::collections::VecDeque::from([(from, 0usize)]);
    prev.insert(from, (from, Direction::W));
    while let Some((p, len)) = queue.pop_front() {
        if len >= max_len {
            continue;
        }
        for d in Direction::ALL {
            let q = g.neighbor(p, d);
            if prev.contains_key(&q) {
                continue;
            }
            let is_goal = is_goal(q);
            if !step_ok(p, d, is_goal) {
                continue;
            }
            prev.insert(q, (p, d));
            if is_goal {
                let mut dirs = Vec::new();
                let mut cur = q;
                while cur != from {
                    let (pp, dd) = prev[&cur];
                    dirs.push(dd);
                    cur = pp;
                }
                dirs.reverse();
                return Some((q, dirs));
            }
            queue.push_back((q, len + 1));
        }
    }
    None
}

impl Sim {
    pub(crate) fn create_flag(&mut self, player: PlayerId, at: MapCoord) -> FlagId {
        let id = self.flags.insert(Flag { pos: at, player, wares: Vec::new(), roads: [None; 6], building: None });
        self.world.set_obj(at, NodeObj::Flag(id.index() as u32));
        self.players[player as usize].road_epoch += 1;
        self.route_cache.borrow_mut().clear();
        if let Some(r) = self.road_at(at) {
            self.split_road(r, at, id);
        }
        id
    }

    pub(crate) fn cmd_place_flag(&mut self, player: PlayerId, at: MapCoord) -> Result<FlagId, RejectReason> {
        if !self.world.grid().contains(at) {
            return Err(RejectReason::NotFound);
        }
        if self.world.owner(at) != Some(player) {
            return Err(RejectReason::NotYourTerritory);
        }
        if self.world.bq_for(at, player) == Bq::Nothing {
            return Err(RejectReason::NotBuildable);
        }
        Ok(self.create_flag(player, at))
    }

    fn split_road(&mut self, r: RoadId, at: MapCoord, mid: FlagId) {
        let g = self.world.grid();
        let road = self.roads[r].clone();
        let nodes = road.nodes(g);
        let k = nodes.iter().position(|p| *p == at).expect("node on road");
        // first part keeps the id (and the carrier); the second is new
        {
            let r1 = &mut self.roads[r];
            r1.to = mid;
            r1.dirs.truncate(k);
        }
        let r2 = self.roads.insert(Road {
            player: road.player,
            from: mid,
            to: road.to,
            start: at,
            dirs: road.dirs[k..].to_vec(),
            carrier: None,
            donkey: None,
            kind: road.kind,
            carrier_wanted: false,
            busy: 0,
            donkey_wanted: false,
        });
        let end_dir = road.dirs[road.dirs.len() - 1].opposite();
        self.flags[road.to].roads[end_dir.index() as usize] = Some(r2);
        self.flags[mid].roads[road.dirs[k - 1].opposite().index() as usize] = Some(r);
        self.flags[mid].roads[road.dirs[k].index() as usize] = Some(r2);
        let gi = g.index(at);
        self.road_nodes[gi] = None;
        for p in &nodes[k + 1..nodes.len() - 1] {
            self.road_nodes[g.index(*p)] = Some(r2);
        }
        // both halves stay what the road was (a donkey road keeps its upgrade)
        self.on_road_split(r, r2);
    }

    /// Validate a road without changing anything. Returns the end flag, if one exists there.
    pub fn check_road(
        &self,
        player: PlayerId,
        from: MapCoord,
        dirs: &[Direction],
    ) -> Result<Option<FlagId>, RejectReason> {
        let g = self.world.grid();
        let start = self.flag_at(from).ok_or(RejectReason::NoFlag)?;
        if self.flags[start].player != player {
            return Err(RejectReason::NotAllowed);
        }
        if dirs.len() < 2 {
            return Err(RejectReason::BadRoad);
        }
        if self.flags[start].roads[dirs[0].index() as usize].is_some() {
            return Err(RejectReason::BadRoad);
        }
        let boat = self.is_water_point(g.neighbor(from, dirs[0]));
        if boat && dirs.len() > MAX_WATERWAY {
            return Err(RejectReason::BadRoad);
        }
        let mut p = from;
        let mut seen = vec![from];
        for (i, &d) in dirs.iter().enumerate() {
            let last = i + 1 == dirs.len();
            let q = g.neighbor(p, d);
            if seen.contains(&q) {
                return Err(RejectReason::BadRoad);
            }
            if boat {
                self.boat_step_ok(player, p, d, last)?;
            } else {
                self.road_step_ok(player, p, d, last)?;
            }
            seen.push(q);
            p = q;
        }
        let end_dir = dirs[dirs.len() - 1].opposite();
        match self.flag_at(p) {
            Some(f) => {
                if f == start
                    || self.flags[f].player != player
                    || self.flags[f].roads[end_dir.index() as usize].is_some()
                {
                    return Err(RejectReason::BadRoad);
                }
                Ok(Some(f))
            }
            None => {
                if self.world.bq_for(p, player) == Bq::Nothing {
                    return Err(RejectReason::NotBuildable);
                }
                Ok(None)
            }
        }
    }

    /// Can a road of `player` go from `p` one step in direction `d`?
    pub fn road_step_ok(&self, player: PlayerId, p: MapCoord, d: Direction, last: bool) -> Result<(), RejectReason> {
        let q = self.world.grid().neighbor(p, d);
        if !self.world.edge_walkable(p, d) || !self.world.node_walkable(q) {
            return Err(RejectReason::BadRoad);
        }
        if self.world.owner(q) != Some(player) {
            return Err(RejectReason::NotYourTerritory);
        }
        match self.world.obj(q) {
            NodeObj::Flag(_) if last => {}
            o if o.blocking() == rr_world::Blocking::None => {}
            _ => return Err(RejectReason::BadRoad),
        }
        if !last && self.world.is_on_road(q) {
            return Err(RejectReason::BadRoad);
        }
        Ok(())
    }

    /// Every triangle around the node is water: boat roads may pass here.
    pub fn is_water_point(&self, p: MapCoord) -> bool {
        self.world.terrains_around(p).iter().all(|t| rr_world::terrain::is_water(*t))
    }

    /// A step of a boat road (RTTR IsRoadAvailable with `boat_road`): water all around the nodes
    /// in between, a land flag at the end.
    pub fn boat_step_ok(&self, player: PlayerId, p: MapCoord, d: Direction, last: bool) -> Result<(), RejectReason> {
        let q = self.world.grid().neighbor(p, d);
        if self.world.owner(q) != Some(player) {
            return Err(RejectReason::NotYourTerritory);
        }
        if last {
            if !self.world.node_walkable(q) {
                return Err(RejectReason::BadRoad);
            }
            return match self.world.obj(q) {
                NodeObj::Flag(_) => Ok(()),
                o if o.blocking() == rr_world::Blocking::None => Ok(()),
                _ => Err(RejectReason::BadRoad),
            };
        }
        if !self.is_water_point(q)
            || self.world.is_on_road(q)
            || self.world.obj(q).blocking() != rr_world::Blocking::None
        {
            return Err(RejectReason::BadRoad);
        }
        Ok(())
    }

    /// Shortest buildable road from `from` to `to` (breadth-first), for the road tool.
    pub fn road_path(
        &self,
        player: PlayerId,
        from: MapCoord,
        to: MapCoord,
        avoid: &[MapCoord],
    ) -> Option<Vec<Direction>> {
        self.land_road_path(player, from, to, avoid).or_else(|| self.boat_road_path(player, from, to))
    }

    /// The road tool's goal for a click: a building (or the extra nodes of a large one) means
    /// its flag.
    pub fn road_click_target(&self, p: MapCoord) -> MapCoord {
        let g = self.world.grid();
        let site = match self.world.obj(p) {
            NodeObj::Building(_) => Some(p),
            // extensions lie W, NW and NE of the building node
            NodeObj::Extension => [Direction::E, Direction::SE, Direction::SW]
                .into_iter()
                .map(|d| g.neighbor(p, d))
                .find(|q| self.building_at(*q).is_some()),
            _ => None,
        };
        site.map_or(p, |b| g.neighbor(b, Direction::SE))
    }

    /// A road from `from` to the network of the flag at `to`: to the nearest flag connected to
    /// it, or to a road node of that network where a flag can go (place one there first).
    pub fn road_join_path(
        &self,
        player: PlayerId,
        from: MapCoord,
        to: MapCoord,
        avoid: &[MapCoord],
    ) -> Option<(MapCoord, Vec<Direction>)> {
        let g = self.world.grid();
        let routes = self.routes_to(self.flag_at(to)?);
        let on_net = |f: FlagId| routes.get(f.index()).is_some_and(|x| x.0 != u32::MAX);
        let goal = |q: MapCoord| match self.flag_at(q) {
            Some(f) => on_net(f),
            None => {
                self.road_at(q).is_some_and(|r| on_net(self.roads[r].from))
                    && self.world.owner(q) == Some(player)
                    && self.world.bq_for(q, player) != Bq::Nothing
            }
        };
        bfs_until(g, from, goal, g.distance(from, to) as usize + 8, |p, d, last| {
            let q = g.neighbor(p, d);
            !avoid.contains(&q)
                && self.road_step_ok(player, p, d, last).is_ok()
                && (last || !matches!(self.world.obj(q), NodeObj::Flag(_)))
        })
    }

    /// Why [`Sim::road_path`] found no road, for the message to the player.
    pub fn road_trouble(&self, player: PlayerId, from: MapCoord, to: MapCoord, avoid: &[MapCoord]) -> RoadTrouble {
        let g = self.world.grid();
        if self.world.owner(to) != Some(player) {
            return RoadTrouble::NotYourLand;
        }
        let obj = self.world.obj(to);
        if !self.world.node_walkable(to)
            || !matches!(obj, NodeObj::Flag(_)) && obj.blocking() != rr_world::Blocking::None
        {
            return RoadTrouble::Blocked;
        }
        if g.distance(from, to) > 40 {
            return RoadTrouble::TooFar;
        }
        // the same search, but over other roads: the first one it crosses needs a flag
        let limit = g.distance(from, to) + 8;
        let over_roads = bfs_path(g, from, to, usize::MAX, |p, d, goal| {
            let q = g.neighbor(p, d);
            !avoid.contains(&q)
                && self.road_step_ok(player, p, d, true).is_ok()
                && (goal || !matches!(self.world.obj(q), NodeObj::Flag(_)))
                && g.distance(q, to) <= limit
        });
        let mut p = from;
        for d in over_roads.unwrap_or_default() {
            p = g.neighbor(p, d);
            if self.world.is_on_road(p) && self.flag_at(p).is_none() {
                return RoadTrouble::CrossesRoad(p);
            }
        }
        RoadTrouble::NoWay
    }

    /// Shortest boat road (at most `MAX_WATERWAY` edges) between two shore nodes.
    fn boat_road_path(&self, player: PlayerId, from: MapCoord, to: MapCoord) -> Option<Vec<Direction>> {
        let g = self.world.grid();
        if g.distance(from, to) as usize > MAX_WATERWAY {
            return None;
        }
        bfs_path(g, from, to, MAX_WATERWAY, |p, d, goal| self.boat_step_ok(player, p, d, goal).is_ok())
            .filter(|dirs| dirs.len() >= 2)
    }

    fn land_road_path(
        &self,
        player: PlayerId,
        from: MapCoord,
        to: MapCoord,
        avoid: &[MapCoord],
    ) -> Option<Vec<Direction>> {
        let g = self.world.grid();
        if from == to || g.distance(from, to) > 40 {
            return None;
        }
        let limit = g.distance(from, to) + 8;
        bfs_path(g, from, to, usize::MAX, |p, d, goal| {
            let q = g.neighbor(p, d);
            // flags end a road; only the goal may be one
            !avoid.contains(&q)
                && self.road_step_ok(player, p, d, goal).is_ok()
                && (goal || !matches!(self.world.obj(q), NodeObj::Flag(_)))
                && g.distance(q, to) <= limit
        })
    }

    pub(crate) fn cmd_build_road(
        &mut self,
        player: PlayerId,
        from: MapCoord,
        dirs: &[Direction],
    ) -> Result<RoadId, RejectReason> {
        let end_flag = self.check_road(player, from, dirs)?;
        let start = self.flag_at(from).expect("checked");
        let mut end_pos = from;
        for d in dirs {
            end_pos = self.world.grid().neighbor(end_pos, *d);
        }
        let end = match end_flag {
            Some(f) => f,
            None => self.create_flag(player, end_pos),
        };
        let id = self.create_road(player, start, end, from, dirs.to_vec());
        if self.is_water_point(self.world.grid().neighbor(from, dirs[0])) {
            self.set_road_kind(id, RoadKind::Boat);
        }
        Ok(id)
    }

    pub(crate) fn create_road(
        &mut self,
        player: PlayerId,
        from: FlagId,
        to: FlagId,
        start: MapCoord,
        dirs: Vec<Direction>,
    ) -> RoadId {
        let g = self.world.grid();
        let id = self.roads.insert(Road {
            player,
            from,
            to,
            start,
            dirs: dirs.clone(),
            carrier: None,
            donkey: None,
            kind: RoadKind::Normal,
            carrier_wanted: false,
            busy: 0,
            donkey_wanted: false,
        });
        let mut p = start;
        for (i, d) in dirs.iter().enumerate() {
            self.world.set_road(p, *d, RoadKind::Normal);
            p = g.neighbor(p, *d);
            if i + 1 < dirs.len() {
                self.road_nodes[g.index(p)] = Some(id);
            }
        }
        self.flags[from].roads[dirs[0].index() as usize] = Some(id);
        self.flags[to].roads[dirs[dirs.len() - 1].opposite().index() as usize] = Some(id);
        self.players[player as usize].road_epoch += 1;
        self.route_cache.borrow_mut().clear();
        self.on_road_created(id);
        self.reroute_flag(from);
        self.reroute_flag(to);
        id
    }

    pub(crate) fn set_road_kind(&mut self, id: RoadId, kind: RoadKind) {
        let road = &self.roads[id];
        let g = self.world.grid();
        let mut p = road.start;
        let dirs = road.dirs.clone();
        for d in dirs {
            self.world.set_road(p, d, kind);
            p = g.neighbor(p, d);
        }
        self.roads[id].kind = kind;
    }

    /// Carriers that are busy most of the time get a pack donkey (RTTR `nofCarrier`: 80 % of 6000 GF).
    pub(crate) fn transport_tick(&mut self) {
        const SAMPLE: u32 = 100;
        if !self.gf.is_multiple_of(SAMPLE) {
            return;
        }
        let window = self.rules.economy.donkey_window;
        let threshold = self.rules.economy.donkey_busy_percent;
        let check = self.gf.is_multiple_of(window) && self.gf > 0;
        for r in self.roads.ids() {
            let Some(c) = self.roads[r].carrier else { continue };
            let busy = self.figures.get(c).is_some_and(|f| !matches!(f.step, crate::figures::Step::CIdle));
            if busy {
                self.roads[r].busy += SAMPLE;
            }
            if check {
                let pct = self.roads[r].busy * 100 / window;
                self.roads[r].busy = 0;
                if pct >= threshold && self.roads[r].kind == RoadKind::Normal {
                    self.set_road_kind(r, RoadKind::Donkey);
                }
            }
        }
    }

    pub(crate) fn destroy_road(&mut self, id: RoadId) {
        let Some(road) = self.roads.get(id).cloned() else {
            return;
        };
        let g = self.world.grid();
        let mut p = road.start;
        for d in &road.dirs {
            self.world.set_road(p, *d, RoadKind::None);
            let q = g.neighbor(p, *d);
            if self.road_nodes[g.index(q)] == Some(id) {
                self.road_nodes[g.index(q)] = None;
            }
            p = q;
        }
        for f in [road.from, road.to] {
            if let Some(flag) = self.flags.get_mut(f) {
                for slot in flag.roads.iter_mut() {
                    if *slot == Some(id) {
                        *slot = None;
                    }
                }
            }
        }
        self.roads.remove(id);
        self.players[road.player as usize].road_epoch += 1;
        self.route_cache.borrow_mut().clear();
        self.on_road_destroyed(&road);
        self.reroute_flag(road.from);
        self.reroute_flag(road.to);
    }

    pub(crate) fn destroy_flag(&mut self, id: FlagId) {
        let Some(flag) = self.flags.get(id).cloned() else {
            return;
        };
        // (a finished building burns with its flag: RTTR noFlag::DestroyAttachedBuilding)
        if let Some(b) = flag.building {
            self.destroy_building(b, true);
        }
        for r in flag.roads.iter().flatten() {
            self.destroy_road(*r);
        }
        for w in &flag.wares {
            self.lose_ware(*w);
        }
        self.flags.remove(id);
        self.world.set_obj(flag.pos, NodeObj::None);
        self.players[flag.player as usize].road_epoch += 1;
        self.route_cache.borrow_mut().clear();
    }

    pub(crate) fn cmd_remove_flag(&mut self, player: PlayerId, at: MapCoord) -> Result<(), RejectReason> {
        let f = self.flag_at(at).ok_or(RejectReason::NoFlag)?;
        if self.flags[f].player != player {
            return Err(RejectReason::NotAllowed);
        }
        if let Some(b) = self.flags[f].building
            && self.buildings[b].kind == rr_rules::BuildingType::Headquarters
        {
            return Err(RejectReason::NotAllowed);
        }
        self.destroy_flag(f);
        Ok(())
    }

    pub(crate) fn cmd_remove_road(
        &mut self,
        player: PlayerId,
        at: MapCoord,
        dir: Direction,
    ) -> Result<(), RejectReason> {
        let f = self.flag_at(at).ok_or(RejectReason::NoFlag)?;
        if self.flags[f].player != player || dir == Direction::NW && self.flags[f].building.is_some() {
            return Err(RejectReason::NotAllowed);
        }
        let r = self.flags[f].roads[dir.index() as usize].ok_or(RejectReason::NotFound)?;
        self.destroy_road(r);
        Ok(())
    }

    /// Shortest road distance (in edges) from every flag to `goal`, plus the first direction to
    /// take. Deterministic: ties break on flag index.
    pub(crate) fn routes_to(&self, goal: FlagId) -> Routes {
        // depends only on the road network: cached until it changes (cleared with road_epoch);
        // shared, not copied (a table as long as the flag arena, asked for many times a pass)
        if let Some(r) = self.route_cache.borrow().get(&goal) {
            return r.clone();
        }
        let r: Routes = self.routes_impl(goal, false, None).into();
        self.route_cache.borrow_mut().insert(goal, r.clone());
        r
    }

    /// Routes for wares: like `routes_to`, plus RTTR's punishment points (busy or unserved roads).
    pub(crate) fn ware_routes_to(&self, goal: FlagId) -> Vec<(u32, Option<Direction>)> {
        self.routes_impl(goal, true, None)
    }

    /// The next road for a ware at `from` heading to `goal` (stops searching once `from` is
    /// settled; same result as the full search).
    pub(crate) fn ware_next_step(&self, from: FlagId, goal: FlagId) -> Option<Direction> {
        // as `routes_impl(goal, true, Some(from))`, in a stamped table kept between searches:
        // a table as long as the flag arena was allocated and filled per ware put down (perf-2
        // #4: 40 % of the sim on 384²)
        let n = self.flags_capacity();
        let mut scratch = self.ware_scratch.borrow_mut();
        let WareScratch { stamp, best, search } = &mut *scratch;
        if stamp.len() < n {
            stamp.resize(n, 0);
            best.resize(n, (u32::MAX, None));
        }
        *search = search.wrapping_add(1);
        if *search == 0 {
            stamp.fill(0);
            *search = 1;
        }
        let now = *search;
        let get = |i: usize, stamp: &[u32], best: &[(u32, Option<Direction>)]| {
            if stamp[i] == now { best[i] } else { (u32::MAX, None) }
        };
        let player = self.flags[goal].player;
        stamp[goal.index()] = now;
        best[goal.index()] = (0, None);
        let mut heap = BinaryHeap::new();
        heap.push(Reverse((0u32, goal.index())));
        while let Some(Reverse((d, fi))) = heap.pop() {
            if d > get(fi, stamp, best).0 {
                continue;
            }
            if from.index() == fi {
                break;
            }
            let fid = self.flags.id_at(fi).expect("live flag");
            for r in self.flags[fid].roads.iter().flatten() {
                let road = &self.roads[*r];
                if road.player != player {
                    continue;
                }
                let other = road.other(fid);
                let nd = d + road.len() as u32 + self.ware_penalty(*r, other);
                let oi = other.index();
                if nd < get(oi, stamp, best).0 {
                    stamp[oi] = now;
                    best[oi] = (nd, Some(road.dir_at(other)));
                    heap.push(Reverse((nd, oi)));
                }
            }
        }
        get(from.index(), stamp, best).1
    }

    pub(crate) fn routes_impl(&self, goal: FlagId, wares: bool, stop: Option<FlagId>) -> Vec<(u32, Option<Direction>)> {
        let n = self.flags_capacity();
        let mut best: Vec<(u32, Option<Direction>)> = vec![(u32::MAX, None); n];
        let player = self.flags[goal].player;
        best[goal.index()] = (0, None);
        let mut heap = BinaryHeap::new();
        heap.push(Reverse((0u32, goal.index())));
        while let Some(Reverse((d, fi))) = heap.pop() {
            if d > best[fi].0 {
                continue;
            }
            if stop.is_some_and(|s| s.index() == fi) {
                break;
            }
            let fid = self.flags.id_at(fi).expect("live flag");
            for r in self.flags[fid].roads.iter().flatten() {
                let road = &self.roads[*r];
                // boat roads carry wares; people can't walk on water
                if road.player != player || !wares && road.kind == RoadKind::Boat {
                    continue;
                }
                let other = road.other(fid);
                let nd = d + road.len() as u32 + if wares { self.ware_penalty(*r, other) } else { 0 };
                let oi = other.index();
                if nd < best[oi].0 {
                    best[oi] = (nd, Some(road.dir_at(other)));
                    heap.push(Reverse((nd, oi)));
                }
            }
        }
        best
    }

    /// RTTR noFlag::GetPunishmentPoints: 2 per ware already waiting at `at` for this road,
    /// +50 while the carrier is on its way, +500 when the road has no carrier at all.
    fn ware_penalty(&self, r: RoadId, at: FlagId) -> u32 {
        let road = &self.roads[r];
        let dir = road.dir_at(at);
        let waiting =
            self.flags[at].wares.iter().filter(|w| self.wares.get(**w).is_some_and(|x| x.next == Some(dir))).count()
                as u32;
        let carrier = if road.carrier.is_some() {
            0
        } else if road.carrier_wanted {
            50
        } else {
            500
        };
        waiting * 2 + carrier
    }

    pub(crate) fn flags_capacity(&self) -> usize {
        self.flags.capacity()
    }

    /// Road distance between two flags, if connected.
    pub fn flag_distance(&self, a: FlagId, b: FlagId) -> Option<u32> {
        let r = self.routes_to(b);
        r.get(a.index()).map(|x| x.0).filter(|d| *d != u32::MAX)
    }

    /// The flag sequence and road list from `a` to `b`.
    pub(crate) fn flag_path(&self, a: FlagId, b: FlagId) -> Option<Vec<RoadId>> {
        let routes = self.routes_to(b);
        if routes.get(a.index())?.0 == u32::MAX {
            return None;
        }
        let mut out = Vec::new();
        let mut cur = a;
        while cur != b {
            let dir = routes[cur.index()].1?;
            let r = self.flags[cur].roads[dir.index() as usize]?;
            out.push(r);
            cur = self.roads[r].other(cur);
            if out.len() > 10_000 {
                return None;
            }
        }
        Some(out)
    }

    /// Node-by-node directions from flag `a` to flag `b` along roads.
    pub(crate) fn road_walk(&self, a: FlagId, b: FlagId) -> Option<Vec<Direction>> {
        let roads = self.flag_path(a, b)?;
        let mut cur = a;
        let mut dirs = Vec::new();
        for r in roads {
            let road = &self.roads[r];
            if road.from == cur {
                dirs.extend(road.dirs.iter().copied());
            } else {
                dirs.extend(road.dirs.iter().rev().map(|d| d.opposite()));
            }
            cur = road.other(cur);
        }
        Some(dirs)
    }
}

/// A route table: per flag index, the road distance to the goal and the first direction.
pub(crate) type Routes = std::rc::Rc<[(u32, Option<Direction>)]>;

/// A ware search's working memory: per flag the number of the search that last touched it, and
/// that search's distance and first road.
#[derive(Default)]
pub(crate) struct WareScratch {
    stamp: Vec<u32>,
    best: Vec<(u32, Option<Direction>)>,
    search: u32,
}
