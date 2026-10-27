//! Soldiers: garrisons, promotions, attacks and fights, capturing buildings.

use crate::figures::{FigKind, Motion, Step};
use crate::{AttackPlan, BState, BuildingId, Event, FigId, PlayerId, RejectReason, Sim, SoundCue};
use rr_core::{Direction, MapCoord, Purpose};
use rr_rules::{BuildingType, Job};
use std::collections::BTreeMap;

/// Frames per fight round, and the fights' clock (RTTR noFighting: 15 a strike; fidelity #10).
const ROUND_GF: u32 = 15;
const TICK: u32 = 5;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SoldierTask {
    /// Walking to a building to join its garrison.
    ToBuilding(BuildingId),
    /// Walking to attack `target`; returns to `home` afterwards.
    Attack { target: BuildingId, home: BuildingId },
    /// Waiting at the target's flag for a fight.
    AtTarget { target: BuildingId, home: BuildingId },
    /// Defending its building, standing at the flag.
    Defend(BuildingId),
    /// Going back to its building after an attack.
    Return(BuildingId),
    /// Stationed inside a building.
    Garrison(BuildingId),
    /// Sent out to meet an attacker on its way (RTTR nofAggressiveDefender).
    Intercept { attacker: FigId, home: BuildingId },
    /// An attack by sea (RTTR sea attacks): walking to board `ship`, at our harbor on the way
    /// out, at the landing on the way back; `target` to take, `home` to return to.
    ToShip { ship: crate::ShipId, target: BuildingId, home: BuildingId },
}

/// An ongoing duel.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Fights {
    /// target -> (attacker, defender, next strike frame, whose turn: 0 attacker / 1 defender)
    pub duels: BTreeMap<BuildingId, (FigId, FigId, u32, u8)>,
    /// Fights in the open: attacker -> (intercepting defender, next strike frame, turn).
    pub field: BTreeMap<FigId, (FigId, u32, u8)>,
    /// Attackers that already had a defender sent after them (RTTR mayBeHunted = false).
    pub hunted: std::collections::BTreeSet<FigId>,
}

impl Sim {
    /// The fights going on: (attacker, defender), at buildings and in the open (for drawing).
    pub fn fights(&self) -> Vec<(FigId, FigId)> {
        let duels = self.military.duels.values().map(|(a, d, _, _)| (*a, *d));
        duels.chain(self.military.field.iter().map(|(a, (d, _, _))| (*a, *d))).collect()
    }

    pub(crate) fn soldier_act(&mut self, f: FigId, task: SoldierTask) {
        match task {
            SoldierTask::ToBuilding(b) | SoldierTask::Return(b) => {
                let ok = self.buildings.get(b).is_some_and(|bd| {
                    bd.player == self.figures[f].player && bd.is_ready() && self.figures[f].pos == bd.pos
                });
                if !ok {
                    // (going home cancels the request, which counts the soldier off)
                    self.figure_lost_home(f);
                    return;
                }
                let bd = &mut self.buildings[b];
                if matches!(task, SoldierTask::ToBuilding(_)) {
                    bd.soldiers_coming = bd.soldiers_coming.saturating_sub(1);
                }
                if bd.inventory.is_some() {
                    self.store_in_warehouse(f, b);
                    return;
                }
                let first = !bd.occupied;
                if first {
                    bd.occupied_at = self.gf;
                }
                bd.occupied = true;
                bd.soldiers.push(f);
                self.figures[f].kind = FigKind::Soldier(SoldierTask::Garrison(b));
                self.figures[f].inside = Some(b);
                self.figures[f].motion = Motion::Idle;
                self.figures[f].step = Step::Inside;
                // coins waiting promote the newcomer too, after the usual delay
                self.arm_promotion(b);
                if first {
                    self.territory_dirty = true;
                    self.start_building_timer(b, 1);
                }
            }
            SoldierTask::Attack { target, home } => {
                // arrived next to the target's flag
                if !self.buildings.contains(target) {
                    self.soldier_return(f, home);
                    return;
                }
                self.figures[f].kind = FigKind::Soldier(SoldierTask::AtTarget { target, home });
                self.figures[f].motion = Motion::Idle;
            }
            SoldierTask::Intercept { attacker, home } => {
                let ok = self.figures.get(attacker).is_some_and(|a| {
                    self.world.grid().distance(a.pos, self.figures[f].pos) <= 1
                        && !self.military.field.contains_key(&attacker)
                });
                if ok {
                    let first = self.rng(2 << 60 | attacker.index() as u64 ^ 0x5eed, Purpose::Fight).below(2) as u8;
                    self.military.field.insert(attacker, (f, self.gf + ROUND_GF, first));
                    self.events.push(Event::Sound { at: self.figures[f].pos, sound: SoundCue::Sword });
                } else {
                    self.soldier_return(f, home);
                }
            }
            SoldierTask::AtTarget { .. } | SoldierTask::Defend(_) | SoldierTask::Garrison(_) => {
                self.figures[f].motion = Motion::Idle;
            }
            SoldierTask::ToShip { ship, home, .. } => {
                if !self.board(f, ship) {
                    self.soldier_return(f, home);
                }
            }
        }
    }

    pub(crate) fn soldier_return_pub(&mut self, f: FigId, home: BuildingId) {
        self.soldier_return(f, home);
    }

    fn soldier_return(&mut self, f: FigId, home: BuildingId) {
        let pos = self.figures[f].pos;
        // landed from the sea: back to the ship waiting at the landing
        if self.buildings.get(home).is_some_and(|h| self.other_land(pos, h.pos))
            && let Some((ship, landing)) = self.sea_attack_ship_of(f)
            && let Some(p) = self.find_path(pos, landing, 4000)
        {
            let target = match self.ships[ship].task {
                crate::ShipTask::SeaAttack { target, .. } => target,
                _ => home,
            };
            self.figures[f].kind = FigKind::Soldier(SoldierTask::ToShip { ship, target, home });
            self.figures[f].path = p.into_iter().rev().collect();
            self.figures[f].step = Step::Go;
            self.figure_continue(f);
            return;
        }
        if let Some(hb) = self.buildings.get(home)
            && hb.player == self.figures[f].player
            && let Some(p) = self.find_path(pos, hb.pos, 4000)
        {
            self.figures[f].kind = FigKind::Soldier(SoldierTask::Return(home));
            self.figures[f].path = p.into_iter().rev().collect();
            self.figures[f].step = Step::Go;
            self.figure_continue(f);
            return;
        }
        self.figure_lost_home(f);
    }

    pub(crate) fn figure_continue(&mut self, f: FigId) {
        // re-enter the event loop: one frame from now the figure takes its next step
        let key = self.schedule(1, crate::Ev::Figure(f));
        if let Some(old) = self.figures[f].ev.replace(key) {
            self.sched.cancel(old);
        }
    }

