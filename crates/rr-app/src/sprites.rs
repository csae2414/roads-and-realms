//! Placeholder vector sprites (open pack v0): drawn from simple shapes so the game is fully
//! playable without any original files. Proportions follow the original's footprint sizes.

use crate::animals;
use eframe::egui::{Color32, Painter, Pos2, Shape, Stroke, pos2, vec2};
use rr_core::Animal;
use rr_rules::{BuildingType as B, Job, Size, Ware};
use rr_world::Bq;

pub enum Sprite {
    Tree {
        species: u8,
        stage: u8,
        seed: u32,
        /// 0 greenland, 1 wasteland, 2 winter.
        look: u8,
    },
    Granite {
        size: u8,
        kind: u8,
    },
    Decoration(u8),
    /// A small pre-rendered prop (bush, flowers, stump, reeds, ...; draw only) at a scale.
    Prop(&'static str, f32),
    Field(u8),
    Fire,
    Sign(u8),
    Flag {
        color: Color32,
        wares: Vec<Ware>,
    },
    Building {
        kind: B,
        color: Color32,
        progress: Option<u32>,
        working: bool,
        soldiers: u8,
        nation: u8,
    },
    Figure {
        job: Job,
        color: Color32,
        carry: Option<Ware>,
        walking: Option<rr_core::Direction>,
        /// Turned to the west without walking (a fighter facing a foe on its left).
        face_west: bool,
        working: bool,
        anim: u32,
        hp: u8,
    },
    /// A holy place of the Roman campaign (`rr_world::HOLY_PLACES`), and whether the viewer
    /// holds it (then birds of omen circle over it).
    Holy {
        god: u8,
        mine: bool,
    },
    /// A carrier idle on its road for a while, in one of its idle motions (`idle::Pose`),
    /// `t` seconds into it.
    Idler {
        color: Color32,
        pose: crate::idle::Pose,
        t: f32,
        west: bool,
    },
    Animal {
        species: Animal,
        walking: bool,
        /// Walking west: the body turned round (owner's notes: animals always faced east).
        west: bool,
        anim: u32,
    },
    Donkey {
        carry: Option<Ware>,
        walking: bool,
        west: bool,
        anim: u32,
    },
    ShipSite(u8),
    /// A carrier's rowing boat.
    Boat,
    Ship {
        color: Color32,
        loaded: bool,
        dir: Option<rr_core::Direction>,
    },
}

pub fn ware_color(w: Ware) -> Color32 {
    use Ware::*;
    match w {
        Wood => Color32::from_rgb(140, 90, 40),
        Boards => Color32::from_rgb(214, 170, 100),
        Stones => Color32::from_rgb(160, 160, 160),
        Coal => Color32::from_rgb(30, 30, 30),
        IronOre => Color32::from_rgb(150, 90, 70),
        Iron => Color32::from_rgb(110, 120, 140),
        Gold => Color32::from_rgb(250, 210, 40),
        Coins => Color32::from_rgb(255, 230, 90),
        Grain => Color32::from_rgb(230, 200, 90),
        Flour => Color32::from_rgb(245, 240, 225),
        Bread => Color32::from_rgb(200, 140, 60),
        Fish => Color32::from_rgb(120, 160, 200),
        Meat | Ham => Color32::from_rgb(200, 80, 80),
        Water => Color32::from_rgb(60, 120, 230),
        Beer => Color32::from_rgb(220, 160, 40),
        Sword => Color32::from_rgb(200, 210, 230),
        Shield => Color32::from_rgb(150, 60, 40),
        Boat => Color32::from_rgb(120, 80, 50),
        _ => Color32::from_rgb(120, 110, 100),
    }
}

fn dark(c: Color32, k: f32) -> Color32 {
    Color32::from_rgb((c.r() as f32 * k) as u8, (c.g() as f32 * k) as u8, (c.b() as f32 * k) as u8)
}

fn grad_tri(p: &Painter, pts: [Pos2; 3], cols: [Color32; 3]) {
    let mut m = eframe::egui::Mesh::default();
    for (pt, c) in pts.iter().zip(cols) {
        m.colored_vertex(*pt, c);
    }
    m.add_triangle(0, 1, 2);
    p.add(Shape::mesh(m));
}

/// A quad with one colour per corner (smooth shading).
fn grad_quad(p: &Painter, pts: [Pos2; 4], cols: [Color32; 4]) {
    let mut m = eframe::egui::Mesh::default();
    for (pt, c) in pts.iter().zip(cols) {
        m.colored_vertex(*pt, c);
    }
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    p.add(Shape::mesh(m));
}

fn poly(p: &Painter, pts: Vec<Pos2>, fill: Color32, stroke: Color32) {
    p.add(Shape::convex_polygon(pts, fill, Stroke::new(1.0_f32, stroke)));
}

/// The pre-rendered tree pictures (`tools/art/trees.py`).
pub const TREE_KINDS: [&str; 8] = ["oak", "beech", "birch", "pine", "olive", "drypine", "frost", "snowfir"];

/// Which picture a tree gets: by its species and the landscape's look (0 green, 1 wasteland,
/// 2 winter); conifers as the vector trees have them.
pub fn tree_kind(species: u8, look: u8) -> &'static str {
    match (look, species % 9) {
        (1, 0 | 3) => "drypine",
        (1, _) => "olive",
        (2, 2 | 5 | 7) => "frost",
        (2, _) => "snowfir",
        (_, 0 | 3) => "pine",
        (_, 1 | 4) => "beech",
        (_, 2 | 6) => "birch",
        _ => "oak",
    }
}

/// How tall a tree of this growth stage stands, of a grown one.
pub fn tree_growth(stage: u8) -> f32 {
    [0.35, 0.55, 0.78, 1.0][stage.min(3) as usize]
}

/// The loose boulders drawn on bare mountain rock (render.rs): grey, as the rock round them.
pub const MOUNTAIN_BOULDER: u8 = 0x16;

/// A decoration's boulder, lit from the upper left with a shadow to the lower right. On the
/// mountains it is grey; elsewhere it is a smaller, earth-brown stone, so it never reads as the
/// grey quarry rocks that block building (owner's playtest 2026-10-27).
fn boulder(p: &Painter, at: Pos2, z: f32, grey: bool) {
    let k = if grey { 1.0 } else { 0.7 };
    let v = |x: f32, y: f32| at + vec2(x * k, y * k) * z;
    let (body, edge, lit) = if grey {
        (Color32::from_rgb(128, 124, 118), Color32::from_rgb(70, 68, 64), Color32::from_rgb(168, 164, 156))
    } else {
        (Color32::from_rgb(140, 116, 84), Color32::from_rgb(84, 66, 46), Color32::from_rgb(176, 150, 112))
    };
    p.add(Shape::ellipse_filled(v(2.0, 0.5), vec2(7.0 * k * z, 2.0 * k * z), Color32::from_black_alpha(55)));
    poly(p, vec![v(-6.5, 0.0), v(-5.0, -5.5), v(-1.0, -8.0), v(4.5, -6.5), v(6.5, 0.0)], body, edge);
    poly(p, vec![v(-5.0, -5.5), v(-1.0, -8.0), v(3.0, -6.8), v(-0.5, -4.0)], lit, Color32::TRANSPARENT);
}

