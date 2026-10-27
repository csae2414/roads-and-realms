//! Computer players. An AI reads the simulation (like a player looking at the screen, plus
//! the resource layer in its own territory) and returns commands like a human would.
//!
//! Strategy, in order of priority each decision tick:
//! 1. connect flags that have no road; split long roads with flags,
//! 2. expand the territory with military buildings (one site at a time),
//! 3. build the next missing economy building from a phase-dependent wish list,
//! 4. attack weak enemy buildings in range, demolish useless buildings.

use rr_core::{Direction, MapCoord, Pcg32, Resource};
use rr_rules::{BuildingType as B, Job, Size, Ware};
use rr_sim::{BState, Cmd, FlagId, PlayerCommand, PlayerId, Sim};
use rr_world::{Bq, NodeObj};

const THINK_EVERY: u32 = 40;
/// Frames before the AI tries again to attack a building it could not reach.
const OUT_OF_REACH_WAIT: u32 = 3000;
/// Frames before the AI looks again for a spot for a kind that had none.
const NO_SPOT_WAIT: u32 = 800;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Level {
    Easy,
    #[default]
    Normal,
    Hard,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Ai {
    pub player: PlayerId,
    pub level: Level,
    rng: Pcg32,
    /// Nodes where placing failed recently (skip them for a while).
    blocked: Vec<(MapCoord, u32)>,
    /// Our land, gathered at the start of each decision (not saved: rebuilt before use).
    #[serde(skip)]
    land: Vec<MapCoord>,
    /// The territory epoch `land` was gathered at (0: never; perf round 3, #7: every think
    /// walked the whole map).
    #[serde(skip)]
    land_epoch: u64,
    /// Building kinds that found no spot recently, until when to skip them.
    no_spot: Vec<(B, u32)>,
    /// Enemy buildings our soldiers could not reach lately, until when to skip them.
    out_of_reach: Vec<(MapCoord, u32)>,
    /// Leave this player alone until this frame (missions spare the newcomer at first).
    pub truce: Option<(PlayerId, u32)>,
    /// Knows this player's coast and lands an expedition there (a mission's invader; not saved:
    /// the game sets it again from the mission).
    #[serde(skip)]
    pub invade: Option<PlayerId>,
    /// Nobody may attack before this frame (a free game's peace time; not saved: the game sets it
    /// again). The AI plans no attack and no catapult until then (veteran round 11, #3: it threw
    /// away up to 589 of its 878 commands in an hour of peace).
    #[serde(skip)]
    pub peace_until: u32,
    /// Places no new military building while it holds this share of the map's land, in percent
    /// (a mission's rival that must leave room for the player's goal; not saved: the game sets
    /// it again from the mission).
    #[serde(skip)]
    pub land_cap: Option<u32>,
}

/// The share of the map's land (walkable nodes) that `player` owns, in percent (a goal counted
/// against the whole map, not against the land the players hold: with two realms that start
/// equal, a share of the held land is met from the start).
pub fn map_land_share(sim: &Sim, player: PlayerId) -> u32 {
    let w = sim.world();
    let (mut land, mut ours) = (0u32, 0u32);
    for p in w.grid().coords() {
        if w.node_walkable(p) {
            land += 1;
            ours += u32::from(w.owner(p) == Some(player));
        }
    }
    (ours * 100).checked_div(land).unwrap_or(0)
}

/// Raise on any change to how the computer players decide (they run on every machine of a
/// network game).
pub const AI_VERSION: u32 = 60;

/// How long the rivals of a mission leave the newcomer (player 0) alone: 30 minutes.
pub const MISSION_TRUCE: u32 = 30 * 60 * 20;

/// After a truce, a building of the spared player is left alone this long after its first
/// soldier moved in (3 minutes): the newcomer gets time to man it.
pub const GRACE: u32 = 3 * 60 * 20;

const GAME_MAGIC_V1: &[u8; 4] = b"RRG1";
/// v2: the game frame follows the magic, so a save list can show it without loading.
const GAME_MAGIC_V2: &[u8; 4] = b"RRG2";
/// v3: as v2, plus a blob for the interface (messages, tips shown...).
const GAME_MAGIC_V3: &[u8; 4] = b"RRG3";
/// v4: as v3; the computer players have a truce.
const GAME_MAGIC_V4: &[u8; 4] = b"RRG4";
/// Version 5: as version 4 (game frame, then the postcard body), the body zlib-compressed: late
/// saves shrink from about 1 MB to a fifth.
const GAME_MAGIC_V5: &[u8; 4] = b"RRG5";
/// Version 6: as version 5, plus a checksum of the unpacked body after the frame, so a damaged
/// file is refused before anything in it is trusted.
const GAME_MAGIC: &[u8; 4] = b"RRG6";

fn body_sum(body: &[u8]) -> u64 {
    use rr_core::StableHasher;
    let mut h = StableHasher::new();
    for chunk in body.chunks(8) {
        let mut b = [0u8; 8];
        b[..chunk.len()].copy_from_slice(chunk);
        h.write_u64(u64::from_le_bytes(b));
    }
    h.write_u64(body.len() as u64);
    h.finish()
}
/// No save unpacks to more than this (a broken or hostile file can't eat the memory).
const MAX_UNPACKED: usize = 512 * 1024 * 1024;

/// A computer player as saved before v4 (no truce).
#[derive(serde::Deserialize)]
#[cfg_attr(test, derive(serde::Serialize))]
struct AiV1 {
    player: PlayerId,
    level: Level,
    rng: Pcg32,
    blocked: Vec<(MapCoord, u32)>,
    no_spot: Vec<(B, u32)>,
    out_of_reach: Vec<(MapCoord, u32)>,
}

impl From<AiV1> for Ai {
    fn from(o: AiV1) -> Self {
        Ai {
            player: o.player,
            level: o.level,
            rng: o.rng,
            blocked: o.blocked,
            land: Vec::new(),
            land_epoch: 0,
            no_spot: o.no_spot,
            out_of_reach: o.out_of_reach,
            truce: None,
            invade: None,
            peace_until: 0,
            land_cap: None,
        }
    }
}

/// Save a game with its computer players (a bare [`Sim::save`] would lose what they planned).
pub fn save_game(sim: &Sim, ais: &[Ai]) -> Vec<u8> {
    save_game_with(sim, ais, &[])
}

/// [`save_game`] with `extra` bytes of the caller's own (see [`load_game_with`]).
pub fn save_game_with(sim: &Sim, ais: &[Ai], extra: &[u8]) -> Vec<u8> {
    let mut out = GAME_MAGIC.to_vec();
    out.extend(sim.gf().0.to_le_bytes());
    let body = postcard::to_stdvec(&(sim.save(), ais, extra)).expect("serialize");
    out.extend(body_sum(&body).to_le_bytes());
    out.extend(miniz_oxide::deflate::compress_to_vec_zlib(&body, SAVE_LEVEL));
    out
}

/// zlib level of saves: level 6 took 22-70 ms on the UI thread per F5, level 1 takes 3-9 ms
/// for about 20 % more bytes (perf-2 #8).
const SAVE_LEVEL: u8 = 1;

/// The game frame of a save from its first bytes (`None` for older or foreign files).
pub fn save_game_time(head: &[u8]) -> Option<u32> {
    let rest = [GAME_MAGIC, GAME_MAGIC_V5, GAME_MAGIC_V4, GAME_MAGIC_V3, GAME_MAGIC_V2]
        .iter()
        .find_map(|m| head.strip_prefix(m.as_slice()))?;
    Some(u32::from_le_bytes(rest.get(..4)?.try_into().ok()?))
}

/// Load a game written by [`save_game`] (or a bare simulation save, which gets no AIs).
pub fn load_game(rules: std::sync::Arc<rr_rules::Rules>, bytes: &[u8]) -> Result<(Sim, Vec<Ai>), String> {
    load_game_with(rules, bytes).map(|(sim, ais, _)| (sim, ais))
}

/// Load a game and the extra bytes saved with it (empty for older saves).
/// The interface's part of a save in today's format, without loading the game (for the save
/// list: which mission a save is). `None` for older formats or a damaged file.
pub fn save_game_extra(bytes: &[u8]) -> Option<Vec<u8>> {
    let b = bytes.strip_prefix(GAME_MAGIC.as_slice())?;
    let body = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(b.get(12..)?, MAX_UNPACKED).ok()?;
    let (_, _, extra): (Vec<u8>, Vec<Ai>, Vec<u8>) = postcard::from_bytes(&body).ok()?;
    Some(extra)
}

pub fn load_game_with(rules: std::sync::Arc<rr_rules::Rules>, bytes: &[u8]) -> Result<(Sim, Vec<Ai>, Vec<u8>), String> {
    let err = |e: postcard::Error| e.to_string();
    let old = |ais: Vec<AiV1>| ais.into_iter().map(Ai::from).collect::<Vec<Ai>>();
    let (sim, ais, extra): (Vec<u8>, Vec<Ai>, Vec<u8>) = if let Some(b) = bytes.strip_prefix(GAME_MAGIC.as_slice()) {
        let sum = u64::from_le_bytes(b.get(4..12).ok_or("truncated save")?.try_into().map_err(|_| "truncated save")?);
        let body = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&b[12..], MAX_UNPACKED)
            .map_err(|e| format!("damaged save ({:?})", e.status))?;
        if body_sum(&body) != sum {
            return Err("damaged save: the checksum doesn't match".into());
        }
        postcard::from_bytes(&body).map_err(err)?
    } else if let Some(b) = bytes.strip_prefix(GAME_MAGIC_V5.as_slice()) {
        let packed = b.get(4..).ok_or("truncated save")?;
        let body = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(packed, MAX_UNPACKED)
            .map_err(|e| format!("damaged save ({:?})", e.status))?;
        postcard::from_bytes(&body).map_err(err)?
    } else if let Some(b) = bytes.strip_prefix(GAME_MAGIC_V4.as_slice()) {
        postcard::from_bytes(b.get(4..).ok_or("truncated save")?).map_err(err)?
    } else if let Some(b) = bytes.strip_prefix(GAME_MAGIC_V3.as_slice()) {
        let (sim, ais, extra): (Vec<u8>, Vec<AiV1>, Vec<u8>) =
            postcard::from_bytes(b.get(4..).ok_or("truncated save")?).map_err(err)?;
        (sim, old(ais), extra)
    } else {
        let body = if let Some(b) = bytes.strip_prefix(GAME_MAGIC_V2.as_slice()) {
            b.get(4..).ok_or("truncated save")?
        } else if let Some(b) = bytes.strip_prefix(GAME_MAGIC_V1.as_slice()) {
            b
        } else {
            return Sim::load(rules, bytes).map(|s| (s, Vec::new(), Vec::new())).map_err(|e| e.to_string());
        };
        let (sim, ais): (Vec<u8>, Vec<AiV1>) = postcard::from_bytes(body).map_err(err)?;
        (sim, old(ais), Vec::new())
    };
    let sim = Sim::load(rules, &sim).map_err(|e| e.to_string())?;
    // computer players only for players that exist, one each
    let mut seen = Vec::new();
    for ai in &ais {
        if ai.player as usize >= sim.players().len() || seen.contains(&ai.player) {
            return Err("damaged save: a computer player for a missing player".into());
        }
        seen.push(ai.player);
    }
    Ok((sim, ais, extra))
}

fn need_bq(size: Size) -> Bq {
    match size {
        Size::Hut => Bq::Hut,
        Size::House => Bq::House,
        Size::Castle => Bq::Castle,
        Size::Mine => Bq::Mine,
        Size::Harbor => Bq::Harbor,
    }
}

impl Ai {
    pub fn new(player: PlayerId) -> Self {
        Self::with_level(player, Level::Normal)
    }

    pub fn with_level(player: PlayerId, level: Level) -> Self {
        Self {
            player,
            level,
            rng: Pcg32::new(0xa1 + player as u64, 7),
            blocked: Vec::new(),
            land: Vec::new(),
            land_epoch: 0,
            no_spot: Vec::new(),
            out_of_reach: Vec::new(),
            truce: None,
            invade: None,
            peace_until: 0,
            land_cap: None,
        }
    }

