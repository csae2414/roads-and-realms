//! Life on the map that is only drawn, never played: wind in the trees, glints and fish in the
//! water, mist, birds, butterflies, worn earth by the doors, footprints in snow and sand.
//!
//! Nothing here is in the map, the save or the game: what lies still comes from the node's
//! coordinates and what the game already put there (terrain, trees, buildings), so the same map
//! always looks the same; what moves runs on the clock, like the trees always swayed. The few
//! things that need the past (footprints, the ground where soldiers fought) are kept by `Life`,
//! next to the carriers' idle times, and only read the game.

use crate::render::{TH, TW, View, neighbor_unwrapped, world_pos_unwrapped};
use eframe::egui::{self, Color32, Mesh, Pos2, Shape, Stroke, Vec2, epaint::Vertex};
use rr_core::{Direction, Landscape, MapCoord};
use rr_sim::{Motion, Sim};
use rr_world::{NodeObj, terrain};
use std::collections::{HashMap, VecDeque};
use std::f32::consts::{PI, TAU};

/// Where the wind blows to (east, a little south): gusts, leaves, mist and birds go this way.
pub const WIND: Vec2 = Vec2::new(0.944, 0.33);

/// Game frames a footprint stays, and the ground where soldiers fought.
pub const TRACKS_LAST: u32 = 900;
pub const SCARS_LAST: u32 = 3600;

/// A well-mixed hash of a lattice point.
pub fn hash(x: i32, y: i32, salt: u32) -> u32 {
    let mut v =
        (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263) ^ salt.wrapping_mul(2_246_822_519);
    v = (v ^ (v >> 13)).wrapping_mul(1_274_126_177);
    v ^ (v >> 16)
}

/// 0..1 from a hash's low bits.
pub fn unit(h: u32) -> f32 {
    (h & 0xffff) as f32 / 65535.0
}

/// Smooth value noise in -1..1: a hash per lattice corner, smoothstepped between them.
pub fn value_noise(px: f32, py: f32, salt: u32) -> f32 {
    let h = |a: i32, b: i32| unit(hash(a, b, salt)) * 2.0 - 1.0;
    let (ix, iy) = (px.floor() as i32, py.floor() as i32);
    let sm = |t: f32| t * t * (3.0 - 2.0 * t);
    let (fx, fy) = (sm(px - ix as f32), sm(py - iy as f32));
    (h(ix, iy) * (1.0 - fx) + h(ix + 1, iy) * fx) * (1.0 - fy)
        + (h(ix, iy + 1) * (1.0 - fx) + h(ix + 1, iy + 1) * fx) * fy
}

/// How hard a gust blows at world point `w` (0 calm .. 1): fronts a few nodes deep roll
/// downwind every few seconds, each of its own strength and breadth, so a forest bends in a
/// wave rather than all at once or each tree on its own.
pub fn gust(w: Vec2, time: f32) -> f32 {
    const SPEED: f32 = 120.0;
    const EVERY: f32 = 9.0;
    const LASTS: f32 = 0.35;
    let phase = (w.dot(WIND) / SPEED - time) / EVERY;
    let k = phase.floor();
    let f = phase - k;
    if f > LASTS {
        return 0.0;
    }
    let bump = (f / LASTS * PI).sin().powi(2);
    let across = Vec2::new(-WIND.y, WIND.x).dot(w);
    let band = (0.65 + 0.5 * value_noise(across / 700.0, k * 3.7, 11)).clamp(0.3, 1.0);
    bump * (0.35 + 0.65 * unit(hash(k as i32, 0, 5))) * band
}

/// A tree's top: how far it leans (screen pixels before growth and zoom) under its own slow
/// sway and the gust passing (`seed` keeps neighbours out of step).
pub fn tree_sway(time: f32, seed: u32, gust: f32) -> f32 {
    let own = (time * 1.3 + seed as f32 * 0.37).sin() * 0.6;
    own + gust * (4.0 + 0.6 * (time * 7.0 + seed as f32).sin())
}

/// How much a meadow prop bends in the wind (reeds most, bushes a little, stones not at all).
pub fn prop_bend(key: &str) -> f32 {
    match key {
        k if k.starts_with("reeds") => 1.0,
        k if k.starts_with("tuft") => 0.8,
        k if k.starts_with("flowers") => 0.6,
        k if k.contains("bush") => 0.25,
        _ => 0.0,
    }
}

/// What the map remembers for its look: footprints on soft ground, where soldiers fought, and
/// the chapter's god's favour. Read from the game each frame, never written to it.
#[derive(Default)]
pub struct Life {
    /// The chapter's god's favour (0-100; 0 outside the campaign).
    pub favour: u32,
    /// The start of the walk last seen per figure, so each step is noted once.
    walks: HashMap<rr_sim::FigId, u32>,
    /// Walks over snow or sand: (node, direction, start, duration), oldest first.
    pub tracks: VecDeque<(MapCoord, Direction, u32, u32)>,
    /// Where soldiers fought, and when last.
    pub scars: HashMap<MapCoord, u32>,
    /// The clock when the life was last listened to (`heard`).
    heard_at: std::cell::Cell<f32>,
    /// The player's "calm map" option: what lies still stays, nothing drawn-only moves.
    pub calm: bool,
    /// Birds startled from a tree by someone walking by: (tree, game frame, away from whom).
    pub flights: VecDeque<(MapCoord, u32, Direction)>,
    /// When each tree last let its birds fly (so a busy path doesn't fire it every step).
    rested: HashMap<MapCoord, u32>,
}

/// Game frames birds startled from a tree are in the air, and before that tree has birds again.
pub const FLIGHT: u32 = 40;
const ROOST: u32 = 1200;

impl Life {
    pub fn track(&mut self, sim: &Sim, favour: u32) {
        self.favour = favour;
        let gf = sim.gf().0;
        let w = sim.world();
        // (another game came, from a save or the editor: what lies off its map, or in its
        // future, is not of it)
        let g = w.grid();
        let on = |p: &MapCoord| p.x < g.width && p.y < g.height;
        self.tracks.retain(|t| on(&t.0) && t.2 <= gf);
        self.scars.retain(|p, t| on(p) && *t <= gf);
        self.flights.retain(|f| on(&f.0) && f.1 <= gf);
        self.rested.retain(|p, t| on(p) && *t <= gf);
        for (id, f) in sim.figures().iter() {
            if let Motion::Walk { dir, start, dur } = f.motion
                && f.is_visible()
                && self.walks.get(&id) != Some(&start)
            {
                self.walks.insert(id, start);
                if soft_ground(w, f.pos) {
                    self.tracks.push_back((f.pos, dir, start, dur));
                }
                self.startle(w, f.pos, start);
            }
        }
        if gf.is_multiple_of(100) {
            self.walks.retain(|id, _| sim.figures().get(*id).is_some());
        }
        while self.tracks.front().is_some_and(|t| gf.saturating_sub(t.2) > TRACKS_LAST) || self.tracks.len() > 6000 {
            self.tracks.pop_front();
        }
        for (a, d) in sim.fights() {
            for x in [a, d] {
                self.scars.insert(sim.figure(x).pos, gf);
            }
        }
        self.scars.retain(|_, t| gf.saturating_sub(*t) < SCARS_LAST);
        while self.flights.front().is_some_and(|f| gf.saturating_sub(f.1) > FLIGHT) || self.flights.len() > 200 {
            self.flights.pop_front();
        }
        if gf.is_multiple_of(500) {
            self.rested.retain(|_, t| gf.saturating_sub(*t) < ROOST);
        }
    }

    /// A walker steps past a tree: now and then its birds fly up, away from the walker (once a
    /// while per tree, chosen by the tree and the step, so a busy path doesn't fire it each step).
    fn startle(&mut self, w: &rr_world::World, at: MapCoord, start: u32) {
        for d in Direction::ALL {
            let q = w.grid().neighbor(at, d);
            if !matches!(w.obj(q), NodeObj::Tree { stage, .. } if stage >= 2) {
                continue;
            }
            let rested = self.rested.get(&q).is_none_or(|t| start.saturating_sub(*t) >= ROOST);
            if rested && hash(q.x as i32, q.y as i32, start) % 4 == 0 {
                self.rested.insert(q, start);
                self.flights.push_back((q, start, d));
            }
        }
    }
}

