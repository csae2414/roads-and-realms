//! What a carrier does when it has stood idle on its road for a while (owner, 2026-10-27: in
//! the original, settlers with nothing to do read or played). Our own small motions, drawn in
//! the figures' lit shapes: sitting, stretching, yawning, reading a scroll, juggling, eating
//! an apple, whistling, napping, looking out. Drawing only: the simulation doesn't know.

use crate::animals::{Frame, part};
use eframe::egui::{Color32, FontId, Painter, Pos2, Shape, Stroke, vec2};

/// How long a carrier stands before it starts (10 s), and how long one motion lasts (12 s).
pub const START_GF: u32 = 200;
pub const POSE_GF: u32 = 240;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pose {
    Sit,
    Stretch,
    Yawn,
    Read,
    Juggle,
    Eat,
    Whistle,
    Nap,
    Look,
}

pub const POSES: [Pose; 9] =
    [Pose::Sit, Pose::Stretch, Pose::Yawn, Pose::Read, Pose::Juggle, Pose::Eat, Pose::Whistle, Pose::Nap, Pose::Look];

/// The motion of a carrier idle for `idle_gf` frames, and how far into it (seconds), or none
/// yet. Each carrier (`seed`, from its id) goes its own way through the motions, so the
/// carriers of one road net don't all do the same.
pub fn pose_at(seed: u32, idle_gf: f32) -> Option<(Pose, f32)> {
    let t = idle_gf - START_GF as f32;
    if t < 0.0 {
        return None;
    }
    let k = (t / POSE_GF as f32) as u32;
    let h = seed.wrapping_mul(2_654_435_761) ^ k.wrapping_mul(40_503).rotate_left(7);
    let pose = POSES[(h % POSES.len() as u32) as usize];
    Some((pose, (t - (k * POSE_GF) as f32) / 20.0))
}

fn dark(c: Color32, k: f32) -> Color32 {
    let f = |x: u8| (x as f32 * k).min(255.0) as u8;
    Color32::from_rgb(f(c.r()), f(c.g()), f(c.b()))
}

fn lerp(a: (f32, f32), b: (f32, f32), k: f32) -> (f32, f32) {
    (a.0 + (b.0 - a.0) * k, a.1 + (b.1 - a.1) * k)
}

/// The pose's shape at `t` seconds: how far seated (0 standing .. 1 on the ground), where the
/// hands are, the head's offset and whether its eyes are closed.
struct Shape2 {
    sit: f32,
    far_hand: (f32, f32),
    near_hand: (f32, f32),
    head: (f32, f32),
    asleep: bool,
}

/// A carrier in its idle motion at `at` (its node's ground point), `t` seconds into it.
pub fn draw(p: &Painter, at: Pos2, z: f32, color: Color32, pose: Pose, t: f32, west: bool) {
    let secs = POSE_GF as f32 / 20.0;
    // into the motion and out of it again over most of a second, from and to standing
    let b = (t.min(secs - t) / 0.8).clamp(0.0, 1.0);
    let b = b * b * (3.0 - 2.0 * b);
    // looking out: turns from one side to the other every 2.5 s
    let west = if pose == Pose::Look && b > 0.5 { west ^ ((t / 2.5) as i32 % 2 == 1) } else { west };
    let fr = Frame { at, z, f: if west { -1.0 } else { 1.0 } };
    let s = shape(pose, t, b);
    let skin = Color32::from_rgb(232, 186, 146);
    let tunic = Color32::from_rgb(176, 146, 104);
    let trousers = Color32::from_rgb(86, 64, 40);
    let boot = Color32::from_rgb(52, 36, 22);
    let ink = Color32::from_rgba_unmultiplied(30, 20, 10, 150);
    let dy = 3.2 * s.sit;
    p.add(Shape::ellipse_filled(
        at + vec2(1.0 * z, 0.3 * z),
        vec2((4.2 + 1.6 * s.sit) * z, 1.4 * z),
        Color32::from_black_alpha(60),
    ));
    // legs: straight down when standing, stretched out in front when seated
    for (x, k) in [(-0.9, 0.75), (0.9, 1.0)] {
        let hip = (x, -6.0 + dy);
        let knee = lerp((x + 0.3, -3.0), (x + 2.4, -3.4), s.sit);
        let foot = lerp((x, -0.7), (x + 4.6, -0.8), s.sit);
        fr.limb(p, &[hip, knee, foot], &[1.9, 1.5, 1.3], dark(trousers, k), None);
        let toe = lerp((foot.0 + 1.1, -0.4), (foot.0 + 0.6, -2.0), s.sit);
        fr.limb(p, &[(foot.0 - 0.2, foot.1 + 0.1), toe], &[1.3, 1.0], dark(boot, k), None);
    }
    let arm = |sx: f32, hand: (f32, f32), k: f32| {
        let shoulder = (sx, -11.0 + dy);
        let elbow = ((sx + hand.0) / 2.0 + sx * 0.25, (shoulder.1 + hand.1) / 2.0 + 1.2);
        fr.limb(p, &[shoulder, elbow, hand], &[1.6, 1.35, 1.1], dark(tunic, k), None);
        fr.dot(p, hand.0, hand.1, 0.85, dark(skin, k));
    };
    arm(-2.3, s.far_hand, 0.7);
    fr.body(p, &[part(0.0, -6.4 + dy, 2.6, 1.4, dark(tunic, 0.9)), part(0.0, -9.3 + dy, 2.7, 3.0, tunic)]);
    let v = |x: f32, y: f32| fr.v(x, y);
    p.line_segment([v(-2.5, -6.3 + dy), v(2.5, -6.3 + dy)], Stroke::new(0.9 * z, dark(tunic, 0.5)));
    p.line_segment([v(-2.2, -11.6 + dy), v(2.3, -6.8 + dy)], Stroke::new(1.5 * z, ink));
    p.line_segment([v(-2.2, -11.6 + dy), v(2.3, -6.8 + dy)], Stroke::new(1.1 * z, color));
    arm(2.3, s.near_hand, 1.0);
    let (hx, hy) = (0.2 + s.head.0, -14.6 + dy + s.head.1);
    fr.body(p, &[part(hx, hy, 2.4, 2.5, skin)]);
    fr.dot(p, hx + 2.3, hy + 0.3, 0.55, dark(skin, 0.9));
    if s.asleep {
        p.line_segment(
            [v(hx + 0.8, hy - 0.3), v(hx + 1.6, hy - 0.3)],
            Stroke::new(0.35 * z, Color32::from_rgb(40, 28, 20)),
        );
    } else {
        fr.dot(p, hx + 1.2, hy - 0.4, 0.33, Color32::from_rgb(40, 28, 20));
    }
    fr.body(p, &[part(hx - 0.5, hy - 1.4, 2.4, 1.4, Color32::from_rgb(110, 72, 36))]);
    held(p, &fr, pose, t, b, dy);
    props_front(p, &fr, pose, t, b, (hx, hy));
}

