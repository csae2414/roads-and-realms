//! The game around the simulation, without a window: the campaign's missions, casts and goals,
//! and how a mission sets up its players and computer players. The app and the command-line
//! tools both build their games from here (architect round 3, #4: the newcomer eval played a
//! different Europe than the app). Texts are English keys; the app translates them.

pub mod missions;
pub mod story;

use missions::{Mission, Stance};
use rr_ai::Level;
use rr_rules::Ware;

/// Game frames in a minute at normal speed.
pub const MINUTE: u32 = 60 * 20;

/// `n` minutes in game frames, saturating (architect round 3, #11: written out by hand 35 times).
pub const fn minutes(n: u32) -> u32 {
    n.saturating_mul(MINUTE)
}

/// Separates a message's template from its arguments.
pub const SEP: char = '\u{1f}';

/// Marks a message argument as plain text, never looked up (a player's name: architect round
/// 3, #12, a player named "Wood" showed as "Holz").
pub const PLAIN: char = '\u{2}';

/// A message argument shown as it is (`PLAIN`).
pub fn plain(s: &str) -> String {
    format!("{PLAIN}{s}")
}

/// A message kept in English so it can be shown in whatever language is chosen later: the
/// template (with `{1}`, `{2}` where the arguments go, in each language's own word order) and
/// arguments that are English keys themselves (or numbers and names, shown as they are).
pub fn msg(template: &str, args: &[&str]) -> String {
    let mut s = template.to_string();
    for a in args {
        s.push(SEP);
        s.push_str(a);
    }
    s
}

/// Does this command break a peace time that lasts until `until`? Attacks do, and catapults,
/// which fire on their own once built. (The one peace filter: the app's and the command line's
/// games; architect round 3, #4.)
pub fn breaks_peace(cmd: &rr_sim::Cmd, gf: u32, until: u32) -> bool {
    gf < until
        && matches!(
            cmd,
            rr_sim::Cmd::Attack { .. } | rr_sim::Cmd::PlaceBuilding { kind: rr_rules::BuildingType::Catapult, .. }
        )
}

/// A ware's English name, the key of its translations ("Rolling pin", "Iron ore").
pub fn ware_name_en(w: Ware) -> String {
    if w == Ware::Rollingpin {
        return "Rolling pin".into();
    }
    let n = w.name();
    let mut out = String::new();
    for (i, c) in n.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push(' ');
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// A computer player's truce with the human: until a frame, or (a peaceful rival's) for good.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Truce {
    Until(u32),
    ForGood,
}

impl Truce {
    /// The frame as the AI keeps it (for good: half the range, the AI adds its graces to the end).
    pub fn gf(self) -> u32 {
        match self {
            Truce::Until(gf) => gf,
            Truce::ForGood => u32::MAX / 2,
        }
    }

    /// The kind of an AI's truce frame (one longer than a quarter of the range is for good).
    pub fn of(gf: u32) -> Self {
        if gf >= u32::MAX / 4 { Truce::ForGood } else { Truce::Until(gf) }
    }
}

/// A mission's map: its region of the world, or a random one of its size (the first, small
/// mission: genre lessons, PLAN §27, #3).
pub fn mission_map(m: &Mission, seed: u64) -> rr_core::MapData {
    let mut map = mission_land(m, seed);
    // the seven hills' holy place: Jupiter's augurs on the Capitol
    if m.seven_hills {
        rr_mapgen::rome::holy_places(&mut map);
    }
    map
}

fn mission_land(m: &Mission, seed: u64) -> rr_core::MapData {
    match rr_mapgen::world::region(m.key).filter(|_| m.map_size == 0) {
        Some(r) => rr_mapgen::region::generate(r, seed),
        None => rr_mapgen::generate(&rr_mapgen::GenParams {
            width: m.map_size,
            height: m.map_size,
            players: m.rivals.len() + 1,
            seed,
            seven_hills: m.seven_hills,
            ..Default::default()
        }),
    }
}

/// A mission's players in slot order: the human first (team 0), then the rivals as the level
/// casts them (game design round 1, #2 and #3): name key, nation, team.
pub fn mission_players(m: &Mission, level: Level) -> Vec<(&'static str, rr_sim::Nation, u8)> {
    let cast = m.cast_at(level);
    let teams = std::iter::once(0).chain((0..m.rivals.len()).map(|i| missions::rival_team(&cast, i)));
    std::iter::once(m.you).chain(m.rivals.iter().copied()).zip(teams).map(|((n, nat), t)| (n, nat, t)).collect()
}

/// The truce a mission's rival in slot `p` keeps with the human in slot 0: easy opponents leave
/// a newcomer an hour (new-player round 7, #2: half an hour lost the first guardhouse minutes
/// after it), a peaceful rival keeps it for good. None for the human's own slot.
pub fn mission_truce(m: &Mission, level: Level, p: u8) -> Option<Truce> {
    let i = (p as usize).checked_sub(1)?;
    let peaceful = m.cast_at(level).get(i) == Some(&Stance::Peaceful);
    let hostile = if level == Level::Easy { 2 * rr_ai::MISSION_TRUCE } else { rr_ai::MISSION_TRUCE };
    Some(if peaceful { Truce::ForGood } else { Truce::Until(hostile) })
}

/// Gives the computer players of a mission at `level` their truce with the human and arms the
/// invader (it knows the human's coast; not saved with the AIs, so set again on a load).
pub fn arm_mission(m: &Mission, level: Level, ais: &mut [rr_ai::Ai]) {
    let invader = m.invader_at(level);
    for ai in ais.iter_mut() {
        if let Some(t) = mission_truce(m, level, ai.player) {
            ai.truce = Some((0, t.gf()));
        }
        ai.invade = (Some(ai.player) == invader).then_some(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Architect round 3, #8: a truce for good is a kind, not a number to compare.
    #[test]
    fn a_truce_for_good_is_a_kind() {
        for t in [Truce::Until(0), Truce::Until(rr_ai::MISSION_TRUCE), Truce::Until(minutes(24 * 60)), Truce::ForGood] {
            assert_eq!(Truce::of(t.gf()), t);
        }
    }

    /// Europe at normal: three rivals at peace for good, two allied against you for half an hour.
    #[test]
    fn a_mission_sets_up_its_players() {
        let europe = missions::mission("europe").unwrap();
        let players = mission_players(europe, Level::Normal);
        assert_eq!(players.len(), 6);
        assert_eq!(players[0].2, 0, "you on team 0");
        let mut ais: Vec<rr_ai::Ai> = (1..6).map(rr_ai::Ai::new).collect();
        arm_mission(europe, Level::Normal, &mut ais);
        let kinds: Vec<Truce> = ais.iter().map(|a| Truce::of(a.truce.unwrap().1)).collect();
        assert_eq!(kinds.iter().filter(|k| **k == Truce::ForGood).count(), 3, "{kinds:?}");
        assert_eq!(kinds.iter().filter(|k| **k == Truce::Until(rr_ai::MISSION_TRUCE)).count(), 2);
        assert_eq!(mission_truce(europe, Level::Easy, 0), None);
    }
}
