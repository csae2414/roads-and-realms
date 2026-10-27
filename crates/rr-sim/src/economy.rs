//! Buildings, construction sites, warehouses, wares and the distributor.

use crate::figures::FigKind;
use crate::{BuildingId, Event, FigId, FlagId, PlayerId, PlayerSettings, RejectReason, Sim, WareId};
use rr_core::{Direction, MapCoord, Purpose};
use rr_rules::{BuildingType, Job, Size, Ware};
use rr_world::{Bq, NodeObj, RoadKind};
use std::collections::BTreeMap;

pub const STATS_EVERY: u32 = 600;
/// Frames between two moves of the warehouse settings.
const POLICY_EVERY: u32 = 100;
const DISTRIBUTE_EVERY: u32 = 20;
/// Workshops order one input ware this often (RTTR nobUsual: 210 GF).
const ORDER_EVERY: u32 = 210;
/// Build steps per material, and the frames each step takes (RTTR `nofBuilder`).
pub const STEPS_PER_MATERIAL: u16 = 8;
pub const BUILD_STEP_GF: u32 = 57;
const WAREHOUSE_OUT_GF: u32 = 20;
const WAREHOUSE_OUT_RAND: u32 = 10;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Inventory {
    pub wares: [u32; Ware::COUNT],
    pub jobs: [u32; Job::COUNT],
}

impl Default for Inventory {
    fn default() -> Self {
        Self { wares: [0; Ware::COUNT], jobs: [0; Job::COUNT] }
    }
}

impl Inventory {
    pub fn ware(&self, w: Ware) -> u32 {
        self.wares[w.index()]
    }
    pub fn job(&self, j: Job) -> u32 {
        self.jobs[j.index()]
    }
    pub fn add_all(&mut self, o: &Inventory) {
        for i in 0..Ware::COUNT {
            self.wares[i] += o.wares[i];
        }
        for i in 0..Job::COUNT {
            self.jobs[i] += o.jobs[i];
        }
    }
    pub fn soldiers(&self) -> u32 {
        Job::SOLDIERS.iter().map(|j| self.job(*j)).sum()
    }

    /// Can a helper here become a `job` (taking its tool, if it needs one)? Soldiers are
    /// recruited and donkeys bred, never made this way.
    pub fn can_make(&self, job: Job, rules: &rr_rules::Rules) -> bool {
        job != Job::Helper
            && job != Job::PackDonkey
            && job.soldier_rank().is_none()
            && self.job(Job::Helper) > 0
            && rules.job(job).tool.is_none_or(|t| self.ware(t) > 0)
    }

    /// Is a `job` here, or can one be made?
    pub fn can_supply(&self, job: Job, rules: &rr_rules::Rules) -> bool {
        self.job(job) > 0 || self.can_make(job, rules)
    }