/// Snow or sand, where a walker leaves prints.
fn soft_ground(w: &rr_world::World, p: MapCoord) -> bool {
    let winter = w.landscape == Landscape::Winter;
    let soft = |t: &u8| matches!(t, 2 | 4 | 7) || (winter && matches!(t, 0 | 8 | 9 | 10 | 15 | 18));
    w.terrains_around(p).iter().filter(|t| soft(t)).count() >= 4
}

fn is_sand(t: u8) -> bool {
    matches!(t, 4 | 7)
}

fn is_lava(t: u8) -> bool {
    matches!(t, 16 | 17 | 20 | 21 | 22)
}

/// A soft patch: `color` in the middle, clear at the rim (radii `r`).
fn blob(m: &mut Mesh, c: Pos2, r: Vec2, color: Color32) {
    let base = m.vertices.len() as u32;
    m.vertices.push(Vertex { pos: c, uv: egui::pos2(0.0, 0.0), color });
    let steps = 12u32;
    for i in 0..steps {
        let a = i as f32 / steps as f32 * TAU;
        let pos = c + Vec2::new(a.cos() * r.x, a.sin() * r.y);
        m.vertices.push(Vertex { pos, uv: egui::pos2(0.0, 0.0), color: Color32::TRANSPARENT });
    }
    for i in 0..steps {
        m.indices.extend_from_slice(&[base, base + 1 + i, base + 1 + (i + 1) % steps]);
    }
}

fn rgba(r: u8, g: u8, b: u8, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, (a.clamp(0.0, 1.0) * 255.0) as u8)
}

/// A node seen this frame: its place, terrain and look-up helpers for the layers below.
struct Node {
    ux: i32,
    uy: i32,
    p: MapCoord,
    c: Pos2,
    ts: [u8; 6],
    wet: usize,
    h: u32,
    seen: bool,
}

/// Where the six triangles round a node lie (their middles, world pixels), in
/// `World::triangles_around`'s order.
const TRI_AT: [(f32, f32); 6] = [
    (-TW / 2.0, -TH / 3.0),
    (0.0, -2.0 * TH / 3.0),
    (TW / 2.0, -TH / 3.0),
    (TW / 2.0, TH / 3.0),
    (0.0, 2.0 * TH / 3.0),
    (-TW / 2.0, TH / 3.0),
];

impl View<'_> {
    /// The world point under a screen point.
    pub fn world_at(&self, at: Pos2) -> Vec2 {
        (at - self.rect.min) / self.cam.zoom + self.cam.offset
    }

    /// The gust at a screen point (see `gust`).
    pub fn gust_at(&self, at: Pos2) -> f32 {
        self.gust_world(self.world_at(at))
    }

    /// The gust at a world point; none on a calm map.
    pub fn gust_world(&self, w: Vec2) -> f32 {
        if self.calm() { 0.0 } else { gust(w, self.time) }
    }

    /// Is the player's "calm map" option on (nothing drawn-only moves)?
    pub fn calm(&self) -> bool {
        self.life.is_some_and(|l| l.calm)
    }

    fn wrap(&self, ux: i32, uy: i32) -> MapCoord {
        let g = self.sim.world().grid();
        MapCoord::new(ux.rem_euclid(g.width as i32) as u16, uy.rem_euclid(g.height as i32) as u16)
    }

    fn screen_of(&self, ux: i32, uy: i32) -> Pos2 {
        self.to_screen(world_pos_unwrapped(self.sim, ux, uy))
    }

    /// The nodes of the window worth a look: explored and on the screen (with a margin).
    fn nodes(&self, (x0, x1, y0, y1): (i32, i32, i32, i32)) -> Vec<Node> {
        let w = self.sim.world();
        let rect = self.rect.expand(60.0 * self.cam.zoom);
        let mut out = Vec::new();
        for uy in y0..y1 {
            for ux in x0..x1 {
                let p = self.wrap(ux, uy);
                if self.vis(p) == rr_sim::Visibility::Unexplored {
                    continue;
                }
                let c = self.screen_of(ux, uy);
                if !rect.contains(c) {
                    continue;
                }
                let ts = w.terrains_around(p);
                let wet = ts.iter().filter(|t| terrain::is_water(**t)).count();
                let h = hash(p.x as i32, p.y as i32, 0);
                out.push(Node { ux, uy, p, c, ts, wet, h, seen: self.seen(p) });
            }
        }
        out
    }

    /// Is every node within `r` steps of `p` open water (no land triangle)?
    fn open_water(&self, p: MapCoord, r: u16) -> bool {
        let w = self.sim.world();
        w.grid().within(p, r).all(|q| w.terrains_around(q).iter().all(|t| terrain::is_water(*t)))
    }

    /// Screen direction from a shore node out to the open water (none: no open water next to it).
    fn seaward(&self, n: &Node) -> Option<Vec2> {
        let w = self.sim.world();
        let mut sum = Vec2::ZERO;
        for d in Direction::ALL {
            let (x, y) = neighbor_unwrapped(n.ux, n.uy, d);
            if w.terrains_around(self.wrap(x, y)).iter().all(|t| terrain::is_water(*t)) {
                sum += self.screen_of(x, y) - n.c;
            }
        }
        (sum.length() > 1.0).then(|| sum.normalized())
    }

    fn shore_normal(&self, n: &Node) -> Vec2 {
        let w = self.sim.world();
        let sum = w.grid().neighbors(n.p).iter().fold(tri_lean(&n.ts), |a, q| a + tri_lean(&w.terrains_around(*q)));
        if sum.length() < 1.0 { Vec2::new(0.0, -1.0) } else { sum.normalized() }
    }

    /// Where a node's water lies: the middle of its water triangles (screen).
    fn water_middle(&self, n: &Node) -> Pos2 {
        let mut sum = Vec2::ZERO;
        for (k, t) in n.ts.iter().enumerate() {
            if terrain::is_water(*t) {
                sum += Vec2::new(TRI_AT[k].0, TRI_AT[k].1);
            }
        }
        n.c + sum / n.wet.max(1) as f32 * self.cam.zoom
    }
}

/// The layer on the ground, under roads and objects: worn earth, the forest floor, the ground
/// where soldiers fought, footprints, cloud shadows, and the water's life.
pub fn ground(v: &View, painter: &egui::Painter, window: (i32, i32, i32, i32)) {
    let z = v.cam.zoom;
    let mut soft = Mesh::default();
    cloud_shadows(v, &mut soft, window);
    if z < 0.45 {
        painter.add(Shape::mesh(soft));
        return;
    }
    let nodes = v.nodes(window);
    for n in &nodes {
        floor(v, &mut soft, n);
        prop_shadow(v, &mut soft, n);
        reflection(v, &mut soft, n);
    }
    scars(v, &mut soft);
    painter.add(Shape::mesh(soft));
    let mut glow = Mesh::default();
    door_glow(v, &mut glow);
    for n in &nodes {
        if !v.calm() {
            water(v, painter, &mut glow, n);
            shallows(v, painter, n);
        }
        hot_and_wet(v, painter, &mut glow, n);
    }
    painter.add(Shape::mesh(glow));
    tracks(v, painter);
}

/// A soft shadow under a rock, bush or stump of the meadow, to the lower right like the
/// buildings' and trees' (not under flowers and tufts: there it read as specks).
fn prop_shadow(v: &View, m: &mut Mesh, n: &Node) {
    let z = v.cam.zoom;
    if z < 0.55 || !v.scenery || !matches!(v.sim.world().obj(n.p), NodeObj::None) {
        return;
    }
    let Some(crate::sprites::Sprite::Prop(key, k)) = crate::render::meadow_prop(v.sim.world(), n.p) else { return };
    let solid = ["stones", "bush", "stump", "bones", "shingle"].iter().any(|s| key.contains(s));
    if !solid || key == "shingle" {
        return;
    }
    let at = n.c + crate::render::prop_jitter(n.p) * z + Vec2::new(4.0, 1.0) * k * z;
    blob(m, at, Vec2::new(15.0, 5.0) * k * z, rgba(0, 0, 0, 0.24));
}

