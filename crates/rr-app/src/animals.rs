//! Animals and pack donkeys, drawn from lit shapes: every part of a body is an ellipse shaded
//! from the upper left (the sun of the pre-rendered buildings), the silhouette gets one thin
//! dark outline so it reads on any ground, and legs are tapered with a knee and a hoof
//! (graphics round 1: flat ellipses on stick legs).

use eframe::egui::{Color32, Mesh, Painter, Pos2, Shape, Stroke, Vec2, vec2};
use rr_core::Animal;
use rr_rules::Ware;

fn dark(c: Color32, k: f32) -> Color32 {
    let f = |x: u8| (x as f32 * k).min(255.0) as u8;
    Color32::from_rgb(f(c.r()), f(c.g()), f(c.b()))
}

/// The outline every silhouette gets.
const INK: Color32 = Color32::from_rgba_premultiplied(22, 15, 8, 170);

/// One part of a body: an ellipse at `c` (body units) with radii `r`.
#[derive(Clone, Copy)]
pub(crate) struct Part {
    c: Vec2,
    r: Vec2,
    color: Color32,
}

pub(crate) fn part(x: f32, y: f32, rx: f32, ry: f32, color: Color32) -> Part {
    Part { c: vec2(x, y), r: vec2(rx, ry), color }
}

/// How bright a point on an ellipse's rim is, lit from the upper left (0.72 .. 1.12).
pub fn rim_light(normal: Vec2) -> f32 {
    let sun = vec2(-0.55, -0.83);
    0.92 + 0.2 * (normal.x * sun.x + normal.y * sun.y)
}

/// A drawing frame: body units to the screen, `f` -1 for a body facing west.
pub(crate) struct Frame {
    pub at: Pos2,
    pub z: f32,
    pub f: f32,
}

impl Frame {
    pub(crate) fn v(&self, x: f32, y: f32) -> Pos2 {
        self.at + vec2(x * self.z * self.f, y * self.z)
    }

    /// The parts as one silhouette: all outlines first, then the lit fills over them.
    pub(crate) fn body(&self, p: &Painter, parts: &[Part]) {
        let o = 0.45;
        for q in parts {
            p.add(Shape::ellipse_filled(self.v(q.c.x, q.c.y), (q.r + vec2(o, o)) * self.z, INK));
        }
        for q in parts {
            self.lit(p, *q);
        }
    }

    /// An ellipse with a light centre towards the sun and a rim from dark to light.
    pub(crate) fn lit(&self, p: &Painter, q: Part) {
        // (a few pixels across needs few pieces: hundreds of carriers are on screen)
        let n = ((q.r.x.max(q.r.y) * self.z * 1.6) as u32).clamp(8, 20);
        let mut m = Mesh::default();
        let c = self.v(q.c.x, q.c.y);
        let hi = self.v(q.c.x - q.r.x * 0.3 * self.f, q.c.y - q.r.y * 0.35);
        m.colored_vertex(hi, dark(q.color, 1.1));
        for i in 0..n {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            let n = vec2(a.cos(), a.sin());
            let pos = c + vec2(n.x * q.r.x * self.z, n.y * q.r.y * self.z);
            m.colored_vertex(pos, dark(q.color, rim_light(vec2(n.x * self.f, n.y)) - 0.06));
        }
        for i in 0..n {
            m.add_triangle(0, 1 + i, 1 + (i + 1) % n);
        }
        p.add(Shape::mesh(m));
    }