    pub(crate) fn soldier_leave_for_attack(&mut self, f: FigId) {
        // by sea: first to the harbor where the ship takes them on
        if let FigKind::Soldier(SoldierTask::ToShip { ship, home, .. }) = self.figures[f].kind.clone() {
            let harbor = match self.ships.get(ship).map(|s| &s.task) {
                Some(crate::ShipTask::SeaAttack { harbor, .. }) => self.buildings.get(*harbor).map(|h| h.pos),
                _ => None,
            };
            let start = self.figures[f].pos;
            let se = self.world.grid().neighbor(start, Direction::SE);
            match harbor.and_then(|h| self.find_path(se, h, 6000)) {
                Some(mut p) => {
                    p.insert(0, Direction::SE);
                    self.figures[f].inside = None;
                    self.figures[f].path = p.into_iter().rev().collect();
                    self.figures[f].step = Step::Go;
                    self.figure_continue(f);
                }
                None => self.soldier_back_inside(f, home),
            }
            return;
        }
        let FigKind::Soldier(SoldierTask::Attack { target, home }) = self.figures[f].kind.clone() else {
            return;
        };
        let Some(tb) = self.buildings.get(target) else {
            self.soldier_back_inside(f, home);
            return;
        };
        let goal = self.flags[tb.flag].pos;
        let start = self.figures[f].pos;
        let se = self.world.grid().neighbor(start, Direction::SE);
        match self.find_path(se, goal, 6000) {
            Some(mut p) => {
                p.insert(0, Direction::SE);
                self.figures[f].inside = None;
                self.figures[f].path = p.into_iter().rev().collect();
                self.figures[f].step = Step::Go;
                self.figure_continue(f);
            }
            None => self.soldier_back_inside(f, home),
        }
    }

    fn soldier_back_inside(&mut self, f: FigId, home: BuildingId) {
        if let Some(b) = self.buildings.get_mut(home) {
            b.soldiers.push(f);
            self.figures[f].kind = FigKind::Soldier(SoldierTask::Garrison(home));
            self.figures[f].inside = Some(home);
            self.figures[f].step = Step::Inside;
        } else {
            self.figure_lost_home(f);
        }
    }

    /// Can `player` attack building `t`? Military buildings someone has moved into, and
    /// warehouses, that the player can see (RTTR nobMilitary/nobBaseMilitary::IsAttackable).
    pub fn is_attackable(&self, player: PlayerId, t: BuildingId) -> bool {
        let tb = &self.buildings[t];
        // the castle and harbors, not a storehouse (RTTR nobStorehouse::IsAttackable is false;
        // veteran round 11, #1: 67 were razed in 15 games)
        let kind_ok = match tb.kind {
            rr_rules::BuildingType::Headquarters | rr_rules::BuildingType::Harbor => true,
            rr_rules::BuildingType::Storehouse => false,
            k => self.rules.building(k).is_military() && tb.occupied,
        };
        kind_ok
            && tb.is_ready()
            && !self.are_allies(tb.player, player)
            && self.visibility(player, tb.pos) == crate::Visibility::Visible
    }

    pub(crate) fn cmd_attack(
        &mut self,
        player: PlayerId,
        target: MapCoord,
        count: u8,
        strong: bool,
    ) -> Result<(), RejectReason> {
        let t = self.building_at(target).ok_or(RejectReason::NotFound)?;
        let tb = &self.buildings[t];
        if self.are_allies(tb.player, player) || !tb.is_ready() {
            return Err(RejectReason::NotAllowed);
        }
        if !self.is_attackable(player, t) {
            return Err(RejectReason::NotAllowed);
        }
        let sea = self.attack_sources(player, t).is_empty().then(|| self.sea_attack_plan(player, t)).flatten();
        let offer = match &sea {
            Some(plan) => self.offer_from(&plan.sources, strong),
            None => self.attack_offer(player, t, strong),
        };
        let mut sent = 0u8;
        for (b, s) in offer.into_iter().take(count as usize) {
            let Some(i) = self.buildings[b].soldiers.iter().position(|x| *x == s) else { continue };
            self.buildings[b].soldiers.remove(i);
            self.figures[s].kind = FigKind::Soldier(match &sea {
                Some(plan) => SoldierTask::ToShip { ship: plan.ship, target: t, home: b },
                None => SoldierTask::Attack { target: t, home: b },
            });
            self.buildings[b].fig_queue.push(s);
            self.schedule_leave(b);
            sent += 1;
        }
        if let Some(plan) = &sea
            && sent > 0
        {
            self.start_sea_attack(plan, t, sent);
        }
        if sent == 0 {
            return Err(RejectReason::NotFound);
        }
        let (tp, tpos) = (self.buildings[t].player, self.buildings[t].pos);
        self.events.push(Event::UnderAttack { player: tp, at: tpos, by: player, soldiers: sent });
        Ok(())
    }

    /// How far the nearest enemy military building is (RTTR `FrontierDistance`): 0 far, 1 middle,
    /// 3 near (2, by a harbor, only with sea attacks, which we don't have).
    pub fn frontier_distance(&self, b: BuildingId) -> usize {
        self.frontier_distance_among(b, &self.frontier_posts())
    }

    /// `frontier_distance` of many buildings, with the posts on the map gathered once.
    pub fn frontier_distances(&self, ids: &[BuildingId]) -> Vec<usize> {
        let posts = self.frontier_posts();
        ids.iter().map(|b| self.frontier_distance_among(*b, &posts)).collect()
    }

    /// The buildings that make a border a border: ready military buildings and warehouses
    /// (buildings, not sites: RTTR LookForMilitaryBuildings), as (player, pos, radius, troops;
    /// None for a warehouse, which sends no attackers).
    pub(crate) fn frontier_posts(&self) -> Vec<FrontierPost> {
        self.buildings
            .iter()
            .filter(|(_, o)| o.is_ready() && (self.rules.building(o.kind).is_military() || o.kind.is_warehouse()))
            .map(|(_, o)| {
                let them = self.rules.building(o.kind).military.as_ref();
                let troops = (!o.kind.is_warehouse()).then(|| them.map_or(1, |m| m.soldiers));
                (o.player, o.pos, them.map_or(0, |m| m.radius), troops, o.kind == rr_rules::BuildingType::Harbor)
            })
            .collect()
    }

    /// `frontier_distance` with the frontier posts worked out once for many buildings.
    pub(crate) fn frontier_distance_among(&self, b: BuildingId, posts: &[FrontierPost]) -> usize {
        let bd = &self.buildings[b];
        let g = self.world.grid();
        let me = self.rules.building(bd.kind).military.as_ref().map_or((0, 1), |m| (m.radius, m.soldiers));
        let mut class = 0;
        let mut by_harbor = false;
        for &(player, pos, radius, troops, harbor) in posts {
            if self.are_allies(player, bd.player) {
                // (one of our harbors near: posts that may sail to attack, RTTR's harbor class)
                by_harbor |= harbor && player == bd.player && g.distance(bd.pos, pos) <= crate::SEA_ATTACK_DISTANCE;
                continue;
            }
            // farther than any class reaches (the distance is at least the rows between): 0
            let reach = (me.0 + radius) as u32 + 21 + me.1.max(troops.unwrap_or(0)) as u32;
            let dy = (bd.pos.y as i32 - pos.y as i32).unsigned_abs();
            if dy.min(g.height as u32 - dy) > reach {
                continue;
            }
            // a rival nobody can walk to makes no border (RTTR's campaign "reachable" frontier
            // rule; veteran round 11, #5: posts facing the sea held soldiers needed elsewhere)
            let (la, lb) = (self.land[g.index(bd.pos)], self.land[g.index(pos)]);
            if la != 0 && lb != 0 && la != lb {
                continue;
            }
            class = class.max(frontier_class(g.distance(bd.pos, pos), me, (radius, troops)));
            if class == 3 {
                break;
            }
        }
        // by one of our harbors, with no rival near by land: the "near harbors" class (RTTR
        // FrontierDistance::Harbor, which comes with sea attacks). Narrower than RTTR (within 17
        // of any harbor spot): full garrisons by every coast drained the fronts (fidelity #4)
        if class <= 1 && by_harbor { 2 } else { class }
    }