    /// Dice of their own for this game seed: on one map, games differ only by the AIs' choices
    /// (`rr-cli fairness` played the same game for every seed). Games don't call it.
    pub fn reseed(&mut self, seed: u64) {
        self.rng = Pcg32::new(0xa1 + self.player as u64 + seed.wrapping_mul(0x9e37_79b9_7f4a_7c15), 7);
    }

    pub fn think(&mut self, sim: &Sim) -> Vec<PlayerCommand> {
        let gf = sim.gf().0;
        let every = match self.level {
            Level::Easy => THINK_EVERY * 3,
            Level::Normal => THINK_EVERY,
            Level::Hard => THINK_EVERY / 2,
        };
        if !(gf + self.player as u32 * 7).is_multiple_of(every) || !sim.players()[self.player as usize].alive {
            return Vec::new();
        }
        self.blocked.retain(|(_, until)| *until > gf);
        let w = sim.world();
        if self.land_epoch != sim.territory_epoch() {
            self.land = w.grid().coords().filter(|p| w.owner(*p) == Some(self.player)).collect();
            self.land_epoch = sim.territory_epoch();
        }
        // RTTR's attack interval: normal 750 GF, hard about every 80 (veteran round 6, #5)
        // (easy every 8 minutes: 80 thoughts of 120 GF; new-player round 8, #1: every two
        // minutes wore a newcomer's front down)
        let attack_every = self.attack_every();
        let attack_tick = (gf / every + self.player as u32).is_multiple_of(attack_every);
        // no warehouse left: give up, as RTTR's AI (TestDefeat); the posts stay for the victor
        // (veteran round 9, #1: tearing them down instead razed whole realms)
        // (a harbor being built still counts: it may bring the realm back)
        if !self.my_buildings(sim).any(|(_, b)| b.inventory.is_some() || b.kind.is_warehouse()) {
            return vec![PlayerCommand { player: self.player, cmd: Cmd::Surrender }];
        }
        // after the first hour, no post, none being built and no soldier: beaten, it gives up
        // (game design round 1, #5: one storehouse kept such a realm alive for 2-4 hours)
        if gf >= BEATEN_AFTER
            && sim.player_inventory(self.player).soldiers() == 0
            && !self.my_buildings(sim).any(|(_, b)| rr_sim::is_frontier_military(b.kind))
        {
            return vec![PlayerCommand { player: self.player, cmd: Cmd::Surrender }];
        }
        let cmd = self
            .keep_reserves(sim)
            .or_else(|| self.gather_recruit_goods(sim))
            .or_else(|| self.coins_to_the_front(sim))
            .or_else(|| self.connect_flags(sim))
            .or_else(|| self.split_roads(sim))
            .or_else(|| self.alternative_road(sim))
            .or_else(|| if attack_tick { self.attack(sim) } else { None })
            .or_else(|| self.balance(sim))
            .or_else(|| self.seafaring(sim))
            .or_else(|| self.military_expand(sim))
            .or_else(|| self.economy(sim))
            .or_else(|| self.cleanup(sim));
        cmd.map(|cmd| vec![PlayerCommand { player: self.player, cmd }]).unwrap_or_default()
    }

    /// Is it this rival's turn on the campaign's player `p`: one rival per window, in turn among
    /// the living ones (new-player round 9, #2).
    fn my_campaign_turn(&self, sim: &Sim, p: PlayerId) -> bool {
        let players = sim.players();
        let rivals: Vec<u8> = (0..players.len() as u8)
            .filter(|q| *q != p && players[*q as usize].alive && !sim.are_allies(*q, p))
            .collect();
        let Some(me) = rivals.iter().position(|q| *q == self.player) else { return true };
        (sim.gf().0 / CAMPAIGN_ATTACK_WINDOW) as usize % rivals.len() == me
    }

    /// Thoughts between attack rounds.
    fn attack_every(&self) -> u32 {
        match self.level {
            Level::Easy => 80,
            Level::Normal => 19,
            Level::Hard => 4,
        }
    }

    /// GF between attack rounds.
    fn attack_period(&self) -> u32 {
        let every = match self.level {
            Level::Easy => THINK_EVERY * 3,
            Level::Normal => THINK_EVERY,
            Level::Hard => THINK_EVERY / 2,
        };
        every * self.attack_every()
    }

    /// Warehouses keep privates at home to defend them (reserves start at none): two in the
    /// headquarters, one elsewhere.
    fn keep_reserves(&self, sim: &Sim) -> Option<Cmd> {
        let me = self.player;
        let g = sim.world().grid();
        // rival soldiers in sight round a warehouse: the reserve grows with them, up to 10
        // (veteran round 9, #6: a castle with 2 at home fell to three privates at minute 15)
        let rivals: Vec<MapCoord> = sim
            .figures()
            .iter()
            .filter(|(_, f)| f.player != me && !sim.are_allies(f.player, me) && f.job.soldier_rank().is_some())
            .filter(|(_, f)| f.inside.is_none() && sim.visibility(me, f.pos) == rr_sim::Visibility::Visible)
            .map(|(_, f)| f.pos)
            .collect();
        self.my_buildings(sim).filter(|(_, b)| b.is_ready() && b.inventory.is_some()).find_map(|(_, b)| {
            let base = if b.kind == B::Headquarters { 2 } else { 1 };
            let near = rivals.iter().filter(|p| g.distance(**p, b.pos) <= 20).count() as u8;
            let want = base.max((near / 2).min(10));
            // (it stays up once raised: letting it fall again lost the castle at minute 15 again)
            (b.reserve[0] < want).then_some(Cmd::SetReserve { at: b.pos, rank: 0, count: want })
        })
    }

    /// Recruits need beer, a sword and a shield in the same warehouse: gather all three at the
    /// headquarters (RTTR SetGatheringForUpgradeWarehouse).
    fn gather_recruit_goods(&self, sim: &Sim) -> Option<Cmd> {
        // the easy computer leaves its kit where it lands: fewer recruits, a gentler opponent
        if self.level == Level::Easy {
            return None;
        }
        let (_, hq) = self.my_buildings(sim).find(|(_, b)| b.kind == B::Headquarters && b.is_ready())?;
        [Ware::Beer, Ware::Sword, Ware::Shield].into_iter().find_map(|ware| {
            (hq.stock_policy[ware.index()] != rr_sim::StockPolicy::Collect).then_some(Cmd::SetStockPolicy {
                at: hq.pos,
                ware,
                policy: rr_sim::StockPolicy::Collect,
            })
        })
    }

    /// Coins only where soldiers fight: far inland a building takes none while the realm has a
    /// front (RTTR MilUpgradeOptim; veteran round 7, #6: most coins promoted inland soldiers).
    fn coins_to_the_front(&self, sim: &Sim) -> Option<Cmd> {
        // every other thought: a realm toggling twenty buildings had no thought left to grow
        if !(sim.gf().0 / THINK_EVERY + u32::from(self.player)).is_multiple_of(2) {
            return None;
        }
        // (the posts on the map gathered once, not per building: perf-2 #3)
        let mine: Vec<(rr_sim::BuildingId, &rr_sim::Building)> = self
            .my_buildings(sim)
            .filter(|(_, b)| b.is_ready() && b.kind != B::Headquarters && sim.rules().building(b.kind).is_military())
            .collect();
        let ids: Vec<rr_sim::BuildingId> = mine.iter().map(|(id, _)| *id).collect();
        let posts: Vec<(usize, &rr_sim::Building)> =
            sim.frontier_distances(&ids).into_iter().zip(mine.iter().map(|(_, b)| *b)).collect();
        // (near or in attack reach: one front post flickering between classes flipped every
        // inland building back and forth)
        let front = posts.iter().any(|(d, _)| *d == 3 || *d == 1);
        let toggle = posts
            .iter()
            // (2, by an own harbor, is as safe as far inland: nobody attacks over the sea;
            // veteran round 8, #6)
            .find(|(d, b)| b.coins_stopped != (front && (*d == 0 || *d == 2)))
            .map(|(_, b)| Cmd::ToggleProduction { at: b.pos });
        if toggle.is_some() {
            return toggle;
        }
        // a post facing a rival wants its full garrison again, though Send home cut it (RTTR
        // HandleBorderChanged resets the troop limits; veteran round 10, #6)
        let cut = posts.iter().find_map(|(d, b)| {
            let full = sim.rules().building(b.kind).military.as_ref()?.soldiers;
            (*d != 0 && b.garrison_wanted < full).then_some(Cmd::SetGarrisonWanted { at: b.pos, wanted: full })
        });
        if cut.is_some() || !front {
            return cut;
        }
        // officers and generals made inland before the war go to it: sent home, the castle
        // passes them on to where soldiers are wanted (RTTR MilUpgradeOptim; item 307)
        let high =
            |b: &rr_sim::Building| b.soldiers.iter().any(|f| sim.figure(*f).job.soldier_rank().is_some_and(|r| r >= 3));
        posts.iter().find(|(d, b)| *d == 0 && b.soldiers.len() > 1 && high(b)).map(|(_, b)| Cmd::SendHome { at: b.pos })
    }

    /// Soldiers in store free to man a post: not those the warehouses keep at home (veteran round
    /// 9, #3: posts built with the reserve stood unmanned, with their stones and boards).
    /// (A reserve keeps back only the soldiers really there: veteran round 10, #7, a realm of 13
    /// warehouses read 0 spare with 8 free.)
    fn spare_soldiers(&self, sim: &Sim) -> usize {
        spare_of(self.my_buildings(sim).filter_map(|(_, b)| {
            let inv = b.inventory.as_ref()?;
            let there: usize = Job::SOLDIERS.iter().map(|j| inv.job(*j) as usize).sum();
            Some((there, b.reserve.iter().map(|r| *r as usize).sum::<usize>()))
        }))
    }

