//! The world campaign: real regions, their casts and goals, and how far a game is toward them.

use rr_sim::Nation;

pub struct Mission {
    pub key: &'static str,
    pub title: &'static str,
    pub briefing: &'static str,
    pub goal: &'static str,
    /// What sets this mission apart, for the menu (new-player round 10, #5).
    pub hint: &'static str,
    /// The rivals live far away: the war comes late (new-player round 10, #4).
    pub far: bool,
    /// The rival (player slot) that knows your coast and lands there by sea, on normal and hard
    /// (game design round 1, #7: Mare Nostrum had no war).
    pub invader: Option<u8>,
    /// What the mission asks for; after it, "defeat all" (game design round 1).
    pub objective: Objective,
    /// How each rival stands (game design round 1, #2: every mission was you against five allies).
    pub cast: &'static [Stance],
    pub you: (&'static str, Nation),
    pub rivals: &'static [(&'static str, Nation)],
    /// A random map of this size instead of a region of the world (0: the region).
    pub map_size: u16,
    /// Gold, silver and bronze: a win within so many minutes (on easy half as long again).
    pub medals: [u32; 3],
    /// The rivals place no new military building while they hold this share of the map's land,
    /// in percent (0: no cap), so a land goal can't become impossible.
    pub land_cap: u32,
    /// The map: the seven hills by the Tiber in one of their layouts (`rr_mapgen::rome`), at
    /// `map_size`.
    pub rome: Option<rr_mapgen::rome::RomeMap>,
    /// The rivals keep the peace at every level, as the cast says (a teaching mission: nothing
    /// can be lost while learning).
    pub gentle: bool,
    /// The hostile rivals' truce in minutes, if not the usual (on easy half as long again; the
    /// Gauls come early: with 30 minutes a 45-minute stand had almost no war).
    pub truce: Option<u32>,
}

/// The world campaign's medals: gold within 90 minutes, silver within 120, bronze within 180.
pub const WORLD_MEDALS: [u32; 3] = [90, 120, 180];

impl Mission {
    /// The invader at a level: none on easy.
    pub fn invader_at(&self, level: rr_ai::Level) -> Option<u8> {
        self.invader.filter(|_| level != rr_ai::Level::Easy)
    }

    /// The cast at a level (game design round 1, #3: difficulty decides who fights, not hidden
    /// holds). Normal is the mission as written; easy leaves at most two rivals at war with you,
    /// the goal's rival first; hard makes peace with nobody.
    pub fn cast_at(&self, level: rr_ai::Level) -> Vec<Stance> {
        let mut cast = self.cast.to_vec();
        // the teaching mission's neighbour keeps the peace at every level (new-player round 11,
        // #3: at hard he was "allied against you", next to a briefing saying he keeps the peace)
        if self.gentle {
            return cast;
        }
        match level {
            rr_ai::Level::Normal => {}
            rr_ai::Level::Hard => {
                for s in &mut cast {
                    if *s == Stance::Peaceful {
                        *s = Stance::Allied;
                    }
                }
            }
            rr_ai::Level::Easy => {
                let goal = match self.objective {
                    Objective::DefeatRival(p) => Some(p as usize - 1),
                    _ => None,
                };
                let order = goal.into_iter().chain((0..cast.len()).filter(|i| Some(*i) != goal));
                let mut fighting = 0;
                for i in order {
                    if cast[i].hostile() {
                        fighting += 1;
                        if fighting > 2 {
                            cast[i] = Stance::Peaceful;
                        }
                    }
                }
            }
        }
        cast
    }
}