#[allow(clippy::too_many_lines)] // to split (architect round 2, #8: no new long ones)
pub fn draw(p: &Painter, at: Pos2, z: f32, sp: &Sprite, time: f32) {
    let v = |x: f32, y: f32| at + vec2(x * z, y * z);
    match sp {
        // pictures only (drawn by the renderer); far away nothing
        Sprite::Prop(..) => {}
        Sprite::Tree { species, stage, seed, look } => {
            let s = [0.35, 0.55, 0.78, 1.0][(*stage).min(3) as usize];
            let sway = ((time * 1.3 + *seed as f32 * 0.37).sin()) * 1.2 * s;
            let trunk = Color32::from_rgb(95, 62, 30);
            // shadow falls to the lower right
            p.add(Shape::ellipse_filled(
                v(6.0 * s, 1.0),
                vec2(10.0 * z * s, 3.0 * z * s),
                Color32::from_black_alpha(55),
            ));
            p.add(Shape::line_segment(
                [v(0.0, 0.0), v(sway * 0.5, -14.0 * s)],
                Stroke::new(3.0 * z * s.max(0.5), trunk),
            ));
            p.add(Shape::line_segment(
                [v(0.8 * s, 0.0), v(sway * 0.5 + 0.8 * s, -12.0 * s)],
                Stroke::new(1.0 * z * s.max(0.5), dark(trunk, 0.7)),
            ));
            let (green, conifer) = match (*look, species % 9) {
                // wasteland: dry olive and rusty crowns
                (1, 0 | 3) => (Color32::from_rgb(70, 78, 40), true),
                (1, 1 | 4 | 6) => (Color32::from_rgb(110, 98, 48), false),
                (1, _) => (Color32::from_rgb(92, 84, 44), false),
                // winter: dark firs, frosted crowns
                (2, 2 | 5 | 7) => (Color32::from_rgb(120, 130, 120), false),
                (2, _) => (Color32::from_rgb(34, 70, 52), true),
                (_, 0 | 3) => (Color32::from_rgb(40, 100, 40), true),
                (_, 1 | 4) => (Color32::from_rgb(70, 130, 40), false),
                (_, 2) => (Color32::from_rgb(90, 140, 50), false),
                (_, 5) => (Color32::from_rgb(50, 120, 60), false),
                _ => (Color32::from_rgb(110, 150, 60), false),
            };
            // far away a tree is a few pixels: a few shapes instead of 16 (zoomed-out views hold
            // thousands of trees, and every shape is tessellated each frame)
            if z * s < 0.55 {
                if conifer {
                    p.add(Shape::convex_polygon(
                        vec![v(-10.0 * s + sway, -4.0 * s), v(sway * 1.3, -35.0 * s), v(10.0 * s + sway, -4.0 * s)],
                        dark(green, 0.9),
                        Stroke::NONE,
                    ));
                } else {
                    let c = v(sway, -22.0 * s);
                    p.circle_filled(c + vec2(0.0, 1.5 * z * s), 8.5 * z * s, dark(green, 0.85));
                    p.circle_filled(c + vec2(-1.5 * z * s, -2.5 * z * s), 5.5 * z * s, dark(green, 1.1));
                }
                return;
            }
            if conifer {
                for (i, w) in [(0.0, 10.0), (1.0, 8.0), (2.0, 5.5)] {
                    let y = -10.0 * s - i * 8.0 * s;
                    let (l, tip, r) =
                        (v(-w * s + sway, y + 6.0 * s), v(sway * 1.3, y - 9.0 * s), v(w * s + sway, y + 6.0 * s));
                    let mid = v(sway * 1.1, y + 7.0 * s);
                    let base = dark(green, 1.0 - i * 0.04);
                    grad_tri(p, [l, tip, mid], [dark(base, 1.15), dark(base, 1.35), dark(base, 0.95)]);
                    grad_tri(p, [mid, tip, r], [dark(base, 0.8), dark(base, 1.0), dark(base, 0.62)]);
                    if *look == 2 {
                        // snow on the upper side of each layer
                        let snow = Color32::from_rgb(235, 240, 248);
                        p.add(Shape::convex_polygon(
                            vec![
                                v(sway * 1.3, y - 9.0 * s),
                                v(-w * s * 0.55 + sway, y),
                                v(w * s * 0.2 + sway, y - 2.0 * s),
                            ],
                            snow,
                            Stroke::NONE,
                        ));
                    }
                }
            } else {
                let c = v(sway, -22.0 * s);
                let r = 8.5 * z * s;
                let clumps = [
                    (4.0, 3.0, 0.72),
                    (-4.5, 3.5, 0.82),
                    (0.0, 1.0, 0.92),
                    (-2.5, -2.5, 1.05),
                    (2.0, -3.5, 1.0),
                    (-1.0, -5.5, 1.18),
                ];
                for (dx, dy, k) in clumps {
                    p.circle_filled(c + vec2(dx * z * s, dy * z * s), r * 0.72, dark(green, k));
                }
                // leaf speckles, fixed per tree
                let mut h = seed.wrapping_mul(2654435761);
                for _ in 0..7 {
                    h = h.wrapping_mul(1103515245).wrapping_add(12345);
                    let dx = ((h >> 8) % 13) as f32 - 6.0;
                    let dy = ((h >> 16) % 11) as f32 - 6.0;
                    let k = if dx + dy < 0.0 { 1.35 } else { 0.7 };
                    p.circle_filled(c + vec2(dx * z * s, dy * z * s), 1.4 * z * s, dark(green, k));
                }
            }
        }
        Sprite::Granite { size, kind } => {
            let n = (*size).clamp(1, 6) as f32;
            let base = if *kind == 0 { Color32::from_rgb(150, 150, 145) } else { Color32::from_rgb(170, 160, 140) };
            for i in 0..(*size).clamp(1, 6) {
                let ox = ((i as f32 * 2.3).sin()) * 6.0 * (n / 6.0 + 0.4);
                let oy = -(i as f32) * 2.2;
                let r = 5.0 + n * 0.8 - i as f32 * 0.5;
                poly(
                    p,
                    vec![
                        v(ox - r, oy),
                        v(ox - r * 0.6, oy - r * 1.2),
                        v(ox + r * 0.5, oy - r * 1.3),
                        v(ox + r, oy - r * 0.2),
                        v(ox + r * 0.3, oy + 1.0),
                    ],
                    dark(base, 1.0 - i as f32 * 0.04),
                    dark(base, 0.55),
                );
            }
        }
        Sprite::Decoration(id) => decoration(p, at, z, *id),
        Sprite::Field(stage) => {
            // the patch itself is drawn with the terrain (render.rs draw_fields); here the
            // plants standing on it, in rows across it
            for row in 0..5 {
                let t = (row as f32 + 0.5) / 5.0;
                let (a, b) = (v(-30.0 + 18.0 * t, 8.0 - 18.0 * t), v(14.0 + 18.0 * t, 8.0 - 18.0 * t));
                let (col, h) = match stage {
                    0 => continue,
                    1 => (Color32::from_rgb(110, 170, 60), 1.4),
                    2 => (Color32::from_rgb(120, 175, 55), 3.2),
                    3 => (Color32::from_rgb(226, 190, 74), 4.6),
                    _ => (Color32::from_rgb(170, 140, 80), 1.0),
                };
                for k in 0..10 {
                    let q = a + (b - a) * (k as f32 + 0.5) / 10.0;
                    p.line_segment([q, q + vec2(0.4 * z, -h * z)], Stroke::new(0.9 * z, col));
                    if *stage == 3 {
                        p.add(Shape::ellipse_filled(q + vec2(0.5 * z, -h * z), vec2(0.6 * z, 1.1 * z), dark(col, 1.1)));
                    }
                }
            }
        }
        Sprite::Fire => {
            // a burning ruin, not a sign (art director round 3, #5): a scorch mark, charred
            // beams and rubble, flickering tongues of flame and dark smoke rising (house-sized)
            let v = |x: f32, y: f32| v(x * 1.4, y * 1.4);
            let z = z * 1.4;
            p.add(Shape::ellipse_filled(v(0.0, -1.0), vec2(17.0 * z, 7.0 * z), Color32::from_black_alpha(90)));
            for (x0, y0, x1, y1) in [(-11.0, -2.0, 4.0, -14.0), (9.0, -1.0, -3.0, -16.0), (-6.0, -3.0, 10.0, -6.0)] {
                p.line_segment([v(x0, y0), v(x1, y1)], Stroke::new(2.4 * z, Color32::from_rgb(38, 28, 22)));
            }
            for (x, y, r) in [(-8.0, -2.0, 3.0), (6.0, -1.5, 2.6), (0.0, -3.0, 3.4), (-2.0, 0.0, 2.2)] {
                p.add(Shape::ellipse_filled(v(x, y), vec2(r * z, r * 0.6 * z), Color32::from_rgb(70, 62, 56)));
            }
            for (k, x) in [-5.0f32, 1.0, 6.0].into_iter().enumerate() {
                let t = time * 7.0 + k as f32 * 2.1;
                let h = 13.0 + 4.0 * t.sin() + 2.0 * (t * 2.3).cos();
                let sway = 1.5 * (t * 0.7).sin();
                poly(
                    p,
                    vec![v(x - 4.0, -3.0), v(x + sway, -3.0 - h), v(x + 4.0, -3.0)],
                    Color32::from_rgba_unmultiplied(235, 110, 25, 220),
                    Color32::from_rgba_unmultiplied(150, 40, 10, 120),
                );
                poly(
                    p,
                    vec![v(x - 2.0, -3.0), v(x + sway * 0.6, -3.0 - h * 0.55), v(x + 2.0, -3.0)],
                    Color32::from_rgba_unmultiplied(255, 215, 90, 230),
                    Color32::TRANSPARENT,
                );
            }
            for k in 0..4 {
                let age = (time * 0.45 + k as f32 * 0.25).fract();
                let c = v(2.0 + age * 9.0 + (k as f32 * 1.7).sin() * 3.0, -18.0 - age * 34.0);
                let a = ((1.0 - age) * 160.0) as u8;
                p.circle_filled(c, (3.0 + age * 7.0) * z, Color32::from_rgba_unmultiplied(55, 50, 48, a));
            }
        }
        Sprite::Sign(code) => {
            p.line_segment([v(0.0, 0.0), v(0.0, -12.0)], Stroke::new(1.5 * z, Color32::from_rgb(100, 70, 40)));
            let c = match code >> 4 {
                1 => Color32::from_rgb(20, 20, 20),
                2 => Color32::from_rgb(170, 90, 60),
                3 => Color32::from_rgb(250, 210, 40),
                4 => Color32::from_rgb(170, 170, 170),
                5 => Color32::from_rgb(60, 120, 230),
                _ => Color32::from_rgb(240, 240, 240),
            };
            let amount = (code & 0x0f) as f32;
            let w = if code >> 4 == 0 { 4.0 } else { 3.0 + amount * 0.6 };
            p.rect_filled(eframe::egui::Rect::from_min_size(v(0.0, -13.0), vec2(w * z, 5.0 * z)), 0.0, c);
        }
        Sprite::Flag { color, wares } => {
            for (i, w) in wares.iter().enumerate() {
                let (ox, oy) = [
                    (-7.0, 2.0),
                    (-3.0, 4.0),
                    (2.0, 4.0),
                    (6.0, 2.0),
                    (-8.0, -2.0),
                    (7.0, -2.0),
                    (-5.0, 6.0),
                    (4.0, 7.0),
                ][i % 8];
                // the ware's own picture, so a pile can be read
                p.add(Shape::ellipse_filled(
                    v(ox + 0.6, oy + 1.4),
                    vec2(2.6 * z, 0.9 * z),
                    Color32::from_black_alpha(50),
                ));
                p.circle_filled(v(ox, oy - 1.0), 3.6 * z, Color32::from_black_alpha(70));
                ware_icon(p, v(ox, oy - 1.0), 6.5 * z, *w);
            }
            // pole with a knob, a cloth that waves in two folds, a shadow
            p.add(Shape::ellipse_filled(v(2.0, 0.4), vec2(3.0 * z, 1.0 * z), Color32::from_black_alpha(55)));
            p.line_segment([v(0.0, 0.0), v(0.0, -19.0)], Stroke::new(1.7 * z, Color32::from_rgb(70, 50, 30)));
            p.line_segment([v(-0.4, -1.0), v(-0.4, -18.0)], Stroke::new(0.6 * z, Color32::from_rgb(120, 92, 60)));
            p.circle_filled(v(0.0, -19.4), 1.0 * z, Color32::from_rgb(220, 190, 90));
            let (w1, w2) = ((time * 4.0).sin() * 1.2, (time * 4.0 - 1.3).sin() * 1.6);
            grad_quad(
                p,
                [v(0.0, -18.5), v(5.0, -18.0 + w1), v(5.0, -12.5 + w1), v(0.0, -12.5)],
                [dark(*color, 1.15), dark(*color, 0.95), dark(*color, 0.8), dark(*color, 1.0)],
            );
            grad_quad(
                p,
                [v(5.0, -18.0 + w1), v(10.0, -17.0 + w2), v(10.0, -13.0 + w2), v(5.0, -12.5 + w1)],
                [dark(*color, 0.95), dark(*color, 1.15), dark(*color, 0.9), dark(*color, 0.8)],
            );
        }
        Sprite::Building { kind, color, progress, working, soldiers, nation } => {
            building(p, at, z, *kind, *color, *progress, *working, *soldiers, *nation, time)
        }
        Sprite::Figure { job, color, carry, walking, face_west, working, anim, hp } => {
            figure(p, at, z, *job, *color, *carry, (*walking, *face_west), *working, *anim, *hp)
        }
        Sprite::Idler { color, pose, t, west } => crate::idle::draw(p, at, z, *color, *pose, *t, *west),
        Sprite::Holy { .. } => altar(p, at, z),
        Sprite::Boat => boat(p, at, z),
        Sprite::ShipSite(step) => {
            // ribs first, then planking as the build steps progress
            let k = (*step as f32 / 9.0).clamp(0.0, 1.0);
            let wood = Color32::from_rgb(140, 95, 50);
            p.line_segment([v(-16.0, -2.0), v(16.0, -2.0)], Stroke::new(2.5 * z, dark(wood, 0.8)));
            for i in 0..7 {
                let x = -13.0 + i as f32 * 4.3;
                p.line_segment([v(x, -2.0), v(x * 1.15, -11.0)], Stroke::new(1.2 * z, wood));
            }
            if k > 0.4 {
                let h = 9.0 * ((k - 0.4) / 0.6);
                grad_quad(
                    p,
                    [v(-15.0, -2.0), v(-17.0, -2.0 - h), v(17.0, -2.0 - h), v(15.0, -2.0)],
                    [dark(wood, 0.9), dark(wood, 1.1), dark(wood, 0.95), dark(wood, 0.75)],
                );
            }
        }
        Sprite::Ship { color, loaded, dir } => {
            let flip = matches!(dir, Some(rr_core::Direction::W | rr_core::Direction::NW | rr_core::Direction::SW));
            let f = if flip { -1.0 } else { 1.0 };
            // a ship is bigger than a hut: drawn 1.6 times
            let z = z * 1.6;
            let v = |x: f32, y: f32| at + vec2(x * z, y * z);
            let wood = Color32::from_rgb(120, 78, 40);
            let bob = (time * 2.0).sin() * 0.8;
            // a dark shadow under the hull and a V of wake behind the stern
            p.add(Shape::ellipse_filled(v(0.0, 1.5), vec2(16.0 * z, 3.0 * z), Color32::from_black_alpha(50)));
            for side in [-1.0, 1.0] {
                p.line_segment(
                    [v(-14.0 * f, 0.5), v(-28.0 * f, 0.5 + side * 4.0)],
                    Stroke::new(0.5 * z, Color32::from_white_alpha(55)),
                );
            }
            grad_quad(
                p,
                [v(-18.0 * f, -6.0 + bob), v(20.0 * f, -6.0 + bob), v(13.0 * f, 1.0 + bob), v(-13.0 * f, 1.0 + bob)],
                [dark(wood, 1.1), dark(wood, 0.9), dark(wood, 0.7), dark(wood, 0.85)],
            );
            p.line_segment([v(0.0, -6.0 + bob), v(0.0, -34.0 + bob)], Stroke::new(1.6 * z, dark(wood, 0.7)));
            p.add(Shape::convex_polygon(
                vec![v(0.0, -34.0 + bob), v(6.0 * f, -32.5 + bob), v(0.0, -31.0 + bob)],
                *color,
                Stroke::NONE,
            ));
            grad_quad(
                p,
                [
                    v(-10.0 * f, -12.0 + bob),
                    v(-9.0 * f, -32.0 + bob),
                    v(11.0 * f, -30.0 + bob),
                    v(11.0 * f, -13.0 + bob),
                ],
                [
                    Color32::from_rgb(225, 215, 190),
                    Color32::from_rgb(245, 238, 220),
                    Color32::from_rgb(220, 210, 185),
                    Color32::from_rgb(200, 190, 165),
                ],
            );
            p.rect_filled(
                eframe::egui::Rect::from_center_size(v(1.0 * f, -22.0 + bob), vec2(7.0 * z, 5.0 * z)),
                1.0,
                *color,
            );
            if *loaded {
                for x in [-8.0, -3.0, 4.0] {
                    p.rect_filled(
                        eframe::egui::Rect::from_center_size(v(x * f, -8.0 + bob), vec2(4.0 * z, 3.5 * z)),
                        0.5,
                        Color32::from_rgb(170, 130, 80),
                    );
                }
            }
        }
        Sprite::Animal { .. } | Sprite::Donkey { .. } => animals::draw(p, at, z, sp),
    }
}