    /// A tapered limb through `pts` (widths `w` at each), outlined, with a darker end.
    pub(crate) fn limb(&self, p: &Painter, pts: &[(f32, f32)], w: &[f32], color: Color32, end: Option<Color32>) {
        for pass in 0..2 {
            for i in 0..pts.len() - 1 {
                let (a, b) = (self.v(pts[i].0, pts[i].1), self.v(pts[i + 1].0, pts[i + 1].1));
                let d = (b - a).normalized();
                let n = vec2(-d.y, d.x);
                let grow = if pass == 0 { 0.45 * self.z } else { 0.0 };
                let (wa, wb) = (w[i] * self.z / 2.0 + grow, w[i + 1] * self.z / 2.0 + grow);
                let col = if pass == 0 { INK } else { color };
                p.add(Shape::convex_polygon(vec![a + n * wa, b + n * wb, b - n * wb, a - n * wa], col, Stroke::NONE));
                p.circle_filled(b, wb, col);
            }
        }
        if let (Some(c), Some(last)) = (end, pts.last()) {
            let tip = self.v(last.0, last.1);
            p.add(Shape::ellipse_filled(tip, vec2(w[w.len() - 1] * 0.7 * self.z, 0.5 * self.z), c));
        }
    }

    pub(crate) fn tri(&self, p: &Painter, pts: [(f32, f32); 3], color: Color32) {
        p.add(Shape::convex_polygon(
            pts.iter().map(|q| self.v(q.0, q.1)).collect(),
            color,
            Stroke::new(0.3 * self.z, INK),
        ));
    }

    pub(crate) fn dot(&self, p: &Painter, x: f32, y: f32, r: f32, color: Color32) {
        p.circle_filled(self.v(x, y), r * self.z, color);
    }
}

/// The drawing's x direction: -1 turns a body walking west round.
pub fn facing(west: bool) -> f32 {
    if west { -1.0 } else { 1.0 }
}

/// A four-legged walk: the leg's swing for a leg of phase offset `o` (0 standing).
fn swing(walking: bool, anim: u32, o: f32) -> f32 {
    if walking { ((anim as f32 + o) * 1.6).sin() } else { 0.0 }
}

/// Four legs from the body's underside at `y_top` down to the ground: the far pair first
/// (darker), the near pair after the body (drawn by the caller in two calls).
fn legs(
    p: &Painter,
    fr: &Frame,
    (rear, front): (f32, f32),
    y_top: f32,
    w: f32,
    color: Color32,
    hoof: Option<Color32>,
    near: bool,
    (walking, anim): (bool, u32),
) {
    let (k, dx, ph) = if near { (1.0, 0.7, 1.6) } else { (0.72, 0.0, 0.0) };
    let c = dark(color, k);
    for (x, o, bend) in [(rear + dx, ph, -0.5), (front + dx, ph + 1.6, 0.35)] {
        let s = swing(walking, anim, o) * 1.3;
        let knee = (x + bend + s * 0.4, y_top * 0.45);
        fr.limb(p, &[(x, y_top), knee, (x + s, 0.0)], &[w * 1.5, w, w * 0.8], c, hoof);
    }
}

/// An animal or a pack donkey (other sprites: nothing).
pub fn draw(p: &Painter, at: Pos2, z: f32, sp: &crate::sprites::Sprite) {
    use crate::sprites::Sprite;
    match sp {
        Sprite::Animal { species, walking, west, anim } => animal(p, at, z, *species, (*walking, *west), *anim),
        Sprite::Donkey { carry, walking, west, anim } => donkey(p, at, z, *carry, (*walking, *west), *anim),
        _ => {}
    }
}