/// A tree by the water mirrored in it: a short, soft, muted strip below its foot that trembles
/// a little (not a mirrored picture: without shaders that read as paint).
fn reflection(v: &View, m: &mut Mesh, n: &Node) {
    let NodeObj::Tree { stage, .. } = v.sim.world().obj(n.p) else { return };
    // (the water must lie below it on the screen: its lower three triangles)
    if stage < 2 || n.ts[3..].iter().filter(|t| terrain::is_water(**t)).count() < 2 {
        return;
    }
    let z = v.cam.zoom;
    let tremble = if v.calm() { 0.0 } else { (v.time * 1.3 + unit(n.h) * TAU).sin() * 1.2 };
    let at = n.c + Vec2::new(tremble, 11.0) * z;
    blob(m, at, Vec2::new(6.0, 10.0) * z, rgba(24, 48, 34, 0.3));
}

/// The firelight of a workshop at work (smithies, the bakery's oven, the brewery's kettle): a
/// bright core on its threshold that flickers like a flame, and a wide warm spill on the ground.
/// Both lie on the ground, under its flag and the figures, so they never hide them.
fn door_glow(v: &View, m: &mut Mesh) {
    use rr_rules::BuildingType as B;
    let z = v.cam.zoom;
    for (_, b) in v.sim.buildings().iter() {
        let fire = matches!(b.kind, B::IronSmelter | B::Armory | B::Mint | B::Metalworks | B::Bakery | B::Brewery);
        if !fire || b.worker.is_none() || !b.is_ready() || !v.seen(b.pos) {
            continue;
        }
        // (in front of the facade: on the node itself the house's picture covered it)
        let door = v.node_screen(b.pos) + Vec2::new(-1.0, 7.0) * z;
        if !v.rect.expand(40.0).contains(door) {
            continue;
        }
        let f = if v.calm() { 0.75 } else { flicker(v.time, unit(hash(b.pos.x as i32, b.pos.y as i32, 6)) * TAU) };
        blob(m, door + Vec2::new(0.0, 3.0) * z, Vec2::new(0.8 * TW, 0.45 * TH) * z, rgba(255, 196, 120, 0.22 * f));
        blob(m, door, Vec2::new(0.3 * TW, 0.14 * TH) * z, rgba(255, 210, 130, 0.5 * f));
        blob(m, door, Vec2::new(0.13 * TW, 0.06 * TH) * z, rgba(255, 238, 196, 0.6 * f));
    }
}

/// A flame's brightness (about 0.4..1): two quick sines out of step, not a smooth pulse.
pub fn flicker(time: f32, phase: f32) -> f32 {
    (0.7 + 0.18 * (time * 7.1 + phase).sin() + 0.12 * (time * 12.7 + phase * 2.0).sin()).clamp(0.0, 1.0)
}

/// Light playing in the shallows near a coast: a few pale curved lines drifting slowly; and
/// now and then a fish's shadow gliding under deeper water.
fn shallows(v: &View, painter: &egui::Painter, n: &Node) {
    // (the water's layers kept apart: foam on the coast's own nodes, this light only on the
    // water just off it, sparkles only out on the open water)
    if !n.seen || n.wet != 6 {
        return;
    }
    let z = v.cam.zoom;
    let t = v.time;
    let ph = unit(n.h >> 5) * TAU;
    if z >= 0.7 && n.h % 4 == 0 && !v.open_water(n.p, 1) {
        // the coast's way: out from the land, summed over the shore nodes around
        let w = v.sim.world();
        let out = w.grid().neighbors(n.p).iter().fold(Vec2::ZERO, |a, q| a + tri_lean(&w.terrains_around(*q)));
        let out = if out.length() < 1.0 { Vec2::new(0.0, -1.0) } else { out.normalized() };
        let along = Vec2::new(-out.y, out.x);
        let at = n.c + (along * (t * 0.12 + ph).sin() * 5.0 + out * 4.0) * z;
        let pts: Vec<Pos2> = (0..5)
            .map(|i| {
                let k = i as f32 - 2.0;
                // (nearly straight: bowed, they read as gulls far off)
                at + (along * k * 3.0 - out * k * k * 0.12) * z
            })
            .collect();
        // (faint: close up, brighter ones read as scratches)
        let alpha = 0.06 + 0.04 * (t * 0.6 + ph).sin();
        painter.add(Shape::line(pts, Stroke::new(0.7 * z, rgba(214, 236, 244, alpha))));
    }
    if z >= 0.6 && n.wet == 6 && n.h % 37 == 5 && v.open_water(n.p, 2) {
        let at = n.c + Vec2::new((t * 0.12 + ph).sin() * 0.6 * TW, (t * 0.07 + ph * 2.0).sin() * 0.3 * TH) * z;
        painter.add(Shape::ellipse_filled(at, Vec2::new(3.2, 1.1) * z, rgba(12, 30, 46, 0.22)));
    }
}

/// What lies still on the ground of one node: worn earth before a door, the dark floor of a
/// forest, lily pads, ice along a winter shore.
fn floor(v: &View, m: &mut Mesh, n: &Node) {
    let z = v.cam.zoom;
    let w = v.sim.world();
    match w.obj(n.p) {
        NodeObj::Tree { .. } => {
            let trees = w.grid().neighbors(n.p).iter().filter(|q| matches!(w.obj(**q), NodeObj::Tree { .. })).count();
            if trees >= 2 {
                let r = Vec2::new(0.95 * TW, 0.6 * TH) * z;
                // (on snow a cool shade, not a dark green: that read as dirt)
                let floor = if w.landscape == Landscape::Winter {
                    rgba(78, 96, 128, 0.1 + 0.03 * trees as f32)
                } else {
                    rgba(18, 34, 10, 0.25 + 0.05 * trees as f32)
                };
                blob(m, n.c + Vec2::new(0.0, 2.0 * z), r, floor);
            }
        }
        NodeObj::Building(_) => {
            let Some(b) = v.sim.building_at(n.p) else { return };
            use rr_rules::BuildingType as B;
            let big = matches!(
                v.sim.building(b).kind,
                B::Headquarters | B::Fortress | B::Farm | B::PigFarm | B::DonkeyBreeder | B::Harbor
            );
            let k = if big { 1.45 } else { 1.0 };
            let (sx, sy) = neighbor_unwrapped(n.ux, n.uy, Direction::SE);
            let flag = v.screen_of(sx, sy);
            let earth =
                if w.landscape == Landscape::Winter { rgba(120, 110, 100, 0.3) } else { rgba(112, 86, 54, 0.4) };
            blob(m, n.c + (flag - n.c) * 0.55, Vec2::new(0.6 * TW, 0.5 * TH) * k * z, earth);
            blob(m, n.c + (flag - n.c) * 0.2, Vec2::new(0.45 * TW, 0.4 * TH) * k * z, earth);
        }
        _ => {}
    }
    if n.wet == 0 || n.wet == 6 || n.h % 3 != 0 {
        return;
    }
    // (still water only, a lake's or a bay's: no open sea within three steps)
    let Some(out) = v.seaward(n) else { return };
    if !still_water(v, n.p) {
        return;
    }
    match w.landscape {
        Landscape::Greenland => lily_pads(m, n.c + out * 0.45 * TW * z, z, n.h),
        Landscape::Winter => {
            blob(m, n.c + out * 0.3 * TW * z, Vec2::new(0.5 * TW, 0.32 * TH) * z, rgba(228, 238, 248, 0.6))
        }
        Landscape::Wasteland => {}
    }
}

