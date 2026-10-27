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
    /// The map: the seven hills by the Tiber (`rr_mapgen::rome`), at `map_size`.
    pub seven_hills: bool,
    /// The rivals keep the peace at every level, as the cast says (a teaching mission: nothing
    /// can be lost while learning).
    pub gentle: bool,
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
                    if cast[i] != Stance::Peaceful {
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
        seven_hills: false,
        gentle: true,
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
        seven_hills: false,
        gentle: false,
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
        seven_hills: false,
        gentle: false,
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
        seven_hills: false,
        gentle: false,
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
        seven_hills: false,
        gentle: false,
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
        seven_hills: false,
        gentle: false,
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

/// The team of a mission's rival `i` (you are team 0).
pub fn rival_team(cast: &[Stance], i: usize) -> u8 {
    match cast[i] {
        Stance::Allied => 1,
        // (a peaceful rival on the allies' team lost its peace with them: QA round 8, #5)
        Stance::Alone | Stance::Peaceful => 2 + i as u8,
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
pub const ROME: [&str; 1] = ["augury"];

/// The Roman campaign: real history from the founding of the city to Augustus, one chapter
/// per skill, gentle first (docs/CAMPAIGN-ROME.md).
pub const ROME_MISSIONS: [Mission; 1] = [Mission {
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
    seven_hills: true,
    gentle: true,
}];