    /// Soldiers a military building wants given the player's settings: `(max - 1) * s / 8 + 1`.
    pub fn required_troops(&self, b: BuildingId) -> usize {
        let bd = &self.buildings[b];
        let Some(m) = self.rules.building(bd.kind).military.as_ref() else { return 0 };
        if bd.kind.is_warehouse() {
            return 0;
        }
        self.troops_for(b, m.soldiers, self.frontier_distance(b))
    }

    /// With coins in stock and someone to promote, the next promotion follows 100 + rand(300) GF
    /// from now unless one is pending (RTTR nobMilitary::PrepareUpgrading).
    pub(crate) fn arm_promotion(&mut self, b: BuildingId) {
        let bd = &self.buildings[b];
        let promotable = bd.soldiers.iter().any(|s| self.figures[*s].job.soldier_rank().is_some_and(|r| r < 4));
        if bd.stock[0] == 0 || bd.promote_at.is_some() || !promotable {
            return;
        }
        let mut rng = self.rng_building(b, Purpose::Military);
        let wait = self.rules.military.promote_wait + rng.below(self.rules.military.promote_rand + 1);
        self.buildings[b].promote_at = Some(self.gf + wait);
        self.start_building_timer(b, wait);
    }

    /// `required_troops` with the frontier posts worked out once for many buildings.
    pub(crate) fn required_troops_among(&self, b: BuildingId, posts: &[FrontierPost]) -> usize {
        let bd = &self.buildings[b];
        let Some(m) = self.rules.building(bd.kind).military.as_ref() else { return 0 };
        if bd.kind.is_warehouse() {
            return 0;
        }
        self.troops_for(b, m.soldiers, self.frontier_distance_among(b, posts))
    }

    fn troops_for(&self, b: BuildingId, max: u8, frontier: usize) -> usize {
        let s = self.players[self.buildings[b].player as usize].settings.military[4 + frontier] as usize;
        (max as usize - 1) * s / crate::MILITARY_SCALE[4 + frontier] as usize + 1
    }

    /// Soldiers a building may send to attack a target `dist` away (RTTR `GetNumSoldiersForAttack`).
    pub fn attackers_available(&self, b: BuildingId, dist: u32) -> usize {
        let bd = &self.buildings[b];
        let n = bd.soldiers.len();
        if n <= 1 {
            return 0;
        }
        let s = self.players[bd.player as usize].settings.military[3] as usize;
        let mut count = (n - 1) * s / crate::MILITARY_SCALE[3] as usize;
        let base = self.rules.military.attack_radius as u32;
        if dist > base {
            count = count.saturating_sub((dist - base) as usize);
        }
        count
    }

    /// How many soldiers `player` could send against the building at `target`.
    /// How many of `player`'s military buildings could send attackers against the building at
    /// `target` if they had soldiers to spare (for explaining why nobody can attack).
    pub fn attack_posts_near(&self, player: PlayerId, target: MapCoord) -> usize {
        let Some(t) = self.building_at(target) else { return 0 };
        self.any_sources(player, t).len()
    }

    pub fn attackers_for(&self, player: PlayerId, target: MapCoord) -> usize {
        let Some(t) = self.building_at(target) else { return 0 };
        self.any_sources(player, t).into_iter().map(|(d, b)| self.attackers_available(b, d)).sum()
    }

    /// Would an attack of `player` on the building at `target` go by sea (nobody can walk there)?
    pub fn attack_goes_by_sea(&self, player: PlayerId, target: MapCoord) -> bool {
        self.building_at(target)
            .is_some_and(|t| self.attack_sources(player, t).is_empty() && self.sea_attack_plan(player, t).is_some())
    }

    /// The posts that may attack `t`: on foot, or else by sea (RTTR sea attacks).
    fn any_sources(&self, player: PlayerId, t: BuildingId) -> Vec<(u32, BuildingId)> {
        let land = self.attack_sources(player, t);
        if !land.is_empty() {
            return land;
        }
        self.sea_attack_plan(player, t).map(|p| p.sources).unwrap_or_default()
    }

    /// The soldiers an attack on `t` takes, in the order it takes them, with their posts (RTTR
    /// GameWorld::Attack: every building offers the soldiers it may send, strongest or weakest
    /// first; all of them are then taken by rank, then by distance).
    fn attack_offer(&self, player: PlayerId, t: BuildingId, strong: bool) -> Vec<(BuildingId, FigId)> {
        self.offer_from(&self.any_sources(player, t), strong)
    }

    /// The soldiers the posts `sources` offer, in the order an attack takes them.
    fn offer_from(&self, sources: &[(u32, BuildingId)], strong: bool) -> Vec<(BuildingId, FigId)> {
        let rank = |s: FigId| self.figures[s].job.soldier_rank().map_or(0, |r| r as i32);
        let key = |s: FigId| if strong { -rank(s) } else { rank(s) };
        let mut offered: Vec<(i32, u32, BuildingId, FigId)> = Vec::new();
        for &(dist, b) in sources {
            let mut own = self.buildings[b].soldiers.clone();
            own.sort_by_key(|s| key(*s));
            let n = self.attackers_available(b, dist);
            offered.extend(own.into_iter().take(n).map(|s| (key(s), dist, b, s)));
        }
        offered.sort_by_key(|(k, d, _, _)| (*k, *d));
        offered.into_iter().map(|(_, _, b, s)| (b, s)).collect()
    }

    /// What an attack window shows of an attack on the building at `target`, from one search
    /// of the posts in reach (perf round 3, #5: three searches per game frame): the posts, how
    /// many may go, and the ranks they would send, strongest and weakest first.
    pub fn attack_plan(&self, player: PlayerId, target: MapCoord) -> AttackPlan {
        let Some(t) = self.building_at(target) else { return AttackPlan::default() };
        let land = self.attack_sources(player, t);
        let by_sea = land.is_empty();
        let sources =
            if by_sea { self.sea_attack_plan(player, t).map(|p| p.sources).unwrap_or_default() } else { land };
        let ranks = |strong| -> Vec<u8> {
            self.offer_from(&sources, strong)
                .into_iter()
                .map(|(_, s)| self.figures[s].job.soldier_rank().unwrap_or(0))
                .collect()
        };
        AttackPlan {
            posts: sources.len(),
            available: sources.iter().map(|(d, b)| self.attackers_available(*b, *d)).sum(),
            strong: ranks(true),
            weak: ranks(false),
            by_sea: by_sea && !sources.is_empty(),
        }
    }