/// The water round a ship (`stern`, `bow`: its waterline's ends on the screen; `moving`: under
/// way): a soft shadow under the hull, foam at the bow and a broken wake that widens and fades
/// behind the stern, along the hull's own line.
pub fn ship_water(painter: &egui::Painter, (stern, bow, moving): (Pos2, Pos2, bool), z: f32, t: f32) {
    let mut m = Mesh::default();
    let length = (bow - stern).length();
    let mid = stern + (bow - stern) * 0.5;
    blob(&mut m, mid + Vec2::new(0.0, 1.0) * z, Vec2::new(0.5 * length, 0.12 * length), rgba(4, 16, 30, 0.28));
    // (a darker line where the hull meets the water, so it sits in it, not on it)
    for k in 0..7 {
        let at = stern + (bow - stern) * (k as f32 / 6.0);
        blob(&mut m, at + Vec2::new(0.0, 0.5) * z, Vec2::new(0.1 * length, 1.8 * z), rgba(2, 10, 20, 0.3));
    }
    if moving && length > 1.0 {
        // a V opening behind the stern, of many small soft patches close together (spaced out,
        // they read as bubbles), broken here and there as the water moves along
        let way = (bow - stern) / length;
        let across = Vec2::new(-way.y, way.x);
        for k in 0..18 {
            let d = k as f32 / 17.0;
            if (t * 1.2 - d * 9.0 + k as f32 * 2.3).sin() < -0.6 {
                continue;
            }
            let back = stern - way * d * 0.75 * length;
            for side in [-1.0, 1.0] {
                // (each patch a little off the line and of its own size: a regular row read as dots)
                let j = unit(hash(k, side as i32, 12)) - 0.5;
                let c = back + across * side * (1.0 + d * 0.16 * length + j * 2.0 * z);
                let r = Vec2::new(2.4 + d * 2.4 + j * 1.2, 1.2 + d * 1.0) * z;
                blob(&mut m, c, r, rgba(240, 248, 255, 0.2 * (1.0 - d) * (1.0 - d)));
            }
        }
        let pulse = 0.75 + 0.25 * (t * 3.0).sin();
        blob(&mut m, bow, Vec2::new(4.0, 1.6) * z, rgba(250, 252, 255, 0.5 * pulse));
    }
    painter.add(Shape::mesh(m));
}

