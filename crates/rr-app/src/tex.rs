//! Procedural terrain textures (the open art pack's first terrain set). Tileable 128x128,
//! colors from the terrain table, detail from value noise.

use eframe::egui::{self, Color32, ColorImage, TextureHandle, TextureOptions};
use rr_core::Landscape;
use std::collections::HashMap;

const SIZE: usize = 256;

/// One full turn across the tile: sines in whole multiples of it repeat seamlessly.
const TAU_PX: f32 = std::f32::consts::TAU / SIZE as f32;

fn hash(x: u32, y: u32, s: u32) -> u32 {
    let mut h = x.wrapping_mul(374761393) ^ y.wrapping_mul(668265263) ^ s.wrapping_mul(2246822519);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    h ^ (h >> 16)
}

/// Tileable value noise, 0..1.
fn vnoise(x: f32, y: f32, period: u32, s: u32) -> f32 {
    let (ix, iy) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x - ix as f32, y - iy as f32);
    let p = period as i32;
    let v = |a: i32, b: i32| (hash(a.rem_euclid(p) as u32, b.rem_euclid(p) as u32, s) & 0xffff) as f32 / 65535.0;
    let sm = |t: f32| t * t * (3.0 - 2.0 * t);
    let (sx, sy) = (sm(fx), sm(fy));
    let a = v(ix, iy) + (v(ix + 1, iy) - v(ix, iy)) * sx;
    let b = v(ix, iy + 1) + (v(ix + 1, iy + 1) - v(ix, iy + 1)) * sx;
    a + (b - a) * sy
}

fn fbm(x: f32, y: f32, s: u32) -> f32 {
    fbm_p(x, y, s, 4)
}

/// `fbm` starting at another period (coarser below 4, finer above), still tileable: scaling the
/// coordinates instead left seams every tile (art director round 3, #4).
fn fbm_p(x: f32, y: f32, s: u32, start: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut period = start.max(1);
    for o in 0..4 {
        let k = period as f32 / SIZE as f32;
        sum += vnoise(x * k, y * k, period, s + o) * amp;
        amp *= 0.5;
        period *= 2;
    }
    sum / 0.9375
}

fn rgb(c: u32) -> [f32; 3] {
    [((c >> 16) & 0xff) as f32, ((c >> 8) & 0xff) as f32, (c & 0xff) as f32]
}

/// Texture style per terrain id.
#[derive(Clone, Copy)]
enum Style {
    Grass,
    Rock,
    Water,
    Snow,
    Sand,
    Lava,
    Swamp,
}

fn style(t: u8) -> Style {
    match t {
        1 | 11 | 12 | 13 | 34 => Style::Rock,
        5 | 6 | 19 => Style::Water,
        2 => Style::Snow,
        4 | 7 => Style::Sand,
        16 | 17 | 20 | 21 | 22 => Style::Lava,
        3 => Style::Swamp,
        _ => Style::Grass,
    }
}