fn animal(p: &Painter, at: Pos2, z: f32, species: Animal, (walking, west): (bool, bool), anim: u32) {
    // game must be seen from the usual zoom: drawn a good deal bigger than life
    let z = z * 1.6;
    let fr = Frame { at, z, f: facing(west) };
    let bob = if walking && anim % 2 == 0 { -0.6 } else { 0.0 };
    let shadow =
        |rx: f32| p.add(Shape::ellipse_filled(fr.v(0.5, 0.2), vec2(rx * z, 1.4 * z), Color32::from_black_alpha(55)));
    let eye = |x: f32, y: f32| fr.dot(p, x, y + bob, 0.32, Color32::from_rgb(15, 10, 5));
    let wg = (walking, anim);
    match species {
        Animal::Stag | Animal::Deer => {
            let c = Color32::from_rgb(160, 106, 60);
            let pale = Color32::from_rgb(232, 214, 184);
            let hoof = Some(Color32::from_rgb(40, 28, 18));
            shadow(5.5);
            legs(p, &fr, (-3.3, 2.8), -6.2 + bob, 0.85, c, hoof, false, wg);
            // short tail over a white rump
            fr.body(p, &[part(-4.6, -7.6 + bob, 1.0, 1.2, pale)]);
            fr.body(
                p,
                &[
                    part(0.0, -7.2 + bob, 4.4, 2.2, c),
                    part(-2.8, -7.4 + bob, 2.2, 2.3, c),
                    part(2.8, -7.6 + bob, 2.0, 2.3, c),
                ],
            );
            fr.lit(p, part(0.2, -5.7 + bob, 3.0, 0.75, pale));
            legs(p, &fr, (-3.3, 2.8), -6.2 + bob, 0.85, c, hoof, true, wg);
            // neck up to a head with a long muzzle and big ears
            fr.limb(p, &[(3.0, -8.4 + bob), (5.1, -11.4 + bob)], &[2.6, 1.5], c, None);
            for (ex, k) in [(4.6, 0.8), (5.5, 1.0)] {
                fr.tri(p, [(ex - 0.5, -12.0 + bob), (ex - 1.4, -14.2 + bob), (ex + 0.4, -12.4 + bob)], dark(c, k));
            }
            fr.body(p, &[part(5.7, -11.9 + bob, 1.5, 1.15, c), part(7.1, -11.4 + bob, 1.1, 0.7, dark(c, 0.95))]);
            fr.dot(p, 8.0, -11.5 + bob, 0.4, Color32::from_rgb(30, 20, 14));
            eye(5.9, -12.2);
            if species == Animal::Stag {
                let antler = Color32::from_rgb(226, 206, 168);
                for (side, k) in [(-0.7f32, 0.8), (0.5, 1.0)] {
                    let b = (5.4 + side, -12.8 + bob);
                    let top = (b.0 + side * 1.6 - 1.0, b.1 - 4.2);
                    fr.limb(p, &[b, (b.0 - 0.3, b.1 - 2.0), top], &[0.7, 0.55, 0.35], dark(antler, k), None);
                    for (t, len) in [(0.35f32, 1.8f32), (0.7, 1.5)] {
                        let at = (b.0 + (top.0 - b.0) * t, b.1 + (top.1 - b.1) * t);
                        fr.limb(p, &[at, (at.0 + len, at.1 - len * 0.6)], &[0.45, 0.3], dark(antler, k), None);
                    }
                }
            }
        }
        Animal::Fox => {
            let c = Color32::from_rgb(214, 108, 36);
            let white = Color32::from_rgb(242, 236, 224);
            let sock = Color32::from_rgb(52, 32, 22);
            shadow(5.0);
            legs(p, &fr, (-2.6, 2.4), -3.6 + bob, 0.7, sock, None, false, wg);
            // a bushy tail held out behind, with a white tip
            fr.body(p, &[part(-5.6, -4.4 + bob, 2.6, 1.3, c), part(-8.0, -4.0 + bob, 1.1, 0.9, white)]);
            fr.body(p, &[part(0.0, -4.6 + bob, 3.8, 1.7, c), part(2.6, -4.8 + bob, 1.6, 1.8, c)]);
            fr.lit(p, part(1.6, -3.6 + bob, 1.8, 0.7, white));
            legs(p, &fr, (-2.6, 2.4), -3.6 + bob, 0.7, sock, None, true, wg);
            // a pointed head with a white cheek and dark-backed ears
            for (ex, k) in [(3.6, 0.7), (4.6, 0.9)] {
                fr.tri(p, [(ex - 0.6, -6.6 + bob), (ex - 0.1, -8.8 + bob), (ex + 0.5, -6.6 + bob)], dark(c, k));
            }
            fr.body(p, &[part(4.2, -5.8 + bob, 1.6, 1.3, c)]);
            fr.tri(p, [(5.0, -6.4 + bob), (7.4, -5.2 + bob), (5.0, -4.6 + bob)], c);
            fr.lit(p, part(4.6, -4.9 + bob, 1.0, 0.5, white));
            fr.dot(p, 7.3, -5.2 + bob, 0.35, Color32::from_rgb(20, 14, 10));
            eye(4.8, -6.1);
        }
        Animal::Rabbit => {
            let c = Color32::from_rgb(170, 148, 122);
            // hops while walking, sits hunched otherwise
            let hop = if walking { -((anim % 4) as f32 * std::f32::consts::FRAC_PI_2).sin().abs() * 1.6 } else { 0.0 };
            p.add(Shape::ellipse_filled(fr.v(0.3, 0.2), vec2(3.6 * z, 1.1 * z), Color32::from_black_alpha(50)));
            fr.body(
                p,
                &[
                    part(-0.6, -2.4 + hop, 2.9, 2.1, c),
                    part(2.3, -4.0 + hop, 1.5, 1.35, c),
                    part(-3.0, -2.6 + hop, 0.9, 0.9, Color32::from_rgb(244, 240, 232)),
                ],
            );
            fr.lit(p, part(-1.3, -0.8 + hop, 1.6, 0.7, dark(c, 0.9)));
            for (ex, k) in [(1.6, 0.8), (2.4, 1.0)] {
                fr.limb(p, &[(ex, -5.0 + hop), (ex - 0.8, -8.0 + hop)], &[0.9, 0.6], dark(c, k), None);
            }
            fr.dot(p, 3.6, -3.9 + hop, 0.35, Color32::from_rgb(200, 140, 140));
            eye(2.6, -4.4 + hop);
        }
        Animal::Sheep => {
            let wool = Color32::from_rgb(238, 235, 224);
            let face = Color32::from_rgb(58, 52, 48);
            shadow(4.6);
            legs(p, &fr, (-2.4, 2.2), -3.0 + bob, 0.7, face, None, false, wg);
            // a woolly cloud
            fr.body(
                p,
                &[
                    part(-2.4, -4.6 + bob, 2.2, 2.1, wool),
                    part(0.0, -5.6 + bob, 2.5, 2.2, wool),
                    part(2.1, -4.8 + bob, 2.1, 2.1, wool),
                    part(0.0, -3.8 + bob, 2.8, 1.8, wool),
                ],
            );
            legs(p, &fr, (-2.4, 2.2), -3.0 + bob, 0.7, face, None, true, wg);
            // a dark face with drooping ears
            fr.body(p, &[part(3.4, -6.4 + bob, 0.8, 0.5, face), part(4.4, -5.6 + bob, 1.2, 1.5, face)]);
            fr.lit(p, part(4.0, -7.0 + bob, 0.8, 0.45, wool));
            eye(4.8, -6.0);
        }
        Animal::Duck => {
            // on the water or the bank: no legs, a pale body, a green head, an orange bill
            let body = Color32::from_rgb(214, 204, 186);
            p.add(Shape::ellipse_filled(fr.v(0.0, 0.0), vec2(3.6 * z, 0.9 * z), Color32::from_black_alpha(45)));
            fr.body(
                p,
                &[
                    part(0.0, -2.0 + bob, 3.3, 1.7, body),
                    part(-3.0, -2.8 + bob, 0.9, 0.6, dark(body, 0.8)),
                    part(2.7, -4.4 + bob, 1.3, 1.2, Color32::from_rgb(36, 110, 62)),
                ],
            );
            fr.lit(p, part(-0.4, -2.6 + bob, 2.0, 0.9, Color32::from_rgb(140, 112, 84)));
            fr.lit(p, part(2.2, -3.3 + bob, 1.0, 0.3, Color32::WHITE));
            fr.tri(p, [(3.7, -4.8 + bob), (5.6, -4.2 + bob), (3.7, -3.7 + bob)], Color32::from_rgb(240, 160, 40));
            eye(2.9, -4.7);
        }
        Animal::PolarBear => {
            // a big creamy body on thick legs, a low head with a dark nose (item 335)
            let fur = Color32::from_rgb(238, 232, 214);
            shadow(7.0);
            legs(p, &fr, (-4.0, 3.4), -3.6 + bob, 1.8, dark(fur, 0.92), None, false, wg);
            fr.body(
                p,
                &[
                    part(0.0, -5.6 + bob, 5.6, 3.0, fur),
                    part(-3.4, -5.8 + bob, 2.8, 3.1, fur),
                    part(3.6, -5.2 + bob, 2.6, 2.6, fur),
                ],
            );
            legs(p, &fr, (-4.0, 3.4), -3.6 + bob, 1.8, dark(fur, 0.92), None, true, wg);
            for ex in [5.8, 7.0] {
                fr.body(p, &[part(ex, -6.6 + bob, 0.6, 0.55, dark(fur, 0.9))]);
            }
            fr.body(p, &[part(6.8, -5.0 + bob, 2.2, 1.7, fur), part(8.6, -4.5 + bob, 1.2, 0.9, dark(fur, 0.95))]);
            fr.dot(p, 9.6, -4.6 + bob, 0.55, Color32::from_rgb(30, 26, 24));
            eye(7.4, -5.5);
        }
    }
}