pub const MISSIONS: [Mission; 6] = [
    Mission {
        key: "first",
        title: "The first valley",
        briefing: "A quiet valley, and one neighbour: Aldric of the Marches. Here you learn how a realm grows: woodcutters and a quarry, a sawmill, roads that carry everything, guardhouses that push the border out, and at last soldiers who take a castle.\n\nAldric keeps the peace as long as you do: nothing here is a race. Follow the First steps at the top right, and attack when your army is ready.",
        goal: "Goal: take the castle of Aldric of the Marches.",
        objective: Objective::DefeatRival(1),
        // (at peace until you attack: you can't lose while you learn, and the first attack
        // teaches that it ends the truce; the normal AI as a stand-in took the castle in 3 of 6
        // games within an hour, the easy one in none in three hours)
        cast: &[Stance::Peaceful],
        hint: "Learn to play: one neighbour, a small valley, a castle to take.",
        far: false,
        invader: None,
        you: ("You (the Valley)", Nation::Romans),
        rivals: &[("Aldric of the Marches", Nation::Vikings)],
        map_size: 64,
        medals: WORLD_MEDALS,
        land_cap: 0,
        rome: None,
        gentle: true,
        truce: None,
    },
    Mission {
        key: "europe",
        title: "The Europe mission",
        briefing: "Your people have settled in the hills of Italy. Beyond the Alps, across the Pyrenees and far in the north, five rulers watch your rise: Livia of Iberia, Sigrid of Britain, Ragnar of Scandinavia, Bjorn of Germania and Hadrian of Sarmatia.\n\nBuild roads, gather wood and stone, and push your borders north. The Alps hold the richest gold. Feed your miners, forge swords and shields, brew beer — and train an army strong enough to take Europe.",
        goal: "Goal: take the castle of Bjorn of Germania.",
        objective: Objective::DefeatRival(4),
        cast: &[Stance::Peaceful, Stance::Peaceful, Stance::Peaceful, Stance::Allied, Stance::Allied],
        hint: "The campaign's first war: rivals by land on every side.",
        far: false,
        invader: None,
        you: ("You (Italy)", Nation::Romans),
        rivals: &[
            ("Livia of Iberia", Nation::Romans),
            ("Sigrid of Britain", Nation::Vikings),
            ("Ragnar of Scandinavia", Nation::Vikings),
            ("Bjorn of Germania", Nation::Vikings),
            ("Hadrian of Sarmatia", Nation::Romans),
        ],
        map_size: 0,
        medals: WORLD_MEDALS,
        land_cap: 0,
        rome: None,
        gentle: false,
        truce: None,
    },
    Mission {
        key: "mediterranean",
        title: "Mare Nostrum",
        briefing: "Carthage rules the harbors of Africa, but the sea is crowded. Around its shores five powers each go their own way: Viriato of Hispania, Cornelia of Roma, Leonidas of Graecia, Tarhun of Anatolia and Nefret of Aegyptus.\n\nThe Sahara gives nothing; your wealth lies in the coastal plains and in the Atlas. Gibraltar, Messina and the Bosporus can be crossed on foot — or build harbors and send your ships.",
        goal: "Goal: mint 60 gold coins: rule the sea by trade.",
        objective: Objective::Make(rr_rules::Ware::Coins, 60),
        cast: &[Stance::Alone, Stance::Alone, Stance::Alone, Stance::Alone, Stance::Alone],
        hint: "Rivals far across the sea and the desert: harbors and expeditions; the war comes late.",
        far: true,
        invader: Some(5),
        you: ("You (Carthago)", Nation::Nubians),
        rivals: &[
            ("Viriato of Hispania", Nation::Romans),
            ("Cornelia of Roma", Nation::Romans),
            ("Leonidas of Graecia", Nation::Romans),
            ("Tarhun of Anatolia", Nation::Japanese),
            ("Nefret of Aegyptus", Nation::Nubians),
        ],
        map_size: 0,
        medals: WORLD_MEDALS,
        land_cap: 0,
        rome: None,
        gentle: false,
        truce: None,
    },
    Mission {
        key: "northamerica",
        title: "North America",
        briefing: "Your settlers have landed in Virginia, between the Appalachians and the sea. The continent is wide, and five rivals are already marching: Ezra of New England, Mary of the Great Lakes, Santiago of Texas, Rosa of California and Elias of Cascadia.\n\nThe plains feed many farms but little wood grows there. The Sierra Nevada and the Black Hills hide gold; the Appalachians are full of coal.",
        goal: "Goal: hold 30 % of the continent.",
        objective: Objective::HoldLand(30),
        cast: &[Stance::Alone, Stance::Alone, Stance::Alone, Stance::Peaceful, Stance::Peaceful],
        hint: "A wide continent: rivals by land, little wood on the plains.",
        far: false,
        invader: None,
        you: ("You (Virginia)", Nation::Romans),
        rivals: &[
            ("Ezra of New England", Nation::Vikings),
            ("Mary of the Great Lakes", Nation::Vikings),
            ("Santiago of Texas", Nation::Nubians),
            ("Rosa of California", Nation::Japanese),
            ("Elias of Cascadia", Nation::Vikings),
        ],
        map_size: 0,
        medals: WORLD_MEDALS,
        land_cap: 0,
        rome: None,
        gentle: false,
        truce: None,
    },
    Mission {
        key: "fareast",
        title: "The Far East",
        briefing: "Your people farm the Central Plain on the Yellow River. Five rulers share the land with you: Temur of Manchuria, Min-ho of Korea, Hideko of Yamato, Lianhua of Jiangnan and Wei of Lingnan.\n\nThe Gobi in the north is barren; the Qinling and the Khingan hide gold. Yamato lies across the sea — only a harbor and a ship will bring your soldiers there.",
        // (Lianhua, not Hideko: Yamato's harbor spots were all Hideko's by minute 40-100 in
        // three games, so no expedition could land; without sea attacks his castle was out of
        // reach. Lianhua fell at minute 63-72 in the same games, Temur in none; item 479)
        goal: "Goal: take the castle of Lianhua of Jiangnan, in the south.",
        objective: Objective::DefeatRival(4),
        cast: &[Stance::Alone, Stance::Alone, Stance::Allied, Stance::Alone, Stance::Alone],
        hint: "Lianhua in the south is your goal; Yamato across the sea fills its coasts early.",
        far: false,
        invader: None,
        you: ("You (Zhongyuan)", Nation::Japanese),
        rivals: &[
            ("Temur of Manchuria", Nation::Vikings),
            ("Min-ho of Korea", Nation::Japanese),
            ("Hideko of Yamato", Nation::Japanese),
            ("Lianhua of Jiangnan", Nation::Japanese),
            ("Wei of Lingnan", Nation::Nubians),
        ],
        map_size: 0,
        medals: WORLD_MEDALS,
        land_cap: 0,
        rome: None,
        gentle: false,
        truce: None,
    },
    Mission {
        key: "north",
        title: "The North",
        briefing: "Winter lies over Svealand, and your people must build in the snow. Five rulers of the North watch you: Halvard of Norvegr, Aino of Suomi, Talis of Livonia, Yaroslav of Novgorod and Niila of Sapmi.\n\nThe taiga gives all the wood you want, but little grows in the fields. The Scandes hide gold; the Khibiny in the far north hold iron and more.",
        goal: "Goal: hold out 90 minutes in the snow.",
        objective: Objective::Survive(90),
        cast: &[Stance::Allied, Stance::Allied, Stance::Peaceful, Stance::Peaceful, Stance::Allied],
        hint: "Snow: wood everywhere, little grain; rivals close by land.",
        far: false,
        invader: None,
        you: ("You (Svealand)", Nation::Vikings),
        rivals: &[
            ("Halvard of Norvegr", Nation::Vikings),
            ("Aino of Suomi", Nation::Vikings),
            ("Talis of Livonia", Nation::Romans),
            ("Yaroslav of Novgorod", Nation::Romans),
            ("Niila of Sapmi", Nation::Vikings),
        ],
        map_size: 0,
        medals: WORLD_MEDALS,
        land_cap: 0,
        rome: None,
        gentle: false,
        truce: None,
    },
];