pub fn make_image(landscape: Landscape, t: u8) -> ColorImage {
    let base = rgb(rr_world::terrain::color(landscape, t));
    // winter's meadows are snowfields
    let st = match (landscape, style(t)) {
        // (18 too: its grass blades on blue-grey read as denim; art director round 3, #4)
        (Landscape::Winter, Style::Grass) if matches!(t, 0 | 8 | 9 | 10 | 15 | 18) => Style::Snow,
        (_, s) => s,
    };
    let mut px: Vec<[f32; 3]> = Vec::with_capacity(SIZE * SIZE);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let (fx, fy) = (x as f32, y as f32);
            let n = fbm(fx, fy, t as u32 * 17 + 3);
            let fine = (hash(x as u32, y as u32, t as u32) & 0xff) as f32 / 255.0;
            let (mut r, mut g, mut b) = (base[0], base[1], base[2]);
            let k = match st {
                Style::Grass => {
                    // blades: fine speckle plus darker patches
                    let blade = if fine > 0.82 {
                        1.1
                    } else if fine < 0.12 {
                        0.9
                    } else {
                        1.0
                    };
                    // gentle broad variation: big dark patches read as fog or shadow
                    (0.88 + n * 0.2) * blade
                }
                Style::Rock => {
                    // greyer than the table's colour, with ridges (folded rock) and strata
                    let grey = (r + g + b) / 3.0;
                    (r, g, b) = (r * 0.75 + grey * 0.25, g * 0.75 + grey * 0.25, b * 0.75 + grey * 0.25 + 4.0);
                    let ridge = 1.0 - (fbm(fx, fy, t as u32 * 13 + 1) * 2.0 - 1.0).abs();
                    let strata = ((fy * 7.0 * TAU_PX + n * 4.0).sin() * 0.5 + 0.5) * 0.05;
                    // cool in the hollows, warm on the crests, dark crevices along the ridges
                    let warm = ridge - 0.5;
                    r *= 1.0 + warm * 0.1;
                    b *= 1.0 - warm * 0.1;
                    // short broken clefts, not closed contour loops (art director round 2, #4)
                    let sm = |a: f32, b: f32, x: f32| ((x - a) / (b - a)).clamp(0.0, 1.0);
                    let broken = fbm(fx + 53.0, fy + 11.0, t as u32 * 13 + 5) > 0.62;
                    let cleft = if broken { sm(0.93, 0.97, ridge) } else { 0.0 };
                    (0.7 + ridge * 0.45 + strata) * (1.0 - cleft * 0.18) * (0.93 + fine * 0.14)
                }
                Style::Water => {
                    // a calm sea darker than the land: deep teal, murky in the wasteland, steel in
                    // winter (art director round 2, #1)
                    (r, g, b) = match landscape {
                        Landscape::Greenland => (48.0, 96.0, 118.0),
                        Landscape::Wasteland => (74.0, 90.0, 82.0),
                        Landscape::Winter => (62.0, 85.0, 104.0),
                    };
                    // soft swells: noise pushed around by noise (no rows), plus a fine ripple;
                    // offsets keep it tileable
                    let warp = fbm(fx + 91.0, fy + 37.0, 71) * 40.0;
                    let swell = fbm(fx + warp, fy - warp * 0.5, 72);
                    let k = 32.0 / SIZE as f32;
                    let ripple = vnoise(fx * k * 2.0, fy * k * 2.0, 64, 73);
                    let v = swell * 0.7 + ripple * 0.3;
                    if fine > 0.997 && swell > 0.68 {
                        // a glint of sky
                        (r, g, b) = (r * 0.4 + 140.0, g * 0.4 + 150.0, b * 0.4 + 155.0);
                        1.0
                    } else {
                        0.9 + (v - 0.5) * 0.24
                    }
                }
                Style::Snow => {
                    // blue-grey hollows, crests that follow the rock beneath, a glitter here and
                    // there; never clipped to flat white
                    // (bright and calm: art director round 2, #7)
                    let hollow = (0.5 - n).max(0.0);
                    b += hollow * 30.0;
                    g += hollow * 6.0;
                    let ridge = 1.0 - (fbm_p(fx, fy, t as u32 * 13 + 1, 5) * 2.0 - 1.0).abs();
                    if fine > 0.99 {
                        1.06
                    } else {
                        (0.97 + n * 0.06 - hollow * 0.1 + (ridge - 0.5) * 0.05).clamp(0.92, 1.04)
                    }
                }
                Style::Sand => 0.88 + n * 0.22 + fine * 0.05,
                Style::Lava => {
                    let hot = (n * 7.0).fract();
                    if hot < 0.15 {
                        r = 255.0;
                        g = 200.0;
                        b = 60.0;
                    }
                    0.8 + n * 0.4
                }
                Style::Swamp => {
                    if fine > 0.9 {
                        r *= 0.5;
                        g *= 0.6;
                        b *= 1.6;
                    }
                    0.75 + n * 0.4
                }
            };
            // a broad change of hue across the tile, so meadows don't look printed
            let tint = fbm_p(fx, fy, t as u32 * 31 + 7, 2) - 0.5;
            if matches!(st, Style::Grass) {
                r *= 1.0 + tint * 0.15;
                g *= 1.0 + tint * 0.08;
                b *= 1.0 - tint * 0.2;
            }
            r = (r * k).clamp(0.0, 255.0);
            g = (g * k).clamp(0.0, 255.0);
            b = (b * k).clamp(0.0, 255.0);
            px.push([r, g, b]);
        }
    }
    detail(&mut px, st, t);
    // the wasteland's dry ground cracks
    if landscape == Landscape::Wasteland && matches!(st, Style::Grass) {
        for y in 0..SIZE {
            for x in 0..SIZE {
                let crack = wasteland_crack(x as f32, y as f32);
                if crack > 0.0 {
                    for c in px[y * SIZE + x].iter_mut() {
                        *c *= 1.0 - 0.18 * crack;
                    }
                }
            }
        }
    }
    ColorImage {
        size: [SIZE, SIZE],
        pixels: px.into_iter().map(|[r, g, b]| Color32::from_rgb(r as u8, g as u8, b as u8)).collect(),
    }
}