    /// The ranks (0 private .. 4 general) of the soldiers an attack of `count` on the building at
    /// `target` would send, without sending them: the attack window shows the odds (genre
    /// lessons, PLAN §27, #2).
    pub fn attack_party(&self, player: PlayerId, target: MapCoord, count: u8, strong: bool) -> Vec<u8> {
        let Some(t) = self.building_at(target) else { return Vec::new() };
        self.attack_offer(player, t, strong)
            .into_iter()
            .take(count as usize)
            .map(|(_, s)| self.figures[s].job.soldier_rank().unwrap_or(0))
            .collect()
    }

    /// The hit points of the soldiers `player` could send against the building at `target`,
    /// strongest first: only from the posts that can walk there (RTTR
    /// GetSoldiersStrengthForAttack).
    pub fn attack_strength(&self, player: PlayerId, target: MapCoord) -> Vec<u8> {
        let Some(t) = self.building_at(target) else { return Vec::new() };
        let mut out: Vec<u8> = Vec::new();
        for (d, b) in self.any_sources(player, t) {
            let mut hp: Vec<u8> = self.buildings[b].soldiers.iter().map(|f| self.figures[*f].hp).collect();
            hp.sort_unstable_by(|a, b| b.cmp(a));
            out.extend(hp.into_iter().take(self.attackers_available(b, d)));
        }
        out.sort_unstable_by(|a, b| b.cmp(a));
        out
    }

    /// Military buildings of `player` that may send soldiers against `target`, nearest first:
    /// within reach (beyond the base distance every extra node costs one soldier, RTTR
    /// GetNumSoldiersForAttack), and on the same land within a 40-step walk.
    fn attack_sources(&self, player: PlayerId, target: BuildingId) -> Vec<(u32, BuildingId)> {
        let g = self.world.grid();
        let radius = self.rules.military.attack_radius as u32 + 10;
        let tpos = self.buildings[target].pos;
        let goal = self.flags[self.buildings[target].flag].pos;
        let mut sources: Vec<(u32, BuildingId)> = self
            .buildings
            .iter()
            .filter(|(_, b)| b.player == player && b.is_ready() && crate::is_frontier_military(b.kind))
            .map(|(id, b)| (g.distance(b.pos, tpos), id))
            .filter(|(d, _)| *d <= radius)
            .filter(|(_, id)| {
                let from = self.flags[self.buildings[*id].flag].pos;
                // RTTR MAX_ATTACKING_RUN_DISTANCE: a walk of at most 40 steps (the rules'
                // attack_max_walk, read by nothing before; map maker round 1, #1)
                self.same_land(from, goal)
                    && self.path_len_within(from, goal, self.rules.military.attack_max_walk as u32).is_some()
            })
            .collect();
        sources.sort();
        sources
    }

    /// Military buildings let figures out one by one, like warehouses.
    fn schedule_leave(&mut self, b: BuildingId) {
        self.start_building_timer(b, 1);
    }

    pub(crate) fn military_building_tick(&mut self, b: BuildingId) {
        // release soldiers queued to leave
        if !self.buildings[b].fig_queue.is_empty() {
            let f = self.buildings[b].fig_queue.remove(0);
            self.figure_leave_building(f);
            // one by one, 30 + rand(10) apart (RTTR nobMilitary; fidelity #10)
            let mut rng = self.rng_building(b, Purpose::Military);
            let next = 30 + rng.below(11);
            self.start_building_timer(b, next);
            return;
        }
        // promotions: one coin promotes one soldier of each rank, once its delay is over
        let soldiers = self.buildings[b].soldiers.clone();
        let promote_due = self.buildings[b].promote_at.is_none_or(|at| at <= self.gf);
        if self.buildings[b].stock[0] > 0 && promote_due {
            self.buildings[b].promote_at = None;
            let mut promoted = false;
            for rank in (0..4u8).rev() {
                if let Some(s) = soldiers.iter().find(|s| self.figures[**s].job.soldier_rank() == Some(rank)) {
                    let next = Job::SOLDIERS[rank as usize + 1];
                    self.figures[*s].job = next;
                    self.figures[*s].hp = self.rules.military.hp[rank as usize + 1];
                    promoted = true;
                }
            }
            if promoted {
                self.buildings[b].stock[0] -= 1;
                self.events.push(Event::Sound { at: self.buildings[b].pos, sound: SoundCue::Promote });
                // the next one follows its own short delay, not the healing clock (fidelity #1)
                self.arm_promotion(b);
            }
        }
        // healing: every wounded soldier gets one hit point every 500 + rand(500) frames (RTTR
        // nofPassiveSoldier: each on his own timer; one shared tick here, same pace)
        let max_hp =
            |sim: &Self, s: FigId| sim.figures[s].job.soldier_rank().map_or(0, |r| sim.rules.military.hp[r as usize]);
        if self.gf >= self.buildings[b].next_heal {
            for s in soldiers.iter().copied() {
                if self.figures[s].hp < max_hp(self, s) {
                    self.figures[s].hp += 1;
                }
            }
            let mut rng = self.rng_building(b, Purpose::Military);
            let wait = self.rules.military.heal_every + rng.below(self.rules.military.heal_rand + 1);
            self.buildings[b].next_heal = self.gf + wait;
        }
        let bd = &self.buildings[b];
        let due = match bd.promote_at {
            Some(p) if bd.stock[0] > 0 => p.min(bd.next_heal),
            _ => bd.next_heal,
        };
        let next = due.saturating_sub(self.gf).max(1);
        self.start_building_timer(b, next);
        // too many soldiers? send some home: the weak ones when the defense setting is above half,
        // else the strong ones (RTTR nobMilitary::RegulateTroops)
        let wanted = (self.buildings[b].garrison_wanted as usize).min(self.required_troops(b));
        let weak_out =
            self.players[self.buildings[b].player as usize].settings.military[1] > crate::MILITARY_SCALE[1] / 2;
        while self.buildings[b].soldiers.len() > wanted.max(1) {
            let soldiers = &self.buildings[b].soldiers;
            let key = |i: &usize| {
                let r = self.figures[soldiers[*i]].job.soldier_rank().map_or(0, |r| r as i32);
                (if weak_out { r } else { -r }, std::cmp::Reverse(*i))
            };
            let i = (0..soldiers.len()).min_by_key(key).expect("soldiers");
            let s = self.buildings[b].soldiers.remove(i);
            self.figures[s].inside = None;
            let bp = self.buildings[b].pos;
            self.figures[s].pos = bp;
            self.figure_lost_home(s);
        }
    }