    fn my_buildings<'a>(&self, sim: &'a Sim) -> impl Iterator<Item = (rr_sim::BuildingId, &'a rr_sim::Building)> + 'a {
        let me = self.player;
        sim.buildings().iter().filter(move |(_, b)| b.player == me)
    }

    fn count(&self, sim: &Sim, kind: B) -> usize {
        self.my_buildings(sim).filter(|(_, b)| b.kind == kind).count()
    }

    /// Construction sites of economy buildings (military sites are counted apart).
    /// A site waiting five minutes or more for wares the realm has none of doesn't count: it
    /// would block everything else (granite mines above all) until the stone comes back.
    fn sites(&self, sim: &Sim) -> usize {
        let inv = sim.player_inventory(self.player);
        let starved = inv.ware(Ware::Stones) == 0 || inv.ware(Ware::Boards) == 0;
        let gf = sim.gf().0;
        self.my_buildings(sim)
            .filter(|(_, b)| !b.is_ready() && !sim.rules().building(b.kind).is_military())
            .filter(|(_, b)| !(starved && gf > b.placed() + 6000))
            .count()
    }

    /// The headquarters, or once it fell any warehouse of ours (veteran round 7, #4: a realm
    /// with three harbors placed nothing after losing its HQ).
    fn hq_pos(&self, sim: &Sim) -> Option<MapCoord> {
        sim.players()[self.player as usize]
            .hq
            .and_then(|h| sim.buildings().get(h))
            .map(|b| b.pos)
            .or_else(|| self.my_buildings(sim).find(|(_, b)| b.is_ready() && b.inventory.is_some()).map(|(_, b)| b.pos))
    }

    /// Flags that are part of the road network connected to a warehouse.
    fn network(&self, sim: &Sim) -> Vec<FlagId> {
        let whs: Vec<FlagId> =
            self.my_buildings(sim).filter(|(_, b)| b.inventory.is_some()).map(|(_, b)| b.flag).collect();
        sim.flags()
            .iter()
            .filter(|(id, f)| {
                f.player == self.player && whs.iter().any(|w| *w == *id || sim.flag_distance(*id, *w).is_some())
            })
            .map(|(id, _)| id)
            .collect()
    }

    // ---------------------------------------------------------------- roads

    fn connect_flags(&mut self, sim: &Sim) -> Option<Cmd> {
        let g = sim.world().grid();
        let net = self.network(sim);
        for (id, f) in sim.flags().iter() {
            if f.player != self.player || net.contains(&id) {
                continue;
            }
            if self.blocked.iter().any(|(p, _)| *p == f.pos) {
                continue;
            }
            // nearest connected flag
            let mut targets: Vec<(u32, MapCoord)> = net
                .iter()
                .map(|n| sim.flag(*n).pos)
                .map(|p| (g.distance(p, f.pos), p))
                .filter(|(d, _)| *d <= 14)
                .collect();
            targets.sort();
            // round farmland where it can (RTTR: farmed nodes are kept free)
            let farmland: Vec<MapCoord> = self
                .my_buildings(sim)
                .filter(|(_, b)| b.kind == B::Farm)
                .flat_map(|(_, b)| g.within(b.pos, 2))
                .filter(|q| g.distance(*q, f.pos) > 1)
                .collect();
            for (_, t) in targets.into_iter().take(4) {
                let around: Vec<MapCoord> = farmland.iter().copied().filter(|q| g.distance(*q, t) > 1).collect();
                let path =
                    sim.road_path(self.player, f.pos, t, &around).or_else(|| sim.road_path(self.player, f.pos, t, &[]));
                if let Some(dirs) = path {
                    // the road must not cut through the flag's own building
                    return Some(Cmd::BuildRoad { from: f.pos, dirs });
                }
            }
            self.blocked.push((f.pos, sim.gf().0 + 1500));
            // cannot connect: give up this building
            // (never the headquarters, nor a harbor: an expedition's colony is the root of its
            // own road network, RTTR only ever connects sites; veteran round 6, #2)
            // (only a site: a finished building cut off keeps its soldiers and its land; the AI
            // razed whole realms after their HQ fell, veteran round 9, #1)
            // (a harbor site of our own placing is given up like any other: Rome's sat unbuilt
            // for an hour, cut off from its roads; game design round 1, #7)
            let colony = |b: &rr_sim::Building| matches!(&b.state, rr_sim::BState::Site(s) if s.from_sea);
            if let Some(b) = f.building
                && sim.building(b).kind != B::Headquarters
                && !(sim.building(b).kind == B::Harbor && colony(sim.building(b)))
                && !sim.building(b).is_ready()
            {
                // (a harbor not there again for a while: it came back to its spot at once)
                if sim.building(b).kind == B::Harbor {
                    self.blocked.push((sim.building(b).pos, sim.gf().0 + 20 * 60 * 20));
                }
                return Some(Cmd::RemoveFlag { at: f.pos });
            }
        }
        None
    }

    /// Now and then one flag of ours gets a short road to a nearby flag that is far away by
    /// road (RTTR BuildAlternativeRoad; veteran round 7, #5: networks were trees, and wares
    /// queued at their hub flags for many minutes).
    fn alternative_road(&mut self, sim: &Sim) -> Option<Cmd> {
        let gf = sim.gf().0;
        // every fourth thought
        let tick = gf / THINK_EVERY;
        if !(tick + u32::from(self.player)).is_multiple_of(4) {
            return None;
        }
        let g = sim.world().grid();
        let net = self.network(sim);
        let f = *net.get((tick / 4) as usize * 7 % net.len().max(1))?;
        let from = sim.flag(f).pos;
        if self.blocked.iter().any(|(p, _)| *p == from) {
            return None;
        }
        let mut best: Option<(u32, MapCoord)> = None;
        for t in &net {
            let to = sim.flag(*t).pos;
            let d = g.distance(from, to);
            if !(2..=8).contains(&d) {
                continue;
            }
            let Some(by_road) = sim.flag_distance(f, *t) else { continue };
            // a detour of three times the way and more
            if by_road >= 3 * d + 4 && best.is_none_or(|(b, _)| by_road > b) {
                best = Some((by_road, to));
            }
        }
        let (by_road, to) = best?;
        self.blocked.push((from, gf + 3000));
        let dirs = sim.road_path(self.player, from, to, &[])?;
        (2 * dirs.len() as u32 + 2 <= by_road).then_some(Cmd::BuildRoad { from, dirs })
    }

    fn split_roads(&mut self, sim: &Sim) -> Option<Cmd> {
        let g = sim.world().grid();
        for (_, r) in sim.roads().iter() {
            if r.player != self.player || r.len() < 4 {
                continue;
            }
            let nodes = r.nodes(g);
            for i in 2..nodes.len() - 2 {
                let p = nodes[i];
                if sim.world().bq_for(p, self.player) != Bq::Nothing {
                    return Some(Cmd::PlaceFlag { at: p });
                }
            }
        }
        None
    }

    // ---------------------------------------------------------------- placement

    /// Best node for a building, by `score` (higher is better). Only nodes near the network.
    fn best_spot(
        &mut self,
        sim: &Sim,
        size: Size,
        near_net: &[u8],
        score: &impl Fn(MapCoord) -> Option<i32>,
    ) -> Option<MapCoord> {
        let w = sim.world();
        let g = w.grid();
        let need = need_bq(size);
        self.hq_pos(sim)?;
        let mut best: Option<(i32, MapCoord)> = None;
        // all of our land: long countries reach far beyond any radius around the HQ
        for i in 0..self.land.len() {
            let p = self.land[i];
            if self.blocked.iter().any(|(b, _)| *b == p) {
                continue;
            }
            let bq = w.bq_for(p, self.player);
            if !bq.allows(need) {
                continue;
            }
            let d = near_net[g.index(g.neighbor(p, Direction::SE))] as u32;
            if d > NEAR_NET {
                continue;
            }
            let Some(s) = score(p) else { continue };
            // don't take castle spots for huts if avoidable
            let waste = if size == Size::Hut && bq == Bq::Castle { 3 } else { 0 };
            let total = s * 10 - d as i32 * 6 - waste + self.rng.below(4) as i32;
            if best.is_none_or(|(b, _)| total > b) {
                best = Some((total, p));
            }
        }
        best.map(|b| b.1)
    }

    fn place(&mut self, sim: &Sim, kind: B, score: impl Fn(MapCoord) -> Option<i32>) -> Option<Cmd> {
        let size = sim.rules().building(kind).size;
        // military buildings keep 4 nodes from each other (the simulation refuses closer ones)
        let me = self.player;
        let military = rr_sim::is_frontier_military(kind);
        // radius 3 around our farms is farmland: only military buildings may go there (RTTR
        // AIPlayerJH::SetFarmedNodes)
        let g0 = sim.world().grid();
        let farms: Vec<MapCoord> = if military {
            Vec::new()
        } else {
            self.my_buildings(sim).filter(|(_, b)| b.kind == B::Farm).map(|(_, b)| b.pos).collect()
        };
        let farmland = |p: MapCoord| {
            let flag = g0.neighbor(p, Direction::SE);
            farms.iter().any(|f| g0.distance(*f, p) <= 3 || g0.distance(*f, flag) <= 3)
        };
        // our harbor spots are for harbors (veteran round 7, #7: realms built over them and never
        // got to sea)
        let invader = self.invade.is_some();
        let spots: Vec<MapCoord> = if kind == B::Harbor {
            Vec::new()
        } else {
            self.land.iter().copied().filter(|p| sim.world().is_harbor_spot(*p)).collect()
        };
        let by_spot = |p: MapCoord| {
            let flag = g0.neighbor(p, Direction::SE);
            // (an invader keeps two nodes free: a harbor is a large building, and one node of room
            // left Egypt's spots built over before it went to sea, game design round 1, #7; for
            // every realm it flipped other games, the ladder held)
            let room = if invader { 2 } else { 1 };
            spots.iter().any(|s| g0.distance(*s, p) <= room || g0.distance(*s, flag) <= room)
        };
        let score = |p: MapCoord| {
            if military && sim.military_building_near(p, me) || farmland(p) || by_spot(p) { None } else { score(p) }
        };
        // try a few spots: the flag must be connectable to the road network
        let g = sim.world().grid();
        let gf = sim.gf().0;
        // nowhere to put this kind lately: don't search again for a while
        if self.no_spot.iter().any(|(k, until)| *k == kind && *until > gf) {
            return None;
        }
        let net: Vec<MapCoord> = self.network(sim).iter().map(|f| sim.flag(*f).pos).collect();
        let near_net = steps_from(g, &net, NEAR_NET);
        for _ in 0..3 {
            let Some(at) = self.best_spot(sim, size, &near_net, &score) else {
                self.no_spot.retain(|(_, until)| *until > gf);
                self.no_spot.push((kind, gf + NO_SPOT_WAIT));
                return None;
            };
            self.blocked.push((at, sim.gf().0 + 400));
            let flag = g.neighbor(at, Direction::SE);
            let mut near: Vec<(u32, MapCoord)> = net.iter().map(|n| (g.distance(*n, flag), *n)).collect();
            near.sort();
            let connectable = near.first().is_some_and(|(d, _)| *d == 0)
                || near.iter().take(3).any(|(_, n)| sim.road_path(self.player, flag, *n, &[at]).is_some());
            if connectable {
                return Some(Cmd::PlaceBuilding { at, kind });
            }
        }
        None
    }

    fn count_around(sim: &Sim, p: MapCoord, r: u16, f: impl Fn(NodeObj) -> bool) -> i32 {
        sim.world().grid().within(p, r).filter(|q| f(sim.world().obj(*q))).count() as i32
    }

    // ---------------------------------------------------------------- military

    fn military_expand(&mut self, sim: &Sim) -> Option<Cmd> {
        // (a site with nothing on its way for five minutes doesn't hold a slot: watchtowers at a
        // colony with nothing incoming blocked a realm's growth; veteran round 8, #4)
        let gf = sim.gf().0;
        let military_sites = self
            .my_buildings(sim)
            .filter(|(_, b)| !b.is_ready() && sim.rules().building(b.kind).military.is_some())
            // (a site counts its deliveries in site_incoming: QA round 5, #4)
            .filter(|(_, b)| gf < b.placed() + 6000 || b.site_incoming.0 + b.site_incoming.1 > 0)
            .count();
        let w = sim.world();
        let g = w.grid();
        let me = self.player;
        let rival = |o: u8| o != me && !sim.are_allies(o, me);
        let enemy_near = |p: MapCoord| g.within(p, 12).any(|q| w.owner(q).is_some_and(|o| o != me));
        let posts: Vec<MapCoord> = self
            .my_buildings(sim)
            .filter(|(_, b)| sim.rules().building(b.kind).is_military())
            .map(|(_, b)| b.pos)
            .collect();
        // the rivals' military buildings in sight
        let seen: Vec<MapCoord> = sim
            .buildings()
            .iter()
            .filter(|(_, b)| {
                rival(b.player)
                    && sim.rules().building(b.kind).is_military()
                    && sim.visibility(me, b.pos) == rr_sim::Visibility::Visible
            })
            .map(|(_, b)| b.pos)
            .collect();
        let seen_enemy = posts.iter().any(|p| seen.iter().any(|e| g.distance(*p, *e) <= 21));
        // a blind front: a post with rival land by it but none of their buildings in sight
        // there; nobody can attack until a bigger building pushes into the gap (veteran round 6,
        // #3). Per post: a war on one front doesn't light up another.
        // (warehouses hold land too: a harbor's colony meets rivals with no post by it)
        let holders: Vec<MapCoord> = self
            .my_buildings(sim)
            .filter(|(_, b)| b.is_ready() && b.inventory.is_some())
            .map(|(_, b)| b.pos)
            .chain(posts.iter().copied())
            .collect();
        let blind_posts: Vec<MapCoord> = holders
            .iter()
            .copied()
            .filter(|p| {
                !seen.iter().any(|e| g.distance(*p, *e) <= 21)
                    // (the owner first: walkable is six terrain lookups, perf-2 #3)
                    && g.within(*p, 14).any(|q| w.owner(q).is_some_and(rival) && w.node_walkable(q))
            })
            .collect();
        let blind = !blind_posts.is_empty();
        if blind && let Some(cmd) = self.place_lookout(sim, &blind_posts) {
            return Some(cmd);
        }
        if let Some(cmd) = self.invader_lookout(sim, &posts) {
            return Some(cmd);
        }
        let inv = sim.player_inventory(self.player);
        let (reserve, boards) = (self.spare_soldiers(sim), inv.ware(Ware::Boards));
        // more sites while soldiers and materials wait in store (RTTR WantMoreMilitaryBlds;
        // veteran round 8, #4: half the map stayed empty). (Holding hard to one site, or posts
        // back while wood is out, lost the ladder: 5:11 and 8:8.)
        let spare = reserve >= 5 && boards >= 20;
        let limit = match (self.level, gf < 3000) {
            (Level::Hard, _) => 2,
            (Level::Normal, true) => 2,
            _ => 1,
        } + usize::from(blind && self.level != Level::Easy)
            + if self.level != Level::Easy && spare && inv.ware(Ware::Stones) >= 15 {
                (reserve / 10).min(3)
            } else {
                0
            };
        if military_sites >= limit {
            return None;
        }
        // a mission's rival leaves the player room to reach the goal (the Roman campaign's
        // rule: no goal may become impossible)
        if self.land_cap.is_some_and(|cap| map_land_share(sim, self.player) >= cap) {
            return None;
        }
        if reserve <= military_sites || boards < 4 {
            return None;
        }

        // the last soldiers man the front before the land grows (veteran round 7, #3: fronts of
        // one soldier while new barracks took the rest)
        let thin: Vec<rr_sim::BuildingId> = self
            .my_buildings(sim)
            .filter(|(_, b)| {
                b.is_ready()
                    && sim
                        .rules()
                        .building(b.kind)
                        .military
                        .as_ref()
                        .is_some_and(|m| b.soldiers.len() * 2 < m.soldiers as usize)
            })
            .map(|(id, _)| id)
            .collect();
        let thin_front = sim.frontier_distances(&thin).contains(&3);
        if inv.soldiers() < 3 && thin_front && !blind {
            return None;
        }
        let lacks_iron = self.count(sim, B::IronMine) == 0;
        let lacks_gold = self.count(sim, B::GoldMine) == 0 && sim.gf().0 > 20_000;
        let near_enemy_any = self.my_buildings(sim).any(|(_, b)| enemy_near(b.pos));
        let stones = inv.ware(Ware::Stones);
        if seen_enemy
            && stones > 20
            && let Some(cmd) = self.place_catapult(sim, &seen)
        {
            return Some(cmd);
        }
        let kind = self.post_kind(blind, seen_enemy, near_enemy_any, stones, reserve, sim.gf().0);
        let radius = sim.rules().building(kind).military.as_ref().map_or(8, |m| m.radius);
        // an invader's colony grows toward its target (game design round 1, #7: Egypt's posts
        // stood 19 from Carthage's and never came closer)
        let target: Vec<MapCoord> = self.invade.map_or_else(Vec::new, |t| {
            sim.buildings()
                .iter()
                .filter(|(_, b)| b.player == t && (rr_sim::is_frontier_military(b.kind) || b.kind.is_warehouse()))
                .map(|(_, b)| b.pos)
                .collect()
        });
        let pull = |p: MapCoord| {
            target.iter().map(|t| g.distance(p, *t)).min().filter(|d| *d <= 40).map_or(0, |d| 40 - d as i32)
        };
        let gain = |p: MapCoord| -> Option<i32> {
            // close to the border, lots of unclaimed walkable land around
            let to_border = g.within(p, 4).any(|q| w.owner(q) != Some(me));
            if !to_border {
                // deeper inside only when there is no spot near the border: by the unclaimed land
                // its radius still reaches (RTTR scores spots by the land they win)
                let won = g.within(p, radius).filter(|q| w.owner(*q).is_none() && w.node_walkable(*q)).count();
                return (won >= 4).then(|| won as i32 / 6 - 1000);
            }
            let free = g.ring(p, 8).filter(|q| w.owner(*q).is_none() && w.node_walkable(*q)).count() as i32;
            // ores count, and the ores this realm has no mine for count three times (an army
            // needs iron for weapons and gold for promotions: RTTR plans for both early)
            let mountain = g
                .within(p, 8)
                .map(|q| match w.resources[g.index(q)] {
                    Resource::Iron(_) => {
                        if lacks_iron {
                            3
                        } else {
                            1
                        }
                    }
                    Resource::Gold(_) => {
                        if lacks_gold {
                            3
                        } else {
                            1
                        }
                    }
                    Resource::Coal(_) => 1,
                    _ => 0,
                })
                .sum::<i32>();
            // at a blind front, toward the rival's land first
            // right at the border: a rival post up to its radius deep is then in sight
            let push = if blind && blind_posts.iter().any(|b| g.distance(*b, p) <= 16) {
                g.within(p, 4)
                    .filter(|q| w.owner(*q).is_some_and(rival) && w.node_walkable(*q))
                    .map(|q| 46 - 8 * g.distance(p, q) as i32)
                    .max()
                    .unwrap_or(0)
            } else {
                0
            };
            Some(free / 3 + mountain.min(12) + if enemy_near(p) { 6 } else { 0 } + push + pull(p))
        };
        self.place(sim, kind, gain)
    }

    /// What military building to place next (RTTR ChooseMilitaryBuilding).
    fn post_kind(
        &mut self,
        blind: bool,
        seen_enemy: bool,
        near_enemy_any: bool,
        stones: u32,
        reserve: usize,
        gf: u32,
    ) -> B {
        // (the first 40 minutes grow with guardhouses: big posts from the start slowed normal
        // so much that easy won 2 of 16)
        let early_growth_over = gf >= EARLY_WAR;
        // RTTR ChooseMilitaryBuilding: facing rivals, a watchtower half the time, else the
        // biggest (a fortress), a guardhouse one time in ten (veteran round 6, #4)
        if blind && !seen_enemy && stones > 10 {
            // into a blind gap a watchtower: it sees far and fits where a fortress doesn't
            B::Watchtower
        } else if (blind || seen_enemy) && stones > 10 {
            if self.rng.chance(10) {
                B::Guardhouse
            } else if self.rng.chance(50) || stones < 20 {
                B::Watchtower
            } else {
                B::Fortress
            }
        } else if near_enemy_any && stones > 10 && self.rng.chance(40) {
            B::Watchtower
        } else if early_growth_over && reserve >= 15 && stones > 20 && !self.rng.chance(33) {
            // soldiers to spare: the biggest (RTTR ChooseMilitaryBuilding: a guardhouse only
            // under 15 privates or one time in three; veteran round 10, #5: 57 guardhouses
            // and 1 fortress, fronts one soldier deep)
            if self.rng.chance(50) { B::Fortress } else { B::Watchtower }
        } else if stones > 6 {
            B::Guardhouse
        } else {
            B::Barracks
        }
    }

    /// A lookout tower by a blind gap sees 20 into it (RTTR VISUALRANGE_LOOKOUTTOWER): posts
    /// whose land meets at 18 apart see only 12-14.
    /// An invader whose posts stand near its target without seeing it builds a lookout tower
    /// toward it (it sees 20): Egypt's colony stood 15-20 from Carthage's posts, blind, with no
    /// rival land by it for the blind-front rule (game design round 1, #7).
    fn invader_lookout(&mut self, sim: &Sim, posts: &[MapCoord]) -> Option<Cmd> {
        let t = self.invade?;
        let g = sim.world().grid();
        let target: Vec<MapCoord> = sim
            .buildings()
            .iter()
            .filter(|(_, b)| b.player == t && (rr_sim::is_frontier_military(b.kind) || b.kind.is_warehouse()))
            .map(|(_, b)| b.pos)
            .collect();
        let me = self.player;
        if target.iter().any(|p| sim.visibility(me, *p) == rr_sim::Visibility::Visible) {
            return None;
        }
        let near: Vec<MapCoord> =
            posts.iter().copied().filter(|p| target.iter().any(|t| g.distance(*t, *p) <= 25)).collect();
        if near.is_empty()
            || sim.player_inventory(me).ware(Ware::Boards) < 6
            || self
                .my_buildings(sim)
                .any(|(_, b)| b.kind == B::LookoutTower && near.iter().any(|p| g.distance(*p, b.pos) <= 12))
        {
            return None;
        }
        self.place(sim, B::LookoutTower, |p| {
            if !near.iter().any(|b| g.distance(*b, p) <= 10) {
                return None;
            }
            let d = target.iter().map(|t| g.distance(*t, p)).min()?;
            Some(40 - d as i32)
        })
    }

    fn place_lookout(&mut self, sim: &Sim, blind_posts: &[MapCoord]) -> Option<Cmd> {
        let w = sim.world();
        let g = w.grid();
        let me = self.player;
        let rival = |o: u8| o != me && !sim.are_allies(o, me);
        // (counting those by today's blind posts: the fronts move on)
        let lookouts = self
            .my_buildings(sim)
            .filter(|(_, b)| b.kind == B::LookoutTower && blind_posts.iter().any(|p| g.distance(*p, b.pos) <= 16))
            .count();
        let lookout_site = self.my_buildings(sim).any(|(_, b)| b.kind == B::LookoutTower && !b.is_ready());
        if !lookout_site
            && lookouts < (1 + blind_posts.len() / 3).min(4)
            && sim.player_inventory(me).ware(Ware::Boards) >= 6
            && let Some(cmd) = self.place(sim, B::LookoutTower, |p| {
                // near a blind post, right by the rival's land, not by a lookout we have
                if !blind_posts.iter().any(|b| g.distance(*b, p) <= 12) {
                    return None;
                }
                g.within(p, 3)
                    .filter(|q| w.owner(*q).is_some_and(rival) && w.node_walkable(*q))
                    .map(|q| 10 - 3 * g.distance(p, q) as i32)
                    .max()
            })
        {
            return Some(cmd);
        }
        None
    }

    /// Facing rivals in sight (`seen`), now and then a catapult within its range of 6-12 (RTTR
    /// ChooseMilitaryBuilding: one time in eight; veteran round 6, #4); never in a truce, never
    /// on easy (new-player round 8, #8).
    fn place_catapult(&mut self, sim: &Sim, seen: &[MapCoord]) -> Option<Cmd> {
        if sim.gf().0 < self.peace_until
            || self.level == Level::Easy
            || self.count(sim, B::Catapult) >= 3
            || !self.rng.chance(12)
        {
            return None;
        }
        // in a truce, only out of range of the realm it spares: a catapult fires at whatever it
        // reaches (veteran round 11, #2: any truce had stopped catapults against the others too)
        let spared: Vec<MapCoord> = match self.truce {
            Some((p, until)) if sim.gf().0 < until => {
                sim.buildings().iter().filter(|(_, b)| b.player == p).map(|(_, b)| b.pos).collect()
            }
            _ => Vec::new(),
        };
        let g = sim.world().grid();
        self.place(sim, B::Catapult, |p| {
            if spared.iter().any(|s| g.distance(*s, p) <= 16) {
                return None;
            }
            let d = seen.iter().map(|e| g.distance(*e, p)).min().unwrap_or(99) as i32;
            (6..=13).contains(&d).then_some(20 - d)
        })
    }

    fn attack(&mut self, sim: &Sim) -> Option<Cmd> {
        if sim.gf().0 < self.peace_until {
            return None;
        }
        let first = match self.level {
            Level::Easy => 40_000,
            Level::Normal => 9_000,
            Level::Hard => 6_000,
        };
        if sim.gf().0 < first {
            return None;
        }
        // in the first 40 minutes no war that empties the castle: every attacker leaves a post
        // the castle refills (veteran round 10, #2: two fronts at once took a store of 46 to 1
        // and the castle fell at minute 16)
        // (an empty store refills nothing: then attacking costs the castle nothing more)
        let store = sim.player_inventory(self.player).soldiers() as usize;
        if sim.gf().0 < EARLY_WAR && (1..CASTLE_KEEP).contains(&store) {
            return None;
        }
        let g = sim.world().grid();
        let me = self.player;
        let hp = |f: rr_sim::FigId| sim.figure(f).hp as usize;
        // targets our soldiers are already marching on: no second trickle behind the first
        let marching: std::collections::BTreeSet<rr_sim::BuildingId> = sim
            .figures()
            .iter()
            .filter(|(_, f)| f.player == me)
            .filter_map(|(_, f)| match f.kind {
                rr_sim::FigKind::Soldier(
                    rr_sim::SoldierTask::Attack { target, .. } | rr_sim::SoldierTask::ToShip { target, .. },
                ) => Some(target),
                _ => None,
            })
            .collect();
        // soldiers that can leave (each building keeps its weakest one), strongest first
        let mine: Vec<(rr_sim::BuildingId, MapCoord, Vec<usize>)> = self
            .my_buildings(sim)
            .filter(|(_, b)| b.is_ready() && sim.rules().building(b.kind).is_military())
            .map(|(id, b)| {
                let mut s: Vec<usize> = b.soldiers.iter().map(|f| hp(*f)).collect();
                s.sort_unstable_by(|a, b| b.cmp(a));
                (id, b.pos, s)
            })
            .collect();
        let mut candidates: Vec<(i32, MapCoord, usize, usize, bool)> = Vec::new();
        // a ship of ours free for an attack by sea
        let sea_ready = sim.ships().iter().any(|(_, s)| s.player == me && s.task == rr_sim::ShipTask::Idle);
        // a campaign player's castle only once its posts have fallen: everything a newcomer owns
        // lies in it (new-player round 10, #1: taken at minute 73 in two of three games); not at
        // hard, which spares nobody (game design round 1, #3: no hidden holds there)
        let player_posts = self.truce.map_or(0, |(p, _)| {
            sim.buildings()
                .iter()
                .filter(|(_, b)| b.player == p && b.is_ready() && rr_sim::is_frontier_military(b.kind))
                .count()
        });
        for (id, t) in sim.buildings().iter() {
            // only what a player can see and may attack (not empty new buildings)
            if !sim.is_attackable(me, id) || marching.contains(&id) {
                continue;
            }
            // (the castle 15 minutes longer: normal rivals took a newcomer's four minutes after
            // the truce; new-player round 8, #5)
            // (and the player of a campaign at most once in four minutes: five allies at normal pace
            // overran a newcomer after the first win, eval 46; one attack round per window)
            if self.truce.is_some_and(|(p, until)| {
                t.player == p
                    && (sim.gf().0 < until
                        || sim.gf().0 < t.occupied_at + GRACE
                        || t.kind == B::Headquarters
                            && (sim.gf().0 < until + CASTLE_GRACE || player_posts > 0 && self.level != Level::Hard)
                        || sim.gf().0 % CAMPAIGN_ATTACK_WINDOW >= self.attack_period().min(CAMPAIGN_ATTACK_WINDOW)
                        || !self.my_campaign_turn(sim, p))
            }) {
                continue;
            }
            let def = sim.rules().building(t.kind);
            if !(def.is_military() || t.kind.is_warehouse()) {
                continue;
            }
            // the same reach the simulation allows attackers
            let reach = sim.rules().military.attack_radius as u32 + 10;
            // what each building may send (settings, distance), strongest first
            let mut avail: Vec<usize> = mine
                .iter()
                .map(|(id, p, s)| (g.distance(*p, t.pos), id, s))
                .filter(|(d, _, _)| *d <= reach)
                .flat_map(|(d, id, s)| s.iter().copied().take(sim.attackers_available(*id, d)))
                .collect();
            // nobody near: perhaps from the sea, with a ship free (owner, 2026-10-27); not in
            // the first 40 minutes, as nothing else empties a castle then (a Far East castle fell
            // to two sailors at minute 39)
            if avail.is_empty() && sea_ready && sim.gf().0 >= EARLY_WAR {
                avail = self.sea_attackers(sim, id, &mine);
            }
            if avail.is_empty() {
                continue;
            }
            avail.sort_unstable_by(|a, b| b.cmp(a));
            // ranks inside are hidden: assume an average soldier in a post, a private in a
            // warehouse (late in a game those are mostly recruits; veteran round 8, #1)
            let typical = sim.rules().military.hp[1] as usize;
            let recruit = sim.rules().military.hp[0] as usize;
            // a warehouse is worn down in waves: half its soldiers' worth will do, as each wave
            // kills defenders who don't come back (RTTR TryToAttack never strength-checks a
            // castle or harbor; veteran round 10, #4: a castle of 7 lived 80 minutes beside 4
            // attackers)
            let defense: usize = if let Some(inv) = &t.inventory {
                Job::SOLDIERS.iter().map(|j| inv.job(*j) as usize).sum::<usize>() * recruit / 2
            } else {
                t.soldiers.len() * typical
            };
            // only with enough strength to beat the defense with a margin
            let mut sum = 0;
            // Easy wants 1.5x the defense, Normal and Hard a little more than it (hard at 0.8x
            // bled its army in long games: 6:10 against normal at 120 minutes; item 317)
            let need = match self.level {
                Level::Easy => defense * 3 / 2 + 2,
                Level::Normal => defense + 1,
                Level::Hard => defense + 1,
            };
            for s in &avail {
                if sum > need {
                    break;
                }
                sum += s;
            }
            if sum <= need {
                continue;
            }
            // then everybody who can go (RTTR TryToAttack sends all available attackers);
            // undefended warehouses first (RTTR hq_or_harbor_without_soldiers)
            let n = avail.len() as i32;
            let empty = if t.inventory.is_some() && defense == 0 { 1000 } else { 0 };
            let score = sum as i32 - defense as i32 * 2 + if t.kind == B::Headquarters { 10 } else { 0 } - n + empty;
            // (in measure: easy always, and a campaign's rivals for 30 minutes after the truce;
            // new-player round 9, #2: 27 against a post of one made the second campaign a cliff)
            // (and the campaign player's castle always: everything a newcomer owns lies in it,
            // and 10-21 against a castle of 4-6 took it at minute 73; new-player round 10, #1)
            let measured = self.level == Level::Easy
                || self.truce.is_some_and(|(p, until)| {
                    t.player == p && (sim.gf().0 < until + CAMPAIGN_MEASURE || t.kind == B::Headquarters)
                });
            candidates.push((score, t.pos, need, defense * 3 / 2 + 2, measured));
        }
        // the best few targets our soldiers can actually walk to (checking is costly: targets
        // found out of reach are left alone for a while)
        let gf = sim.gf().0;
        self.out_of_reach.retain(|(_, until)| *until > gf);
        candidates.retain(|(_, at, _, _, _)| !self.out_of_reach.iter().any(|(p, _)| p == at));
        candidates.sort_by_key(|(score, at, _, _, _)| (std::cmp::Reverse(*score), at.x, at.y));
        // judged again on the soldiers that can walk there: posts across water or too far by
        // road counted above but don't go (veteran round 8, #1: won fights were dropped whole)
        for (_, at, need, easy_need, measured) in candidates.into_iter().take(3) {
            if sim.gf().0 < EARLY_WAR && sim.attack_goes_by_sea(me, at) {
                continue;
            }
            let can_go = sim.attack_strength(me, at);
            if can_go.iter().map(|h| *h as usize).sum::<usize>() > need {
                // easy sends what its rule asks for, the strongest first; the others all who can
                // go (new-player round 8, #1: 22 easy attackers against a guardhouse of 3)
                let n = if measured {
                    let mut sum = 0;
                    can_go
                        .iter()
                        .take_while(|h| {
                            let before = sum;
                            sum += **h as usize;
                            before <= need.max(easy_need)
                        })
                        .count()
                } else {
                    can_go.len()
                };
                return Some(Cmd::Attack { target: at, soldiers: n.min(255) as u8, strong: true });
            }
            self.out_of_reach.push((at, gf + OUT_OF_REACH_WAIT));
        }
        None
    }

    /// The soldiers' hit points an attack by sea on `t` could take, strongest first, per post as
    /// on land (`mine`: our posts and their soldiers' hit points, strongest first).
    fn sea_attackers(
        &self,
        sim: &Sim,
        t: rr_sim::BuildingId,
        mine: &[(rr_sim::BuildingId, MapCoord, Vec<usize>)],
    ) -> Vec<usize> {
        let Some(plan) = sim.sea_attack_plan(self.player, t) else { return Vec::new() };
        plan.sources
            .iter()
            .filter_map(|(d, b)| mine.iter().find(|(id, _, _)| id == b).map(|(_, _, s)| (d, b, s)))
            .flat_map(|(d, b, s)| s.iter().copied().take(sim.attackers_available(*b, *d)))
            .collect()
    }

    // ---------------------------------------------------------------- economy

    #[allow(clippy::too_many_lines)] // to split (architect round 2, #8: no new long ones)
    fn economy(&mut self, sim: &Sim) -> Option<Cmd> {
        // two sites at a time finish sooner than more (materials are not spread thin); the
        // easy AI builds one thing at a time
        // (more once boards and stones pile up: a realm of 130 buildings with 200 boards in store
        // built its farms and mills two at a time)
        let stock = sim.player_inventory(self.player);
        let spare = (stock.ware(Ware::Boards) / 50).min(stock.ware(Ware::Stones) / 25).min(2) as usize;
        let max_sites = match self.level {
            Level::Easy => 1,
            Level::Normal => 2 + spare,
            Level::Hard => 3 + spare,
        };
        if self.sites(sim) >= max_sites {
            return None;
        }
        let inv = sim.player_inventory(self.player);
        // a forester with no woodcutter by it plants for nobody (veteran round 7: realms of
        // eighty buildings on three woodcutters, two foresters idle among their trees)
        // (only one woodcutter, right by it: one anywhere else left it lonely and fired again at
        // every free slot, 60-90 woodcutters a realm; veteran round 8, #2)
        let lonely: Option<MapCoord> = {
            let g = sim.world().grid();
            let cutters: Vec<MapCoord> =
                self.my_buildings(sim).filter(|(_, b)| b.kind == B::Woodcutter).map(|(_, b)| b.pos).collect();
            self.my_buildings(sim)
                .find(|(_, b)| {
                    b.kind == B::Forester && b.is_ready() && !cutters.iter().any(|c| g.distance(*c, b.pos) <= 6)
                })
                .map(|(_, b)| b.pos)
        };
        // woodcutters in step with the foresters (RTTR BuildingPlanner: 2 × foresters + 1), and
        // none while wood piles up (balance would stop it at once)
        let cutters = self.count(sim, B::Woodcutter);
        let more_cutters = cutters < 2 || inv.ware(Ware::Wood) <= 40 && cutters < 2 * self.count(sim, B::Forester) + 3;
        let lonely = lonely.filter(|_| more_cutters);
        if let Some(f) = lonely
            && inv.ware(Ware::Boards) >= 2
            && let Some(cmd) = self.place_woodcutter_by(sim, f)
        {
            return Some(cmd);
        }
        if inv.ware(Ware::Boards) < 2 {
            // only wood production helps now
            if let Some(f) = lonely {
                return self.place_woodcutter_by(sim, f);
            }
            if cutters < 2 {
                return self.place_woodcutter(sim);
            }
            if self.count(sim, B::Sawmill) < 1 || inv.ware(Ware::Wood) > 40 && self.count(sim, B::Sawmill) < 4 {
                return self.place(sim, B::Sawmill, |_| Some(0));
            }
            return None;
        }
        let c = |k: B| self.count(sim, k);
        let w = sim.world();
        let g = w.grid();
        let mines_ok = |p: MapCoord, f: fn(Resource) -> bool| -> Option<i32> {
            let n = g.within(p, 2).filter(|q| f(w.resources[g.index(*q)])).count() as i32;
            (n > 0).then_some(n)
        };
        let food_buildings = c(B::Fishery) + c(B::Hunter) + c(B::Bakery) + c(B::Slaughterhouse);
        let quarry_working = self.my_buildings(sim).any(|(_, b)| {
            b.kind == B::Quarry && (!b.is_ready() || b.productivity() > 0 || sim.gf().0 < b.placed() + 6000)
        });
        let military_count = self
            .my_buildings(sim)
            .filter(|(_, b)| sim.rules().building(b.kind).is_military() && b.kind != B::Headquarters)
            .count();
        let total = self.my_buildings(sim).count();
        // the food chain grows with the realm once mines eat (RTTR BuildingPlanner): mills for the
        // grain the farms leave, a bakery per mill, pig farms by the farms, wells for all of them,
        // fisheries by the posts (200-minute games: farms capped at 8 fed 11 mines at 2-46 %)
        let feeding = self.level != Level::Easy
            && c(B::CoalMine) + c(B::IronMine) + c(B::GoldMine) > 0
            && inv.ware(Ware::Fish) + inv.ware(Ware::Bread) + inv.ware(Ware::Meat) < 40;
        let food_users = c(B::Mill) + c(B::Brewery) + c(B::PigFarm) + c(B::DonkeyBreeder);
        let mills_wanted = if c(B::Farm) + c(B::Mill) >= food_users {
            (c(B::Farm) + c(B::Mill) - food_users).min(c(B::Bakery) + 1).min(6)
        } else {
            c(B::Mill)
        };
        let pigs_wanted =
            (if c(B::Farm) < 8 { c(B::Farm) / 4 } else { (c(B::Farm) - 2) / 4 }).min(c(B::Slaughterhouse) + 1).min(4);
        let iron_mines_busy = self
            .my_buildings(sim)
            .filter(|(_, b)| b.kind == B::IronMine)
            .all(|(_, b)| b.is_ready() && b.productivity() >= 80);
        let wish: Vec<(B, bool)> = vec![
            // wood piling up by the hundred while the sites wait for boards
            (B::Sawmill, inv.ware(Ware::Wood) > 40 && inv.ware(Ware::Boards) < 20 && c(B::Sawmill) < 4),
            (B::Woodcutter, c(B::Woodcutter) < 2),
            (B::Quarry, c(B::Quarry) < 1),
            (B::Forester, c(B::Forester) < 1),
            (B::Sawmill, c(B::Sawmill) < 1),
            // a spare sawmill early: losing the only one with no boards in stock deadlocks
            (B::Sawmill, c(B::Sawmill) < 2 && c(B::Woodcutter) >= 3),
            (B::Woodcutter, c(B::Woodcutter) < 3),
            // early, whatever the stock: tools run out, and new workers need them (veteran round 3)
            (B::Metalworks, c(B::Metalworks) < 1 && c(B::Sawmill) > 0),
            (B::Fishery, c(B::Fishery) < 1),
            (B::Hunter, c(B::Hunter) < 1),
            (B::Well, c(B::Well) < 1),
            // fields take minutes to grow: the farms before the mines that will eat their grain
            // (hard opens with the mines: its early pressure is what makes it hard)
            // (item 306: the first farm stood at minute 17-23, grain came late to everything)
            (B::Farm, c(B::Farm) < 2),
            (B::Forester, c(B::Forester) < 2),
            // quarries grow with the realm (RTTR BuildingPlanner: 2-6 by military buildings)
            (B::Quarry, c(B::Quarry) < (2 + military_count / 4).min(6) && inv.ware(Ware::Stones) < 100),
            // stone from the mountain as soon as the rocks give out (RTTR: granite mines ahead of
            // every workshop; veteran round 7, #2: it was the last wish of all)
            (
                B::GraniteMine,
                c(B::GraniteMine) < military_count / 15 + 1 && !quarry_working && inv.ware(Ware::Stones) < 100,
            ),
            // coal before anything but wood and stone when ore waits for it (veteran round 7, #3)
            (B::CoalMine, inv.ware(Ware::Coal) < 5 && inv.ware(Ware::IronOre) > 20 && c(B::CoalMine) < 6),
            (B::CoalMine, c(B::CoalMine) < 1),
            (B::IronMine, c(B::IronMine) < 1),
            (B::Farm, c(B::Farm) < 2),
            (B::Mill, c(B::Mill) < 1 && c(B::Farm) > 0),
            (B::Bakery, c(B::Bakery) < 1 && c(B::Mill) > 0),
            (B::IronSmelter, c(B::IronSmelter) < 1 && c(B::IronMine) > 0),
            (B::Brewery, c(B::Brewery) < 1 && c(B::Farm) > 0),
            (B::Armory, c(B::Armory) < 1 && c(B::IronSmelter) > 0),
            // the weapon chain as a unit, ahead of the extras (RTTR BuildingPlanner: smelters =
            // iron mines, armories = smelters; veteran round 9, #2: a smelter short, armies that
            // only shrank)
            (B::IronSmelter, c(B::IronSmelter) < c(B::IronMine) && self.level != Level::Easy),
            (B::Armory, c(B::Armory) < c(B::IronSmelter) && self.level != Level::Easy),
            // the mines keep up and food piles up: one more iron mine
            (
                B::IronMine,
                self.level != Level::Easy
                    && c(B::IronMine) > 0
                    && c(B::IronMine) < 6
                    && iron_mines_busy
                    && inv.ware(Ware::Fish) + inv.ware(Ware::Bread) + inv.ware(Ware::Meat) > 40,
            ),
            (B::PigFarm, c(B::PigFarm) < 1 && c(B::Farm) > 1),
            (B::Slaughterhouse, c(B::Slaughterhouse) < 1 && c(B::PigFarm) > 0),
            (B::Well, c(B::Well) < 2 && c(B::Bakery) > 0),
            // gold and the mint once coal flows (RTTR BuildingPlanner wants them early)
            (B::GoldMine, c(B::GoldMine) < 1 && c(B::CoalMine) > 0),
            (B::Mint, c(B::Mint) < c(B::GoldMine).min(2)),
            (B::CoalMine, c(B::CoalMine) < 2 && c(B::Armory) > 0),
            (B::Fishery, c(B::Fishery) < 3 && food_buildings < 6),
            (B::Hunter, c(B::Hunter) < 2 && c(B::CoalMine) > 0),
            (B::Bakery, c(B::Bakery) < 2 && c(B::Farm) > 2),
            (B::Mill, c(B::Mill) < 2 && c(B::Bakery) > 1),
            (B::Well, c(B::Well) < 3 && c(B::Bakery) > 1 && inv.ware(Ware::Water) < 20),
            (B::Slaughterhouse, c(B::Slaughterhouse) < 2 && c(B::PigFarm) > 1),
            (B::PigFarm, c(B::PigFarm) < 2 && c(B::Farm) > 3),
            (B::Farm, c(B::Farm) < 6 && c(B::Mill) > 1),
            (B::Forester, c(B::Forester) < 3 && c(B::Woodcutter) > 2),
            (B::IronMine, c(B::IronMine) < 2 && c(B::IronSmelter) > 0 && c(B::Farm) > 3),
            // the metal chain grows with the iron mines (RTTR BuildingPlanner): a smelter per iron
            // mine, an armory per smelter, coal for both and for gold
            // (the easy computer stays at one of each: a gentler opponent)
            (B::IronSmelter, c(B::IronSmelter) < c(B::IronMine) && self.level != Level::Easy),
            // a big farming realm digs more iron (RTTR BuildingPlanner: min(farms / 2, smelters + 1))
            (
                B::IronMine,
                c(B::Farm) > 7
                    && c(B::IronMine) < (c(B::Farm) / 2).min(c(B::IronSmelter) + 1)
                    && self.level != Level::Easy,
            ),
            (B::Armory, c(B::Armory) < c(B::IronSmelter) && self.level != Level::Easy),
            (B::CoalMine, c(B::CoalMine) < (2 * c(B::IronMine) + c(B::GoldMine)).saturating_sub(1).min(food_buildings)),
            (B::Farm, c(B::Farm) < 4 && c(B::PigFarm) > 0),
            (B::Woodcutter, c(B::Woodcutter) < 3 + total / 10 && inv.ware(Ware::Boards) < 60),
            (B::Forester, c(B::Forester) < 1 + c(B::Woodcutter) / 2),
            // (RTTR BuildingPlanner: 1 + armories / 5)
            (B::Brewery, c(B::Brewery) < 1 + c(B::Armory) / 5 && c(B::Farm) > 3),
            // donkeys for the busy roads of a big realm (RTTR BuildingPlanner: over 7 farms)
            (B::DonkeyBreeder, c(B::DonkeyBreeder) < 1 && c(B::Farm) > 6),
            (B::Farm, c(B::Farm) < 8 && sim.gf().0 > 40_000),
            (B::Sawmill, c(B::Sawmill) < 1 + c(B::Woodcutter) / 3),
            (B::Storehouse, c(B::Storehouse) < 1 + total / 35 && total > 30),
            // (past 7 farms the donkeys come first: the roads get busy)
            (B::Farm, feeding && c(B::Farm) < (food_users + 3).min(16) && (c(B::Farm) < 7 || c(B::DonkeyBreeder) > 0)),
            (B::Mill, feeding && c(B::Mill) < mills_wanted),
            (B::Bakery, feeding && c(B::Bakery) < c(B::Mill)),
            (B::PigFarm, feeding && c(B::PigFarm) < pigs_wanted),
            (B::Slaughterhouse, feeding && c(B::Slaughterhouse) < c(B::PigFarm)),
            (B::Well, feeding && c(B::Well) < c(B::Bakery) + c(B::PigFarm) + c(B::Brewery)),
            (B::Fishery, feeding && c(B::Fishery) < (military_count / 2 + 2).min(10)),
            (B::Hunter, feeding && c(B::Hunter) < 4),
            (
                B::Catapult,
                self.level != Level::Easy
                    && c(B::Catapult) < 2
                    && inv.ware(Ware::Stones) > 40
                    && sim.gf().0 > 30_000
                    // a catapult can't tell who is under truce: none until it is over
                    && self.truce.is_none_or(|(_, until)| sim.gf().0 >= until),
            ),
            (B::CoalMine, c(B::CoalMine) < 4 && inv.ware(Ware::Coal) < 5 && inv.ware(Ware::IronOre) > 20),
            (
                B::Fishery,
                c(B::Fishery) < 5
                    && inv.ware(Ware::Fish) + inv.ware(Ware::Bread) + inv.ware(Ware::Meat) < 5
                    && c(B::CoalMine) > 1,
            ),
        ];
        // food follows its users (RTTR BuildingPlanner): farms for the grain users, coal mines for
        // the food there is, and without any mine one bakery and one slaughterhouse
        let grain_users = c(B::Mill) + c(B::PigFarm) + c(B::Brewery) + c(B::DonkeyBreeder);
        let mines = c(B::CoalMine) + c(B::IronMine) + c(B::GoldMine) + c(B::GraniteMine);
        let food_short = inv.ware(Ware::Fish) + inv.ware(Ware::Bread) + inv.ware(Ware::Meat) < 5;
        let (farms, coal_mines, fisheries) = (c(B::Farm), c(B::CoalMine), c(B::Fishery));
        let food_counts: Vec<(B, usize)> =
            [B::Bakery, B::Slaughterhouse, B::Mill, B::PigFarm].into_iter().map(|k| (k, c(k))).collect();
        let allowed = |k: B| match k {
            B::Farm => farms < grain_users + 3,
            B::CoalMine if food_short => coal_mines < (farms + fisheries) / 2 + 2,
            B::Bakery | B::Slaughterhouse | B::Mill | B::PigFarm if mines == 0 => {
                food_counts.iter().any(|(f, n)| *f == k && *n < 1)
            }
            _ => true,
        };
        // no building that needs stone while there is none and none coming (veteran round 7, #2:
        // such sites stood unfinished by the dozen, each with boards and a builder)
        let stone_income = quarry_working
            || self.my_buildings(sim).any(|(_, b)| b.kind == B::GraniteMine && b.is_ready() && b.productivity() > 0);
        let stoneless = inv.ware(Ware::Stones) == 0 && !stone_income;
        for (kind, wanted) in wish {
            if !wanted
                || !allowed(kind)
                || stoneless && sim.rules().building(kind).stones > 0
                || kind == B::Woodcutter && !more_cutters
            {
                continue;
            }
            let cmd = match kind {
                B::Woodcutter => self.place_woodcutter(sim),
                B::Quarry => self.place(sim, kind, |p| {
                    let n = Self::count_around(sim, p, 6, |o| matches!(o, NodeObj::Granite { .. }));
                    (n > 0).then_some(n * 2)
                }),
                B::Forester => {
                    // trees planted on farmland take the farmer's fields
                    let farms: Vec<MapCoord> =
                        self.my_buildings(sim).filter(|(_, b)| b.kind == B::Farm).map(|(_, b)| b.pos).collect();
                    let reach = sim.rules().economy.forester_radius as u32 + 2;
                    self.place(sim, kind, |p| {
                        if farms.iter().any(|f| g.distance(*f, p) <= reach) {
                            return None;
                        }
                        let free = g.within(p, 5).filter(|q| w.obj(*q) == NodeObj::None).count() as i32;
                        (free > 25).then_some(free / 4)
                    })
                }
                B::Fishery => self.place(sim, kind, |p| {
                    let n =
                        // (fish left, not fished-out grounds: QA round 5, #6)
                        g.within(p, 6).filter(|q| matches!(w.resources[g.index(*q)], Resource::Fish(a) if a > 0)).count()
                            as i32;
                    (n > 2).then_some(n)
                }),
                B::Hunter => {
                    // only where game is in reach (RTTR AIPlayerJH HuntablesInRange)
                    let reach = sim.rules().economy.hunter_radius as u32;
                    let game = GameNear::new(
                        &g,
                        sim.figures()
                            .iter()
                            .filter(|(_, f)| matches!(f.kind, rr_sim::FigKind::Animal { .. }))
                            .map(|(_, f)| f.pos),
                        reach,
                    );
                    self.place(sim, kind, |p| {
                        let n = game.count(&g, p, reach) as i32;
                        (n >= 2).then_some(n.min(10))
                    })
                }
                B::Farm => self.place(sim, kind, |p| {
                    let free =
                        g.within(p, 2).filter(|q| w.obj(*q) == NodeObj::None && !w.is_on_road(*q)).count() as i32;
                    (free > 8).then_some(free)
                }),
                B::CoalMine => self.place(sim, kind, |p| mines_ok(p, |r| matches!(r, Resource::Coal(_)))),
                B::IronMine => self.place(sim, kind, |p| mines_ok(p, |r| matches!(r, Resource::Iron(_)))),
                B::GoldMine => self.place(sim, kind, |p| mines_ok(p, |r| matches!(r, Resource::Gold(_)))),
                B::GraniteMine => self.place(sim, kind, |p| mines_ok(p, |r| matches!(r, Resource::Granite(_)))),
                B::Storehouse => {
                    // spread warehouses out: far from the existing ones, near the action
                    let whs: Vec<MapCoord> =
                        self.my_buildings(sim).filter(|(_, b)| b.inventory.is_some()).map(|(_, b)| b.pos).collect();
                    self.place(sim, kind, |p| {
                        let d = whs.iter().map(|w| g.distance(*w, p)).min().unwrap_or(99) as i32;
                        (d >= 16).then_some(d.min(30))
                    })
                }
                B::Catapult => {
                    // within range (13) of an enemy garrison
                    let me = self.player;
                    // the same targets the catapult will take: seen, occupied military buildings
                    let enemies: Vec<MapCoord> = sim
                        .buildings()
                        .iter()
                        .filter(|(_, b)| {
                            !sim.are_allies(b.player, me)
                                && rr_sim::is_frontier_military(b.kind)
                                && b.occupied
                                && sim.visibility(me, b.pos) == rr_sim::Visibility::Visible
                        })
                        .map(|(_, b)| b.pos)
                        .collect();
                    self.place(sim, kind, |p| {
                        let d = enemies.iter().map(|e| g.distance(*e, p)).min().unwrap_or(99) as i32;
                        (6..=12).contains(&d).then_some(20 - d)
                    })
                }
                _ => self.place(sim, kind, |_| Some(0)),
            };
            if cmd.is_some() {
                return cmd;
            }
        }
        None
    }

    /// Harbor on an owned spot, a shipyard next to it, then expeditions to free spots.
    fn seafaring(&mut self, sim: &Sim) -> Option<Cmd> {
        if sim.gf().0 < 25_000 {
            return None;
        }
        let me = self.player;
        let w = sim.world();
        let g = w.grid();
        let harbors: Vec<(rr_sim::BuildingId, &rr_sim::Building)> =
            self.my_buildings(sim).filter(|(_, b)| b.kind == B::Harbor).collect();
        // (not where a site of ours was given up lately: cut off from the roads, Rome's harbor
        // came back to its spot at once and waited for an hour; game design round 1, #7)
        let blocked: Vec<MapCoord> = self.blocked.iter().map(|b| b.0).collect();
        let given_up = |p: &MapCoord| blocked.contains(p);
        if harbors.is_empty() {
            let spot = g.coords().find(|p| {
                w.is_harbor_spot(*p) && w.bq_for(*p, me) == Bq::Harbor && sim.harbor_sea(*p).is_some() && !given_up(p)
            });
            return spot.map(|at| Cmd::PlaceBuilding { at, kind: B::Harbor });
        }
        let &(hid, hb) = harbors.iter().find(|(_, b)| b.is_ready())?;
        let inv = sim.player_inventory(me);
        if self.count(sim, B::Shipyard) == 0 {
            // within 7 of any ready harbor (RTTR IsInvalidShipyardPosition); with no room by
            // any, another harbor where there is (veteran round 10, #1: Britain's one cramped
            // harbor meant no ship all game, and 66 soldiers idle)
            // (or, with boards to spare, farther, wherever the shipwright reaches the harbor's
            // sea: Rome's harbors stood on mountain coasts with no room within 7 and 199 boards
            // idle; game design round 1, #7. Without the spare boards it cost hard the ladder,
            // 14:18 against 21:11)
            let ready: Vec<MapCoord> = harbors.iter().filter(|(_, b)| b.is_ready()).map(|(_, b)| b.pos).collect();
            let far_ok = inv.ware(Ware::Boards) >= 60;
            let yard = self.place(sim, B::Shipyard, |p| {
                let d = ready.iter().map(|h| g.distance(p, *h)).min()? as i32;
                (d <= 7 || far_ok && d <= 16 && sim.ships_from(me, g.neighbor(p, Direction::SE))).then_some(20 - d)
            });
            // (one more harbor at most, and only with boards to spare: a harbor costs a lot, and
            // a realm spent its last boards on a third)
            if yard.is_some()
                || harbors.len() >= 2
                || inv.ware(Ware::Boards) < 30
                || harbors.iter().any(|(_, b)| !b.is_ready())
            {
                return yard;
            }
            let spot = g.coords().find(|p| {
                w.is_harbor_spot(*p)
                    && w.bq_for(*p, me) == Bq::Harbor
                    && sim.harbor_sea(*p).is_some()
                    && harbors.iter().all(|(_, b)| g.distance(*p, b.pos) > 7)
                    && !given_up(p)
            });
            return spot.map(|at| Cmd::PlaceBuilding { at, kind: B::Harbor });
        }
        // the shipyard makes boats until told to make ships
        if let Some((_, y)) = self.my_buildings(sim).find(|(_, b)| b.kind == B::Shipyard && !b.build_ships) {
            return Some(Cmd::SetShipyardMode { at: y.pos, ships: true });
        }
        let ships = sim.ships().iter().filter(|(_, s)| s.player == me).count();
        // enough ships: the shipyard rests (a ship takes 9 boards; built at RTTR's pace an
        // endless yard ate a realm's boards: North America's land fell from 57 % to 30 %)
        // (an invader keeps two: its boards go to the posts by the colony)
        let wanted = if self.invade.is_some() { 2 } else { (2 + harbors.len()).min(6) };
        if let Some((_, y)) = self.my_buildings(sim).find(|(_, b)| b.kind == B::Shipyard && b.is_ready())
            && (ships >= wanted) != y.stopped
        {
            return Some(Cmd::ToggleProduction { at: y.pos });
        }
        if ships == 0 {
            return None;
        }
        // nobody in sight across the water: a ship explores the coasts (sea attacks need eyes)
        if let Some(sea) = sim.harbor_sea(hb.pos)
            && sim.idle_ship_for(me, hid).is_some()
            && !sim.unexplored_spots(me, sea, hb.pos).is_empty()
            && !sim
                .buildings()
                .iter()
                .any(|(t, b)| b.player != me && sim.is_attackable(me, t) && sim.other_land_pub(hb.pos, b.pos))
            && self.rng.chance(10)
        {
            return Some(Cmd::StartExploration { harbor: hb.pos });
        }
        // an invader rushes until one of its harbors stands near its target, then builds there
        let rushing = self.invade.is_some_and(|t| {
            !harbors
                .iter()
                .any(|(_, h)| sim.buildings().iter().any(|(_, b)| b.player == t && g.distance(b.pos, h.pos) <= 30))
        });
        match &hb.expedition {
            // (only with boards and stones to spare: an expedition takes a harbor's worth, and a
            // realm that lost its HQ spent its last boards on one)
            // (an invader sets out as soon as it can, with an expedition's own wares: game
            // design round 1, #7)
            None if rushing && inv.ware(Ware::Boards) >= 4 && inv.ware(Ware::Stones) >= 6 => {
                Some(Cmd::StartExpedition { harbor: hb.pos })
            }
            None if inv.ware(Ware::Boards) >= 20 && inv.ware(Ware::Stones) >= 10 && self.rng.chance(20) => {
                Some(Cmd::StartExpedition { harbor: hb.pos })
            }
            Some(_) if sim.expedition_ready(hid) => {
                let sea = sim.harbor_sea(hb.pos)?;
                let hpos = hb.pos;
                // land within striking distance of a rival (but not under its walls); otherwise
                // settle the nearest free coast
                let enemies: Vec<MapCoord> = sim
                    .buildings()
                    .iter()
                    .filter(|(_, b)| {
                        !sim.are_allies(b.player, me)
                            && (sim.rules().building(b.kind).is_military() || b.kind.is_warehouse())
                            // only what we have seen: no aiming through the fog, but an invader
                            // knows its target's coast
                            && (sim.visibility(me, b.pos) != rr_sim::Visibility::Unexplored
                                || self.invade == Some(b.player))
                    })
                    .map(|(_, b)| b.pos)
                    .collect();
                sim.free_harbor_spots(me, sea)
                    .into_iter()
                    .min_by_key(|p| {
                        let de = enemies.iter().map(|e| g.distance(*e, *p)).min().unwrap_or(u32::MAX);
                        if (12..=30).contains(&de) { (0, de, p.x, p.y) } else { (1, g.distance(*p, hpos), p.x, p.y) }
                    })
                    .map(|to| Cmd::SendExpedition { harbor: hpos, to })
            }
            _ => None,
        }
    }

    /// Steer the economy with settings: beer for recruits, pause surplus stone.
    fn balance(&mut self, sim: &Sim) -> Option<Cmd> {
        let inv = sim.player_inventory(self.player);
        let settings = &sim.players()[self.player as usize].settings;
        let weight = |w: Ware, b: B| settings.distribution.iter().find(|e| e.0 == w && e.1 == b).map_or(5, |e| e.2);
        // all but one may attack; with soldiers in reserve, man the inland buildings too, so
        // there are attackers wherever the front moves
        let manned = settings.military[4] > 0;
        let reserve = if manned { inv.soldiers() >= 4 } else { inv.soldiers() > 16 };
        let (far, middle) = if reserve { (3, 6) } else { (0, 1) };
        // (by an own harbor as many as in the middle: attacks over the sea start there; not full,
        // as full garrisons there held half the generals and the soldiers the borders lacked,
        // veteran round 8)
        if settings.military[3] != 5
            || settings.military[4] != far
            || settings.military[5] != middle
            || settings.military[6] != middle
        {
            let mut m = settings.military;
            m[3] = 5;
            m[4] = far;
            m[5] = middle;
            m[6] = middle;
            return Some(Cmd::SetMilitary { values: m });
        }
        // coal shared evenly by smelters, armories and mints (RTTR InitDistribution; veteran
        // round 6, #6: armories used to hoard it while the smelters starved)
        for b in [B::IronSmelter, B::Armory, B::Mint] {
            if weight(Ware::Coal, b) != 10 {
                return Some(Cmd::SetDistribution { ware: Ware::Coal, building: b, weight: 10 });
            }
        }
        // food the same for every mine (InitDistribution; veteran round 7, #3: coal mines
        // starved beside the iron mines, the granite mine got the least)
        // (fish stands for all food in the distribution table)
        for b in [B::CoalMine, B::IronMine, B::GoldMine, B::GraniteMine] {
            if weight(Ware::Fish, b) != 10 {
                return Some(Cmd::SetDistribution { ware: Ware::Fish, building: b, weight: 10 });
            }
        }
        let want_beer = inv.ware(Ware::Beer) < 3 && inv.ware(Ware::Sword) > 2;
        let target = if want_beer { 10 } else { 3 };
        if weight(Ware::Grain, B::Brewery) != target {
            return Some(Cmd::SetDistribution { ware: Ware::Grain, building: B::Brewery, weight: target });
        }
        // pause producers whose ware piles up, resume them when it runs low
        let limits = [
            (B::Quarry, Ware::Stones, 150, 60),
            (B::Woodcutter, Ware::Wood, 120, 40),
            (B::Sawmill, Ware::Boards, 200, 80),
            (B::Well, Ware::Water, 80, 25),
            (B::Fishery, Ware::Fish, 80, 30),
            (B::Hunter, Ware::Meat, 60, 20),
            // beer piled up by the hundred on grain the mills and pigs wanted
            (B::Brewery, Ware::Beer, 40, 15),
        ];
        for (_, b) in self.my_buildings(sim) {
            if !b.is_ready() {
                continue;
            }
            if let Some((_, w, hi, lo)) = limits.iter().find(|l| l.0 == b.kind) {
                let n = inv.ware(*w);
                if n > *hi && !b.stopped || n < *lo && b.stopped {
                    return Some(Cmd::ToggleProduction { at: b.pos });
                }
            }
        }
        None
    }

    /// A woodcutter within 5 of the forester at `f`.
    fn place_woodcutter_by(&mut self, sim: &Sim, f: MapCoord) -> Option<Cmd> {
        let g = sim.world().grid();
        self.place(sim, B::Woodcutter, |p| {
            (g.distance(f, p) <= 5).then(|| Self::count_around(sim, p, 6, |o| matches!(o, NodeObj::Tree { .. })))
        })
    }

    fn place_woodcutter(&mut self, sim: &Sim) -> Option<Cmd> {
        let g = sim.world().grid();
        let foresters: Vec<MapCoord> =
            self.my_buildings(sim).filter(|(_, b)| b.kind == B::Forester).map(|(_, b)| b.pos).collect();
        let cutters: Vec<MapCoord> =
            self.my_buildings(sim).filter(|(_, b)| b.kind == B::Woodcutter).map(|(_, b)| b.pos).collect();
        self.place(sim, B::Woodcutter, |p| {
            let trees = Self::count_around(sim, p, 6, |o| matches!(o, NodeObj::Tree { .. }));
            let near_forester = foresters.iter().any(|f| g.distance(*f, p) <= 5);
            let crowded = cutters.iter().filter(|c| g.distance(**c, p) <= 4).count() as i32;
            (trees > 2 || near_forester).then_some(trees + if near_forester { 8 } else { 0 } - crowded * 6)
        })
    }

    fn cleanup(&mut self, sim: &Sim) -> Option<Cmd> {
        let g = sim.world().grid();
        for (_, b) in self.my_buildings(sim) {
            if !b.is_ready() || b.attempts < 16 {
                continue;
            }
            let useless = match b.kind {
                B::Woodcutter => {
                    b.productivity() == 0
                        && Self::count_around(sim, b.pos, 6, |o| matches!(o, NodeObj::Tree { .. })) == 0
                }
                B::Quarry => Self::count_around(sim, b.pos, 8, |o| matches!(o, NodeObj::Granite { .. })) == 0,
                // (a mine running low never quite stops: down to a find in ten, its food is wasted)
                B::CoalMine | B::IronMine | B::GoldMine | B::GraniteMine => b.exhausted && b.productivity() <= 10,
                B::Fishery => b.productivity() == 0,
                _ => false,
            };
            if useless {
                return Some(Cmd::RemoveBuilding { at: b.pos });
            }
        }
        // sites that never get started (unreachable) are removed after a long time
        let _ = (g, Job::Helper, BState::Ready);
        None
    }
}

