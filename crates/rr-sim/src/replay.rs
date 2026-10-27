//! Replays: the map, the setup and every command with its frame. Playing one back reproduces
//! the game exactly (the simulation is deterministic); AIs are not re-run.

use crate::{GameSetup, Nation, PlayerCommand, PlayerSetup, SetupError, Sim};
use rr_core::MapData;
use rr_rules::Rules;
use std::sync::Arc;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Replay {
    pub version: u32,
    pub rules_hash: u64,
    pub map: MapData,
    pub seed: u64,
    pub players: Vec<(String, u8, Option<u8>)>,
    pub start_goods: crate::StartGoods,
    /// (frame, command), in execution order.
    pub commands: Vec<(u32, PlayerCommand)>,
    /// Sync hash after the last frame, for checking.
    pub final_gf: u32,
    pub final_hash: u64,
}

/// The save format and the simulation's rules of play: a replay plays back only with both the
/// same (QA round 4, #6). Older replays stored the save format alone, so they are refused too.
fn replay_version() -> u32 {
    crate::SAVE_VERSION * 1000 + crate::SIM_VERSION
}

fn nation_code(n: Nation) -> u8 {
    n as u8
}

fn nation_from(c: u8) -> Nation {
    match c {
        1 => Nation::Vikings,
        2 => Nation::Nubians,
        3 => Nation::Japanese,
        _ => Nation::Romans,
    }
}

impl Replay {
    pub fn new(rules: &Rules, map: &MapData, setup: &GameSetup) -> Self {
        Self {
            version: replay_version(),
            rules_hash: rules.hash(),
            map: map.clone(),
            seed: setup.seed,
            players: setup.players.iter().map(|p| (p.name.clone(), nation_code(p.nation), p.team)).collect(),
            start_goods: setup.start_goods,
            commands: Vec::new(),
            final_gf: 0,
            final_hash: 0,
        }
    }

    pub fn setup(&self) -> GameSetup {
        GameSetup {
            seed: self.seed,
            players: self
                .players
                .iter()
                .map(|(name, n, team)| PlayerSetup { name: name.clone(), nation: nation_from(*n), team: *team })
                .collect(),
            start_goods: self.start_goods,
        }
    }

    /// Record the commands given to `sim.step` for the frame `sim` is about to run.
    pub fn record(&mut self, gf: u32, cmds: &[PlayerCommand]) {
        self.commands.extend(cmds.iter().cloned().map(|c| (gf, c)));
    }

    pub fn finish(&mut self, sim: &Sim) {
        self.final_gf = sim.gf().0;
        self.final_hash = sim.sync_hash();
    }

    /// The game at frame 0, ready to be fed the recorded commands.
    pub fn play_setup(&self, rules: Arc<Rules>) -> Result<Sim, SetupError> {
        if self.version != replay_version() || self.rules_hash != rules.hash() {
            return Err(SetupError::ReplayMismatch);
        }
        Sim::new(rules, &self.map, &self.setup())
    }

    /// Re-run the game up to the recorded end.
    pub fn play(&self, rules: Arc<Rules>) -> Result<Sim, SetupError> {
        let mut sim = self.play_setup(rules)?;
        let mut i = 0;
        while sim.gf().0 < self.final_gf {
            let gf = sim.gf().0;
            let start = i;
            while i < self.commands.len() && self.commands[i].0 == gf {
                i += 1;
            }
            let cmds: Vec<PlayerCommand> = self.commands[start..i].iter().map(|c| c.1.clone()).collect();
            sim.step(&cmds);
        }
        Ok(sim)
    }

    /// At the recorded end, the game must be the recorded one (false: it played differently).
    pub fn verify(&self, sim: &Sim) -> bool {
        self.final_gf == 0 || sim.gf().0 != self.final_gf || sim.sync_hash() == self.final_hash
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = b"RRREPLAY".to_vec();
        out.extend(postcard::to_stdvec(self).expect("serializable"));
        out
    }

    pub fn from_bytes(b: &[u8]) -> Option<Self> {
        postcard::from_bytes(b.strip_prefix(b"RRREPLAY".as_slice())?).ok()
    }
}
