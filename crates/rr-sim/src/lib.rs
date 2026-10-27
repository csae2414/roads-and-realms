//! The deterministic game simulation. One `step` is exactly one game frame (GF).
//!
//! All state lives in `Sim`. Behavior is split over modules as `impl Sim` blocks:
//! `transport` (flags, roads, routing, carriers), `economy` (buildings, wares, distribution),
//! `figures` (walking and jobs), `territory`, `military`.
#![cfg_attr(not(test), deny(clippy::float_arithmetic))]
// (no floats in the simulation: peers on other systems must compute alike; net round 4, #8)

mod audit;
mod economy;
mod figures;
mod fog;
mod military;
mod nature;
mod replay;
mod sched;
mod seafaring;
mod territory;
mod transport;

pub use economy::{BState, Building, Inventory, Sample, Site, WareItem, WareLoc, is_frontier_military};
pub use figures::{FigKind, Figure, Motion, Step};
pub use fog::Visibility;
pub use military::{CATAPULT_RANGE, SoldierTask};
pub use replay::Replay;
pub use sched::Scheduler;
pub use seafaring::{COLONY_KIT, Cargo, Expedition, SEA_ATTACK_DISTANCE, SHIP_CAPACITY, SeaAttackPlan, Ship, ShipTask};
pub use transport::{Flag, Road, RoadTrouble};

use rr_core::{Arena, Direction, Gf, Id, MapCoord, MapData, StableHash, StableHasher};
use rr_rules::{BuildingType, Rules, Ware};
use rr_world::World;
use std::sync::Arc;

pub type FlagId = Id<Flag>;
pub type RoadId = Id<Road>;
pub type BuildingId = Id<Building>;
pub type FigId = Id<Figure>;
pub type WareId = Id<WareItem>;
pub type ShipId = Id<Ship>;
pub type PlayerId = u8;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Nation {
    #[default]
    Romans,
    Vikings,
    Nubians,
    Japanese,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct PlayerSetup {
    pub name: String,
    pub nation: Nation,
    /// Players with the same team are allies.
    pub team: Option<u8>,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct GameSetup {
    pub seed: u64,
    pub players: Vec<PlayerSetup>,
    /// What every headquarters starts with (the original's game setup).
    pub start_goods: StartGoods,
}

impl GameSetup {
    /// A campaign mission: player 0 against all the others, who are allied (one place for this
    /// rule, used by the game, the command line and the tests; architect round 2, #1).
    pub fn campaign(seed: u64, players: usize) -> Self {
        GameSetup {
            seed,
            players: (0..players).map(|i| PlayerSetup { team: Some(u8::from(i > 0)), ..Default::default() }).collect(),
            ..Default::default()
        }
    }
}

/// The original's warehouse settings, per ware: what a warehouse does with it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum StockPolicy {
    #[default]
    Normal,
    /// Bring this ware here from the other warehouses.
    Collect,
    /// Take no more of it in.
    Stop,
    /// Send what is here to the other warehouses.
    MoveOut,
}

/// The original's starting conditions; the amounts are RTTR's `nobHQ::getStartInventory`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum StartGoods {
    /// Half of everything, no beer and no weapons, 26 privates.
    Low,
    #[default]
    Normal,
    /// Twice the normal amounts.
    High,
}

#[derive(Debug)]
pub enum SetupError {
    NoHq(usize),
    HqNotBuildable(usize),
    /// A replay recorded with another version or other rules would play differently.
    ReplayMismatch,
    /// The map itself is broken (a damaged file or replay).
    BadMap(String),
}

impl std::fmt::Display for SetupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SetupError::NoHq(p) => write!(f, "player {p} has no HQ position on this map"),
            SetupError::HqNotBuildable(p) => write!(f, "HQ of player {p} is not on buildable land"),
            SetupError::ReplayMismatch => write!(f, "the replay was recorded with another version or other rules"),
            SetupError::BadMap(why) => write!(f, "{why}"),
        }
    }
}
impl std::error::Error for SetupError {}

/// The longest road a command may lay in one go (the original's roads are far shorter).
pub const MAX_ROAD_STEPS: usize = 1024;

/// Maximum of each military slider (RTTR `MILITARY_SETTINGS_SCALE`).
pub const MILITARY_SCALE: [u8; 8] = [10, 5, 5, 5, 8, 8, 8, 8];