/// Animals bucketed in cells of `reach + 1` nodes, so counting those in reach of a spot looks
/// at a few cells, not at every animal on the map (perf-2 #1: 41-140 ms hunter placements).
struct GameNear {
    cell: u32,
    cols: u32,
    cells: Vec<Vec<MapCoord>>,
}

impl GameNear {
    fn new(g: &rr_core::Grid, animals: impl Iterator<Item = MapCoord>, reach: u32) -> Self {
        let cell = reach + 1;
        let (cols, rows) = ((g.width as u32).div_ceil(cell), (g.height as u32).div_ceil(cell));
        let mut cells = vec![Vec::new(); (cols * rows) as usize];
        for a in animals {
            cells[(a.y as u32 / cell * cols + a.x as u32 / cell) as usize].push(a);
        }
        Self { cell, cols, cells }
    }

    /// The animals within `reach` of `p` (exactly as a scan of all of them).
    fn count(&self, g: &rr_core::Grid, p: MapCoord, reach: u32) -> usize {
        // the hex distance is at least the rows apart and half the columns apart: the cells
        // that hold the rows and columns within that (the map wraps; the last cells may be
        // narrower)
        let span = |c: u16, r: i32, size: u16| {
            let mut v: Vec<u32> =
                (c as i32 - r..=c as i32 + r).map(|k| k.rem_euclid(size as i32) as u32 / self.cell).collect();
            v.dedup();
            v.sort_unstable();
            v.dedup();
            v
        };
        let r = reach as i32;
        let (xs, ys) = (span(p.x, 2 * r + 1, g.width), span(p.y, r, g.height));
        let mut n = 0;
        for y in &ys {
            for x in &xs {
                let i = (y * self.cols + x) as usize;
                n += self.cells[i].iter().filter(|a| g.distance(**a, p) <= reach).count();
            }
        }
        n
    }
}

