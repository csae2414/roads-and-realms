//! Territory: every node belongs to the player whose territory-giving building is closest
//! (within its radius). On a tie the current owner keeps the node.

use crate::{BState, Sim};
use rr_world::NodeObj;

impl Sim {
    pub(crate) fn recompute_territory(&mut self) {
        self.territory_dirty = false;
        let g = self.world.grid();
        let n = g.len();
        let mut best: Vec<(u32, Option<u8>)> = vec![(u32::MAX, None); n];
        for (_, b) in self.buildings.iter() {
            let Some(radius) = self.territory_radius(b) else {
                continue;
            };
            for q in g.within(b.pos, radius) {
                let d = g.distance(b.pos, q);
                let i = g.index(q);
                let cur_owner = self.world.owner(q);
                let (bd, bo) = best[i];
                if d < bd || (d == bd && Some(b.player) == cur_owner && bo != cur_owner) {
                    best[i] = (d, Some(b.player));
                }
            }
        }
        let mut changed_at = Vec::new();
        // land that changed hands between players, for the news: (from, to) -> (nodes, a node)
        let mut taken: std::collections::BTreeMap<(u8, u8), (u32, rr_core::MapCoord)> = Default::default();
        self.owned.resize(self.players.len(), 0);
        for (i, (_, owner)) in best.into_iter().enumerate() {
            let p = g.coord(i);
            let old = self.world.owner(p);
            if old != owner {
                changed_at.push(p);
                self.world.set_owner(p, owner);
                if let Some(o) = old {
                    self.owned[o as usize] -= 1;
                    if let Some(n) = owner {
                        taken.entry((o, n)).or_insert((0, p)).0 += 1;
                    }
                }
                if let Some(o) = owner {
                    self.owned[o as usize] += 1;
                }
            }
        }
        if !changed_at.is_empty() {
            self.territory_epoch = crate::next_epoch();
        }
        for ((player, to), (nodes, at)) in taken {
            self.events.push(crate::Event::LandLost { player, to, nodes, at });
        }
        self.destroy_player_rests(&changed_at);
        for b in self.buildings.ids() {
            let bd = &self.buildings[b];
            if Some(bd.player) != self.world.owner(bd.pos) {
                self.destroy_building(b, true);
            }
        }
    }

    /// What a border change leaves on the wrong side (RTTR GameWorld::RecalcTerritory and
    /// DestroyPlayerRests): on the changed nodes and their neighbours, a flag, building or site
    /// on land its player no longer owns, or on its player's border (a neighbour owned by someone
    /// else or by no one), goes; then the roads running through such nodes. Buildings holding
    /// land of their own stay (owner's playtest 2026-10-27: roads and flags stayed on the border
    /// line).
    fn destroy_player_rests(&mut self, changed_at: &[rr_core::MapCoord]) {
        let g = self.world.grid();
        let mut handle: Vec<rr_core::MapCoord> = changed_at
            .iter()
            .flat_map(|p| std::iter::once(*p).chain(rr_core::Direction::ALL.into_iter().map(|d| g.neighbor(*p, d))))
            .collect();
        handle.sort_by_key(|p| g.index(*p));
        handle.dedup();
        let inside = |s: &Self, p: rr_core::MapCoord, player: u8| {
            s.world.owner(p) == Some(player)
                && rr_core::Direction::ALL.into_iter().all(|d| s.world.owner(g.neighbor(p, d)) == Some(player))
        };
        for &p in &handle {
            if let Some(f) = self.flag_at(p) {
                let fl = &self.flags[f];
                let holds = fl.building.is_some_and(|b| self.territory_radius(&self.buildings[b]).is_some());
                if !holds && !inside(self, p, fl.player) {
                    self.destroy_flag(f);
                }
            } else if let Some(b) = self.building_at(p) {
                let bd = &self.buildings[b];
                if self.territory_radius(bd).is_none() && !inside(self, p, bd.player) {
                    self.destroy_building(b, true);
                }
            }
        }
        for &p in &handle {
            if matches!(self.world.obj(p), NodeObj::Flag(_) | NodeObj::Building(_)) {
                continue;
            }
            if let Some(r) = self.road_at(p)
                && !inside(self, p, self.roads[r].player)
            {
                self.destroy_road(r);
            }
        }
    }

    /// Radius of land a building claims, if it claims any now.
    pub(crate) fn territory_radius(&self, b: &crate::Building) -> Option<u16> {
        let def = self.rules.building(b.kind);
        let m = def.military.as_ref()?;
        match &b.state {
            BState::Site(s) if s.from_sea => Some(m.radius),
            BState::Site(_) => None,
            _ if b.kind.is_warehouse() => Some(m.radius),
            _ if !b.occupied => None,
            _ => Some(m.radius),
        }
    }
}