/// The campaign's order: Europe first, the sea missions last (new-player round 10, #4 and #5:
/// Mare Nostrum, second in the menu, had no contact for two hours).
pub const CAMPAIGN: [&str; 6] = ["first", "europe", "northamerica", "north", "fareast", "mediterranean"];

/// How a mission's rival stands toward you and the others.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stance {
    /// Allied with the other allies against you.
    Allied,
    /// On its own: at war with everybody, you too.
    Alone,
    /// Leaves you alone (defends itself, may fight the others).
    Peaceful,
    /// Allied with you (your team: the Roman campaign's allied towns of the Appian Way).
    Friend,
}

impl Stance {
    /// At war with you.
    pub fn hostile(self) -> bool {
        matches!(self, Stance::Allied | Stance::Alone)
    }
}

/// The nodes each player owns.
pub fn land_by_player(sim: &rr_sim::Sim) -> Vec<u32> {
    sim.land_by_player()
}

/// The living players not allied with `me`: the one "is a rival" of the goals and news.
pub fn rivals(sim: &rr_sim::Sim, me: u8) -> Vec<u8> {
    let players = sim.players();
    (0..players.len() as u8).filter(|p| players[*p as usize].alive && !sim.are_allies(*p, me)).collect()
}

/// The share of all owned land that `me` and its allies hold, in percent (0 when nobody holds any).
pub fn land_share(sim: &rr_sim::Sim, me: u8) -> u32 {
    let land = land_by_player(sim);
    let total: u32 = land.iter().sum();
    let ours: u32 = (0..land.len()).filter(|p| sim.are_allies(*p as u8, me)).map(|p| land[p]).sum();
    (ours * 100).checked_div(total).unwrap_or(0)
}