/// A holy place without its picture: a small stone altar.
fn altar(p: &Painter, at: Pos2, z: f32) {
    let v = |x: f32, y: f32| at + vec2(x * z, y * z);
    grad_quad(
        p,
        [v(-4.0, -6.0), v(4.0, -6.0), v(4.0, 0.0), v(-4.0, 0.0)],
        [Color32::from_gray(210), Color32::from_gray(180), Color32::from_gray(140), Color32::from_gray(170)],
    );
}

/// A map decoration: a bush, a stone or flowers.
fn decoration(p: &Painter, at: Pos2, z: f32, id: u8) {
    let v = |x: f32, y: f32| at + vec2(x * z, y * z);
    if id % 3 == 0 {
        p.circle_filled(v(0.0, -3.0), 4.0 * z, Color32::from_rgb(60, 110, 40));
        p.circle_filled(v(3.0, -4.0), 3.0 * z, Color32::from_rgb(80, 130, 50));
    } else if id % 3 == 1 {
        boulder(p, at, z, id == MOUNTAIN_BOULDER);
    } else {
        for i in 0..3 {
            p.circle_filled(v(-3.0 + i as f32 * 3.0, -2.0), 1.5 * z, Color32::from_rgb(230, 200, 60));
        }
    }
}

/// A boat road's boat under its carrier.
fn boat(p: &Painter, at: Pos2, z: f32) {
    let v = |x: f32, y: f32| at + vec2(x * z, y * z);
    let wood = Color32::from_rgb(130, 85, 45);
    grad_quad(
        p,
        [v(-9.0, -3.0), v(9.0, -3.0), v(6.0, 1.5), v(-6.0, 1.5)],
        [dark(wood, 1.1), dark(wood, 0.9), dark(wood, 0.7), dark(wood, 0.85)],
    );
}

/// One picture of the art gallery: a sprite, or one of the campaign's creatures that aren't
/// sprites yet.
enum Cell {
    Sp(Sprite),
    Goose(bool),
    Bird(Color32),
}

/// The art gallery's cells in reading order, ten to a row, each with its name (for the site's
/// animation page: `tools/animations.sh`).
fn gallery_cells(anim: u32, time: f32) -> Vec<(String, Cell)> {
    let blue = Color32::from_rgb(40, 90, 220);
    let mut cells: Vec<(String, Cell)> = Vec::new();
    let animals =
        [Animal::Deer, Animal::Stag, Animal::Fox, Animal::Rabbit, Animal::Sheep, Animal::Duck, Animal::PolarBear];
    for species in animals {
        cells.push((format!("{species:?}"), Cell::Sp(Sprite::Animal { species, walking: false, west: false, anim })));
    }
    cells.push((
        "Pack donkey".into(),
        Cell::Sp(Sprite::Donkey { carry: Some(Ware::Boards), walking: true, west: false, anim }),
    ));
    cells.push(("Goose".into(), Cell::Goose(false)));
    cells.push(("Goose walking".into(), Cell::Goose(true)));
    for species in animals {
        let sp = Sprite::Animal { species, walking: true, west: true, anim: anim + 1 };
        cells.push((format!("{species:?} walking"), Cell::Sp(sp)));
    }
    cells.push(("Vulture (omen)".into(), Cell::Bird(Color32::from_rgb(70, 58, 50))));
    cells.push(("Raven (omen)".into(), Cell::Bird(Color32::from_rgb(34, 30, 34))));
    cells.push(("Flag".into(), Cell::Sp(Sprite::Flag { color: blue, wares: vec![Ware::Wood, Ware::Stones] })));
    let figure = |job, carry, walking, working| Sprite::Figure {
        job,
        color: blue,
        carry,
        walking,
        face_west: false,
        working,
        anim,
        hp: 3,
    };
    let walkers = [
        Job::Helper,
        Job::Woodcutter,
        Job::Builder,
        Job::Farmer,
        Job::Miner,
        Job::Geologist,
        Job::Private,
        Job::General,
    ];
    for (i, job) in walkers.into_iter().enumerate() {
        // every other one walks west, to see them turn with their tools
        let dir = Some(if i % 2 == 0 { rr_core::Direction::E } else { rr_core::Direction::W });
        let carry = (i == 0).then_some(Ware::Wood);
        let name = if i == 0 { "Carrier with wood".to_string() } else { format!("{job:?} walking") };
        cells.push((name, Cell::Sp(figure(job, carry, dir, false))));
    }
    cells.push(("Fire".into(), Cell::Sp(Sprite::Fire)));
    cells.push(("Ship".into(), Cell::Sp(Sprite::Ship { color: blue, loaded: true, dir: Some(rr_core::Direction::E) })));
    let workers = [
        Job::Woodcutter,
        Job::Stonemason,
        Job::Miner,
        Job::Farmer,
        Job::Fisher,
        Job::Forester,
        Job::Builder,
        Job::Geologist,
        Job::Hunter,
        Job::Officer,
    ];
    for job in workers {
        let name =
            if job.soldier_rank().is_some() { "Soldier fighting".to_string() } else { format!("{job:?} at work") };
        cells.push((name, Cell::Sp(figure(job, None, None, true))));
    }
    // the idle motions of a carrier, each whole (12 s)
    for pose in crate::idle::POSES {
        let sp = Sprite::Idler { color: blue, pose, t: time % (crate::idle::POSE_GF as f32 / 20.0), west: false };
        cells.push((format!("Idle: {pose:?}"), Cell::Sp(sp)));
    }
    cells
}