/// How much of a crack the wasteland's ground has at texel (x, y), 0 to 1.
/// A thin ridge of the noise, broken up by a second noise (the rock's crack, art director round
/// 2, #4): the old ridge formed closed loops as heavy as the roads (round 4, #8).
fn wasteland_crack(x: f32, y: f32) -> f32 {
    let ridge = 1.0 - (fbm_p(x, y, 991, 3) * 2.0 - 1.0).abs();
    let t = ((ridge - 0.985) / 0.01).clamp(0.0, 1.0);
    let line = t * t * (3.0 - 2.0 * t);
    if fbm_p(x * 2.0, y * 2.0, 517, 2) > 0.55 { line } else { 0.0 }
}

/// Soft dark clouds, seamless, for the land nobody has seen: a cool purple-grey that moves.
pub fn clouds_image() -> ColorImage {
    let mut px = Vec::with_capacity(SIZE * SIZE);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let n = fbm(x as f32, y as f32, 4242);
            let k = 0.55 + n * 0.9;
            px.push(Color32::from_rgb((42.0 * k) as u8, (38.0 * k) as u8, (52.0 * k) as u8));
        }
    }
    ColorImage { size: [SIZE, SIZE], pixels: px }
}

/// Planks running across, 256 × 64, seamless: grain, a seam between boards, a nail at the ends.
pub fn wood_image() -> ColorImage {
    const W: usize = 256;
    const H: usize = 64;
    let mut px = Vec::with_capacity(W * H);
    for y in 0..H {
        for x in 0..W {
            let (fx, fy) = (x as f32, y as f32);
            let board = y / 16;
            let grain = fbm(fx * 0.25, fy * 4.0 + board as f32 * 40.0, 77 + board as u32);
            let rings = ((fy * 0.9 + grain * 9.0).sin() * 0.5 + 0.5) * 0.12;
            let seam = y % 16 == 0 || y % 16 == 15;
            let nail = (x % 128 == 6 || x % 128 == 7) && (y % 16 == 7 || y % 16 == 8);
            let k = if seam {
                0.45
            } else if nail {
                0.35
            } else {
                0.78 + grain * 0.3 + rings
            };
            let tone = 1.0 + (board as f32 * 0.07) * if board % 2 == 0 { 1.0 } else { -1.0 };
            px.push(Color32::from_rgb(
                (92.0 * k * tone).clamp(0.0, 255.0) as u8,
                (64.0 * k * tone).clamp(0.0, 255.0) as u8,
                (38.0 * k * tone).clamp(0.0, 255.0) as u8,
            ));
        }
    }
    ColorImage { size: [W, H], pixels: px }
}