/// Where the body and the hands are in a pose, `b` 0..1 into it from standing.
fn shape(pose: Pose, t: f32, b: f32) -> Shape2 {
    let rest = ((-2.0, -6.4), (2.6, -6.4));
    let (sit, far, near, head, asleep) = match pose {
        Pose::Sit => {
            let breathe = (t * 1.3).sin() * 0.15;
            (1.0, (2.2, -4.4 + breathe), (3.2, -4.2 + breathe), (0.0, breathe), false)
        }
        Pose::Nap => (1.0, (1.8, -4.6), (2.8, -4.4), (0.9, 1.4), b > 0.6),
        Pose::Read => {
            // a page turned every four seconds
            let flick = if (t % 4.0) > 3.6 { -0.8 } else { 0.0 };
            (1.0, (3.4, -6.6), (5.4, -6.8 + flick), (0.4, 0.6), false)
        }
        Pose::Stretch => {
            let up = (t * 1.1).sin() * 0.5 + 0.5;
            (0.0, (-1.4, -19.0 - up), (2.0, -19.4 - up), (-0.3, -0.4 * up), up > 0.7)
        }
        Pose::Yawn => (0.0, rest.0, (2.4, -13.6), (-0.3, -0.3), (t % 4.0) < 1.6),
        Pose::Juggle => {
            let a = (t * 2.2 * std::f32::consts::PI).sin();
            (0.0, (2.0, -9.0 + a * 0.6), (3.4, -9.0 - a * 0.6), (0.2, 0.0), false)
        }
        Pose::Eat => {
            let up = ((t * 2.1).sin() * 0.5 + 0.5).powf(3.0);
            let hand = lerp((3.0, -8.0), (2.8, -13.4), up);
            (0.0, rest.0, hand, (0.0, 0.0), false)
        }
        // hands behind the back, nodding to the tune
        Pose::Whistle => (0.0, (-2.8, -7.8), (-2.0, -8.0), (0.0, (t * 3.0).sin() * 0.25), false),
        // a hand shading the eyes
        Pose::Look => (0.0, rest.0, (2.6, -15.4), (0.0, 0.0), false),
    };
    Shape2 {
        sit: sit * b,
        far_hand: lerp(rest.0, far, b),
        near_hand: lerp(rest.1, near, b),
        head: lerp((0.0, 0.0), head, b),
        asleep,
    }
}