/// The gallery's cell size and where its first cell's ground point lies in the panel.
pub const GALLERY_CELL: (f32, f32) = (120.0, 118.0);
pub const GALLERY_FIRST: (f32, f32) = (60.0, 100.0);

/// Every animal, figure and motion, big, on a meadow panel, ten to a row (RR_GALLERY: art work
/// and the site's animation page only).
pub fn gallery(p: &Painter, rect: eframe::egui::Rect, time: f32) -> Vec<String> {
    let panel = rect.shrink(40.0);
    p.rect_filled(panel, 6.0, Color32::from_rgb(96, 128, 56));
    let anim = (time * 4.0) as u32;
    let z = 4.0;
    let cells = gallery_cells(anim, time);
    for (i, (_, cell)) in cells.iter().enumerate() {
        let (col, row) = ((i % 10) as f32, (i / 10) as f32);
        let at = panel.min + vec2(GALLERY_FIRST.0 + col * GALLERY_CELL.0, GALLERY_FIRST.1 + row * GALLERY_CELL.1);
        match cell {
            // (a ship and a fire are as big as a building: smaller here, to fit their cells)
            Cell::Sp(sp @ (Sprite::Ship { .. } | Sprite::Fire)) => draw(p, at, z * 0.55, sp, time),
            Cell::Sp(sp) => draw(p, at, z, sp, time),
            Cell::Goose(walking) => animals::goose(p, at, z, (*walking, false), anim),
            Cell::Bird(c) => animals::omen_bird(p, at - vec2(0.0, 40.0), z, false, (time * 1.5).fract(), *c),
        }
    }
    cells.into_iter().map(|(n, _)| n).collect()
}

fn size_of(kind: B) -> Size {
    use B::*;
    match kind {
        Barracks | Guardhouse | LookoutTower | Woodcutter | Fishery | Quarry | Forester | Hunter | Well => Size::Hut,
        GraniteMine | CoalMine | IronMine | GoldMine => Size::Mine,
        Headquarters | Fortress | PigFarm | Farm | DonkeyBreeder => Size::Castle,
        Harbor => Size::Harbor,
        _ => Size::House,
    }
}

/// Distinct roof color and emblem per building.
fn look(kind: B) -> (Color32, Color32, Option<Color32>) {
    use B::*;
    let plaster = Color32::from_rgb(222, 205, 170);
    let stone = Color32::from_rgb(170, 165, 155);
    let timber = Color32::from_rgb(150, 105, 60);
    let red = Color32::from_rgb(180, 70, 50);
    let thatch = Color32::from_rgb(200, 165, 80);
    let slate = Color32::from_rgb(90, 95, 110);
    match kind {
        Headquarters | Fortress | Watchtower | Guardhouse | Barracks => (stone, slate, None),
        Woodcutter => (timber, thatch, Some(ware_color(Ware::Wood))),
        Forester => (timber, Color32::from_rgb(90, 140, 60), None),
        Quarry => (stone, thatch, Some(ware_color(Ware::Stones))),
        Fishery => (timber, thatch, Some(ware_color(Ware::Fish))),
        Hunter => (timber, thatch, Some(ware_color(Ware::Meat))),
        Well => (stone, red, Some(ware_color(Ware::Water))),
        Sawmill => (plaster, red, Some(ware_color(Ware::Boards))),
        Mill => (plaster, thatch, Some(ware_color(Ware::Flour))),
        Bakery => (plaster, red, Some(ware_color(Ware::Bread))),
        Brewery => (plaster, red, Some(ware_color(Ware::Beer))),
        Slaughterhouse => (plaster, red, Some(ware_color(Ware::Meat))),
        IronSmelter => (stone, slate, Some(ware_color(Ware::Iron))),
        Armory => (stone, red, Some(ware_color(Ware::Sword))),
        Metalworks => (stone, red, Some(ware_color(Ware::Tongs))),
        Mint => (stone, red, Some(ware_color(Ware::Coins))),
        Storehouse | Harbor => (plaster, slate, None),
        Farm => (plaster, thatch, Some(ware_color(Ware::Grain))),
        PigFarm => (plaster, thatch, Some(ware_color(Ware::Ham))),
        DonkeyBreeder => (plaster, thatch, None),
        Shipyard => (timber, red, Some(ware_color(Ware::Boat))),
        Catapult => (timber, slate, Some(ware_color(Ware::Stones))),
        LookoutTower => (timber, slate, None),
        GraniteMine | CoalMine | IronMine | GoldMine => (timber, timber, None),
    }
}