/// How many allied realms `me`'s land touches (a node of ours beside a node of theirs).
pub fn reached_allies(sim: &rr_sim::Sim, me: u8) -> u32 {
    let w = sim.world();
    let g = w.grid();
    let mut touched = std::collections::BTreeSet::new();
    for p in g.coords().filter(|p| w.owner(*p) == Some(me)) {
        for q in g.neighbors(p) {
            if let Some(o) = w.owner(q).filter(|o| *o != me && sim.are_allies(*o, me)) {
                touched.insert(o);
            }
        }
    }
    touched.len() as u32
}

/// The team of a mission's rival `i` (you are team 0).
pub fn rival_team(cast: &[Stance], i: usize) -> u8 {
    match cast[i] {
        Stance::Allied => 1,
        // (a peaceful rival on the allies' team lost its peace with them: QA round 8, #5)
        Stance::Alone | Stance::Peaceful => 2 + i as u8,
        Stance::Friend => 0,
    }
}

/// What a mission asks for (game design round 1, #1: "defeat all five" took no player to an end
/// in an evening). Met, the game offers to go on and conquer all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Objective {
    DefeatAll,
    /// The rival of this player slot is defeated.
    DefeatRival(u8),
    /// This share of the players' land, in percent.
    HoldLand(u32),
    /// Still standing after this many minutes.
    Survive(u32),
    /// This many of a ware made (not kept in store: coins went to promotions as they came, and
    /// no stand-in ever had 60 in store; item 478).
    Make(rr_rules::Ware, u32),
    /// This share of the whole map's land, in percent (`rr_ai::map_land_share`): with two
    /// realms a share of the land the players hold is met at the start.
    HoldMapLand(u32),
    /// This many rival military buildings taken or burnt (the statistics' "vanquished").
    Vanquish(u32),
    /// This many allied realms reached: our land touches theirs (the Appian Way's towns).
    Reach(u32),
}

/// How far a goal is: met or not, and the tracker's line (a stored message, see `msg`).
pub struct Progress {
    pub met: bool,
    pub line: String,
}