/// What the drawn life makes heard since it was last listened to: a splash as a fish falls back,
/// now and then a duck; and the gust over the view's middle (for the wind's sound).
pub fn heard(v: &View) -> (Vec<(&'static str, MapCoord, f32)>, f32) {
    let gust = v.gust_at(v.rect.center());
    let Some(life) = v.life else { return (Vec::new(), gust) };
    let (from, to) = (life.heard_at.replace(v.time), v.time);
    // (nothing after a jump of the clock: a pause, a load)
    if v.cam.zoom < 0.45 || to <= from || to - from > 1.0 || v.calm() {
        return (Vec::new(), gust);
    }
    let passed = |period: f32, offset: f32, at: f32| {
        ((to + offset - at) / period).floor() > ((from + offset - at) / period).floor()
    };
    let mut out = Vec::new();
    for n in v.nodes(v.visible()).iter().filter(|n| n.seen && n.wet == 6) {
        if n.h % 29 == 1 && passed(17.0, unit(n.h >> 4) * 17.0, 0.6) && v.open_water(n.p, 1) {
            out.push(("splash", n.p, 0.3));
        }
        let ducks = v.sim.world().landscape != Landscape::Winter && n.h % 41 == 3;
        if ducks && passed(23.0, unit(n.h >> 5) * 23.0, 0.0) && !v.open_water(n.p, 3) {
            out.push(("duck", n.p, 0.3));
        }
    }
    (out, gust)
}

/// Is the water by `p` still: a pond's, a narrow bay's, a river's (no open water a few steps out)?
fn still_water(v: &View, p: MapCoord) -> bool {
    !v.sim.world().grid().within(p, 3).any(|q| v.open_water(q, 1))
}

/// Two or three round leaves on the water, one with a white flower.
fn lily_pads(m: &mut Mesh, c: Pos2, z: f32, h: u32) {
    for k in 0..(2 + h % 2) {
        let j = |s: u32| unit(hash(h as i32, k as i32, s)) - 0.5;
        let at = c + Vec2::new(j(1) * 14.0, j(2) * 6.0) * z;
        blob(m, at, Vec2::new(2.6, 1.3) * z, rgba(58, 104, 46, 0.95));
        blob(m, at, Vec2::new(3.0, 1.6) * z, rgba(40, 80, 34, 0.5));
        if k == 0 && h & 8 == 0 {
            blob(m, at + Vec2::new(0.5, -0.6) * z, Vec2::new(1.0, 0.7) * z, rgba(250, 238, 245, 1.0));
        }
    }
}

/// Slow shadows of clouds drifting over the known land (faint: player colours stay readable).
fn cloud_shadows(v: &View, m: &mut Mesh, (x0, x1, y0, y1): (i32, i32, i32, i32)) {
    let z = v.cam.zoom;
    let step = 3;
    for gy in (y0.div_euclid(step) - 1)..=(y1.div_euclid(step) + 1) {
        for gx in (x0.div_euclid(step) - 1)..=(x1.div_euclid(step) + 1) {
            let (ux, uy) = (gx * step, gy * step);
            if v.vis(v.wrap(ux, uy)) == rr_sim::Visibility::Unexplored {
                continue;
            }
            let world = Vec2::new(ux as f32 * TW, uy as f32 * TH);
            let moved = world - WIND * if v.calm() { 0.0 } else { v.time } * 14.0;
            let n = value_noise(moved.x / 650.0, moved.y / 420.0, 21);
            if n > 0.25 {
                let r = Vec2::new(3.3 * TW, 3.4 * TH) * z;
                blob(m, v.to_screen(world), r, rgba(8, 14, 30, (n - 0.25) / 0.75 * 0.16));
            }
        }
    }
}

/// The ground where soldiers fought: trampled and dark, fading over a few minutes.
fn scars(v: &View, m: &mut Mesh) {
    let Some(life) = v.life else { return };
    let z = v.cam.zoom;
    let gf = v.sim.gf().0;
    for (p, t) in &life.scars {
        if !v.seen(*p) {
            continue;
        }
        let c = v.node_screen(*p);
        if !v.rect.expand(40.0).contains(c) {
            continue;
        }
        let fresh = 1.0 - gf.saturating_sub(*t) as f32 / SCARS_LAST as f32;
        for k in 0..3 {
            let j = |s: u32| unit(hash(p.x as i32, p.y as i32 + k * 17, s)) - 0.5;
            let at = c + Vec2::new(j(1) * 0.8 * TW, j(2) * 0.6 * TH) * z;
            blob(m, at, Vec2::new(0.45 * TW, 0.32 * TH) * z, rgba(62, 46, 30, 0.42 * fresh));
        }
    }
}

/// Footprints behind walkers on snow and sand, fading.
fn tracks(v: &View, painter: &egui::Painter) {
    let Some(life) = v.life else { return };
    let z = v.cam.zoom;
    let w = v.sim.world();
    let mut m = Mesh::default();
    for (p, dir, start, dur) in &life.tracks {
        let age = v.render_gf - *start as f32;
        if !(0.0..TRACKS_LAST as f32).contains(&age) || !v.seen(*p) {
            continue;
        }
        let a = v.node_screen(*p);
        if !v.rect.expand(40.0).contains(a) {
            continue;
        }
        let b = v.node_screen_dir(*p, *dir);
        let done = (age / (*dur).max(1) as f32).clamp(0.0, 1.0);
        let fade = 1.0 - age / TRACKS_LAST as f32;
        let sand = w.terrains_around(*p).iter().filter(|t| is_sand(**t)).count() >= 3;
        let color = if sand { rgba(92, 68, 40, 0.6 * fade) } else { rgba(110, 128, 165, 0.5 * fade) };
        let side = Vec2::new(-(b - a).y, (b - a).x).normalized() * 1.6 * z;
        for i in 0..6 {
            let s = (i as f32 + 0.5) / 6.0;
            if s > done {
                break;
            }
            let at = a + (b - a) * s + side * if i % 2 == 0 { 1.0 } else { -1.0 };
            blob(&mut m, at, Vec2::new(1.9, 1.1) * z, color);
        }
    }
    painter.add(Shape::mesh(m));
}

/// The water's life: glints of sun, a fish jumping now and then, far out a whale or dolphins,
/// waves rolling in on the beaches, white water where a river falls.
fn water(v: &View, painter: &egui::Painter, glow: &mut Mesh, n: &Node) {
    if !n.seen {
        return;
    }
    let z = v.cam.zoom;
    let t = v.time;
    if n.wet == 6 {
        if n.h % 3 == 0 && v.open_water(n.p, 1) {
            let b = (t * 0.6 + unit(n.h) * TAU * 3.0).sin().max(0.0).powi(12);
            if b > 0.05 {
                let j = |s: u32| unit(hash(n.p.x as i32, n.p.y as i32, s)) - 0.5;
                let at = n.c + Vec2::new(j(3) * 0.8 * TW, j(4) * 0.8 * TH) * z;
                // (a soft flat sparkle lying on the water: a cross read as a cursor)
                blob(glow, at, Vec2::new(3.2, 0.9) * z * (0.5 + 0.5 * b), rgba(255, 250, 230, 0.55 * b));
                blob(glow, at + Vec2::new(2.5, 0.4) * z, Vec2::new(1.6, 0.5) * z, rgba(255, 250, 230, 0.35 * b));
            }
        }
        if n.h % 29 == 1 && v.open_water(n.p, 1) {
            fish(painter, n.c, z, (t + unit(n.h >> 4) * 17.0) % 17.0);
        }
        if n.h % 211 == 7 && v.open_water(n.p, 3) {
            sea_life(painter, glow, n.c, z, ((t + unit(n.h >> 3) * 45.0) % 45.0, n.h & 32 == 0));
        }
        return;
    }
    if n.wet == 0 {
        return;
    }
    match v.seaward(n) {
        Some(_) if n.h % 2 == 0 && v.sim.world().landscape != Landscape::Winter => {
            waves(glow, v.water_middle(n), v.shore_normal(n), z, t + unit(n.h) * 5.0)
        }
        Some(_) => {}
        // (no open water next to it: a river's bank)
        None => rapids(v, painter, glow, n),
    }
}

/// A fish leaping out of the water and the rings it leaves (`t`: seconds into its cycle).
fn fish(painter: &egui::Painter, c: Pos2, z: f32, t: f32) {
    let ring = |r: f32, a: f32| {
        painter.add(Shape::ellipse_stroke(c, Vec2::new(r, r * 0.45) * z, Stroke::new(0.8 * z, rgba(235, 245, 250, a))))
    };
    if t < 0.6 {
        let s = t / 0.6;
        let at = c + Vec2::new(-5.0 + s * 10.0, -(s * PI).sin() * 7.0) * z;
        painter.add(Shape::ellipse_filled(at, Vec2::new(1.8, 0.8) * z, rgba(170, 182, 190, 1.0)));
        ring(2.0 + s, 0.5 * (1.0 - s));
    } else if t < 2.4 {
        let s = (t - 0.6) / 1.8;
        ring(2.0 + s * 8.0, 0.7 * (1.0 - s));
        ring(1.0 + s * 4.0, 0.5 * (1.0 - s));
    }
}

/// Far out at sea: a whale's spout and back, or two dolphins arcing (`t`: seconds into its
/// cycle).
fn sea_life(painter: &egui::Painter, glow: &mut Mesh, c: Pos2, z: f32, (t, whale): (f32, bool)) {
    let dark = |a: f32| rgba(52, 62, 74, a);
    if whale && t < 5.0 {
        let up = (t / 5.0 * PI).sin();
        painter.add(Shape::ellipse_filled(c, Vec2::new(9.0, 2.2 * up) * z, dark(0.85 * up)));
        if t < 2.5 {
            for k in 0..5 {
                let s = t / 2.5;
                let at = c + Vec2::new(-4.0 + k as f32 * 0.4, -4.0 - s * 12.0 - k as f32 * 1.5) * z;
                blob(glow, at, Vec2::new(2.0 + s * 3.0, 1.6 + s * 2.0) * z, rgba(255, 255, 255, 0.6 * (1.0 - s)));
            }
        }
    } else if !whale {
        for (k, off) in [0.0f32, 0.6].iter().enumerate() {
            let s = (t - off) / 1.3;
            if !(0.0..1.0).contains(&s) {
                continue;
            }
            let base = c + Vec2::new(k as f32 * 8.0 - 10.0 + s * 16.0, k as f32 * 3.0) * z;
            let pts: Vec<Pos2> = (0..6)
                .map(|i| {
                    let q = (s - 0.12 + i as f32 * 0.05).clamp(0.0, 1.0);
                    base + Vec2::new((q - s) * 16.0, -(q * PI).sin() * 8.0) * z
                })
                .collect();
            painter.add(Shape::line(pts, Stroke::new(2.4 * z, dark(0.9))));
        }
    }
}

/// Out from the land into the water: its triangles' (water minus land), summed over a node and
/// its neighbours, so the line follows the coast and not each node's 60° steps.
fn tri_lean(ts: &[u8; 6]) -> Vec2 {
    let mut sum = Vec2::ZERO;
    for (k, t) in ts.iter().enumerate() {
        let side = if terrain::is_water(*t) { 1.0 } else { -1.0 };
        sum += Vec2::new(TRI_AT[k].0, TRI_AT[k].1) * side;
    }
    sum
}

/// A surge of foam rolling in towards a beach and fading before it (`water`: the middle of the
/// node's water, so it never lies on the sand; `out`: towards the open water). A soft patch,
/// not a line: the coast is too ragged for a line to lie along it.
fn waves(glow: &mut Mesh, water: Pos2, out: Vec2, z: f32, t: f32) {
    let s = (t * 0.2).fract();
    let at = water + out * 0.3 * (1.0 - s) * TW * z;
    blob(glow, at, Vec2::new(0.2 * TW, 0.11 * TH) * z, rgba(244, 249, 255, 0.24 * (s * PI).sin()));
}

/// White water where a river drops steeply from one node to the next, spray where it falls.
fn rapids(v: &View, painter: &egui::Painter, glow: &mut Mesh, n: &Node) {
    if n.wet > 4 {
        return;
    }
    let w = v.sim.world();
    let z = v.cam.zoom;
    let here = w.altitude(n.p) as i32;
    for d in Direction::ALL {
        let (x, y) = neighbor_unwrapped(n.ux, n.uy, d);
        let q = v.wrap(x, y);
        let ts = w.terrains_around(q);
        let wet = ts.iter().filter(|t| terrain::is_water(**t)).count();
        let drop = here - w.altitude(q) as i32;
        if !(1..=4).contains(&wet) || drop < 3 {
            continue;
        }
        let other = Node { ux: x, uy: y, p: q, c: v.screen_of(x, y), ts, wet, h: 0, seen: true };
        if v.seaward(&other).is_some() {
            continue;
        }
        let (a, b) = (v.water_middle(n), v.water_middle(&other));
        let side = Vec2::new(-(b - a).y, (b - a).x).normalized();
        for k in 0..3 {
            let s = (v.time * 0.9 + k as f32 / 3.0 + unit(n.h)).fract();
            let at = a + (b - a) * s + side * (k as f32 - 1.0) * 2.5 * z;
            let along = (b - a).normalized() * 3.5 * z;
            painter.line_segment([at, at + along], Stroke::new(1.2 * z, rgba(245, 250, 255, 0.75 * (s * PI).sin())));
        }
        if drop >= 6 {
            let pulse = 0.5 + 0.5 * (v.time * 2.0 + unit(n.h) * TAU).sin();
            blob(glow, b, Vec2::new(5.0, 3.0) * z, rgba(255, 255, 255, 0.35 + 0.2 * pulse));
        }
    }
}

/// Lava glowing and spitting embers; a swamp's bubbles, fireflies and low mist; mist over
/// rivers and lakes.
fn hot_and_wet(v: &View, painter: &egui::Painter, glow: &mut Mesh, n: &Node) {
    let z = v.cam.zoom;
    let t = v.time;
    let lava = n.ts.iter().filter(|t| is_lava(**t)).count();
    let swamp = n.ts.iter().filter(|t| **t == 3).count();
    // (a calm map: the glow, scorch and mist stay, still; no embers, bubbles or fireflies)
    let (t, moving) = if v.calm() { (0.0, false) } else { (t, true) };
    if (1..6).contains(&lava) && n.wet == 0 {
        // scorched, ashen ground where lava meets the land, out on the land's side (it met the
        // grass in a hard line; on the node itself the lava's own soft edge covered it)
        let mut towards = Vec2::ZERO;
        for (k, t) in n.ts.iter().enumerate() {
            towards += Vec2::new(TRI_AT[k].0, TRI_AT[k].1) * if is_lava(*t) { 1.0 } else { -1.0 };
        }
        let at = n.c - towards.normalized() * 0.4 * TW * z;
        blob(glow, at, Vec2::new(0.7 * TW, 0.45 * TH) * z, rgba(36, 28, 22, 0.55));
    }
    if n.seen && lava >= 3 {
        let pulse = 0.5 + 0.5 * (t * 1.5 + unit(n.h) * TAU).sin();
        blob(glow, n.c, Vec2::new(0.6 * TW, 0.42 * TH) * z, rgba(255, 110, 30, 0.1 + 0.1 * pulse));
        if moving && n.h % 3 == 0 && lava == 6 {
            let s = ((t + unit(n.h >> 5) * 2.5) / 2.5).fract();
            let j = unit(n.h >> 9) - 0.5;
            let at = n.c + Vec2::new(j * 10.0 + s * 3.0, -s * 16.0) * z;
            painter.circle_filled(at, 0.8 * z, rgba(255, 170, 60, 0.9 * (1.0 - s)));
        }
    }
    if moving && n.seen && swamp >= 3 {
        let s = ((t + unit(n.h) * 4.0) / 4.0).fract();
        if n.h % 2 == 0 && s < 0.25 {
            let j = |k: u32| unit(n.h >> k) - 0.5;
            let at = n.c + Vec2::new(j(3) * 0.7 * TW, j(7) * 0.5 * TH) * z;
            let r = (1.0 + s * 10.0) * z;
            painter.add(Shape::ellipse_stroke(
                at,
                Vec2::new(r, r * 0.6),
                Stroke::new(0.7 * z, rgba(205, 215, 180, 0.8 * (1.0 - s * 4.0))),
            ));
        }
        if n.h % 4 == 0 {
            let ph = unit(n.h >> 2) * TAU;
            let at = n.c + Vec2::new((t * 0.7 + ph).sin() * 0.5 * TW, -6.0 + (t * 0.9 + ph * 2.0).cos() * 0.3 * TH) * z;
            let b = (t * 3.0 + ph).sin().max(0.0).powi(3);
            blob(glow, at, Vec2::splat(2.6 * z), rgba(210, 240, 130, 0.18 * b));
            painter.circle_filled(at, 0.55 * z, rgba(235, 250, 170, 0.8 * b));
        }
    }
    // (over rivers, ponds and swamps only: over the sea it read as fog)
    let still = n.wet >= 2 && n.wet < 6 && n.h % 6 == 0 && v.seaward(n).is_none();
    let misty = still || (swamp >= 3 && n.h % 3 == 0);
    if misty && v.sim.world().landscape != Landscape::Wasteland {
        let ph = unit(n.h >> 6) * TAU;
        let drift = Vec2::new((t * 0.05 + ph).sin() * 22.0, (t * 0.03 + ph).cos() * 5.0) * z;
        let a = 0.05 + 0.03 * (t * 0.11 + ph).sin();
        blob(glow, n.c + drift, Vec2::new(1.6 * TW, 0.85 * TH) * z, rgba(240, 244, 250, a));
    }
}

/// The layer over the objects: work you can see (chips, dust, sparks), ducks, butterflies,
/// dust devils, eagles, gulls, things the gusts carry, birds crossing with their shadows,
/// snowflakes.
pub fn above(v: &View, painter: &egui::Painter) {
    let z = v.cam.zoom;
    if z < 0.45 {
        return;
    }
    if v.calm() {
        return;
    }
    let window = v.visible();
    work(v, painter);
    flights(v, painter);
    let nodes = v.nodes(window);
    for n in nodes.iter().filter(|n| n.seen) {
        creatures(v, painter, n);
        carried(v, painter, n);
    }
    harbour_gulls(v, painter);
    flocks(v, painter);
    if v.sim.world().landscape == Landscape::Winter {
        snowfall(v, painter);
    }
}

/// Birds startled from a tree flying up and away from whoever walked by.
fn flights(v: &View, painter: &egui::Painter) {
    let Some(life) = v.life else { return };
    let z = v.cam.zoom;
    for (tree, gf, away) in &life.flights {
        let age = v.render_gf - *gf as f32;
        if !(0.0..FLIGHT as f32).contains(&age) || !v.seen(*tree) {
            continue;
        }
        let top = v.node_screen(*tree) - Vec2::new(0.0, 30.0) * z;
        if !v.rect.expand(60.0).contains(top) {
            continue;
        }
        let dir = (v.node_screen_dir(*tree, *away) - v.node_screen(*tree)).normalized();
        let s = age / FLIGHT as f32;
        for k in 0..3 {
            let spread = Vec2::new(-dir.y, dir.x) * (k as f32 - 1.0) * 6.0 * s;
            let at = top + (dir * s * 70.0 + Vec2::new(0.0, -s * 34.0)) * z + spread * z;
            let flap = (age * 0.35 + k as f32 * 0.3).fract();
            let color = rgba(48, 44, 40, if s < 0.7 { 1.0 } else { (1.0 - s) / 0.3 });
            crate::animals::omen_bird(painter, at, 0.3 * z, dir.x < 0.0, flap, color);
        }
    }
}

/// Bits flying from work: wood chips at a felling, stone dust at a quarry, earth from a
/// spade, the dust of building; sparks at the door of a smithy at work.
fn work(v: &View, painter: &egui::Painter) {
    use rr_rules::Job as J;
    let z = v.cam.zoom;
    for (id, f) in v.sim.figures().iter() {
        let Motion::Work { start, .. } = f.motion else { continue };
        if !f.is_visible() || !v.seen(f.pos) {
            continue;
        }
        let at = v.node_screen(f.pos);
        if !v.rect.expand(30.0).contains(at) {
            continue;
        }
        let (color, puff, origin) = match f.job {
            J::Woodcutter => (rgba(168, 126, 70, 1.0), false, Vec2::new(7.0, -9.0)),
            J::Stonemason | J::Geologist => (rgba(176, 172, 164, 1.0), true, Vec2::new(7.0, -5.0)),
            J::Builder | J::Planer => (rgba(170, 146, 110, 1.0), true, Vec2::new(0.0, -2.0)),
            J::Forester => (rgba(110, 82, 50, 1.0), false, Vec2::new(4.0, 0.0)),
            J::Farmer => (rgba(222, 196, 104, 1.0), false, Vec2::new(5.0, -4.0)),
            _ => continue,
        };
        let s = ((v.render_gf - start as f32) / 12.0).fract();
        let o = at + origin * z;
        if puff {
            blob_now(
                painter,
                o + Vec2::new(0.0, -s * 5.0) * z,
                (2.0 + s * 5.0) * z,
                color.gamma_multiply(0.5 * (1.0 - s)),
            );
        }
        for k in 0..4 {
            let hk = hash(id.index() as i32, k, 3);
            let dir = Vec2::new(unit(hk) - 0.5, -0.4 - unit(hk >> 8) * 0.6) * 14.0;
            let p = o + (dir * s + Vec2::new(0.0, s * s * 12.0)) * z;
            painter.circle_filled(p, 0.8 * z, color.gamma_multiply(1.0 - s));
        }
    }
    use rr_rules::BuildingType as B;
    for (_, b) in v.sim.buildings().iter() {
        if !matches!(b.kind, B::IronSmelter | B::Armory | B::Mint | B::Metalworks)
            || b.worker.is_none()
            || !b.is_ready()
        {
            continue;
        }
        if !v.seen(b.pos) {
            continue;
        }
        let door = v.node_screen(b.pos) + Vec2::new(4.0, -4.0) * z;
        if !v.rect.expand(30.0).contains(door) {
            continue;
        }
        let cycle = v.time / 1.3 + unit(hash(b.pos.x as i32, b.pos.y as i32, 9));
        let (k, s) = (cycle.floor() as i32, cycle.fract());
        for i in 0..5 {
            let hk = hash(b.pos.x as i32 + k, i, 4);
            let vel = Vec2::new((unit(hk) - 0.5) * 18.0, -8.0 - unit(hk >> 8) * 10.0);
            let p = door + (vel * s + Vec2::new(0.0, s * s * 16.0)) * z;
            painter.circle_filled(p, 0.7 * z, rgba(255, 190, 80, 1.0 - s));
        }
    }
}

fn blob_now(painter: &egui::Painter, c: Pos2, r: f32, color: Color32) {
    let mut m = Mesh::default();
    blob(&mut m, c, Vec2::new(r, r * 0.8), color);
    painter.add(Shape::mesh(m));
}

/// The creatures of one node: ducks paddling on a quiet water, butterflies over flowers, a
/// dust devil on the dry land, an eagle high over the peaks.
fn creatures(v: &View, painter: &egui::Painter, n: &Node) {
    let z = v.cam.zoom;
    let t = v.time;
    let w = v.sim.world();
    let ph = unit(n.h >> 3) * TAU;
    match w.landscape {
        Landscape::Greenland | Landscape::Wasteland if n.wet == 6 && n.h % 41 == 3 && !v.open_water(n.p, 3) => {
            for k in 0..2 {
                let a = t * 0.08 + ph - k as f32 * 0.35;
                let at = n.c + Vec2::new(a.cos() * 0.5 * TW, a.sin() * 0.3 * TH) * z;
                duck(painter, at, z, a.sin() > 0.0, k == 0);
            }
        }
        Landscape::Greenland if n.wet == 0 && n.h % 2 == 0 => {
            if let Some(crate::sprites::Sprite::Prop(key, _)) = crate::render::meadow_prop(w, n.p)
                && key.starts_with("flowers")
                && matches!(w.obj(n.p), NodeObj::None)
            {
                const WINGS: [(u8, u8, u8); 4] = [(250, 250, 245), (240, 220, 80), (120, 150, 230), (232, 140, 60)];
                let (r, g, b) = WINGS[(n.h >> 7) as usize % 4];
                let at = n.c + Vec2::new((t * 0.8 + ph).sin() * 10.0, -7.0 + (t * 1.9 + ph).sin() * 4.0) * z;
                let flap = (t * 18.0 + ph).sin().abs();
                for side in [-1.0f32, 1.0] {
                    let wing = at + Vec2::new(side * (0.6 + 0.9 * flap), -0.3) * z;
                    painter.add(Shape::ellipse_filled(wing, Vec2::new(0.3 + 0.9 * flap, 1.1) * z, rgba(r, g, b, 1.0)));
                }
            }
        }
        Landscape::Wasteland if n.wet == 0 && n.h % 173 == 5 => {
            let s = ((t + ph * 3.0) % 20.0) / 6.0;
            if s < 1.0 {
                let base = n.c + WIND * s * 30.0 * z;
                for i in 0..8 {
                    let a = t * 6.0 + i as f32;
                    let at = base + Vec2::new(a.cos() * (2.0 + i as f32 * 0.7), -(i as f32) * 3.0 + a.sin() * 0.6) * z;
                    blob_now(painter, at, (1.5 + i as f32 * 0.3) * z, rgba(196, 168, 120, 0.3 * (s * PI).sin()));
                }
            }
        }
        _ => {}
    }
    if n.h % 97 == 11 && n.ts.iter().all(|t| terrain::is_mountain(*t)) && w.altitude(n.p) >= 24 {
        let a = t * 0.22 + ph;
        let ground = n.c + Vec2::new(a.cos() * 45.0, a.sin() * 16.0) * z;
        blob_now(painter, ground + Vec2::new(22.0, 30.0) * z, 3.0 * z, rgba(10, 10, 10, 0.12));
        crate::animals::omen_bird(
            painter,
            ground - Vec2::new(0.0, 60.0) * z,
            0.55 * z,
            a.sin() > 0.0,
            0.25,
            rgba(98, 72, 40, 1.0),
        );
    }
}

/// A duck on the water, seen from the side, with the small V of its wake (`drake`: green
/// head, else a brown duck).
fn duck(painter: &egui::Painter, at: Pos2, z: f32, west: bool, drake: bool) {
    let f = if west { -1.0 } else { 1.0 };
    let wake = Stroke::new(0.7 * z, rgba(240, 248, 255, 0.5));
    painter.line_segment([at + Vec2::new(-f * 2.0, 0.6) * z, at + Vec2::new(-f * 6.0, -0.6) * z], wake);
    painter.line_segment([at + Vec2::new(-f * 2.0, 0.6) * z, at + Vec2::new(-f * 6.0, 1.8) * z], wake);
    let body = if drake { rgba(196, 190, 176, 1.0) } else { rgba(150, 118, 78, 1.0) };
    let head = if drake { rgba(40, 96, 56, 1.0) } else { rgba(128, 98, 64, 1.0) };
    painter.add(Shape::ellipse_filled(at, Vec2::new(2.6, 1.2) * z, body));
    painter.circle_filled(at + Vec2::new(f * 2.0, -1.4) * z, 0.95 * z, head);
    painter.circle_filled(at + Vec2::new(f * 3.0, -1.2) * z, 0.45 * z, rgba(226, 160, 50, 1.0));
}

/// What a passing gust carries: leaves from the trees, sand over the dry land, snow.
fn carried(v: &View, painter: &egui::Painter, n: &Node) {
    if n.wet > 0 || n.h % 5 != 0 {
        return;
    }
    let w = v.sim.world();
    let leaves = w.landscape == Landscape::Greenland;
    if leaves && !matches!(w.obj(n.p), NodeObj::Tree { .. }) {
        return;
    }
    let g = v.gust_at(n.c);
    if g < 0.15 {
        return;
    }
    let z = v.cam.zoom;
    let s = (v.time * 0.5 + unit(n.h >> 2)).fract();
    let at = n.c + (WIND * s * 50.0 + Vec2::new(0.0, -(s * PI).sin() * 12.0 - if leaves { 14.0 } else { 2.0 })) * z;
    let color = match w.landscape {
        Landscape::Greenland if n.h & 64 == 0 => rgba(196, 150, 50, 1.0),
        Landscape::Greenland => rgba(90, 130, 40, 1.0),
        Landscape::Wasteland => rgba(200, 176, 130, 1.0),
        Landscape::Winter => rgba(250, 252, 255, 1.0),
    };
    let tumble = (v.time * 9.0 + unit(n.h) * TAU).sin();
    let a = g * (s * PI).sin();
    painter.add(Shape::ellipse_filled(at, Vec2::new(1.3 * tumble.abs() + 0.3, 0.7) * z, color.gamma_multiply(a)));
}

/// Gulls over the harbours, gliding in figures of eight.
fn harbour_gulls(v: &View, painter: &egui::Painter) {
    let z = v.cam.zoom;
    for (_, b) in v.sim.buildings().iter() {
        if b.kind != rr_rules::BuildingType::Harbor || !v.seen(b.pos) {
            continue;
        }
        let c = v.node_screen(b.pos);
        if !v.rect.expand(80.0).contains(c) {
            continue;
        }
        for i in 0..3 {
            let a = v.time * (0.45 + 0.08 * i as f32) + i as f32 * 2.1;
            let at = c + Vec2::new(a.sin() * 50.0, (2.0 * a).sin() * 14.0 - 46.0 - i as f32 * 6.0) * z;
            let flap = if (v.time * 0.3 + i as f32).fract() < 0.5 { (v.time * 1.6).fract() } else { 0.25 };
            crate::animals::omen_bird(painter, at, 0.45 * z, a.cos() < 0.0, flap, rgba(236, 236, 232, 1.0));
        }
    }
}

/// Now and then a flock in a V crosses the land with the wind, its shadow on the ground.
fn flocks(v: &View, painter: &egui::Painter) {
    const CELL: Vec2 = Vec2::new(1600.0, 1000.0);
    const CYCLE: f32 = 50.0;
    let z = v.cam.zoom;
    let lo = v.world_at(v.rect.min) - CELL;
    let hi = v.world_at(v.rect.max) + CELL;
    let across = Vec2::new(-WIND.y, WIND.x);
    for cy in (lo.y / CELL.y).floor() as i32..=(hi.y / CELL.y).ceil() as i32 {
        for cx in (lo.x / CELL.x).floor() as i32..=(hi.x / CELL.x).ceil() as i32 {
            let cycle = v.time / CYCLE + unit(hash(cx, cy, 31));
            let (k, s) = (cycle.floor() as i32, cycle.fract());
            if hash(cx, cy, 32 + k as u32) % 3 != 0 {
                continue;
            }
            let origin = Vec2::new(cx as f32 * CELL.x, cy as f32 * CELL.y + unit(hash(cx, cy + k, 33)) * CELL.y);
            let head = origin + WIND * (s * 2000.0 - 200.0);
            for i in 0..7 {
                let back = (i as f32 - 3.0).abs() * 9.0;
                let world = head - WIND * back + across * (i as f32 - 3.0) * 10.0;
                let ground = v.to_screen(world);
                if !v.rect.contains(ground) || !v.seen(v.wrap((world.x / TW) as i32, (world.y / TH) as i32)) {
                    continue;
                }
                blob_now(painter, ground, 2.2 * z, rgba(10, 10, 12, 0.14));
                let flap = (v.time * 2.2 + i as f32 * 0.15).fract();
                crate::animals::omen_bird(
                    painter,
                    ground - Vec2::new(30.0, 90.0) * z,
                    0.42 * z,
                    false,
                    flap,
                    rgba(64, 64, 70, 1.0),
                );
            }
        }
    }
}

/// Snow falling over a winter land, drifting with the wind.
fn snowfall(v: &View, painter: &egui::Painter) {
    const CELL: f32 = 360.0;
    let z = v.cam.zoom;
    let lo = v.world_at(v.rect.min);
    let hi = v.world_at(v.rect.max);
    for cy in (lo.y / CELL).floor() as i32 - 1..=(hi.y / CELL).ceil() as i32 {
        for cx in (lo.x / CELL).floor() as i32..=(hi.x / CELL).ceil() as i32 {
            for i in 0..5 {
                let h = hash(cx, cy, 40 + i);
                let fall = (v.time * (16.0 + unit(h >> 4) * 10.0) + unit(h) * CELL) % CELL;
                let sway = (v.time * 0.7 + unit(h >> 8) * TAU).sin() * 10.0;
                let world =
                    Vec2::new(cx as f32 * CELL + unit(h >> 12) * CELL + sway + fall * 0.35, cy as f32 * CELL + fall);
                let at = v.to_screen(world);
                if v.rect.contains(at) && v.seen(v.wrap((world.x / TW) as i32, (world.y / TH) as i32)) {
                    painter.circle_filled(at, (0.8 + unit(h >> 16) * 0.6) * z, rgba(252, 253, 255, 0.85));
                }
            }
        }
    }
}

/// Over a holy place the viewer holds, smoke from the altar; round Concordia's, flowers that
/// spread as her favour grows, and doves once she is pleased.
pub fn holy_life(v: &View, painter: &egui::Painter, top: Pos2, (god, mine): (u8, bool)) {
    let z = v.cam.zoom;
    let t = v.time;
    if mine {
        for k in 0..5 {
            let s = ((t * 0.25 + k as f32 / 5.0) % 1.0).fract();
            let at = top + (Vec2::new(0.0, -s * 34.0) + WIND * s * 14.0) * z;
            blob_now(painter, at, (2.0 + s * 6.0) * z, rgba(210, 205, 196, 0.45 * (1.0 - s)));
        }
    }
    let favour = v.life.map_or(0, |l| l.favour);
    if god == 1 && favour >= 50 {
        for i in 0..2 {
            let a = t * 0.7 + i as f32 * PI;
            let at = top + Vec2::new(a.cos() * 22.0, a.sin() * 8.0 - 14.0) * z;
            crate::animals::omen_bird(
                painter,
                at,
                0.4 * z,
                a.sin() > 0.0,
                (t * 1.8 + i as f32 * 0.5).fract(),
                rgba(248, 248, 244, 1.0),
            );
        }
    }
}

/// How many flowers bloom round Concordia's place at this favour (none at 0, eight at 100).
pub fn concordia_flowers(favour: u32) -> usize {
    (favour as usize * 8).div_ceil(100).min(8)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Gusts come and go: calm most of the time, a front now and then, the same for the same
    /// place and moment.
    #[test]
    fn gusts_come_and_go() {
        let at = Vec2::new(300.0, 200.0);
        let samples: Vec<f32> = (0..900).map(|i| gust(at, i as f32 * 0.05)).collect();
        assert!(samples.iter().all(|g| (0.0..=1.0).contains(g)));
        let calm = samples.iter().filter(|g| **g == 0.0).count();
        assert!(calm > 450, "calm most of the time: {calm} of 900");
        assert!(samples.iter().any(|g| *g > 0.2), "but a gust comes");
        assert_eq!(gust(at, 3.3), gust(at, 3.3));
    }

    /// A gust rolls downwind: what blows here blows a little further on a moment later.
    #[test]
    fn a_gust_rolls_downwind() {
        let t = (0..2000).map(|i| i as f32 * 0.01).find(|t| gust(Vec2::ZERO, *t) > 0.3).expect("a gust");
        let later = t + 1.0;
        assert!((gust(WIND * 120.0, later) - gust(Vec2::ZERO, t)).abs() < 1e-3);
    }

    /// Walkers leave prints in the snow, and only on soft ground (snow, sand).
    #[test]
    fn walkers_leave_prints_in_the_snow() {
        let a = ["--size", "48", "--seed", "3", "--landscape", "winter"].map(String::from);
        let (mut sim, _) = crate::new_game_with_replay(&crate::parse_args_from(a.into_iter())).unwrap();
        let mut life = Life::default();
        for _ in 0..1500 {
            sim.step(&[]);
            life.track(&sim, 0);
        }
        eprintln!("{} prints", life.tracks.len());
        assert!(!life.tracks.is_empty(), "nobody left a print in the snow");
        assert!(life.tracks.iter().all(|t| soft_ground(sim.world(), t.0)));
    }

    /// A tree lets its birds fly now and then, and then not again until it has rested.
    #[test]
    fn a_tree_startles_its_birds_once_a_while() {
        let a = ["--size", "48", "--seed", "3"].map(String::from);
        let (sim, _) = crate::new_game_with_replay(&crate::parse_args_from(a.into_iter())).unwrap();
        let w = sim.world();
        let g = w.grid();
        let tree = (0..g.height)
            .flat_map(|y| (0..g.width).map(move |x| MapCoord::new(x, y)))
            .find(|p| matches!(w.obj(*p), NodeObj::Tree { stage, .. } if stage >= 2))
            .expect("a grown tree");
        let walker = g.neighbor(tree, Direction::W);
        let mut life = Life::default();
        for start in 0..ROOST * 2 {
            life.startle(w, walker, start);
        }
        let times: Vec<u32> = life.flights.iter().filter(|f| f.0 == tree).map(|f| f.1).collect();
        assert!(!times.is_empty(), "the birds never flew");
        assert!(times.windows(2).all(|t| t[1] - t[0] >= ROOST), "they flew again too soon: {times:?}");
        assert!(life.flights.iter().filter(|f| f.0 == tree).all(|f| f.2 == Direction::E), "away from the walker");
    }

    /// A load to a smaller map: what the life remembered of the bigger one is let go (drawn, it
    /// crashed the game: the eval of 2026-10-29).
    #[test]
    fn a_smaller_map_drops_what_lay_off_it() {
        let a = ["--size", "48", "--seed", "3"].map(String::from);
        let (sim, _) = crate::new_game_with_replay(&crate::parse_args_from(a.into_iter())).unwrap();
        let mut life = Life::default();
        let far = MapCoord::new(200, 200);
        life.tracks.push_back((far, Direction::E, 0, 20));
        life.scars.insert(far, 0);
        life.flights.push_back((far, 0, Direction::E));
        life.track(&sim, 0);
        assert!(life.tracks.is_empty() && life.scars.is_empty() && life.flights.is_empty());
    }

    #[test]
    fn concordia_blooms_with_her_favour() {
        assert_eq!(concordia_flowers(0), 0);
        assert_eq!(concordia_flowers(1), 1);
        assert_eq!(concordia_flowers(50), 4);
        assert_eq!(concordia_flowers(100), 8);
    }
}