#[allow(clippy::too_many_lines)] // to split (architect round 2, #8: no new long ones)
fn building(
    p: &Painter,
    at: Pos2,
    z: f32,
    kind: B,
    color: Color32,
    progress: Option<u32>,
    working: bool,
    soldiers: u8,
    nation: u8,
    time: f32,
) {
    let v = |x: f32, y: f32| at + vec2(x * z, y * z);
    let size = size_of(kind);
    let (wall, roof, emblem) = look(kind);
    let is_stone = wall == Color32::from_rgb(170, 165, 155);
    // the four nations build in their own materials
    let (wall, roof) = match nation {
        1 if !is_stone => (Color32::from_rgb(150, 105, 60), Color32::from_rgb(105, 75, 45)),
        2 if !is_stone => (Color32::from_rgb(205, 160, 105), Color32::from_rgb(185, 130, 80)),
        3 if !is_stone => (Color32::from_rgb(230, 225, 215), Color32::from_rgb(70, 80, 100)),
        1 => (wall, Color32::from_rgb(80, 70, 60)),
        2 => (Color32::from_rgb(190, 160, 120), Color32::from_rgb(170, 120, 70)),
        3 => (wall, Color32::from_rgb(60, 70, 90)),
        _ => (wall, roof),
    };
    let (w, h, d) = match size {
        Size::Hut => (17.0, 14.0, 8.0),
        Size::House => (23.0, 20.0, 11.0),
        Size::Castle | Size::Harbor => (30.0, 25.0, 14.0),
        Size::Mine => (14.0, 12.0, 6.0),
    };
    // ground shadow
    p.add(Shape::ellipse_filled(v(-2.0, -1.0), vec2(w * 1.15 * z, d * 0.7 * z), Color32::from_black_alpha(50)));
    if let Some(pct) = progress {
        // construction site: stakes, then walls growing
        let k = pct as f32 / 100.0;
        p.add(Shape::ellipse_filled(v(-2.0, -2.0), vec2(w * z, d * 0.6 * z), Color32::from_rgb(150, 120, 80)));
        for (x, y) in [(-w, -2.0), (w * 0.7, -2.0), (-w * 0.6, -d), (w, -d * 0.8)] {
            p.line_segment([v(x, y), v(x, y - 10.0)], Stroke::new(1.5 * z, Color32::from_rgb(110, 80, 40)));
        }
        if k > 0.0 {
            let hh = h * k.min(0.7) / 0.7;
            poly(
                p,
                vec![v(-w, -2.0), v(-w, -2.0 - hh), v(w * 0.4, -2.0 - hh), v(w * 0.4, -2.0)],
                dark(wall, 0.95),
                dark(wall, 0.6),
            );
            if k > 0.7 {
                let rk = (k - 0.7) / 0.3;
                poly(
                    p,
                    vec![v(-w - 2.0, -2.0 - h), v(-w * 0.3, -2.0 - h - 12.0 * rk), v(w * 0.4 + 2.0, -2.0 - h)],
                    roof,
                    dark(roof, 0.6),
                );
            }
        }
        return;
    }
    if size == Size::Mine {
        poly(
            p,
            vec![v(-w, 0.0), v(-w * 0.7, -h), v(w * 0.7, -h), v(w, 0.0)],
            Color32::from_rgb(110, 95, 75),
            Color32::from_rgb(60, 50, 40),
        );
        poly(
            p,
            vec![v(-5.0, 0.0), v(-5.0, -8.0), v(5.0, -8.0), v(5.0, 0.0)],
            Color32::from_rgb(25, 20, 15),
            Color32::from_rgb(25, 20, 15),
        );
        p.line_segment([v(-6.0, -9.0), v(6.0, -9.0)], Stroke::new(2.0 * z, Color32::from_rgb(140, 100, 50)));
        let c = match kind {
            B::CoalMine => ware_color(Ware::Coal),
            B::IronMine => ware_color(Ware::IronOre),
            B::GoldMine => ware_color(Ware::Gold),
            _ => ware_color(Ware::Stones),
        };
        p.circle_filled(v(9.0, -3.0), 3.0 * z, c);
        return;
    }
    // side wall (depth) and front wall with light from the upper left
    let x1 = w * 0.45;
    let stone = is_stone;
    let timber_wall = wall == Color32::from_rgb(150, 105, 60);
    grad_quad(
        p,
        [v(x1, 0.0), v(x1, -h), v(x1 + d, -h - d * 0.6), v(x1 + d, -d * 0.6)],
        [dark(wall, 0.66), dark(wall, 0.74), dark(wall, 0.62), dark(wall, 0.52)],
    );
    grad_quad(
        p,
        [v(-w, 0.0), v(-w, -h), v(x1, -h), v(x1, 0.0)],
        [dark(wall, 0.9), dark(wall, 1.12), dark(wall, 1.0), dark(wall, 0.82)],
    );
    // wall detail: stone joints, log rows or timber framing
    let line = |a: Pos2, b: Pos2, c: Color32, wd: f32| p.line_segment([a, b], Stroke::new(wd * z, c));
    if stone {
        let rows = (h / 4.0) as i32;
        for r in 1..rows {
            let y = -(r as f32) * h / rows as f32;
            line(v(-w, y), v(x1, y), dark(wall, 0.72), 0.5);
            let off = if r % 2 == 0 { 0.0 } else { 3.0 };
            let mut x = -w + off;
            while x < x1 - 1.0 {
                line(v(x, y), v(x, y + h / rows as f32), dark(wall, 0.75), 0.5);
                x += 6.0;
            }
        }
    } else if timber_wall {
        let rows = (h / 3.0) as i32;
        for r in 1..rows {
            let y = -(r as f32) * h / rows as f32;
            line(v(-w, y), v(x1, y), dark(wall, 0.6), 0.8);
            line(v(x1, y), v(x1 + d, y - d * 0.6), dark(wall, 0.45), 0.6);
        }
    } else {
        let beam = Color32::from_rgb(92, 62, 36);
        line(v(-w, -h * 0.5), v(x1, -h * 0.5), beam, 1.4);
        line(v(-w, -h + 0.5), v(x1, -h + 0.5), beam, 1.6);
        for x in [-w, -w * 0.3, x1] {
            line(v(x, 0.0), v(x, -h), beam, 1.4);
        }
        line(v(-w, -h * 0.5), v(-w * 0.65, -h), beam, 1.0);
        line(v(x1, -h * 0.5), v(x1 - w * 0.35, -h), beam, 1.0);
        line(v(x1, -h * 0.5), v(x1 + d, -h * 0.5 - d * 0.6), dark(beam, 0.8), 1.2);
    }
    // door with frame and a little light on its left edge
    grad_quad(
        p,
        [v(-4.5, 0.0), v(-4.5, -9.0), v(2.5, -9.0), v(2.5, 0.0)],
        [
            Color32::from_rgb(70, 44, 24),
            Color32::from_rgb(92, 60, 34),
            Color32::from_rgb(64, 40, 20),
            Color32::from_rgb(48, 30, 14),
        ],
    );
    p.line_segment([v(-5.0, 0.0), v(-5.0, -9.5)], Stroke::new(1.0 * z, Color32::from_rgb(120, 90, 60)));
    p.line_segment([v(-5.0, -9.5), v(3.0, -9.5)], Stroke::new(1.0 * z, Color32::from_rgb(120, 90, 60)));
    // windows with frames and a glint
    if size != Size::Hut {
        for x in [-w * 0.75, w * 0.12] {
            let r = eframe::egui::Rect::from_min_size(v(x, -h * 0.78), vec2(5.0 * z, 5.0 * z));
            p.rect_filled(r, 0.0, Color32::from_rgb(40, 34, 36));
            p.rect_stroke(
                r,
                0.0,
                Stroke::new(0.9 * z, Color32::from_rgb(110, 80, 50)),
                eframe::egui::StrokeKind::Outside,
            );
            p.line_segment([r.center_top(), r.center_bottom()], Stroke::new(0.6 * z, Color32::from_rgb(110, 80, 50)));
            p.circle_filled(r.min + vec2(1.2 * z, 1.2 * z), 0.7 * z, Color32::from_white_alpha(120));
        }
    }
    let military = matches!(kind, B::Barracks | B::Guardhouse | B::Watchtower | B::Fortress | B::Headquarters);
    if military {
        // crenellations and a tower
        for i in 0..5 {
            let x = -w + i as f32 * (w * 1.45 / 4.5);
            grad_quad(
                p,
                [v(x, -h), v(x, -h - 4.0), v(x + 4.0, -h - 4.0), v(x + 4.0, -h)],
                [dark(wall, 1.0), dark(wall, 1.15), dark(wall, 0.95), dark(wall, 0.8)],
            );
        }
        let tw = w * 0.35;
        let th = -h - 14.0 - w * 0.3;
        grad_quad(
            p,
            [v(-tw, -h), v(-tw, th), v(tw * 0.2, th), v(tw * 0.2, -h)],
            [dark(wall, 0.95), dark(wall, 1.15), dark(wall, 1.0), dark(wall, 0.85)],
        );
        grad_quad(
            p,
            [v(tw * 0.2, -h), v(tw * 0.2, th), v(tw, th), v(tw, -h)],
            [dark(wall, 0.7), dark(wall, 0.8), dark(wall, 0.62), dark(wall, 0.55)],
        );
        for k in 1..4 {
            let y = -h + (th + h) * k as f32 / 4.0;
            p.line_segment([v(-tw, y), v(tw, y)], Stroke::new(0.5 * z, dark(wall, 0.7)));
        }
        p.rect_filled(
            eframe::egui::Rect::from_min_size(v(-1.5, th * 0.5 - h * 0.5 - 2.0), vec2(2.5 * z, 5.0 * z)),
            0.0,
            Color32::from_rgb(35, 30, 30),
        );
        grad_tri(
            p,
            [v(-tw - 2.0, th), v(0.0, th - 12.0), v(tw * 0.2, th)],
            [dark(roof, 1.05), dark(roof, 1.25), dark(roof, 1.0)],
        );
        grad_tri(
            p,
            [v(tw * 0.2, th), v(0.0, th - 12.0), v(tw + 2.0, th)],
            [dark(roof, 0.75), dark(roof, 0.9), dark(roof, 0.6)],
        );
        let top = -h - 26.0 - w * 0.3;
        p.line_segment([v(0.0, top), v(0.0, top - 10.0)], Stroke::new(1.2 * z, Color32::from_rgb(60, 40, 20)));
        let wave = (time * 3.0).sin();
        poly(p, vec![v(0.0, top - 10.0), v(8.0, top - 8.0 + wave), v(0.0, top - 5.0)], color, dark(color, 0.6));
        if kind != B::Headquarters {
            for i in 0..soldiers.min(9) {
                p.circle_filled(v(-w + 3.0 + i as f32 * 3.5, 4.0), 1.3 * z, color);
            }
        }
    } else {
        let ridge = -h - 13.0 - w * 0.25;
        let (rx, rx2) = (-w * 0.27, -w * 0.27 + d);
        // gable (front triangle) and the long roof plane, shaded
        grad_quad(
            p,
            [v(-w - 3.0, -h), v(rx, ridge), v(rx, ridge), v(x1 + 3.0, -h)],
            [dark(roof, 1.08), dark(roof, 1.2), dark(roof, 1.2), dark(roof, 0.95)],
        );
        grad_quad(
            p,
            [v(x1 + 3.0, -h), v(rx, ridge), v(rx2, ridge - d * 0.6), v(x1 + d + 3.0, -h - d * 0.6)],
            [dark(roof, 0.86), dark(roof, 0.98), dark(roof, 0.8), dark(roof, 0.66)],
        );
        // tile / thatch rows on the long plane
        for i in 1..5 {
            let f = i as f32 / 5.0;
            let a = v(x1 + 3.0 + (rx - x1 - 3.0) * f, -h + (ridge + h) * f);
            let b = v(x1 + d + 3.0 + (rx2 - x1 - d - 3.0) * f, -h - d * 0.6 + (ridge - d * 0.6 + h + d * 0.6) * f);
            p.line_segment([a, b], Stroke::new(0.7 * z, dark(roof, 0.62)));
        }
        // eave shadow under the roof
        p.line_segment([v(-w, -h + 1.0), v(x1, -h + 1.0)], Stroke::new(1.6 * z, Color32::from_black_alpha(70)));
        // chimney on houses
        if size != Size::Hut {
            let cx = rx + d * 0.6;
            grad_quad(
                p,
                [v(cx, ridge + 4.0), v(cx, ridge - 6.0), v(cx + 3.5, ridge - 6.0), v(cx + 3.5, ridge + 4.0)],
                [
                    Color32::from_rgb(150, 80, 60),
                    Color32::from_rgb(170, 95, 70),
                    Color32::from_rgb(130, 70, 50),
                    Color32::from_rgb(110, 60, 45),
                ],
            );
        }
        // a small player-color pennant on the gable
        let gx = -w * 0.27;
        let gy = -h - 13.0 - w * 0.25;
        p.line_segment([v(gx, gy), v(gx, gy - 6.0)], Stroke::new(1.0 * z, Color32::from_rgb(60, 40, 20)));
        poly(p, vec![v(gx, gy - 6.0), v(gx + 5.0, gy - 5.0), v(gx, gy - 3.0)], color, dark(color, 0.6));
    }
    if let Some(e) = emblem {
        p.circle_filled(v(w * 0.45 + d * 0.5, -h * 0.55), 3.0 * z, e);
        p.circle_stroke(v(w * 0.45 + d * 0.5, -h * 0.55), 3.0 * z, Stroke::new(1.0_f32, dark(e, 0.5)));
    }
    if working
        && matches!(kind, B::Sawmill | B::IronSmelter | B::Mint | B::Bakery | B::Brewery | B::Armory | B::Metalworks)
    {
        let t = time * 0.8;
        for i in 0..3 {
            let k = (t + i as f32 * 0.33).fract();
            p.circle_filled(
                v(-w * 0.8 + k * 4.0, -h - 14.0 - k * 18.0),
                (2.0 + k * 3.0) * z,
                Color32::from_white_alpha((120.0 * (1.0 - k)) as u8),
            );
        }
    }
}