fn donkey(p: &Painter, at: Pos2, z: f32, carry: Option<Ware>, (walking, west): (bool, bool), anim: u32) {
    let fr = Frame { at, z, f: facing(west) };
    let bob = if walking && anim % 2 == 0 { -0.8 } else { 0.0 };
    let c = Color32::from_rgb(128, 114, 100);
    let pale = Color32::from_rgb(206, 196, 182);
    let hoof = Some(Color32::from_rgb(40, 32, 26));
    let wg = (walking, anim);
    p.add(Shape::ellipse_filled(fr.v(0.5, 0.3), vec2(8.0 * z, 1.6 * z), Color32::from_black_alpha(55)));
    legs(p, &fr, (-4.6, 3.6), -6.0 + bob, 1.2, c, hoof, false, wg);
    // a tail with a tuft
    fr.limb(p, &[(-6.8, -8.6 + bob), (-8.0, -5.6 + bob)], &[0.8, 0.5], dark(c, 0.8), None);
    fr.dot(p, -8.1, -5.2 + bob, 0.7, dark(c, 0.45));
    fr.body(
        p,
        &[part(0.0, -7.8 + bob, 6.6, 3.2, c), part(-4.2, -8.0 + bob, 2.8, 3.0, c), part(4.0, -8.0 + bob, 2.6, 3.0, c)],
    );
    fr.lit(p, part(0.5, -5.8 + bob, 4.0, 1.0, pale));
    legs(p, &fr, (-4.6, 3.6), -6.0 + bob, 1.2, c, hoof, true, wg);
    // neck with a dark mane, the head with a pale muzzle, two long ears
    fr.limb(p, &[(4.6, -9.4 + bob), (7.4, -12.4 + bob)], &[3.2, 2.0], c, None);
    fr.limb(p, &[(4.0, -10.8 + bob), (6.6, -13.6 + bob)], &[0.9, 0.7], dark(c, 0.5), None);
    for (ex, k) in [(7.2, 0.75), (8.2, 0.95)] {
        fr.limb(p, &[(ex, -13.4 + bob), (ex - 1.0, -17.2 + bob)], &[1.1, 0.6], dark(c, k), None);
    }
    fr.body(p, &[part(8.4, -12.6 + bob, 2.2, 1.6, c), part(10.2, -12.0 + bob, 1.2, 1.0, pale)]);
    fr.dot(p, 8.6, -13.2 + bob, 0.4, Color32::from_rgb(20, 15, 10));
    if let Some(w) = carry {
        // saddle bags with the ware's picture
        let bag = Color32::from_rgb(124, 86, 48);
        fr.body(p, &[part(-2.2, -7.6 + bob, 2.4, 2.6, bag), part(2.0, -7.6 + bob, 2.4, 2.6, dark(bag, 1.08))]);
        crate::sprites::ware_icon(p, fr.v(-0.2, -12.6 + bob), 5.5 * z, w);
    }
}

