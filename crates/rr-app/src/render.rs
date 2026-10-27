//! Map renderer: terrain triangles with slope shading, roads, borders, objects and figures,
//! sorted back to front like the original's painter algorithm.

use crate::sprites::{self, Sprite};
use crate::tex::TerrainTextures;
use eframe::egui::{self, Color32, Mesh, Pos2, Rect, Shape, Stroke, Vec2, epaint::Vertex};
use rr_core::{Direction, MapCoord};
use rr_sim::{FigKind, Motion, Sim};
use rr_world::{Bq, NodeObj, RoadKind};
use std::collections::HashMap;

pub const TW: f32 = 56.0;
pub const TH: f32 = 28.0;
pub const HS: f32 = 5.0;

/// The first six (as many as the menu offers) stay apart for red-green and blue-yellow colour
/// blindness too (CIELAB distance at least 28 in every simulation, see the tests).
/// Blue, red, cyan, orange, magenta and light yellow for the six menu players: apart in normal
/// sight and with any colour blindness (item 304: two pinks and two reds before), then sea green
/// and lilac. All eight also stand out from grass and water in every vision (a11y round 2, #2:
/// the red sank into grass for deutan players, the purple into water; searched for the nearest
/// palette that passes the tests below).
pub const PLAYER_COLORS: [Color32; 8] = [
    Color32::from_rgb(37, 96, 230),
    Color32::from_rgb(182, 0, 17),
    Color32::from_rgb(98, 227, 221),
    Color32::from_rgb(231, 137, 30),
    Color32::from_rgb(213, 20, 143),
    Color32::from_rgb(255, 250, 120),
    Color32::from_rgb(69, 182, 100),
    Color32::from_rgb(211, 165, 239),
];

/// A border ribbon's screen widths (under-stroke, ribbon) at zoom `z`: never thinner than a
/// road when zoomed out (art director round 3, #1: 0.9 px at zoom 0.3).
pub fn ribbon_widths(z: f32) -> (f32, f32) {
    let w = if z < 0.8 { 3.0 } else { 2.4 };
    (((w + 1.2) * z).max(3.4), (w * z).max(2.2))
}

pub fn player_color(p: u8) -> Color32 {
    PLAYER_COLORS[p as usize % PLAYER_COLORS.len()]
}

/// A player's colour for a name in text on the dark panels: lifted until it reads (new-player
/// round 12, #6: the dark red and blue names were unreadable on the end table).
pub fn player_text_color(p: u8) -> Color32 {
    let c = player_color(p);
    let light = |v: u8| v as f32 * 0.299;
    let luma = light(c.r()) + c.g() as f32 * 0.587 + c.b() as f32 * 0.114;
    if luma >= 150.0 {
        return c;
    }
    // mixed with white until the light reaches 150
    let t = (150.0 - luma) / (255.0 - luma);
    let mix = |v: u8| (v as f32 + (255.0 - v as f32) * t).round() as u8;
    Color32::from_rgb(mix(c.r()), mix(c.g()), mix(c.b()))
}

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    /// World pixel at the top-left of the view.
    pub offset: Vec2,
    pub zoom: f32,
}

impl Camera {
    pub fn center_on(&mut self, sim: &Sim, p: MapCoord, view: Vec2) {
        self.show_at(sim, p, view / 2.0);
    }

    /// Zoom by `factor` (within 0.35–3) keeping the world point under view point `at` in place.
    /// The UI scale went from `old` to `new`: the map keeps its size on the screen and what it
    /// shows (the view's points grow with the scale; the offset stays).
    pub fn ui_rescaled(&mut self, old: f32, new: f32) {
        if old > 0.0 && new > 0.0 {
            self.zoom *= old / new;
        }
    }

    pub fn zoom_at(&mut self, factor: f32, at: Vec2) {
        let before = self.offset + at / self.zoom;
        self.zoom = (self.zoom * factor).clamp(0.35, 3.0);
        self.offset = before - at / self.zoom;
    }

    /// Scroll so that node `p` shows at screen point `at`.
    pub fn show_at(&mut self, sim: &Sim, p: MapCoord, at: Vec2) {
        let w = world_pos_unwrapped(sim, p.x as i32, p.y as i32);
        self.offset = w - at / self.zoom;
    }
}

/// World pixel position of an unwrapped node coordinate.
pub fn world_pos_unwrapped(sim: &Sim, ux: i32, uy: i32) -> Vec2 {
    let g = sim.world().grid();
    let x = ux.rem_euclid(g.width as i32) as u16;
    let y = uy.rem_euclid(g.height as i32) as u16;
    let alt = sim.world().altitude(MapCoord::new(x, y)) as f32;
    Vec2::new(ux as f32 * TW + if uy & 1 == 1 { TW / 2.0 } else { 0.0 }, uy as f32 * TH - alt * HS)
}

fn neighbor_unwrapped(ux: i32, uy: i32, d: Direction) -> (i32, i32) {
    let odd = uy & 1 == 1;
    match d {
        Direction::W => (ux - 1, uy),
        Direction::E => (ux + 1, uy),
        Direction::NW => (if odd { ux } else { ux - 1 }, uy - 1),
        Direction::NE => (if odd { ux + 1 } else { ux }, uy - 1),
        Direction::SW => (if odd { ux } else { ux - 1 }, uy + 1),
        Direction::SE => (if odd { ux + 1 } else { ux }, uy + 1),
    }
}

/// The three triangles next to triangle (`ux`, `uy`, `rsu`): an RSU borders the USDs of its
/// node and of its W and SW neighbours, a USD the RSUs of its node and of its NE and E ones.
fn tri_neighbors(ux: i32, uy: i32, rsu: bool) -> [(i32, i32, bool); 3] {
    if rsu {
        let (w, sw) = (neighbor_unwrapped(ux, uy, Direction::W), neighbor_unwrapped(ux, uy, Direction::SW));
        [(w.0, w.1, false), (sw.0, sw.1, false), (ux, uy, false)]
    } else {
        let (ne, e) = (neighbor_unwrapped(ux, uy, Direction::NE), neighbor_unwrapped(ux, uy, Direction::E));
        [(ne.0, ne.1, true), (e.0, e.1, true), (ux, uy, true)]
    }
}

/// Is a water triangle part of a river: land on two sides or more, and every water neighbour
/// touching land too (a notch in a coast opens onto open sea and is no river; art director
/// round 3, #2: notches were drawn as mud strips floating in the sea), and a run of at least
/// two such triangles (a lone notch in a stepped coast touches the coast's own triangles, and
/// was drawn as a crisp blue tooth with no shore; graphics round 1).
fn is_river_tri(terr: &impl Fn(i32, i32, bool) -> u8, ux: i32, uy: i32, rsu: bool) -> bool {
    use rr_world::terrain::is_water;
    let land =
        |x: i32, y: i32, r: bool| tri_neighbors(x, y, r).iter().filter(|n| !is_water(terr(n.0, n.1, n.2))).count();
    let narrow = |x: i32, y: i32, r: bool| {
        is_water(terr(x, y, r))
            && land(x, y, r) >= 2
            && tri_neighbors(x, y, r).iter().all(|n| !is_water(terr(n.0, n.1, n.2)) || land(n.0, n.1, n.2) >= 1)
    };
    narrow(ux, uy, rsu) && tri_neighbors(ux, uy, rsu).iter().any(|n| narrow(n.0, n.1, n.2))
}

/// An order not yet carried out: a building site, a flag or a road.
#[derive(Clone, Debug)]
pub enum Planned {
    Site(MapCoord),
    Flag(MapCoord),
    Road(MapCoord, Vec<Direction>),
}

/// What of `cmds` can be shown on the map before it happens.
pub fn planned_marks(cmds: &[rr_sim::Cmd]) -> Vec<Planned> {
    cmds.iter()
        .filter_map(|c| match c {
            rr_sim::Cmd::PlaceBuilding { at, .. } => Some(Planned::Site(*at)),
            rr_sim::Cmd::PlaceFlag { at } => Some(Planned::Flag(*at)),
            rr_sim::Cmd::BuildRoad { from, dirs } => Some(Planned::Road(*from, dirs.clone())),
            _ => None,
        })
        .collect()
}

/// A map label's plate: the windows' wood (art director round 4, #2: bare 11 px text on grass,
/// the red warning at 1.38:1).
const PLATE: Color32 = Color32::from_rgba_premultiplied(54, 40, 25, 215);
const PLATE_TEXT: Color32 = Color32::from_rgb(245, 230, 190);

/// A label on a plate at `pos`, with a chip of `chip` colour before it.
fn label_on_plate(
    painter: &egui::Painter,
    pos: Pos2,
    txt: &str,
    font: egui::FontId,
    col: Color32,
    chip: Option<Color32>,
) {
    let galley = painter.layout_no_wrap(txt.to_string(), font, col);
    let chip_w = if chip.is_some() { 9.0 } else { 0.0 };
    let size = galley.size() + Vec2::new(8.0 + chip_w, 2.0);
    let rect = Rect::from_center_size(pos, size);
    painter.rect(rect, 3.0, PLATE, Stroke::new(1.0_f32, Color32::from_rgb(150, 115, 60)), egui::StrokeKind::Inside);
    if let Some(c) = chip {
        painter.circle_filled(Pos2::new(rect.left() + 7.0, rect.center().y), 3.0, c);
    }
    painter.galley(Pos2::new(rect.left() + 4.0 + chip_w, rect.top() + 1.0), galley, col);
}

/// The overlay's label for a productivity: a colour, and for the low band a mark too, so the
/// red ones are found without telling red from green (a11y round 2, #11).
pub fn prod_label(p: u32) -> (String, Color32) {
    // (light blue for working well: green and amber were 12 apart for protan eyes; a11y round
    // 3, #11)
    if p >= 70 {
        (crate::i18n::pct(p), Color32::from_rgb(130, 205, 255))
    } else if p >= 35 {
        (crate::i18n::pct(p), Color32::from_rgb(240, 210, 80))
    } else {
        (format!("! {}", crate::i18n::pct(p)), Color32::from_rgb(255, 135, 115))
    }
}

/// A flag's waiting wares for the productivity overlay: from five, amber; full, red and marked
/// (genre lessons, PLAN §27, #4: where the transport jams, seen at a glance).
pub fn flag_label(wares: usize, capacity: usize) -> Option<(String, Color32)> {
    if wares >= capacity {
        Some((format!("! {wares}"), Color32::from_rgb(255, 135, 115)))
    } else if wares >= 5 {
        Some((wares.to_string(), Color32::from_rgb(240, 210, 80)))
    } else {
        None
    }
}

/// The least pull of a flag or road on a click, in screen pixels (a11y round 2, #8).
const SNAP_PX: f32 = 14.0;

pub struct View<'a> {
    pub sim: &'a Sim,
    pub cam: Camera,
    pub rect: Rect,
    pub render_gf: f32,
    pub player: u8,
    pub show_bq: bool,
    /// Productivity / construction progress over buildings (the original's info overlay).
    pub show_prod: bool,
    /// Building names above the buildings (RTTR's key C).
    pub show_names: bool,
    pub selected: Option<MapCoord>,
    pub road_preview: Option<(MapCoord, Vec<Direction>)>,
    /// Orders given in a pause, shown as planned until the game runs (a11y round 2, #4).
    pub planned: Vec<Planned>,
    /// The work area of the building being chosen: its node and radius.
    pub work_area: Option<(MapCoord, u16)>,
    pub time: f32,
    /// Fog of war: land out of sight is dimmed and its figures hidden. Off: the original's
    /// classic exploration, where explored land stays in view.
    pub fog_of_war: bool,
    /// Since when each carrier has stood idle (`app::track_idle`), for its idle motions; none:
    /// no idle motions (the editor's and the tests' views).
    pub idle: Option<&'a std::collections::HashMap<rr_sim::FigId, u32>>,
    /// Show the whole map (the map editor).
    pub reveal_all: bool,
    /// Draw the props that aren't in the map (bushes, stumps, boulders): off in the editor, so
    /// the map's own objects can be told apart (map maker round 1, #9).
    pub scenery: bool,
    /// This frame's fog per node in the drawn window (x0, y0, width, values), so the soft edge
    /// doesn't look each node up eight times.
    pub fog_cache: std::cell::RefCell<Option<(i32, i32, i32, Vec<f32>)>>,
}

/// A catapult stone in flight, thrown at game frame `gf`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    pub from: MapCoord,
    pub to: MapCoord,
    pub hit: bool,
    pub gf: u32,
}

/// Game frames a stone flies, and the dust after it lands.
const SHOT_FLIGHT: f32 = 36.0;
const SHOT_DUST: f32 = 14.0;

impl Shot {
    pub fn done(&self, gf: u32) -> bool {
        gf as f32 > self.gf as f32 + SHOT_FLIGHT + SHOT_DUST
    }

    /// Where the stone is at `t` (0 thrown, 1 landed) from `a` to `b` in screen space, and its
    /// shadow on the ground: a high arc, the throwing arm's height at the start.
    pub fn arc(a: Pos2, b: Pos2, t: f32, z: f32) -> (Pos2, Pos2) {
        let t = t.clamp(0.0, 1.0);
        let ground = a + (b - a) * t;
        let rise = 0.35 * (b - a).length() + 40.0 * z;
        let arm = 18.0 * z * (1.0 - t);
        (ground - Vec2::new(0.0, 4.0 * rise * t * (1.0 - t) + arm), ground)
    }
}