/// The progress toward `obj` of player `me`.
pub fn progress(obj: Objective, sim: &rr_sim::Sim, me: u8) -> Progress {
    use crate::msg;
    let players = sim.players();
    match obj {
        Objective::DefeatAll => {
            let left = rivals(sim, me).len();
            Progress { met: left == 0, line: msg("Goal: defeat all rivals ({1} left)", &[&left.to_string()]) }
        }
        Objective::DefeatRival(p) => {
            // the goal says "take the castle": met when it falls (new-player round 11, #2: it
            // waited for every last post, 36 of them in a slow game)
            let Some(rival) = players.get(p as usize) else {
                return Progress { met: false, line: msg("Goal: take the castle of {1}", &["?"]) };
            };
            let alive = rival.alive;
            let castle =
                sim.buildings().iter().any(|(_, b)| b.player == p && b.kind == rr_rules::BuildingType::Headquarters);
            let name = players.get(p as usize).map_or(String::new(), |x| x.setup.name.clone());
            Progress { met: !alive || !castle, line: msg("Goal: take the castle of {1}", &[&crate::plain(&name)]) }
        }
        Objective::HoldLand(percent) => {
            let now = land_share(sim, me);
            Progress {
                met: now >= percent,
                line: msg("Goal: hold {1} % of the land ({2} % now)", &[&percent.to_string(), &now.to_string()]),
            }
        }
        Objective::Survive(minutes) => {
            let left = crate::minutes(minutes).saturating_sub(sim.gf().0);
            let alive = players.get(me as usize).is_some_and(|x| x.alive);
            Progress {
                met: left == 0 && alive,
                line: msg("Goal: hold out {1} minutes ({2} left)", &[&minutes.to_string(), &(left / 1200).to_string()]),
            }
        }
        Objective::Make(ware, n) => {
            let have = players.get(me as usize).map_or(0, |p| p.stats.produced_by_ware[ware.index()]);
            let name = crate::ware_name_en(ware);
            // coins are minted, in German too (German round 1, #9: "60 Münzen herstellen")
            let line = if ware == rr_rules::Ware::Coins {
                msg("Goal: mint {1} gold coins ({2} so far)", &[&n.to_string(), &have.to_string()])
            } else {
                msg("Goal: make {1} {2} ({3} so far)", &[&n.to_string(), &name, &have.to_string()])
            };
            Progress { met: have >= n, line }
        }
        Objective::Reach(n) => {
            let done = reached_allies(sim, me);
            Progress {
                met: done >= n,
                line: msg("Goal: reach {1} allied towns ({2} so far)", &[&n.to_string(), &done.to_string()]),
            }
        }
        Objective::Vanquish(n) => {
            let done = players.get(me as usize).map_or(0, |p| p.stats.vanquished);
            let line = if n == 1 {
                msg("Goal: take one rival military building ({1} so far)", &[&done.to_string()])
            } else {
                msg("Goal: take {1} rival military buildings ({2} so far)", &[&n.to_string(), &done.to_string()])
            };
            Progress { met: done >= n, line }
        }
        Objective::HoldMapLand(percent) => {
            let now = rr_ai::map_land_share(sim, me);
            Progress {
                met: now >= percent,
                line: msg("Goal: hold {1} % of the land ({2} % now)", &[&percent.to_string(), &now.to_string()]),
            }
        }
    }
}

/// The mission after `key` in its campaign, if any (game design round 1, #4: a win led nowhere).
pub fn next_mission(key: &str) -> Option<&'static Mission> {
    let order = campaign_of(key)?;
    let i = order.iter().position(|k| *k == key)?;
    order.get(i + 1).and_then(|k| mission(k))
}

pub fn mission(key: &str) -> Option<&'static Mission> {
    MISSIONS.iter().chain(ROME_MISSIONS.iter()).find(|m| m.key == key)
}

/// The campaign order a mission belongs to.
pub fn campaign_of(key: &str) -> Option<&'static [&'static str]> {
    [&CAMPAIGN[..], &ROME[..]].into_iter().find(|o| o.contains(&key))
}

/// Whether a mission can be played: the world campaign's are all open; a Roman chapter opens
/// once the one before it is won (the easy chapters teach what the later ones ask for).
pub fn unlocked(key: &str, won: &[String]) -> bool {
    match ROME.iter().position(|k| *k == key) {
        Some(0) | None => true,
        Some(i) => won.iter().any(|w| w == ROME[i - 1]),
    }
}

/// The Roman campaign's chapters in order (docs/CAMPAIGN-ROME.md; more as they are built).
pub const ROME: [&str; 10] =
    ["augury", "sabines", "veii", "gauls", "appian", "pyrrhus", "mylae", "hannibal", "carthage", "janus"];

