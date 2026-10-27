//! The world campaign as the app shows it: the missions themselves are in `rr_game::missions`;
//! here the menu's cast line and a map file's own mission.

pub use rr_game::missions::*;

/// Who is at war with you at a level, and who at peace, for the menu's hover.
pub fn cast_line(m: &Mission, level: rr_ai::Level) -> String {
    let cast = m.cast_at(level);
    let mut parts = Vec::new();
    for (stance, head) in CAST_HEADS {
        let names: Vec<&str> =
            m.rivals.iter().zip(&cast).filter(|(_, s)| **s == stance).map(|(r, _)| crate::i18n::tr(r.0)).collect();
        if !names.is_empty() {
            parts.push(format!("{} {}.", crate::i18n::tr(head), names.join(", ")));
        }
    }
    parts.join(" ")
}

/// How the menu's hover and the briefing head each group of a cast (new-player round 12, #9:
/// "at war with you" in one, "allied against you" in the other).
pub const CAST_HEADS: [(Stance, &str); 4] = [
    (Stance::Allied, "Allied against you:"),
    (Stance::Alone, "Each on their own, at war with all:"),
    (Stance::Peaceful, "At peace with you:"),
    (Stance::Friend, "Allied with you:"),
];

/// A mission that comes with a map file, kept next to it as `NAME.mission.json`: a title, the
/// briefing shown before the game, the goal, how many minutes the computer players leave you
/// alone, and names for the players (slot order).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct MapMission {
    pub title: String,
    pub briefing: String,
    pub goal: String,
    pub truce_minutes: u32,
    pub names: Vec<String>,
    /// What the map's game asks for (map maker round 1, #2: the goal was only a label).
    pub objective: Option<Objective>,
    /// The human's player slot (else the menu's choice).
    pub human: Option<u8>,
}

/// An objective as a mission file writes it: "defeat_all", {"defeat_rival": 2}, {"hold_land": 40},
/// {"hold_map_land": 40}, {"survive": 60} or {"mint_coins": 30}.
fn objective_json(o: Objective) -> serde_json::Value {
    use serde_json::json;
    match o {
        Objective::DefeatAll => json!("defeat_all"),
        Objective::DefeatRival(p) => json!({ "defeat_rival": p }),
        Objective::HoldLand(n) => json!({ "hold_land": n }),
        Objective::Survive(n) => json!({ "survive": n }),
        Objective::Make(_, n) => json!({ "mint_coins": n }),
        Objective::HoldMapLand(n) => json!({ "hold_map_land": n }),
        Objective::Vanquish(n) => json!({ "vanquish": n }),
        Objective::Reach(n) => json!({ "reach": n }),
    }
}

fn objective_from(v: &serde_json::Value) -> Result<Objective, String> {
    if v.as_str() == Some("defeat_all") {
        return Ok(Objective::DefeatAll);
    }
    let bad = || {
        "objective: one of \"defeat_all\", {\"defeat_rival\": n}, {\"hold_land\": percent}, {\"hold_map_land\": percent}, {\"survive\": minutes}, {\"mint_coins\": n}".to_string()
    };
    let o = v.as_object().filter(|o| o.len() == 1).ok_or_else(bad)?;
    let (k, n) = o.iter().next().ok_or_else(bad)?;
    let n = n.as_u64().ok_or_else(bad)? as u32;
    match k.as_str() {
        "defeat_rival" => Ok(Objective::DefeatRival(n.min(7) as u8)),
        "hold_land" => Ok(Objective::HoldLand(n.min(100))),
        "hold_map_land" => Ok(Objective::HoldMapLand(n.min(100))),
        "vanquish" => Ok(Objective::Vanquish(n)),
        "reach" => Ok(Objective::Reach(n)),
        // (a day at most: 3 579 140 minutes overflowed the frame count; QA round 8, #4)
        "survive" => Ok(Objective::Survive(n.min(24 * 60))),
        "mint_coins" => Ok(Objective::Make(rr_rules::Ware::Coins, n)),
        _ => Err(bad()),
    }
}