impl View<'_> {
    /// Free harbor spots an expedition of ours could reach: an anchor on a blue disc.
    pub fn draw_harbor_spots(&self, painter: &egui::Painter, spots: &[MapCoord]) {
        let z = self.cam.zoom;
        for &p in spots {
            let at = self.node_screen(p) - Vec2::new(0.0, 6.0 * z);
            if !self.rect.expand(20.0).contains(at) {
                continue;
            }
            let r = 10.0 * z.clamp(0.7, 1.5);
            painter.circle(
                at,
                r,
                Color32::from_rgba_unmultiplied(40, 90, 170, 200),
                Stroke::new(1.5_f32, Color32::WHITE),
            );
            painter.text(at, egui::Align2::CENTER_CENTER, "⚓", egui::FontId::proportional(r * 1.3), Color32::WHITE);
        }
    }

    /// The reach of our catapults at `at` (their windows are open): a ring of red dots at the
    /// farthest it throws, and a red ring round each building it may hit now.
    pub fn draw_catapult_reach(&self, painter: &egui::Painter, at: &[MapCoord]) {
        let z = self.cam.zoom;
        let red = Color32::from_rgb(235, 80, 60);
        for &p in at {
            let Some(b) = self.sim.building_at(p) else { continue };
            let c = self.sim.building(b);
            if c.kind != rr_rules::BuildingType::Catapult || c.player != self.player {
                continue;
            }
            for q in self.sim.world().grid().ring(p, rr_sim::CATAPULT_RANGE) {
                painter.circle(self.node_screen(q), 2.8 * z, red, Stroke::new(1.0_f32, Color32::from_black_alpha(140)));
            }
            for t in self.sim.catapult_targets(b) {
                let s = self.node_screen(self.sim.building(t).pos) - Vec2::new(0.0, 14.0 * z);
                painter.circle_stroke(s, 22.0 * z, Stroke::new(4.0_f32, Color32::from_black_alpha(120)));
                painter.circle_stroke(s, 22.0 * z, Stroke::new(2.5_f32, red));
            }
        }
    }

    /// A gold star above each of our buildings with the player's priority (`Cmd::SetPriority`).
    pub fn draw_priorities(&self, painter: &egui::Painter) {
        let z = self.cam.zoom;
        for (_, b) in self.sim.buildings().iter() {
            if b.player != self.player || !b.priority() || !self.seen(b.pos) {
                continue;
            }
            let at = self.node_screen(b.pos) - Vec2::new(0.0, 52.0 * z);
            if !self.rect.expand(20.0).contains(at) {
                continue;
            }
            let font = egui::FontId::proportional(18.0 * z.clamp(0.7, 1.6));
            for d in [Vec2::new(1.0, 1.0), Vec2::new(-1.0, 1.0), Vec2::new(1.0, -1.0), Vec2::new(-1.0, -1.0)] {
                painter.text(at + d, egui::Align2::CENTER_CENTER, "★", font.clone(), Color32::from_black_alpha(170));
            }
            painter.text(at, egui::Align2::CENTER_CENTER, "★", font, Color32::from_rgb(250, 205, 120));
        }
    }

    /// Catapult stones in flight: a dark stone with a lit side over its shadow, a puff of dust
    /// where it lands (a miss lands a little short, beside the building).
    pub fn draw_shots(&self, painter: &egui::Painter, shots: &[Shot]) {
        let z = self.cam.zoom;
        let g = self.sim.world().grid();
        for s in shots {
            let a = self.node_screen(s.from);
            // the target unwrapped next to the catapult, not next to the view's center
            let d = |v: i32, size: i32| (v + size / 2).rem_euclid(size) - size / 2;
            let (ux, uy) = self.nearest_unwrapped(s.from);
            let (dx, dy) = (
                d(s.to.x as i32 - s.from.x as i32, g.width as i32),
                d(s.to.y as i32 - s.from.y as i32, g.height as i32),
            );
            // (an odd row step shifts the half column: take the node's own unwrapped position)
            let mut b = self.to_screen(world_pos_unwrapped(self.sim, ux + dx, uy + dy)) - Vec2::new(0.0, 14.0 * z);
            if !s.hit {
                let side = if s.gf % 2 == 0 { 1.0 } else { -1.0 };
                b += Vec2::new(side * TW * 0.8 * z, TH * 0.5 * z);
            }
            let t = (self.render_gf - s.gf as f32) / SHOT_FLIGHT;
            if t < 0.0 {
                continue;
            }
            if t < 1.0 {
                let (at, shadow) = Shot::arc(a, b, t, z);
                painter.add(Shape::ellipse_filled(shadow, Vec2::new(6.5 * z, 2.8 * z), Color32::from_black_alpha(90)));
                painter.circle_filled(at, 6.0 * z, Color32::from_rgb(40, 36, 34));
                painter.circle_filled(at, 5.0 * z, Color32::from_rgb(112, 106, 98));
                painter.circle_filled(at + Vec2::new(-1.6, -1.6) * z, 2.2 * z, Color32::from_rgb(190, 182, 168));
            } else {
                let k = ((t - 1.0) * SHOT_FLIGHT / SHOT_DUST).min(1.0);
                let alpha = ((1.0 - k) * 200.0) as u8;
                for (i, off) in [(-7.0, 0.0), (7.0, -1.0), (0.0, -6.0)].iter().enumerate() {
                    let r = (6.0 + 14.0 * k + i as f32) * z;
                    let c = Color32::from_rgba_unmultiplied(170, 150, 120, alpha);
                    painter.circle_filled(b + Vec2::new(off.0, off.1) * z * (1.0 + k), r, c);
                }
            }
        }
    }

    pub fn to_screen(&self, w: Vec2) -> Pos2 {
        self.rect.min + (w - self.cam.offset) * self.cam.zoom
    }

    /// Unwrapped coordinates of a node closest to the view center.
    pub fn nearest_unwrapped(&self, p: MapCoord) -> (i32, i32) {
        let g = self.sim.world().grid();
        let center = self.cam.offset + self.rect.size() / (2.0 * self.cam.zoom);
        let (w, h) = (g.width as i32, g.height as i32);
        let cx = (center.x / TW).floor() as i32;
        let cy = (center.y / TH).floor() as i32;
        let ux = p.x as i32 + ((cx - p.x as i32) as f32 / w as f32).round() as i32 * w;
        let uy = p.y as i32 + ((cy - p.y as i32) as f32 / h as f32).round() as i32 * h;
        (ux, uy)
    }

    pub fn node_screen(&self, p: MapCoord) -> Pos2 {
        let (ux, uy) = self.nearest_unwrapped(p);
        self.to_screen(world_pos_unwrapped(self.sim, ux, uy))
    }

    fn node_screen_dir(&self, p: MapCoord, d: Direction) -> Pos2 {
        let (ux, uy) = self.nearest_unwrapped(p);
        let (nx, ny) = neighbor_unwrapped(ux, uy, d);
        self.to_screen(world_pos_unwrapped(self.sim, nx, ny))
    }

    /// Visible unwrapped node range (rows extended for altitude).
    fn visible(&self) -> (i32, i32, i32, i32) {
        let size = self.rect.size() / self.cam.zoom;
        let x0 = (self.cam.offset.x / TW).floor() as i32 - 2;
        let x1 = ((self.cam.offset.x + size.x) / TW).ceil() as i32 + 2;
        let y0 = (self.cam.offset.y / TH).floor() as i32 - 2;
        let y1 = ((self.cam.offset.y + size.y) / TH).ceil() as i32 + 14;
        (x0, x1, y0, y1)
    }

    /// The node under a screen point.
    pub fn pick(&self, pos: Pos2) -> MapCoord {
        let g = self.sim.world().grid();
        let w = self.cam.offset + (pos - self.rect.min) / self.cam.zoom;
        let gy = (w.y / TH).round() as i32;
        let mut best = (f32::MAX, MapCoord::new(0, 0));
        for uy in gy - 2..=gy + 14 {
            let gx = ((w.x - if uy & 1 == 1 { TW / 2.0 } else { 0.0 }) / TW).round() as i32;
            for ux in gx - 1..=gx + 1 {
                let s = world_pos_unwrapped(self.sim, ux, uy);
                let d = (s - w).length_sq();
                if d < best.0 {
                    let p = MapCoord::new(ux.rem_euclid(g.width as i32) as u16, uy.rem_euclid(g.height as i32) as u16);
                    best = (d, p);
                }
            }
        }
        best.1
    }

    /// The node a click means: the building whose picture is under the pointer (buildings stand
    /// up from their node, so most of the picture lies over other nodes; the lowest on screen is
    /// in front), else the nearest node.
    pub fn pick_click(&self, pos: Pos2) -> MapCoord {
        let near = self.pick(pos);
        // a flag or a road right under the pointer is what the click means, even in front of
        // a building's picture (new-player round 7, #3); the pull is at least 14 px however far
        // out the view is zoomed (a11y round 2, #8: 5.9 px at zoom 0.35), and a flag pulls from
        // the next nodes too
        let snap = (0.3 * TW * self.cam.zoom).max(SNAP_PX);
        let world = self.sim.world();
        let g = world.grid();
        let flag = std::iter::once(near)
            .chain(Direction::ALL.iter().map(|d| g.neighbor(near, *d)))
            .filter(|p| matches!(world.obj(*p), NodeObj::Flag(_)))
            .map(|p| ((self.node_screen(p) - pos).length(), p))
            .filter(|(d, _)| *d <= snap)
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, p)) = flag {
            return p;
        }
        let close = (self.node_screen(near) - pos).length() <= snap;
        if close && world.is_on_road(near) {
            return near;
        }
        let w = self.cam.offset + (pos - self.rect.min) / self.cam.zoom;
        let g = self.sim.world().grid();
        let (nx, ny) = self.nearest_unwrapped(near);
        let mut best: Option<(f32, MapCoord)> = None;
        for uy in ny - 1..=ny + 5 {
            for ux in nx - 2..=nx + 2 {
                let p = MapCoord::new(ux.rem_euclid(g.width as i32) as u16, uy.rem_euclid(g.height as i32) as u16);
                let Some(b) = self.sim.building_at(p) else { continue };
                if self.sim.building(b).pos != p || !self.seen(p) {
                    continue;
                }
                let kind = self.sim.building(b).kind;
                // the picture's box in world pixels (about what draw_building_image covers)
                let (hw, h) = match self.sim.rules().building(kind).size {
                    rr_rules::Size::Hut | rr_rules::Size::Mine => (TW * 0.4, TW * 1.0),
                    rr_rules::Size::House => (TW * 0.65, TW * 1.3),
                    _ => (TW * 0.8, TW * 1.6),
                };
                let k = if kind == rr_rules::BuildingType::Headquarters { 1.3 } else { 1.0 };
                let s = world_pos_unwrapped(self.sim, ux, uy);
                let inside = (w.x - s.x).abs() <= hw * k && w.y <= s.y + TH * 0.3 && w.y >= s.y - h * k;
                if inside && best.is_none_or(|(y, _)| s.y > y) {
                    best = Some((s.y, p));
                }
            }
        }
        best.map_or(near, |b| b.1)
    }

    /// Fields as patches of land use around their node, under roads and objects (art director
    /// round 2, #8); the ears on top come with the objects.
    fn draw_fields(&self, painter: &egui::Painter, tex: &TerrainTextures, (x0, x1, y0, y1): (i32, i32, i32, i32)) {
        let g = self.sim.world().grid();
        let mut meshes: Vec<Mesh> = tex.fields.iter().map(|t| Mesh::with_texture(t.id())).collect();
        // in winter a layer of frost over the furrows: not summer brown on the snow
        let winter = self.sim.world().landscape == rr_core::Landscape::Winter;
        let mut frost = Mesh::default();
        for uy in y0..y1 {
            for ux in x0..x1 {
                let p = MapCoord::new(ux.rem_euclid(g.width as i32) as u16, uy.rem_euclid(g.height as i32) as u16);
                let NodeObj::Field(stage) = self.sim.world().obj(p) else { continue };
                if !self.seen(p) {
                    continue;
                }
                let Some(m) = meshes.get_mut((stage as usize).min(4)) else { continue };
                let c = self.to_screen(world_pos_unwrapped(self.sim, ux, uy));
                let world = Vec2::new(ux as f32 * TW, uy as f32 * TH);
                let poly = field_patch(c, self.cam.zoom, world);
                let light = self.shade(ux, uy);
                let base = m.vertices.len() as u32;
                // the texture is pinned to the world, so neighbouring patches line up
                let uv = |q: Pos2| (((q - self.rect.min) / self.cam.zoom + self.cam.offset) / 64.0).to_pos2();
                m.vertices.push(Vertex { pos: c, uv: uv(c), color: light });
                for q in &poly {
                    m.vertices.push(Vertex { pos: *q, uv: uv(*q), color: Color32::TRANSPARENT });
                }
                // an opaque middle out to a firm edge (the soft rim from 72 % was an airbrush blur)
                let inner: Vec<Pos2> = poly.iter().map(|q| c + (*q - c) * 0.9).collect();
                for q in &inner {
                    m.vertices.push(Vertex { pos: *q, uv: uv(*q), color: light });
                }
                let n = poly.len() as u32;
                for k in 0..n {
                    let (o0, o1) = (base + 1 + k, base + 1 + (k + 1) % n);
                    let (i0, i1) = (base + 1 + n + k, base + 1 + n + (k + 1) % n);
                    m.indices.extend_from_slice(&[base, i0, i1, i0, o0, o1, i0, o1, i1]);
                }
                if winter {
                    // a cold blue-white over the furrows, fading out on the field's own rim (it
                    // was even to the very edge, and the field fades there: a white outline
                    // round every field; graphics round 2)
                    let fb = frost.vertices.len() as u32;
                    let rime = Color32::from_rgba_premultiplied(58, 62, 68, 68);
                    frost.colored_vertex(c, rime);
                    for q in &poly {
                        frost.colored_vertex(*q, Color32::TRANSPARENT);
                    }
                    for q in &inner {
                        frost.colored_vertex(*q, rime);
                    }
                    for k in 0..n {
                        let (o0, o1) = (fb + 1 + k, fb + 1 + (k + 1) % n);
                        let (i0, i1) = (fb + 1 + n + k, fb + 1 + n + (k + 1) % n);
                        frost.indices.extend_from_slice(&[fb, i0, i1, i0, o0, o1, i0, o1, i1]);
                    }
                }
            }
        }
        for m in meshes {
            painter.add(Shape::mesh(m));
        }
        if winter {
            painter.add(Shape::mesh(frost));
        }
    }

    /// Dark drifting clouds over the land nobody has seen (it stays hidden: they cover it).
    fn draw_clouds(&self, painter: &egui::Painter, tex: &TerrainTextures, (x0, x1, y0, y1): (i32, i32, i32, i32)) {
        if self.reveal_all {
            return;
        }
        let mut m = Mesh::with_texture(tex.clouds.id());
        let drift = Vec2::new(self.time * 0.01, self.time * 0.004);
        for uy in y0..y1 {
            for ux in x0..x1 {
                let corners = [
                    (ux, uy),
                    neighbor_unwrapped(ux, uy, Direction::SW),
                    neighbor_unwrapped(ux, uy, Direction::SE),
                    neighbor_unwrapped(ux, uy, Direction::E),
                ];
                let seen = corners.map(|(x, y)| self.fog_node(x, y) > 0.0);
                if seen.iter().all(|s| *s) {
                    continue;
                }
                let base = m.vertices.len() as u32;
                for (x, y) in corners {
                    // fully covering where unexplored, thinning out over two triangles
                    let cover = (1.0 - self.fog_at(x, y) / 0.35).clamp(0.0, 1.0);
                    let a = (cover * 255.0) as u8;
                    let flat = Vec2::new(x as f32 * TW + if y & 1 == 1 { TW / 2.0 } else { 0.0 }, y as f32 * TH);
                    let at = world_pos_unwrapped(self.sim, x, y);
                    m.vertices.push(Vertex {
                        pos: self.to_screen(at),
                        uv: (flat / (TW * 6.0) + drift).to_pos2(),
                        color: Color32::from_rgba_premultiplied(a, a, a, a),
                    });
                }
                m.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            }
        }
        painter.add(Shape::mesh(m));
    }

    /// Soldiers in a fight: how far each stands off its node (world pixels) and whether it
    /// faces west. On one node the attacker stands west of it and the defender east, each
    /// turned to the other (they were drawn on the very same spot); apart, each faces the other.
    fn fighters(&self) -> HashMap<rr_sim::FigId, (f32, bool)> {
        let mut out = HashMap::new();
        for (a, d) in self.sim.fights() {
            let (pa, pd) = (self.sim.figure(a).pos, self.sim.figure(d).pos);
            let (sa, sd) = fight_stance(pa == pd, self.node_screen(pd).x < self.node_screen(pa).x);
            out.insert(a, sa);
            out.insert(d, sd);
        }
        out
    }

    fn shade(&self, ux: i32, uy: i32) -> Color32 {
        let g = self.sim.world().grid();
        let at = |x: i32, y: i32| {
            self.sim
                .world()
                .altitude(MapCoord::new(x.rem_euclid(g.width as i32) as u16, y.rem_euclid(g.height as i32) as u16))
                as f32
        };
        // the slope from all six neighbours, lit from the north-west: gentle, no facets
        let h = |d: Direction| {
            let (x, y) = neighbor_unwrapped(ux, uy, d);
            at(x, y)
        };
        let dx = h(Direction::E) - h(Direction::W);
        let dy = (h(Direction::SE) + h(Direction::SW)) - (h(Direction::NE) + h(Direction::NW));
        // mountains get their relief (the Alps must read as mountains); meadows stay calm
        let g2 = self.sim.world().grid();
        let node = MapCoord::new(ux.rem_euclid(g2.width as i32) as u16, uy.rem_euclid(g2.height as i32) as u16);
        let rocky =
            self.sim.world().terrains_around(node).iter().any(|t| rr_world::terrain::is_mountain(*t) || *t == 2);
        let b = if rocky {
            // relief with a lit side, not only a dark one (art director round 2, #4); strong
            // enough that ranges read as ranges (graphics round 2: they were a flat sheet)
            (1.0 + (dx + dy * 0.5) * 0.2).clamp(0.6, 1.2)
        } else {
            (1.0 + (dx + dy * 0.5) * 0.08).clamp(0.72, 1.15)
        };
        if self.sim.world().landscape == rr_core::Landscape::Winter {
            // snow is lit brighter, and its shade is blue, not grey (art director round 2, #7)
            let v = (b / 1.2 * 255.0 * 1.12 * self.fog_at(ux, uy)).min(255.0);
            let k = if b < 1.0 { 1.0 - b } else { 0.0 };
            let c = |t: f32| (v * (1.0 - k * t)) as u8;
            return Color32::from_rgb(c(0.75), c(0.55), c(0.15));
        }
        let v = (b / 1.2 * 255.0 * self.fog_at(ux, uy)) as u8;
        Color32::from_rgb(v, v, v)
    }

    /// How much of the land's light fog lets through (0 unexplored, 1 in sight), averaged with
    /// the six neighbours so the edge of the known world is soft, not a hexagon.
    fn fog_at(&self, ux: i32, uy: i32) -> f32 {
        let mut sum = self.fog_node(ux, uy) * 2.0;
        for d in Direction::ALL {
            let (x, y) = neighbor_unwrapped(ux, uy, d);
            sum += self.fog_node(x, y);
        }
        let mean = sum / 8.0;
        // a node of seen land itself never goes fully dark
        if self.fog_node(ux, uy) > 0.0 { mean.max(0.35) } else { mean * 0.6 }
    }

    fn fog_node(&self, ux: i32, uy: i32) -> f32 {
        if let Some((x0, y0, w, vals)) = self.fog_cache.borrow().as_ref() {
            let (dx, dy) = (ux - x0, uy - y0);
            if dx >= 0 && dx < *w && dy >= 0 && (dy * w + dx) < vals.len() as i32 {
                return vals[(dy * w + dx) as usize];
            }
        }
        self.fog_node_uncached(ux, uy)
    }

    fn fog_node_uncached(&self, ux: i32, uy: i32) -> f32 {
        let g = self.sim.world().grid();
        let p = MapCoord::new(ux.rem_euclid(g.width as i32) as u16, uy.rem_euclid(g.height as i32) as u16);
        match self.vis(p) {
            rr_sim::Visibility::Visible => 1.0,
            rr_sim::Visibility::Fog if self.fog_of_war => 0.55,
            rr_sim::Visibility::Fog => 1.0,
            rr_sim::Visibility::Unexplored => 0.0,
        }
    }

    fn vis(&self, p: MapCoord) -> rr_sim::Visibility {
        if self.reveal_all {
            return rr_sim::Visibility::Visible;
        }
        self.sim.visibility(self.player, p)
    }

    /// Is what happens at `p` shown to the player?
    fn seen(&self, p: MapCoord) -> bool {
        match self.vis(p) {
            rr_sim::Visibility::Visible => true,
            rr_sim::Visibility::Fog => !self.fog_of_war,
            rr_sim::Visibility::Unexplored => false,
        }
    }

    /// Soft transitions: along each edge between two terrains, the one with the higher
    /// priority reaches a little into the other triangle and fades out.
    #[allow(clippy::too_many_lines)] // to split (architect round 2, #8: no new long ones)
    fn draw_edges(&self, painter: &egui::Painter, tex: &TerrainTextures, shades: &Shades) {
        let (x0, x1, y0, y1) = shades.window;
        let g = self.sim.world().grid();
        let world = self.sim.world();
        let wrap = |ux: i32, uy: i32| {
            MapCoord::new(ux.rem_euclid(g.width as i32) as u16, uy.rem_euclid(g.height as i32) as u16)
        };
        let terr = |ux: i32, uy: i32, rsu: bool| {
            world.terrain(wrap(ux, uy), if rsu { rr_world::Tri::Rsu } else { rr_world::Tri::Usd })
        };
        let vert = |ux: i32, uy: i32, d: Option<Direction>| {
            let (cx, cy) = match d {
                None => (ux, uy),
                Some(d) => neighbor_unwrapped(ux, uy, d),
            };
            let flat = Vec2::new(cx as f32 * TW + if cy & 1 == 1 { TW / 2.0 } else { 0.0 }, cy as f32 * TH);
            (world_pos_unwrapped(self.sim, cx, cy), shades.get(cx, cy).unwrap_or_else(|| self.shade(cx, cy)), flat)
        };
        let mut meshes: HashMap<u8, Mesh> = HashMap::new();
        // surf along the coasts: a soft round patch at every shore point (no spikes along the
        // triangle grid), and a strip of sand on the land side
        let mut foam = Mesh::default();
        let mut shore_points: std::collections::HashSet<(i32, i32)> = std::collections::HashSet::new();
        let mut sand = Mesh::with_texture(tex.get(4).id());
        let mut ribbon = Mesh::with_texture(tex.get(5).id());
        let mut banks = Mesh::default();
        let (mut river_pts, mut river_flat): (Vec<Vec2>, Vec<Vec2>) = (Vec::new(), Vec::new());
        for uy in y0..y1 {
            for ux in x0..x1 {
                // (own terrain, corners, [(edge corner a, edge corner b, opposite corner, neighbor terrain)])
                let rsu = terr(ux, uy, true);
                let usd = terr(ux, uy, false);
                let (wx, wy) = neighbor_unwrapped(ux, uy, Direction::W);
                let (swx, swy) = neighbor_unwrapped(ux, uy, Direction::SW);
                let (nex, ney) = neighbor_unwrapped(ux, uy, Direction::NE);
                let (ex, ey) = neighbor_unwrapped(ux, uy, Direction::E);
                let (w_usd, sw_usd, ne_rsu, e_rsu) =
                    (terr(wx, wy, false), terr(swx, swy, false), terr(nex, ney, true), terr(ex, ey, true));
                // one terrain all around: no edge, no shore, no river here, and nothing to work out
                // (perf round 3, #6: four shades per node of the window, 4.9 ms zoomed out)
                if [usd, w_usd, sw_usd, ne_rsu, e_rsu].iter().all(|t| *t == rsu) {
                    continue;
                }
                let p = vert(ux, uy, None);
                let sw = vert(ux, uy, Some(Direction::SW));
                let se = vert(ux, uy, Some(Direction::SE));
                let e = vert(ux, uy, Some(Direction::E));
                let tris = [
                    (rsu, [(p, sw, se, w_usd), (sw, se, p, sw_usd), (se, p, sw, usd)]),
                    (usd, [(p, e, se, ne_rsu), (e, se, p, e_rsu), (se, p, e, rsu)]),
                ];
                // the light of water here: fog only, as in the terrain pass
                let water_light = self.fog_at(ux, uy);
                for (k, (own, edges)) in tris.into_iter().enumerate() {
                    // a river: a water triangle with land on two or three sides stays crisp and
                    // has no surf (else the banks cover it)
                    // (the first is the node's RSU, the second its USD)
                    let river = is_river_tri(&terr, ux, uy, k == 0);
                    if river {
                        // collected here, drawn below as one smooth line per river
                        let (a, b, c) = (edges[0].0, edges[0].1, edges[0].2);
                        river_pts.push((a.0 + b.0 + c.0) / 3.0);
                        river_flat.push((a.2 + b.2 + c.2) / 3.0);
                    }
                    for (a, b, c, other) in edges {
                        if rr_world::terrain::is_water(own) && !rr_world::terrain::is_water(other) && !river {
                            for v in [&a, &b] {
                                let key = (v.2.x.round() as i32, v.2.y.round() as i32);
                                if shore_points.insert(key) {
                                    let w = (self.time * 1.4 + (v.2.x + v.2.y) * 0.031).sin() * 0.5 + 0.5;
                                    // faint, a little out in the water, so the patches merge into
                                    // a band off the coast (art director round 3, #2: centred on
                                    // the coast they read as fog puffs on the grass)
                                    let alpha = ((12.0 + 16.0 * w) * (v.1.r() as f32 / 222.0).min(1.0)) as u8;
                                    let at = self.to_screen(v.0) + (self.to_screen(c.0) - self.to_screen(v.0)) * 0.25;
                                    soft_disc(&mut foam, at, 0.34 * TW * self.cam.zoom, alpha);
                                }
                            }
                        }
                        // (same priority: the higher id fades over, so meadow variants blend too)
                        if (edge_priority(other), other) <= (edge_priority(own), own) {
                            continue;
                        }
                        let mesh = meshes.entry(other).or_insert_with(|| Mesh::with_texture(tex.get(other).id()));
                        let base = mesh.vertices.len() as u32;
                        // land over water reaches further (round coasts); land over land stays crisp
                        let k = if river {
                            0.12
                        } else if rr_world::terrain::is_water(own) {
                            0.5
                        } else {
                            0.32
                        };
                        // snow lies in tongues over the rock
                        let k = if other == 2 && !rr_world::terrain::is_water(own) { 0.6 } else { k };
                        let col = |s: Color32, alpha: u8| {
                            let f = alpha as f32 / 255.0;
                            Color32::from_rgba_premultiplied(
                                (s.r() as f32 * f) as u8,
                                (s.g() as f32 * f) as u8,
                                (s.b() as f32 * f) as u8,
                                alpha,
                            )
                        };
                        let water = rr_world::terrain::is_water(other);
                        // water reaching over the shore is shallow water
                        let sc = |s: Color32| if water { shallow(water_light) } else { s };
                        let uv = |f: Vec2| (f / (TW * 2.3)).to_pos2();
                        // ragged and steep (art director round 2, #2): the edge in six pieces, each
                        // reaching a depth from world-space noise, solid to 70 % of it, then gone
                        const SEG: u32 = 6;
                        // land reaching over water ends in a strip of sand on the new coastline
                        let beach = rr_world::terrain::is_water(own) && !water;
                        let sand_base = sand.vertices.len() as u32;
                        for i in 0..=SEG {
                            let t = i as f32 / SEG as f32;
                            let e = (a.0 + (b.0 - a.0) * t, a.2 + (b.2 - a.2) * t);
                            let light = lerp_color(a.1, b.1, t);
                            let ki = if i == 0 || i == SEG { k } else { edge_depth(e.1, k) };
                            if beach {
                                for (depth, alpha) in [(ki * 0.4, 0u8), (ki * 0.68, 160), (ki * 0.9, 0)] {
                                    let (pw, pf) = (e.0 + (c.0 - e.0) * depth, e.1 + (c.2 - e.1) * depth);
                                    sand.vertices.push(Vertex {
                                        pos: self.to_screen(pw),
                                        uv: uv(pf),
                                        color: lit(light, alpha),
                                    });
                                }
                            }
                            for (depth, alpha) in [(0.0, 255u8), (ki * 0.7, 255), (ki, 0)] {
                                let (pw, pf) = (e.0 + (c.0 - e.0) * depth, e.1 + (c.2 - e.1) * depth);
                                mesh.vertices.push(Vertex {
                                    pos: self.to_screen(pw),
                                    uv: uv(pf),
                                    color: col(sc(light), alpha),
                                });
                            }
                        }
                        for i in 0..SEG {
                            let (r0, r1) = (base + i * 3, base + (i + 1) * 3);
                            for j in 0..2 {
                                mesh.indices.extend_from_slice(&[
                                    r0 + j,
                                    r1 + j,
                                    r1 + j + 1,
                                    r0 + j,
                                    r1 + j + 1,
                                    r0 + j + 1,
                                ]);
                            }
                            if beach {
                                let (r0, r1) = (sand_base + i * 3, sand_base + (i + 1) * 3);
                                for j in 0..2 {
                                    sand.indices.extend_from_slice(&[
                                        r0 + j,
                                        r1 + j,
                                        r1 + j + 1,
                                        r0 + j,
                                        r1 + j + 1,
                                        r0 + j + 1,
                                    ]);
                                }
                            }
                        }
                    }
                }
            }
        }
        self.corner_overlays(tex, &mut meshes, &mut sand, (x0, x1, y0, y1), shades);
        let mut keys: Vec<u8> = meshes.keys().copied().collect();
        keys.sort();
        for k in keys {
            painter.add(Shape::mesh(meshes.remove(&k).unwrap()));
        }
        self.river_strips(&mut banks, &mut ribbon, &river_pts, &river_flat, (x0, x1, y0, y1));
        painter.add(Shape::mesh(sand));
        painter.add(Shape::mesh(banks));
        painter.add(Shape::mesh(ribbon));
        painter.add(Shape::mesh(foam));
    }

    /// Rivers: each chain of river triangles as one smoothed strip, muddy banks under the water
    /// (art director round 2, #3).
    fn river_strips(
        &self,
        banks: &mut Mesh,
        ribbon: &mut Mesh,
        river_pts: &[Vec2],
        river_flat: &[Vec2],
        (x0, x1, y0, y1): (i32, i32, i32, i32),
    ) {
        let fog = self.fog_at((x0 + x1) / 2, (y0 + y1) / 2);
        let water = shallow(fog);
        for chain in river_chains(river_flat, TW * 1.05) {
            if chain.len() < 2 {
                continue;
            }
            let line = chaikin(&chaikin(&chain.iter().map(|i| river_pts[*i]).collect::<Vec<_>>()));
            let flat = chaikin(&chaikin(&chain.iter().map(|i| river_flat[*i]).collect::<Vec<_>>()));
            let n = line.len();
            let mut along = 0.0;
            let (bank_base, water_base) = (banks.vertices.len() as u32, ribbon.vertices.len() as u32);
            for k in 0..n {
                let prev = line[k.saturating_sub(1)];
                let next = line[(k + 1).min(n - 1)];
                let dir = (next - prev).normalized();
                let side = Vec2::new(-dir.y, dir.x * 0.6);
                // tapered ends
                let end = (k.min(n - 1 - k) as f32 / 3.0).min(1.0);
                let half = TW * (0.15 + 0.17 * end);
                if k > 0 {
                    along += (flat[k] - flat[k - 1]).length();
                }
                let mud = Color32::from_rgba_premultiplied(48, 43, 31, 160);
                for s in [-1.4f32, 1.4] {
                    banks.vertices.push(Vertex {
                        pos: self.to_screen(line[k] + side * half * s),
                        uv: egui::pos2(0.0, 0.0),
                        color: mud,
                    });
                }
                for s in [-1.0f32, 1.0] {
                    let uv = egui::pos2((along - self.time * 6.0) / (TW * 2.3), (0.5 + s * 0.5) * 0.3);
                    ribbon.vertices.push(Vertex { pos: self.to_screen(line[k] + side * half * s), uv, color: water });
                }
            }
            for k in 0..n as u32 - 1 {
                for (mesh, b) in [(&mut *banks, bank_base), (&mut *ribbon, water_base)] {
                    let (i0, i1) = (b + k * 2, b + k * 2 + 2);
                    mesh.indices.extend_from_slice(&[i0, i0 + 1, i1 + 1, i0, i1 + 1, i1]);
                }
            }
        }
    }

    /// Land reaching over a sea triangle that touches it at one corner only: a ragged fan from
    /// that corner, as deep as the land's edge overlays beside it, with their strip of sand (no
    /// edge of such a triangle borders land, so nothing covered its tip and every stepped coast
    /// had blue teeth; graphics round 1).
    fn corner_overlays(
        &self,
        tex: &TerrainTextures,
        meshes: &mut HashMap<u8, Mesh>,
        sand: &mut Mesh,
        (x0, x1, y0, y1): (i32, i32, i32, i32),
        shades: &Shades,
    ) {
        use rr_world::terrain::is_water;
        let world = self.sim.world();
        let g = world.grid();
        let wrap = |ux: i32, uy: i32| {
            MapCoord::new(ux.rem_euclid(g.width as i32) as u16, uy.rem_euclid(g.height as i32) as u16)
        };
        let terr = |ux: i32, uy: i32, rsu: bool| {
            world.terrain(wrap(ux, uy), if rsu { rr_world::Tri::Rsu } else { rr_world::Tri::Usd })
        };
        let flat_of =
            |cx: i32, cy: i32| Vec2::new(cx as f32 * TW + if cy & 1 == 1 { TW / 2.0 } else { 0.0 }, cy as f32 * TH);
        let light_of = |cx: i32, cy: i32| shades.get(cx, cy).unwrap_or_else(|| self.shade(cx, cy));
        // the six triangles round a node, in turn (as `World::triangles_around`): the corners
        // they reach, and which triangle each is
        use Direction::*;
        const SPAN: [(Direction, Direction); 6] = [(W, NW), (NW, NE), (NE, E), (E, SE), (SE, SW), (SW, W)];
        let uv = |f: Vec2| (f / (TW * 2.3)).to_pos2();
        const K: f32 = 0.5;
        // (zoomed far out a fan is a few pixels: fewer pieces, perf)
        let seg: u32 = if self.cam.zoom < 0.6 { 2 } else { 4 };
        // which triangles are water, for the window and a row and column round it (two lookups
        // a node, not six: the pass looks at every node)
        let (wx0, wy0, ww) = (x0 - 1, y0 - 1, x1 - x0 + 2);
        let wet: Vec<[bool; 2]> = (0..ww * (y1 - y0 + 1).max(0))
            .map(|i| {
                let (x, y) = (wx0 + i % ww, wy0 + i / ww);
                [is_water(terr(x, y, true)), is_water(terr(x, y, false))]
            })
            .collect();
        let wet_at = |(x, y): (i32, i32), rsu: bool| wet[((y - wy0) * ww + x - wx0) as usize][usize::from(!rsu)];
        for uy in y0..y1 {
            for ux in x0..x1 {
                let (nw, ne, w) =
                    (neighbor_unwrapped(ux, uy, NW), neighbor_unwrapped(ux, uy, NE), neighbor_unwrapped(ux, uy, W));
                let ids = [(nw, true), (nw, false), (ne, true), ((ux, uy), false), ((ux, uy), true), (w, false)];
                let wets = ids.map(|(q, r)| wet_at(q, r));
                if wets.iter().all(|w| *w) || wets.iter().all(|w| !*w) {
                    continue;
                }
                let around = world.terrains_around(wrap(ux, uy));
                for i in 0..6 {
                    let Some(land) = corner_land(&around, i) else {
                        continue;
                    };
                    let ((tx, ty), rsu) = ids[i];
                    if is_river_tri(&terr, tx, ty, rsu) {
                        continue;
                    }
                    let (v, vf, vl) = (world_pos_unwrapped(self.sim, ux, uy), flat_of(ux, uy), light_of(ux, uy));
                    let corner = |d: Direction| {
                        let (cx, cy) = neighbor_unwrapped(ux, uy, d);
                        (world_pos_unwrapped(self.sim, cx, cy), flat_of(cx, cy), light_of(cx, cy))
                    };
                    let (a, b) = (corner(SPAN[i].0), corner(SPAN[i].1));
                    let mesh = meshes.entry(land).or_insert_with(|| Mesh::with_texture(tex.get(land).id()));
                    let (base, sand_base) = (mesh.vertices.len() as u32, sand.vertices.len() as u32);
                    for j in 0..=seg {
                        let t = j as f32 / seg as f32;
                        let (dw, df) = ((a.0 - v) * (1.0 - t) + (b.0 - v) * t, (a.1 - vf) * (1.0 - t) + (b.1 - vf) * t);
                        let k = if j == 0 || j == seg { K } else { edge_depth(vf + df * K, K) };
                        let light = lerp_color(vl, lerp_color(a.2, b.2, t), k);
                        for (depth, alpha) in [(0.0, 255u8), (k * 0.7, 255), (k, 0)] {
                            mesh.vertices.push(Vertex {
                                pos: self.to_screen(v + dw * depth),
                                uv: uv(vf + df * depth),
                                color: lit(light, alpha),
                            });
                        }
                        for (depth, alpha) in [(k * 0.4, 0u8), (k * 0.68, 160), (k * 0.9, 0)] {
                            sand.vertices.push(Vertex {
                                pos: self.to_screen(v + dw * depth),
                                uv: uv(vf + df * depth),
                                color: lit(light, alpha),
                            });
                        }
                    }
                    for j in 0..seg {
                        for (m, b0) in [(&mut *mesh, base), (&mut *sand, sand_base)] {
                            let (r0, r1) = (b0 + j * 3, b0 + (j + 1) * 3);
                            for q in 0..2 {
                                m.indices.extend_from_slice(&[
                                    r0 + q,
                                    r1 + q,
                                    r1 + q + 1,
                                    r0 + q,
                                    r1 + q + 1,
                                    r0 + q + 1,
                                ]);
                            }
                        }
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_lines)] // to split (architect round 2, #8: no new long ones)
    pub fn draw(&self, painter: &egui::Painter, tex: &TerrainTextures) {
        let (x0, x1, y0, y1) = self.visible();
        {
            // fog for the window plus a margin of two nodes
            let (cx0, cy0, cw, ch) = (x0 - 2, y0 - 2, x1 - x0 + 4, y1 - y0 + 4);
            let mut vals = Vec::with_capacity((cw * ch).max(0) as usize);
            for y in cy0..cy0 + ch {
                for x in cx0..cx0 + cw {
                    vals.push(self.fog_node_uncached(x, y));
                }
            }
            *self.fog_cache.borrow_mut() = Some((cx0, cy0, cw, vals));
        }
        let g = self.sim.world().grid();
        let world = self.sim.world();
        let wrap = |ux: i32, uy: i32| {
            MapCoord::new(ux.rem_euclid(g.width as i32) as u16, uy.rem_euclid(g.height as i32) as u16)
        };

        // ---- terrain
        // how deep the water is at each corner in the window: steps to land, up to 4 (one breadth
        // first pass per frame, art director round 2, #1)
        const DEEP: u8 = 4;
        let (dx0, dy0) = (x0 - DEEP as i32 - 1, y0 - DEEP as i32 - 1);
        let (dw, dh) = (x1 - x0 + 2 * DEEP as i32 + 3, y1 - y0 + 2 * DEEP as i32 + 3);
        let mut depth = vec![DEEP; (dw * dh).max(0) as usize];
        let mut queue = std::collections::VecDeque::new();
        for y in 0..dh {
            for x in 0..dw {
                if !world.terrains_around(wrap(dx0 + x, dy0 + y)).iter().all(|t| rr_world::terrain::is_water(*t)) {
                    depth[(y * dw + x) as usize] = 0;
                    queue.push_back((x, y));
                }
            }
        }
        while let Some((x, y)) = queue.pop_front() {
            let d = depth[(y * dw + x) as usize];
            if d + 1 >= DEEP {
                continue;
            }
            for dir in Direction::ALL {
                let (nx, ny) = neighbor_unwrapped(dx0 + x, dy0 + y, dir);
                let (nx, ny) = (nx - dx0, ny - dy0);
                if nx < 0 || ny < 0 || nx >= dw || ny >= dh {
                    continue;
                }
                let i = (ny * dw + nx) as usize;
                if depth[i] > d + 1 {
                    depth[i] = d + 1;
                    queue.push_back((nx, ny));
                }
            }
        }
        let depth_at = |cx: i32, cy: i32| {
            let (x, y) = (cx - dx0, cy - dy0);
            if x < 0 || y < 0 || x >= dw || y >= dh { DEEP } else { depth[(y * dw + x) as usize] }
        };
        let mut meshes: HashMap<u8, Mesh> = HashMap::new();
        let water_shift = self.time * 6.0;
        // every node's corner once a frame, not once per each of its six triangles (perf-2 #6:
        // 11.7 ms a frame zoomed out); the triangles' corners reach one column left and one
        // row down
        let (tx0, tw, th) = (x0 - 1, (x1 - x0 + 2).max(0), (y1 - y0 + 1).max(0));
        let table: Vec<(Pos2, Color32, Vec2, f32, f32)> = (0..th * tw)
            .map(|i| {
                let (cx, cy) = (tx0 + i % tw, y0 + i / tw);
                (
                    self.to_screen(world_pos_unwrapped(self.sim, cx, cy)),
                    self.shade(cx, cy),
                    Vec2::new(cx as f32 * TW + if cy & 1 == 1 { TW / 2.0 } else { 0.0 }, cy as f32 * TH),
                    // 0 at the shore, 1 four steps out
                    depth_at(cx, cy) as f32 / DEEP as f32,
                    self.fog_at(cx, cy),
                )
            })
            .collect();
        for uy in y0..y1 {
            for ux in x0..x1 {
                let p = wrap(ux, uy);
                let corners = |dirs: [Option<Direction>; 3]| {
                    dirs.map(|d| {
                        let (cx, cy) = match d {
                            None => (ux, uy),
                            Some(d) => neighbor_unwrapped(ux, uy, d),
                        };
                        table[((cy - y0) * tw + cx - tx0) as usize]
                    })
                };
                for (t, tri) in [
                    (world.terrain(p, rr_world::Tri::Rsu), corners([None, Some(Direction::SW), Some(Direction::SE)])),
                    (world.terrain(p, rr_world::Tri::Usd), corners([None, Some(Direction::E), Some(Direction::SE)])),
                ] {
                    let mesh = meshes.entry(t).or_insert_with(|| Mesh::with_texture(tex.get(t).id()));
                    let base = mesh.vertices.len() as u32;
                    let is_water = rr_world::terrain::is_water(t);
                    for (wp, shade, flat, deep, fog) in tri {
                        let mut uv = flat / (TW * 2.3);
                        if is_water {
                            // a slow drift in two directions, not a conveyor belt
                            uv.x += water_shift / 128.0;
                            uv.y += water_shift / 300.0;
                        }
                        let c = if is_water {
                            // light shallows fading into the deep over four steps; water lies flat:
                            // no hill light, only fog
                            let g = 230.0 * fog;
                            let lerp = |a: f32, b: f32| a + (b - a) * deep;
                            let (r, gg, b) = (lerp(SHALLOW[0], 0.95), lerp(SHALLOW[1], 1.0), lerp(SHALLOW[2], 1.02));
                            Color32::from_rgb(
                                (g * r).min(255.0) as u8,
                                (g * gg).min(255.0) as u8,
                                (g * b).min(255.0) as u8,
                            )
                        } else {
                            shade
                        };
                        mesh.vertices.push(Vertex { pos: wp, uv: uv.to_pos2(), color: c });
                    }
                    mesh.indices.extend_from_slice(&[base, base + 1, base + 2]);
                }
            }
        }
        let mut keys: Vec<u8> = meshes.keys().copied().collect();
        keys.sort();
        for k in keys {
            painter.add(Shape::mesh(meshes.remove(&k).unwrap()));
        }
        // (the edges light their corners from the terrain's table: perf round 3, #6)
        let shades = Shades { window: (x0, x1, y0, y1), x0: tx0, w: tw, v: table.iter().map(|t| t.1).collect() };
        self.draw_edges(painter, tex, &shades);
        self.draw_fields(painter, tex, (x0, x1, y0, y1));
        self.draw_clouds(painter, tex, (x0, x1, y0, y1));

        let z = self.cam.zoom;
        // ---- roads: textured strips with soft edges, a round patch at every joint
        let mut edge = Mesh::default();
        let mut dirt = Mesh::with_texture(tex.road.id());
        let mut cobbles = Mesh::with_texture(tex.cobbles.id());
        let mut joints: std::collections::HashSet<(i32, i32)> = std::collections::HashSet::new();
        for uy in y0..y1 {
            for ux in x0..x1 {
                let p = wrap(ux, uy);
                if self.vis(p) == rr_sim::Visibility::Unexplored {
                    continue;
                }
                for d in [Direction::E, Direction::SE, Direction::SW] {
                    let k = world.road(p, d);
                    if k == RoadKind::None {
                        continue;
                    }
                    let a = self.to_screen(world_pos_unwrapped(self.sim, ux, uy));
                    let (nx, ny) = neighbor_unwrapped(ux, uy, d);
                    let b = self.to_screen(world_pos_unwrapped(self.sim, nx, ny));
                    if k == RoadKind::Boat {
                        painter.line_segment([a, b], Stroke::new(6.0 * z, Color32::from_rgb(30, 60, 120)));
                        painter.line_segment([a, b], Stroke::new(3.8 * z, Color32::from_rgb(80, 120, 200)));
                        continue;
                    }
                    // a dark edge under the road, so it shows on any ground; both waver a little
                    // along the way, the same each frame (a road is trodden, not cut)
                    let seed = (p.x as u32).wrapping_mul(73_856_093) ^ (p.y as u32).wrapping_mul(19_349_663) ^ d as u32;
                    let gray = Color32::from_gray(70);
                    road_strip(&mut edge, (a, b), ROAD_HALF_WIDTH * 1.25 * z, (gray, gray), seed);
                    let mesh = if k == RoadKind::Donkey { &mut cobbles } else { &mut dirt };
                    road_strip(mesh, (a, b), ROAD_HALF_WIDTH * z, (self.shade(ux, uy), self.shade(nx, ny)), seed);
                    for (end, node) in [(a, (ux, uy)), (b, (nx, ny))] {
                        if joints.insert(node) {
                            road_joint(mesh, end, ROAD_HALF_WIDTH * z, self.shade(node.0, node.1));
                        }
                    }
                }
            }
        }
        // (the edge: a black strip at a quarter strength under the surface)
        for v in edge.vertices.iter_mut() {
            let a = (v.color.a() as f32 * 0.24) as u8;
            v.color = Color32::from_black_alpha(a);
        }
        painter.add(Shape::mesh(edge));
        painter.add(Shape::mesh(dirt));
        painter.add(Shape::mesh(cobbles));
        if let Some((c, r)) = self.work_area {
            for q in self.sim.world().grid().ring(c, r) {
                let p = self.node_screen(q);
                painter.circle(
                    p,
                    2.5 * z,
                    Color32::from_white_alpha(170),
                    Stroke::new(1.0_f32, Color32::from_black_alpha(120)),
                );
            }
        }
        // orders given in a pause: dashed, until the game runs and carries them out
        let planned = Color32::from_rgba_unmultiplied(255, 255, 255, 210);
        for m in &self.planned {
            match m {
                Planned::Site(at) => {
                    let c = self.node_screen(*at);
                    painter.circle(
                        c,
                        9.0 * z,
                        Color32::from_rgba_unmultiplied(255, 240, 160, 90),
                        Stroke::new(2.0_f32, planned),
                    );
                    painter.text(c, egui::Align2::CENTER_CENTER, "⚒", egui::FontId::proportional(13.0 * z), planned);
                }
                Planned::Flag(at) => {
                    painter.circle_stroke(self.node_screen(*at), 5.0 * z, Stroke::new(2.0_f32, planned));
                }
                Planned::Road(from, dirs) => {
                    let mut p = *from;
                    for d in dirs {
                        let (a, b) = (self.node_screen(p), self.node_screen_dir(p, *d));
                        painter.add(Shape::dashed_line(&[a, b], Stroke::new(3.0 * z, planned), 6.0 * z, 4.0 * z));
                        p = g.neighbor(p, *d);
                    }
                }
            }
        }
        if let Some((start, dirs)) = &self.road_preview {
            let mut p = *start;
            for d in dirs {
                let a = self.node_screen(p);
                let b = self.node_screen_dir(p, *d);
                painter.line_segment([a, b], Stroke::new(4.0 * z, Color32::from_rgba_unmultiplied(255, 240, 120, 200)));
                p = g.neighbor(p, *d);
            }
            let e = self.node_screen(p);
            painter.circle_stroke(e, 7.0 * z, Stroke::new(2.0_f32, Color32::YELLOW));
            // where the road can go next, coloured by the slope (the original's road markers)
            let mut visited = vec![*start];
            let mut q = *start;
            for d in dirs {
                q = g.neighbor(q, *d);
                visited.push(q);
            }
            let alt = world.altitude(p) as i32;
            for d in Direction::ALL {
                let n = g.neighbor(p, d);
                let to_flag = self.sim.flag_at(n).is_some();
                if visited.contains(&n) && !(to_flag && n != *start) {
                    continue;
                }
                let ok = self.sim.road_step_ok(self.player, p, d, false).is_ok()
                    || to_flag && self.sim.road_step_ok(self.player, p, d, true).is_ok();
                if !ok {
                    continue;
                }
                let slope = (world.altitude(n) as i32 - alt).abs();
                let m = e + (self.node_screen_dir(p, d) - e) * 0.55;
                slope_mark(painter, m, z, slope);
            }
        }

        // ---- borders
        for uy in y0..y1 {
            for ux in x0..x1 {
                let p = wrap(ux, uy);
                let Some(o) = world.owner(p) else { continue };
                if self.vis(p) == rr_sim::Visibility::Unexplored {
                    continue;
                }
                // borders over water too (coasts show whose they are); banners only on land
                let is_border = |q: MapCoord| {
                    world.owner(q) == Some(o) && g.neighbors(q).iter().any(|n| world.owner(*n) != Some(o))
                };
                let on_land = !world.terrains_around(p).iter().all(|t| rr_world::terrain::is_water(*t));
                if !is_border(p) {
                    continue;
                }
                // coastal waters only: across open sea a border read as construction lines
                // (art director round 3, #1)
                // (within 1 of land, both ends: 2 left straight stubs out at sea; art director
                // round 4, #3)
                let near_land = |q: MapCoord| {
                    g.within(q, 1).any(|r| !world.terrains_around(r).iter().all(|t| rr_world::terrain::is_water(*t)))
                };
                if !near_land(p) {
                    continue;
                }
                // a ribbon in the player's colour along the border, nudged toward the owner's
                // side (where two realms meet both colours show side by side), and a small
                // banner on every other node
                let s = self.to_screen(world_pos_unwrapped(self.sim, ux, uy));
                let c = player_color(o);
                let inward = {
                    let mut v = Vec2::ZERO;
                    for d in Direction::ALL {
                        if world.owner(g.neighbor(p, d)) == Some(o) {
                            let (nx, ny) = neighbor_unwrapped(ux, uy, d);
                            v += self.to_screen(world_pos_unwrapped(self.sim, nx, ny)) - s;
                        }
                    }
                    if v.length() > 0.01 { v.normalized() * 1.5 * z } else { Vec2::ZERO }
                };
                for d in [Direction::E, Direction::SE, Direction::SW] {
                    if is_border(g.neighbor(p, d)) && near_land(g.neighbor(p, d)) {
                        let (nx, ny) = neighbor_unwrapped(ux, uy, d);
                        let n = self.to_screen(world_pos_unwrapped(self.sim, nx, ny));
                        let (a, b) = (s + inward, n + inward);
                        let (under, top) = ribbon_widths(z);
                        // on water a light under-stroke: blue and purple vanished on dark sea
                        let edge = if on_land { Color32::from_black_alpha(60) } else { Color32::from_white_alpha(50) };
                        painter.line_segment([a, b], Stroke::new(under, edge));
                        let r = ribbon_color(c);
                        painter.line_segment(
                            [a, b],
                            Stroke::new(top, Color32::from_rgba_unmultiplied(r.r(), r.g(), r.b(), 170)),
                        );
                    }
                }
                if z >= 0.8 && (ux + uy) & 1 == 0 && on_land {
                    let top = s + inward + Vec2::new(0.0, -9.0 * z);
                    painter.line_segment([s + inward, top], Stroke::new(1.2 * z, Color32::from_rgb(70, 50, 30)));
                    painter.add(Shape::convex_polygon(
                        vec![top, top + Vec2::new(4.0 * z, 1.5 * z), top + Vec2::new(0.0, 3.0 * z)],
                        c,
                        Stroke::new(0.6 * z, Color32::from_black_alpha(120)),
                    ));
                }
            }
        }

        // ---- objects and figures, back to front
        let mut items: Vec<(f32, f32, Sprite)> = Vec::new();
        for uy in y0..y1 {
            for ux in x0..x1 {
                let p = wrap(ux, uy);
                let s = self.to_screen(world_pos_unwrapped(self.sim, ux, uy));
                if self.vis(p) == rr_sim::Visibility::Unexplored {
                    continue;
                }
                let sp = match world.obj(p) {
                    // loose boulders on bare mountain rock (drawn only: nothing in the game)
                    NodeObj::None
                        if self.scenery
                            && (p.x as u32 * 7 + p.y as u32 * 13 + (p.x as u32 * p.y as u32) % 5) % 3 == 0
                            && world.terrains_around(p).iter().all(|t| rr_world::terrain::is_mountain(*t))
                            && !world.is_on_road(p) =>
                    {
                        // one in three a lit crag of mountain stone (art director round 2, #4)
                        let h = (p.x as u32).wrapping_mul(73_856_093) ^ (p.y as u32).wrapping_mul(19_349_663);
                        if z >= 0.55 && h % 3 == 0 {
                            const CRAGS: [&str; 4] = ["crag1", "crag2", "crag3", "crag4"];
                            Sprite::Prop(CRAGS[((h >> 4) % 4) as usize], 0.45 + ((h >> 8) % 20) as f32 / 100.0)
                        } else {
                            Sprite::Decoration(0x16)
                        }
                    }
                    // small props on free land: bushes, flowers, stumps, reeds by the water (draw
                    // only; art director round 2, #6)
                    NodeObj::None if z >= 0.55 && self.scenery => match meadow_prop(world, p) {
                        Some(sp) => sp,
                        None => continue,
                    },
                    NodeObj::None | NodeObj::Extension => continue,
                    NodeObj::Tree { species, stage } => Sprite::Tree {
                        species,
                        stage,
                        seed: (p.x as u32) * 31 + p.y as u32,
                        look: match world.landscape {
                            rr_core::Landscape::Greenland => 0,
                            rr_core::Landscape::Wasteland => 1,
                            rr_core::Landscape::Winter => 2,
                        },
                    },
                    NodeObj::Granite { size, kind } => Sprite::Granite { size, kind },
                    NodeObj::Decoration(id) => self.decoration(p, id),
                    NodeObj::Field(st) => Sprite::Field(st),
                    NodeObj::Fire => Sprite::Fire,
                    NodeObj::Sign(code) => Sprite::Sign(code),
                    NodeObj::ShipSite(step) => Sprite::ShipSite(step),
                    NodeObj::Flag(_) => {
                        let Some(f) = self.sim.flag_at(p) else { continue };
                        let fl = self.sim.flag(f);
                        let wares: Vec<rr_rules::Ware> = fl.wares.iter().map(|w| self.sim.wares()[*w].kind).collect();
                        Sprite::Flag { color: player_color(fl.player), wares }
                    }
                    NodeObj::Building(_) => {
                        let Some(b) = self.sim.building_at(p) else { continue };
                        let bd = self.sim.building(b);
                        Sprite::Building {
                            kind: bd.kind,
                            color: player_color(bd.player),
                            progress: if bd.is_ready() { None } else { Some(self.sim.site_progress_percent(b)) },
                            working: bd.worker.is_some() && bd.is_ready(),
                            soldiers: bd.soldiers.len() as u8,
                            nation: self.sim.players()[bd.player as usize].setup.nation as u8,
                        }
                    }
                };
                // loose boulders lie anywhere around their node, not on the grid
                let s = if matches!(sp, Sprite::Decoration(0x16) | Sprite::Prop(..))
                    && matches!(world.obj(p), NodeObj::None)
                {
                    let h = (p.x as u32).wrapping_mul(2_654_435_761) ^ (p.y as u32).wrapping_mul(40_503);
                    let j = |k: u32| ((h >> k) % 7) as f32 / 6.0 - 0.5;
                    s + Vec2::new(j(3) * 0.6 * TW, j(9) * 0.6 * TH) * z
                } else {
                    s
                };
                items.push((s.y, s.x, sp));
            }
        }
        // figures
        let rect = self.rect.expand(80.0);
        let fighters = self.fighters();
        for (id, f) in self.sim.figures().iter() {
            if !f.is_visible() || !self.seen(f.pos) {
                continue;
            }
            let base = self.node_screen(f.pos);
            if !rect.contains(base) {
                continue;
            }
            let (pos, walking) = match f.motion {
                Motion::Walk { dir, start, dur } => {
                    let to = self.node_screen_dir(f.pos, dir);
                    let t = ((self.render_gf - start as f32) / dur as f32).clamp(0.0, 1.0);
                    (base + (to - base) * t, Some(dir))
                }
                _ => (base, None),
            };
            // two fighters on one node stand apart, facing each other
            let shift = fighters.get(&id).map_or(0.0, |x| x.0);
            let pos = pos + Vec2::new(shift * self.cam.zoom, 0.0);
            let anim = (self.render_gf / 4.0) as u32;
            let fight = fighters.get(&id).map(|x| x.1);
            let sp = self.idler(id, f).unwrap_or_else(|| figure_sprite(f, walking, fight, anim));
            if let FigKind::Carrier { road, .. } = &f.kind
                && self.sim.roads().get(*road).is_some_and(|r| r.kind == RoadKind::Boat)
                && self.sim.is_water_point(f.pos)
            {
                items.push((pos.y + 0.4, pos.x, Sprite::Boat));
            }
            items.push((pos.y + 0.5, pos.x, sp));
        }
        // ships
        for (_, s) in self.sim.ships().iter() {
            if !self.seen(s.pos) {
                continue;
            }
            let base = self.node_screen(s.pos);
            if !rect.contains(base) {
                continue;
            }
            let (pos, dir) = match s.motion {
                Motion::Walk { dir, start, dur } => {
                    let to = self.node_screen_dir(s.pos, dir);
                    let t = ((self.render_gf - start as f32) / dur as f32).clamp(0.0, 1.0);
                    (base + (to - base) * t, Some(dir))
                }
                _ => (base, None),
            };
            let loaded = !matches!(s.task, rr_sim::ShipTask::Idle | rr_sim::ShipTask::ToHarbor(_));
            items.push((pos.y + 0.5, pos.x, Sprite::Ship { color: player_color(s.player), loaded, dir }));
        }
        items.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then(a.1.partial_cmp(&b.1).unwrap()));
        // one sun, upper left: every building and tree casts its own outline to the lower right,
        // all before any object, so shadows lie on the ground (art director round 2, #5)
        for (y, x, sp) in &items {
            let at = Pos2::new(*x, *y);
            match sp {
                Sprite::Building { kind, nation, progress, .. } => {
                    if let Some((tex_id, w, h, ax, ay)) = tex
                        .buildings
                        .get(&(*kind, *nation))
                        .or_else(|| tex.buildings.get(&(*kind, 0)))
                        .map(|b| (b.0.id(), b.1, b.2, b.3, b.4))
                    {
                        let k = if *kind == rr_rules::BuildingType::Headquarters { 1.3 } else { 1.0 };
                        let s = z * 0.5 * k;
                        let full = Rect::from_min_size(at - Vec2::new(ax, ay) * s, Vec2::new(w, h) * s);
                        // a site casts the shadow of what stands of it so far
                        let built = progress.map_or(1.0, |p| (p as f32 / 100.0).clamp(0.0, 1.0));
                        silhouette_shadow(painter, tex_id, full, at.y, built);
                    }
                }
                Sprite::Prop(key, k) => {
                    if let Some((t, w, h)) = tex.props.get(*key) {
                        silhouette_shadow(painter, t.id(), prop_rect(at, *w, *h, *k * z), at.y, 1.0);
                    }
                }
                Sprite::Holy { god, .. } => holy_shadow(painter, tex, at, z, *god),
                Sprite::Granite { size, .. } if z >= 0.55 => {
                    if let Some((t, w, h)) = tex.props.get(&format!("granite{}", (*size).clamp(1, 6))) {
                        silhouette_shadow(painter, t.id(), prop_rect(at, *w, *h, 0.5 * z), at.y, 1.0);
                    }
                }
                Sprite::Tree { species, stage, look, .. } if z >= 0.55 => {
                    if let Some((t, w, h)) = tex.trees.get(sprites::tree_kind(*species, *look)) {
                        let height = 40.0 * sprites::tree_growth(*stage) * z;
                        let width = height * w / h;
                        let full = Rect::from_min_max(
                            Pos2::new(at.x - width / 2.0, at.y - height),
                            Pos2::new(at.x + width / 2.0, at.y + 0.5 * z),
                        );
                        silhouette_shadow(painter, t.id(), full, at.y, 1.0);
                    }
                }
                _ => {}
            }
        }
        for (y, x, sp) in &items {
            let at = Pos2::new(*x, *y);
            if let Sprite::Building { kind, color, progress, soldiers, nation, working } = sp
                && let Some((tex_id, w, h, ax, ay)) = tex
                    .buildings
                    .get(&(*kind, *nation))
                    .or_else(|| tex.buildings.get(&(*kind, 0)))
                    .map(|b| (b.0.id(), b.1, b.2, b.3, b.4))
            {
                // the seat of power is the biggest building on the map
                let k = if *kind == rr_rules::BuildingType::Headquarters { 1.3 } else { 1.0 };
                let (w, h, ax, ay) = (w * k, h * k, ax * k, ay * k);
                draw_building_image(
                    painter,
                    at,
                    z,
                    tex_id,
                    (w, h, ax, ay),
                    *kind,
                    *color,
                    *progress,
                    *soldiers,
                    self.time,
                );
                if *working && progress.is_none() {
                    chimney_smoke(painter, at, z, (w, h, ax, ay), *kind, self.time);
                }
                if progress.is_none() {
                    yard_animals(painter, at, z, *kind, self.time);
                }
                continue;
            }
            if let Sprite::Holy { god, mine } = sp
                && self.draw_holy(painter, tex, at, *god, *mine)
            {
                continue;
            }
            // props and granite from their pictures
            let picture = match sp {
                Sprite::Prop(key, k) => tex.props.get(*key).map(|t| (t, *k, prop_tint(key))),
                Sprite::Granite { size, .. } if z >= 0.55 => {
                    tex.props.get(&format!("granite{}", (*size).clamp(1, 6))).map(|t| (t, 0.5, Color32::WHITE))
                }
                _ => None,
            };
            if let Some(((t, w, h), k, tint)) = picture {
                painter.image(
                    t.id(),
                    prop_rect(at, *w, *h, k * z),
                    Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    tint,
                );
                continue;
            }
            if matches!(sp, Sprite::Prop(..)) {
                continue;
            }
            // trees from their pictures: one textured quad each, the top swaying (far away the
            // few-shape vector tree is cheaper and looks the same)
            if let Sprite::Tree { species, stage, seed, look } = sp
                && z >= 0.55
                && let Some((t, w, h)) = tex.trees.get(sprites::tree_kind(*species, *look))
            {
                tree_picture(painter, t.id(), (*w, *h), at, z, (*stage, *seed), self.time);
                continue;
            }
            sprites::draw(painter, at, z, sp, self.time);
        }
        // the selected node: a ring in front of everything, on empty ground too
        if let Some(p) = self.selected {
            let s = self.node_screen(p);
            painter.circle_stroke(s, 9.0 * z, Stroke::new(2.5_f32, Color32::from_black_alpha(140)));
            painter.circle_stroke(s, 9.0 * z, Stroke::new(1.5_f32, Color32::WHITE));
        }

        // ---- productivity overlay
        if self.show_prod {
            for (id, b) in self.sim.buildings().iter() {
                if b.player != self.player || !self.seen(b.pos) {
                    continue;
                }
                let s = self.node_screen(b.pos);
                if !self.rect.contains(s) {
                    continue;
                }
                let def = self.sim.rules().building(b.kind);
                let (txt, col) = if !b.is_ready() {
                    // (in brackets: a site's progress isn't a workshop's productivity)
                    (
                        format!("[{}]", crate::i18n::pct(self.sim.site_progress_percent(id))),
                        Color32::from_rgb(230, 230, 230),
                    )
                } else if def.worker.is_some() && !def.is_military() && b.attempts > 0 {
                    prod_label(b.productivity())
                } else {
                    continue;
                };
                let pos = s + Vec2::new(0.0, 8.0 * z);
                let font = egui::FontId::proportional(11.0 * z.clamp(1.0, 1.6));
                label_on_plate(painter, pos, &txt, font, col, None);
            }
            // and the flags where wares pile up
            let cap = self.sim.rules().economy.flag_capacity as usize;
            for (_, f) in self.sim.flags().iter() {
                if f.player != self.player || !self.seen(f.pos) {
                    continue;
                }
                let Some((txt, col)) = flag_label(f.wares.len(), cap) else { continue };
                let s = self.node_screen(f.pos);
                if !self.rect.contains(s) {
                    continue;
                }
                let font = egui::FontId::proportional(10.0 * z.clamp(1.0, 1.6));
                label_on_plate(painter, s + Vec2::new(0.0, -26.0 * z), &txt, font, col, None);
            }
        }

        // ---- building names
        if self.show_names {
            for (_, b) in self.sim.buildings().iter() {
                if !self.seen(b.pos) {
                    continue;
                }
                let s = self.node_screen(b.pos);
                if !self.rect.contains(s) {
                    continue;
                }
                // below the house's foot, not on its roof; hidden when zoomed far out
                if z < 0.7 {
                    continue;
                }
                let pos = s + Vec2::new(0.0, 8.0 * z + if self.show_prod { 18.0 } else { 0.0 });
                let font = egui::FontId::proportional(11.0 * z.clamp(1.0, 1.6));
                let name = crate::ui::building_name(b.kind);
                label_on_plate(painter, pos, name, font, PLATE_TEXT, Some(player_color(b.player)));
            }
        }

        // ---- building-quality overlay
        if self.show_bq {
            for uy in y0..y1 {
                for ux in x0..x1 {
                    let p = wrap(ux, uy);
                    if world.owner(p) != Some(self.player) {
                        continue;
                    }
                    let bq = world.bq_for(p, self.player);
                    let s = self.to_screen(world_pos_unwrapped(self.sim, ux, uy));
                    sprites::bq_icon(painter, s, z, bq);
                }
            }
        }
        let _ = Bq::Nothing;
    }
}

/// A building from the art pack. Images are stored at 2x; sites rise from the ground.
/// The product shown on a workshop's sign (the medium houses look alike otherwise).
pub fn sign_ware(kind: rr_rules::BuildingType) -> Option<rr_rules::Ware> {
    use rr_rules::{BuildingType as B, Ware as W};
    Some(match kind {
        B::Sawmill => W::Boards,
        B::Bakery => W::Bread,
        B::Brewery => W::Beer,
        B::Armory => W::Sword,
        B::Mint => W::Coins,
        B::Slaughterhouse => W::Meat,
        B::IronSmelter => W::Iron,
        B::Metalworks => W::Hammer,
        B::Shipyard => W::Boat,
        _ => return None,
    })
}

/// The animals of a farm yard, ambling in front of it (item 336: pigs and donkeys were only part
/// of the picture): three pigs at a pig farm, two donkeys at the breeder.
fn yard_animals(painter: &egui::Painter, at: Pos2, z: f32, kind: rr_rules::BuildingType, time: f32) {
    use rr_rules::BuildingType as B;
    let spots: &[(f32, f32)] = match kind {
        B::PigFarm => &[(14.0, 4.0), (20.0, 9.0), (9.0, 10.0)],
        B::DonkeyBreeder => &[(16.0, 5.0), (8.0, 11.0)],
        _ => return,
    };
    for (k, (x, y)) in spots.iter().enumerate() {
        // a slow amble to and fro, a pause now and then
        let t = time * 0.35 + k as f32 * 2.3;
        let dx = (t.sin() * 4.0).clamp(-3.0, 3.0);
        let walking = t.cos().abs() > 0.3;
        let anim = (time * 6.0) as u32;
        let p = at + Vec2::new((x + dx) * z, y * z);
        if kind == B::DonkeyBreeder {
            sprites::draw(painter, p, z, &Sprite::Donkey { carry: None, walking, west: t.cos() < 0.0, anim }, time);
        } else {
            let pink = Color32::from_rgb(232, 168, 160);
            let bob = if walking && anim % 2 == 0 { -0.4 * z } else { 0.0 };
            for lx in [-2.0f32, -1.0, 1.4, 2.4] {
                painter.line_segment(
                    [p + Vec2::new(lx * z, -1.6 * z + bob), p + Vec2::new(lx * z, 0.0)],
                    Stroke::new(0.9 * z, Color32::from_rgb(170, 110, 104)),
                );
            }
            painter.add(Shape::ellipse_filled(p + Vec2::new(0.0, -2.8 * z + bob), Vec2::new(3.4 * z, 2.0 * z), pink));
            painter.circle_filled(p + Vec2::new(3.4 * z, -3.0 * z + bob), 1.5 * z, pink);
            painter.add(Shape::ellipse_filled(
                p + Vec2::new(4.8 * z, -2.8 * z + bob),
                Vec2::new(0.6 * z, 0.5 * z),
                Color32::from_rgb(210, 130, 128),
            ));
            painter.circle_filled(p + Vec2::new(3.8 * z, -3.6 * z + bob), 0.25 * z, Color32::from_rgb(40, 30, 30));
        }
    }
}

/// Smoke from a working workshop's chimney: puffs that rise, drift with the wind and fade.
fn chimney_smoke(
    painter: &egui::Painter,
    at: Pos2,
    z: f32,
    (w, h, ax, ay): (f32, f32, f32, f32),
    kind: rr_rules::BuildingType,
    time: f32,
) {
    use rr_rules::BuildingType as B;
    // where the chimney stands in the picture (fractions of its width and height)
    let (fx, fy) = match kind {
        B::Bakery | B::Brewery | B::Armory => (0.7, 0.1),
        B::Metalworks => (0.62, 0.1),
        B::IronSmelter => (0.68, 0.02),
        B::Mint | B::Slaughterhouse => (0.5, 0.15),
        _ => return,
    };
    let s = z * 0.5;
    let origin = at - Vec2::new(ax, ay) * s + Vec2::new(w * fx, h * fy) * s;
    for i in 0..5 {
        let ph = (time * 0.33 + i as f32 / 5.0).fract();
        let drift = Vec2::new(ph * 9.0 + (ph * 6.0 + i as f32).sin() * 1.5, -ph * 28.0) * z;
        let alpha = ((1.0 - ph) * (ph * 6.0).min(1.0) * 110.0) as u8;
        painter.circle_filled(
            origin + drift,
            (1.8 + ph * 4.5) * z,
            Color32::from_rgba_unmultiplied(205, 205, 210, alpha),
        );
    }
}

fn draw_building_image(
    painter: &egui::Painter,
    at: Pos2,
    z: f32,
    tex: egui::TextureId,
    (w, h, ax, ay): (f32, f32, f32, f32),
    kind: rr_rules::BuildingType,
    color: Color32,
    progress: Option<u32>,
    soldiers: u8,
    time: f32,
) {
    let s = z * 0.5;
    let min = at - Vec2::new(ax, ay) * s;
    let full = Rect::from_min_size(min, Vec2::new(w, h) * s);
    // a faint contact shadow right under it (the cast shadow is drawn before all objects)
    painter.add(Shape::ellipse_filled(at, Vec2::new(w * 0.36 * s, h * 0.07 * s), Color32::from_black_alpha(35)));
    match progress {
        Some(pct) => {
            // ground and stakes, then the building appears from the bottom up
            painter.add(Shape::ellipse_filled(
                at,
                Vec2::new(w * 0.4 * s, h * 0.1 * s),
                Color32::from_rgb(150, 120, 80),
            ));
            let k = (pct as f32 / 100.0).clamp(0.0, 1.0);
            // what it will be: the finished building, faint, from the first day
            painter.image(
                tex,
                full,
                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::from_rgba_unmultiplied(255, 255, 255, 70),
            );
            if let Some(ware) = sign_ware(kind) {
                let c = at + Vec2::new(w * 0.3 * s, -9.0 * z);
                painter.circle(
                    c,
                    6.0 * z,
                    Color32::from_rgb(216, 196, 154),
                    Stroke::new(0.8 * z, Color32::from_rgb(70, 52, 30)),
                );
                sprites::ware_icon(painter, c, 9.0 * z, ware);
            }
            if k > 0.0 {
                let top = 1.0 - k;
                let r = Rect::from_min_max(egui::pos2(full.min.x, full.min.y + full.height() * top), full.max);
                let uv = Rect::from_min_max(egui::pos2(0.0, top), egui::pos2(1.0, 1.0));
                painter.image(tex, r, uv, Color32::from_rgba_unmultiplied(235, 225, 210, 235));
            }
            // scaffolding: poles that grow with the walls, a plank at every level
            let wood = Color32::from_rgb(120, 86, 44);
            let built = (full.height() * k * 0.8).max(14.0 * z);
            let (l, r) = (at.x - w * 0.36 * s, at.x + w * 0.36 * s);
            for x in [l, r, at.x - w * 0.12 * s] {
                painter.line_segment(
                    [egui::pos2(x, at.y), egui::pos2(x, at.y - built - 4.0 * z)],
                    egui::Stroke::new(1.4_f32 * z, wood),
                );
            }
            let mut y = at.y - 9.0 * z;
            while y > at.y - built {
                painter.line_segment(
                    [egui::pos2(l - 2.0 * z, y), egui::pos2(r + 2.0 * z, y)],
                    egui::Stroke::new(1.8_f32 * z, Color32::from_rgb(150, 112, 62)),
                );
                y -= 11.0 * z;
            }
            // the materials waiting on the ground: boards and stones
            let pile = at + Vec2::new(-w * 0.46 * s, 3.0 * z);
            for i in 0..3 {
                let o = Vec2::new(i as f32 * 0.6 * z, -(i as f32) * 1.6 * z);
                painter.rect_filled(
                    Rect::from_center_size(pile + o, Vec2::new(10.0, 1.6) * z),
                    0.0,
                    Color32::from_rgb(196, 152, 92),
                );
            }
            let heap = at + Vec2::new(w * 0.46 * s, 3.0 * z);
            for (dx, dy) in [(-2.0, 0.0), (2.0, 0.0), (0.0, -2.2)] {
                painter.rect_filled(
                    Rect::from_center_size(heap + Vec2::new(dx, dy) * z, Vec2::splat(3.4 * z)),
                    0.6 * z,
                    Color32::from_rgb(158, 156, 150),
                );
            }
            // dust where the builder works
            if k > 0.0 && k < 1.0 {
                for i in 0..3 {
                    let ph = (time * 0.6 + i as f32 / 3.0).fract();
                    let x = at.x + (i as f32 - 1.0) * w * 0.18 * s + ph * 4.0 * z;
                    let y = at.y - built + 2.0 * z - ph * 10.0 * z;
                    let a = ((1.0 - ph) * 70.0) as u8;
                    painter.circle_filled(
                        egui::pos2(x, y),
                        (2.0 + ph * 4.0) * z,
                        Color32::from_rgba_unmultiplied(196, 176, 140, a),
                    );
                }
            }
        }
        None => {
            painter.image(tex, full, Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
            // a sign with the product in front of look-alike workshops
            if let Some(ware) = sign_ware(kind) {
                let foot = at + Vec2::new(w * 0.3 * s, 2.0 * z);
                let board = Rect::from_center_size(foot - Vec2::new(0.0, 11.0 * z), Vec2::new(13.0, 12.0) * z);
                painter.line_segment([foot, board.center()], egui::Stroke::new(1.2 * z, Color32::from_rgb(70, 45, 20)));
                painter.rect(
                    board,
                    1.5 * z,
                    Color32::from_rgb(196, 160, 104),
                    egui::Stroke::new(0.8 * z, Color32::from_rgb(70, 45, 20)),
                    egui::StrokeKind::Inside,
                );
                sprites::ware_icon(painter, board.center(), 9.5 * z, ware);
            }
            // player colour: a pennant over the roof
            let top = egui::pos2(full.center().x + w * 0.08 * s, full.min.y + 2.0 * z);
            let military = matches!(
                kind,
                rr_rules::BuildingType::Barracks
                    | rr_rules::BuildingType::Guardhouse
                    | rr_rules::BuildingType::Watchtower
                    | rr_rules::BuildingType::Fortress
                    | rr_rules::BuildingType::Headquarters
            );
            let pole = if military { 16.0 } else { 9.0 } * z;
            painter.line_segment(
                [top, top - Vec2::new(0.0, pole)],
                egui::Stroke::new(1.3_f32 * z, Color32::from_rgb(60, 40, 20)),
            );
            let wave = (time * 3.0).sin() * z;
            let fl = if military { 9.0 } else { 6.0 } * z;
            painter.add(Shape::convex_polygon(
                vec![
                    top - Vec2::new(0.0, pole),
                    top - Vec2::new(-fl, pole - 2.0 * z + wave),
                    top - Vec2::new(0.0, pole - fl * 0.6),
                ],
                color,
                egui::Stroke::new(0.8_f32, Color32::from_black_alpha(120)),
            ));
            if military && kind != rr_rules::BuildingType::Headquarters {
                for i in 0..soldiers.min(9) {
                    painter.circle_filled(at + Vec2::new((-10.0 + i as f32 * 3.0) * z, 5.0 * z), 1.3 * z, color);
                }
            }
        }
    }
}

/// Half the width of a road at zoom 1.
const ROAD_HALF_WIDTH: f32 = 6.0;

/// How far two fighters on one node stand off it, each to its side (world pixels).
pub const FIGHT_GAP: f32 = TW * 0.17;

/// A tree from its picture: one textured quad, the top swaying.
fn tree_picture(
    painter: &egui::Painter,
    t: egui::TextureId,
    (w, h): (f32, f32),
    at: Pos2,
    z: f32,
    (stage, seed): (u8, u32),
    time: f32,
) {
    let s = sprites::tree_growth(stage);
    let sway = ((time * 1.3 + seed as f32 * 0.37).sin()) * 1.2 * s * z;
    let height = 40.0 * s * z;
    let width = height * w / h;
    let mut m = Mesh::with_texture(t);
    let (l, r) = (at.x - width / 2.0, at.x + width / 2.0);
    let (top, bottom) = (at.y - height, at.y + 0.5 * z);
    let white = Color32::WHITE;
    m.vertices.push(Vertex { pos: Pos2::new(l + sway, top), uv: egui::pos2(0.0, 0.0), color: white });
    m.vertices.push(Vertex { pos: Pos2::new(r + sway, top), uv: egui::pos2(1.0, 0.0), color: white });
    m.vertices.push(Vertex { pos: Pos2::new(r, bottom), uv: egui::pos2(1.0, 1.0), color: white });
    m.vertices.push(Vertex { pos: Pos2::new(l, bottom), uv: egui::pos2(0.0, 1.0), color: white });
    m.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    painter.add(Shape::mesh(m));
}

/// A holy place's shadow, as a building's.
fn holy_shadow(painter: &egui::Painter, tex: &TerrainTextures, at: Pos2, z: f32, god: u8) {
    if let Some((t, r)) = holy_rect(tex, at, z, god) {
        silhouette_shadow(painter, t, r, at.y, 1.0);
    }
}

/// Where a holy place's picture lies on the screen (as a building's), and its texture.
fn holy_rect(tex: &TerrainTextures, at: Pos2, z: f32, god: u8) -> Option<(egui::TextureId, Rect)> {
    let (t, w, h, ax, ay) = tex.holy.get(&god)?;
    let s = z * 0.5;
    Some((t.id(), Rect::from_min_size(at - Vec2::new(*ax, *ay) * s, Vec2::new(*w, *h) * s)))
}

impl View<'_> {
    /// A map decoration's picture: a holy place's, and whether the viewer holds it, or a bush,
    /// stone or flowers.
    fn decoration(&self, p: MapCoord, id: u8) -> Sprite {
        let w = self.sim.world();
        match rr_world::holy_god(NodeObj::Decoration(id)) {
            Some(god) => Sprite::Holy { god, mine: w.holy_holder(p) == rr_world::Holder::Player(self.player) },
            None => Sprite::Decoration(id),
        }
    }

    /// A holy place's picture, and over one the viewer holds, birds of omen circling high up
    /// (Jupiter's vultures: docs/CAMPAIGN-ROME.md). False without its picture.
    fn draw_holy(&self, painter: &egui::Painter, tex: &TerrainTextures, at: Pos2, god: u8, mine: bool) -> bool {
        let Some((t, r)) = holy_rect(tex, at, self.cam.zoom, god) else { return false };
        let full = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        painter.image(t, r, full, Color32::WHITE);
        if mine {
            let z = self.cam.zoom;
            let vulture = Color32::from_rgb(70, 58, 50);
            for i in 0..5 {
                // each on its own circle above the hill, slowly round, wings beating now and then
                let a = self.time * 0.35 + i as f32 * 1.26;
                let rad = (26.0 + i as f32 * 7.0) * z;
                let c = r.center_top() + Vec2::new(a.cos() * rad, -18.0 * z + a.sin() * rad * 0.35);
                let flap = (self.time * 0.8 + i as f32 * 0.3).fract();
                crate::animals::omen_bird(painter, c, 0.9 * z, a.sin() > 0.0, flap, vulture);
            }
        }
        true
    }

    /// A carrier idle long enough for one of its idle motions (`idle::pose_at`), if it is one.
    fn idler(&self, id: rr_sim::FigId, f: &rr_sim::Figure) -> Option<Sprite> {
        let since = self.idle?.get(&id)?;
        let (pose, t) = crate::idle::pose_at(id.index() as u32, self.render_gf - *since as f32)?;
        Some(Sprite::Idler { color: player_color(f.player), pose, t, west: false })
    }
}

/// A figure's picture: an animal, a pack donkey or a person (`fight`: in a fight, facing west
/// or not).
fn figure_sprite(f: &rr_sim::Figure, walking: Option<Direction>, fight: Option<bool>, anim: u32) -> Sprite {
    let west = matches!(walking, Some(Direction::W | Direction::NW | Direction::SW));
    match &f.kind {
        FigKind::Animal { species } => Sprite::Animal { species: *species, walking: walking.is_some(), west, anim },
        _ if f.job == rr_rules::Job::PackDonkey => {
            Sprite::Donkey { carry: f.carry, walking: walking.is_some(), west, anim }
        }
        _ => Sprite::Figure {
            job: f.job,
            color: if f.player == 255 { Color32::GRAY } else { player_color(f.player) },
            carry: f.carry,
            walking,
            face_west: fight == Some(true),
            working: matches!(f.motion, Motion::Work { .. }) || fight.is_some(),
            anim,
            hp: f.hp,
        },
    }
}

/// Where an attacker and a defender stand and which way they face: (offset, faces west) for
/// each. On one node (`same`) the attacker steps west and the defender east; on two, each turns
/// to the other (`defender_west`: the defender is drawn west of the attacker).
pub fn fight_stance(same: bool, defender_west: bool) -> ((f32, bool), (f32, bool)) {
    if same { ((-FIGHT_GAP, false), (FIGHT_GAP, true)) } else { ((0.0, defender_west), (0.0, !defender_west)) }
}

/// The surface's own light times the land's light at that end.
fn lit(light: Color32, alpha: u8) -> Color32 {
    let f = alpha as f32 / 255.0;
    Color32::from_rgba_premultiplied(
        (light.r() as f32 * f) as u8,
        (light.g() as f32 * f) as u8,
        (light.b() as f32 * f) as u8,
        alpha,
    )
}

/// A soft white disc: `alpha` in the middle, clear at the rim.
/// River triangles (by centroid) linked into chains, each in order along its river: neighbours
/// are closer than `link`; a chain starts at an end (one neighbour) where it has one.
pub fn river_chains(pts: &[Vec2], link: f32) -> Vec<Vec<usize>> {
    let near: Vec<Vec<usize>> = (0..pts.len())
        .map(|i| (0..pts.len()).filter(|j| *j != i && (pts[*j] - pts[i]).length() <= link).collect())
        .collect();
    let mut used = vec![false; pts.len()];
    let mut chains = Vec::new();
    // ends first, so a chain runs from one end to the other
    let mut order: Vec<usize> = (0..pts.len()).collect();
    order.sort_by_key(|i| near[*i].len());
    for start in order {
        if used[start] {
            continue;
        }
        let mut chain = vec![start];
        used[start] = true;
        let mut at = start;
        // walk to the nearest unused neighbour until none is left
        while let Some(next) = near[at]
            .iter()
            .copied()
            .filter(|j| !used[*j])
            .min_by(|a, b| (pts[*a] - pts[at]).length().total_cmp(&(pts[*b] - pts[at]).length()))
        {
            used[next] = true;
            chain.push(next);
            at = next;
        }
        chains.push(chain);
    }
    chains
}

/// One pass of Chaikin's corner cutting; the ends stay where they are.
pub fn chaikin(line: &[Vec2]) -> Vec<Vec2> {
    if line.len() < 3 {
        return line.to_vec();
    }
    let mut out = vec![line[0]];
    for w in line.windows(2) {
        out.push(w[0] * 0.75 + w[1] * 0.25);
        out.push(w[0] * 0.25 + w[1] * 0.75);
    }
    out.push(line[line.len() - 1]);
    out
}

/// A border ribbon's colour: the player's, a quarter toward grey and a little darker.
fn ribbon_color(c: Color32) -> Color32 {
    let l = 0.3 * c.r() as f32 + 0.59 * c.g() as f32 + 0.11 * c.b() as f32;
    let m = |v: u8| ((v as f32 * 0.72 + l * 0.28) * 0.9) as u8;
    Color32::from_rgb(m(c.r()), m(c.g()), m(c.b()))
}

/// Where a prop's picture of `w`x`h` pixels goes, standing on `at` (its foot a little above the
/// picture's bottom, as for a rounded object seen from above).
fn prop_rect(at: Pos2, w: f32, h: f32, scale: f32) -> Rect {
    let (w, h) = (w * scale, h * scale);
    Rect::from_min_size(Pos2::new(at.x - w / 2.0, at.y - h * 0.85), Vec2::new(w, h))
}

/// A prop for a free node, or none: about one node in four on meadow, reeds on the shore, a dry
/// set in the wasteland and a snowy one in winter.
/// A prop's colour over its picture. The meadow's pebbles are earth-brown and a little see-through,
/// so they never read as the grey quarry rocks that block building (owner's playtest
/// 2026-10-27: "we must just be sure that it doesn't interfere visually").
fn prop_tint(key: &str) -> Color32 {
    match key {
        "stones1" | "stones2" => Color32::from_rgba_unmultiplied(196, 168, 128, 215),
        _ => Color32::WHITE,
    }
}

/// How big a meadow prop is drawn: pebbles at most half a quarry rock's size.
fn prop_scale(key: &str, scale: f32) -> f32 {
    if key.starts_with("stones") { scale * 0.7 } else { scale }
}

fn meadow_prop(world: &rr_world::World, p: MapCoord) -> Option<Sprite> {
    if world.is_on_road(p) {
        return None;
    }
    let h = (p.x as u32).wrapping_mul(2_246_822_519) ^ (p.y as u32).wrapping_mul(3_266_489_917);
    let h = (h ^ (h >> 15)).wrapping_mul(668_265_263);
    let ts = world.terrains_around(p);
    let wet = ts.iter().filter(|t| rr_world::terrain::is_water(**t)).count();
    let land = |t: &u8| !rr_world::terrain::is_water(*t) && !rr_world::terrain::is_mountain(*t) && *t != 2;
    let scale = 0.45 + ((h >> 20) % 16) as f32 / 100.0;
    if wet > 0 && wet < 6 && ts.iter().any(land) {
        // the shore: reeds on one node in three (none in winter)
        return (h % 3 == 0 && world.landscape != rr_core::Landscape::Winter)
            .then_some(Sprite::Prop(if (h >> 8) & 1 == 0 { "reeds1" } else { "reeds2" }, scale * 0.75));
    }
    if h % 4 != 0 || !ts.iter().all(|t| land(t) && rr_world::terrain::is_vital(*t)) {
        return None;
    }
    let set: &[&'static str] = match world.landscape {
        rr_core::Landscape::Greenland => &[
            "bush1", "bush2", "bush3", "flowers1", "flowers2", "flowers3", "stump", "tuft1", "tuft2", "tuft1", "tuft2",
            "stones1", "stones2",
        ],
        rr_core::Landscape::Wasteland => &["drybush", "drybush", "bones", "stones1", "stones2", "tuft2"],
        rr_core::Landscape::Winter => &["snowbush", "snowbush", "icestones", "stones2"],
    };
    let key = set[((h >> 4) as usize) % set.len()];
    Some(Sprite::Prop(key, prop_scale(key, scale)))
}

/// The outline of a field's patch around its node at screen point `c`: an eight-sided shape
/// flattened like the terrain, reaching most of the way to the neighbours.
pub fn field_patch(c: Pos2, zoom: f32, world: Vec2) -> Vec<Pos2> {
    // 16 corners, each at a radius of its own from world-space noise, so a field's outline is
    // ragged like the terrain edges and the same at every zoom (art director round 4, #5: a
    // regular octagon read as a placeholder)
    (0..16)
        .map(|k| {
            let a = k as f32 / 16.0 * std::f32::consts::TAU + 0.2;
            let dir = Vec2::new(a.cos(), a.sin());
            let j = 0.85 + (edge_depth(world + dir * 30.0, 1.0) - 0.55) / 0.9 * 0.3;
            c + Vec2::new(dir.x * TW * 0.8, dir.y * TH * 0.95) * j * zoom
        })
        .collect()
}

/// A sprite's cast shadow: its picture's part above the foot, flattened and sheared to the lower
/// right (sun at the upper left). Corners top-left, top-right, foot-right, foot-left, with the
/// picture's v at the foot.
pub fn shadow_quad(full: Rect, foot_y: f32) -> ([Pos2; 4], f32) {
    let above = (foot_y - full.min.y).max(0.0);
    let vf = (above / full.height()).clamp(0.0, 1.0);
    let shift = Vec2::new(0.6 * above, -0.22 * above);
    (
        [
            Pos2::new(full.min.x, foot_y) + shift,
            Pos2::new(full.max.x, foot_y) + shift,
            Pos2::new(full.max.x, foot_y),
            Pos2::new(full.min.x, foot_y),
        ],
        vf,
    )
}

/// Draw the cast shadow of a textured sprite; `built` 0..1 casts only the lower part (a site).
fn silhouette_shadow(painter: &egui::Painter, tex: egui::TextureId, full: Rect, foot_y: f32, built: f32) {
    let (q, vf) = shadow_quad(full, foot_y);
    let v0 = vf * (1.0 - built);
    // the shadow of only what is built: from the foot up to the built height
    let top = |p: Pos2, foot: Pos2| foot + (p - foot) * built;
    let (tl, tr) = (top(q[0], q[3]), top(q[1], q[2]));
    let c = Color32::from_black_alpha(90);
    let mut m = Mesh::with_texture(tex);
    m.vertices.push(Vertex { pos: tl, uv: egui::pos2(0.0, v0), color: c });
    m.vertices.push(Vertex { pos: tr, uv: egui::pos2(1.0, v0), color: c });
    m.vertices.push(Vertex { pos: q[2], uv: egui::pos2(1.0, vf), color: c });
    m.vertices.push(Vertex { pos: q[3], uv: egui::pos2(0.0, vf), color: c });
    m.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    painter.add(Shape::mesh(m));
}

/// How far a terrain fade reaches into the next triangle at the edge point `flat` (world
/// space, so both sides of a shared point agree): `k` jittered by 0.55-1.45.
/// The light of each corner of the drawn window (x0, x1, y0, y1), worked out once a frame:
/// from column `x0`, `w` wide, from the window's first row.
struct Shades {
    window: (i32, i32, i32, i32),
    x0: i32,
    w: i32,
    v: Vec<Color32>,
}

impl Shades {
    fn get(&self, x: i32, y: i32) -> Option<Color32> {
        let (dx, dy) = (x - self.x0, y - self.window.2);
        (dx >= 0 && dx < self.w && dy >= 0).then(|| self.v.get((dy * self.w + dx) as usize).copied()).flatten()
    }
}

/// Which of two terrains reaches over the other along their edge: the higher.
fn edge_priority(t: u8) -> u8 {
    match t {
        5 | 6 | 19 => 1,
        3 => 2,
        4 | 7 => 3,
        0 | 14 => 4,
        8 | 9 | 10 | 15 => 5,
        18 | 34 => 6,
        1 | 11 | 12 | 13 => 7,
        2 => 8,
        _ => 9,
    }
}

/// The land reaching into triangle `i` of the six round a node (`World::terrains_around`)
/// at that corner: when it and both triangles beside it are water and land touches the node,
/// the nearest land triangle's terrain (its texture continues into the fan).
fn corner_land(around: &[u8; 6], i: usize) -> Option<u8> {
    use rr_world::terrain::is_water;
    if ![i, i + 1, i + 5].iter().all(|j| is_water(around[j % 6])) {
        return None;
    }
    [i + 2, i + 4, i + 3].iter().map(|j| around[j % 6]).find(|t| !is_water(*t))
}

fn edge_depth(flat: Vec2, k: f32) -> f32 {
    let (x, y) = (flat.x / 9.0, flat.y / 9.0);
    let (ix, iy) = (x.floor(), y.floor());
    let h = |a: f32, b: f32| {
        let mut v = (a as i32 as u32).wrapping_mul(374_761_393) ^ (b as i32 as u32).wrapping_mul(668_265_263);
        v = (v ^ (v >> 13)).wrapping_mul(1_274_126_177);
        ((v ^ (v >> 16)) & 0xffff) as f32 / 65535.0
    };
    let sm = |t: f32| t * t * (3.0 - 2.0 * t);
    let (fx, fy) = (sm(x - ix), sm(y - iy));
    let top = h(ix, iy) + (h(ix + 1.0, iy) - h(ix, iy)) * fx;
    let bottom = h(ix, iy + 1.0) + (h(ix + 1.0, iy + 1.0) - h(ix, iy + 1.0)) * fx;
    k * (0.55 + 0.9 * (top + (bottom - top) * fy))
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgba_premultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

/// Shallow water's tint (the terrain pass's shore colour; `fog` 0 dark .. 1 clear).
fn shallow(fog: f32) -> Color32 {
    let g = fog * 230.0;
    let c = |k: f32| (g * k).min(255.0) as u8;
    Color32::from_rgb(c(SHALLOW[0]), c(SHALLOW[1]), c(SHALLOW[2]))
}

/// The lift of shallow water over the deep (a soft turquoise, not a cyan glow).
const SHALLOW: [f32; 3] = [1.12, 1.3, 1.18];

fn soft_disc(mesh: &mut Mesh, c: Pos2, r: f32, alpha: u8) {
    let base = mesh.vertices.len() as u32;
    mesh.vertices.push(Vertex { pos: c, uv: egui::pos2(0.0, 0.0), color: Color32::from_white_alpha(alpha) });
    let steps = 10u32;
    for i in 0..steps {
        let a = i as f32 / steps as f32 * std::f32::consts::TAU;
        mesh.vertices.push(Vertex {
            pos: c + Vec2::new(a.cos() * r, a.sin() * r * 0.6),
            uv: egui::pos2(0.0, 0.0),
            color: Color32::TRANSPARENT,
        });
    }
    for i in 0..steps {
        mesh.indices.extend_from_slice(&[base, base + 1 + i, base + 1 + (i + 1) % steps]);
    }
}

/// One road segment from `a` to `b`: four strips across (soft edge, surface, surface, soft
/// edge), the texture running along.
fn road_strip(mesh: &mut Mesh, (a, b): (Pos2, Pos2), half: f32, (la, lb): (Color32, Color32), seed: u32) {
    let along = b - a;
    let len = along.length().max(0.001);
    let n = Vec2::new(-along.y, along.x) / len;
    let v_end = len / (half * 4.0);
    let cols = [(-1.0, 0.0, 0u8), (-0.62, 0.19, 255), (0.62, 0.81, 255), (1.0, 1.0, 0)];
    // in pieces, each side a little wider or narrower (the ends as wide as the joints)
    const PIECES: u32 = 4;
    let base = mesh.vertices.len() as u32;
    for i in 0..=PIECES {
        let t = i as f32 / PIECES as f32;
        let at = a + along * t;
        let light = lerp_color(la, lb, t);
        let wobble = |side: u32| {
            if i == 0 || i == PIECES {
                1.0
            } else {
                let mut h = seed ^ (i * 2 + side).wrapping_mul(2_654_435_761);
                h = (h ^ (h >> 15)).wrapping_mul(2_246_822_519);
                1.0 + (((h >> 8) & 0xff) as f32 / 255.0 - 0.5) * 0.3
            }
        };
        for (off, u, alpha) in cols {
            let w = if off < 0.0 { wobble(0) } else { wobble(1) };
            mesh.vertices.push(Vertex {
                pos: at + n * (off * half * w),
                uv: egui::pos2(u, v_end * t),
                color: lit(light, alpha),
            });
        }
    }
    for i in 0..PIECES {
        for c in 0..3u32 {
            let (j, k) = (base + i * 4 + c, base + (i + 1) * 4 + c);
            mesh.indices.extend_from_slice(&[j, k, k + 1, j, k + 1, j + 1]);
        }
    }
}

/// A round patch of road surface where segments meet (no notches at the bends).
fn road_joint(mesh: &mut Mesh, c: Pos2, half: f32, light: Color32) {
    let base = mesh.vertices.len() as u32;
    mesh.vertices.push(Vertex { pos: c, uv: egui::pos2(0.5, 0.5), color: lit(light, 255) });
    let steps = 10;
    for i in 0..steps {
        let a = i as f32 / steps as f32 * std::f32::consts::TAU;
        let d = Vec2::new(a.cos(), a.sin() * 0.85);
        mesh.vertices.push(Vertex {
            pos: c + d * (half * 0.62),
            uv: egui::pos2(0.5 + d.x * 0.31, 0.5 + d.y * 0.31),
            color: lit(light, 255),
        });
        mesh.vertices.push(Vertex {
            pos: c + d * half,
            uv: egui::pos2(0.5 + d.x * 0.5, 0.5 + d.y * 0.5),
            color: lit(light, 0),
        });
    }
    for i in 0..steps as u32 {
        let (a, b) = (base + 1 + i * 2, base + 1 + ((i + 1) % steps as u32) * 2);
        mesh.indices.extend_from_slice(&[base, a, b, a, a + 1, b + 1, a, b + 1, b]);
    }
}

/// How steep a road step is: 0 flat, 1 medium, 2 steep.
pub fn slope_class(slope: i32) -> u8 {
    match slope {
        0..=1 => 0,
        2..=3 => 1,
        _ => 2,
    }
}

/// A road step marker that reads without colour too: flat a small light dot, medium a larger
/// ring, steep a dark upward wedge (light to dark as well as green to red).
fn slope_mark(painter: &egui::Painter, m: Pos2, z: f32, slope: i32) {
    let dark = Color32::from_rgb(30, 30, 30);
    match slope_class(slope) {
        0 => {
            painter.circle_filled(m, 3.2 * z, dark);
            painter.circle_filled(m, 2.2 * z, Color32::from_rgb(150, 235, 150));
        }
        1 => {
            painter.circle_filled(m, 4.4 * z, dark);
            painter.circle_stroke(m, 3.0 * z, Stroke::new(1.8 * z, Color32::from_rgb(240, 190, 50)));
        }
        _ => {
            let pts = vec![m + Vec2::new(-4.5, 3.5) * z, m + Vec2::new(0.0, -4.5) * z, m + Vec2::new(4.5, 3.5) * z];
            painter.add(egui::Shape::convex_polygon(pts, Color32::from_rgb(150, 25, 25), Stroke::new(1.2 * z, dark)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CIELAB of an sRGB colour as seen with a colour vision `m` (Machado et al. 2009, severity 1).
    fn lab_seen(c: Color32, m: &[[f64; 3]; 3]) -> [f64; 3] {
        let lin = |v: u8| {
            let v = v as f64 / 255.0;
            if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        let l = [lin(c.r()), lin(c.g()), lin(c.b())];
        let s: Vec<f64> = m.iter().map(|r| (r[0] * l[0] + r[1] * l[1] + r[2] * l[2]).clamp(0.0, 1.0)).collect();
        let xyz = [
            (0.4124 * s[0] + 0.3576 * s[1] + 0.1805 * s[2]) / 0.95047,
            0.2126 * s[0] + 0.7152 * s[1] + 0.0722 * s[2],
            (0.0193 * s[0] + 0.1192 * s[1] + 0.9505 * s[2]) / 1.08883,
        ];
        let f = |t: f64| if t > 0.008856 { t.cbrt() } else { 7.787 * t + 16.0 / 116.0 };
        let (x, y, z) = (f(xyz[0]), f(xyz[1]), f(xyz[2]));
        [116.0 * y - 16.0, 500.0 * (x - y), 200.0 * (y - z)]
    }

    /// Art director round 3, #2: a notch in a straight coast is no river; a river is.
    /// New-player round 12, #6: every player's name reads on the dark panels.
    #[test]
    fn player_names_read_on_dark_panels() {
        for p in 0..8 {
            let c = player_text_color(p);
            let luma = c.r() as f32 * 0.299 + c.g() as f32 * 0.587 + c.b() as f32 * 0.114;
            assert!(luma >= 149.0, "player {p}: {c:?}");
        }
    }

    #[test]
    fn coast_notches_are_not_rivers() {
        use std::collections::BTreeSet;
        // land (meadow 8) above row 10, sea (5) from row 10 down; one water notch at row 9
        let notch: BTreeSet<(i32, i32, bool)> = [(20, 9, true)].into();
        // a stretch of river inland: two water triangles side by side, each with land around
        let sw = neighbor_unwrapped(30, 5, Direction::SW);
        let river: BTreeSet<(i32, i32, bool)> = [(30, 5, true), (sw.0, sw.1, false)].into();
        let terr = |x: i32, y: i32, r: bool| {
            if y >= 10 || notch.contains(&(x, y, r)) || river.contains(&(x, y, r)) { 5 } else { 8 }
        };
        assert!(!is_river_tri(&terr, 20, 9, true), "a notch");
        assert!(is_river_tri(&terr, 30, 5, true), "a river");
        // a notch in a stepped coast: the sea triangle under it touches land as well (the blue
        // teeth along every coast, graphics round 1)
        let sw = neighbor_unwrapped(20, 9, Direction::SW);
        let step = (sw.0 + 1, sw.1, true);
        let stepped = |x: i32, y: i32, r: bool| if (x, y, r) == step { 8 } else { terr(x, y, r) };
        assert!(!is_river_tri(&stepped, 20, 9, true), "a notch by a step");
    }

    /// Graphics round 1: a sea triangle touching land at one corner only gets the land of the
    /// nearest land triangle there; one with a land edge (the edge overlays cover it) or with
    /// no land at the node gets none.
    #[test]
    fn sea_corners_on_land_are_covered() {
        // land (meadow 8, 9) in triangles 2 and 3, sea elsewhere
        let around = [5, 5, 8, 9, 5, 5];
        assert_eq!(corner_land(&around, 0), Some(8), "two along: the nearer land");
        assert_eq!(corner_land(&around, 5), Some(9));
        assert_eq!(corner_land(&around, 1), None, "beside land: its edge is covered");
        assert_eq!(corner_land(&around, 4), None);
        assert_eq!(corner_land(&[5; 6], 0), None, "open sea");
        assert_eq!(corner_land(&around, 2), None, "land itself");
    }

    /// Graphics round 2: a road's sides waver along the way, but its ends are as wide as the
    /// joints, so the pieces meet; the same seed gives the same road every frame.
    #[test]
    fn roads_waver_but_meet_at_their_ends() {
        let (a, b) = (Pos2::new(0.0, 0.0), Pos2::new(100.0, 0.0));
        let strip = |seed| {
            let mut m = Mesh::default();
            road_strip(&mut m, (a, b), 6.0, (Color32::WHITE, Color32::WHITE), seed);
            m
        };
        let m = strip(7);
        // rows of four across the road: the first and the last span the full width
        let width = |row: usize| m.vertices[row * 4 + 3].pos.y - m.vertices[row * 4].pos.y;
        assert!((width(0).abs() - 12.0).abs() < 1e-3 && (width(4).abs() - 12.0).abs() < 1e-3);
        assert!((1..4).any(|r| (width(r).abs() - 12.0).abs() > 0.3), "wavers between");
        assert_eq!(
            m.vertices.iter().map(|v| v.pos).collect::<Vec<_>>(),
            strip(7).vertices.iter().map(|v| v.pos).collect::<Vec<_>>()
        );
    }

    /// Owner's playtest: two fighters on one node were drawn on the very same spot; they stand
    /// apart and face each other.
    /// A catapult stone leaves from the arm, flies high over the middle and lands on the target;
    /// its shadow runs along the ground (playtest 2026-10-27: no stone was seen to fly).
    #[test]
    fn a_stone_flies_in_an_arc() {
        let (a, b) = (Pos2::new(0.0, 100.0), Pos2::new(200.0, 100.0));
        let (start, _) = Shot::arc(a, b, 0.0, 1.0);
        assert!(start.y < a.y && (start.x - a.x).abs() < 0.01, "from the arm, above the catapult");
        let (top, shadow) = Shot::arc(a, b, 0.5, 1.0);
        assert!(top.y < start.y - 50.0, "high in the middle: {top:?}");
        assert_eq!(shadow, Pos2::new(100.0, 100.0));
        assert_eq!(Shot::arc(a, b, 1.0, 1.0).0, b, "lands on the target");
        let s = Shot { from: MapCoord::new(0, 0), to: MapCoord::new(3, 0), hit: true, gf: 100 };
        assert!(!s.done(140) && s.done(200));
    }

    #[test]
    fn fighters_face_each_other() {
        let ((a, a_west), (d, d_west)) = fight_stance(true, false);
        assert!(a < 0.0 && d > 0.0 && !a_west && d_west, "on one node: apart, face to face");
        let ((a, a_west), (d, d_west)) = fight_stance(false, true);
        assert!(a == 0.0 && d == 0.0 && a_west && !d_west, "apart: turned to each other");
    }

    /// Art director round 3, #1: borders stay readable zoomed out.
    #[test]
    fn border_ribbons_keep_a_minimum_width() {
        let (under, top) = ribbon_widths(0.25);
        assert!(top >= 2.2 && under > top, "{under} {top}");
        let (u, t) = ribbon_widths(1.0);
        assert!((u - 3.6).abs() < 1e-4 && (t - 2.4).abs() < 1e-4, "unchanged up close: {u} {t}");
    }

    /// a11y round 2, #3: the UI scale made the map bigger too (a quarter of the land at 2x); the
    /// map keeps its size on the screen, and its centre.
    #[test]
    fn ui_scale_leaves_the_map_alone() {
        let mut cam = Camera { offset: Vec2::new(300.0, 200.0), zoom: 1.0 };
        let physical = Vec2::new(1366.0, 700.0);
        let world_at = |c: &Camera, scale: f32, px: Vec2| c.offset + px / scale / c.zoom;
        let before = (world_at(&cam, 1.0, Vec2::ZERO), world_at(&cam, 1.0, physical));
        cam.ui_rescaled(1.0, 2.0);
        let after = (world_at(&cam, 2.0, Vec2::ZERO), world_at(&cam, 2.0, physical));
        assert!((before.0 - after.0).length() < 1e-3 && (before.1 - after.1).length() < 1e-3, "{before:?} {after:?}");
    }

    /// a11y round 2, #4: orders given in a pause show at once, as planned marks.
    #[test]
    fn paused_orders_are_marked() {
        use rr_sim::Cmd;
        let at = MapCoord::new(5, 5);
        let marks = planned_marks(&[
            Cmd::PlaceBuilding { at, kind: rr_rules::BuildingType::Farm },
            Cmd::PlaceFlag { at },
            Cmd::BuildRoad { from: at, dirs: vec![Direction::E] },
            Cmd::SetMilitary { values: [0; 8] },
        ]);
        assert_eq!(marks.len(), 3);
        assert!(matches!(marks[0], Planned::Site(p) if p == at));
        assert!(matches!(marks[2], Planned::Road(p, ref d) if p == at && d.len() == 1));
    }

    /// Art director round 4, #2: every map label reads at 4.5:1 on its plate.
    #[test]
    fn map_labels_read_on_their_plates() {
        let lum = |c: Color32| {
            let ch = |v: u8| {
                let v = v as f32 / 255.0;
                if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
            };
            0.2126 * ch(c.r()) + 0.7152 * ch(c.g()) + 0.0722 * ch(c.b())
        };
        // the plate over grass, as it is seen
        let a = PLATE.a() as f32 / 255.0;
        let mix = |p: u8, g: u8| (p as f32 + g as f32 * (1.0 - a)) as u8;
        let seen = Color32::from_rgb(mix(PLATE.r(), 84), mix(PLATE.g(), 116), mix(PLATE.b(), 49));
        for c in [
            prod_label(10).1,
            prod_label(50).1,
            prod_label(90).1,
            PLATE_TEXT,
            Color32::from_rgb(230, 230, 230),
            flag_label(5, 8).unwrap().1,
            flag_label(8, 8).unwrap().1,
        ] {
            let (x, y) = (lum(c), lum(seen));
            let ratio = (x.max(y) + 0.05) / (x.min(y) + 0.05);
            assert!(ratio >= 4.5, "{c:?}: {ratio:.2}");
        }
    }

    /// Genre lessons (PLAN §27, #4): a flag where wares pile up shows how many, amber from
    /// five, red and marked when full; quiet flags show nothing.
    #[test]
    fn crowded_flags_are_marked() {
        assert!(flag_label(4, 8).is_none());
        let (t5, c5) = flag_label(5, 8).unwrap();
        let (t8, c8) = flag_label(8, 8).unwrap();
        assert_eq!(t5, "5");
        assert!(t8.starts_with('!'), "full: marked, not only red");
        assert_ne!(c5, c8);
    }

    /// A11y round 3, #11: the three productivity colours stay apart under protanopia (a
    /// simulated red-blind view, Machado et al. 2009), at least as far as amber from red.
    #[test]
    fn productivity_colours_part_for_protan_eyes() {
        let protan = |c: Color32| {
            let (r, g, b) = (c.r() as f32, c.g() as f32, c.b() as f32);
            (0.152 * r + 1.053 * g - 0.205 * b, 0.115 * r + 0.786 * g + 0.099 * b, -0.004 * r - 0.048 * g + 1.052 * b)
        };
        let dist = |a: Color32, b: Color32| {
            let (x, y) = (protan(a), protan(b));
            ((x.0 - y.0).powi(2) + (x.1 - y.1).powi(2) + (x.2 - y.2).powi(2)).sqrt()
        };
        let (lo, mid, hi) = (prod_label(10).1, prod_label(50).1, prod_label(90).1);
        assert!(dist(mid, hi) > 60.0, "{}", dist(mid, hi));
        assert!(dist(lo, hi) > 60.0, "{}", dist(lo, hi));
    }

    #[test]
    fn low_productivity_is_marked_not_only_red() {
        assert!(prod_label(20).0.starts_with('!'));
        assert!(!prod_label(50).0.starts_with('!') && !prod_label(90).0.starts_with('!'));
    }

    #[test]
    fn slope_marks_have_three_classes() {
        assert_eq!([0, 1, 2, 3, 4, 9].map(slope_class), [0, 0, 1, 1, 2, 2]);
    }

    /// a11y roast #3: the six menu players can be told apart with normal vision and with deutan,
    /// protan or tritan colour blindness.
    #[test]
    fn player_colours_survive_colour_blindness() {
        let vision: [[[f64; 3]; 3]; 4] = [
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [[0.367322, 0.860646, -0.227968], [0.280085, 0.672501, 0.047413], [-0.011820, 0.042940, 0.968881]],
            [[0.152286, 1.052583, -0.204868], [0.114503, 0.786281, 0.099216], [-0.003882, -0.048116, 1.051998]],
            [[1.255528, -0.076749, -0.178779], [-0.078411, 0.930809, 0.147602], [0.004733, 0.691367, 0.303900]],
        ];
        for (k, m) in vision.iter().enumerate() {
            for i in 0..6 {
                for j in i + 1..6 {
                    let (a, b) = (lab_seen(PLAYER_COLORS[i], m), lab_seen(PLAYER_COLORS[j], m));
                    let d = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
                    // (28: the coral that replaced a near-white, which read as foam and snow;
                    // with normal sight 45: four reds and pinks were hard to tell apart in the
                    // statistics, item 304)
                    let need = if k == 0 { 45.0 } else { 28.0 };
                    assert!(d >= need, "players {i} and {j} differ by {d:.1} in vision {k}");
                }
            }
        }
    }

    /// a11y round 2, #2: players 7 and 8 too (7-player maps exist), and every colour stands out
    /// from grass and water in every vision by 18 (red on grass was 6.8 apart for deutan players).
    #[test]
    fn all_player_colours_stand_out() {
        let vision: [[[f64; 3]; 3]; 4] = [
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [[0.367322, 0.860646, -0.227968], [0.280085, 0.672501, 0.047413], [-0.011820, 0.042940, 0.968881]],
            [[0.152286, 1.052583, -0.204868], [0.114503, 0.786281, 0.099216], [-0.003882, -0.048116, 1.051998]],
            [[1.255528, -0.076749, -0.178779], [-0.078411, 0.930809, 0.147602], [0.004733, 0.691367, 0.303900]],
        ];
        let de =
            |a: [f64; 3], b: [f64; 3]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
        // grass and water as drawn, and the minimap's sea, snow and wasteland (art director
        // round 4, #4: its ultramarine sea was 20 from the player's blue)
        let mini = |l: rr_core::Landscape, t: u8| {
            let c = crate::ui::minimap_ground(rr_world::terrain::color(l, t));
            Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
        };
        let ground = [
            Color32::from_rgb(84, 116, 49),
            Color32::from_rgb(47, 92, 111),
            mini(rr_core::Landscape::Greenland, 5),
            mini(rr_core::Landscape::Winter, 8),
            mini(rr_core::Landscape::Wasteland, 8),
        ];
        let mut bad = Vec::new();
        for (k, m) in vision.iter().enumerate() {
            for i in 0..8 {
                for j in i + 1..8 {
                    let d = de(lab_seen(PLAYER_COLORS[i], m), lab_seen(PLAYER_COLORS[j], m));
                    if d < 18.0 {
                        bad.push(format!("players {i}/{j} vision {k}: {d:.1}"));
                    }
                }
                for (g, c) in ground.iter().enumerate() {
                    let d = de(lab_seen(PLAYER_COLORS[i], m), lab_seen(*c, m));
                    if d < 18.0 {
                        bad.push(format!("player {i} on ground {g} vision {k}: {d:.1}"));
                    }
                }
            }
        }
        assert!(bad.is_empty(), "{bad:#?}");
    }
    use rr_rules::BuildingType as B;

    /// Art director round 2, smaller note: border ribbons are calmer than the player colour
    /// (the green one glowed), but keep their hue.
    #[test]
    fn border_ribbons_are_muted() {
        let chroma = |c: Color32| c.r().max(c.g()).max(c.b()) as i32 - c.r().min(c.g()).min(c.b()) as i32;
        for c in PLAYER_COLORS {
            let m = ribbon_color(c);
            assert!(chroma(m) < chroma(c), "{c:?} -> {m:?}");
            // the strongest channel stays the strongest
            let top = |c: Color32| [c.r(), c.g(), c.b()].iter().enumerate().max_by_key(|x| x.1).unwrap().0;
            assert_eq!(top(m), top(c), "{c:?}");
        }
    }

    /// Art director round 2, #8: a field is a big patch of land use, not a small object.
    #[test]
    fn fields_are_big_patches() {
        let poly = field_patch(Pos2::new(100.0, 100.0), 1.0, Vec2::new(300.0, 200.0));
        let area =
            poly.iter().zip(poly.iter().cycle().skip(1)).map(|(a, b)| a.x * b.y - b.x * a.y).sum::<f32>().abs() / 2.0;
        assert!(area >= 1500.0, "{area} px²");
        let wide = field_patch(Pos2::new(100.0, 100.0), 2.0, Vec2::new(300.0, 200.0));
        assert!((wide[0] - Pos2::new(100.0, 100.0)).length() > (poly[0] - Pos2::new(100.0, 100.0)).length());
        // art director round 4, #5: not a regular octagon: 16 corners at radii of their own
        assert!(poly.len() >= 16);
        let k = |q: &Pos2| ((q.x - 100.0) / TW).hypot((q.y - 100.0) / TH);
        let (lo, hi) = poly.iter().map(k).fold((f32::MAX, 0.0f32), |(a, b), v| (a.min(v), b.max(v)));
        assert!(hi / lo >= 1.1, "radii {lo:.3}..{hi:.3}");
    }

    /// Art director round 2, #5: one sun: shadows fall to the lower right, a tall sprite's
    /// further than a low one's, and start at the foot.
    #[test]
    fn shadows_fall_to_the_lower_right() {
        let tall = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(40.0, 100.0));
        let low = Rect::from_min_max(Pos2::new(0.0, 70.0), Pos2::new(40.0, 100.0));
        let (q, vf) = shadow_quad(tall, 100.0);
        assert!(q[0].x > tall.min.x && q[1].x > tall.max.x, "to the right");
        assert!(q[0].y > tall.min.y, "flattened: below the sprite's top");
        assert_eq!((q[2].y, q[3].y), (100.0, 100.0), "from the foot");
        assert_eq!(vf, 1.0);
        let (ql, _) = shadow_quad(low, 100.0);
        assert!(q[0].x - tall.min.x > ql[0].x - low.min.x, "a tower casts further than a hut");
    }

    /// Art director round 2, #3: river triangles become chains (one line per river), smoothed.
    #[test]
    fn river_triangles_make_one_smooth_line() {
        // a zigzag of centroids (as a carved river has) and a separate pond far away
        let mut pts: Vec<Vec2> =
            (0..8).map(|i| Vec2::new(i as f32 * 30.0, if i % 2 == 0 { 0.0 } else { 18.0 })).collect();
        pts.swap(2, 5);
        pts.push(Vec2::new(900.0, 900.0));
        let cs = river_chains(&pts, 40.0);
        assert_eq!(cs.len(), 2, "{cs:?}");
        let long = cs.iter().find(|c| c.len() == 8).expect("one chain of eight");
        // in order along the river
        let xs: Vec<f32> = long.iter().map(|i| pts[*i].x).collect();
        assert!(xs.windows(2).all(|w| w[1] > w[0]) || xs.windows(2).all(|w| w[1] < w[0]), "{xs:?}");
        let line: Vec<Vec2> = long.iter().map(|i| pts[*i]).collect();
        let smooth = chaikin(&chaikin(&line));
        assert_eq!(smooth.first(), line.first(), "the ends stay put");
        assert!(smooth.len() > line.len() * 3);
    }

    /// Art director round 2, #2: terrain edges are ragged, not straight fades: the depth varies
    /// along an edge and is the same for both quads at a shared point.
    #[test]
    fn terrain_edges_are_ragged() {
        let k = 0.4;
        let along: Vec<f32> = (0..60).map(|i| edge_depth(Vec2::new(i as f32 * 4.0, 100.0), k)).collect();
        let (lo, hi) = along.iter().fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
        assert!(hi - lo > 0.3 * k, "depth {lo:.2}..{hi:.2}");
        assert!(lo >= 0.5 * k && hi <= 1.5 * k);
        assert_eq!(edge_depth(Vec2::new(33.0, 7.0), k), edge_depth(Vec2::new(33.0, 7.0), k));
    }

    /// New-player round 7, #3: a flag right by a building's picture is what a click on it
    /// means, not the building.
    #[test]
    fn a_click_on_a_flag_picks_the_flag() {
        let mut m = rr_core::MapData::flat(64, 64, 8);
        m.hq = vec![Some(MapCoord::new(20, 20))];
        let setup = rr_sim::GameSetup { seed: 1, players: vec![Default::default()], ..Default::default() };
        let mut sim = Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &m, &setup).unwrap();
        let hut = MapCoord::new(26, 20);
        let g = sim.world().grid();
        let cmd = |cmd| rr_sim::PlayerCommand { player: 0, cmd };
        sim.step(&[cmd(rr_sim::Cmd::PlaceBuilding { at: hut, kind: rr_rules::BuildingType::Woodcutter })]);
        let flag = [Direction::NE, Direction::NW]
            .into_iter()
            .map(|d| g.neighbor(hut, d))
            .find(|p| {
                sim.step(&[cmd(rr_sim::Cmd::PlaceFlag { at: *p })]);
                sim.flag_at(*p).is_some()
            })
            .expect("a flag fits north of the hut");
        let view = View {
            sim: &sim,
            cam: Camera { offset: Vec2::new(600.0, 400.0), zoom: 1.0 },
            rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0)),
            render_gf: 0.0,
            player: 0,
            show_bq: false,
            show_prod: false,
            show_names: false,
            selected: None,
            road_preview: None,
            planned: Vec::new(),
            work_area: None,
            time: 0.0,
            fog_of_war: false,
            idle: None,
            reveal_all: true,
            scenery: true,
            fog_cache: Default::default(),
        };
        assert_eq!(view.pick_click(view.node_screen(flag)), flag);
        // the hut's own picture still picks the hut
        assert_eq!(view.pick_click(view.node_screen(hut) - Vec2::new(0.0, 25.0)), hut);
        // a11y round 2, #8: zoomed out, a click within 14 px of a flag still means the flag,
        // though another node lies nearer (the pull was 5.9 px at zoom 0.35)
        let far = View { cam: Camera { offset: Vec2::new(600.0, 400.0), zoom: 0.35 }, ..view };
        let off = far.node_screen(flag) + Vec2::new(6.0, 6.0);
        assert_ne!(far.pick(off), flag, "the test needs another node nearer");
        assert_eq!(far.pick_click(off), flag);
    }

    /// New-player round 6, #2: a click on a building's picture selects the building, not the
    /// ground behind it; an empty spot is still the nearest node.
    #[test]
    fn a_click_on_the_picture_picks_the_building() {
        let mut m = rr_core::MapData::flat(64, 64, 8);
        let hq = MapCoord::new(20, 20);
        m.hq = vec![Some(hq)];
        let setup = rr_sim::GameSetup { seed: 1, players: vec![Default::default()], ..Default::default() };
        let sim = Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &m, &setup).unwrap();
        let view = View {
            sim: &sim,
            cam: Camera { offset: Vec2::new(600.0, 400.0), zoom: 1.0 },
            rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0)),
            render_gf: 0.0,
            player: 0,
            show_bq: false,
            show_prod: false,
            show_names: false,
            selected: None,
            road_preview: None,
            planned: Vec::new(),
            work_area: None,
            time: 0.0,
            fog_of_war: false,
            idle: None,
            reveal_all: true,
            scenery: true,
            fog_cache: Default::default(),
        };
        let at = view.node_screen(hq);
        let above = at - Vec2::new(0.0, 30.0);
        assert_ne!(view.pick(above), hq, "the nearest node lies behind the castle");
        assert_eq!(view.pick_click(above), hq);
        // far from any building: the nearest node, as before
        let empty = view.node_screen(MapCoord::new(30, 30));
        assert_eq!(view.pick_click(empty), MapCoord::new(30, 30));
    }

    /// New-player round 4, #7: zooming keeps the point under the screen centre in place.
    #[test]
    fn zoom_keeps_the_middle() {
        let mut cam = Camera { offset: Vec2::new(300.0, 200.0), zoom: 1.0 };
        let mid = Vec2::new(640.0, 380.0);
        let world = cam.offset + mid / cam.zoom;
        cam.zoom_at(1.25, mid);
        assert!((cam.offset + mid / cam.zoom - world).length() < 0.01);
        assert!((cam.zoom - 1.25).abs() < 1e-6);
    }

    /// New-player roast #8: the medium workshops share one house shape; a sign with the
    /// product tells them apart (until the art pack has its own pictures).
    #[test]
    fn look_alike_workshops_carry_a_sign() {
        let rules = rr_rules::Rules::classic();
        let mut seen = Vec::new();
        for b in B::ALL {
            let d = rules.building(*b);
            if d.size == rr_rules::Size::House
                && d.worker.is_some()
                && !d.is_military()
                && !matches!(b, B::Mill | B::Catapult)
            {
                let w = sign_ware(*b).unwrap_or_else(|| panic!("{b:?} has no sign"));
                assert!(!seen.contains(&w), "{b:?} shares its sign");
                seen.push(w);
            }
        }
        assert!(seen.len() >= 6);
    }
}