/// A road surface, 64 wide (across) by 128 long (along), seamless along: trodden dirt with
/// pebbles and two faint ruts, or cobbles laid in rows.
pub fn road_image(cobbled: bool) -> ColorImage {
    const W: usize = 64;
    const L: usize = 128;
    let mut px = Vec::with_capacity(W * L);
    for y in 0..L {
        for x in 0..W {
            let (fx, fy) = (x as f32, y as f32);
            let n = fbm(fx * 2.0, fy, 91);
            let fine = (hash(x as u32, y as u32, 7) & 0xff) as f32 / 255.0;
            let across = fx / W as f32;
            let (base, k) = if cobbled {
                // stones of 8×10 px, every other row shifted by half
                let row = y / 10;
                let sx = (x + if row % 2 == 1 { 4 } else { 0 }) % 8;
                let sy = y % 10;
                let joint = sx == 0 || sy == 0;
                let shade = 0.85 + (hash(((x + (row % 2) * 4) / 8) as u32, row as u32, 3) & 0x3f) as f32 / 255.0;
                ([140.0, 128.0, 112.0], if joint { 0.62 } else { shade * (0.95 + fine * 0.08) })
            } else {
                // worn pale in the middle, darker towards the verges, two soft ruts, pebbles
                // here and there (graphics round 2: hard rut lines read as planks)
                let mid = 1.0 - ((across - 0.5).abs() * 2.0).powi(2);
                let rut = (1.0 - ((across - 0.3).abs().min((across - 0.7).abs()) / 0.09)).max(0.0);
                let pebble = if fine > 0.97 {
                    1.2
                } else if fine < 0.03 {
                    0.8
                } else {
                    1.0
                };
                ([176.0, 140.0, 96.0], (0.8 + n * 0.28) * pebble * (0.9 + mid * 0.14) * (1.0 - rut * 0.08))
            };
            px.push(Color32::from_rgb(
                (base[0] * k).clamp(0.0, 255.0) as u8,
                (base[1] * k).clamp(0.0, 255.0) as u8,
                (base[2] * k).clamp(0.0, 255.0) as u8,
            ));
        }
    }
    ColorImage { size: [W, L], pixels: px }
}

/// Small strokes over the noise: grass blades and a few flowers, stones in rock, ripples in
/// sand. All wrap around the edges, so the tile stays seamless.
fn detail(px: &mut [[f32; 3]], st: Style, t: u8) {
    let at = |x: i32, y: i32| (y.rem_euclid(SIZE as i32) as usize) * SIZE + x.rem_euclid(SIZE as i32) as usize;
    let mut h = 0x9e37_79b9u32 ^ (t as u32).wrapping_mul(0x85eb_ca6b);
    let mut rnd = |n: u32| {
        h ^= h << 13;
        h ^= h >> 17;
        h ^= h << 5;
        h % n.max(1)
    };
    let scale = |p: &mut [f32; 3], k: f32| {
        for c in p.iter_mut() {
            *c = (*c * k).clamp(0.0, 255.0);
        }
    };
    match st {
        Style::Grass => {
            // blades: short strokes leaning a little, light on top, dark at the foot
            for _ in 0..2600 {
                let (x, y) = (rnd(SIZE as u32) as i32, rnd(SIZE as u32) as i32);
                let len = 3 + rnd(4) as i32;
                let lean = rnd(3) as i32 - 1;
                let light = rnd(3) != 0;
                for i in 0..len {
                    let k = if light { 1.08 + 0.05 * i as f32 / len as f32 } else { 0.84 };
                    scale(&mut px[at(x + lean * i / 2, y - i)], k);
                }
            }
            // tufts: darker round clumps
            for _ in 0..90 {
                let (x, y, r) = (rnd(SIZE as u32) as i32, rnd(SIZE as u32) as i32, 2 + rnd(3) as i32);
                for dy in -r..=r {
                    for dx in -r..=r {
                        if dx * dx + dy * dy <= r * r {
                            scale(&mut px[at(x + dx, y + dy)], 0.9);
                        }
                    }
                }
            }
            // a few flowers
            for _ in 0..30 {
                let i = at(rnd(SIZE as u32) as i32, rnd(SIZE as u32) as i32);
                px[i] = if rnd(2) == 0 { [235.0, 230.0, 200.0] } else { [230.0, 200.0, 70.0] };
            }
        }
        Style::Rock => {
            // stones: lit from the upper left, shadow to the lower right
            for _ in 0..320 {
                let (x, y, r) = (rnd(SIZE as u32) as i32, rnd(SIZE as u32) as i32, 2 + rnd(5) as i32);
                for dy in -r..=r {
                    for dx in -r..=r {
                        if dx * dx + dy * dy <= r * r {
                            let k = 1.0 + (-(dx + dy) as f32 / (2 * r) as f32) * 0.22;
                            scale(&mut px[at(x + dx, y + dy)], k);
                        }
                    }
                }
                for d in 1..=2 {
                    scale(&mut px[at(x + r / 2 + d, y + r / 2 + d)], 0.8);
                }
            }
        }
        Style::Sand => {
            // wind ripples
            for y in 0..SIZE as i32 {
                for x in 0..SIZE as i32 {
                    // (whole waves per tile, so the tile repeats without a seam)
                    let (fx, fy) = (x as f32 * TAU_PX, y as f32 * TAU_PX);
                    let w = ((fx * 9.0 + fy * 22.0 + (fx * 2.0).sin() * 3.0).sin() * 0.5 + 0.5) * 0.08;
                    scale(&mut px[at(x, y)], 0.96 + w);
                }
            }
        }
        _ => {}
    }
}

