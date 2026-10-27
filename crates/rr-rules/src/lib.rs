//! Typed, validated game rules loaded from `data/rules/*.toml`.

mod types;

pub use types::{BuildingType, Job, Size, Ware};

use rr_core::{StableHash, StableHasher};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const CLASSIC_TOML: &str = include_str!("../../../data/rules/classic.toml");

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MilitaryRules {
    pub hp: [u8; 5],
    pub fight_k: u32,
    pub heal_every: u32,
    pub heal_rand: u32,
    pub promote_wait: u32,
    pub promote_rand: u32,
    pub attack_radius: u16,
    pub attack_max_walk: u16,
    pub recruit_wait: u32,
    pub recruit_rand: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EconomyRules {
    pub flag_capacity: u8,
    pub helper_spawn: u32,
    pub helper_spawn_rand: u32,
    pub helper_max: u32,
    pub donkey_busy_percent: u32,
    pub donkey_window: u32,
    pub woodcutter_radius: u16,
    pub forester_radius: u16,
    pub stonemason_radius: u16,
    pub fisher_radius: u16,
    pub hunter_radius: u16,
    pub farmer_radius: u16,
    pub geologist_radius: u16,
    pub geologist_flags: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobDef {
    #[serde(default)]
    pub tool: Option<Ware>,
    pub work: u32,
    pub wait1: u32,
    pub wait2: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MilitaryDef {
    pub soldiers: u8,
    pub coins: u8,
    pub radius: u16,
    pub sight: u16,
}

fn default_slots() -> u8 {
    6
}
fn yes() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildingDef {
    pub size: Size,
    #[serde(default)]
    pub boards: u8,
    #[serde(default)]
    pub stones: u8,
    #[serde(default)]
    pub worker: Option<Job>,
    #[serde(default)]
    pub inputs: Vec<Ware>,
    #[serde(default = "default_slots")]
    pub slots_per_input: u8,
    /// True: one of each input per cycle. False: one of whichever input is most stocked.
    #[serde(default = "yes")]
    pub use_one_each: bool,
    #[serde(default)]
    pub output: Option<Ware>,
    #[serde(default)]
    pub military: Option<MilitaryDef>,
}

impl BuildingDef {
    /// (boards, stones)
    pub fn cost(&self) -> (u8, u8) {
        (self.boards, self.stones)
    }

    pub fn is_military(&self) -> bool {
        self.military.as_ref().is_some_and(|m| m.soldiers > 0)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartGoods {
    pub wares: BTreeMap<Ware, u32>,
    pub jobs: BTreeMap<Job, u32>,
}

impl StartGoods {
    pub fn ware(&self, w: Ware) -> u32 {
        self.wares.get(&w).copied().unwrap_or(0)
    }
    pub fn job(&self, j: Job) -> u32 {
        self.jobs.get(&j).copied().unwrap_or(0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RulesFile {
    military: MilitaryRules,
    economy: EconomyRules,
    jobs: BTreeMap<Job, JobDef>,
    buildings: BTreeMap<BuildingType, BuildingDef>,
    start: StartGoods,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rules {
    pub military: MilitaryRules,
    pub economy: EconomyRules,
    jobs: Vec<JobDef>,
    buildings: Vec<BuildingDef>,
    pub start: StartGoods,
}

#[derive(Debug)]
pub enum RulesError {
    Parse(String),
    Missing(String),
}

impl std::fmt::Display for RulesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RulesError::Parse(e) => write!(f, "rules parse error: {e}"),
            RulesError::Missing(what) => write!(f, "rules: missing definition for {what}"),
        }
    }
}

impl std::error::Error for RulesError {}

impl Rules {
    pub fn classic() -> Self {
        Self::from_toml(CLASSIC_TOML).expect("built-in classic rules are valid")
    }

    /// Rules from a file of the classic file's shape (`--rules FILE`; map maker round 1, #1:
    /// the rules were built in, with no way to try a change); the error names the file.
    pub fn from_file(path: &std::path::Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::from_toml(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn from_toml(text: &str) -> Result<Self, RulesError> {
        let mut f: RulesFile = toml::from_str(text).map_err(|e| RulesError::Parse(e.to_string()))?;
        let jobs = Job::ALL
            .iter()
            .map(|j| f.jobs.remove(j).ok_or_else(|| RulesError::Missing(format!("job {j:?}"))))
            .collect::<Result<Vec<_>, _>>()?;
        let buildings = BuildingType::ALL
            .iter()
            .map(|b| f.buildings.remove(b).ok_or_else(|| RulesError::Missing(format!("building {b:?}"))))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { military: f.military, economy: f.economy, jobs, buildings, start: f.start })
    }

    pub fn building(&self, b: BuildingType) -> &BuildingDef {
        &self.buildings[b as usize]
    }

    pub fn job(&self, j: Job) -> &JobDef {
        &self.jobs[j as usize]
    }

    /// Fingerprint stored in saves and replays. Hashes the canonical TOML rendering (sorted maps,
    /// fixed field order), which is stable for a given rules content.
    pub fn hash(&self) -> u64 {
        let file = RulesFile {
            military: self.military.clone(),
            economy: self.economy.clone(),
            jobs: Job::ALL.iter().map(|j| (*j, self.job(*j).clone())).collect(),
            buildings: BuildingType::ALL.iter().map(|b| (*b, self.building(*b).clone())).collect(),
            start: self.start.clone(),
        };
        let text = toml::to_string(&file).expect("serializable");
        let mut h = StableHasher::new();
        text.stable_hash(&mut h);
        h.finish()
    }
}

#[cfg(test)]
mod file_tests {
    use super::*;

    /// Map maker round 1, #1: rules come from a file; a changed value is read, a typo in a key
    /// is refused with its name and line.
    #[test]
    fn rules_from_a_file() {
        let changed = CLASSIC_TOML.replacen("fight_k = ", "fight_k = 1", 1);
        let r = Rules::from_toml(&changed).expect("a changed value");
        assert_ne!(r.military.fight_k, Rules::classic().military.fight_k);
        assert_ne!(r.hash(), Rules::classic().hash());
        let typo = CLASSIC_TOML.replacen("fight_k = ", "fight_kk = ", 1);
        let e = Rules::from_toml(&typo).unwrap_err().to_string();
        assert!(e.contains("fight_kk") && e.contains("line"), "{e}");
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let p = std::env::temp_dir().join(format!("rr-rules-{nanos}.toml"));
        std::fs::write(&p, &changed).unwrap();
        assert_eq!(Rules::from_file(&p).map(|r| r.hash()), Ok(r.hash()));
        assert!(Rules::from_file(std::path::Path::new("/no/such/rules.toml")).unwrap_err().contains("rules.toml"));
        let _ = std::fs::remove_file(&p);
    }
}

#[cfg(test)]
mod doc_tests {
    /// Map maker round 1, #1: every rule key is in docs/MODDING.md (a field added without a doc
    /// line fails here).
    #[test]
    fn modding_names_every_key() {
        let src = include_str!("lib.rs");
        let doc = include_str!("../../../docs/MODDING.md");
        for name in ["MilitaryRules", "EconomyRules", "JobDef", "MilitaryDef", "BuildingDef"] {
            let start = src.find(&format!("pub struct {name} {{")).expect(name);
            let body = &src[start..start + src[start..].find("\n}").unwrap()];
            for line in body.lines().skip(1) {
                let Some(field) = line.trim().strip_prefix("pub ").and_then(|l| l.split(':').next()) else { continue };
                assert!(doc.contains(&format!("`{field}`")), "{name}.{field} is not in MODDING.md");
            }
        }
    }
}