/// Soldiers free in the warehouses, given (soldiers there, reserve there) of each.
fn spare_of(warehouses: impl IntoIterator<Item = (usize, usize)>) -> usize {
    warehouses.into_iter().map(|(there, keep)| there.saturating_sub(keep)).sum()
}

#[cfg(test)]
mod departure_tests {
    /// Veteran round 11, #4: every hold the AI keeps against the original is listed in
    /// docs/DEPARTURES.md (a working copy's document: skipped where it isn't).
    #[test]
    fn the_holds_are_written_down() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/DEPARTURES.md");
        let Ok(doc) = std::fs::read_to_string(path) else { return };
        for c in [
            "GRACE",
            "CASTLE_GRACE",
            "CAMPAIGN_ATTACK_WINDOW",
            "CAMPAIGN_MEASURE",
            "EARLY_WAR",
            "BEATEN_AFTER",
            "CASTLE_KEEP",
        ] {
            assert!(doc.contains(&format!("`{c}`")), "{c} is not in DEPARTURES.md");
        }
    }
}

#[cfg(test)]
mod spare_tests {
    /// Veteran round 10, #7: only soldiers really in a warehouse are kept back by its reserve.
    #[test]
    fn spare_soldiers_count_what_is_there() {
        // a castle holding 6 with a reserve of 2, three storehouses with a reserve of 1 holding none
        assert_eq!(super::spare_of([(6, 2), (0, 1), (0, 1), (0, 1)]), 4);
        assert_eq!(super::spare_of([(1, 3)]), 0);
    }
}