pub struct TerrainTextures {
    #[allow(dead_code)]
    pub landscape: Landscape,
    pub tex: HashMap<u8, TextureHandle>,
    /// Building sprites from `assets/art/buildings` (texture, width, height, anchor x, anchor y), 2x scale.
    pub buildings: HashMap<(rr_rules::BuildingType, u8), (TextureHandle, f32, f32, f32, f32)>,
    /// The Roman campaign's holy places from `assets/art/holy`, by god (`rr_world::HOLY_PLACES`):
    /// (texture, width, height, anchor x, anchor y), as the buildings.
    pub holy: HashMap<u8, (TextureHandle, f32, f32, f32, f32)>,
    /// Pre-rendered trees from `assets/art/trees` by kind (texture, width, height).
    pub trees: HashMap<&'static str, (TextureHandle, f32, f32)>,
    /// Small props and granite piles from `assets/art/props` by name (texture, width, height).
    pub props: HashMap<String, (TextureHandle, f32, f32)>,
    /// Field patches by growth stage 0-4: ploughed soil, sprouts, green, gold, stubble.
    pub fields: Vec<TextureHandle>,
    /// Dark clouds over unexplored land.
    pub clouds: TextureHandle,
    /// Wooden planks behind the bottom bar.
    pub wood: TextureHandle,
    /// Road surfaces: trodden dirt, and cobbles for donkey roads (u across, v along the road).
    pub road: TextureHandle,
    pub cobbles: TextureHandle,
}

fn load_trees(ctx: &egui::Context) -> HashMap<&'static str, (TextureHandle, f32, f32)> {
    let mut out = HashMap::new();
    let dir = rr_audio::assets_dir().join("art").join("trees");
    for key in crate::sprites::TREE_KINDS {
        let Ok(img) = image::open(dir.join(format!("{key}.png"))) else { continue };
        let img = img.to_rgba8();
        let (w, h) = (img.width() as f32, img.height() as f32);
        let ci = ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
        out.insert(key, (ctx.load_texture(format!("tree_{key}"), ci, TextureOptions::LINEAR), w, h));
    }
    out
}

/// A field's patch at a growth stage (art director round 2, #8): furrows along one diagonal,
/// soil between; sprouts, green blades, golden ears or stubble on the ridges. Tileable.
pub fn field_image(stage: u8) -> ColorImage {
    const N: usize = 128;
    let soil = [112.0, 84.0, 52.0];
    let crop = match stage {
        1 => [120.0, 160.0, 64.0],
        2 => [96.0, 150.0, 52.0],
        3 => [222.0, 182.0, 72.0],
        4 => [176.0, 148.0, 92.0],
        _ => soil,
    };
    // how much of a ridge the crop covers
    let cover = [0.0, 0.35, 0.8, 0.95, 0.5][stage.min(4) as usize];
    let mut px = Vec::with_capacity(N * N);
    for y in 0..N {
        for x in 0..N {
            // 8 furrows across the tile, along the diagonal
            let u = ((x + y) % (N / 8)) as f32 / (N / 8) as f32;
            let ridge = (u * std::f32::consts::TAU).sin() * 0.5 + 0.5;
            let n = (hash(x as u32, y as u32, 91 + stage as u32) & 0xff) as f32 / 255.0;
            let on = ridge * cover + (n - 0.5) * 0.3 > 1.0 - cover;
            let base = if on { crop } else { soil };
            let k = 0.78 + ridge * 0.3 + (n - 0.5) * 0.12;
            px.push(Color32::from_rgb(
                (base[0] * k).min(255.0) as u8,
                (base[1] * k).min(255.0) as u8,
                (base[2] * k).min(255.0) as u8,
            ));
        }
    }
    ColorImage { size: [N, N], pixels: px }
}