fn figure(
    p: &Painter,
    at: Pos2,
    z: f32,
    job: Job,
    color: Color32,
    carry: Option<Ware>,
    (walking, face_west): (Option<rr_core::Direction>, bool),
    working: bool,
    anim: u32,
    hp: u8,
) {
    use crate::animals::{Frame, part};
    // facing: walking west turns the figure round, as does a foe on the left
    let f = if face_west
        || matches!(walking, Some(rr_core::Direction::W | rr_core::Direction::NW | rr_core::Direction::SW))
    {
        -1.0
    } else {
        1.0
    };
    let fr = Frame { at, z, f };
    let v = |x: f32, y: f32| fr.v(x, y);
    let phase = (anim % 4) as f32 * std::f32::consts::FRAC_PI_2;
    let step = if walking.is_some() { phase.sin() * 1.7 } else { 0.0 };
    let bob = if walking.is_some() { -phase.sin().abs() * 0.8 } else { 0.0 };
    let skin = Color32::from_rgb(232, 186, 146);
    let soldier = job.soldier_rank().is_some();
    let tunic = match job {
        Job::Helper => Color32::from_rgb(176, 146, 104),
        Job::Builder => Color32::from_rgb(104, 118, 142),
        Job::Woodcutter | Job::Forester => Color32::from_rgb(64, 118, 52),
        Job::Miner | Job::Stonemason => Color32::from_rgb(104, 104, 108),
        Job::Geologist => Color32::from_rgb(196, 172, 64),
        Job::Farmer => Color32::from_rgb(196, 168, 110),
        Job::Fisher => Color32::from_rgb(70, 110, 150),
        Job::Hunter => Color32::from_rgb(110, 90, 50),
        _ if soldier => Color32::from_rgb(150, 150, 166),
        _ => Color32::from_rgb(132, 100, 158),
    };
    let ink = Color32::from_rgba_unmultiplied(30, 20, 10, 150);
    let trousers = Color32::from_rgb(86, 64, 40);
    let boot = Color32::from_rgb(52, 36, 22);
    // shadow on the ground
    p.add(Shape::ellipse_filled(at + vec2(1.0 * z, 0.3 * z), vec2(4.2 * z, 1.4 * z), Color32::from_black_alpha(60)));
    // legs (the far one darker) with a bent knee and boots, tapered and outlined
    for (x, s, k) in [(-0.9, -step, 0.75), (0.9, step, 1.0)] {
        let knee = (x + s * 0.45 + 0.3, -3.0 + bob * 0.5);
        fr.limb(p, &[(x, -6.0 + bob), knee, (x + s, -0.7)], &[1.9, 1.5, 1.3], dark(trousers, k), None);
        fr.limb(p, &[(x + s - 0.2, -0.6), (x + s + 1.1, -0.4)], &[1.3, 1.0], dark(boot, k), None);
    }
    // the arms: sleeves bent at the elbow, a hand; held up when carrying
    let carrying = carry.is_some() && !soldier;
    let arm = |sx: f32, swing: f32, k: f32| {
        let shoulder = (sx, -11.0 + bob);
        let (elbow, hand) = if carrying {
            ((sx * 1.15, -14.0 + bob), (sx * 0.55, -17.4 + bob))
        } else {
            ((sx + swing * 0.5, -8.6 + bob), (sx + swing + 0.3, -6.4 + bob))
        };
        fr.limb(p, &[shoulder, elbow, hand], &[1.6, 1.35, 1.1], dark(tunic, k), None);
        fr.dot(p, hand.0, hand.1, 0.85, dark(skin, k));
    };
    arm(-2.3, step * 0.8, 0.7);
    // tunic: a lit chest over the hips, a belt and a sash in the player's colour
    fr.body(p, &[part(0.0, -6.4 + bob, 2.6, 1.4, dark(tunic, 0.9)), part(0.0, -9.3 + bob, 2.7, 3.0, tunic)]);
    p.line_segment([v(-2.5, -6.3 + bob), v(2.5, -6.3 + bob)], Stroke::new(0.9 * z, dark(tunic, 0.5)));
    p.line_segment([v(-2.2, -11.6 + bob), v(2.3, -6.8 + bob)], Stroke::new(1.5 * z, ink));
    p.line_segment([v(-2.2, -11.6 + bob), v(2.3, -6.8 + bob)], Stroke::new(1.1 * z, color));
    arm(2.3, -step * 0.8, 1.0);
    // head: lit, a nose and an eye towards the way it faces, hair or the job's hat
    let (hx, hy) = (0.2, -14.6 + bob);
    fr.body(p, &[part(hx, hy, 2.4, 2.5, skin)]);
    fr.dot(p, hx + 2.3, hy + 0.3, 0.55, dark(skin, 0.9));
    fr.dot(p, hx + 1.2, hy - 0.4, 0.33, Color32::from_rgb(40, 28, 20));
    if soldier {
        let rank = job.soldier_rank().unwrap_or(0);
        // the rank by the helmet: iron, bronze, silver, gold (the general's brightest)
        let metal = [
            Color32::from_rgb(120, 122, 128),
            Color32::from_rgb(176, 120, 64),
            Color32::from_rgb(196, 200, 210),
            Color32::from_rgb(222, 184, 72),
            Color32::from_rgb(250, 214, 96),
        ][rank.min(4) as usize];
        fr.body(p, &[part(hx, hy - 1.3, 2.7, 1.9, metal)]);
        p.line_segment([v(hx - 2.8, hy - 0.4), v(hx + 2.8, hy - 0.4)], Stroke::new(0.8 * z, dark(metal, 0.7)));
        fr.tri(p, [(hx - 0.5, hy - 3.0), (hx, hy - 5.2), (hx + 0.5, hy - 3.0)], dark(metal, 0.9));
        let swing = if working { ((anim % 4) as f32 - 1.5) * 2.0 } else { 0.0 };
        fr.limb(
            p,
            &[(3.0, -9.0 + bob), (7.0, -15.0 + bob + swing)],
            &[1.1, 0.7],
            Color32::from_rgb(222, 224, 234),
            None,
        );
        fr.limb(p, &[(2.4, -8.4 + bob), (3.6, -9.6 + bob)], &[0.9, 0.9], Color32::from_rgb(120, 84, 40), None);
        // the shield: the player's colour round an iron boss
        fr.body(p, &[part(-3.4, -9.0 + bob, 2.3, 2.6, color)]);
        fr.dot(p, -3.6, -9.2 + bob, 0.7, Color32::from_rgb(200, 200, 206));
        // hit points
        for i in 0..hp.min(7) {
            p.circle_filled(at + vec2((-3.5 + i as f32 * 1.4) * z, -22.0 * z), 0.6 * z, Color32::from_rgb(220, 40, 40));
        }
    } else {
        let hat = |c: Color32, brim: f32, top: f32| {
            fr.body(p, &[part(hx, hy - 1.6, brim, 0.9, dark(c, 0.85)), part(hx, hy - 2.4, 2.0, top, c)]);
        };
        match job {
            Job::Farmer | Job::Fisher => hat(Color32::from_rgb(222, 196, 112), 4.0, 1.4),
            Job::Woodcutter | Job::Forester | Job::Hunter => hat(dark(tunic, 0.8), 2.8, 1.8),
            Job::Miner => {
                hat(Color32::from_rgb(120, 110, 90), 2.9, 1.6);
                fr.dot(p, hx + 1.4, hy - 2.3, 0.7, Color32::from_rgb(255, 220, 120));
            }
            Job::Builder | Job::Stonemason => hat(Color32::from_rgb(200, 70, 50), 2.6, 1.3),
            _ => {
                // hair over the back of the head
                fr.body(p, &[part(hx - 0.5, hy - 1.4, 2.4, 1.4, Color32::from_rgb(110, 72, 36))]);
            }
        }
    }
    if !soldier {
        job_tool(p, at, z, job, (working, walking.is_some() && carry.is_none()), anim, (bob, f));
    }
    if let Some(w) = carry {
        // held up with both hands, big enough to tell what it is
        let c = at + vec2(0.0, (-22.0 + bob) * z);
        p.circle_filled(c, 5.2 * z, Color32::from_black_alpha(60));
        // never smaller than 11 px: the economy is watched at zoom 1
        ware_icon(p, c, (9.5 * z).max(11.0), w);
    }
}

