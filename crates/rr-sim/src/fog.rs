//! Fog of war: unexplored, explored-but-not-seen (fog), visible. Buildings and figures see
//! around themselves; sight radii from the rules (military) or the defaults below.

use crate::{BState, PlayerId, Sim};
use rr_core::MapCoord;
use rr_rules::BuildingType;

const UPDATE_EVERY: u32 = 20;

/// Fog updates halfway between distribution passes: players' passes fall on frames -p mod 20,
/// and the fog on player 0's doubled its frame (perf-2 #4, round 1 #7).
fn fog_due(gf: u32) -> bool {
    (gf + UPDATE_EVERY / 2).is_multiple_of(UPDATE_EVERY)
}
// RTTR MilitaryConsts.h: military buildings see radius + 3 (in the rules as `sight`), soldiers 2,
// scouts 3, lookout towers 20, ships 2 (VISUALRANGE_SHIP); other buildings and figures see nothing.
const SOLDIER_SIGHT: u16 = 2;
const SHIP_SIGHT: u16 = 2;
const SCOUT_SIGHT: u16 = 3;
const LOOKOUT_SIGHT: u16 = 20;
/// The Roman campaign's Jupiter: his holder sees far from the augurs' hill (docs/CAMPAIGN-ROME.md,
/// chapter 1; ours, not RTTR's).
pub const JUPITER_SIGHT: u16 = 14;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Visibility {
    Unexplored,
    Fog,
    Visible,
}

impl Sim {
    pub fn visibility(&self, player: PlayerId, p: MapCoord) -> Visibility {
        match self.vis.get(player as usize).map(|v| v[self.world.grid().index(p)]) {
            Some(2) => Visibility::Visible,
            Some(1) => Visibility::Fog,
            _ => Visibility::Unexplored,
        }
    }

    pub(crate) fn update_fog(&mut self, force: bool) {
        if !force && !fog_due(self.gf) {
            return;
        }
        let g = self.world.grid();
        let n = g.len();
        while self.vis.len() < self.players.len() {
            self.vis.push(vec![0; n]);
        }
        let mut sources: Vec<Vec<(MapCoord, u16)>> = vec![Vec::new(); self.players.len()];
        for (_, b) in self.buildings.iter() {
            let def = self.rules.building(b.kind);
            let r = match (&b.state, &def.military) {
                (BState::Site(_), _) => continue,
                (_, _) if b.kind == BuildingType::LookoutTower && b.worker.is_some() => LOOKOUT_SIGHT,
                (_, Some(m)) if b.kind.is_warehouse() || !b.soldiers.is_empty() => m.sight,
                _ => continue,
            };
            if let Some(s) = sources.get_mut(b.player as usize) {
                s.push((b.pos, r));
            }
        }
        for (_, f) in self.figures.iter() {
            if f.inside.is_some() || f.player as usize >= sources.len() {
                continue;
            }
            let r = if f.job == rr_rules::Job::Scout {
                SCOUT_SIGHT
            } else if f.job.soldier_rank().is_some() {
                SOLDIER_SIGHT
            } else {
                continue;
            };
            sources[f.player as usize].push((f.pos, r));
        }
        // a holy place of Jupiter: its holder watches the sky from the hill
        for (at, god) in self.world.holy_places() {
            if let (0, rr_world::Holder::Player(p)) = (god, self.world.holy_holder(at))
                && let Some(src) = sources.get_mut(p as usize)
            {
                src.push((at, JUPITER_SIGHT));
            }
        }
        // ships see the coasts they pass (an archipelago's other islands come into view)
        for (_, s) in self.ships.iter() {
            if let Some(src) = sources.get_mut(s.player as usize) {
                let r = if matches!(s.task, crate::ShipTask::Explore { .. }) {
                    crate::seafaring::EXPLORE_SIGHT
                } else {
                    SHIP_SIGHT
                };
                src.push((s.pos, r));
            }
        }
        for (p, src) in sources.into_iter().enumerate() {
            let v = &mut self.vis[p];
            for x in v.iter_mut() {
                if *x == 2 {
                    *x = 1;
                }
            }
            for (at, r) in src {
                g.for_each_within(at, r, |i| v[i] = 2);
            }
        }
    }
}

#[cfg(test)]
mod due_tests {
    #[test]
    fn fog_is_not_on_a_distribution_frame() {
        let due: Vec<u32> = (0..40).filter(|g| super::fog_due(*g)).collect();
        assert_eq!(due, vec![10, 30]);
        // players 0-7 distribute on frames 0, 19, 18, ... 13
        for p in 0..8u32 {
            assert!(!(0..40).any(|g| super::fog_due(g) && (g + p).is_multiple_of(20)), "player {p}");
        }
    }
}