impl MapMission {
    pub fn path_for(map: &std::path::Path) -> std::path::PathBuf {
        map.with_extension("mission.json")
    }

    /// The mission next to a map: none if there is no file, an error if it can't be read.
    pub fn read(map: &std::path::Path) -> Result<Option<Self>, String> {
        let p = Self::path_for(map);
        let Ok(text) = std::fs::read_to_string(&p) else { return Ok(None) };
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        Self::from_json(&text).map(Some).map_err(|e| format!("{name}: {e}"))
    }

    /// The mission next to a map, if there is one that can be read.
    pub fn load(map: &std::path::Path) -> Option<Self> {
        Self::read(map).ok().flatten()
    }

    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    pub fn to_json(&self) -> String {
        let mut v = serde_json::json!({
            "title": self.title,
            "briefing": self.briefing,
            "goal": self.goal,
            "truce_minutes": self.truce_minutes,
            "names": self.names,
        });
        if let Some(o) = self.objective {
            v["objective"] = objective_json(o);
        }
        if let Some(h) = self.human {
            v["human"] = serde_json::json!(h);
        }
        serde_json::to_string_pretty(&v).unwrap_or_default()
    }

    /// A mission from its file's text; an error says where it broke, or which key is unknown
    /// (map maker round 1, #3: a typo meant no mission, and no word of it).
    pub fn from_json(s: &str) -> Result<Self, String> {
        let v: serde_json::Value = serde_json::from_str(s).map_err(|e| e.to_string())?;
        let obj = v.as_object().ok_or("not a JSON object")?;
        let known = ["title", "briefing", "goal", "truce_minutes", "names", "objective", "human"];
        if let Some(k) = obj.keys().find(|k| !known.contains(&k.as_str())) {
            return Err(format!("unknown key \"{k}\" (known: {})", known.join(", ")));
        }
        let text = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        Ok(Self {
            title: text("title"),
            briefing: text("briefing"),
            goal: text("goal"),
            truce_minutes: v.get("truce_minutes").and_then(|x| x.as_u64()).unwrap_or(0).min(240) as u32,
            names: v
                .get("names")
                .and_then(|x| x.as_array())
                .map(|a| a.iter().filter_map(|n| n.as_str().map(str::to_string)).collect())
                .unwrap_or_default(),
            objective: v.get("objective").map(objective_from).transpose()?,
            human: v.get("human").and_then(|x| x.as_u64()).map(|h| h.min(7) as u8),
        })
    }
}

#[cfg(test)]
mod map_mission_tests {
    use super::MapMission;

