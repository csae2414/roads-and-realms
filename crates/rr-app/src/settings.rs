//! Options that outlive a session (volumes, UI scale, language, play aids), kept as JSON next to
//! the saves. Missing or broken entries fall back to the defaults.

use serde_json::{Value, json};

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// Slider positions (0 silent, then -40 dB to 0 dB).
    pub master: f32,
    pub music: f32,
    pub sfx: f32,
    pub ambience: f32,
    pub mute_in_background: bool,
    pub shuffle: bool,
    pub ui_scale: f32,
    /// None: follow `$LANG`.
    pub german: Option<bool>,
    pub fog_of_war: bool,
    pub tips: bool,
    pub auto_road: bool,
    /// Pause the game when attacked (single player; a11y round 2, #5).
    pub pause_on_attack: bool,
    /// A calm map: the drawn-only life that moves (gusts, birds, glints, flying bits) stays
    /// still, for players whom motion bothers.
    pub calm_map: bool,
    /// A game has been won: the menu no longer starts on easy opponents.
    pub won_once: bool,
    /// The missions won (keys), marked ✔ in the menu.
    pub won_missions: Vec<String>,
    /// The best run per mission and level (game design round 1, #4: a win left no record).
    pub best: Vec<Best>,
    /// Keys changed from their defaults: (action, key name), see `keys::ACTS`.
    pub keys: Vec<(String, String)>,
}

/// A mission won: in how many minutes, and its score.
#[derive(Clone, Debug, PartialEq)]
pub struct Best {
    pub mission: String,
    /// 0 easy, 1 normal, 2 hard
    pub level: u8,
    pub minutes: u32,
    pub score: u32,
}

/// Keep `b` if it is the first or a faster win of its mission and level; whether it was.
pub fn record_best(best: &mut Vec<Best>, b: Best) -> bool {
    match best.iter_mut().find(|x| x.mission == b.mission && x.level == b.level) {
        Some(x) if x.minutes <= b.minutes => false,
        Some(x) => {
            *x = b;
            true
        }
        None => {
            best.push(b);
            true
        }
    }
}

pub fn best_of<'a>(best: &'a [Best], mission: &str, level: u8) -> Option<&'a Best> {
    best.iter().find(|b| b.mission == mission && b.level == level)
}

/// The mark for a win in so many minutes, by the mission's limits for gold, silver and bronze
/// (`Mission::medals`); on easy (level 0) the limits are half as long again, so every level
/// has all three medals to earn (docs/CAMPAIGN-ROME.md).
pub fn medal(minutes: u32, limits: [u32; 3], level: u8) -> &'static str {
    let scale = |m: u32| if level == 0 { m * 3 / 2 } else { m };
    match limits.iter().position(|l| minutes <= scale(*l)) {
        Some(0) => "gold",
        Some(1) => "silver",
        Some(2) => "bronze",
        _ => "",
    }
}

/// A win's score: the land share in percent, ten for each realm vanquished, less half a point a
/// minute (the proposal of game design round 1, #4; to be tuned).
pub fn score(land_percent: u32, vanquished: u32, minutes: u32) -> u32 {
    (land_percent + 10 * vanquished).saturating_sub(minutes / 2)
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            master: 1.0,
            music: 0.75,
            sfx: 0.9,
            ambience: 0.7,
            mute_in_background: true,
            shuffle: false,
            ui_scale: 1.0,
            german: None,
            fog_of_war: false,
            tips: true,
            auto_road: true,
            pause_on_attack: false,
            calm_map: false,
            won_once: false,
            won_missions: Vec::new(),
            best: Vec::new(),
            keys: Vec::new(),
        }
    }
}

/// The opponents the menu starts with: easy until the first game is won (new-player round 6, #4).
pub fn first_level(won_once: bool) -> u8 {
    if won_once { 1 } else { 0 }
}

impl Settings {
    pub fn to_json(&self) -> String {
        let mut v = json!({
            "master_level": self.master,
            "music_level": self.music,
            "sfx_level": self.sfx,
            "ambience_level": self.ambience,
            "mute_in_background": self.mute_in_background,
            "shuffle": self.shuffle,
            "ui_scale": self.ui_scale,
            "fog_of_war": self.fog_of_war,
            "tips": self.tips,
            "auto_road": self.auto_road,
            "pause_on_attack": self.pause_on_attack,
            "calm_map": self.calm_map,
            "won_once": self.won_once,
            "won_missions": self.won_missions,
            "keys": self.keys.iter().map(|(a, k)| (a.clone(), json!(k))).collect::<serde_json::Map<_, _>>(),
            "best": self.best.iter().map(|b| json!({
                "mission": b.mission, "level": b.level, "minutes": b.minutes, "score": b.score,
            })).collect::<Vec<_>>(),
        });
        if let Some(de) = self.german {
            v["german"] = json!(de);
        }
        serde_json::to_string_pretty(&v).unwrap_or_default()
    }