fn load_props(ctx: &egui::Context) -> HashMap<String, (TextureHandle, f32, f32)> {
    let mut out = HashMap::new();
    let dir = rr_audio::assets_dir().join("art").join("props");
    let Ok(text) = std::fs::read_to_string(dir.join("manifest.json")) else { return out };
    let Ok(m) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&text) else { return out };
    for key in m.keys() {
        let Ok(img) = image::open(dir.join(format!("{key}.png"))) else { continue };
        let img = img.to_rgba8();
        let (w, h) = (img.width() as f32, img.height() as f32);
        let ci = ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
        out.insert(key.clone(), (ctx.load_texture(format!("prop_{key}"), ci, TextureOptions::LINEAR), w, h));
    }
    out
}

fn load_holy(ctx: &egui::Context) -> HashMap<u8, (TextureHandle, f32, f32, f32, f32)> {
    let mut out = HashMap::new();
    let dir = rr_audio::assets_dir().join("art").join("holy");
    let Ok(text) = std::fs::read_to_string(dir.join("manifest.json")) else { return out };
    let Ok(m) = serde_json::from_str::<serde_json::Value>(&text) else { return out };
    for (god, name) in rr_world::HOLY_PLACES.iter().enumerate() {
        // (Janus's gates are drawn closed until his switch exists)
        let key = if *name == "janus" { "janus_closed" } else { name };
        let (Some(e), Ok(img)) = (m.get(key), image::open(dir.join(format!("{key}.png")))) else { continue };
        let img = img.to_rgba8();
        let ci = ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
        let opts = TextureOptions { mipmap_mode: Some(egui::TextureFilter::Linear), ..TextureOptions::LINEAR };
        let f = |k: &str| e.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
        out.insert(god as u8, (ctx.load_texture(format!("holy_{key}"), ci, opts), f("w"), f("h"), f("ax"), f("ay")));
    }
    out
}

fn load_buildings(
    ctx: &egui::Context,
    landscape: Landscape,
) -> HashMap<(rr_rules::BuildingType, u8), (TextureHandle, f32, f32, f32, f32)> {
    let mut out = HashMap::new();
    let dir = rr_audio::assets_dir().join("art").join("buildings");
    let Ok(text) = std::fs::read_to_string(dir.join("manifest.json")) else { return out };
    let Ok(m) = serde_json::from_str::<serde_json::Value>(&text) else { return out };
    for (b, nation) in rr_rules::BuildingType::ALL.iter().flat_map(|b| (0u8..4).map(move |n| (b, n))) {
        let base = b.name().to_lowercase();
        let key = if nation == 0 { base } else { format!("{base}_n{nation}") };
        // winter: the snowy picture where there is one
        let snowy = format!("{key}_w");
        let key = if landscape == Landscape::Winter && m.get(&snowy).is_some() { snowy } else { key };
        let Some(e) = m.get(&key) else { continue };
        let Ok(img) = image::open(dir.join(format!("{key}.png"))) else { continue };
        let img = img.to_rgba8();
        let ci = ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
        // (pictures carry more pixels than their size on the map: mipmaps keep them calm far away)
        let opts = TextureOptions { mipmap_mode: Some(egui::TextureFilter::Linear), ..TextureOptions::LINEAR };
        let tex = ctx.load_texture(format!("bld_{key}"), ci, opts);
        let f = |k: &str| e.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
        out.insert((*b, nation), (tex, f("w"), f("h"), f("ax"), f("ay")));
    }
    out
}

impl TerrainTextures {
    pub fn new(ctx: &egui::Context, landscape: Landscape) -> Self {
        let mut tex = HashMap::new();
        for t in (0u8..=22).chain([34]) {
            let img = make_image(landscape, t);
            tex.insert(t, ctx.load_texture(format!("terrain{t}"), img, TextureOptions::LINEAR_REPEAT));
        }
        let road = ctx.load_texture("road", road_image(false), TextureOptions::LINEAR_REPEAT);
        let cobbles = ctx.load_texture("cobbles", road_image(true), TextureOptions::LINEAR_REPEAT);
        let wood = ctx.load_texture("wood", wood_image(), TextureOptions::LINEAR_REPEAT);
        let clouds = ctx.load_texture("clouds", clouds_image(), TextureOptions::LINEAR_REPEAT);
        let fields = (0..5u8)
            .map(|s| ctx.load_texture(format!("field_{s}"), field_image(s), TextureOptions::LINEAR_REPEAT))
            .collect();
        Self {
            landscape,
            tex,
            buildings: load_buildings(ctx, landscape),
            holy: load_holy(ctx),
            trees: load_trees(ctx),
            props: load_props(ctx),
            fields,
            road,
            cobbles,
            wood,
            clouds,
        }
    }