/// The Roman campaign: real history from the founding of the city to Augustus, one chapter
/// per skill, gentle first (docs/CAMPAIGN-ROME.md).
pub const ROME_MISSIONS: [Mission; 10] = [
    Mission {
        key: "augury",
        title: "The Augury",
        briefing: "Two brothers stand on two hills above the Tiber, Romulus and Remus. Both want to found the city, and they leave it to the gods: each watches the sky from his own hill for a sign. You are Romulus, on the Palatine. Remus builds on the Aventine, at peace with you.\n\nHere you learn how a realm grows: woodcutters and a quarry, a sawmill, roads that carry everything, and guardhouses that push your border over the hills. Whoever holds the hills will hold the city.",
        goal: "Goal: hold 35 % of the land.",
        objective: Objective::HoldMapLand(35),
        cast: &[Stance::Peaceful],
        hint: "The Roman campaign begins: build, connect and grow over the seven hills.",
        far: false,
        invader: None,
        you: ("Romulus", Nation::Romans),
        rivals: &[("Remus", Nation::Romans)],
        // (56 nodes: the 52 soldiers of the start hold 35 % with posts to spare; on 64 the player's
        // stand-in ran out at 30-36 %, there being no mountains for weapons)
        map_size: 56,
        medals: [40, 60, 90],
        // (Remus stops at 35 %: the 35 % of the goal always stay within reach)
        land_cap: 35,
        rome: Some(rr_mapgen::rome::RomeMap::Augury),
        gentle: true,
        truce: None,
    },
    Mission {
        key: "sabines",
        title: "The Sabine Peace",
        briefing: "Rome has walls now, and too few women. At the feast of Consus your men seized the Sabines' daughters and made them their wives, and their fathers have not forgiven it. The Sabine king Titus Tatius rules the Quirinal across the valley, at peace with you for now.\n\nHere you learn war: a brewery for beer, swords and shields from your store, soldiers recruited in the castle, and your first attack. Keep your posts manned: Tatius will not wait forever. Between the two hills stands Concordia's shrine: where your land meets the Sabines' there, the women carry gifts across, and the peace comes sooner.",
        goal: "Goal: take two Sabine military buildings.",
        objective: Objective::Vanquish(2),
        cast: &[Stance::Peaceful],
        hint: "Chapter II: soldiers, a betrayal, and a first war that ends in peace.",
        far: false,
        invader: None,
        you: ("Romulus", Nation::Romans),
        rivals: &[("Titus Tatius", Nation::Romans)],
        map_size: 64,
        medals: [40, 60, 90],
        land_cap: 0,
        rome: Some(rr_mapgen::rome::RomeMap::Sabines),
        gentle: true,
        truce: None,
    },
    Mission {
        key: "veii",
        title: "Veii",
        briefing: "Veii, the rich Etruscan city, stands on its rock across the valley, behind posts manned by trained soldiers. Its king will not wait for you: after the truce, Veii fights.\n\nHere you learn the weapons chain: send geologists into the hills, build mines and feed their miners (fish, meat or bread), smelt iron with coal, and let the armory make swords and shields. Only an army of many soldiers takes Veii. Juno's old altar stands on your side of the valley: where your land lies all around it, offerings of bread win her over.",
        goal: "Goal: take the castle of Veii.",
        objective: Objective::DefeatRival(1),
        cast: &[Stance::Alone],
        hint: "Chapter III: geologists, mines and the weapons chain against a fortified city.",
        far: false,
        invader: None,
        you: ("Camillus", Nation::Romans),
        rivals: &[("Veii", Nation::Romans)],
        map_size: 80,
        medals: [60, 90, 120],
        land_cap: 0,
        rome: Some(rr_mapgen::rome::RomeMap::Veii),
        gentle: false,
        truce: None,
    },
    Mission {
        key: "gauls",
        title: "The Geese of the Capitol",
        briefing: "The Gauls have beaten Rome's army at the Allia and are coming for the city: two war bands of the Senones, stronger than you and soon here. You cannot beat them in the field. Hold the hills until Camillus comes with an army.\n\nHere you learn defence: man the posts that face the north, set the military window so the border holds more soldiers than the inland, keep beer, swords and shields coming for new recruits, and use catapults. Juno's temple on the Capitol is yours: feed her sacred geese with grain, and they may wake the guards.",
        goal: "Goal: hold out 45 minutes until Camillus comes.",
        objective: Objective::Survive(45),
        cast: &[Stance::Allied, Stance::Allied],
        hint: "Chapter IV: defence. Hold the hills against a stronger invader.",
        far: false,
        invader: None,
        you: ("Marcus Manlius", Nation::Romans),
        rivals: &[("Brennus", Nation::Vikings), ("The Senones", Nation::Vikings)],
        map_size: 64,
        // (a stand of 45 minutes: whoever stands is in time)
        medals: [45, 45, 45],
        land_cap: 0,
        rome: Some(rr_mapgen::rome::RomeMap::Gauls),
        gentle: false,
        truce: Some(10),
    },
    Mission {
        key: "appian",
        title: "The Appian Way",
        briefing: "The Samnites hold the mountains, and once they trapped a Roman army in the pass of the Caudine Forks. Never again, says the censor Appius Claudius: Rome will build a road, paved and straight, to its allies in the south, so that the legions reach them in days, not weeks.\n\nHere you learn logistics: long roads, flags close together, donkeys on the busy roads (a donkey breeder), warehouses on the way, and the transport window's order. Reach the allied towns of Capua, Cumae and Neapolis: your land must touch theirs. The Samnites will try to cut your way. Janus's gates stand on the road: hold them, and the god of beginnings sends you helpers.",
        goal: "Goal: reach the three allied towns: your land must touch theirs.",
        objective: Objective::Reach(3),
        cast: &[Stance::Friend, Stance::Friend, Stance::Friend, Stance::Alone],
        hint: "Chapter V: long roads, donkeys and warehouses to reach three allied towns.",
        far: false,
        invader: None,
        you: ("Appius Claudius", Nation::Romans),
        rivals: &[
            ("Capua", Nation::Romans),
            ("Cumae", Nation::Romans),
            ("Neapolis", Nation::Romans),
            ("The Samnites", Nation::Romans),
        ],
        map_size: 96,
        medals: [60, 90, 120],
        land_cap: 0,
        rome: Some(rr_mapgen::rome::RomeMap::Appian),
        gentle: false,
        truce: None,
    },
    Mission {
        key: "pyrrhus",
        title: "A Pyrrhic Victory",
        briefing: "King Pyrrhus of Epirus has crossed to Italy to fight for the Greek cities of the south, with elephants and an army trained in every war since Alexander. His soldiers are few, but each is worth three of yours.\n\nHere you learn soldier quality: gold mines and a mint, coins for the military buildings so their soldiers are promoted, and attacks with your strongest soldiers. The Field of Mars lies near Rome: while six or more soldiers wait in your warehouses, they drill there, Mars's favour grows, and he promotes some of them.",
        goal: "Goal: take the castle of Pyrrhus.",
        objective: Objective::DefeatRival(1),
        cast: &[Stance::Alone],
        hint: "Chapter VI: soldier quality, gold and promotions against a master of war.",
        far: false,
        invader: None,
        you: ("Curius", Nation::Romans),
        rivals: &[("Pyrrhus", Nation::Romans)],
        map_size: 80,
        medals: [60, 90, 120],
        land_cap: 0,
        rome: Some(rr_mapgen::rome::RomeMap::Pyrrhus),
        gentle: false,
        truce: None,
    },
    Mission {
        key: "mylae",
        title: "Mylae",
        briefing: "Carthage rules the sea, and its fleet has never lost. Rome has no ships at all. You, Gaius Duilius, will change that: build a harbour on a free coast, switch a shipyard to sea ships, and carry your soldiers across.\n\nHere you learn seafaring: harbour spots (the anchors), shipyards, expeditions that found a colony on another coast, and attacks from the sea. Neptune's shrine stands on a coast between you and Carthage: offer fish there, and he may send you sailors.",
        goal: "Goal: take a Carthaginian military building across the sea.",
        objective: Objective::Vanquish(1),
        cast: &[Stance::Alone],
        hint: "Chapter VII: harbours, ships and attacks across the sea.",
        far: true,
        invader: None,
        you: ("Gaius Duilius", Nation::Romans),
        rivals: &[("Carthage", Nation::Nubians)],
        map_size: 96,
        medals: [90, 120, 150],
        land_cap: 0,
        rome: Some(rr_mapgen::rome::RomeMap::Mylae),
        gentle: false,
        truce: None,
    },
    Mission {
        key: "hannibal",
        title: "Hannibal at the Gates",
        briefing: "Hannibal has crossed the Alps with his army and his elephants and beaten every army Rome sent against him. You are Quintus Fabius Maximus, dictator: do not meet him in the open. Hold the strong places, let him come to you, and let Italy itself wear him down.\n\nHold out fifty minutes, until Scipio is ready to sail for Africa. Cybele's temple stands near Rome: the Sibylline books say the invader leaves Italy only when the Great Mother comes to Rome. Hold it, and Rome's people rally to you.",
        goal: "Goal: hold out 50 minutes, until Scipio sails for Africa.",
        objective: Objective::Survive(50),
        cast: &[Stance::Alone],
        hint: "Chapter VIII: hold out against Hannibal, and let time fight for you.",
        far: false,
        invader: None,
        you: ("Fabius", Nation::Romans),
        rivals: &[("Hannibal", Nation::Nubians)],
        map_size: 96,
        medals: [50, 50, 50],
        land_cap: 0,
        rome: Some(rr_mapgen::rome::RomeMap::Hannibal),
        gentle: false,
        truce: Some(15),
    },
    Mission {
        key: "carthage",
        title: "Carthago delenda est",
        briefing: "Fifty years after Zama, Carthage is rich again, and in the senate old Cato ends every speech with the same words: Carthage must be destroyed. The senate sends you, Scipio Aemilianus. King Massinissa of Numidia fights on your side.\n\nThis chapter asks for everything at once: an economy that feeds an army, soldiers of high rank, and a long siege. The Field of Mars near Rome drills your waiting soldiers. Carthage's temple stands near its walls: where your land closes around it, its god no longer guards the city.",
        goal: "Goal: take the castle of Carthage.",
        objective: Objective::DefeatRival(1),
        cast: &[Stance::Alone, Stance::Friend],
        hint: "Chapter IX: everything you have learned, in a long war on land.",
        far: false,
        invader: None,
        you: ("Scipio Aemilianus", Nation::Romans),
        rivals: &[("Carthage", Nation::Nubians), ("Numidia", Nation::Nubians)],
        map_size: 112,
        medals: [90, 120, 150],
        land_cap: 0,
        rome: Some(rr_mapgen::rome::RomeMap::Carthage),
        gentle: false,
        truce: None,
    },
    Mission {
        key: "janus",
        title: "The Gates of Janus",
        briefing: "After a century of civil wars two men are left: you, Octavian, in Rome, and Mark Antony, Cleopatra's ally, his legions gathered in the north and his fleet in Greece. The Cantabri in Spain have never been tamed, and beyond the Rhine and the Channel the Germans and the Britons watch.\n\nEverything you have learned, on the whole Mediterranean. When Antony falls, the doors of the temple of Janus can close: Rome at peace everywhere, for the first time in two hundred years. Janus's gates stand near Rome: hold them, and veterans of every war come to serve you.",
        goal: "Goal: take Mark Antony's castle. Then the gates of Janus close.",
        objective: Objective::DefeatRival(4),
        cast: &[Stance::Alone, Stance::Peaceful, Stance::Peaceful, Stance::Alone, Stance::Peaceful],
        hint: "Chapter X: the whole Mediterranean, and at the end, peace.",
        far: false,
        invader: None,
        you: ("Octavian", Nation::Romans),
        rivals: &[
            ("The Cantabri", Nation::Vikings),
            ("The Britons", Nation::Vikings),
            ("The Suebi", Nation::Vikings),
            ("Mark Antony", Nation::Romans),
            ("The Dacians", Nation::Vikings),
        ],
        map_size: 0,
        medals: [120, 150, 180],
        land_cap: 0,
        rome: Some(rr_mapgen::rome::RomeMap::Janus),
        gentle: false,
        truce: None,
    },
];