    /// Map maker round 1, #2: a map mission can set a real goal and the human's slot.
    #[test]
    fn a_map_mission_has_a_goal_and_a_human() {
        use super::Objective;
        for o in [
            Objective::DefeatAll,
            Objective::DefeatRival(2),
            Objective::HoldLand(40),
            Objective::Survive(60),
            Objective::Make(rr_rules::Ware::Coins, 30),
        ] {
            let m = MapMission { objective: Some(o), human: Some(1), ..Default::default() };
            assert_eq!(MapMission::from_json(&m.to_json()), Ok(m), "{o:?}");
        }
        let m = MapMission::from_json(r#"{"objective": {"defeat_rival": 2}, "human": 1}"#).unwrap();
        assert_eq!((m.objective, m.human), (Some(Objective::DefeatRival(2)), Some(1)));
        let e = MapMission::from_json(r#"{"objective": {"conquer": 2}}"#).unwrap_err();
        assert!(e.contains("objective"), "{e}");
    }

    #[test]
    fn a_map_mission_round_trips_and_tolerates_gaps() {
        let m = MapMission {
            title: "Two valleys".into(),
            briefing: "Hold the pass.".into(),
            goal: "Defeat the north.".into(),
            truce_minutes: 20,
            names: vec!["You".into(), "Hrothgar".into()],
            ..Default::default()
        };
        assert_eq!(MapMission::from_json(&m.to_json()), Ok(m));
        let partial = MapMission::from_json(r#"{"title": "Only a title", "truce_minutes": 9999}"#).unwrap();
        assert_eq!(partial.title, "Only a title");
        assert_eq!(partial.truce_minutes, 240);
        assert!(MapMission::from_json("nope").is_err());
        // map maker round 1, #3: a broken file says where, and a typo in a key says which
        let e = MapMission::from_json("{\"title\": 1,}").unwrap_err();
        assert!(e.contains("line 1"), "{e}");
        let e = MapMission::from_json(r#"{"title": "T", "truce": 20}"#).unwrap_err();
        assert!(e.contains("\"truce\""), "{e}");
        assert_eq!(
            MapMission::path_for(std::path::Path::new("/m/Two valleys.rrmap")),
            std::path::PathBuf::from("/m/Two valleys.mission.json")
        );
    }
}

#[cfg(test)]
mod campaign_tests {
    use super::*;

    /// New-player round 10, #5: the menu shows a campaign: an order, and each mission's own line.
    #[test]
    fn the_campaign_has_an_order_and_its_own_lines() {
        let order = CAMPAIGN;
        assert_eq!(order[..2], ["first", "europe"]);
        assert_eq!(order.len(), MISSIONS.len());
        for k in order {
            assert!(mission(k).is_some(), "{k}");
        }
        let hints: Vec<&str> = MISSIONS.iter().chain(ROME_MISSIONS.iter()).map(|m| m.hint).collect();
        for (i, a) in hints.iter().enumerate() {
            assert!(crate::i18n::has_german(a), "{a}");
            for b in &hints[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }
}

#[cfg(test)]
mod objective_tests {
    use super::*;

    fn sim(players: usize) -> rr_sim::Sim {
        let mut m = rr_core::MapData::flat(64, 64, 8);
        m.hq = (0..players).map(|i| Some(rr_core::MapCoord::new(10 + 20 * i as u16, 20))).collect();
        let setup = rr_sim::GameSetup { players: vec![Default::default(); players], ..Default::default() };
        rr_sim::Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &m, &setup).unwrap()
    }

    /// Architect round 3, #5 and #8: one rivals list and one land share for every goal (the
    /// free game's land goal read the last stats sample, the mission's the live map).
    #[test]
    fn one_rivals_list_and_one_land_share() {
        let mut s = sim(3);
        assert_eq!(rivals(&s, 0), vec![1, 2]);
        let land = land_by_player(&s);
        let total: u32 = land.iter().sum();
        assert_eq!(land_share(&s, 0), land[0] * 100 / total);
        // allies count together and are nobody's rivals
        let mut setup = rr_sim::GameSetup { players: vec![Default::default(); 3], ..Default::default() };
        setup.players[1].team = Some(1);
        setup.players[2].team = Some(1);
        let mut m = rr_core::MapData::flat(64, 64, 8);
        m.hq = (0..3).map(|i| Some(rr_core::MapCoord::new(10 + 20 * i as u16, 20))).collect();
        s = rr_sim::Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &m, &setup).unwrap();
        assert_eq!(rivals(&s, 1), vec![0]);
        assert_eq!(land_share(&s, 1), (land[1] + land[2]) * 100 / total);
        // before any sample is taken the land goal already counts (it read 0 % then)
        assert!(s.players()[0].stats.samples.is_empty() && land_share(&s, 0) > 0);
    }

    /// The menu's hover says who fights you at the chosen level (the level changes the cast,
    /// item 457, and the menu didn't say so).
    #[test]
    fn the_hover_names_the_cast() {
        let europe = mission("europe").unwrap();
        assert_eq!(
            cast_line(europe, rr_ai::Level::Easy),
            "Allied against you: Bjorn of Germania, Hadrian of Sarmatia. At peace with you: Livia of Iberia, Sigrid of Britain, Ragnar of Scandinavia."
        );
        assert!(!cast_line(europe, rr_ai::Level::Hard).contains("At peace"), "hard: all at war");
        assert!(cast_line(europe, rr_ai::Level::Easy).starts_with("Allied against you:"), "as the briefing says");
    }

    /// QA round 8, #5: a peaceful rival is nobody's ally: attacking an allied rival doesn't end
    /// its peace (they shared a team, and the attack ended the truce with all of them).
    #[test]
    fn peaceful_rivals_are_on_their_own() {
        let cast = [Stance::Peaceful, Stance::Allied, Stance::Allied, Stance::Peaceful];
        let teams: Vec<u8> = (0..4).map(|i| rival_team(&cast, i)).collect();
        assert_eq!(teams[1], teams[2], "the allies together");
        assert!(teams[0] != teams[1] && teams[3] != teams[1] && teams[0] != teams[3], "{teams:?}");
        assert!(teams.iter().all(|t| *t != 0), "nobody on the player's team");
    }

    /// Genre lessons (PLAN §27, #3): a small first mission teaches before Europe: one rival,
    /// a small map, its castle the goal.
    #[test]
    fn the_first_mission_teaches() {
        let m = mission("first").expect("the first mission");
        assert_eq!((m.rivals.len(), m.cast.len()), (1, 1));
        assert_eq!(m.objective, Objective::DefeatRival(1));
        let map = crate::load_map(&crate::Args { mission: Some("first".into()), seed: 1, ..Default::default() });
        assert_eq!(map.players(), 2);
        assert!(map.grid.width <= 72 && map.grid.height <= 72, "{}x{}", map.grid.width, map.grid.height);
    }

    /// The Roman campaign (docs/CAMPAIGN-ROME.md): its own order, its chapters in German, the
    /// first open from the start, each later one once the one before is won; a win leads on
    /// within the campaign, never into the other one.
    #[test]
    fn the_roman_campaign_opens_chapter_by_chapter() {
        assert_eq!(ROME.len(), ROME_MISSIONS.len());
        for (k, m) in ROME.iter().zip(ROME_MISSIONS.iter()) {
            assert_eq!(*k, m.key);
            assert!(
                m.gentle == ["augury", "sabines"].contains(&m.key),
                "{k}: the ★ chapters keep the peace at every level"
            );
            for t in [m.title, m.briefing, m.goal, m.hint, m.you.0] {
                assert!(crate::i18n::has_german(t), "{k}: {t}");
            }
            assert!(!CAMPAIGN.contains(k) && MISSIONS.iter().all(|w| w.key != *k), "{k} is in both campaigns");
        }
        assert!(unlocked("augury", &[]));
        assert!(unlocked("europe", &[]), "the world campaign is open");
        assert!(next_mission("augury").is_none_or(|m| ROME.contains(&m.key)));
        if let Some(second) = ROME.get(1) {
            assert!(!unlocked(second, &[]));
            assert!(unlocked(second, &["augury".to_string()]));
        }
    }

    /// Chapter 1 (docs/CAMPAIGN-ROME.md): the seven hills, Remus at peace at every level, a land
    /// goal of the whole map that isn't met at the start, and a cap that keeps it in reach.
    #[test]
    fn the_augury_can_always_be_won() {
        use rr_ai::Level;
        let m = mission("augury").unwrap();
        for level in [Level::Easy, Level::Normal, Level::Hard] {
            assert_eq!(m.cast_at(level), vec![Stance::Peaceful], "{level:?}");
        }
        let Objective::HoldMapLand(goal) = m.objective else { panic!("{:?}", m.objective) };
        assert!(m.land_cap > 0 && m.land_cap + goal <= 100, "Remus must leave room for the goal");
        let args = crate::Args { mission: Some("augury".into()), seed: 1, ..Default::default() };
        let map = crate::load_map(&args);
        assert_eq!((map.players(), map.grid.width), (2, 56));
        let sim = crate::new_game(&args);
        assert!(!progress(m.objective, &sim, 0).met, "met at the start");
    }

    /// Every Roman chapter's holy places stand on its map, on every seed: as many as its layout
    /// has, each its own god, its ring walkable.
    #[test]
    fn every_chapter_has_its_holy_places() {
        for m in ROME_MISSIONS.iter() {
            let layout = m.rome.expect("a Roman chapter has its layout");
            for seed in 1..=3 {
                let map = crate::load_map(&crate::Args { mission: Some(m.key.into()), seed, ..Default::default() });
                let w = rr_world::World::new(&map);
                let mut gods: Vec<u8> = w.holy_places().iter().map(|h| h.1).collect();
                let mut want: Vec<u8> = layout.holy().iter().map(|h| h.1).collect();
                gods.sort();
                want.sort();
                assert_eq!(gods, want, "{} seed {seed}", m.key);
                for (at, _) in w.holy_places() {
                    assert!(
                        map.grid.within(at, rr_world::HOLY_RING).all(|q| w.node_walkable(q)),
                        "{} seed {seed}",
                        m.key
                    );
                }
            }
        }
    }

    /// Chapter III (docs/CAMPAIGN-ROME.md): on every seed Juno's altar stands on the player's
    /// side of the way to Veii, on land, away from both castles.
    #[test]
    fn junos_altar_stands_on_the_way_to_veii() {
        for seed in 1..=6 {
            let args = crate::Args { mission: Some("veii".into()), seed, ..Default::default() };
            let map = crate::load_map(&args);
            let w = rr_world::World::new(&map);
            let holy = w.holy_places();
            assert_eq!(holy.iter().map(|h| h.1).collect::<Vec<_>>(), vec![2], "seed {seed}");
            let (g, at) = (map.grid, holy[0].0);
            let (me, veii) = (map.hq[0].unwrap(), map.hq[1].unwrap());
            assert!(g.distance(me, at) > 6 && g.distance(me, at) < g.distance(veii, at), "seed {seed}");
            assert!(g.within(at, rr_world::HOLY_RING).all(|q| w.node_walkable(q)), "seed {seed}");
        }
    }

    /// Game design round 1, #4: a win leads on to the next mission, the last to none.
    #[test]
    fn a_win_leads_on() {
        assert_eq!(next_mission("first").map(|m| m.key), Some("europe"));
        assert_eq!(next_mission("europe").map(|m| m.key), Some(CAMPAIGN[2]));
        assert!(next_mission(CAMPAIGN[CAMPAIGN.len() - 1]).is_none());
        assert!(next_mission("nowhere").is_none());
    }

    /// Game design round 1, #2: five missions, five designs: no two share their goal's kind and
    /// their cast, and every one has someone to fight.
    #[test]
    fn five_missions_five_designs() {
        let kind = |o: Objective| std::mem::discriminant(&o);
        for (i, a) in MISSIONS.iter().enumerate() {
            // (the first mission's one neighbour keeps the peace until you attack)
            assert!(a.cast.iter().any(|s| *s != Stance::Peaceful) || a.key == "first", "{}: nobody to fight", a.key);
            for b in &MISSIONS[i + 1..] {
                assert!(kind(a.objective) != kind(b.objective) || a.cast != b.cast, "{} and {}", a.key, b.key);
            }
        }
    }

    /// Game design round 1, #3: the level decides how many rivals are at war with you. Easy
    /// leaves at most two, hard makes peace with nobody; the goal's rival always fights.
    #[test]
    fn the_level_decides_who_fights() {
        use rr_ai::Level;
        let hostile = |c: Vec<Stance>| c.iter().filter(|s| **s != Stance::Peaceful).count();
        for m in MISSIONS {
            let (easy, normal, hard) = (m.cast_at(Level::Easy), m.cast_at(Level::Normal), m.cast_at(Level::Hard));
            assert_eq!(normal, m.cast, "{}", m.key);
            assert!((1..=2).contains(&hostile(easy.clone())) || m.key == "first", "{}: {easy:?}", m.key);
            assert!(hostile(hard.clone()) == m.cast.len() || m.key == "first", "{}", m.key);
            if m.key == "first" {
                assert_eq!((hostile(easy.clone()), hostile(hard.clone())), (0, 0), "the first valley stays at peace");
            }
            assert!(hostile(easy.clone()) <= hostile(normal.clone()) && hostile(normal) <= hostile(hard), "{}", m.key);
            if let Objective::DefeatRival(p) = m.objective
                && m.key != "first"
            {
                assert_ne!(easy[p as usize - 1], Stance::Peaceful, "{}: the goal's rival must fight", m.key);
            }
            // a rival keeps its side when it fights
            for (e, n) in easy.iter().zip(m.cast.iter().copied()) {
                assert!(*e == n || *e == Stance::Peaceful, "{}", m.key);
            }
        }
    }

    /// The briefing says who stands how only in its cast lines: its story promised five allies
    /// against you where three were at peace (Europe, the Far East, the North).
    #[test]
    fn briefings_promise_no_alliance() {
        for m in MISSIONS {
            assert!(!m.briefing.contains("against you") && !m.briefing.contains("united"), "{}", m.key);
        }
    }

    /// Game design round 1, #1: each kind of goal reports its progress, and is met when it is.
    #[test]
    fn objectives_report_their_progress() {
        let s = sim(3);
        let rival = progress(Objective::DefeatRival(2), &s, 0);
        assert!(!rival.met && rival.line.contains("{1}"), "{}", rival.line);
        // new-player round 11, #2: "take the castle" is met when the castle falls, not after
        // every last post (the tracker grew from 1 to 36 buildings left)
        // QA round 8, #2: a slot the map hasn't is no goal met at 00:00
        assert!(!progress(Objective::DefeatRival(6), &s, 0).met);
        // QA round 8, #4: huge minutes don't overflow
        assert!(!progress(Objective::Survive(u32::MAX), &s, 0).met);
        let mut s2 = sim(3);
        s2.debug_destroy_building(rr_core::MapCoord::new(50, 20));
        assert!(progress(Objective::DefeatRival(2), &s2, 0).met);
        assert!(!progress(Objective::DefeatAll, &s, 0).met);
        assert!(!progress(Objective::Survive(90), &s, 0).met);
        assert!(progress(Objective::Survive(0), &s, 0).met);
        let land = progress(Objective::HoldLand(30), &s, 0);
        assert!(land.met, "a third of all owned land at the start: {}", land.line);
        assert!(!progress(Objective::HoldLand(60), &s, 0).met);
        assert!(!progress(Objective::Make(rr_rules::Ware::Coins, 60), &s, 0).met);
    }
}

#[cfg(test)]
mod soak_tests {
    use super::*;

    /// Game design round 1, #1 (its acceptance): every mission's goal can be met in an evening.
    /// Each mission as the app sets it up (casts and truces at easy), a hard AI for the player
    /// that keeps the truce, seeds 1-3, up to 180 minutes: the minute its goal is met. (Item 478:
    /// Europe 62-67, the North 90, North America 96-145, the first valley 45-124, Mare Nostrum
    /// 100-176 once its coins counted minted, the Far East 63-85 once its goal was Lianhua's
    /// castle (Hideko's on Yamato in none, item 479); 16 of 18.)
    #[test]
    #[ignore = "18 whole games: cargo test --release -p rr-app --bin rr-app -- --ignored goals"]
    fn goals_are_met_in_an_evening() {
        let mut jobs = Vec::new();
        // RR_GOALS=augury,first: only these missions
        let only = std::env::var("RR_GOALS").ok();
        let wanted = |k: &str| only.as_deref().is_none_or(|o| o.split(',').any(|x| x == k));
        for m in MISSIONS.iter().chain(ROME_MISSIONS.iter()).filter(|m| wanted(m.key)) {
            // (RR_GOALS_SEEDS=6: more games each)
            let seeds = std::env::var("RR_GOALS_SEEDS").ok().and_then(|v| v.parse().ok()).unwrap_or(3u64);
            for seed in 1..=seeds {
                jobs.push((m.key, seed));
            }
        }
        let res: Vec<(&str, u64, Option<u32>)> = std::thread::scope(|sc| {
            let hs: Vec<_> = jobs
                .iter()
                .map(|&(key, seed)| {
                    sc.spawn(move || {
                        // (RR_GOALS_LEVEL=1: the rivals at normal, not easy)
                        let ai_level = std::env::var("RR_GOALS_LEVEL").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
                        let args = crate::Args { mission: Some(key.into()), seed, ai_level, ..Default::default() };
                        let mut sim = crate::new_game(&args);
                        let mut ais = crate::app::make_ais_for_test(&args, sim.players().len());
                        ais.push(rr_ai::Ai::with_level(0, rr_ai::Level::Hard));
                        let m = mission(key).unwrap();
                        // the settings the game gives the player at the start (the stand-in AI
                        // sets the inland and the middle itself, the border it leaves)
                        let values = crate::app::newcomer_military(sim.players()[0].settings.military, Some(m));
                        sim.step(&[rr_sim::PlayerCommand { player: 0, cmd: rr_sim::Cmd::SetMilitary { values } }]);
                        let truce = crate::app::truce_end(&ais, 0).unwrap_or(0);
                        // (and leaves a rival at peace for good alone where the goal isn't to fight
                        // it: it took Remus's castle)
                        let peace = m.cast_at(crate::level(ai_level)).iter().all(|s| !s.hostile())
                            && !matches!(
                                m.objective,
                                Objective::DefeatRival(_) | Objective::DefeatAll | Objective::Vanquish(_)
                            );
                        let mut met = None;
                        let mut deeds = 0u32;
                        while sim.gf().0 < 180 * 1200 && met.is_none() {
                            let mut cmds = Vec::new();
                            for ai in ais.iter_mut() {
                                cmds.extend(ai.think(&sim));
                            }
                            // (the stand-in keeps the truce, as a player would)
                            cmds.retain(|c| {
                                c.player != 0
                                    || (sim.gf().0 >= truce && !peace)
                                    || !matches!(c.cmd, rr_sim::Cmd::Attack { .. })
                            });
                            // the story's deeds as the app does them (Tarpeia: a post handed over, war)
                            if let Some(tale) = rr_game::story::tale(key).filter(|_| sim.gf().0 % 100 == 0) {
                                let seen = rr_game::story::Seen { minute: sim.gf().0 / 1200, ..Default::default() };
                                for (bit, deed) in rr_game::story::due_deeds(tale, &seen, deeds) {
                                    deeds |= bit;
                                    match deed {
                                        rr_game::story::Deed::HandOver { to, .. } => {
                                            if let Some(at) = rr_game::story::hand_over_spot(&sim, 0, to) {
                                                let cmd = rr_sim::Cmd::HandOver { at, to };
                                                cmds.push(rr_sim::PlayerCommand { player: 0, cmd });
                                            }
                                        }
                                        rr_game::story::Deed::Bless { .. } => {}
                                        rr_game::story::Deed::War { with } => {
                                            ais.iter_mut().filter(|a| a.player == with).for_each(|a| a.truce = None);
                                        }
                                    }
                                }
                            }
                            sim.step(&cmds);
                            if sim.gf().0 % 1200 == 0 && progress(m.objective, &sim, 0).met {
                                met = Some(sim.gf().0 / 1200);
                            }
                        }
                        if std::env::var_os("RR_GOALS_SHARES").is_some() {
                            let share = |p| rr_ai::map_land_share(&sim, p);
                            let posts = |p: u8| sim.buildings().iter().filter(|(_, b)| b.player == p && sim.rules().building(b.kind).is_military()).count();
                            let inv = sim.player_inventory(0);
                            let out: usize = sim.buildings().iter().filter(|(_, b)| b.player == 0).map(|(_, b)| b.soldiers.len()).sum();
                            eprintln!(
                                "{key} {seed}: gf {} shares {} {} posts {} {} soldiers home {} out {} boards {} stones {} vanquished {}",
                                sim.gf().0, share(0), share(1), posts(0), posts(1), inv.soldiers(), out,
                                inv.ware(rr_rules::Ware::Boards), inv.ware(rr_rules::Ware::Stones),
                                sim.players()[0].stats.vanquished
                            );
                        }
                        (key, seed, met)
                    })
                })
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        for r in &res {
            eprintln!("goal: {r:?}");
        }
        let met = res.iter().filter(|r| r.2.is_some()).count();
        assert!(met + 3 >= res.len(), "goals met in {met} of {} games", res.len());
    }
}