    pub(crate) fn cmd_send_home(&mut self, player: PlayerId, at: MapCoord) -> Result<(), RejectReason> {
        let b = self.building_at(at).ok_or(RejectReason::NotFound)?;
        let bd = &self.buildings[b];
        if bd.player != player || !bd.is_ready() || bd.kind.is_warehouse() || bd.soldiers.len() < 2 {
            return Err(RejectReason::NotAllowed);
        }
        let rank = |s: FigId| self.figures[s].job.soldier_rank().map_or(0, |r| r as i32);
        let i =
            (0..bd.soldiers.len()).max_by_key(|i| (rank(bd.soldiers[*i]), std::cmp::Reverse(*i))).expect("soldiers");
        let s = self.buildings[b].soldiers.remove(i);
        // the smaller garrison is what it wants now: no one comes in its place
        self.buildings[b].garrison_wanted = self.buildings[b].soldiers.len() as u8;
        self.figures[s].inside = None;
        self.figures[s].pos = at;
        self.figure_lost_home(s);
        Ok(())
    }

    pub(crate) fn cmd_garrison_wanted(
        &mut self,
        player: PlayerId,
        at: MapCoord,
        wanted: u8,
    ) -> Result<(), RejectReason> {
        let b = self.building_at(at).ok_or(RejectReason::NotFound)?;
        let bd = &self.buildings[b];
        if bd.player != player {
            return Err(RejectReason::NotAllowed);
        }
        let cap = self.rules.building(bd.kind).military.as_ref().map_or(0, |m| m.soldiers);
        self.buildings[b].garrison_wanted = wanted.clamp(1, cap.max(1));
        self.start_building_timer(b, 1);
        Ok(())
    }

    /// Fights at attacked buildings; runs every few frames.
    pub(crate) fn military_tick(&mut self) {
        if !self.gf.is_multiple_of(TICK) {
            return;
        }
        self.field_tick();
        // forget attackers that are gone (they could be re-used ids otherwise)
        let figures = &self.figures;
        self.military.hunted.retain(|f| figures.contains(*f));
        // attackers waiting at targets
        let mut waiting: BTreeMap<BuildingId, Vec<FigId>> = BTreeMap::new();
        for (id, f) in self.figures.iter() {
            if let FigKind::Soldier(SoldierTask::AtTarget { target, .. }) = f.kind
                && f.path.is_empty()
                && matches!(f.motion, Motion::Idle)
            {
                waiting.entry(target).or_default().push(id);
            }
        }
        // finished duels
        let duels: Vec<_> = self.military.duels.iter().map(|(k, v)| (*k, *v)).collect();
        for (target, (att, def, next, turn)) in duels {
            if !self.figures.contains(att) || !self.figures.contains(def) || !self.buildings.contains(target) {
                self.military.duels.remove(&target);
                // both stop fighting (idle, so that going home wakes them)
                for f in [att, def] {
                    if let Some(x) = self.figures.get_mut(f) {
                        x.motion = Motion::Idle;
                    }
                }
                if let Some(fd) = self.figures.get(def)
                    && fd.kind == FigKind::Soldier(SoldierTask::Defend(target))
                {
                    self.defender_back_in(def, target);
                }
                if let Some(FigKind::Soldier(SoldierTask::AtTarget { home, .. })) =
                    self.figures.get(att).map(|f| f.kind.clone())
                    && !self.buildings.contains(target)
                {
                    self.soldier_return(att, home);
                }
                continue;
            }
            if self.gf < next {
                continue;
            }
            self.fight_round(target, att, def, turn);
        }
        for (target, attackers) in waiting {
            // earlier captures in this pass may have removed figures
            let attackers: Vec<FigId> = attackers.into_iter().filter(|a| self.figures.contains(*a)).collect();
            if attackers.is_empty() || self.military.duels.contains_key(&target) {
                continue;
            }
            let Some(tb) = self.buildings.get(target) else {
                for a in attackers {
                    if let Some(FigKind::Soldier(SoldierTask::AtTarget { home, .. })) =
                        self.figures.get(a).map(|f| f.kind.clone())
                    {
                        self.soldier_return(a, home);
                    }
                }
                continue;
            };
            let attacker = attackers[0];
            if self.are_allies(tb.player, self.figures[attacker].player) {
                // already ours: move in while there is room (RTTR), the rest go home
                let cap = self.rules.building(tb.kind).military.as_ref().map_or(0, |m| m.soldiers) as usize;
                let ours = tb.player == self.figures[attacker].player;
                for a in attackers {
                    let Some(FigKind::Soldier(SoldierTask::AtTarget { home, .. })) =
                        self.figures.get(a).map(|f| f.kind.clone())
                    else {
                        continue;
                    };
                    if ours && self.buildings[target].soldiers.len() < cap {
                        self.soldier_move_in(a, target);
                    } else {
                        self.soldier_return(a, home);
                    }
                }
                continue;
            }
            match self.take_defender(target) {
                Some(def) => {
                    // who strikes first is drawn by lot
                    let first = self.rng_figure(attacker, Purpose::Fight).below(2) as u8;
                    self.military.duels.insert(target, (attacker, def, self.gf + ROUND_GF, first));
                    self.events.push(Event::Sound { at: self.figures[attacker].pos, sound: SoundCue::Sword });
                }
                None => self.capture(target, attacker),
            }
        }
    }

    /// A defender steps out of the building (or out of the warehouse's soldier stock).
    /// The soldier who steps out to defend or intercept (RTTR nobMilitary::ChooseSoldier): of
    /// the ranks inside, the one at `(ranks - 1) * setting / 5` (defense setting), weakest first.
    fn choose_soldier(&self, b: BuildingId) -> Option<usize> {
        let soldiers = &self.buildings[b].soldiers;
        let rank = |i: usize| self.figures[soldiers[i]].job.soldier_rank().unwrap_or(0);
        // the first soldier of each rank, weakest rank first
        let mut firsts: Vec<usize> = Vec::new();
        for i in 0..soldiers.len() {
            if !firsts.iter().any(|j| rank(*j) == rank(i)) {
                firsts.push(i);
            }
        }
        firsts.sort_by_key(|i| rank(*i));
        let setting = self.players[self.buildings[b].player as usize].settings.military[1] as usize;
        let k = (firsts.len().checked_sub(1)?) * setting / crate::MILITARY_SCALE[1] as usize;
        firsts.get(k).copied()
    }

    /// Who would step out to defend building `b` now (for tests and the interface).
    pub fn defender_choice(&self, b: BuildingId) -> Option<FigId> {
        self.choose_soldier(b).map(|i| self.buildings[b].soldiers[i])
    }

    /// The rank a warehouse sends out to defend: by the defense setting among the ranks it holds,
    /// as in `choose_soldier`.
    pub fn warehouse_defender(&self, b: BuildingId) -> Option<Job> {
        let bd = &self.buildings[b];
        let inv = bd.inventory.as_ref()?;
        let ranks: Vec<Job> = Job::SOLDIERS.iter().copied().filter(|j| inv.job(*j) > 0).collect();
        let setting = self.players[bd.player as usize].settings.military[1] as usize;
        let k = (ranks.len().checked_sub(1)?) * setting / crate::MILITARY_SCALE[1] as usize;
        ranks.get(k).copied()
    }

