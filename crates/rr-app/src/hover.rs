//! What the map says about the point under the pointer, when it rests there a moment (owner,
//! 2026-10-27: the hover showed only a circle): what stands there, what a geologist's sign
//! means, and what can be built there.

use crate::i18n::{msg, show, tr};
use rr_core::MapCoord;
use rr_rules::BuildingType as B;
use rr_world::{Bq, NodeObj};

/// The lines for `p` as `player` sees it (empty where they have seen nothing).
pub fn lines(sim: &rr_sim::Sim, player: u8, p: MapCoord) -> Vec<String> {
    if sim.visibility(player, p) == rr_sim::Visibility::Unexplored {
        return Vec::new();
    }
    if let Some(god) = rr_world::holy_god(sim.world().obj(p)) {
        return holy_lines(sim, player, p, god);
    }
    let mut out = Vec::new();
    if let Some(what) = what_stands(sim, player, p) {
        out.push(what);
    }
    out.extend(sign_lines(sim, p));
    // (on a building or a flag, "nothing can be built here" goes without saying)
    if !matches!(sim.world().obj(p), NodeObj::Building(_) | NodeObj::Extension | NodeObj::Flag(_)) {
        out.push(build_line(sim, player, p));
    }
    out
}

/// A holy place: what it is, and who may use it (the land fully around it decides).
fn holy_lines(sim: &rr_sim::Sim, player: u8, p: MapCoord, god: u8) -> Vec<String> {
    let name = tr(match god {
        0 => "Jupiter's augur platform, a holy place",
        _ => "A holy place",
    });
    let state = match sim.world().holy_holder(p) {
        rr_world::Holder::Player(o) if o == player => tr(match god {
            0 => "Yours: Jupiter's birds show you the land far around the hill.",
            _ => "Yours: its god is with you.",
        })
        .to_string(),
        rr_world::Holder::Player(o) => {
            let rival = sim.players().get(o as usize).map_or(String::new(), |x| x.setup.name.clone());
            show(&msg("Held by {1}: its god is with them.", &[&rr_game::plain(&rival)]))
        }
        rr_world::Holder::Contested => tr("Contested: a border runs through its ring, so nobody may use it.").into(),
        rr_world::Holder::Nobody => tr("Nobody's: close your border all around it to win its god.").into(),
    };
    vec![name.to_string(), state]
}

/// The building, flag or thing on the point.
fn what_stands(sim: &rr_sim::Sim, player: u8, p: MapCoord) -> Option<String> {
    let w = sim.world();
    let whose = |owner: u8, name: &str| -> String {
        if owner == player {
            name.to_string()
        } else {
            let rival = sim.players().get(owner as usize).map_or(String::new(), |x| x.setup.name.clone());
            show(&msg("{1} of {2}", &[name, &rr_game::plain(&rival)]))
        }
    };
    Some(match w.obj(p) {
        NodeObj::Building(_) | NodeObj::Extension => {
            let at = if matches!(w.obj(p), NodeObj::Extension) { extension_of(sim, p)? } else { p };
            let bd = sim.building(sim.building_at(at)?);
            let name = whose(bd.player, crate::ui::building_name(bd.kind));
            if bd.is_ready() {
                name
            } else {
                let b = sim.building_at(at)?;
                let done = sim.site_progress_percent(b).to_string();
                format!("{name}: {}", show(&msg("building site, {1} % done", &[&done])))
            }
        }
        NodeObj::Flag(_) => {
            let f = sim.flag(sim.flag_at(p)?);
            let flag = whose(f.player, tr("Flag"));
            match f.wares.len() {
                0 => flag,
                1 => format!("{flag}: {}", tr("1 ware waits here")),
                n => format!("{flag}: {}", show(&msg("{1} wares wait here", &[&n.to_string()]))),
            }
        }
        NodeObj::Tree { stage, .. } if stage < 3 => tr("A young tree: a woodcutter fells it once it is grown").into(),
        NodeObj::Tree { .. } => tr("A tree: wood for a woodcutter").into(),
        NodeObj::Granite { .. } => tr("Rocks: stone for a quarry").into(),
        NodeObj::Field(_) => tr("A grain field").into(),
        NodeObj::Fire => tr("A burning ruin").into(),
        NodeObj::ShipSite(_) => tr("A ship being built").into(),
        o @ NodeObj::Decoration(_) if o.blocking() != rr_world::Blocking::None => {
            tr("Something that is in the way").into()
        }
        _ if let Some(r) = sim.road_at(p) => {
            let road = sim.road(r);
            let kind = match road.kind {
                rr_world::RoadKind::Donkey => "A donkey road",
                rr_world::RoadKind::Boat => "A boat road",
                _ => "A road",
            };
            let line = whose(road.player, tr(kind));
            if road.carrier.is_none() { format!("{line}: {}", tr("its carrier is on the way")) } else { line }
        }
        _ if sim.is_water_point(p) => tr("Water").into(),
        _ => return None,
    })
}

/// The building a large building's extra node belongs to (it lies E, SE or SW of it).
fn extension_of(sim: &rr_sim::Sim, p: MapCoord) -> Option<MapCoord> {
    use rr_core::Direction::*;
    let g = sim.world().grid();
    [E, SE, SW].into_iter().map(|d| g.neighbor(p, d)).find(|q| sim.building_at(*q).is_some())
}

