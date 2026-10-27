//! The bookkeeping audit (moved here from the AI fuzz test: architect round 2, #3).

use crate::Sim;

macro_rules! check {
    ($c:expr, $($m:tt)+) => {
        if !$c {
            return Err(format!($($m)+));
        }
    };
}

macro_rules! check_eq {
    ($a:expr, $b:expr, $($m:tt)+) => {
        if $a != $b {
            return Err(format!("{} ({:?} != {:?})", format!($($m)+), $a, $b));
        }
    };
}

impl Sim {
    /// Check the game's bookkeeping (architect round 2, #3): flags, roads and buildings point at
    /// live things, every "on its way" counter matches what is really on its way, carriers and
    /// roads agree, wares are where they say. The first broken rule comes back as an error.
    /// Cheap enough for every few hundred frames in tests and soaks (`rr-cli soak --audit`).
    pub fn audit(&self) -> Result<(), String> {
        let sim = self;
        let rules = sim.rules();
        // what a load checks holds at every frame too (QA round 9, #1: the audit passed while
        // saves didn't load)
        sim.validate().map_err(|e| format!("a save wouldn't load: {e}"))?;
        // the kept land count is the map's
        let mut owned = vec![0u32; sim.players.len()];
        for q in sim.world.grid().coords() {
            if let Some(o) = sim.world.owner(q) {
                owned[o as usize] += 1;
            }
        }
        check!(owned == sim.owned, "land count {:?}, the map says {owned:?}", sim.owned);
        for (id, f) in sim.flags().iter() {
            check!(f.wares.len() <= rules.economy.flag_capacity as usize, "flag {id:?} over capacity");
            for w in &f.wares {
                check!(sim.wares().get(*w).is_some(), "flag {id:?} holds a dead ware");
            }
            for r in f.roads.iter().flatten() {
                let road = sim.roads().get(*r).expect("flag points to a dead road");
                check!(road.from == id || road.to == id, "road does not end at its flag");
            }
        }
        for (id, r) in sim.roads().iter() {
            check!(sim.flags().get(r.from).is_some() && sim.flags().get(r.to).is_some(), "road {id:?} has a dead end");
            check!(r.len() >= 2 || sim.flags().get(r.from).is_some(), "road {id:?} too short");
            if let Some(c) = r.carrier {
                check!(sim.figures().get(c).is_some(), "road {id:?} carrier is dead");
            }
        }
        for (id, b) in sim.buildings().iter() {
            check!(sim.flags().get(b.flag).is_some(), "building {id:?} has no flag");
            check_eq!(sim.building_at(b.pos), Some(id), "building {id:?} not on its node");
            for s in &b.soldiers {
                check!(sim.figures().get(*s).is_some(), "building {id:?} has a dead soldier");
            }
        }
        // every "on its way" counter matches the wares actually heading there
        // (by the whole id: a dead goal must not count for the building now in its slot)
        let mut heading: std::collections::BTreeMap<(crate::BuildingId, rr_rules::Ware), u32> = Default::default();
        for (wid, w) in sim.wares().iter() {
            if let Some(g) = w.goal {
                check!(sim.buildings().get(g).is_some(), "ware {wid:?} heads to a building that is gone");
                *heading.entry((g, w.kind)).or_default() += 1;
            }
        }
        for (id, b) in sim.buildings().iter() {
            let n = |k: rr_rules::Ware| heading.get(&(id, k)).copied().unwrap_or(0);
            if b.inventory.is_some() {
                continue;
            }
            match &b.state {
                crate::BState::Site(_) => {
                    check_eq!(b.site_incoming.0 as u32, n(rr_rules::Ware::Boards), "{:?} {:?}: boards", b.kind, b.pos);
                    check_eq!(b.site_incoming.1 as u32, n(rr_rules::Ware::Stones), "{:?} {:?}: stones", b.kind, b.pos);
                }
                crate::BState::Ready => {
                    for (i, k) in rules.building(b.kind).inputs.iter().enumerate() {
                        check_eq!(b.incoming[i] as u32, n(*k), "{:?} {:?}: incoming {k:?}", b.kind, b.pos);
                    }
                }
            }
        }
        // a carrier that has arrived is its road's one carrier (or donkey), and vice versa
        for (id, f) in sim.figures().iter() {
            if let crate::FigKind::Carrier { road, .. } = f.kind
                && f.step != crate::Step::Go
                && let Some(r) = sim.roads().get(road)
            {
                let holder = if f.job == rr_rules::Job::PackDonkey { r.donkey } else { r.carrier };
                check_eq!(
                    holder,
                    Some(id),
                    "{:?} at {:?} serves road {road:?} that has another carrier: {f:?}",
                    f.job,
                    f.pos
                );
            }
        }
        for (rid, r) in sim.roads().iter() {
            for c in r.carrier.iter().chain(r.donkey.iter()) {
                check!(
                    matches!(sim.figures().get(*c).map(|f| &f.kind), Some(crate::FigKind::Carrier { road, .. }) if *road == rid),
                    "road {rid:?}: its carrier serves another road"
                );
            }
        }
        check!(sim.debug_route_cache_ok(), "a cached route table went stale");
        // exit queues hold live figures that are inside that building
        for (id, b) in sim.buildings().iter() {
            for f in &b.fig_queue {
                let fig = sim.figures().get(*f);
                check!(fig.is_some_and(|x| x.inside == Some(id)), "{:?} {:?}: stale queue entry {f:?}", b.kind, b.pos);
            }
        }
        // animals and lost wanderers never go into buildings (they would vanish from view)
        for (_, f) in sim.figures().iter() {
            if matches!(f.kind, crate::FigKind::Animal { .. } | crate::FigKind::Lost { .. }) {
                check!(f.inside.is_none(), "{f:?} is inside a building");
            }
        }
        // soldiers on the way are counted where they are going
        for (id, b) in sim.buildings().iter() {
            let coming = sim
                .figures()
                .iter()
                .filter(
                    |(_, f)| matches!(f.kind, crate::FigKind::Soldier(crate::SoldierTask::ToBuilding(t)) if t == id),
                )
                .count();
            check_eq!(b.soldiers_coming as usize, coming, "{:?} {:?}: soldiers coming", b.kind, b.pos);
        }
        for (id, w) in sim.wares().iter() {
            if let crate::WareLoc::Carried(f) = w.loc {
                check_eq!(sim.figures().get(f).and_then(|f| f.ware), Some(id), "ware {id:?} not held by its carrier");
            }
            if let crate::WareLoc::Flag(f) = w.loc {
                check!(sim.flags().get(f).is_some_and(|fl| fl.wares.contains(&id)), "ware {id:?} not on its flag");
            }
        }
        Ok(())
    }
}