    pub(crate) fn take_defender(&mut self, target: BuildingId) -> Option<FigId> {
        let bd = &self.buildings[target];
        let (player, pos) = (bd.player, bd.pos);
        let flag_pos = self.flags[bd.flag].pos;
        let f = if bd.inventory.is_some() {
            let job = self.warehouse_defender(target)?;
            self.buildings[target].inventory.as_mut().expect("inv").jobs[job.index()] -= 1;
            self.new_figure(player, job, pos, FigKind::Soldier(SoldierTask::Defend(target)))
        } else {
            if bd.soldiers.is_empty() {
                return None;
            }
            let i = self.choose_soldier(target)?;
            let s = self.buildings[target].soldiers.remove(i);
            self.figures[s].kind = FigKind::Soldier(SoldierTask::Defend(target));
            s
        };
        self.figures[f].inside = None;
        self.figures[f].pos = flag_pos;
        self.figures[f].motion = Motion::Idle;
        Some(f)
    }

    fn defender_back_in(&mut self, def: FigId, target: BuildingId) {
        let Some(bd) = self.buildings.get_mut(target) else {
            self.figure_lost_home(def);
            return;
        };
        if let Some(inv) = bd.inventory.as_mut() {
            let job = self.figures[def].job;
            inv.jobs[job.index()] += 1;
            self.figures.remove(def);
            return;
        }
        bd.soldiers.push(def);
        let pos = bd.pos;
        self.figures[def].inside = Some(target);
        self.figures[def].pos = pos;
        self.figures[def].kind = FigKind::Soldier(SoldierTask::Garrison(target));
    }

    /// One strike between two soldiers; returns Some(true) if `b` died, Some(false) if `a` died.
    fn strike(&mut self, a_id: FigId, b_id: FigId, turn: u8) -> Option<bool> {
        let k = self.rules.military.fight_k;
        let mut rng = self.rng(4 << 60 | a_id.index() as u64 ^ ((b_id.index() as u64) << 20), Purpose::Fight);
        let ra = self.figures[a_id].job.soldier_rank().unwrap_or(0) as u32;
        let rb = self.figures[b_id].job.soldier_rank().unwrap_or(0) as u32;
        let a = rng.below(ra + k);
        let b = rng.below(rb + k);
        self.figures[a_id].motion = Motion::Work { start: self.gf, dur: ROUND_GF };
        self.figures[b_id].motion = Motion::Work { start: self.gf, dur: ROUND_GF };
        // every strike clangs, hit or parried (one clang per 5-6 s duel, sound designer 2 #3)
        self.events.push(Event::Sound { at: self.figures[a_id].pos, sound: SoundCue::Sword });
        if turn == 0 && a > b {
            self.figures[b_id].hp = self.figures[b_id].hp.saturating_sub(1);
        } else if turn == 1 && b > a {
            self.figures[a_id].hp = self.figures[a_id].hp.saturating_sub(1);
        }
        if self.figures[b_id].hp == 0 {
            Some(true)
        } else if self.figures[a_id].hp == 0 {
            Some(false)
        } else {
            None
        }
    }

    /// Fights in the open between attackers and intercepting defenders.
    fn field_tick(&mut self) {
        let field: Vec<_> = self.military.field.iter().map(|(k, v)| (*k, *v)).collect();
        for (att, (def, next, turn)) in field {
            if !self.figures.contains(att) || !self.figures.contains(def) {
                self.military.field.remove(&att);
                if self.figures.contains(att) {
                    self.figures[att].motion = Motion::Idle;
                    self.figure_continue(att);
                }
                if let Some(FigKind::Soldier(SoldierTask::Intercept { home, .. })) =
                    self.figures.get(def).map(|f| f.kind.clone())
                {
                    self.figures[def].motion = Motion::Idle;
                    self.soldier_return(def, home);
                }
                continue;
            }
            if self.gf < next {
                continue;
            }
            match self.strike(att, def, turn) {
                Some(true) => {
                    self.military.field.remove(&att);
                    self.kill_figure(def);
                    self.figures[att].motion = Motion::Idle;
                    // the attacker walks on to its target
                    self.figure_continue(att);
                }
                Some(false) => {
                    self.military.field.remove(&att);
                    self.kill_figure(att);
                    self.figures[def].motion = Motion::Idle;
                    if let FigKind::Soldier(SoldierTask::Intercept { home, .. }) = self.figures[def].kind.clone() {
                        self.soldier_return(def, home);
                    }
                }
                None => {
                    self.military.field.insert(att, (def, self.gf + ROUND_GF, 1 - turn));
                }
            }
        }
    }

    /// An attacker stepped onto a node: maybe a nearby enemy building sends a defender
    /// (RTTR nofAttacker::TryToOrderAggressiveDefender: 20 % per step, buildings within 14,
    /// each player decides per attacker with chance slider3 / 5). Returns true if the attacker
    /// must stop and wait for the fight.
    pub(crate) fn try_hunt(&mut self, f: FigId, target: BuildingId) -> bool {
        if self.military.hunted.contains(&f) {
            return false;
        }
        let pos = self.figures[f].pos;
        let attacker_player = self.figures[f].player;
        let mut rng = self.rng(5 << 60 | f.index() as u64 ^ ((pos.x as u64) << 16 | pos.y as u64), Purpose::Military);
        if rng.below(10) >= 2 {
            return false;
        }
        let Some(tb) = self.buildings.get(target) else { return false };
        let victim = tb.player;
        let g = self.world.grid();
        let mut cands: Vec<(u32, BuildingId)> = self
            .buildings
            .iter()
            .filter(|(id, b)| {
                self.are_allies(b.player, victim)
                    && !self.are_allies(b.player, attacker_player)
                    && b.is_ready()
                    && self.rules.building(b.kind).is_military()
                    && (b.kind != BuildingType::Headquarters || *id == target)
                    && b.soldiers.len() > 1
            })
            .map(|(id, b)| (g.distance(b.pos, pos), id))
            .filter(|(d, _)| *d <= 14)
            .collect();
        cands.sort();
        for (_, b) in cands {
            let owner = self.buildings[b].player;
            let s = self.players[owner as usize].settings.military[2] as u32;
            if rng.below(crate::MILITARY_SCALE[2] as u32) >= s {
                continue;
            }
            let bpos = self.buildings[b].pos;
            let start = g.neighbor(bpos, Direction::SE);
            let Some(mut path) = self.find_path(start, pos, 2000).filter(|p| p.len() <= 30) else { continue };
            path.insert(0, Direction::SE);
            let pick = self.choose_soldier(b).expect("more than one soldier");
            let d = self.buildings[b].soldiers.remove(pick);
            self.figures[d].kind = FigKind::Soldier(SoldierTask::Intercept { attacker: f, home: b });
            self.figures[d].inside = None;
            self.figures[d].pos = bpos;
            self.figures[d].path = path.into_iter().rev().collect();
            self.figures[d].step = Step::Go;
            self.figure_continue(d);
            self.military.hunted.insert(f);
            // the attacker waits where it stands
            self.figures[f].motion = Motion::Idle;
            return true;
        }
        false
    }