/// A white goose of Juno's flock (the Roman campaign's holy places; drawn in the gallery until
/// the flock walks the border): a plump body, a long neck, an orange bill and orange legs.
pub fn goose(p: &Painter, at: Pos2, z: f32, (walking, west): (bool, bool), anim: u32) {
    let fr = Frame { at, z, f: facing(west) };
    let bob = if walking && anim % 2 == 0 { -0.4 } else { 0.0 };
    let white = Color32::from_rgb(236, 232, 222);
    let orange = Color32::from_rgb(232, 132, 36);
    p.add(Shape::ellipse_filled(fr.v(0.0, 0.0), vec2(3.2 * z, 0.8 * z), Color32::from_black_alpha(45)));
    // the legs waddle in turn
    for (i, x) in [-0.6f32, 0.8].into_iter().enumerate() {
        let step = if walking { ((anim as f32 + i as f32 * 2.0) * 1.6).sin() * 0.6 } else { 0.0 };
        fr.limb(p, &[(x, -1.6 + bob), (x + step, 0.0)], &[0.5, 0.4], orange, Some(orange));
    }
    fr.body(
        p,
        &[
            part(0.0, -3.0 + bob, 3.4, 1.9, white),
            part(-3.0, -3.6 + bob, 1.0, 0.7, white),
            part(2.6, -5.2 + bob, 0.8, 1.6, white),
            part(3.0, -7.4 + bob, 1.1, 0.95, white),
        ],
    );
    // a grey wing over the back
    fr.lit(p, part(-0.6, -3.4 + bob, 2.2, 1.0, Color32::from_rgb(196, 192, 184)));
    fr.tri(p, [(3.9, -7.8 + bob), (5.6, -7.2 + bob), (3.9, -6.8 + bob)], orange);
    fr.dot(p, 3.3, -7.7 + bob, 0.3, Color32::from_rgb(15, 10, 5));
}