#[cfg(test)]
mod game_near_tests {
    use super::*;

    #[test]
    fn counts_game_as_a_scan_of_all_does() {
        for (w, h, reach) in [(64u16, 64u16, 19u32), (40, 30, 19), (192, 128, 19), (50, 50, 5)] {
            let g = rr_core::Grid::new(w, h);
            let mut rng = Pcg32::new(w as u64 * 7 + reach as u64, 1);
            let animals: Vec<MapCoord> =
                (0..300).map(|_| MapCoord::new(rng.below(w as u32) as u16, rng.below(h as u32) as u16)).collect();
            let near = GameNear::new(&g, animals.iter().copied(), reach);
            for _ in 0..300 {
                let p = MapCoord::new(rng.below(w as u32) as u16, rng.below(h as u32) as u16);
                let all = animals.iter().filter(|a| g.distance(**a, p) <= reach).count();
                assert_eq!(near.count(&g, p, reach), all, "{w}x{h} reach {reach} at {p:?}");
            }
        }
    }
}

#[cfg(test)]
mod save_tests {
    use super::*;

    /// Saves from before the truce (RRG3) still load, with no truce.
    #[test]
    fn older_saves_load() {
        let map = rr_core::MapData::flat(40, 40, 8);
        let mut map = map;
        map.hq = vec![Some(MapCoord::new(10, 10)), Some(MapCoord::new(30, 30))];
        let setup = rr_sim::GameSetup { seed: 1, players: vec![Default::default(); 2], ..Default::default() };
        let rules = std::sync::Arc::new(rr_rules::Rules::classic());
        let sim = Sim::new(rules.clone(), &map, &setup).unwrap();
        let ai = Ai::with_level(1, Level::Hard);
        let v1 = AiV1 {
            player: ai.player,
            level: ai.level,
            rng: ai.rng.clone(),
            blocked: vec![(MapCoord::new(1, 2), 3)],
            no_spot: Vec::new(),
            out_of_reach: Vec::new(),
        };
        let mut bytes = GAME_MAGIC_V3.to_vec();
        bytes.extend(0u32.to_le_bytes());
        bytes.extend(postcard::to_stdvec(&(sim.save(), vec![v1], b"x".to_vec())).unwrap());
        let (_, ais, extra) = load_game_with(rules, &bytes).unwrap();
        assert_eq!((ais[0].player, ais[0].level, ais[0].truce, extra), (1, Level::Hard, None, b"x".to_vec()));
        assert_eq!(ais[0].blocked, vec![(MapCoord::new(1, 2), 3)]);
    }
}