    pub fn get(&self, t: u8) -> &TextureHandle {
        self.tex.get(&t).or_else(|| self.tex.get(&8)).expect("meadow texture")
    }
}

#[cfg(test)]
mod tests {
    /// Art director round 4, #8: the wasteland's cracks are short and broken, not closed loops
    /// as heavy as the roads.
    #[test]
    fn wasteland_cracks_are_broken_up() {
        let n = SIZE;
        let on: Vec<bool> = (0..n * n).map(|i| super::wasteland_crack((i % n) as f32, (i / n) as f32) > 0.3).collect();
        let mut seen = vec![false; n * n];
        let mut biggest = 0;
        for start in 0..n * n {
            if !on[start] || seen[start] {
                continue;
            }
            let (mut x0, mut x1, mut y0, mut y1) = (n, 0, n, 0);
            let mut stack = vec![start];
            seen[start] = true;
            while let Some(i) = stack.pop() {
                let (x, y) = (i % n, i / n);
                (x0, x1, y0, y1) = (x0.min(x), x1.max(x), y0.min(y), y1.max(y));
                for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx < 0 || ny < 0 || nx >= n as i32 || ny >= n as i32 {
                        continue;
                    }
                    let k = ny as usize * n + nx as usize;
                    if on[k] && !seen[k] {
                        seen[k] = true;
                        stack.push(k);
                    }
                }
            }
            biggest = biggest.max((x1 - x0).max(y1 - y0));
        }
        assert!(biggest < n / 4, "a crack network {biggest} texels across");
        assert!(on.iter().filter(|c| **c).count() > n * n / 400, "some cracks are left");
    }

    use super::*;

    /// Art director round 3, #4: every texture tiles: its first and last columns (and rows)
    /// match, or the ground shows straight seams every tile.
    #[test]
    fn every_texture_tiles() {
        let mut bad = Vec::new();
        for landscape in [Landscape::Greenland, Landscape::Wasteland, Landscape::Winter] {
            for t in [0u8, 1, 2, 3, 4, 5, 6, 8, 9, 10, 11, 12, 13, 14, 15, 18, 34] {
                let img = make_image(landscape, t);
                let px = |x: usize, y: usize| img.pixels[y * SIZE + x];
                let diff = |a: egui::Color32, b: egui::Color32| {
                    (a.r() as i32 - b.r() as i32).abs()
                        + (a.g() as i32 - b.g() as i32).abs()
                        + (a.b() as i32 - b.b() as i32).abs()
                };
                // the seam against two neighbouring columns inside (the grain differs pixel to pixel)
                let cols: i32 = (0..SIZE).map(|y| diff(px(0, y), px(SIZE - 1, y))).sum::<i32>() / SIZE as i32;
                let rows: i32 = (0..SIZE).map(|x| diff(px(x, 0), px(x, SIZE - 1))).sum::<i32>() / SIZE as i32;
                let inner: i32 = (0..SIZE).map(|y| diff(px(100, y), px(101, y))).sum::<i32>() / SIZE as i32;
                if cols > inner * 3 / 2 + 3 || rows > inner * 3 / 2 + 3 {
                    bad.push(format!("{landscape:?} {t}: seams {cols} / {rows}, inside {inner}"));
                }
            }
        }
        assert!(bad.is_empty(), "{bad:#?}");
    }

    /// Art director round 2, #1: a calm sea, not a saturated striped jersey, and its own water
    /// per landscape.
    #[test]
    fn the_sea_is_calm_and_not_striped() {
        let img = make_image(Landscape::Greenland, 5);
        let px = |x: usize, y: usize| img.pixels[y * SIZE + x];
        // mean saturation and lightness (HSL)
        let (mut sat, mut light) = (0.0, 0.0);
        for c in &img.pixels {
            let v = [c.r() as f32 / 255.0, c.g() as f32 / 255.0, c.b() as f32 / 255.0];
            let (mx, mn) = (v[0].max(v[1]).max(v[2]), v[0].min(v[1]).min(v[2]));
            let l = (mx + mn) / 2.0;
            light += l;
            sat += if mx == mn { 0.0 } else { (mx - mn) / (1.0 - (2.0 * l - 1.0).abs()) };
        }
        let n = img.pixels.len() as f32;
        assert!(sat / n <= 0.45, "saturation {:.2}", sat / n);
        assert!((0.25..=0.42).contains(&(light / n)), "lightness {:.2}", light / n);
        // no stripes: along a column the luminance doesn't repeat every 10-30 texels
        for x in [17, 100, 200] {
            let lum: Vec<f32> = (0..SIZE)
                .map(|y| {
                    let c = px(x, y);
                    c.r() as f32 + c.g() as f32 + c.b() as f32
                })
                .collect();
            let mean = lum.iter().sum::<f32>() / lum.len() as f32;
            let d: Vec<f32> = lum.iter().map(|v| v - mean).collect();
            let var: f32 = d.iter().map(|v| v * v).sum();
            // smooth swells correlate at short lags; stripes show as a peak further out
            let ac: Vec<f32> =
                (0..=40).map(|lag| (0..SIZE - lag).map(|i| d[i] * d[i + lag]).sum::<f32>() / var).collect();
            for lag in 10..=30 {
                let peak = ac[lag] > ac[lag - 1] && ac[lag] >= ac[lag + 1];
                assert!(!(peak && ac[lag] > 0.3), "column {x}: repeat at lag {lag} ({:.2})", ac[lag]);
            }
        }
        let mean = |l: Landscape| {
            let img = make_image(l, 5);
            let s = img
                .pixels
                .iter()
                .fold([0u32; 3], |a, c| [a[0] + c.r() as u32, a[1] + c.g() as u32, a[2] + c.b() as u32]);
            s.map(|v| v / img.pixels.len() as u32)
        };
        assert_ne!(mean(Landscape::Greenland), mean(Landscape::Winter));
        assert_ne!(mean(Landscape::Greenland), mean(Landscape::Wasteland));
    }
}