    /// One strike (RTTR noFighting::StartAttack): both roll `rand(rank + K)`; the soldier whose
    /// turn it is hits only with a strictly higher roll. Turns alternate.
    /// One round of a duel at `target`'s flag.
    fn fight_round(&mut self, target: BuildingId, att: FigId, def: FigId, turn: u8) {
        let outcome = self.strike(att, def, turn);
        if outcome == Some(true) {
            self.military.duels.remove(&target);
            // (kill_figure screams; a second one here doubled 15-20 % of deaths, sound designer 2 #3)
            self.kill_figure(def);
            self.figures[att].motion = Motion::Idle;
        } else if outcome == Some(false) {
            self.military.duels.remove(&target);
            self.kill_figure(att);
            self.figures[def].motion = Motion::Idle;
            self.defender_back_in(def, target);
        } else {
            self.military.duels.insert(target, (att, def, self.gf + ROUND_GF, 1 - turn));
        }
    }

    /// No defenders left: the attacker takes the building (warehouses are destroyed).
    fn capture(&mut self, target: BuildingId, attacker: FigId) {
        let tb = self.buildings[target].clone();
        let new_owner = self.figures[attacker].player;
        self.players[new_owner as usize].stats.vanquished += 1;
        if tb.kind.is_warehouse() {
            self.events.push(Event::Razed { player: tb.player, at: tb.pos, kind: tb.kind, by: new_owner });
            self.destroy_building(target, true);
            if let FigKind::Soldier(SoldierTask::AtTarget { home, .. }) = self.figures[attacker].kind {
                self.soldier_return(attacker, home);
            }
            return;
        }
        let old = tb.player;
        // the flag changes hands; its roads are torn down
        let flag = tb.flag;
        let roads: Vec<_> = self.flags[flag].roads.iter().flatten().copied().collect();
        for r in roads {
            self.destroy_road(r);
        }
        for w in self.flags[flag].wares.clone() {
            self.lose_ware(w);
        }
        self.flags[flag].player = new_owner;
        self.release_wares_heading_to(target, None);
        // the old owner's people walking there turn back
        let coming: Vec<FigId> = self
            .figures
            .iter()
            .filter(|(_, f)| f.player == old && f.target_building() == Some(target))
            .map(|(i, _)| i)
            .collect();
        for f in coming {
            self.figure_lost_home(f);
        }
        // the old owner's people queued to leave go home instead
        for f in std::mem::take(&mut self.buildings[target].fig_queue) {
            self.figure_lost_home(f);
        }
        // RTTR nobMilitary::Capture: the coins go to the new owner with the building, the coin
        // setting stays, and the troop limit is the building's most (veteran round 11, #6: 131
        // coins were lost in 53 captures, and the old owner's limit kept)
        let most = self.rules.building(self.buildings[target].kind).military.as_ref().map_or(1, |m| m.soldiers);
        let bd = &mut self.buildings[target];
        bd.player = new_owner;
        bd.stock = [bd.stock[0], 0, 0];
        bd.incoming = [0; 3];
        bd.soldiers_coming = 0;
        bd.garrison_wanted = most;
        bd.state = BState::Ready;
        let pos = bd.pos;
        self.soldier_move_in(attacker, target);
        self.events.push(Event::BuildingCaptured { player: old, at: pos, by: new_owner });
        self.territory_dirty = true;
        self.players[old as usize].road_epoch += 1;
        self.route_cache.borrow_mut().clear();
        self.players[new_owner as usize].road_epoch += 1;
        self.route_cache.borrow_mut().clear();
        self.check_defeat(old);
    }

    /// Soldiers of `player` away from their building (attacking, waiting, defending at the flag,
    /// intercepting, coming back), by building.
    pub(crate) fn soldiers_on_mission(&self, player: PlayerId) -> BTreeMap<BuildingId, usize> {
        let mut out = BTreeMap::new();
        for (_, f) in self.figures.iter().filter(|(_, f)| f.player == player) {
            let home = match &f.kind {
                FigKind::Soldier(
                    SoldierTask::Attack { home, .. }
                    | SoldierTask::AtTarget { home, .. }
                    | SoldierTask::Intercept { home, .. }
                    | SoldierTask::ToShip { home, .. }
                    | SoldierTask::Defend(home)
                    | SoldierTask::Return(home),
                ) => *home,
                _ => continue,
            };
            *out.entry(home).or_default() += 1;
        }
        out
    }

    /// An attacker becomes part of the garrison of the building he took.
    fn soldier_move_in(&mut self, s: FigId, target: BuildingId) {
        let pos = self.buildings[target].pos;
        self.buildings[target].occupied = true;
        self.buildings[target].occupied_at = self.gf;
        self.buildings[target].soldiers.push(s);
        let f = &mut self.figures[s];
        f.inside = Some(target);
        f.pos = pos;
        f.motion = Motion::Idle;
        f.kind = FigKind::Soldier(SoldierTask::Garrison(target));
    }

    pub(crate) fn check_defeat(&mut self, player: PlayerId) {
        let alive = self.buildings.iter().any(|(_, b)| {
            b.player == player && b.is_ready() && (b.inventory.is_some() || self.rules.building(b.kind).is_military())
        });
        if !alive && self.players[player as usize].alive {
            self.defeat(player);
        }
    }

    pub(crate) fn defeat(&mut self, player: PlayerId) {
        self.players[player as usize].alive = false;
        self.events.push(Event::PlayerDefeated { player });
    }

    pub(crate) fn catapult_try(&mut self, f: FigId, b: BuildingId) {
        // the catapult throws a stone at an enemy military building in range
        if self.buildings[b].stock[0] == 0 {
            self.figures[f].motion = Motion::Idle;
            let key = self.schedule(200, crate::Ev::Figure(f));
            self.figures[f].ev = Some(key);
            return;
        }
        let pos = self.buildings[b].pos;
        let targets = self.catapult_targets(b);
        let mut rng = self.rng_figure(f, Purpose::Military);
        if targets.is_empty() {
            // nothing in range: look again later (RTTR CATAPULT_WAIT1_LENGTH)
            self.figures[f].motion = Motion::Idle;
            let key = self.schedule(1300, crate::Ev::Figure(f));
            self.figures[f].ev = Some(key);
            return;
        }
        let t = targets[rng.below(targets.len() as u32) as usize];
        self.buildings[b].stock[0] -= 1;
        // heard where it throws and where the stone lands (sound designer 2 #6)
        self.events.push(Event::Sound { at: self.buildings[b].pos, sound: SoundCue::Catapult });
        self.events.push(Event::Sound { at: self.buildings[t].pos, sound: SoundCue::Thump });
        // 70% hit; the weakest soldier inside dies (RTTR HitOfCatapultStone)
        let hit = rng.chance(70);
        self.events.push(Event::CatapultThrow { from: pos, to: self.buildings[t].pos, hit });
        if hit {
            let weakest = {
                let soldiers = &self.buildings[t].soldiers;
                (0..soldiers.len()).min_by_key(|i| (self.figures[soldiers[*i]].job.soldier_rank(), *i))
            };
            if let Some(i) = weakest {
                let s = self.buildings[t].soldiers.remove(i);
                self.kill_figure(s);
            }
            let (tp, tpos) = (self.buildings[t].player, self.buildings[t].pos);
            let by = self.buildings[b].player;
            self.events.push(Event::CatapultHit { player: tp, at: tpos, by });
            // the last soldier gone: the building burns (RTTR HitOfCatapultStone; fidelity #7)
            if self.buildings[t].soldiers.is_empty() {
                self.destroy_building(t, true);
                self.territory_dirty = true;
            }
        }
        // the next look after RTTR's wait (CATAPULT_WAIT1_LENGTH 1 300: "raised because ware
        // distribution is too fast"), plus turning to aim and back: 15 GF per step of the w
        // steps to the target's direction, +1 and +3 (fidelity #2: it threw every 400 GF)
        let tpos = self.buildings.get(t).map_or(pos, |x| x.pos);
        let aim = catapult_aim(self.world.grid(), pos, tpos);
        self.figures[f].motion = Motion::Idle;
        let key = self.schedule(1300 + aim, crate::Ev::Figure(f));
        self.figures[f].ev = Some(key);
    }
}