    pub fn from_json(s: &str) -> Settings {
        let d = Settings::default();
        let Ok(v) = serde_json::from_str::<Value>(s) else { return d };
        let f = |k: &str, def: f32, lo: f32, hi: f32| {
            v.get(k).and_then(Value::as_f64).map_or(def, |x| (x as f32).clamp(lo, hi))
        };
        let b = |k: &str, def: bool| v.get(k).and_then(Value::as_bool).unwrap_or(def);
        Settings {
            // (the levels were linear "music"/"sfx" before; those keys are ignored)
            master: f("master_level", d.master, 0.0, 1.0),
            music: f("music_level", d.music, 0.0, 1.0),
            sfx: f("sfx_level", d.sfx, 0.0, 1.0),
            ambience: f("ambience_level", d.ambience, 0.0, 1.0),
            mute_in_background: b("mute_in_background", d.mute_in_background),
            shuffle: b("shuffle", d.shuffle),
            ui_scale: f("ui_scale", d.ui_scale, 1.0, 3.0),
            german: v.get("german").and_then(Value::as_bool),
            fog_of_war: b("fog_of_war", d.fog_of_war),
            tips: b("tips", d.tips),
            auto_road: b("auto_road", d.auto_road),
            pause_on_attack: b("pause_on_attack", d.pause_on_attack),
            calm_map: b("calm_map", d.calm_map),
            won_once: b("won_once", d.won_once),
            won_missions: v
                .get("won_missions")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                .unwrap_or_default(),
            keys: v
                .get("keys")
                .and_then(Value::as_object)
                .map(|o| o.iter().filter_map(|(a, k)| Some((a.clone(), k.as_str()?.to_string()))).collect())
                .unwrap_or_default(),
            best: v
                .get("best")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|x| {
                            let n = |k: &str| x.get(k).and_then(Value::as_u64).map(|n| n as u32);
                            Some(Best {
                                mission: x.get("mission")?.as_str()?.to_string(),
                                level: n("level")?.min(2) as u8,
                                minutes: n("minutes")?,
                                score: n("score")?,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    /// Next to the quick save, so `XDG_DATA_HOME` moves both.
    pub fn path() -> std::path::PathBuf {
        crate::app::App::save_path().with_file_name("settings.json")
    }

    pub fn load() -> Settings {
        std::fs::read_to_string(Self::path()).map(|s| Self::from_json(&s)).unwrap_or_default()
    }

    pub fn store(&self) {
        let p = Self::path();
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = crate::app::write_safely(&p, self.to_json().as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// New-player round 6, #4: the first games start against easy opponents, until one is won.
    #[test]
    fn easy_opponents_until_a_game_is_won() {
        let fresh = Settings::from_json("");
        assert!(!fresh.won_once);
        assert_eq!(first_level(fresh.won_once), 0);
        let won = Settings { won_once: true, ..Settings::default() };
        let back = Settings::from_json(&won.to_json());
        assert!(back.won_once);
        // and which missions were won (new-player round 10, #5)
        let s = Settings { won_missions: vec!["europe".into(), "north".into()], ..Settings::default() };
        assert_eq!(Settings::from_json(&s.to_json()).won_missions, s.won_missions);
        assert_eq!(first_level(back.won_once), 1);
    }

    /// Game design round 1, #4: a mission's best per level is kept: the shorter time wins, and
    /// it travels in the settings file.
    #[test]
    fn the_best_runs_are_kept() {
        let mut best = Vec::new();
        assert!(record_best(&mut best, Best { mission: "europe".into(), level: 1, minutes: 120, score: 40 }));
        assert!(!record_best(&mut best, Best { mission: "europe".into(), level: 1, minutes: 130, score: 90 }));
        assert!(record_best(&mut best, Best { mission: "europe".into(), level: 1, minutes: 95, score: 55 }));
        assert!(record_best(&mut best, Best { mission: "europe".into(), level: 2, minutes: 200, score: 10 }));
        assert_eq!(best.len(), 2);
        assert_eq!(best_of(&best, "europe", 1).map(|b| b.minutes), Some(95));
        let s = Settings { best: best.clone(), ..Settings::default() };
        assert_eq!(Settings::from_json(&s.to_json()).best, best);
        const W: [u32; 3] = rr_game::missions::WORLD_MEDALS;
        assert_eq!(
            (medal(85, W, 1), medal(90, W, 1), medal(119, W, 1), medal(150, W, 1), medal(181, W, 1)),
            ("gold", "gold", "silver", "bronze", "")
        );
        // easy: half as long again
        assert_eq!((medal(135, W, 0), medal(136, W, 0), medal(270, W, 0)), ("gold", "silver", "bronze"));
        assert_eq!(score(40, 2, 100), 10);
        assert_eq!(score(5, 0, 300), 0, "never below nothing");
    }

    #[test]
    fn round_trip() {
        let s = Settings {
            master: 0.8,
            music: 0.5,
            sfx: 0.25,
            ambience: 0.3,
            mute_in_background: false,
            shuffle: true,
            ui_scale: 2.0,
            german: Some(true),
            fog_of_war: true,
            tips: false,
            auto_road: true,
            pause_on_attack: true,
            calm_map: true,
            won_once: true,
            won_missions: vec!["europe".into()],
            best: vec![Best { mission: "north".into(), level: 0, minutes: 77, score: 60 }],
            keys: vec![("pause".into(), "K".into())],
        };
        assert_eq!(Settings::from_json(&s.to_json()), s);
        let d = Settings::default();
        assert_eq!(Settings::from_json(&d.to_json()), d);
    }

    #[test]
    fn broken_or_partial_files_fall_back() {
        assert_eq!(Settings::from_json("not json"), Settings::default());
        let s = Settings::from_json(r#"{"music_level": 7, "ui_scale": 0.1, "tips": "yes", "german": false}"#);
        assert_eq!(s.music, 1.0);
        assert_eq!(s.ui_scale, 1.0);
        assert!(s.tips);
        assert_eq!(s.german, Some(false));
        assert_eq!(s.sfx, Settings::default().sfx);
    }
}