#[cfg(test)]
mod rock_tests {
    use super::*;

    /// Art director round 2, #4: mountains are pale stone, lighter than the meadow, not dark mud.
    #[test]
    fn mountains_are_lighter_than_meadow() {
        let mean_l = |t: u8| {
            let img = make_image(Landscape::Greenland, t);
            img.pixels
                .iter()
                .map(|c| {
                    let mx = c.r().max(c.g()).max(c.b()) as f32;
                    let mn = c.r().min(c.g()).min(c.b()) as f32;
                    (mx + mn) / 510.0
                })
                .sum::<f32>()
                / img.pixels.len() as f32
        };
        let (rock, meadow) = (mean_l(1), mean_l(8));
        assert!(rock >= 0.45, "rock lightness {rock:.2}");
        assert!(rock > meadow, "rock {rock:.2} vs meadow {meadow:.2}");
    }
}

#[cfg(test)]
mod snow_tests {
    use super::*;

    /// Art director round 2, #7: winter meadows are bright snow, not grey fog.
    #[test]
    fn snow_is_bright() {
        let img = make_image(Landscape::Winter, 8);
        let (mut l, mut s) = (0.0, 0.0);
        for c in &img.pixels {
            let v = [c.r() as f32 / 255.0, c.g() as f32 / 255.0, c.b() as f32 / 255.0];
            let (mx, mn) = (v[0].max(v[1]).max(v[2]), v[0].min(v[1]).min(v[2]));
            let li = (mx + mn) / 2.0;
            l += li;
            // chroma: HSL saturation blows up near white
            s += mx - mn;
        }
        let n = img.pixels.len() as f32;
        assert!(l / n >= 0.85, "snow lightness {:.2}", l / n);
        assert!(s / n <= 0.12, "snow chroma {:.2}", s / n);
    }
}