/// Per-player settings changed by commands (the original's settings windows).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PlayerSettings {
    /// Distribution weights 0..=10 per (ware, consumer building).
    pub distribution: Vec<(Ware, BuildingType, u8)>,
    /// Transport priority: earlier wares are carried first.
    pub transport_order: Vec<Ware>,
    /// Military: soldiers to send for attacks etc. (0..=10 sliders).
    /// RTTR order: recruits, defense strength, defenders, attackers, troops far / middle / harbor / near.
    pub military: [u8; 8],
    /// Tool production priority 0..=10 for the metalworks.
    pub tools: Vec<(Ware, u8)>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Player {
    pub setup: PlayerSetup,
    pub alive: bool,
    pub hq: Option<BuildingId>,
    pub settings: PlayerSettings,
    /// Increments on every flag/road change; invalidates routes.
    pub road_epoch: u32,
    pub stats: economy::Stats,
    /// Weighted round-robin state of the distribution, per (ware, consumer type).
    pub distribution_rr: std::collections::BTreeMap<(Ware, BuildingType), i32>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Cmd {
    PlaceFlag {
        at: MapCoord,
    },
    RemoveFlag {
        at: MapCoord,
    },
    /// Build a road from the flag at `from` following `dirs`. If the end has no flag, one is placed.
    BuildRoad {
        from: MapCoord,
        dirs: Vec<Direction>,
    },
    RemoveRoad {
        flag: MapCoord,
        dir: Direction,
    },
    PlaceBuilding {
        at: MapCoord,
        kind: BuildingType,
    },
    RemoveBuilding {
        at: MapCoord,
    },
    ToggleProduction {
        at: MapCoord,
    },
    SendGeologist {
        flag: MapCoord,
    },
    SendScout {
        flag: MapCoord,
    },
    Attack {
        target: MapCoord,
        soldiers: u8,
        /// Send the strongest soldiers (else the weakest), as in RTTR's attack window.
        strong: bool,
    },
    SetDistribution {
        ware: Ware,
        building: BuildingType,
        weight: u8,
    },
    SetTransportOrder {
        order: Vec<Ware>,
    },
    SetMilitary {
        values: [u8; 8],
    },
    SetTools {
        values: Vec<(Ware, u8)>,
    },
    /// A warehouse's setting for one ware.
    SetStockPolicy {
        at: MapCoord,
        ware: Ware,
        policy: StockPolicy,
    },
    /// A warehouse's setting for one kind of settler (soldiers too).
    SetPeoplePolicy {
        at: MapCoord,
        job: rr_rules::Job,
        policy: StockPolicy,
    },
    /// Ask a military building to send one soldier home / request more.
    SetGarrisonWanted {
        at: MapCoord,
        wanted: u8,
    },
    UpgradeRoad {
        flag: MapCoord,
        dir: Direction,
    },
    /// Prepare an expedition in the harbor at `harbor`.
    StartExpedition {
        harbor: MapCoord,
    },
    /// Send the ready expedition of `harbor` to the harbor spot `to`.
    SendExpedition {
        harbor: MapCoord,
        to: MapCoord,
    },
    Surrender,
    /// A shipyard builds boats (for boat roads) or, switched over, sea ships.
    SetShipyardMode {
        at: MapCoord,
        ships: bool,
    },
    /// Soldiers of a rank (0 private … 4 general) a warehouse keeps at home for its defense.
    SetReserve {
        at: MapCoord,
        rank: u8,
        count: u8,
    },
    /// A military building sends its strongest soldier home and keeps the smaller garrison
    /// (RTTR MilUpgradeOptim: promoted soldiers leave the inland for the fronts).
    SendHome {
        at: MapCoord,
    },
    /// The player's priority for a building or site (ours, not RTTR's): it is supplied with
    /// building materials and inputs before the others, and gets its builder, worker and
    /// soldiers first.
    SetPriority {
        at: MapCoord,
        on: bool,
    },
    /// A ship from the harbor at `harbor` explores the coasts of its sea (RTTR exploration
    /// expedition): up to three unexplored harbor spots, seeing far, then home.
    StartExploration {
        harbor: MapCoord,
    },
    /// A story beat: the player's military building at `at` goes over to player `to`, its
    /// soldiers walking home (the Roman campaign's Tarpeia; ours, not RTTR's). Only the owner's
    /// own command can give a building away.
    HandOver {
        at: MapCoord,
        to: PlayerId,
    },
    /// An offering at the holy place at `at` (the Roman campaign's gods): `n` of `ware` leave the
    /// player's warehouses, only while the player holds the place (`World::holy_holder`).
    Offer {
        at: MapCoord,
        ware: Ware,
        n: u32,
    },
    /// A god's answer (a story's deed): `n` of the player's privates in its warehouses become
    /// privates first class, or (`recruit`) `n` new privates come to its castle; only while the
    /// player holds the holy place at `at`.
    Bless {
        at: MapCoord,
        n: u32,
        recruit: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PlayerCommand {
    pub player: PlayerId,
    pub cmd: Cmd,
}

impl Cmd {
    /// Every map node the command refers to.
    pub fn coords(&self) -> Vec<MapCoord> {
        match self {
            Cmd::PlaceFlag { at }
            | Cmd::RemoveFlag { at }
            | Cmd::PlaceBuilding { at, .. }
            | Cmd::RemoveBuilding { at }
            | Cmd::ToggleProduction { at }
            | Cmd::SetGarrisonWanted { at, .. }
            | Cmd::SetStockPolicy { at, .. }
            | Cmd::SetPeoplePolicy { at, .. }
            | Cmd::SetShipyardMode { at, .. }
            | Cmd::SetReserve { at, .. }
            | Cmd::SendHome { at }
            | Cmd::SetPriority { at, .. }
            | Cmd::HandOver { at, .. }
            | Cmd::Offer { at, .. }
            | Cmd::Bless { at, .. } => vec![*at],
            Cmd::BuildRoad { from, .. } => vec![*from],
            Cmd::RemoveRoad { flag, .. } | Cmd::UpgradeRoad { flag, .. } => vec![*flag],
            Cmd::SendGeologist { flag } | Cmd::SendScout { flag } => vec![*flag],
            Cmd::Attack { target, .. } => vec![*target],
            Cmd::StartExpedition { harbor } | Cmd::StartExploration { harbor } => vec![*harbor],
            Cmd::SendExpedition { harbor, to } => vec![*harbor, *to],
            Cmd::SetDistribution { .. }
            | Cmd::SetTransportOrder { .. }
            | Cmd::SetMilitary { .. }
            | Cmd::SetTools { .. }
            | Cmd::Surrender => vec![],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RejectReason {
    NotYourTerritory,
    NotBuildable,
    NoFlag,
    BadRoad,
    NotFound,
    NotAllowed,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Event {
    CommandRejected {
        player: PlayerId,
        cmd: Cmd,
        reason: RejectReason,
    },
    /// A building, flag or road was placed as asked.
    Placed {
        player: PlayerId,
    },
    /// The owner tore a building or site down (not a loss: no lament).
    BuildingRemoved {
        player: PlayerId,
        at: MapCoord,
        kind: BuildingType,
    },
    BuildingFinished {
        player: PlayerId,
        at: MapCoord,
        kind: BuildingType,
    },
    BuildingLost {
        player: PlayerId,
        at: MapCoord,
        kind: BuildingType,
    },
    UnderAttack {
        player: PlayerId,
        at: MapCoord,
        /// The attacker.
        by: PlayerId,
        /// How many soldiers march on it.
        soldiers: u8,
    },
    MineExhausted {
        player: PlayerId,
        at: MapCoord,
    },
    /// A mine's ore runs low: it still works, ever less often (it never stops quite).
    MineRunningLow {
        player: PlayerId,
        at: MapCoord,
    },
    NoResources {
        player: PlayerId,
        at: MapCoord,
    },
    GeologistFound {
        player: PlayerId,
        at: MapCoord,
        what: rr_core::Resource,
    },
    BuildingCaptured {
        player: PlayerId,
        at: MapCoord,
        by: PlayerId,
    },
    /// Land of `player` became `to`'s (a new or captured post of theirs is nearer): how many
    /// nodes, and one of them (RTTR: "Lost land by this building").
    LandLost {
        player: PlayerId,
        to: PlayerId,
        nodes: u32,
        at: MapCoord,
    },
    /// A warehouse or castle taken by an attack burns instead of changing hands: whose, and by
    /// whom (a mission's "take the castle" goal counts only the player's side).
    Razed {
        player: PlayerId,
        at: MapCoord,
        kind: BuildingType,
        by: PlayerId,
    },
    PlayerDefeated {
        player: PlayerId,
    },
    /// A visual/sound cue for the frontend.
    Sound {
        at: MapCoord,
        sound: SoundCue,
    },
    /// A woodcutter, quarry, fisher, hunter... found nothing to work in reach for a while.
    NothingInReach {
        player: PlayerId,
        at: MapCoord,
        kind: BuildingType,
    },
    /// A catapult at `from` throws a stone at the building at `to`; `hit` if it strikes (the
    /// stone is decided as it leaves; the app draws its flight).
    CatapultThrow {
        from: MapCoord,
        to: MapCoord,
        hit: bool,
    },
    /// A catapult of `by` hit a military building of `player`.
    CatapultHit {
        player: PlayerId,
        at: MapCoord,
        by: PlayerId,
    },
    /// The player offered `n` of `ware` at the holy place at `at` (`Cmd::Offer`).
    Offered {
        player: PlayerId,
        at: MapCoord,
        ware: Ware,
        n: u32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SoundCue {
    TreeFall,
    Chop,
    Hammer,
    Pick,
    Saw,
    Sword,
    Death,
    Build,
    Fanfare,
    Splash,
    /// The hunter's bow.
    Shot,
    /// A farmer's scythe.
    Scythe,
    /// A forester digging in a sapling.
    Dig,
    /// A mill turning.
    Creak,
    /// A baker's oven door.
    Door,
    /// A butcher's knife.
    Knife,
    /// A bucket at the well.
    Bucket,
    /// Hammer on an anvil: the smiths, the minter and the smelter.
    Anvil,
    /// A building on fire.
    Fire,
    /// A ship slides off the shipwright's slip.
    Launch,
    /// A ship comes alongside a harbor.
    Dock,
    /// A catapult throws.
    Catapult,
    /// A catapult's stone lands.
    Thump,
    /// A brewery's kettle.
    Brew,
    /// Pigs in their sty.
    Pig,
    /// A donkey in the breeder's yard.
    Bray,
    /// Soldiers promoted with a coin.
    Promote,
}

#[cfg(test)]
mod cue_tests {
    use rr_rules::Job;

    /// a11y roast #15: the food chain and the farms are heard too.
    #[test]
    fn the_village_is_heard() {
        for (job, outside) in [
            (Job::Farmer, true),
            (Job::Forester, true),
            (Job::Miller, false),
            (Job::Baker, false),
            (Job::Butcher, false),
            (Job::Shipwright, false),
            (Job::Helper, false),
        ] {
            assert!(super::work_cue(job, outside).is_some(), "{job:?}");
        }
        assert_eq!(super::work_cue(Job::Woodcutter, false), None);
    }

    /// Sound designer 2 #6: every trade is heard at its work, inside or outside, unless it is
    /// silent on purpose.
    #[test]
    fn every_trade_has_a_sound() {
        // (soldiers and carriers aren't trades; the scout and geologist walk, the builder's
        // and planer's work is the site's hammer)
        let silent = [
            Job::Builder,
            Job::Planer,
            Job::Geologist,
            Job::Scout,
            Job::PackDonkey,
            Job::Private,
            Job::PrivateFirstClass,
            Job::Sergeant,
            Job::Officer,
            Job::General,
        ];
        for job in Job::ALL.iter().copied() {
            let heard = super::work_cue(job, false).is_some() || super::work_cue(job, true).is_some();
            assert!(heard || silent.contains(&job), "{job:?} works in silence");
        }
        assert!(super::work_cue(Job::Shipwright, true).is_some(), "the shipwright on the hull");
    }
}

/// The sound of a worker's job: outside (at the work spot) or inside the building.
pub fn work_cue(job: rr_rules::Job, outside: bool) -> Option<SoundCue> {
    use rr_rules::Job;
    Some(match (job, outside) {
        (Job::Woodcutter, true) => SoundCue::Chop,
        (Job::Stonemason, true) => SoundCue::Pick,
        (Job::Hunter, true) => SoundCue::Shot,
        (Job::Fisher, true) => SoundCue::Splash,
        (Job::Farmer, true) => SoundCue::Scythe,
        (Job::Forester, true) => SoundCue::Dig,
        (Job::Carpenter, false) => SoundCue::Saw,
        (Job::Miner, false) => SoundCue::Pick,
        (Job::Armorer | Job::Metalworker | Job::Minter | Job::IronFounder, false) => SoundCue::Anvil,
        (Job::Shipwright, _) => SoundCue::Hammer,
        (Job::Brewer, false) => SoundCue::Brew,
        (Job::PigBreeder, false) => SoundCue::Pig,
        (Job::DonkeyBreeder, false) => SoundCue::Bray,
        (Job::Miller, false) => SoundCue::Creak,
        (Job::Baker, false) => SoundCue::Door,
        (Job::Butcher, false) => SoundCue::Knife,
        (Job::Helper, false) => SoundCue::Bucket,
        _ => return None,
    })
}

#[derive(Clone, Debug, Default)]
pub struct StepOutput {
    pub events: Vec<Event>,
}

/// Scheduled things.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum Ev {
    /// A figure finished its current motion or wait.
    Figure(FigId),
    /// A building's timer (production cycle, warehouse output, recruiting).
    Building(BuildingId),
    /// A tree/field grows, etc.
    Nature(MapCoord),
    /// A ship reached the next node.
    Ship(ShipId),
}

/// An attack as the attack window shows it (`Sim::attack_plan`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AttackPlan {
    /// The own posts that can walk to the target.
    pub posts: usize,
    /// The soldiers they may send (`attackers_for`).
    pub available: usize,
    /// The ranks an attack takes, in its order: strong soldiers first, or weak ones.
    pub strong: Vec<u8>,
    pub weak: Vec<u8>,
    /// Nobody can walk there: the attack goes by ship from a harbor.
    pub by_sea: bool,
}

impl AttackPlan {
    /// The ranks an attack of `count` sends.
    pub fn party(&self, count: u8, strong: bool) -> &[u8] {
        let all = if strong { &self.strong } else { &self.weak };
        &all[..all.len().min(count as usize)]
    }
}

/// A fresh territory epoch, never handed out before in this process (never 0).
pub(crate) fn next_epoch() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

pub struct Sim {
    pub(crate) rules: Arc<Rules>,
    pub(crate) gf: u32,
    pub(crate) seed: u64,
    pub(crate) world: World,
    pub(crate) players: Vec<Player>,
    pub(crate) flags: Arena<Flag>,
    pub(crate) roads: Arena<Road>,
    pub(crate) buildings: Arena<Building>,
    pub(crate) figures: Arena<Figure>,
    pub(crate) wares: Arena<WareItem>,
    pub(crate) sched: Scheduler<Ev>,
    /// Road (if any) passing through each node, excluding its end flags.
    pub(crate) road_nodes: Vec<Option<RoadId>>,
    pub(crate) events: Vec<Event>,
    pub(crate) territory_dirty: bool,
    /// Overseas requests made in the current distribution pass, per (harbor, cargo slot);
    /// rebuilt every pass, never saved.
    pub(crate) sea_asked: std::collections::BTreeMap<(BuildingId, usize), u32>,
    /// The sea of each harbor spot asked about (the seas never change; never saved).
    pub(crate) harbor_seas: std::cell::RefCell<std::collections::BTreeMap<MapCoord, Option<u16>>>,
    /// Sea paths searched before, by (from, to); derived, never saved.
    pub(crate) sea_paths: std::cell::RefCell<std::collections::BTreeMap<(MapCoord, MapCoord), Option<Vec<Direction>>>>,
    /// People walking to a harbor to be shipped, per (player, job, harbor), counted once in a
    /// frame and kept up as more are sent (never saved).
    pub(crate) harbor_walkers: Option<(u32, std::collections::BTreeMap<(PlayerId, rr_rules::Job, MapCoord), usize>)>,
    pub(crate) nature: nature::Nature,
    pub(crate) military: military::Fights,
    /// Per player and node: 0 unexplored, 1 fog, 2 visible.
    pub(crate) vis: Vec<Vec<u8>>,
    pub(crate) ships: Arena<Ship>,
    /// Sea id per node (0 = not a ship node).
    pub(crate) sea: Vec<u16>,
    /// Land masses by terrain (walkable nodes connected on foot); derived, never saved.
    pub(crate) land: Vec<u16>,
    /// The nodes each player owns, kept with the territory; derived, never saved (perf round 3,
    /// #1: a land goal walked the whole map every game frame).
    pub(crate) owned: Vec<u32>,
    /// Changes whenever a node changes hands; unique across games (`territory_epoch`).
    pub(crate) territory_epoch: u64,
    /// People's routes to a flag, valid until the road network changes; never saved.
    pub(crate) route_cache: std::cell::RefCell<std::collections::BTreeMap<FlagId, crate::transport::Routes>>,
    /// Working memory of the path search, kept between searches (never saved).
    pub(crate) path_scratch: std::cell::RefCell<figures::PathScratch>,
    /// Working memory of a ware's next-road search, kept between searches (never saved).
    pub(crate) ware_scratch: std::cell::RefCell<transport::WareScratch>,
}

/// Bumped whenever the save layout changes.
pub const SAVE_VERSION: u32 = 17;
/// Raise on any change to the rules of play that doesn't change the save format (network games
/// must run the same simulation everywhere).
pub const SIM_VERSION: u32 = 56;
/// Most soldiers of one rank a warehouse can hold in reserve.
pub const MAX_RESERVE: u8 = 20;
const SAVE_MAGIC: &[u8; 8] = b"RRSAVE\0\0";

#[derive(serde::Serialize, serde::Deserialize)]
struct SaveData {
    version: u32,
    rules_hash: u64,
    gf: u32,
    seed: u64,
    world: World,
    players: Vec<Player>,
    flags: Arena<Flag>,
    roads: Arena<Road>,
    buildings: Arena<Building>,
    figures: Arena<Figure>,
    wares: Arena<WareItem>,
    sched: Scheduler<Ev>,
    road_nodes: Vec<Option<RoadId>>,
    territory_dirty: bool,
    nature: nature::Nature,
    military: military::Fights,
    vis: Vec<Vec<u8>>,
    ships: Arena<Ship>,
    sea: Vec<u16>,
}

/// `SaveData` borrowed: the same bytes (serde writes a reference as what it points to) without
/// copying the whole game first (saves and every network sync check).
#[derive(serde::Serialize)]
struct SaveDataRef<'a> {
    version: u32,
    rules_hash: u64,
    gf: u32,
    seed: u64,
    world: &'a World,
    players: &'a Vec<Player>,
    flags: &'a Arena<Flag>,
    roads: &'a Arena<Road>,
    buildings: &'a Arena<Building>,
    figures: &'a Arena<Figure>,
    wares: &'a Arena<WareItem>,
    sched: &'a Scheduler<Ev>,
    road_nodes: &'a Vec<Option<RoadId>>,
    territory_dirty: bool,
    nature: &'a nature::Nature,
    military: &'a military::Fights,
    vis: &'a Vec<Vec<u8>>,
    ships: &'a Arena<Ship>,
    sea: &'a Vec<u16>,
}

#[derive(Debug)]
pub enum LoadError {
    NotASave,
    Version(u32),
    RulesMismatch,
    Corrupt(String),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::NotASave => write!(f, "not a save game"),
            LoadError::Version(v) => write!(f, "save version {v} is not supported (current {SAVE_VERSION})"),
            LoadError::RulesMismatch => write!(f, "the save was made with different game rules"),
            LoadError::Corrupt(e) => write!(f, "corrupt save: {e}"),
        }
    }
}
impl std::error::Error for LoadError {}

impl Sim {
    /// Serialize the complete game state (postcard).
    pub fn save(&self) -> Vec<u8> {
        let data = SaveDataRef {
            version: SAVE_VERSION,
            rules_hash: self.rules.hash(),
            gf: self.gf,
            seed: self.seed,
            world: &self.world,
            players: &self.players,
            flags: &self.flags,
            roads: &self.roads,
            buildings: &self.buildings,
            figures: &self.figures,
            wares: &self.wares,
            sched: &self.sched,
            road_nodes: &self.road_nodes,
            territory_dirty: self.territory_dirty,
            nature: &self.nature,
            military: &self.military,
            vis: &self.vis,
            ships: &self.ships,
            sea: &self.sea,
        };
        let mut out = SAVE_MAGIC.to_vec();
        out.extend(postcard::to_stdvec(&data).expect("serializable state"));
        out
    }

    pub fn load(rules: Arc<Rules>, bytes: &[u8]) -> Result<Self, LoadError> {
        let body = bytes.strip_prefix(SAVE_MAGIC.as_slice()).ok_or(LoadError::NotASave)?;
        // version and rules first: an older save may not even decode any more
        let ((version, rules_hash), _): ((u32, u64), _) =
            postcard::take_from_bytes(body).map_err(|e| LoadError::Corrupt(e.to_string()))?;
        if version != SAVE_VERSION {
            return Err(LoadError::Version(version));
        }
        if rules_hash != rules.hash() {
            return Err(LoadError::RulesMismatch);
        }
        let d: SaveData = postcard::from_bytes(body).map_err(|e| LoadError::Corrupt(e.to_string()))?;
        let mut sim = Sim {
            rules,
            gf: d.gf,
            seed: d.seed,
            world: d.world,
            players: d.players,
            flags: d.flags,
            roads: d.roads,
            buildings: d.buildings,
            figures: d.figures,
            wares: d.wares,
            sched: d.sched,
            road_nodes: d.road_nodes,
            events: Vec::new(),
            territory_dirty: d.territory_dirty,
            sea_asked: Default::default(),
            harbor_seas: Default::default(),
            sea_paths: Default::default(),
            harbor_walkers: None,
            nature: d.nature,
            military: d.military,
            vis: d.vis,
            ships: d.ships,
            sea: d.sea,
            land: Vec::new(),
            owned: Vec::new(),
            territory_epoch: next_epoch(),
            route_cache: Default::default(),
            path_scratch: Default::default(),
            ware_scratch: Default::default(),
        };
        sim.validate().map_err(|e| LoadError::Corrupt(e.into()))?;
        sim.compute_land();
        sim.owned = vec![0; sim.players.len()];
        for q in sim.world.grid().coords() {
            if let Some(o) = sim.world.owner(q) {
                sim.owned[o as usize] += 1;
            }
        }
        Ok(sim)
    }

    /// A save that decodes may still be damaged: check that what refers to what exists, once,
    /// at load (a damaged save crashed the game a few frames later, or recursed without end).
    fn validate(&self) -> Result<(), &'static str> {
        let n = self.players.len();
        let g = self.world.grid();
        if !self.world.layers_ok() || self.road_nodes.len() != g.len() || self.vis.iter().any(|v| v.len() != g.len()) {
            return Err("map layers of the wrong size");
        }
        let player_ok = |p: PlayerId| (p as usize) < n;
        for (_, fl) in self.flags.iter() {
            if !player_ok(fl.player) || !g.contains(fl.pos) {
                return Err("a flag of a missing player or off the map");
            }
            if fl.roads.iter().flatten().any(|r| !self.roads.contains(*r))
                || fl.building.is_some_and(|b| !self.buildings.contains(b))
                || fl.wares.iter().any(|w| !self.wares.contains(*w))
            {
                return Err("a flag refers to something missing");
            }
        }
        for (_, r) in self.roads.iter() {
            if !player_ok(r.player)
                || !self.flags.contains(r.from)
                || !self.flags.contains(r.to)
                || !g.contains(r.start)
                || [r.carrier, r.donkey].iter().flatten().any(|f| !self.figures.contains(*f))
            {
                return Err("a road refers to something missing");
            }
        }
        for (i, r) in self.road_nodes.iter().enumerate() {
            if let Some(r) = r
                && (!self.roads.contains(*r) || !self.roads[*r].nodes(g).contains(&g.coord(i)))
            {
                return Err("the road map disagrees with the roads");
            }
        }
        for (_, b) in self.buildings.iter() {
            if !player_ok(b.player)
                || !g.contains(b.pos)
                || !self.flags.contains(b.flag)
                || b.worker.is_some_and(|f| !self.figures.contains(f))
                || b.soldiers.iter().any(|f| !self.figures.contains(*f))
                || b.out_queue.iter().any(|w| !self.wares.contains(*w))
                || b.fig_queue.iter().any(|f| !self.figures.contains(*f))
            {
                return Err("a building refers to something missing");
            }
        }
        for (_, w) in self.wares.iter() {
            let here = match w.loc {
                economy::WareLoc::Flag(f) => self.flags.contains(f),
                economy::WareLoc::Carried(f) => self.figures.contains(f),
                economy::WareLoc::Building(b) => self.buildings.contains(b),
            };
            if !here || !player_ok(w.player) {
                return Err("a ware in a missing place");
            }
        }
        for (_, f) in self.figures.iter() {
            // (wild animals belong to nobody: 255)
            if !player_ok(f.player) && f.player != 255 {
                return Err("a figure of a missing player");
            }
            if !g.contains(f.pos) {
                return Err("a figure off the map");
            }
            if f.inside.is_some_and(|b| !self.buildings.contains(b)) {
                return Err("a figure in a missing building");
            }
            if f.ware.is_some_and(|w| !self.wares.contains(w)) {
                return Err("a figure carries a missing ware");
            }
            if let figures::FigKind::Carrier { road, .. } = f.kind
                && !self.roads.contains(road)
            {
                return Err("a carrier on a missing road");
            }
        }
        Ok(())
    }

    pub fn new(rules: Arc<Rules>, map: &MapData, setup: &GameSetup) -> Result<Self, SetupError> {
        // every way into a game (map files, replays, generators, the editor) passes here
        map.validate().map_err(SetupError::BadMap)?;
        let world = World::new(map);
        let n = world.grid().len();
        let mut sim = Sim {
            rules,
            gf: 0,
            seed: setup.seed,
            world,
            players: Vec::new(),
            flags: Arena::new(),
            roads: Arena::new(),
            buildings: Arena::new(),
            figures: Arena::new(),
            wares: Arena::new(),
            sched: Scheduler::default(),
            road_nodes: vec![None; n],
            events: Vec::new(),
            territory_dirty: false,
            sea_asked: Default::default(),
            harbor_seas: Default::default(),
            sea_paths: Default::default(),
            harbor_walkers: None,
            nature: nature::Nature::default(),
            military: military::Fights::default(),
            vis: Vec::new(),
            ships: Arena::new(),
            sea: Vec::new(),
            land: Vec::new(),
            owned: Vec::new(),
            territory_epoch: next_epoch(),
            route_cache: Default::default(),
            path_scratch: Default::default(),
            ware_scratch: Default::default(),
        };
        for (i, ps) in setup.players.iter().enumerate() {
            sim.players.push(Player {
                setup: ps.clone(),
                alive: true,
                hq: None,
                settings: economy::default_settings(),
                road_epoch: 0,
                stats: Default::default(),
                distribution_rr: Default::default(),
            });
            let pos = map.hq.get(i).copied().flatten().ok_or(SetupError::NoHq(i))?;
            sim.place_hq(i as u8, pos, setup.start_goods).map_err(|_| SetupError::HqNotBuildable(i))?;
        }
        sim.recompute_territory();
        sim.nature_init(map);
        sim.compute_seas();
        sim.compute_land();
        sim.update_fog(true);
        sim.events.clear();
        Ok(sim)
    }

    pub fn rules(&self) -> &Rules {
        &self.rules
    }

    pub fn gf(&self) -> Gf {
        Gf(self.gf)
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn players(&self) -> &[Player] {
        &self.players
    }

    /// A number that changes whenever any node changes hands, and differs between games: what
    /// was derived from the territory at one epoch holds while it stays (perf round 3, #7).
    pub fn territory_epoch(&self) -> u64 {
        self.territory_epoch
    }

    /// The nodes each player owns, live (the statistics sample the same count).
    pub fn land_by_player(&self) -> Vec<u32> {
        self.owned.clone()
    }

    pub fn flags(&self) -> &Arena<Flag> {
        &self.flags
    }

    pub fn roads(&self) -> &Arena<Road> {
        &self.roads
    }

    pub fn buildings(&self) -> &Arena<Building> {
        &self.buildings
    }

    pub fn figures(&self) -> &Arena<Figure> {
        &self.figures
    }

    pub fn wares(&self) -> &Arena<WareItem> {
        &self.wares
    }

    pub fn flag(&self, id: FlagId) -> &Flag {
        &self.flags[id]
    }

    pub fn road(&self, id: RoadId) -> &Road {
        &self.roads[id]
    }

    /// Where a player's headquarters stands, while it stands (`Player::hq` keeps the id of a
    /// lost one, which may since point at nothing or at someone else's building).
    pub fn hq_pos(&self, player: PlayerId) -> Option<MapCoord> {
        let id = self.players.get(player as usize)?.hq?;
        self.buildings.get(id).filter(|b| b.player == player && b.kind == BuildingType::Headquarters).map(|b| b.pos)
    }

    pub fn building(&self, id: BuildingId) -> &Building {
        &self.buildings[id]
    }

    pub fn figure(&self, id: FigId) -> &Figure {
        &self.figures[id]
    }

    pub fn flag_at(&self, p: MapCoord) -> Option<FlagId> {
        match self.world.obj(p) {
            rr_world::NodeObj::Flag(i) => self.flags.id_at(i as usize),
            _ => None,
        }
    }

    pub fn building_at(&self, p: MapCoord) -> Option<BuildingId> {
        match self.world.obj(p) {
            rr_world::NodeObj::Building(i) => self.buildings.id_at(i as usize),
            _ => None,
        }
    }

    pub fn road_at(&self, p: MapCoord) -> Option<RoadId> {
        self.road_nodes[self.world.grid().index(p)]
    }

    pub fn are_allies(&self, a: PlayerId, b: PlayerId) -> bool {
        a == b || {
            let ta = self.players.get(a as usize).and_then(|p| p.setup.team);
            ta.is_some() && ta == self.players.get(b as usize).and_then(|p| p.setup.team)
        }
    }

    /// Advance exactly one frame.
    pub fn step(&mut self, cmds: &[PlayerCommand]) -> StepOutput {
        let mut sorted: Vec<&PlayerCommand> = cmds.iter().collect();
        sorted.sort_by_key(|c| c.player);
        for c in sorted {
            match self.apply(c.player, &c.cmd) {
                Err(reason) => {
                    self.events.push(Event::CommandRejected { player: c.player, cmd: c.cmd.clone(), reason })
                }
                // a confirmation for the interface (a sound when it worked, not when it was asked)
                Ok(())
                    if matches!(c.cmd, Cmd::PlaceBuilding { .. } | Cmd::PlaceFlag { .. } | Cmd::BuildRoad { .. }) =>
                {
                    self.events.push(Event::Placed { player: c.player })
                }
                Ok(()) => {}
            }
        }
        while let Some(ev) = self.sched.pop_due(self.gf) {
            match ev {
                Ev::Figure(f) => self.figure_event(f),
                Ev::Building(b) => self.building_event(b),
                Ev::Nature(p) => self.nature_event(p),
                Ev::Ship(s) => self.ship_event(s),
            }
        }
        if self.territory_dirty {
            self.recompute_territory();
        }
        self.distribute();
        self.military_tick();
        self.transport_tick();
        self.nature_tick();
        self.update_fog(false);
        if self.gf.is_multiple_of(economy::STATS_EVERY) {
            self.record_stats();
        }
        self.gf += 1;
        StepOutput { events: std::mem::take(&mut self.events) }
    }

    fn apply(&mut self, player: PlayerId, cmd: &Cmd) -> Result<(), RejectReason> {
        if player as usize >= self.players.len() || !self.players[player as usize].alive {
            return Err(RejectReason::NotAllowed);
        }
        // the sim never panics on input: nodes off the map are refused up front
        let g = self.world.grid();
        // ...and so are lists longer than anything a player can mean (a 4-million-entry
        // transport order slowed every machine of a network game 145-fold)
        let too_long = match cmd {
            Cmd::BuildRoad { dirs, .. } => dirs.len() > MAX_ROAD_STEPS,
            Cmd::SetTransportOrder { order } => order.len() > rr_rules::Ware::ALL.len(),
            Cmd::SetTools { values } => values.len() > rr_rules::Ware::ALL.len(),
            _ => false,
        };
        if too_long {
            return Err(RejectReason::NotAllowed);
        }
        if cmd.coords().iter().any(|p| !g.contains(*p)) {
            return Err(RejectReason::NotFound);
        }
        match cmd {
            Cmd::PlaceFlag { at } => self.cmd_place_flag(player, *at).map(|_| ()),
            Cmd::RemoveFlag { at } => self.cmd_remove_flag(player, *at),
            Cmd::BuildRoad { from, dirs } => self.cmd_build_road(player, *from, dirs).map(|_| ()),
            Cmd::RemoveRoad { flag, dir } => self.cmd_remove_road(player, *flag, *dir),
            Cmd::PlaceBuilding { at, kind } => self.cmd_place_building(player, *at, *kind).map(|_| ()),
            Cmd::RemoveBuilding { at } => self.cmd_remove_building(player, *at),
            Cmd::ToggleProduction { at } => self.cmd_toggle_production(player, *at),
            Cmd::SetPriority { at, on } => self.cmd_set_priority(player, *at, *on),
            Cmd::SendGeologist { flag } => self.cmd_send_specialist(player, *flag, rr_rules::Job::Geologist),
            Cmd::SendScout { flag } => self.cmd_send_specialist(player, *flag, rr_rules::Job::Scout),
            Cmd::Attack { target, soldiers, strong } => self.cmd_attack(player, *target, *soldiers, *strong),
            Cmd::SetDistribution { ware, building, weight } => {
                let s = &mut self.players[player as usize].settings;
                for e in s.distribution.iter_mut() {
                    if e.0 == *ware && e.1 == *building {
                        e.2 = (*weight).min(10);
                    }
                }
                Ok(())
            }
            Cmd::SetTransportOrder { order } => {
                let mut o = order.clone();
                for w in rr_rules::Ware::ALL {
                    if !o.contains(w) {
                        o.push(*w);
                    }
                }
                self.players[player as usize].settings.transport_order = o;
                Ok(())
            }
            Cmd::SetMilitary { values } => {
                let mut v = *values;
                for (x, max) in v.iter_mut().zip(MILITARY_SCALE) {
                    *x = (*x).min(max);
                }
                self.players[player as usize].settings.military = v;
                Ok(())
            }
            Cmd::SetTools { values } => {
                let s = &mut self.players[player as usize].settings;
                for (w, v) in values {
                    if let Some(e) = s.tools.iter_mut().find(|e| e.0 == *w) {
                        e.1 = (*v).min(10);
                    }
                }
                Ok(())
            }
            Cmd::SetGarrisonWanted { at, wanted } => self.cmd_garrison_wanted(player, *at, *wanted),
            Cmd::SetStockPolicy { at, ware, policy } => self.cmd_stock_policy(player, *at, *ware, *policy),
            Cmd::SetPeoplePolicy { at, job, policy } => self.cmd_people_policy(player, *at, *job, *policy),
            // roads are upgraded by traffic (pack donkeys), not by hand
            Cmd::UpgradeRoad { .. } => Err(RejectReason::NotAllowed),
            Cmd::StartExpedition { harbor } => self.cmd_start_expedition(player, *harbor),
            Cmd::StartExploration { harbor } => self.cmd_start_exploration(player, *harbor),
            Cmd::HandOver { at, to } => self.cmd_hand_over(player, *at, *to),
            Cmd::Offer { at, ware, n } => self.cmd_offer(player, *at, *ware, *n),
            Cmd::Bless { at, n, recruit } => self.cmd_bless(player, *at, *n, *recruit),
            Cmd::SendExpedition { harbor, to } => self.cmd_send_expedition(player, *harbor, *to),
            Cmd::Surrender => {
                self.defeat(player);
                Ok(())
            }
            Cmd::SendHome { at } => self.cmd_send_home(player, *at),
            Cmd::SetReserve { at, rank, count } => {
                let b = self.building_at(*at).ok_or(RejectReason::NotFound)?;
                let bd = &mut self.buildings[b];
                if bd.player != player || bd.inventory.is_none() || *rank as usize >= rr_rules::Job::SOLDIERS.len() {
                    return Err(RejectReason::NotAllowed);
                }
                bd.reserve[*rank as usize] = (*count).min(MAX_RESERVE);
                Ok(())
            }
            Cmd::SetShipyardMode { at, ships } => {
                let b = self.building_at(*at).ok_or(RejectReason::NotFound)?;
                let bd = &mut self.buildings[b];
                if bd.player != player || bd.kind != BuildingType::Shipyard {
                    return Err(RejectReason::NotAllowed);
                }
                bd.build_ships = *ships;
                Ok(())
            }
        }
    }

    pub(crate) fn schedule(&mut self, delay: u32, ev: Ev) -> (u32, u64) {
        self.sched.at(self.gf + delay.max(1), ev)
    }

    pub(crate) fn rng(&self, entity: u64, purpose: rr_core::Purpose) -> rr_core::Pcg32 {
        rr_core::rng_for(self.seed, self.gf, entity, purpose)
    }

    // entity kinds get separate key spaces, so a building, a figure and a node with the same
    // number never draw the same numbers in one frame
    pub(crate) fn rng_building(&self, b: BuildingId, purpose: rr_core::Purpose) -> rr_core::Pcg32 {
        self.rng(1 << 60 | b.index() as u64, purpose)
    }

    pub(crate) fn rng_figure(&self, f: FigId, purpose: rr_core::Purpose) -> rr_core::Pcg32 {
        self.rng(2 << 60 | f.index() as u64, purpose)
    }

    pub(crate) fn rng_node(&self, p: MapCoord, purpose: rr_core::Purpose) -> rr_core::Pcg32 {
        self.rng(3 << 60 | (p.x as u64) << 16 | p.y as u64, purpose)
    }

    /// Hash of the complete game state: everything a save holds (world layers, nature,
    /// military, fog, seas, scheduler, every entity field). Compare it between peers or runs.
    pub fn sync_hash(&self) -> u64 {
        let bytes = self.save();
        let mut h = StableHasher::new();
        for chunk in bytes.chunks(8) {
            let mut b = [0u8; 8];
            b[..chunk.len()].copy_from_slice(chunk);
            h.write_u64(u64::from_le_bytes(b));
        }
        (bytes.len() as u64).stable_hash(&mut h);
        h.finish()
    }

    /// Wares and people of a player in all warehouses.
    pub fn player_inventory(&self, player: PlayerId) -> Inventory {
        let mut inv = Inventory::default();
        for (_, b) in self.buildings.iter() {
            if b.player == player
                && let Some(i) = &b.inventory
            {
                inv.add_all(i);
            }
        }
        inv
    }

    /// Diffable text dump of the state, for desync hunting and debugging.
    pub fn dump(&self, w: &mut dyn std::fmt::Write) -> std::fmt::Result {
        writeln!(w, "gf {}", self.gf)?;
        for (id, b) in self.buildings.iter() {
            writeln!(
                w,
                "building {id:?} {:?} p{} at ({},{}) {:?} worker {:?} coming {} stock {:?} incoming {:?} site_in {:?} soldiers {:?} queue {:?}",
                b.kind,
                b.player,
                b.pos.x,
                b.pos.y,
                b.state,
                b.worker,
                b.worker_coming,
                b.stock,
                b.incoming,
                b.site_incoming,
                b.soldiers,
                b.fig_queue
            )?;
        }
        for (id, r) in self.roads.iter() {
            writeln!(
                w,
                "road {id:?} {:?}->{:?} len {} carrier {:?} wanted {}",
                r.from,
                r.to,
                r.len(),
                r.carrier,
                r.carrier_wanted
            )?;
        }
        for (id, f) in self.flags.iter() {
            writeln!(w, "flag {id:?} at ({},{}) wares {:?}", f.pos.x, f.pos.y, f.wares)?;
        }
        for (id, f) in self.figures.iter() {
            if f.player == 255 {
                continue;
            }
            writeln!(
                w,
                "fig {id:?} {:?} at ({},{}) {:?} {:?} step {:?} path {} inside {:?} ware {:?} ev {:?}",
                f.job,
                f.pos.x,
                f.pos.y,
                f.kind,
                f.motion,
                f.step,
                f.path.len(),
                f.inside,
                f.ware,
                f.ev
            )?;
        }
        for (id, s) in self.ships.iter() {
            writeln!(
                w,
                "ship {id:?} p{} at ({},{}) {:?} docked {:?} path {}",
                s.player,
                s.pos.x,
                s.pos.y,
                s.task,
                s.docked,
                s.path.len()
            )?;
        }
        for (id, x) in self.wares.iter() {
            writeln!(w, "ware {id:?} {:?} {:?} goal {:?} next {:?}", x.kind, x.loc, x.goal, x.next)?;
        }
        Ok(())
    }

    pub fn dump_string(&self) -> String {
        let mut s = String::new();
        let _ = self.dump(&mut s);
        s
    }

    /// Test helper: every cached route table still equals a fresh one.
    pub fn debug_route_cache_ok(&self) -> bool {
        let cache = self.route_cache.borrow().clone();
        cache.iter().all(|(goal, r)| !self.flags.contains(*goal) || r[..] == self.routes_impl(*goal, false, None)[..])
    }

    /// A worker is on the way to work at this node (tree, stone, field, animal).
    pub fn is_work_reserved(&self, p: MapCoord) -> bool {
        self.nature.reserved.contains(&p)
    }

    /// Test helper: set a soldier's hit points.
    pub fn debug_set_hp(&mut self, f: FigId, hp: u8) {
        self.figures[f].hp = hp;
    }

    /// Test helper: `n` new wares of `player` at the flag at `at`, as if a workshop had made them.
    pub fn debug_drop_ware(&mut self, player: PlayerId, at: MapCoord, ware: Ware, n: u32) {
        let Some(f) = self.flag_at(at) else { return };
        for _ in 0..n {
            let w = self.wares.insert(WareItem { kind: ware, player, loc: WareLoc::Flag(f), goal: None, next: None });
            self.put_ware_at_flag(w, f);
        }
    }

    /// Test helper: every soldier of the building at `at` steps out to defend it, then the land is
    /// worked out anew.
    pub fn debug_send_defenders(&mut self, at: MapCoord) {
        if let Some(b) = self.building_at(at) {
            while self.take_defender(b).is_some() {}
            self.recompute_territory();
        }
    }

    /// Test helper: burn the building at `at` as if it were destroyed in the game.
    /// Set a building's stock of input `slot` (tests).
    pub fn debug_set_stock(&mut self, b: BuildingId, slot: usize, n: u8) {
        self.buildings[b].stock[slot] = n;
    }

    pub fn debug_destroy_building(&mut self, at: MapCoord) {
        if let Some(b) = self.building_at(at) {
            self.destroy_building(b, true);
        }
    }

    /// Test helper: remove up to `n` wares from the player's HQ.
    pub fn debug_take_ware(&mut self, player: PlayerId, w: Ware, n: u32) {
        if let Some(hq) = self.players[player as usize].hq
            && let Some(inv) = self.buildings[hq].inventory.as_mut()
        {
            inv.wares[w.index()] = inv.wares[w.index()].saturating_sub(n);
        }
    }

    /// Test helper: a ship of `player` at the coast of the harbor spot `spot`.
    pub fn debug_spawn_ship(&mut self, player: PlayerId, spot: MapCoord) {
        let coast = self.harbor_coast(spot).expect("a coast");
        self.spawn_ship(player, coast);
    }

    /// Test helper: take wares out of the store of warehouse `b`.
    pub fn debug_take_ware_at(&mut self, b: BuildingId, w: Ware, n: u32) {
        if let Some(inv) = self.buildings.get_mut(b).and_then(|x| x.inventory.as_mut()) {
            inv.wares[w.index()] = inv.wares[w.index()].saturating_sub(n);
        }
    }

    /// Test helper: add settlers to the player's HQ.
    pub fn debug_add_job(&mut self, player: PlayerId, j: rr_rules::Job, n: u32) {
        if let Some(hq) = self.players[player as usize].hq
            && let Some(inv) = self.buildings[hq].inventory.as_mut()
        {
            inv.jobs[j.index()] += n;
        }
    }

    /// Test helper: remove settlers from the player's HQ.
    pub fn debug_take_job(&mut self, player: PlayerId, j: rr_rules::Job, n: u32) {
        if let Some(hq) = self.players[player as usize].hq
            && let Some(inv) = self.buildings[hq].inventory.as_mut()
        {
            inv.jobs[j.index()] = inv.jobs[j.index()].saturating_sub(n);
        }
    }

    /// Test helper: add wares to the player's HQ.
    pub fn debug_add_ware(&mut self, player: PlayerId, w: Ware, n: u32) {
        if let Some(hq) = self.players[player as usize].hq
            && let Some(inv) = self.buildings[hq].inventory.as_mut()
        {
            inv.wares[w.index()] += n;
        }
    }

    /// Test/editor helper: put an object on a node directly.
    pub fn debug_set_obj(&mut self, p: MapCoord, o: rr_world::NodeObj) {
        self.world.set_obj(p, o);
    }

    pub fn road_between(&self, a: MapCoord, b: MapCoord) -> Option<RoadId> {
        let fa = self.flag_at(a)?;
        let fb = self.flag_at(b)?;
        self.flags[fa].roads.iter().flatten().copied().find(|r| {
            let r = &self.roads[*r];
            (r.from == fa && r.to == fb) || (r.from == fb && r.to == fa)
        })
    }
}