/// The tool of a job: swung while working, on the shoulder while walking.
fn job_tool(
    p: &Painter,
    at: Pos2,
    z: f32,
    job: Job,
    (working, walking): (bool, bool),
    anim: u32,
    (bob, f): (f32, f32),
) {
    // (`f` -1: the figure faces west, and its tool turns with it)
    let m = |x: f32, y: f32| vec2(x * f, y);
    let v = |x: f32, y: f32| at + m(x * z, y * z);
    let handle = Color32::from_rgb(120, 82, 42);
    let metal = Color32::from_rgb(190, 195, 205);
    // swing phase 0..1: wind up, strike
    let ph = ((anim % 8) as f32) / 8.0;
    let swing = if ph < 0.6 { ph / 0.6 } else { 1.0 - (ph - 0.6) / 0.4 };
    let line = |a: Pos2, b: Pos2, c: Color32, w: f32| p.line_segment([a, b], Stroke::new(w * z, c));
    match job {
        Job::Woodcutter | Job::Stonemason | Job::Miner | Job::Builder | Job::Geologist | Job::Armorer => {
            if !working && !walking {
                return;
            }
            // hand at the shoulder; the tool rotates from up-back to down-front
            // (walking: upright against the front shoulder, beside the face; it lay across it)
            let ang = if working { -2.2 + swing * 2.6 } else { -1.25 };
            let hand = v(2.6, -10.8 + bob);
            let len = if job == Job::Builder || job == Job::Geologist { 5.5 } else { 7.0 };
            let tip = hand + m(ang.cos() * len * z, ang.sin() * len * z);
            line(hand, tip, handle, 1.2);
            let d = (tip - hand).normalized();
            let n = m(-d.y, d.x);
            match job {
                Job::Woodcutter => p.add(Shape::convex_polygon(
                    vec![tip, tip + n * 2.2 * z - d * 1.0 * z, tip + n * 2.0 * z + d * 1.2 * z],
                    metal,
                    Stroke::NONE,
                )),
                Job::Stonemason | Job::Miner => line(tip - n * 2.2 * z, tip + n * 2.2 * z, metal, 1.0),
                _ => line(tip - n * 1.4 * z, tip + n * 1.4 * z, metal, 1.8),
            };
        }
        Job::Farmer => {
            if !working && !walking {
                return;
            }
            let ang = if working { 0.3 + swing * 1.2 } else { -1.2 };
            let hand = v(1.5, -11.0 + bob);
            let top = hand + m(-(ang.cos()) * 8.0 * z, -(ang.sin().abs() + 0.4) * 8.0 * z);
            line(hand, top, handle, 1.0);
            line(top, top + m(5.0 * z, 1.5 * z), metal, 1.0);
        }
        Job::Fisher => {
            if !working && !walking {
                return;
            }
            let hand = v(2.0, -11.0 + bob);
            let tip = hand + m(9.0 * z, if working { -6.0 + swing * 2.0 } else { -9.0 } * z);
            line(hand, tip, handle, 0.9);
            if working {
                line(tip, tip + m(1.0 * z, 9.0 * z), Color32::from_white_alpha(160), 0.5);
            }
        }
        Job::Forester => {
            if !working && !walking {
                return;
            }
            let dig = if working { swing * 3.0 } else { 0.0 };
            let hand = v(2.5, -11.0 + bob);
            let blade = v(4.0, -2.0 + dig);
            line(hand, blade, handle, 1.1);
            p.add(Shape::ellipse_filled(blade, vec2(1.3 * z, 1.8 * z), metal));
        }
        Job::Hunter => {
            let hand = v(-3.0, -11.0 + bob);
            p.add(Shape::line(
                vec![hand + m(0.0, -5.0 * z), hand + m(-1.8 * z, 0.0), hand + m(0.0, 5.0 * z)],
                Stroke::new(0.9 * z, handle),
            ));
            if working {
                line(hand + m(0.0, -5.0 * z), hand + m(0.0, 5.0 * z), Color32::from_white_alpha(170), 0.4);
                line(hand, hand + m(7.0 * z, 0.0), handle, 0.6);
            }
        }
        _ => {
            if working {
                let a = ((anim % 6) as f32 / 6.0) * std::f32::consts::TAU;
                line(
                    v(2.0, -10.0),
                    v(2.0 + a.cos() * 5.0, -12.0 + a.sin() * 4.0),
                    Color32::from_rgb(120, 120, 120),
                    1.3,
                );
            }
        }
    }
}

/// Small building-quality marker like the original's "building help" overlay.
pub fn bq_icon(p: &Painter, at: Pos2, z: f32, bq: Bq) {
    let v = |x: f32, y: f32| at + vec2(x * z, y * z);
    // a soft shadow so the markers sit on the ground
    if bq != Bq::Nothing {
        p.add(Shape::ellipse_filled(v(0.5, 0.5), vec2(5.0 * z, 1.8 * z), Color32::from_black_alpha(50)));
    }
    match bq {
        Bq::Nothing => {}
        Bq::Flag => {
            p.line_segment([v(0.0, 0.0), v(0.0, -8.0)], Stroke::new(1.2 * z, Color32::from_rgb(60, 40, 20)));
            poly(p, vec![v(0.0, -8.0), v(5.0, -7.0), v(0.0, -5.0)], Color32::YELLOW, Color32::from_rgb(120, 100, 0));
        }
        Bq::Mine => {
            // dark with a light rim: grey on grey rock was hard to see (new-player round 7, #5)
            let pts = vec![v(-5.5, 0.5), v(0.0, -8.5), v(5.5, 0.5)];
            p.add(Shape::convex_polygon(
                pts,
                Color32::from_rgb(52, 46, 44),
                Stroke::new(1.6 * z, Color32::from_rgb(236, 226, 200)),
            ));
            p.line_segment([v(-2.0, -1.5), v(2.0, -1.5)], Stroke::new(1.2 * z, Color32::from_rgb(236, 226, 200)));
        }
        Bq::Hut | Bq::House => {
            let (s, c) = if bq == Bq::Hut {
                (3.5, Color32::from_rgba_unmultiplied(230, 210, 122, 215))
            } else {
                (5.0, Color32::from_rgba_unmultiplied(217, 154, 58, 215))
            };
            poly(
                p,
                vec![v(-s, 0.0), v(-s, -s * 1.2), v(0.0, -s * 2.2), v(s, -s * 1.2), v(s, 0.0)],
                c,
                Color32::from_rgb(60, 40, 20),
            );
            if bq == Bq::House {
                // a door tells it from a hut without its colour
                p.rect_filled(
                    eframe::egui::Rect::from_min_max(v(-1.2, -3.0), v(1.2, 0.0)),
                    0.0,
                    Color32::from_rgb(60, 40, 20),
                );
            }
        }
        _ => {
            // a large site: a wide house with two gables (battlements read as "military here")
            let c = Color32::from_rgba_unmultiplied(180, 85, 60, 215);
            let edge = Color32::from_rgb(60, 40, 20);
            for ox in [-3.6, 3.6] {
                poly(
                    p,
                    vec![v(ox - 3.6, 0.0), v(ox - 3.6, -5.0), v(ox, -9.0), v(ox + 3.6, -5.0), v(ox + 3.6, 0.0)],
                    c,
                    edge,
                );
            }
            p.rect_filled(eframe::egui::Rect::from_min_max(v(-1.2, -3.2), v(1.2, 0.0)), 0.0, edge);
        }
    }
    let _ = pos2(0.0, 0.0);
}