#[cfg(test)]
mod draw_bench {
    use super::*;

    /// How long `View::draw` takes on a late 256² game, at the usual zoom and zoomed out
    /// (perf round 3, #6). `cargo test --release -p rr-app --bin rr-app -- --ignored draw_bench
    /// --nocapture`; RR_BENCH_PART=terrain|edges|fields|clouds draws only that part.
    #[test]
    #[ignore = "a benchmark: minutes of game first"]
    fn draw_bench() {
        let map = rr_mapgen::generate(&rr_mapgen::GenParams {
            width: 256,
            height: 256,
            players: 7,
            seed: 1,
            ..Default::default()
        });
        let setup = rr_sim::GameSetup { seed: 1, players: vec![Default::default(); 7], ..Default::default() };
        let mut sim = Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &map, &setup).unwrap();
        let mut ais: Vec<rr_ai::Ai> = (0..7).map(rr_ai::Ai::new).collect();
        let frames: u32 = std::env::var("RR_BENCH_GF").ok().and_then(|v| v.parse().ok()).unwrap_or(60_000);
        for _ in 0..frames {
            let mut cmds = Vec::new();
            for ai in ais.iter_mut() {
                cmds.extend(ai.think(&sim));
            }
            sim.step(&cmds);
        }
        let ctx = egui::Context::default();
        let tex = TerrainTextures::new(&ctx, sim.world().landscape);
        let home = sim.building(sim.players()[0].hq.unwrap()).pos;
        let _ = ctx.run(Default::default(), |ctx| {
            let painter = ctx.layer_painter(egui::LayerId::background());
            for zoom in [1.35, 0.35] {
                let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1920.0, 1050.0));
                let mut cam = Camera { offset: Vec2::ZERO, zoom };
                cam.center_on(&sim, home, rect.size());
                let view = View {
                    sim: &sim,
                    cam,
                    rect,
                    render_gf: sim.gf().0 as f32,
                    player: 0,
                    show_bq: false,
                    show_prod: false,
                    show_names: false,
                    selected: None,
                    road_preview: None,
                    planned: Vec::new(),
                    work_area: None,
                    time: 1.0,
                    fog_of_war: true,
                    idle: None,
                    reveal_all: false,
                    scenery: true,
                    fog_cache: Default::default(),
                };
                let mut times = Vec::new();
                for _ in 0..20 {
                    let t = std::time::Instant::now();
                    view.draw(&painter, &tex);
                    times.push(t.elapsed().as_secs_f64() * 1000.0);
                }
                times.sort_by(f64::total_cmp);
                eprintln!("zoom {zoom}: draw median {:.2} ms, best {:.2} ms", times[10], times[0]);
                let vis = view.visible();
                let part = |name: &str, f: &dyn Fn()| {
                    let t = std::time::Instant::now();
                    for _ in 0..10 {
                        f();
                    }
                    eprintln!("  {name}: {:.2} ms", t.elapsed().as_secs_f64() * 100.0);
                };
                let none = Shades { window: vis, x0: 0, w: 0, v: Vec::new() };
                part("edges", &|| view.draw_edges(&painter, &tex, &none));
                part("fields", &|| view.draw_fields(&painter, &tex, vis));
                part("clouds", &|| view.draw_clouds(&painter, &tex, vis));
            }
        });
    }
}