/// How long after the truce a campaign's rivals attack the player in easy's measure (30 min).
const CAMPAIGN_MEASURE: u32 = 30 * 60 * 20;

/// A campaign's rivals attack the player in one round per window this long (4 min): only in its
/// first stretch as long as the rival's own attack rhythm (one attack thought fits), so a slower rival
/// (easy: every 8 minutes) is never held back (new-player round 9, #1: with a fixed 700 GF easy
/// rivals never attacked at all).
const CAMPAIGN_ATTACK_WINDOW: u32 = 4 * 60 * 20;

/// From when a realm without posts and soldiers counts as beaten: an hour in.
const BEATEN_AFTER: u32 = 60 * 60 * 20;

/// The first 40 minutes, when a realm keeps a third of its first army at home (17 of 52).
const EARLY_WAR: u32 = 40 * 60 * 20;
const CASTLE_KEEP: usize = 17;

/// How much longer than the truce a newcomer's headquarters is spared: 15 minutes.
const CASTLE_GRACE: u32 = 15 * 60 * 20;

/// How many steps from the road network a new building's flag may be.
const NEAR_NET: u32 = 8;

/// Steps from the nearest of `from` for every node, up to `max` (farther: `u8::MAX`). The same as
/// the smallest `Grid::distance`, found in one breadth-first search instead of a distance per
/// node and flag (the AI's placement was most of its time).
fn steps_from(g: rr_core::Grid, from: &[MapCoord], max: u32) -> Vec<u8> {
    let mut d = vec![u8::MAX; g.len()];
    let mut frontier: Vec<MapCoord> = Vec::new();
    for p in from {
        if d[g.index(*p)] != 0 {
            d[g.index(*p)] = 0;
            frontier.push(*p);
        }
    }
    for step in 1..=max.min(254) as u8 {
        let mut next = Vec::new();
        for p in frontier {
            for n in g.neighbors(p) {
                if d[g.index(n)] == u8::MAX {
                    d[g.index(n)] = step;
                    next.push(n);
                }
            }
        }
        frontier = next;
    }
    d
}

#[cfg(test)]
mod steps_tests {
    use super::steps_from;
    use rr_core::{Grid, MapCoord};

    /// Performance roast #3: the search gives the smallest distance, exactly, up to the limit.
    #[test]
    fn steps_equal_the_nearest_distance() {
        for (w, h) in [(40, 40), (64, 30), (33, 20)] {
            let g = Grid::new(w, h);
            let from = [MapCoord::new(3, 4), MapCoord::new(20, 10), MapCoord::new(w - 1, h - 1)];
            let d = steps_from(g, &from, 8);
            for p in g.coords() {
                let want = from.iter().map(|f| g.distance(*f, p)).min().unwrap();
                let got = d[g.index(p)] as u32;
                if want <= 8 {
                    assert_eq!(got, want, "{p:?}");
                } else {
                    assert_eq!(got, u8::MAX as u32, "{p:?}");
                }
            }
        }
    }
}