/// A scroll held up to read, or juggling balls: in front of the body and the head.
fn held(p: &Painter, fr: &Frame, pose: Pose, t: f32, b: f32, dy: f32) {
    if b < 0.5 {
        return;
    }
    match pose {
        Pose::Read => {
            let parchment = Color32::from_rgb(236, 222, 186);
            // (between the hands, held at the chest of the seated reader)
            let c = (4.4, -10.4 + dy);
            p.add(Shape::convex_polygon(
                vec![
                    fr.v(c.0 - 1.0, c.1 - 1.8),
                    fr.v(c.0 + 1.2, c.1 - 2.0),
                    fr.v(c.0 + 1.2, c.1 + 1.6),
                    fr.v(c.0 - 1.0, c.1 + 1.8),
                ],
                parchment,
                Stroke::new(0.3 * fr.z, Color32::from_rgb(120, 96, 60)),
            ));
            // two rolled ends and a few lines of writing
            for y in [c.1 - 1.9, c.1 + 1.7] {
                fr.limb(p, &[(c.0 - 1.2, y), (c.0 + 1.4, y)], &[0.6, 0.6], Color32::from_rgb(150, 110, 60), None);
            }
            for i in 0..3 {
                let y = c.1 - 0.9 + i as f32 * 0.8;
                p.line_segment(
                    [fr.v(c.0 - 0.6, y), fr.v(c.0 + 0.8, y)],
                    Stroke::new(0.2 * fr.z, Color32::from_rgb(90, 70, 50)),
                );
            }
        }
        Pose::Juggle => {
            let colors =
                [Color32::from_rgb(210, 60, 50), Color32::from_rgb(240, 200, 60), Color32::from_rgb(70, 120, 210)];
            for (i, c) in colors.into_iter().enumerate() {
                let u = (t * 1.1 + i as f32 / 3.0).fract();
                // (an arc from hand to hand in front of the face, up over the head)
                let x = 3.6 + 1.9 * (u * std::f32::consts::TAU).cos();
                let y = -9.5 - 10.0 * (u * std::f32::consts::PI).sin();
                fr.body(p, &[part(x, y, 0.8, 0.8, c)]);
            }
        }
        _ => {}
    }
}

/// An apple at the mouth, a yawn, notes of a tune, a sleeper's z: drawn over the head.
fn props_front(p: &Painter, fr: &Frame, pose: Pose, t: f32, b: f32, (hx, hy): (f32, f32)) {
    if b < 0.5 {
        return;
    }
    let z = fr.z;
    match pose {
        Pose::Eat => {
            let up = ((t * 2.1).sin() * 0.5 + 0.5).powf(3.0);
            let hand = lerp((3.0, -8.0), (2.8, -13.4), up);
            fr.dot(p, hand.0 + 0.5, hand.1 - 0.6, 0.9, Color32::from_rgb(196, 40, 36));
            fr.dot(p, hand.0 + 0.3, hand.1 - 1.0, 0.3, Color32::from_rgb(255, 160, 150));
            p.line_segment(
                [fr.v(hand.0 + 0.5, hand.1 - 1.4), fr.v(hand.0 + 1.0, hand.1 - 1.9)],
                Stroke::new(0.3 * z, Color32::from_rgb(60, 130, 40)),
            );
        }
        Pose::Yawn if (t % 4.0) < 1.6 => {
            p.add(Shape::ellipse_filled(
                fr.v(hx + 1.9, hy + 1.1),
                vec2(0.55 * z, 0.75 * z),
                Color32::from_rgb(90, 30, 30),
            ));
        }
        Pose::Whistle => {
            fr.dot(p, hx + 2.4, hy + 1.0, 0.3, Color32::from_rgb(90, 30, 30));
            // a note every 1.5 s, rising and fading over 2 s
            for k in 0..2 {
                let age = (t - k as f32 * 0.75).rem_euclid(1.5) / 1.5;
                let alpha = (255.0 * (1.0 - age)) as u8;
                let c = Color32::from_rgba_unmultiplied(40, 30, 20, alpha);
                let at = fr.v(hx + 3.0 + age * 2.0 + k as f32, hy - 1.5 - age * 6.0);
                p.add(Shape::ellipse_filled(at, vec2(0.55 * z, 0.4 * z), c));
                p.line_segment([at + vec2(0.5 * z, 0.0), at + vec2(0.5 * z, -2.0 * z)], Stroke::new(0.25 * z, c));
            }
        }
        Pose::Nap => {
            for k in 0..3 {
                let age = (t * 0.5 + k as f32 / 3.0).fract();
                let alpha = (230.0 * (1.0 - age)) as u8;
                let at = fr.v(hx + 2.0 + age * 3.0, hy - 2.5 - age * 7.0);
                p.text(
                    at,
                    eframe::egui::Align2::CENTER_CENTER,
                    "z",
                    FontId::proportional((2.4 + age * 1.6) * z),
                    Color32::from_rgba_unmultiplied(250, 250, 255, alpha),
                );
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A carrier waits 10 s before its first motion, each motion lasts 12 s, and carriers of
    /// one road net don't all do the same thing at once.
    #[test]
    fn idle_motions_start_late_and_vary() {
        assert!(pose_at(1, 199.0).is_none());
        let (_, t) = pose_at(1, 200.0).unwrap();
        assert_eq!(t, 0.0);
        let (_, t) = pose_at(1, 200.0 + 239.0).unwrap();
        assert!((t - 11.95).abs() < 1e-3, "{t}");
        let first: std::collections::BTreeSet<_> =
            (0..40u32).map(|s| format!("{:?}", pose_at(s, 300.0).unwrap().0)).collect();
        assert!(first.len() >= 6, "only {first:?}");
        // and one carrier changes what it does over time
        let own: std::collections::BTreeSet<_> =
            (0..12u32).map(|k| format!("{:?}", pose_at(7, 200.0 + k as f32 * 240.0).unwrap().0)).collect();
        assert!(own.len() >= 4, "only {own:?}");
    }
}