    /// Take a `job` out of the stock, making one from a helper and its tool if none waits.
    pub fn take_person(&mut self, job: Job, rules: &rr_rules::Rules) -> bool {
        if self.job(job) > 0 {
            self.jobs[job.index()] -= 1;
            return true;
        }
        if !self.can_make(job, rules) {
            return false;
        }
        self.jobs[Job::Helper.index()] -= 1;
        if let Some(t) = rules.job(job).tool {
            self.wares[t.index()] -= 1;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::Inventory;
    use rr_rules::{Job, Rules, Ware};

    #[test]
    fn people_are_made_from_helpers_and_tools() {
        let rules = Rules::classic();
        let mut inv = Inventory::default();
        inv.jobs[Job::Helper.index()] = 2;
        assert!(!inv.can_supply(Job::Woodcutter, &rules), "no axe");
        assert!(!inv.can_supply(Job::PackDonkey, &rules), "donkeys are bred");
        assert!(!inv.can_supply(Job::Private, &rules), "soldiers are recruited");
        inv.wares[Ware::Axe.index()] = 1;
        assert!(inv.take_person(Job::Woodcutter, &rules));
        assert_eq!((inv.job(Job::Helper), inv.ware(Ware::Axe)), (1, 0));
        // the helper job itself needs nothing
        assert!(inv.can_supply(Job::Helper, &rules));
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Site {
    pub planing: bool,
    /// Nodes the planer still has to level.
    pub to_level: Vec<MapCoord>,
    pub boards: u8,
    pub stones: u8,
    pub used_boards: u8,
    pub used_stones: u8,
    pub progress: u16,
    pub builder: Option<FigId>,
    pub planer: Option<FigId>,
    /// A harbor founded by an expedition: holds territory while being built (RTTR).
    pub from_sea: bool,
}

impl Site {
    pub fn total_steps(cost: (u8, u8)) -> u16 {
        (cost.0 as u16 + cost.1 as u16) * STEPS_PER_MATERIAL
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BState {
    Site(Site),
    Ready,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Building {
    pub kind: BuildingType,
    pub pos: MapCoord,
    pub player: PlayerId,
    pub flag: FlagId,
    pub state: BState,
    pub worker: Option<FigId>,
    /// A worker is on the way (or wanted).
    pub worker_coming: bool,
    /// Input stock, by index into the rules' input list.
    pub stock: [u8; 3],
    /// Inputs on the way.
    pub incoming: [u8; 3],
    /// Construction material on the way: (boards, stones).
    pub site_incoming: (u8, u8),
    pub inventory: Option<Inventory>,
    /// Wares waiting to leave a warehouse.
    pub out_queue: Vec<WareId>,
    /// Figures waiting to leave a warehouse.
    pub fig_queue: Vec<FigId>,
    pub soldiers: Vec<FigId>,
    pub soldiers_coming: u8,
    /// Wanted garrison size (military buildings).
    pub garrison_wanted: u8,
    pub stopped: bool,
    pub coins_stopped: bool,
    /// Last 16 production attempts as bits (1 = produced), for the productivity display.
    pub history: u16,
    pub attempts: u8,
    /// Frame the next queue/production timer was set for.
    pub timer: Option<(u32, u64)>,
    /// Frame the building was placed (construction sites are supplied in this order), and in
    /// the top bit the player's priority (`placed()`, `priority()`: a bit of this field, not a
    /// field of its own, so the saves keep their layout).
    pub(crate) placed: u32,
    /// Mines/fisheries/... found nothing on the last attempt.
    pub exhausted: bool,
    /// Military buildings: a soldier has moved in once. From then on the building holds its land,
    /// even while its soldiers are out (RTTR `!new_built`).
    pub occupied: bool,
    /// When it was first occupied (or captured).
    pub occupied_at: u32,
    /// Workshops order one input ware at a time (RTTR nobUsual): the input asked for next, and
    /// from when.
    pub order_input: u8,
    pub order_at: u32,
    /// Shipyards: sea ships instead of boats (RTTR nobShipYard mode; boats by default).
    pub build_ships: bool,
    /// Warehouses: soldiers per rank kept at home (RTTR reserves; none by default).
    pub reserve: [u8; 5],
    /// Warehouses: next frame a helper appears / a soldier is recruited.
    pub next_helper: u32,
    pub next_recruit: u32,
    /// Military buildings: the frame the next soldier heals, and when a delivered coin
    /// promotes (both run on their own clock, whatever else wakes the building).
    pub next_heal: u32,
    pub promote_at: Option<u32>,
    /// Warehouses: the setting for each ware, and when the settings act next.
    pub stock_policy: Vec<crate::StockPolicy>,
    /// The same settings for people, by job.
    pub people_policy: Vec<crate::StockPolicy>,
    pub next_policy: u32,
    /// Harbors: the expedition being prepared, if any.
    pub expedition: Option<crate::seafaring::Expedition>,
    /// Harbors: goods and people waiting for a ship, with their destination harbor.
    pub ship_queue: Vec<(crate::seafaring::Cargo, BuildingId)>,
    /// Harbors: wares wanted for shipping overseas (requested like a building's inputs).
    pub sea_demand: Vec<u8>,
    /// Harbors: wares and people on their way here by ship.
    pub sea_incoming: Vec<u8>,
}

/// The top bit of `Building::placed`: the player's priority.
const PRIORITY_BIT: u32 = 1 << 31;

impl Building {
    /// The frame it was placed.
    pub fn placed(&self) -> u32 {
        self.placed & !PRIORITY_BIT
    }

    /// The player gave it priority (`Cmd::SetPriority`).
    pub fn priority(&self) -> bool {
        self.placed & PRIORITY_BIT != 0
    }

    pub fn is_ready(&self) -> bool {
        self.state == BState::Ready
    }

    pub fn site(&self) -> Option<&Site> {
        match &self.state {
            BState::Site(s) => Some(s),
            BState::Ready => None,
        }
    }

    /// Productivity in percent over the last attempts.
    pub fn productivity(&self) -> u32 {
        if self.attempts == 0 {
            return 0;
        }
        let n = self.attempts.min(16) as u32;
        self.history.count_ones() * 100 / n
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WareLoc {
    Flag(FlagId),
    Carried(FigId),
    /// Waiting in a warehouse's out queue, or being carried out by a worker.
    Building(BuildingId),
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct WareItem {
    pub kind: Ware,
    pub player: PlayerId,
    pub loc: WareLoc,
    /// Where the ware is going. `None`: no destination yet.
    pub goal: Option<BuildingId>,
    /// Direction of the next road at the current flag.
    pub next: Option<Direction>,
}

/// Statistics sampled every `STATS_EVERY` frames.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Stats {
    pub samples: Vec<Sample>,
    pub produced: u32,
    /// Enemy buildings taken or burnt by our soldiers.
    pub vanquished: u32,
    /// Wares produced so far, by kind.
    pub produced_by_ware: [u32; Ware::COUNT],
}

/// One point of the statistics window's curves (the original's categories).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Sample {
    pub gf: u32,
    pub land: u32,
    pub buildings: u32,
    /// Everybody: people in warehouses and on the map.
    pub inhabitants: u32,
    /// Wares in warehouses and on the way.
    pub merchandise: u32,
    /// Soldiers weighted by rank (a general counts 5).
    pub military: u32,
    /// Coins minted so far (RTTR nofMinter raises its Gold statistic per coin: the coins in
    /// store read 0-4 all game, promotions spend them as they come).
    pub gold: u32,
    /// Average productivity of the production buildings, in percent.
    pub productivity: u32,
    pub vanquished: u32,
    /// Wares produced so far.
    pub produced: u32,
    /// Wares produced so far, by kind (the original's ware statistics).
    pub by_ware: [u32; Ware::COUNT],
}

impl Sample {
    pub const CATEGORIES: [&'static str; 9] = [
        "Land",
        "Buildings",
        "Inhabitants",
        "Merchandise",
        "Military",
        "Gold",
        "Productivity",
        "Vanquished",
        "Produced",
    ];

    pub fn value(&self, category: usize) -> u32 {
        [
            self.land,
            self.buildings,
            self.inhabitants,
            self.merchandise,
            self.military,
            self.gold,
            self.productivity,
            self.vanquished,
            self.produced,
        ][category.min(8)]
    }
}

/// RTTR `distributionMap` defaults. `Headquarters` stands for construction sites; the fish
/// entries apply to all three foods.
pub(crate) fn default_settings() -> PlayerSettings {
    use BuildingType as B;
    use Ware as W;
    let distribution = vec![
        (W::Fish, B::GraniteMine, 3),
        (W::Fish, B::CoalMine, 5),
        (W::Fish, B::IronMine, 7),
        (W::Fish, B::GoldMine, 10),
        (W::Grain, B::Mill, 5),
        (W::Grain, B::PigFarm, 3),
        (W::Grain, B::DonkeyBreeder, 2),
        (W::Grain, B::Brewery, 3),
        (W::Iron, B::Armory, 8),
        (W::Iron, B::Metalworks, 4),
        (W::Coal, B::Armory, 8),
        (W::Coal, B::IronSmelter, 7),
        (W::Coal, B::Mint, 10),
        (W::Wood, B::Sawmill, 8),
        (W::Boards, B::Headquarters, 10),
        (W::Boards, B::Metalworks, 4),
        (W::Boards, B::Shipyard, 2),
        (W::Water, B::Bakery, 6),
        (W::Water, B::Brewery, 3),
        (W::Water, B::PigFarm, 2),
        (W::Water, B::DonkeyBreeder, 2),
        (W::Ham, B::Slaughterhouse, 8),
        (W::Stones, B::Headquarters, 10),
        (W::Stones, B::Catapult, 5),
    ];
    // RTTR GamePlayer::LoadStandardToolSettings
    let tools = vec![
        (W::Tongs, 1),
        (W::Hammer, 4),
        (W::Axe, 2),
        (W::Saw, 5),
        (W::PickAxe, 7),
        (W::Shovel, 1),
        (W::Crucible, 3),
        (W::RodAndLine, 1),
        (W::Scythe, 2),
        (W::Cleaver, 1),
        (W::Rollingpin, 2),
        (W::Bow, 1),
    ];
    PlayerSettings {
        distribution,
        transport_order: default_transport_order(),
        military: [10, 3, 5, 3, 0, 1, 8, 8],
        tools,
    }
}

fn default_transport_order() -> Vec<Ware> {
    use Ware::*;
    let mut v = vec![
        Coins, Sword, Shield, Beer, Iron, Gold, IronOre, Coal, Boards, Stones, Wood, Water, Fish, Meat, Bread, Grain,
        Flour, Ham,
    ];
    for w in Ware::ALL {
        if !v.contains(w) {
            v.push(*w);
        }
    }
    v
}

impl Sim {
    // ------------------------------------------------------------------ placement

    pub(crate) fn place_hq(
        &mut self,
        player: PlayerId,
        pos: MapCoord,
        goods: crate::StartGoods,
    ) -> Result<BuildingId, RejectReason> {
        // HQs are placed by the map; only the terrain must allow a castle.
        if !self.world.bq(pos).allows(Bq::Castle) {
            return Err(RejectReason::NotBuildable);
        }
        let se = self.world.grid().neighbor(pos, Direction::SE);
        let flag = self.create_flag(player, se);
        let mut inv = Inventory::default();
        for (w, n) in &self.rules.start.wares {
            inv.wares[w.index()] = *n;
        }
        for (j, n) in &self.rules.start.jobs {
            inv.jobs[j.index()] = *n;
        }
        match goods {
            crate::StartGoods::Normal => {}
            crate::StartGoods::High => {
                inv.wares.iter_mut().chain(inv.jobs.iter_mut()).for_each(|n| *n *= 2);
            }
            crate::StartGoods::Low => {
                inv.wares.iter_mut().chain(inv.jobs.iter_mut()).for_each(|n| *n /= 2);
                for w in [Ware::Beer, Ware::Sword, Ware::Shield] {
                    inv.wares[w.index()] = 0;
                }
                inv.jobs[Job::Private.index()] = 26;
            }
        }
        let id = self.insert_building(player, BuildingType::Headquarters, pos, flag, BState::Ready);
        self.buildings[id].inventory = Some(inv);
        self.players[player as usize].hq = Some(id);
        self.territory_dirty = true;
        self.start_building_timer(id, 1);
        Ok(id)
    }

    pub(crate) fn insert_building_pub(
        &mut self,
        player: PlayerId,
        kind: BuildingType,
        pos: MapCoord,
        flag: FlagId,
        state: BState,
    ) -> BuildingId {
        self.insert_building(player, kind, pos, flag, state)
    }

    fn insert_building(
        &mut self,
        player: PlayerId,
        kind: BuildingType,
        pos: MapCoord,
        flag: FlagId,
        state: BState,
    ) -> BuildingId {
        let def = self.rules.building(kind);
        let garrison = def.military.as_ref().map_or(0, |m| m.soldiers);
        let id = self.buildings.insert(Building {
            kind,
            pos,
            player,
            flag,
            state,
            worker: None,
            worker_coming: false,
            stock: [0; 3],
            incoming: [0; 3],
            site_incoming: (0, 0),
            inventory: None,
            out_queue: Vec::new(),
            fig_queue: Vec::new(),
            soldiers: Vec::new(),
            soldiers_coming: 0,
            garrison_wanted: garrison,
            stopped: false,
            coins_stopped: false,
            history: 0,
            attempts: 0,
            timer: None,
            exhausted: false,
            occupied: false,
            occupied_at: 0,
            order_input: 0,
            order_at: 0,
            build_ships: false,
            reserve: [0; 5],
            next_helper: 0,
            next_recruit: 0,
            next_heal: 0,
            promote_at: None,
            stock_policy: vec![crate::StockPolicy::Normal; Ware::COUNT],
            people_policy: vec![crate::StockPolicy::Normal; Job::COUNT],
            next_policy: 0,
            expedition: None,
            ship_queue: Vec::new(),
            sea_demand: vec![0; Ware::COUNT],
            sea_incoming: vec![0; Ware::COUNT + Job::COUNT],
            placed: self.gf,
        });
        self.world.set_obj(pos, NodeObj::Building(id.index() as u32));
        if matches!(def.size, Size::Castle | Size::Harbor) {
            for d in [Direction::W, Direction::NW, Direction::NE] {
                let q = self.world.grid().neighbor(pos, d);
                if matches!(self.world.obj(q), NodeObj::None | NodeObj::Decoration(_)) {
                    self.world.set_obj(q, NodeObj::Extension);
                }
            }
        }
        self.flags[flag].building = Some(id);
        id
    }

    /// Is there any own spot left where a military building may be placed?
    pub fn military_spot_exists(&self, player: PlayerId) -> bool {
        self.world.grid().coords().any(|p| {
            self.world.owner(p) == Some(player)
                && self.world.bq_for(p, player).allows(Bq::Hut)
                && !self.military_building_near(p, player)
        })
    }

    /// An own military building (or site), headquarters or harbor within 4 nodes of `at`? Then no
    /// new military building may go there (RTTR GameWorldBase::IsMilitaryBuildingNearNode).
    pub fn military_building_near(&self, at: MapCoord, player: PlayerId) -> bool {
        let g = self.world.grid();
        g.within(at, 4).filter(|p| *p != at).any(|p| {
            self.world.owner(p) == Some(player)
                && self.building_at(p).is_some_and(|b| {
                    let k = self.buildings[b].kind;
                    is_frontier_military(k) || matches!(k, BuildingType::Headquarters | BuildingType::Harbor)
                })
        })
    }

    pub(crate) fn cmd_place_building(
        &mut self,
        player: PlayerId,
        at: MapCoord,
        kind: BuildingType,
    ) -> Result<BuildingId, RejectReason> {
        if !self.world.grid().contains(at) {
            return Err(RejectReason::NotFound);
        }
        if kind == BuildingType::Headquarters {
            return Err(RejectReason::NotAllowed);
        }
        if self.world.owner(at) != Some(player) {
            return Err(RejectReason::NotYourTerritory);
        }
        let need = match self.rules.building(kind).size {
            Size::Hut => Bq::Hut,
            Size::House => Bq::House,
            Size::Castle => Bq::Castle,
            Size::Mine => Bq::Mine,
            Size::Harbor => Bq::Harbor,
        };
        if !self.world.bq_for(at, player).allows(need) {
            return Err(RejectReason::NotBuildable);
        }
        if is_frontier_military(kind) && self.military_building_near(at, player) {
            return Err(RejectReason::NotAllowed);
        }
        let se = self.world.grid().neighbor(at, Direction::SE);
        let flag = match self.flag_at(se) {
            Some(f) => f,
            None => self.create_flag(player, se),
        };
        let size = self.rules.building(kind).size;
        let mut site = Site::default();
        if matches!(size, Size::House | Size::Castle | Size::Harbor) {
            let alt = self.world.altitude(at);
            site.to_level = Direction::ALL
                .iter()
                .filter(|d| **d != Direction::SE)
                .map(|d| self.world.grid().neighbor(at, *d))
                .filter(|q| self.world.altitude(*q) != alt)
                .collect();
            site.planing = !site.to_level.is_empty();
        }
        let id = self.insert_building(player, kind, at, flag, BState::Site(site));
        // a building is connected to its flag by a short "road" that needs no carrier
        self.players[player as usize].road_epoch += 1;
        self.route_cache.borrow_mut().clear();
        self.events.push(Event::Sound { at, sound: crate::SoundCue::Build });
        Ok(id)
    }

    pub(crate) fn cmd_remove_building(&mut self, player: PlayerId, at: MapCoord) -> Result<(), RejectReason> {
        let b = self.building_at(at).ok_or(RejectReason::NotFound)?;
        if self.buildings[b].player != player || self.buildings[b].kind == BuildingType::Headquarters {
            return Err(RejectReason::NotAllowed);
        }
        self.destroy_building(b, true);
        // the owner's own doing: removed, not lost
        if let Some(e) =
            self.events.iter_mut().rev().find(|e| matches!(e, Event::BuildingLost { at: p, .. } if *p == at))
        {
            if let Event::BuildingLost { player, at, kind } = *e {
                *e = Event::BuildingRemoved { player, at, kind };
            }
        }
        Ok(())
    }

    /// The player's priority for a building (`Cmd::SetPriority`).
    pub(crate) fn cmd_set_priority(&mut self, player: PlayerId, at: MapCoord, on: bool) -> Result<(), RejectReason> {
        let b = self.building_at(at).ok_or(RejectReason::NotFound)?;
        let bd = &mut self.buildings[b];
        // (warehouses ask for nothing: the castle, storehouses and harbors keep no priority)
        if bd.player != player || (bd.inventory.is_some() && bd.is_ready()) {
            return Err(RejectReason::NotAllowed);
        }
        bd.placed = if on { bd.placed | PRIORITY_BIT } else { bd.placed & !PRIORITY_BIT };
        Ok(())
    }

    pub(crate) fn cmd_toggle_production(&mut self, player: PlayerId, at: MapCoord) -> Result<(), RejectReason> {
        let b = self.building_at(at).ok_or(RejectReason::NotFound)?;
        if self.buildings[b].player != player {
            return Err(RejectReason::NotAllowed);
        }
        let bd = &mut self.buildings[b];
        if self.rules.building(bd.kind).is_military() {
            bd.coins_stopped = !bd.coins_stopped;
            if bd.coins_stopped {
                // RTTR SetCoinsAllowed(false): coins on their way turn back to a warehouse
                self.release_wares_heading_to(b, Some(Ware::Coins));
            }
        } else {
            bd.stopped = !bd.stopped;
        }
        Ok(())
    }

    /// Remove a building. Workers and soldiers inside go home; stock is lost.
    pub(crate) fn destroy_building(&mut self, id: BuildingId, fire: bool) {
        let Some(b) = self.buildings.get(id).cloned() else {
            return;
        };
        let def = self.rules.building(b.kind).clone();
        // only a finished building burns; a site leaves bare ground (RTTR noBuildingSite::Destroy;
        // owner's playtest: a site with nothing built yet went up in flames)
        let fire = fire && b.is_ready();
        self.events.push(Event::BuildingLost { player: b.player, at: b.pos, kind: b.kind });
        if fire {
            self.events.push(Event::Sound { at: b.pos, sound: crate::SoundCue::Fire });
        }
        if let Some(key) = b.timer {
            self.sched.cancel(key);
        }
        // people inside leave
        let mut leaving: Vec<FigId> = Vec::new();
        leaving.extend(b.worker);
        leaving.extend(b.soldiers.iter().copied());
        leaving.extend(b.fig_queue.iter().copied());
        if let BState::Site(s) = &b.state {
            leaving.extend(s.builder);
            leaving.extend(s.planer);
        }
        for w in &b.out_queue {
            self.lose_ware(*w);
        }
        if let Some(f) = self.flags.get_mut(b.flag) {
            f.building = None;
        }
        self.world.set_obj(b.pos, if fire { NodeObj::Fire } else { NodeObj::None });
        if fire {
            self.nature_schedule_fire_end(b.pos);
        }
        if matches!(def.size, Size::Castle | Size::Harbor) {
            for d in [Direction::W, Direction::NW, Direction::NE] {
                let q = self.world.grid().neighbor(b.pos, d);
                if self.world.obj(q) == NodeObj::Extension {
                    self.world.set_obj(q, NodeObj::None);
                }
            }
        }
        self.buildings.remove(id);
        // only now: wares on their way are routed again, and must not pick this warehouse (QA
        // round 5, #1: they kept its dead id)
        self.release_wares_heading_to(id, None);
        for f in leaving {
            self.figure_lost_home(f);
        }
        // shipments that were waiting here will not come
        for (c, dest) in &b.ship_queue {
            if let Some(d) = self.buildings.get_mut(*dest) {
                let i = crate::seafaring::cargo_slot(*c);
                d.sea_incoming[i] = d.sea_incoming[i].saturating_sub(1);
            }
        }
        // a loaded expedition ship looks for another harbor
        if let Some(s) = b.expedition.as_ref().and_then(|e| e.ship)
            && self.ships.contains(s)
        {
            self.expedition_return(s);
        }
        // figures walking towards this building
        let coming: Vec<FigId> =
            self.figures.iter().filter(|(_, f)| f.target_building() == Some(id)).map(|(i, _)| i).collect();
        for f in coming {
            // still waiting in a warehouse's exit queue: straight back into its stock
            let queued =
                self.figures[f].inside.filter(|wh| self.buildings.get(*wh).is_some_and(|w| w.fig_queue.contains(&f)));
            if let Some(wh) = queued {
                self.buildings[wh].fig_queue.retain(|x| *x != f);
                self.store_in_warehouse(f, wh);
            } else {
                self.figure_lost_home(f);
            }
        }
        // nobody stays "inside" a building that is gone (QA round 9, #1: a Helper walking home
        // from it kept the link, and no save loaded for 151 frames)
        let inside: Vec<FigId> = self.figures.iter().filter(|(_, f)| f.inside == Some(id)).map(|(i, _)| i).collect();
        for f in inside {
            self.figures[f].inside = None;
        }
        if def.military.is_some() {
            self.territory_dirty = true;
        }
        self.players[b.player as usize].road_epoch += 1;
        self.route_cache.borrow_mut().clear();
        // losing the last warehouse or military building ends the player
        if b.kind == BuildingType::Headquarters || b.kind.is_warehouse() || def.military.is_some() {
            self.check_defeat(b.player);
        }
    }

    // ------------------------------------------------------------------ wares

    /// Wares heading to building `id` need a new goal (it burnt or changed hands).
    /// `only`: just wares of that kind, which the building no longer expects.
    pub(crate) fn release_wares_heading_to(&mut self, id: BuildingId, only: Option<Ware>) {
        let heading: Vec<WareId> = self
            .wares
            .iter()
            .filter(|(_, w)| w.goal == Some(id) && only.is_none_or(|k| w.kind == k))
            .map(|(i, _)| i)
            .collect();
        for w in heading {
            if only.is_some() {
                self.unreserve(id, self.wares[w].kind);
            }
            self.wares[w].goal = None;
            self.wares[w].next = None;
            if let WareLoc::Building(wh) = self.wares[w].loc {
                // still queued in a warehouse: back into its stock
                let kind = self.wares[w].kind;
                self.wares.remove(w);
                if let Some(whb) = self.buildings.get_mut(wh) {
                    whb.out_queue.retain(|x| *x != w);
                    if let Some(inv) = whb.inventory.as_mut() {
                        inv.wares[kind.index()] += 1;
                    }
                }
            } else if let WareLoc::Flag(f) = self.wares[w].loc {
                self.route_ware(w, f);
            }
        }
    }

    pub(crate) fn lose_ware(&mut self, w: WareId) {
        let Some(ware) = self.wares.remove(w) else {
            return;
        };
        if let Some(g) = ware.goal {
            self.unreserve(g, ware.kind);
        }
        if let WareLoc::Flag(f) = ware.loc
            && let Some(flag) = self.flags.get_mut(f)
        {
            flag.wares.retain(|x| *x != w);
        }
    }

    /// A building no longer expects this ware.
    fn unreserve(&mut self, b: BuildingId, kind: Ware) {
        let def = self.rules.clone();
        let Some(bd) = self.buildings.get_mut(b) else {
            return;
        };
        match &bd.state {
            BState::Site(_) => {
                if kind == Ware::Boards {
                    bd.site_incoming.0 = bd.site_incoming.0.saturating_sub(1);
                } else if kind == Ware::Stones {
                    bd.site_incoming.1 = bd.site_incoming.1.saturating_sub(1);
                }
            }
            BState::Ready => {
                if let Some(i) = def.building(bd.kind).inputs.iter().position(|x| *x == kind) {
                    bd.incoming[i] = bd.incoming[i].saturating_sub(1);
                }
            }
        }
    }

    /// Put a ware down at a flag and decide where it goes next.
    pub(crate) fn put_ware_at_flag(&mut self, w: WareId, f: FlagId) {
        self.wares[w].loc = WareLoc::Flag(f);
        self.flags[f].wares.push(w);
        self.route_ware(w, f);
    }

    pub fn flag_has_room(&self, f: FlagId) -> bool {
        self.flags[f].wares.len() < self.rules.economy.flag_capacity as usize
    }

    /// Pick the next road for a ware lying at flag `f`; wake the carrier.
    pub(crate) fn route_ware(&mut self, w: WareId, f: FlagId) {
        let ware = &self.wares[w];
        let goal = match ware.goal {
            Some(g) if self.buildings.contains(g) => Some(g),
            _ => {
                let g = self.nearest_warehouse_for(ware.player, f, Some(ware.kind));
                if let Some(g) = g {
                    self.wares[w].goal = Some(g);
                }
                g
            }
        };
        let Some(goal) = goal else {
            self.wares[w].next = None;
            return;
        };
        let goal_flag = self.buildings[goal].flag;
        if goal_flag == f {
            // destination is the building at this very flag: carry it in directly
            self.wares[w].next = Some(Direction::NW);
            self.deliver_from_flag(w, f);
            return;
        }
        let next = self.ware_next_step(f, goal_flag);
        self.wares[w].next = next;
        if let Some(d) = next
            && let Some(r) = self.flags[f].roads[d.index() as usize]
        {
            self.wake_carrier(r);
        }
    }

    /// A ware at the flag of its goal building goes inside (carried by the building's people).
    fn deliver_from_flag(&mut self, w: WareId, f: FlagId) {
        self.flags[f].wares.retain(|x| *x != w);
        let b = self.wares[w].goal.expect("goal");
        self.deliver_ware(w, b);
    }

    /// Re-route every ware lying at flag `f` (its roads changed).
    pub(crate) fn reroute_flag(&mut self, f: FlagId) {
        let Some(flag) = self.flags.get(f) else {
            return;
        };
        for w in flag.wares.clone() {
            if self.wares.get(w).is_some_and(|x| x.loc == WareLoc::Flag(f)) {
                self.route_ware(w, f);
            }
        }
    }

    /// Wares at flags with no way on (or whose road is gone) try again; if their goal stays
    /// unreachable, they give it up and head for a warehouse, so the goal can order anew.
    fn rescue_stranded_wares(&mut self, player: PlayerId) {
        let stranded: Vec<(WareId, FlagId)> = self
            .wares
            .iter()
            .filter(|(_, w)| w.player == player)
            .filter_map(|(id, w)| match w.loc {
                WareLoc::Flag(f) if w.next.is_none_or(|d| self.flags[f].roads[d.index() as usize].is_none()) => {
                    Some((id, f))
                }
                _ => None,
            })
            .collect();
        for (w, f) in stranded {
            self.route_ware(w, f);
            let Some(ware) = self.wares.get(w) else {
                continue;
            };
            if ware.next.is_none()
                && ware.loc == WareLoc::Flag(f)
                && let Some(g) = ware.goal
            {
                let kind = ware.kind;
                self.wares[w].goal = None;
                self.unreserve(g, kind);
                self.route_ware(w, f);
            }
        }
    }

    /// A ware arrives inside building `b`.
    pub(crate) fn deliver_ware(&mut self, w: WareId, b: BuildingId) {
        let Some(ware) = self.wares.remove(w) else {
            return;
        };
        let rules = self.rules.clone();
        let Some(bd) = self.buildings.get_mut(b) else {
            return;
        };
        if let Some(inv) = bd.inventory.as_mut() {
            inv.wares[ware.kind.index()] += 1;
            return;
        }
        match &mut bd.state {
            BState::Site(s) => {
                if ware.kind == Ware::Boards {
                    s.boards += 1;
                    bd.site_incoming.0 = bd.site_incoming.0.saturating_sub(1);
                } else if ware.kind == Ware::Stones {
                    s.stones += 1;
                    bd.site_incoming.1 = bd.site_incoming.1.saturating_sub(1);
                }
                if let Some(builder) = s.builder {
                    self.wake_figure(builder);
                }
            }
            BState::Ready => {
                let military = rules.building(bd.kind).is_military();
                if let Some(i) = rules.building(bd.kind).inputs.iter().position(|x| *x == ware.kind) {
                    bd.stock[i] += 1;
                    bd.incoming[i] = bd.incoming[i].saturating_sub(1);
                    // the worker waiting inside starts now
                    if let Some(w) = bd.worker {
                        self.wake_worker(w);
                    }
                }
                // a coin for a military building starts the promotion clock (QA round 4, #4)
                if military && ware.kind == Ware::Coins {
                    // RTTR: the promotion follows after 100 + rand(300) GF
                    let mut rng = self.rng_building(b, Purpose::Military);
                    let wait = rules.military.promote_wait + rng.below(rules.military.promote_rand + 1);
                    let at = self.gf + wait;
                    let bd = &mut self.buildings[b];
                    bd.promote_at = Some(bd.promote_at.map_or(at, |p| p.min(at)));
                    self.start_building_timer(b, wait);
                }
            }
        }
    }

    /// The nearest connected warehouse that accepts this ware.
    /// The warehouse a ware goes to: the nearest that collects it, else the nearest that takes
    /// it in at all (the warehouse settings "stop" and "move out" refuse it).
    pub(crate) fn nearest_warehouse_for(
        &self,
        player: PlayerId,
        from: FlagId,
        ware: Option<Ware>,
    ) -> Option<BuildingId> {
        use crate::StockPolicy as P;
        let policy =
            |b: &Building| ware.map_or(P::Normal, |w| b.stock_policy.get(w.index()).copied().unwrap_or_default());
        let whs: Vec<(BuildingId, FlagId, P)> = self
            .buildings
            .iter()
            .filter(|(_, b)| b.player == player && b.inventory.is_some() && b.is_ready())
            .map(|(id, b)| (id, b.flag, policy(b)))
            .filter(|(_, _, p)| !matches!(p, P::Stop | P::MoveOut))
            .collect();
        if whs.is_empty() {
            return None;
        }
        let routes = self.routes_from(from);
        // (not collecting, distance, id): collecting warehouses first
        whs.into_iter()
            .filter_map(|(id, f, p)| {
                let d = routes.get(f.index()).map_or(u32::MAX, |r| r.0);
                (d != u32::MAX).then_some(((p != P::Collect) as u8, d, id.index(), id))
            })
            .min()
            .map(|x| x.3)
    }

    pub(crate) fn cmd_stock_policy(
        &mut self,
        player: PlayerId,
        at: MapCoord,
        ware: Ware,
        policy: crate::StockPolicy,
    ) -> Result<(), RejectReason> {
        let b = self.building_at(at).ok_or(RejectReason::NotFound)?;
        let bd = &mut self.buildings[b];
        if bd.player != player || bd.inventory.is_none() {
            return Err(RejectReason::NotAllowed);
        }
        bd.stock_policy[ware.index()] = policy;
        Ok(())
    }

    pub(crate) fn cmd_people_policy(
        &mut self,
        player: PlayerId,
        at: MapCoord,
        job: Job,
        policy: crate::StockPolicy,
    ) -> Result<(), RejectReason> {
        let b = self.building_at(at).ok_or(RejectReason::NotFound)?;
        let bd = &mut self.buildings[b];
        if bd.player != player || bd.inventory.is_none() {
            return Err(RejectReason::NotAllowed);
        }
        bd.people_policy[job.index()] = policy;
        Ok(())
    }

    /// The warehouses a `job` coming home may enter: those that collect it first, then those
    /// that take it in; if every one refuses, all of them (people must go somewhere).
    pub(crate) fn homes_for(&self, player: PlayerId, job: Job) -> Vec<(BuildingId, MapCoord, bool)> {
        use crate::StockPolicy as P;
        let all: Vec<(BuildingId, MapCoord, P)> = self
            .buildings
            .iter()
            .filter(|(_, b)| b.player == player && b.inventory.is_some() && b.is_ready())
            .map(|(id, b)| (id, b.pos, b.people_policy.get(job.index()).copied().unwrap_or_default()))
            .collect();
        let open: Vec<_> = all.iter().filter(|x| !matches!(x.2, P::Stop | P::MoveOut)).collect();
        let pick = if open.is_empty() { all.iter().collect() } else { open };
        pick.into_iter().map(|(id, p, pol)| (*id, *p, *pol == P::Collect)).collect()
    }

    /// Warehouse settings at work: one ware moves out to, or is fetched from, another
    /// warehouse every `POLICY_EVERY` frames.
    fn stock_policy_tick(&mut self, b: BuildingId) {
        use crate::StockPolicy as P;
        if self.gf < self.buildings[b].next_policy {
            return;
        }
        self.buildings[b].next_policy = self.gf + POLICY_EVERY;
        let (player, flag) = (self.buildings[b].player, self.buildings[b].flag);
        let pol = self.buildings[b].stock_policy.clone();
        let inv = self.buildings[b].inventory.clone().expect("warehouse");
        // people first: one settler moves per round
        let ppol = self.buildings[b].people_policy.clone();
        for j in Job::ALL {
            let routes = || self.routes_to(flag);
            match ppol[j.index()] {
                P::MoveOut if inv.job(*j) > 0 => {
                    let r = routes();
                    let dest = self
                        .homes_for(player, *j)
                        .into_iter()
                        .filter(|(id, _, _)| *id != b)
                        .filter_map(|(id, pos, collect)| {
                            let d = r.get(self.buildings[id].flag.index()).map_or(u32::MAX, |x| x.0);
                            (d != u32::MAX).then_some(((!collect) as u8, d, id.index(), pos))
                        })
                        .min();
                    if let Some((_, _, _, pos)) = dest {
                        let f = self.spawn_from_warehouse(b, *j, crate::figures::FigKind::Home);
                        self.figures[f].target = Some(pos);
                        return;
                    }
                }
                P::Collect => {
                    let r = routes();
                    let src = self
                        .buildings
                        .iter()
                        .filter(|(id, x)| *id != b && x.player == player && x.is_ready())
                        .filter(|(_, x)| x.people_policy.get(j.index()) != Some(&P::Collect))
                        .filter(|(_, x)| x.inventory.as_ref().is_some_and(|i| i.job(*j) > 0))
                        .filter_map(|(id, x)| {
                            let d = r.get(x.flag.index()).map_or(u32::MAX, |y| y.0);
                            (d != u32::MAX).then_some((d, id.index(), id))
                        })
                        .min();
                    if let Some((_, _, src)) = src {
                        let pos = self.buildings[b].pos;
                        let f = self.spawn_from_warehouse(src, *j, crate::figures::FigKind::Home);
                        self.figures[f].target = Some(pos);
                        return;
                    }
                }
                _ => {}
            }
        }
        for w in Ware::ALL {
            match pol[w.index()] {
                P::MoveOut if inv.ware(*w) > 0 => {
                    if let Some(dest) = self.nearest_warehouse_for(player, flag, Some(*w)).filter(|d| *d != b) {
                        self.take_from_warehouse(b, *w, dest);
                        return;
                    }
                }
                P::Collect => {
                    let routes = self.ware_routes_to(flag);
                    let src = self
                        .buildings
                        .iter()
                        .filter(|(id, x)| *id != b && x.player == player && x.is_ready())
                        .filter(|(_, x)| x.stock_policy.get(w.index()) != Some(&P::Collect))
                        .filter(|(_, x)| x.inventory.as_ref().is_some_and(|i| i.ware(*w) > 0))
                        .filter_map(|(id, x)| {
                            let d = routes.get(x.flag.index()).map_or(u32::MAX, |r| r.0);
                            (d != u32::MAX).then_some((d, id.index(), id))
                        })
                        .min();
                    if let Some((_, _, src)) = src {
                        self.take_from_warehouse(src, *w, b);
                        return;
                    }
                }
                _ => {}
            }
        }
    }

    /// Ware distances from flag `a` to all flags.
    pub(crate) fn routes_from(&self, a: FlagId) -> Vec<(u32, Option<Direction>)> {
        self.ware_routes_to(a)
    }

    // ------------------------------------------------------------------ distributor

    /// What a building still needs: (ware, how many, how many of those may come from a
    /// warehouse now). Workshops take fresh wares at flags for any free slot, but order from
    /// warehouses one ware per turn (RTTR FindClientForWare / nobUsual).
    fn needs(&self, b: &Building) -> Vec<(Ware, u8, u8)> {
        let def = self.rules.building(b.kind);
        match &b.state {
            BState::Site(s) => {
                let have_b = s.used_boards + s.boards + b.site_incoming.0;
                let have_s = s.used_stones + s.stones + b.site_incoming.1;
                let mut v = Vec::new();
                if have_b < def.boards {
                    v.push((Ware::Boards, def.boards - have_b, def.boards - have_b));
                }
                if have_s < def.stones {
                    v.push((Ware::Stones, def.stones - have_s, def.stones - have_s));
                }
                v
            }
            BState::Ready => {
                if b.inventory.is_some() {
                    return self.harbor_needs(b).into_iter().map(|(w, k)| (w, k, k)).collect();
                }
                if def.is_military() {
                    if b.coins_stopped || b.soldiers.is_empty() {
                        return Vec::new();
                    }
                    let have = b.stock[0] + b.incoming[0];
                    let slots = def.military.as_ref().map_or(0, |m| m.coins);
                    return if have < slots { vec![(Ware::Coins, slots - have, slots - have)] } else { Vec::new() };
                }
                if b.stopped && def.worker.is_some() && b.kind != BuildingType::Catapult {
                    return Vec::new();
                }
                let due = self.gf >= b.order_at;
                def.inputs
                    .iter()
                    .enumerate()
                    .filter_map(|(i, w)| {
                        let have = b.stock[i] + b.incoming[i];
                        // (one input at a time from the warehouses, RTTR's pace; the player's
                        // priority takes all it lacks)
                        let missing = def.slots_per_input.saturating_sub(have);
                        let from_wh = if b.priority() { missing } else { u8::from(due && i == b.order_input as usize) };
                        (have < def.slots_per_input).then(|| (*w, def.slots_per_input - have, from_wh))
                    })
                    .collect()
            }
        }
    }

    /// Harbors ask for what their expedition and their overseas customers still need.
    fn harbor_needs(&self, b: &Building) -> Vec<(Ware, u8)> {
        if b.kind != BuildingType::Harbor {
            return Vec::new();
        }
        let inv = b.inventory.as_ref().expect("harbor stock");
        let mut want = b.sea_demand.clone();
        want.resize(Ware::COUNT, 0);
        if let Some(e) = &b.expedition {
            let cost = self.expedition_load();
            want[Ware::Boards.index()] = want[Ware::Boards.index()].max(cost.0 - e.boards);
            want[Ware::Stones.index()] = want[Ware::Stones.index()].max(cost.1 - e.stones);
        }
        let me = self.building_at(b.pos);
        let mut out = Vec::new();
        for w in Ware::ALL {
            let n = want[w.index()] as u32;
            if n == 0 {
                continue;
            }
            let coming = self.wares.iter().filter(|(_, x)| x.goal == me && x.kind == *w).count() as u32;
            let have = inv.ware(*w) + coming;
            if have < n {
                out.push((*w, (n - have).min(255) as u8));
            }
        }
        out
    }

    fn weight(&self, player: PlayerId, ware: Ware, b: &Building) -> u8 {
        let key_ware = if ware.is_food() { Ware::Fish } else { ware };
        let key_bld = if b.site().is_some() { BuildingType::Headquarters } else { b.kind };
        self.players[player as usize]
            .settings
            .distribution
            .iter()
            .find(|e| e.0 == key_ware && e.1 == key_bld)
            .map_or(5, |e| e.2)
    }

    /// Match open needs with wares in warehouses and unassigned wares at flags.
    pub(crate) fn distribute(&mut self) {
        let n = self.players.len() as u32;
        for p in 0..n {
            if !(self.gf + p).is_multiple_of(DISTRIBUTE_EVERY) || !self.players[p as usize].alive {
                continue;
            }
            self.distribute_player(p as u8);
            self.assign_jobs(p as u8);
        }
    }

    fn distribute_player(&mut self, player: PlayerId) {
        self.rescue_stranded_wares(player);
        self.distribute_wares(player);
        // workshops whose order came up asked this pass: the next input in 210 GF (RTTR nobUsual)
        let gf = self.gf;
        for (_, b) in self.buildings.iter_mut() {
            let n = self.rules.building(b.kind).inputs.len();
            if b.player == player && n > 0 && b.inventory.is_none() && b.is_ready() && gf >= b.order_at {
                b.order_input = ((b.order_input as usize + 1) % n) as u8;
                b.order_at = gf + ORDER_EVERY;
            }
        }
    }

    fn distribute_wares(&mut self, player: PlayerId) {
        // open requests: per ware, per consumer type (sites count as one type), the buildings
        let mut by_ware: BTreeMap<Ware, BTreeMap<BuildingType, (u8, Vec<(u32, BuildingId, u8, u8)>)>> = BTreeMap::new();
        for (id, b) in self.buildings.iter() {
            if b.player != player {
                continue;
            }
            for (w, k, from_wh) in self.needs(b) {
                let weight = self.weight(player, w, b);
                if weight > 0 {
                    let key = if b.site().is_some() { BuildingType::Headquarters } else { b.kind };
                    let e = by_ware.entry(w).or_default().entry(key).or_insert((weight, Vec::new()));
                    // (the priority's bit sorts a building before every other)
                    e.1.push((b.placed ^ PRIORITY_BIT, id, k, from_wh));
                }
            }
        }
        // harbors ask their warehouses only for what the last pass could not ship over: the
        // overseas requests below set it again
        self.sea_asked.clear();
        for (_, b) in self.buildings.iter_mut().filter(|(_, b)| b.player == player && b.kind == BuildingType::Harbor) {
            b.sea_demand.iter_mut().for_each(|d| *d = 0);
        }
        if by_ware.is_empty() {
            return;
        }
        let mut stock: Vec<(BuildingId, FlagId, Inventory)> = self
            .buildings
            .iter()
            .filter(|(_, b)| b.player == player && b.is_ready())
            .filter_map(|(id, b)| b.inventory.clone().map(|inv| (id, b.flag, inv)))
            .collect();
        let mut free: Vec<(WareId, FlagId, Ware)> = self
            .wares
            .iter()
            .filter(|(_, w)| w.player == player)
            .filter_map(|(id, w)| match w.loc {
                WareLoc::Flag(f)
                    if w.goal.is_none_or(|g| self.buildings.get(g).is_none_or(|b| b.inventory.is_some())) =>
                {
                    Some((id, f, w.kind))
                }
                _ => None,
            })
            .collect();
        let rot = (self.gf / DISTRIBUTE_EVERY) as usize;
        let mut routes: BTreeMap<FlagId, Vec<(u32, Option<Direction>)>> = BTreeMap::new();
        for (ware, mut types) in by_ware {
            // the player's priorities first, whatever their type, in placement order (ours,
            // not RTTR's: owner's playtest 2026-10-27)
            let mut first: Vec<(u32, BuildingId, u8, u8)> =
                types.values_mut().flat_map(|(_, v)| v.extract_if(.., |(key, ..)| *key & PRIORITY_BIT == 0)).collect();
            first.sort_by_key(|(placed, id, _, _)| (*placed, id.index()));
            for (_, b, count, from_wh) in first {
                self.serve_first(b, ware, (count, from_wh), &mut stock, &mut free, &mut routes);
            }
            types.retain(|_, (_, v)| !v.is_empty());
            // queues per type: sites in placement order (the first placed is finished first),
            // other buildings take turns
            let mut queues: Vec<(BuildingType, u8, std::collections::VecDeque<(BuildingId, u8, u8)>)> = types
                .into_iter()
                .map(|(t, (wgt, mut v))| {
                    v.sort_by_key(|(placed, id, _, _)| (*placed, id.index()));
                    if t != BuildingType::Headquarters && !v.is_empty() {
                        let n = v.len();
                        v.rotate_left(rot % n);
                    }
                    (t, wgt, v.into_iter().map(|(_, id, k, wh)| (id, k, wh)).collect())
                })
                .collect();
            // RTTR shares a ware among the consumer types by the distribution weights: a smooth
            // weighted round robin whose state carries over from pass to pass
            loop {
                let supply = stock.iter().any(|(_, _, inv)| inv.ware(ware) > 0) || free.iter().any(|x| x.2 == ware);
                let active: Vec<usize> = (0..queues.len()).filter(|i| !queues[*i].2.is_empty()).collect();
                if active.is_empty() {
                    break;
                }
                let total: i32 = active.iter().map(|i| queues[*i].1 as i32).sum();
                let rr = &mut self.players[player as usize].distribution_rr;
                let mut pick = active[0];
                let mut best = i32::MIN;
                for &i in &active {
                    let c = rr.entry((ware, queues[i].0)).or_insert(0);
                    *c += queues[i].1 as i32;
                    if *c > best {
                        best = *c;
                        pick = i;
                    }
                }
                let (b, count, from_wh) = queues[pick].2.pop_front().expect("active");
                let served = if supply {
                    self.serve_one(b, ware, &mut stock, &mut free, &mut routes, from_wh > 0)
                } else {
                    None
                };
                let rr = &mut self.players[player as usize].distribution_rr;
                if let Some(by_wh) = served {
                    *rr.get_mut(&(ware, queues[pick].0)).expect("entry") -= total;
                    if count > 1 {
                        let left = (b, count - 1, from_wh - u8::from(by_wh));
                        if queues[pick].0 == BuildingType::Headquarters {
                            queues[pick].2.push_front(left);
                        } else {
                            queues[pick].2.push_back(left);
                        }
                    }
                } else if from_wh == 0 {
                    // only fresh wares would do, and none reaches it: no harm, no ship
                    for &i in &active {
                        *rr.get_mut(&(ware, queues[i].0)).expect("entry") -= queues[i].1 as i32;
                    }
                } else {
                    // nobody could be served: undo this round, and try by ship
                    for &i in &active {
                        *rr.get_mut(&(ware, queues[i].0)).expect("entry") -= queues[i].1 as i32;
                    }
                    if let Some(bflag) = self.buildings.get(b).map(|x| x.flag) {
                        self.overseas_supply(player, bflag, crate::seafaring::Cargo::Ware(ware), count);
                    }
                }
            }
        }
    }

    /// A building with the player's priority: up to `count` of `ware`, of which up to `from_wh`
    /// may come from warehouses (a workshop orders one input at a time), before anyone else.
    fn serve_first(
        &mut self,
        b: BuildingId,
        ware: Ware,
        (count, mut from_wh): (u8, u8),
        stock: &mut [(BuildingId, FlagId, Inventory)],
        free: &mut Vec<(WareId, FlagId, Ware)>,
        routes: &mut BTreeMap<FlagId, Vec<(u32, Option<Direction>)>>,
    ) {
        for i in 0..count {
            match self.serve_one(b, ware, stock, free, routes, from_wh > 0) {
                Some(by_wh) => from_wh -= u8::from(by_wh),
                None => {
                    // none reaches it by road: by ship, as for anyone (a colony's site)
                    if from_wh > 0
                        && let Some(bd) = self.buildings.get(b)
                    {
                        let (player, flag) = (bd.player, bd.flag);
                        self.overseas_supply(player, flag, crate::seafaring::Cargo::Ware(ware), count - i);
                    }
                    break;
                }
            }
        }
    }

    /// Send one `ware` to building `b`: the nearest free ware at a flag, or (if `from_wh`) from
    /// the nearest warehouse that has it. `Some(true)` when it came from a warehouse; `None` if
    /// none can reach `b` by road.
    fn serve_one(
        &mut self,
        b: BuildingId,
        ware: Ware,
        stock: &mut [(BuildingId, FlagId, Inventory)],
        free: &mut Vec<(WareId, FlagId, Ware)>,
        routes: &mut BTreeMap<FlagId, Vec<(u32, Option<Direction>)>>,
        from_wh: bool,
    ) -> Option<bool> {
        let bflag = self.buildings.get(b).map(|x| x.flag)?;
        let r = &*routes.entry(bflag).or_insert_with(|| self.ware_routes_to(bflag));
        let dist = |f: FlagId| r.get(f.index()).map_or(u32::MAX, |x| x.0);
        let best_free = free
            .iter()
            .enumerate()
            .filter(|(_, (_, _, k))| *k == ware)
            .map(|(i, (_, f, _))| (dist(*f), i))
            .filter(|(d, _)| *d != u32::MAX)
            .min();
        let best_wh = stock
            .iter()
            .enumerate()
            .filter(|(_, (id, _, inv))| from_wh && inv.ware(ware) > 0 && *id != b)
            .map(|(i, (_, f, _))| (dist(*f), i))
            .filter(|(d, _)| *d != u32::MAX)
            .min();
        match (best_free, best_wh) {
            (Some((df, i)), wh) if wh.is_none_or(|(dw, _)| df <= dw) => {
                let (wid, f, _) = free.remove(i);
                if let Some(old) = self.wares[wid].goal {
                    self.unreserve(old, ware);
                }
                self.wares[wid].goal = Some(b);
                self.reserve(b, ware);
                self.route_ware(wid, f);
                Some(false)
            }
            (_, Some((_, i))) => {
                let wh = stock[i].0;
                stock[i].2.wares[ware.index()] -= 1;
                // (the snapshot can be stale: the warehouse itself decides)
                self.take_from_warehouse(wh, ware, b).then_some(true)
            }
            _ => None,
        }
    }

    fn reserve(&mut self, b: BuildingId, ware: Ware) {
        let rules = self.rules.clone();
        let bd = &mut self.buildings[b];
        match &bd.state {
            BState::Site(_) => {
                if ware == Ware::Boards {
                    bd.site_incoming.0 += 1;
                } else {
                    bd.site_incoming.1 += 1;
                }
            }
            BState::Ready => {
                if let Some(i) = rules.building(bd.kind).inputs.iter().position(|x| *x == ware) {
                    bd.incoming[i] += 1;
                }
            }
        }
    }

    /// Queue a ware to leave a warehouse towards `goal`; false (and nothing done) if the
    /// warehouse has none (a stale snapshot of its stock must not make wares out of nothing).
    fn take_from_warehouse(&mut self, wh: BuildingId, ware: Ware, goal: BuildingId) -> bool {
        let player = self.buildings[wh].player;
        match self.buildings[wh].inventory.as_mut() {
            Some(inv) if inv.wares[ware.index()] > 0 => inv.wares[ware.index()] -= 1,
            _ => return false,
        }
        let w = self.wares.insert(WareItem {
            kind: ware,
            player,
            loc: WareLoc::Building(wh),
            goal: Some(goal),
            next: None,
        });
        self.reserve(goal, ware);
        self.buildings[wh].out_queue.push(w);
        self.start_building_timer(wh, 1);
        true
    }

    pub(crate) fn start_building_timer(&mut self, b: BuildingId, delay: u32) {
        let due = self.gf + delay.max(1);
        if let Some(key) = self.buildings[b].timer {
            if key.0 <= due {
                return;
            }
            self.sched.cancel(key);
        }
        let key = self.schedule(delay, crate::Ev::Building(b));
        self.buildings[b].timer = Some(key);
    }

    /// Timers: warehouse queues, recruiting, helpers; military coin use.
    pub(crate) fn building_event(&mut self, b: BuildingId) {
        let Some(bd) = self.buildings.get_mut(b) else {
            return;
        };
        bd.timer = None;
        if bd.inventory.is_some() {
            if bd.kind == BuildingType::Harbor {
                self.harbor_tick(b);
            }
            self.warehouse_tick(b);
        } else if self.rules.building(self.buildings[b].kind).is_military() {
            self.military_building_tick(b);
        }
    }

    fn warehouse_tick(&mut self, b: BuildingId) {
        let flag = self.buildings[b].flag;
        let mut again = false;
        // RTTR nobBaseWarehouse: one person or (if none waits) one ware leaves per 20 + rand(10) GF
        if let Some(&f) = self.buildings[b].fig_queue.first() {
            self.buildings[b].fig_queue.remove(0);
            self.figure_leave_building(f);
            again = true;
        } else if let Some(&w) = self.buildings[b].out_queue.first() {
            if self.flag_has_room(flag) {
                self.buildings[b].out_queue.remove(0);
                self.put_ware_at_flag(w, flag);
            }
            again = true;
        }
        if !self.buildings[b].fig_queue.is_empty() || !self.buildings[b].out_queue.is_empty() {
            again = true;
        }
        self.stock_policy_tick(b);
        // helpers appear, soldiers get recruited
        let rules = self.rules.clone();
        let gf = self.gf;
        let mut rng = self.rng_building(b, Purpose::Production);
        let bd = &mut self.buildings[b];
        let inv = bd.inventory.as_mut().expect("warehouse");
        // every warehouse breeds helpers while it has fewer than 100 (RTTR nobBaseWarehouse)
        if gf >= bd.next_helper {
            bd.next_helper = gf + rules.economy.helper_spawn + rng.below(rules.economy.helper_spawn_rand + 1);
            if inv.job(Job::Helper) < rules.economy.helper_max {
                inv.jobs[Job::Helper.index()] += 1;
            }
        }
        // recruiting in batches: min(swords, shields, beer, helpers) * rate / 10, rounded at random
        let ratio = self.players[bd.player as usize].settings.military[0] as u32;
        // RTTR AreRecruitingConditionsComply: keep max(1, 100 - 10 x setting) helpers
        let reserve = (100u32.saturating_sub(10 * ratio)).max(1);
        if gf >= bd.next_recruit {
            bd.next_recruit = gf + rules.military.recruit_wait + rng.below(rules.military.recruit_rand + 1);
            let max = if inv.job(Job::Helper) < reserve {
                0
            } else {
                inv.job(Job::Helper).min(inv.ware(Ware::Sword)).min(inv.ware(Ware::Shield)).min(inv.ware(Ware::Beer))
            };
            let mut n = max * ratio / 10;
            if (max * ratio) % 10 != 0 && rng.below(10) < (max * ratio) % 10 {
                n += 1;
            }
            let n = n.min(max);
            inv.jobs[Job::Helper.index()] -= n;
            inv.wares[Ware::Sword.index()] -= n;
            inv.wares[Ware::Shield.index()] -= n;
            inv.wares[Ware::Beer.index()] -= n;
            inv.jobs[Job::Private.index()] += n;
        }
        let delay = if again {
            WAREHOUSE_OUT_GF
                + self.rng(1 << 60 | b.index() as u64 ^ 0x1eaf, Purpose::Production).below(WAREHOUSE_OUT_RAND + 1)
        } else {
            50
        };
        self.start_building_timer(b, delay);
    }

    /// Give buildings their workers, roads their carriers.
    fn assign_jobs(&mut self, player: PlayerId) {
        // carriers
        let roads: Vec<_> = self
            .roads
            .iter()
            .filter(|(_, r)| r.player == player && r.carrier.is_none() && !r.carrier_wanted)
            .map(|(id, r)| (id, r.from))
            .collect();
        for (r, f) in roads {
            if self.roads[r].kind == RoadKind::Boat {
                // a boat carrier: a helper takes a boat along (RTTR boat roads)
                let routes = self.routes_to(f);
                let wh = self
                    .buildings
                    .iter()
                    .filter(|(_, b)| b.player == player && b.is_ready())
                    .filter(|(_, b)| {
                        b.inventory.as_ref().is_some_and(|i| i.job(Job::Helper) > 0 && i.ware(Ware::Boat) > 0)
                    })
                    .filter(|(_, b)| routes.get(b.flag.index()).is_some_and(|x| x.0 != u32::MAX))
                    .min_by_key(|(id, b)| (routes[b.flag.index()].0, id.index()))
                    .map(|(id, _)| id);
                if let Some(wh) = wh {
                    self.buildings[wh].inventory.as_mut().expect("stock").wares[Ware::Boat.index()] -= 1;
                    self.roads[r].carrier_wanted = true;
                    let c = self.spawn_from_warehouse(wh, Job::Helper, FigKind::Carrier { road: r, pos: 0 });
                    self.figures[c].carry = Some(Ware::Boat);
                }
                continue;
            }
            if let Some(wh) = self.warehouse_with_job(player, f, Job::Helper) {
                self.roads[r].carrier_wanted = true;
                self.spawn_from_warehouse(wh, Job::Helper, FigKind::Carrier { road: r, pos: 0 });
            }
        }
        // pack donkeys for donkey roads
        let donkey_roads: Vec<_> = self
            .roads
            .iter()
            .filter(|(_, r)| r.player == player && r.kind == RoadKind::Donkey && r.donkey.is_none() && !r.donkey_wanted)
            .map(|(id, r)| (id, r.from))
            .collect();
        for (r, f) in donkey_roads {
            if let Some(wh) = self.warehouse_with_job(player, f, Job::PackDonkey) {
                self.roads[r].donkey_wanted = true;
                self.spawn_from_warehouse(wh, Job::PackDonkey, FigKind::Carrier { road: r, pos: 0 });
            }
        }
        // workers, builders, planers, soldiers
        let out = self.soldiers_on_mission(player);
        // the border situation doesn't change during this pass: work it out once
        let posts = self.frontier_posts();
        // without a harbor no soldier comes from overseas (overseas_supply would say no)
        let has_harbor =
            self.buildings.iter().any(|(_, h)| h.player == player && h.kind == BuildingType::Harbor && h.is_ready());
        let mut ids = self.buildings.ids();
        // the player's priorities get their people first (a stable sort: the rest as before)
        ids.sort_by_key(|b| !self.buildings[*b].priority());
        for b in ids {
            let bd = &self.buildings[b];
            if bd.player != player {
                continue;
            }
            let (flag, state, worker, coming) = (bd.flag, bd.state.clone(), bd.worker, bd.worker_coming);
            // soldiers out attacking or defending still belong to the building (RTTR troops_on_mission)
            let have = bd.soldiers.len() + bd.soldiers_coming as usize + out.get(&b).copied().unwrap_or(0);
            let def = self.rules.building(bd.kind);
            let (military, worker_job) = (def.is_military(), def.worker);
            match state {
                BState::Site(s) => {
                    if s.planing && s.planer.is_none() && !coming {
                        if let Some(wh) = self.warehouse_with_job(player, flag, Job::Planer) {
                            self.buildings[b].worker_coming = true;
                            self.spawn_from_warehouse(wh, Job::Planer, FigKind::Planer { site: b });
                        }
                    } else if !s.planing
                        && s.builder.is_none()
                        && !coming
                        && let Some(wh) = self.warehouse_with_job(player, flag, Job::Builder)
                    {
                        self.buildings[b].worker_coming = true;
                        self.spawn_from_warehouse(wh, Job::Builder, FigKind::Builder { site: b });
                    }
                }
                BState::Ready => {
                    if let Some(job) = worker_job
                        && worker.is_none()
                        && !coming
                        && let Some(wh) = self.warehouse_with_job(player, flag, job)
                    {
                        self.buildings[b].worker_coming = true;
                        self.spawn_from_warehouse(wh, job, FigKind::Worker { building: b });
                    }
                    // (how many it wants only matters for a manned building that may want more)
                    let want = if military && have < self.buildings[b].garrison_wanted as usize {
                        (self.buildings[b].garrison_wanted as usize).min(self.required_troops_among(b, &posts))
                    } else {
                        0
                    };
                    if military && have < want {
                        if let Some((wh, job)) = self.warehouse_with_soldier(player, flag) {
                            self.buildings[b].soldiers_coming += 1;
                            self.spawn_from_warehouse(
                                wh,
                                job,
                                FigKind::Soldier(crate::military::SoldierTask::ToBuilding(b)),
                            );
                        } else if has_harbor && let Some(route) = self.overseas_route(player, flag) {
                            // none on this coast: a harbor across the sea may ship one over
                            for job in Job::SOLDIERS.iter().rev() {
                                let cargo = crate::seafaring::Cargo::Person(*job);
                                if self.overseas_supply_by(player, &route, cargo, (want - have) as u8) {
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Nearest connected warehouse that has `job` (or a helper plus the tool to make one).
    fn warehouse_with_job(&mut self, player: PlayerId, to: FlagId, job: Job) -> Option<BuildingId> {
        let routes = self.routes_to(to);
        let mut best: Option<(u32, BuildingId)> = None;
        for (id, b) in self.buildings.iter() {
            if b.player != player || !b.is_ready() {
                continue;
            }
            let Some(inv) = &b.inventory else { continue };
            if !inv.can_supply(job, &self.rules) {
                continue;
            }
            let d = routes.get(b.flag.index()).map_or(u32::MAX, |r| r.0);
            if d != u32::MAX && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, id));
            }
        }
        let Some((_, wh)) = best else {
            // nobody here: perhaps a harbor across the sea can send one
            if job != Job::PackDonkey {
                self.overseas_supply(player, to, crate::seafaring::Cargo::Person(job), 1);
            }
            return None;
        };
        // the person leaves through `spawn_from_warehouse`, which counts the job off
        let rules = self.rules.clone();
        let inv = self.buildings[wh].inventory.as_mut().expect("warehouse");
        if inv.job(job) == 0 && inv.take_person(job, &rules) {
            inv.jobs[job.index()] += 1;
        }
        Some(wh)
    }

    fn warehouse_with_soldier(&self, player: PlayerId, to: FlagId) -> Option<(BuildingId, Job)> {
        let routes = self.routes_to(to);
        let mut best: Option<(u32, BuildingId, Job)> = None;
        for (id, b) in self.buildings.iter() {
            if b.player != player || !b.is_ready() {
                continue;
            }
            let Some(inv) = &b.inventory else { continue };
            // the military setting decides: strong (high rank) or weak soldiers first, strong from
            // half the scale up (RTTR `>= MILITARY_SETTINGS_SCALE[1] / 2`)
            let strong_first = self.players[player as usize].settings.military[1] >= crate::MILITARY_SCALE[1] / 2;
            let ranks: Vec<Job> =
                if strong_first { Job::SOLDIERS.iter().rev().copied().collect() } else { Job::SOLDIERS.to_vec() };
            // soldiers beyond the reserve of their rank may go
            let spare = |j: &Job| inv.job(*j) > j.soldier_rank().map_or(0, |r| b.reserve[r as usize] as u32);
            let Some(job) = ranks.into_iter().find(spare) else {
                continue;
            };
            let d = routes.get(b.flag.index()).map_or(u32::MAX, |r| r.0);
            if d != u32::MAX && best.is_none_or(|(bd, _, _)| d < bd) {
                best = Some((d, id, job));
            }
        }
        best.map(|(_, id, j)| (id, j))
    }

    pub(crate) fn record_stats(&mut self) {
        let by_player = self.land_by_player();
        for p in 0..self.players.len() {
            let me = p as u8;
            let land = by_player[p];
            let blds = self.buildings.iter().filter(|(_, b)| b.player == me).count() as u32;
            let inv = self.player_inventory(me);
            let out: Vec<&crate::Figure> = self.figures.iter().map(|(_, f)| f).filter(|f| f.player == me).collect();
            let inhabitants = inv.jobs.iter().sum::<u32>() + out.len() as u32;
            let on_way = self.wares.iter().filter(|(_, w)| w.player == me).count() as u32;
            let merchandise = inv.wares.iter().sum::<u32>() + on_way;
            let rank_weight = |j: Job| j.soldier_rank().map_or(0, |r| r as u32 + 1);
            let military = Job::SOLDIERS.iter().map(|j| inv.job(*j) * rank_weight(*j)).sum::<u32>()
                + out.iter().map(|f| rank_weight(f.job)).sum::<u32>();
            let prod: Vec<u32> = self
                .buildings
                .iter()
                .filter(|(_, b)| b.player == me && b.is_ready() && b.attempts > 0)
                .map(|(_, b)| b.productivity())
                .collect();
            let productivity = if prod.is_empty() { 0 } else { prod.iter().sum::<u32>() / prod.len() as u32 };
            let st = &self.players[p].stats;
            let gold = st.produced_by_ware[Ware::Coins.index()];
            let sample = Sample {
                gf: self.gf,
                land,
                buildings: blds,
                inhabitants,
                merchandise,
                military,
                gold,
                productivity,
                vanquished: st.vanquished,
                produced: st.produced,
                by_ware: st.produced_by_ware,
            };
            self.players[p].stats.samples.push(sample);
        }
    }

    // ------------------------------------------------------------------ construction

    /// The builder finished a step; returns true when the building is complete.
    pub(crate) fn site_build_step(&mut self, b: BuildingId) -> bool {
        let cost = self.rules.building(self.buildings[b].kind).cost();
        let BState::Site(s) = &mut self.buildings[b].state else {
            return false;
        };
        s.progress += 1;
        s.progress >= Site::total_steps(cost)
    }

    /// Take the next material the builder needs; boards first, then stones.
    pub(crate) fn site_take_material(&mut self, b: BuildingId) -> bool {
        let cost = self.rules.building(self.buildings[b].kind).cost();
        let BState::Site(s) = &mut self.buildings[b].state else {
            return false;
        };
        let boards_phase = s.progress < cost.0 as u16 * STEPS_PER_MATERIAL || cost.1 == 0;
        if boards_phase {
            if s.boards > 0 {
                s.boards -= 1;
                s.used_boards += 1;
                return true;
            }
        } else if s.stones > 0 {
            s.stones -= 1;
            s.used_stones += 1;
            return true;
        }
        false
    }

    pub(crate) fn finish_building(&mut self, b: BuildingId) {
        let bd = &mut self.buildings[b];
        let colony = matches!(&bd.state, BState::Site(s) if s.from_sea);
        bd.state = BState::Ready;
        bd.worker_coming = false;
        let (player, pos, kind) = (bd.player, bd.pos, bd.kind);
        if kind.is_warehouse() {
            let mut inv = Inventory::default();
            // a colony's harbor: the starter kit the expedition brought (seafaring COLONY_KIT)
            if colony {
                inv.wares[Ware::Boards.index()] += crate::seafaring::COLONY_KIT.0 as u32;
                inv.wares[Ware::Stones.index()] += crate::seafaring::COLONY_KIT.1 as u32;
            }
            bd.inventory = Some(inv);
            self.start_building_timer(b, 1);
        }
        if self.rules.building(kind).military.is_some() {
            // HQ-like buildings (harbors) claim land as soon as they stand
            self.territory_dirty = true;
        }
        // (news: the owner's audio plays its jingle, others in view a quiet "done")
        self.events.push(Event::BuildingFinished { player, at: pos, kind });
        self.players[player as usize].road_epoch += 1;
        self.route_cache.borrow_mut().clear();
    }

    pub fn site_progress_percent(&self, b: BuildingId) -> u32 {
        let bd = &self.buildings[b];
        match &bd.state {
            BState::Ready => 100,
            BState::Site(s) => {
                let total = Site::total_steps(self.rules.building(bd.kind).cost()).max(1) as u32;
                s.progress as u32 * 100 / total
            }
        }
    }
}

/// The buildings RTTR counts as military for spacing and attacks (not lookout towers or catapults).
pub fn is_frontier_military(kind: BuildingType) -> bool {
    matches!(
        kind,
        BuildingType::Barracks | BuildingType::Guardhouse | BuildingType::Watchtower | BuildingType::Fortress
    )
}