/// What a geologist's sign on the point says, and what to build there.
fn sign_lines(sim: &rr_sim::Sim, p: MapCoord) -> Vec<String> {
    let NodeObj::Sign(code) = sim.world().obj(p) else { return Vec::new() };
    let mine = match code >> 4 {
        0 => return vec![tr("A geologist's sign: nothing found here.").into()],
        5 => return vec![tr("A geologist's sign: water. A well finds it here.").into()],
        1 => B::CoalMine,
        2 => B::IronMine,
        3 => B::GoldMine,
        _ => B::GraniteMine,
    };
    let what = tr(match mine {
        B::CoalMine => "coal",
        B::IronMine => "iron ore",
        B::GoldMine => "gold",
        _ => "granite",
    });
    let amount = tr(match code & 0x0f {
        5.. => "a lot",
        3..=4 => "some",
        _ => "a little",
    });
    vec![
        show(&msg("A geologist's sign: {1}, {2}.", &[what, amount])),
        show(&msg("The mine for it: {1}.", &[crate::ui::building_key(mine)])),
    ]
}

/// What the player could build on the point, or why nothing.
fn build_line(sim: &rr_sim::Sim, player: u8, p: MapCoord) -> String {
    let w = sim.world();
    match w.owner(p) {
        None => return tr("No one's land: push your border here to build.").into(),
        Some(o) if o != player && !sim.are_allies(o, player) => {
            let rival = sim.players().get(o as usize).map_or(String::new(), |x| x.setup.name.clone());
            return show(&msg("The land of {1}.", &[&rr_game::plain(&rival)]));
        }
        Some(o) if o != player => return tr("An ally's land.").into(),
        _ => {}
    }
    let bq = w.bq_for(p, player);
    let line = tr(match bq {
        Bq::Nothing => "Nothing can be built here.",
        Bq::Flag => "Only a flag fits here.",
        Bq::Hut => "Room for a small building (or a flag).",
        Bq::House => "Room for a medium or a small building.",
        Bq::Castle => "Room for a large building, or anything smaller.",
        Bq::Mine => "Room for a mine.",
        Bq::Harbor => "Room for a harbor, or any other building.",
    })
    .to_string();
    // military buildings keep 4 points from each other and from the castle
    if matches!(bq, Bq::Hut | Bq::House | Bq::Castle | Bq::Harbor) && sim.military_building_near(p, player) {
        return format!("{line} {}", tr("No military building: one stands too near."));
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sim() -> rr_sim::Sim {
        let mut m = rr_core::MapData::flat(48, 48, 8);
        m.hq = vec![Some(MapCoord::new(20, 20))];
        let setup = rr_sim::GameSetup { players: vec![Default::default()], ..Default::default() };
        rr_sim::Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &m, &setup).unwrap()
    }

    /// The hover tells what stands on a point, what a sign means and what fits there.
    #[test]
    fn the_hover_tells_what_is_here() {
        let mut s = sim();
        s.step(&[]);
        let hq = lines(&s, 0, MapCoord::new(20, 20));
        assert_eq!(hq[0], "Castle", "{hq:?}");
        // a free spot of our own, away from the castle: room to build
        let free = lines(&s, 0, MapCoord::new(24, 24));
        assert!(free.last().unwrap().starts_with("Room for"), "{free:?}");
        // right by the castle: no military building
        let near = lines(&s, 0, MapCoord::new(23, 20));
        assert!(
            near.last().unwrap().contains("No military building") || near.last().unwrap().starts_with("Only"),
            "{near:?}"
        );
        // outside our land
        let out = lines(&s, 0, MapCoord::new(20, 32));
        assert!(out.is_empty() || out.last().unwrap().starts_with("No one's land"), "{out:?}");
        // a road
        let from = s.world().grid().neighbor(MapCoord::new(20, 20), rr_core::Direction::SE);
        let road = rr_sim::Cmd::BuildRoad { from, dirs: vec![rr_core::Direction::E; 4] };
        s.step(&[rr_sim::PlayerCommand { player: 0, cmd: road }]);
        let mid =
            s.world().grid().neighbor(s.world().grid().neighbor(from, rr_core::Direction::E), rr_core::Direction::E);
        assert!(lines(&s, 0, mid)[0].starts_with("A road"), "{:?}", lines(&s, 0, mid));
        // a holy place: nobody's until a border closes round it
        let holy = MapCoord::new(30, 20);
        s.debug_set_obj(holy, NodeObj::Decoration(rr_world::HOLY_BASE));
        let h = lines(&s, 0, holy);
        assert!(h[0].starts_with("Jupiter's") && h.len() == 2, "{h:?}");
        assert!(h[1].starts_with("Nobody's") || h[1].starts_with("Yours"), "{h:?}");
        // a geologist's sign: coal, a lot, and the mine for it
        let at = MapCoord::new(24, 22);
        s.debug_set_obj(at, NodeObj::Sign(0x16));
        let sign = lines(&s, 0, at);
        assert!(sign.contains(&"A geologist's sign: coal, a lot.".to_string()), "{sign:?}");
        assert!(sign.iter().any(|l| l == "The mine for it: Coal mine."), "{sign:?}");
        s.debug_set_obj(at, NodeObj::Sign(0x00));
        assert!(lines(&s, 0, at).iter().any(|l| l.contains("nothing found")));
    }
}