/// How far a catapult throws: nodes closer than 14 (RTTR nofCatapultMan).
pub const CATAPULT_RANGE: u16 = 13;

impl Sim {
    /// What the catapult `b` may throw at now (RTTR nofCatapultMan): occupied military buildings
    /// (not warehouses) of a non-ally, that its owner sees, within `CATAPULT_RANGE`.
    pub fn catapult_targets(&self, b: BuildingId) -> Vec<BuildingId> {
        let Some(c) = self.buildings.get(b) else { return Vec::new() };
        let (pos, player, g) = (c.pos, c.player, self.world.grid());
        self.buildings
            .iter()
            .filter(|(_, t)| {
                !self.are_allies(t.player, player)
                    && crate::is_frontier_military(t.kind)
                    && t.occupied
                    && self.visibility(player, t.pos) == crate::Visibility::Visible
                    && g.distance(pos, t.pos) <= CATAPULT_RANGE as u32
            })
            .map(|(id, _)| id)
            .collect()
    }
}

/// A catapult's time to turn to `to` and back: 15 GF per sixth of a turn away from east, +1 and
/// +3 (RTTR nofCatapultMan). The offsets wrap with the map (QA round 6, #6: a target just across
/// the seam to the west was aimed at as if it lay east).
fn catapult_aim(g: rr_core::Grid, from: MapCoord, to: MapCoord) -> u32 {
    let wrap = |d: i32, size: i32| {
        if d > size / 2 {
            d - size
        } else if d < -size / 2 {
            d + size
        } else {
            d
        }
    };
    let dx = wrap(to.x as i32 - from.x as i32, g.width as i32);
    let dy = wrap(to.y as i32 - from.y as i32, g.height as i32);
    let sector = aim_sector(dx, dy);
    let w = sector.min(6 - sector);
    15 * (w + 1) + 15 * (w + 3)
}

/// The sixth of a turn (0 east, counting toward +y) nearest the direction (dx, dy), in integers:
/// the borders at 30 and 150 degrees are where 3 dy² = dx² (tan² 30° = 1/3), never met exactly
/// by whole offsets (net round 4, #8: the sim's only float, a libm atan2).
fn aim_sector(dx: i32, dy: i32) -> u32 {
    let (x2, y2) = ((dx as i64).pow(2), 3 * (dy as i64).pow(2));
    let flat = y2 < x2;
    match (dx.signum(), dy.signum()) {
        (0, 0) | (1, 0) => 0,
        (-1, 0) => 3,
        (1, 1) => {
            if flat {
                0
            } else {
                1
            }
        }
        (1, -1) => {
            if flat {
                0
            } else {
                5
            }
        }
        (-1, 1) => {
            if flat {
                3
            } else {
                2
            }
        }
        (-1, -1) => {
            if flat {
                3
            } else {
                4
            }
        }
        // straight down and up: 90 degrees rounds up to 2, -90 to 5
        (0, 1) => 2,
        _ => 5,
    }
}

#[cfg(test)]
mod aim_tests {
    use rr_core::{Grid, MapCoord};

    /// Net round 4, #8: the integer sector is the float one for every offset up to 64.
    #[test]
    fn the_aim_is_the_float_aim_in_integers() {
        for dx in -64..=64i32 {
            for dy in -64..=64i32 {
                let f = ((dy as f32).atan2(dx as f32).to_degrees() + 360.0) / 60.0;
                assert_eq!(super::aim_sector(dx, dy), f.round() as u32 % 6, "({dx}, {dy})");
            }
        }
    }

    #[test]
    fn the_aim_wraps_with_the_map() {
        let g = Grid::new(64, 64);
        let at = MapCoord::new(1, 20);
        // due west across the seam: half a turn
        assert_eq!(super::catapult_aim(g, at, MapCoord::new(60, 20)), 150);
        // due east, no wrap
        assert_eq!(super::catapult_aim(g, at, MapCoord::new(6, 20)), 60);
        assert_eq!(super::catapult_aim(g, MapCoord::new(10, 20), MapCoord::new(4, 20)), 150);
    }
}

/// RTTR nobMilitary::LookForEnemyBuildings for one enemy building `dist` away: 3 (near) when the
/// two military radii touch, 1 (middle) when either side could attack the other
/// (`dist < 21 + max troops - 1`, the attacker's troops; warehouses send none), else 0 (far).
/// `me` and the enemy are (radius, max troops); `None` troops: a warehouse.
/// A ready military building or warehouse as far as borders go: (player, pos, radius, troops,
/// a harbor).
pub(crate) type FrontierPost = (PlayerId, rr_core::MapCoord, u16, Option<u8>, bool);

pub(crate) fn frontier_class(dist: u32, me: (u16, u8), them: (u16, Option<u8>)) -> usize {
    const BASE_ATTACKING_DISTANCE: u32 = 21;
    if dist <= me.0 as u32 + them.0 as u32 {
        3
    } else if dist < BASE_ATTACKING_DISTANCE + me.1 as u32 - 1
        || them.1.is_some_and(|t| dist < BASE_ATTACKING_DISTANCE + t as u32 - 1)
    {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod frontier_tests {
    use super::frontier_class;

    /// Veteran round 3, #7: the border classes follow RTTR's formula.
    #[test]
    fn frontier_classes() {
        // a guardhouse (radius 9, 3 soldiers) and an enemy barracks (radius 8, 2 soldiers)
        let (gh, bar) = ((9, 3), (8, Some(2)));
        assert_eq!(frontier_class(17, gh, bar), 3);
        assert_eq!(frontier_class(18, gh, bar), 1);
        assert_eq!(frontier_class(22, gh, bar), 1);
        assert_eq!(frontier_class(23, gh, bar), 0);
        // a fortress's 9 troops reach further: the fortress side counts as middle up to 28
        assert_eq!(frontier_class(28, (12, 9), bar), 1);
        assert_eq!(frontier_class(28, gh, (12, Some(9))), 1);
        assert_eq!(frontier_class(29, gh, (12, Some(9))), 0);
        // an enemy headquarters sends no attackers
        assert_eq!(frontier_class(24, gh, (9, None)), 0);
    }
}