/// A small pictogram for a ware, centred at `c`, about `s` pixels tall.
#[allow(clippy::too_many_lines)] // to split (architect round 2, #8: no new long ones)
pub fn ware_icon(p: &Painter, c: Pos2, s: f32, w: Ware) {
    use Ware::*;
    let u = s / 10.0;
    let v = |x: f32, y: f32| c + vec2(x * u, y * u);
    let col = ware_color(w);
    let st = |k: f32| Stroke::new((k * u).max(1.2), dark(col, 0.55));
    let handle = Color32::from_rgb(120, 80, 40);
    let metal = Color32::from_rgb(185, 190, 200);
    let tool = |head: &dyn Fn()| {
        p.line_segment([v(-3.5, 4.0), v(3.0, -3.5)], Stroke::new((1.6 * u).max(1.2), handle));
        head();
    };
    match w {
        Wood => {
            p.add(Shape::convex_polygon(vec![v(-4.5, -2.0), v(3.5, -2.0), v(3.5, 2.0), v(-4.5, 2.0)], col, st(0.6)));
            p.circle_filled(v(3.5, 0.0), 2.0 * u, Color32::from_rgb(200, 160, 110));
            p.circle_stroke(v(3.5, 0.0), 1.0 * u, Stroke::new((0.5 * u).max(1.2), Color32::from_rgb(140, 100, 60)));
        }
        Boards => {
            for (i, y) in [-3.0, 0.0, 3.0].iter().enumerate() {
                p.rect_filled(
                    eframe::egui::Rect::from_center_size(v(i as f32 * 0.6 - 0.6, *y), vec2(9.0 * u, 2.3 * u)),
                    0.3,
                    dark(col, 1.0 - i as f32 * 0.08),
                );
            }
        }
        Stones => {
            p.add(Shape::convex_polygon(
                vec![v(-4.0, 3.0), v(-3.5, -2.0), v(0.0, -4.0), v(4.0, -2.0), v(4.0, 3.0)],
                col,
                st(0.6),
            ));
            p.line_segment([v(-3.5, -2.0), v(4.0, -2.0)], st(0.4));
        }
        Gold => {
            // a gleaming nugget
            p.add(Shape::convex_polygon(
                vec![v(-4.0, 1.5), v(-2.5, -2.5), v(1.0, -3.5), v(4.0, -1.0), v(3.0, 3.0), v(-1.5, 3.5)],
                col,
                st(0.6),
            ));
            p.circle_filled(v(-0.5, -1.0), 1.0 * u, Color32::from_rgb(255, 250, 200));
        }
        Coal | IronOre => {
            for (x, y, r) in [(-2.0, 1.5, 2.5), (2.0, 1.8, 2.2), (0.0, -1.5, 2.4)] {
                p.circle_filled(v(x, y), r * u, col);
                p.circle_filled(v(x - 0.7, y - 0.7), r * 0.35 * u, dark(col, 1.4));
            }
        }
        Iron => {
            p.add(Shape::convex_polygon(vec![v(-4.5, 2.0), v(-3.0, -1.5), v(4.5, -1.5), v(3.0, 2.0)], col, st(0.6)));
        }
        Coins => {
            // a stack of coins seen from the side
            for i in 0..4 {
                let y = 3.0 - i as f32 * 1.6;
                p.add(Shape::ellipse_filled(v(0.0, y), vec2(4.0 * u, 1.2 * u), dark(col, 0.85 + i as f32 * 0.07)));
                p.add(Shape::ellipse_stroke(v(0.0, y), vec2(4.0 * u, 1.2 * u), st(0.4)));
            }
        }
        Grain => {
            for x in [-2.0, 0.0, 2.0] {
                p.line_segment([v(x * 0.5, 4.5), v(x, -1.0)], Stroke::new((0.6 * u).max(1.2), dark(col, 0.7)));
                p.add(Shape::ellipse_filled(v(x, -2.5), vec2(0.9 * u, 2.2 * u), col));
            }
        }
        Flour => {
            p.add(Shape::convex_polygon(
                vec![v(-3.5, 4.0), v(-3.0, -2.0), v(-1.0, -4.0), v(1.0, -4.0), v(3.0, -2.0), v(3.5, 4.0)],
                col,
                st(0.6),
            ));
        }
        Bread => {
            p.add(Shape::ellipse_filled(v(0.0, 0.5), vec2(4.5 * u, 2.8 * u), col));
            for x in [-1.5, 0.5, 2.5] {
                p.line_segment([v(x - 0.8, -1.0), v(x, 0.5)], st(0.5));
            }
        }
        Fish => {
            p.add(Shape::ellipse_filled(v(-0.5, 0.0), vec2(3.5 * u, 2.0 * u), col));
            p.add(Shape::convex_polygon(vec![v(2.5, 0.0), v(4.5, -2.0), v(4.5, 2.0)], col, Stroke::NONE));
            p.circle_filled(v(-2.5, -0.4), 0.5 * u, Color32::BLACK);
        }
        Meat => {
            // a steak with a fat rim
            p.add(Shape::ellipse_filled(v(0.0, 0.0), vec2(4.3 * u, 3.0 * u), Color32::from_rgb(235, 225, 200)));
            p.add(Shape::ellipse_filled(v(-0.3, 0.2), vec2(3.6 * u, 2.4 * u), col));
        }
        Ham => {
            // a ham leg on a bone, tied with string
            p.add(Shape::convex_polygon(
                vec![v(-4.5, 0.5), v(-3.0, -3.0), v(1.0, -2.5), v(2.5, 0.0), v(1.0, 2.5), v(-3.0, 3.5)],
                dark(col, 0.9),
                st(0.5),
            ));
            p.line_segment(
                [v(2.0, 0.0), v(4.8, 0.0)],
                Stroke::new((1.4 * u).max(1.2), Color32::from_rgb(240, 230, 210)),
            );
            p.circle_filled(v(4.8, 0.0), 1.0 * u, Color32::from_rgb(240, 230, 210));
            p.line_segment(
                [v(-1.0, -2.8), v(-1.5, 3.3)],
                Stroke::new((0.4 * u).max(1.2), Color32::from_rgb(240, 230, 210)),
            );
        }
        Water => {
            p.add(Shape::convex_polygon(
                vec![v(-3.0, -3.0), v(3.0, -3.0), v(2.2, 4.0), v(-2.2, 4.0)],
                Color32::from_rgb(140, 100, 60),
                st(0.5),
            ));
            p.rect_filled(eframe::egui::Rect::from_min_max(v(-2.7, -2.5), v(2.7, -1.0)), 0.0, col);
        }
        Beer => {
            p.rect_filled(eframe::egui::Rect::from_min_max(v(-2.5, -3.0), v(2.0, 4.0)), 0.5, col);
            p.rect_filled(
                eframe::egui::Rect::from_min_max(v(-2.5, -4.0), v(2.0, -2.5)),
                0.5,
                Color32::from_rgb(250, 250, 240),
            );
            p.circle_stroke(v(3.0, 0.5), 1.6 * u, Stroke::new((0.8 * u).max(1.2), col));
        }
        Sword => {
            p.line_segment([v(-3.5, 3.5), v(4.0, -4.0)], Stroke::new((1.3 * u).max(1.2), metal));
            p.line_segment([v(-3.0, 1.0), v(-1.0, 3.0)], Stroke::new((1.0 * u).max(1.2), handle));
        }
        Shield => {
            p.circle_filled(v(0.0, 0.0), 4.0 * u, col);
            p.circle_stroke(v(0.0, 0.0), 4.0 * u, Stroke::new((0.8 * u).max(1.2), metal));
            p.circle_filled(v(0.0, 0.0), 1.2 * u, metal);
        }
        Boat => {
            p.add(Shape::convex_polygon(vec![v(-4.5, 0.0), v(4.5, 0.0), v(3.0, 3.0), v(-3.0, 3.0)], col, st(0.6)));
        }
        Axe => tool(&|| {
            // a broad blade with a curved edge
            p.add(Shape::convex_polygon(
                vec![v(1.0, -2.0), v(2.5, -4.8), v(5.0, -4.2), v(5.0, 0.2), v(2.5, 0.0)],
                metal,
                st(0.4),
            ));
        }),
        Hammer => tool(&|| {
            // a heavy square head across the handle
            p.add(Shape::convex_polygon(vec![v(0.2, -4.6), v(2.2, -6.0), v(5.4, -1.6), v(3.4, -0.2)], metal, st(0.4)));
        }),
        PickAxe => tool(&|| {
            // a pick pointed at both ends
            p.add(Shape::line(
                vec![v(-2.0, -6.0), v(1.5, -4.8), v(4.5, -2.0), v(6.0, 2.0)],
                Stroke::new((2.0 * u).max(1.5), metal),
            ));
        }),
        Shovel => tool(&|| {
            // a broad spade blade
            p.add(Shape::convex_polygon(vec![v(1.5, -2.0), v(3.0, -6.5), v(6.5, -5.0), v(5.0, -0.5)], metal, st(0.4)));
        }),
        Saw => {
            p.add(Shape::convex_polygon(
                vec![v(-4.0, 1.5), v(4.0, -1.5), v(4.0, 1.0), v(-4.0, 3.5)],
                metal,
                Stroke::NONE,
            ));
            p.rect_filled(eframe::egui::Rect::from_center_size(v(-4.0, 2.5), vec2(2.0 * u, 3.0 * u)), 0.3, handle);
        }
        Scythe => tool(&|| {
            // a long curved blade
            p.add(Shape::line(
                vec![v(3.0, -3.5), v(0.5, -5.5), v(-3.0, -6.0), v(-6.0, -5.0)],
                Stroke::new((1.8 * u).max(1.5), metal),
            ));
        }),
        RodAndLine => {
            p.line_segment([v(-4.0, 4.0), v(4.0, -4.0)], Stroke::new((0.9 * u).max(1.2), handle));
            p.line_segment([v(4.0, -4.0), v(4.0, 2.0)], Stroke::new((0.4 * u).max(1.2), Color32::WHITE));
            // the float
            p.circle_filled(v(4.0, 2.5), 1.4 * u, Color32::from_rgb(220, 50, 40));
        }
        Bow => {
            p.add(Shape::line(
                vec![v(-1.5, -4.5), v(1.5, -2.0), v(2.0, 0.0), v(1.5, 2.0), v(-1.5, 4.5)],
                Stroke::new((1.0 * u).max(1.2), handle),
            ));
            p.line_segment([v(-1.5, -4.5), v(-1.5, 4.5)], Stroke::new((0.4 * u).max(1.2), Color32::WHITE));
        }
        Crucible => {
            p.add(Shape::convex_polygon(
                vec![v(-3.5, -2.0), v(3.5, -2.0), v(2.0, 3.5), v(-2.0, 3.5)],
                Color32::from_rgb(110, 100, 95),
                st(0.5),
            ));
            p.rect_filled(
                eframe::egui::Rect::from_min_max(v(-3.0, -2.0), v(3.0, -0.8)),
                0.0,
                Color32::from_rgb(250, 160, 40),
            );
        }
        Tongs => {
            // two arms crossing at a pivot, with jaws at the top
            p.line_segment([v(-2.5, 4.5), v(1.5, -2.0)], Stroke::new((0.9 * u).max(1.2), metal));
            p.line_segment([v(2.5, 4.5), v(-1.5, -2.0)], Stroke::new((0.9 * u).max(1.2), metal));
            p.add(Shape::line(vec![v(1.5, -2.0), v(2.5, -4.0), v(1.0, -5.0)], Stroke::new((0.9 * u).max(1.2), metal)));
            p.add(Shape::line(
                vec![v(-1.5, -2.0), v(-2.5, -4.0), v(-1.0, -5.0)],
                Stroke::new((0.9 * u).max(1.2), metal),
            ));
            p.circle_filled(v(0.0, 0.3), 0.8 * u, dark(metal, 0.6));
        }
        Cleaver => {
            p.rect_filled(eframe::egui::Rect::from_min_max(v(-1.0, -4.0), v(4.0, 0.5)), 0.3, metal);
            p.line_segment([v(-1.0, -1.5), v(-4.5, 3.5)], Stroke::new((1.4 * u).max(1.2), handle));
        }
        Rollingpin => {
            p.line_segment([v(-6.0, 0.0), v(6.0, 0.0)], Stroke::new((1.2 * u).max(1.2), handle));
            p.rect_filled(
                eframe::egui::Rect::from_center_size(v(0.0, 0.0), vec2(8.0 * u, 4.0 * u)),
                2.0 * u,
                Color32::from_rgb(214, 174, 112),
            );
        }
    }
}