/// A bird of omen in flight, high over the land (Jupiter's vultures, the ravens of a warning):
/// seen from below, both broad wings spread, their tips rising and falling with `flap` (0..1)
/// as it beats or glides; `west` turns its head.
pub fn omen_bird(p: &Painter, at: Pos2, z: f32, west: bool, flap: f32, color: Color32) {
    let fr = Frame { at, z, f: facing(west) };
    let lift = (flap * std::f32::consts::TAU).sin() * 1.6;
    let tip = dark(color, 0.8);
    // both wings: broad at the body, the fingered tips raised or lowered
    for side in [-1.0f32, 1.0] {
        let w = |x: f32, y: f32| (x * side, y);
        fr.limb(p, &[w(0.0, -0.2), w(3.0, -0.9 - lift * 0.4), w(6.0, -1.4 - lift)], &[2.2, 1.6, 0.9], color, None);
        for k in 0..3 {
            let y = -1.4 - lift + k as f32 * 0.5;
            fr.limb(p, &[w(5.6, y), w(7.2, y - 0.3 - lift * 0.2)], &[0.5, 0.25], tip, None);
        }
    }
    // a short fanned tail, the body and a small pale-billed head
    fr.tri(p, [(-0.5, 0.9), (0.5, 0.9), (0.0, 2.6)], tip);
    fr.body(p, &[part(0.0, 0.2, 0.9, 1.4, color), part(0.0, -1.5, 0.6, 0.6, dark(color, 1.2))]);
    fr.dot(p, 0.35, -1.9, 0.25, Color32::from_rgb(200, 170, 90));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Owner's notes: an animal walking west turns round (they all faced east).
    #[test]
    fn animals_turn_to_the_west() {
        assert_eq!(facing(true), -1.0);
        assert_eq!(facing(false), 1.0);
    }

    /// Graphics round 1: the sun is in the upper left, as on the pre-rendered buildings.
    #[test]
    fn bodies_are_lit_from_the_upper_left() {
        assert!(rim_light(vec2(-0.6, -0.8)) > rim_light(vec2(0.6, 0.8)));
        assert!(rim_light(vec2(-1.0, 0.0)) > rim_light(vec2(1.0, 0.0)));
        assert!((0.7..=1.13).contains(&rim_light(vec2(0.0, 1.0))));
    }
}
