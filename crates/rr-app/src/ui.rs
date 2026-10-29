//! Windows and panels in the spirit of the original: a bottom button bar, a context window
//! for the clicked node, and the economy/military settings windows.

use crate::app::{App, SPEEDS};
use eframe::egui::{self, Color32, RichText, Vec2};
use rr_core::{MapCoord, Resource};
use rr_rules::{BuildingType as B, Job, Size, Ware};
use rr_sim::{BState, Cmd};
use rr_world::{Bq, NodeObj};

#[derive(Default)]
pub struct Ui {
    pub action: Option<Action>,
    /// Buildings whose windows stay open beside the action window, oldest first (as in the
    /// original: opening another building leaves the last one's window open).
    pub kept: Vec<MapCoord>,
    /// The main-selection window (the original's middle button).
    pub main: bool,
    /// The save/load window.
    pub saves: bool,
    /// The building overview window.
    pub buildings: bool,
    /// Asking whether to give up.
    pub surrender: bool,
    /// Minimap zoom (0: not chosen yet).
    pub mm_zoom: u8,
    /// Minimap layers switched off.
    pub mm_hide_territory: bool,
    pub mm_hide_buildings: bool,
    pub inventory: bool,
    pub distribution: bool,
    pub transport: bool,
    pub military: bool,
    pub tools: bool,
    pub stats: bool,
    pub messages: bool,
    pub help: bool,
    pub options: bool,
    pub map: bool,
    pub stats_cat: usize,
    pub briefing: bool,
    /// The attack panel's choices (architect round 3, #7: kept in egui memory under names).
    pub attack: AttackPanel,
}

/// What the attack panel keeps between frames and attacks: the soldiers to send (`None`: all),
/// strong or weak ones, and the building whose truce question is open.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AttackPanel {
    pub count: Option<u8>,
    pub weak: bool,
    pub asking: Option<rr_sim::BuildingId>,
}

#[derive(Clone, Copy, Debug)]
pub struct Action {
    pub node: MapCoord,
    pub tab: usize,
}

/// A game window: never collapses (the original's don't), and each one has its own place
/// on the screen, so several open windows don't pile up in a corner.
fn win(title: impl Into<egui::WidgetText>) -> egui::Window<'static> {
    let title: egui::WidgetText = title.into();
    let h = title.text().bytes().fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32));
    let slot = (h % 7) as f32;
    // kept on the screen above the bar (a11y round 2, #3: at a big UI scale windows ran off the
    // bottom and over the bar's buttons)
    let w = egui::Window::new(title).collapsible(false).default_pos(egui::pos2(24.0 + slot * 70.0, 24.0 + slot * 36.0));
    match WINDOW_ROOM.with(|r| r.get()) {
        // (no scrolling here: a scrolling egui window no longer shrinks to its content, and a
        // flag's four buttons filled the screen's height; art director round 4, #1. The tall
        // windows scroll on their own: `tall`)
        Some(room) => w.constrain_to(room),
        None => w.constrain(true),
    }
}

/// A window whose content can be taller than the screen (help, messages, buildings,
/// statistics): it scrolls within the room above the bar (new-player round 10, #6).
fn tall(w: egui::Window<'static>) -> egui::Window<'static> {
    match WINDOW_ROOM.with(|r| r.get()) {
        Some(room) => w.max_height(room.height() - 40.0).vscroll(true),
        None => w.vscroll(true),
    }
}

thread_local! {
    /// The room the panels leave for windows this frame (set once the bar is laid out).
    static WINDOW_ROOM: std::cell::Cell<Option<egui::Rect>> = const { std::cell::Cell::new(None) };
}

/// [`beside`] for the window `id`, worked out once (when it opens, at the click) and kept:
/// recomputed from the moving pointer, the window would flee from it.
fn anchored(ctx: &egui::Context, id: egui::Id, pointer: egui::Pos2, screen: egui::Rect) -> (egui::Pos2, egui::Align2) {
    let key = id.with("anchor");
    ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(key, || beside(pointer, screen)))
}

/// Where a window about a clicked spot goes: beside it, never over it (egui only moves windows
/// up or down to keep them on the screen), and away from the middle of the screen when there is
/// room there, as the middle is usually the castle the next road goes to.
fn beside(click: egui::Pos2, screen: egui::Rect) -> (egui::Pos2, egui::Align2) {
    const ROOM: f32 = 45.0 + 330.0;
    // pinned at the top, just above the click, so the window grows downward when a tab with more
    // buildings opens; high enough that the tallest one (about 440) still fits above the bar
    let top = (click.y - 60.0).min(screen.bottom() - 48.0 - 440.0).max(screen.top() + 4.0);
    let right = (egui::pos2(click.x + 45.0, top), egui::Align2::LEFT_TOP);
    let left = (egui::pos2(click.x - 45.0, top), egui::Align2::RIGHT_TOP);
    let outward_right = click.x >= screen.center().x;
    match (outward_right, screen.right() - click.x >= ROOM, click.x - screen.left() >= ROOM) {
        (true, true, _) => right,
        (false, _, true) => left,
        (true, false, _) => left,
        (false, _, false) => right,
    }
}

pub fn setup_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    // wood-and-parchment tones of the original's windows
    style.visuals.window_fill = Color32::from_rgb(64, 48, 30);
    style.visuals.panel_fill = Color32::from_rgb(52, 38, 24);
    style.visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(96, 72, 44);
    style.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(130, 98, 60);
    style.visuals.widgets.active.weak_bg_fill = Color32::from_rgb(150, 115, 70);
    style.visuals.window_stroke = egui::Stroke::new(2.0_f32, Color32::from_rgb(150, 115, 60));
    style.visuals.override_text_color = Some(Color32::from_rgb(245, 230, 190));
    // headings (strong text) in the title's gold: the one font has no bold, and white beside
    // the cream looked the same (art director round 4, #6)
    style.visuals.widgets.active.fg_stroke.color = Color32::from_rgb(250, 205, 120);
    // faint text is the text tinted halfway to this: light enough for 4.5:1 on the windows
    // (a11y round 2, #10: about 3:1)
    style.visuals.widgets.noninteractive.weak_bg_fill = Color32::from_rgb(120, 104, 80);
    // gold, not egui's blue, for what is chosen; rounded framed windows; buttons with an edge
    style.visuals.selection.bg_fill = Color32::from_rgb(150, 108, 38);
    style.visuals.selection.stroke = egui::Stroke::new(1.0_f32, Color32::from_rgb(250, 220, 140));
    style.visuals.window_corner_radius = egui::CornerRadius::same(6);
    style.visuals.window_shadow =
        egui::epaint::Shadow { offset: [3, 5], blur: 14, spread: 0, color: Color32::from_black_alpha(110) };
    style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, Color32::from_rgb(128, 96, 56));
    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, Color32::from_rgb(210, 170, 100));
    style.visuals.hyperlink_color = Color32::from_rgb(240, 200, 110);
    // title bars and grid stripes in dark wood instead of grey
    style.visuals.widgets.open.weak_bg_fill = Color32::from_rgb(52, 38, 24);
    style.visuals.faint_bg_color = Color32::from_rgb(72, 54, 34);
    style.visuals.extreme_bg_color = Color32::from_rgb(36, 26, 16);
    // small print stays readable on laptop screens (egui's default is 9 px)
    style.text_styles.insert(egui::TextStyle::Small, egui::FontId::proportional(11.0));
    ctx.set_style(style);
}

pub fn building_name(b: B) -> &'static str {
    crate::i18n::tr(building_key(b))
}

/// A building's English name, the key of its translation.
pub fn building_key(b: B) -> &'static str {
    match b {
        // (one name everywhere: the hints say castle; new-player round 10, #8)
        B::Headquarters => "Castle",
        B::Barracks => "Barracks",
        B::Guardhouse => "Guardhouse",
        B::Watchtower => "Watchtower",
        B::Fortress => "Fortress",
        B::LookoutTower => "Lookout tower",
        B::Catapult => "Catapult",
        B::GraniteMine => "Granite mine",
        B::CoalMine => "Coal mine",
        B::IronMine => "Iron mine",
        B::GoldMine => "Gold mine",
        B::Woodcutter => "Woodcutter",
        B::Fishery => "Fishery",
        B::Quarry => "Quarry",
        B::Forester => "Forester",
        B::Slaughterhouse => "Slaughterhouse",
        B::Hunter => "Hunter",
        B::Brewery => "Brewery",
        B::Armory => "Armory",
        B::Metalworks => "Metalworks",
        B::IronSmelter => "Iron smelter",
        B::PigFarm => "Pig farm",
        B::Storehouse => "Storehouse",
        B::Mill => "Mill",
        B::Bakery => "Bakery",
        B::Sawmill => "Sawmill",
        B::Mint => "Mint",
        B::Well => "Well",
        B::Shipyard => "Shipyard",
        B::Farm => "Farm",
        B::DonkeyBreeder => "Donkey breeder",
        B::Harbor => "Harbor",
    }
}

pub fn ware_name(w: Ware) -> String {
    crate::i18n::tr(&ware_name_en(w)).to_string()
}

pub(crate) use rr_game::ware_name_en;

pub fn job_name(j: Job) -> String {
    // a few jobs are spelled like their building in English, not in German
    let en = job_name_en(j);
    let as_job = format!("{en} (job)");
    let de = crate::i18n::tr(&as_job);
    if de != as_job { de.to_string() } else { crate::i18n::tr(&en).to_string() }
}

fn job_name_en(j: Job) -> String {
    let n = j.name();
    let mut out = String::new();
    for (i, c) in n.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push(' ');
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// A resource's English name and amount, if it has one worth telling.
pub fn resource_key(r: Resource) -> (&'static str, Option<u8>) {
    match r {
        Resource::Coal(a) => ("coal", Some(a)),
        Resource::Iron(a) => ("iron ore", Some(a)),
        Resource::Gold(a) => ("gold", Some(a)),
        Resource::Granite(a) => ("granite", Some(a)),
        Resource::Water => ("water", None),
        Resource::Fish(_) => ("fish", None),
        Resource::None => ("nothing", None),
    }
}

/// Why a building that found nothing to do is idle (window line and message).
pub fn idle_reason(kind: B) -> &'static str {
    match kind {
        B::Woodcutter => "No trees in reach: build a forester nearby.",
        B::Quarry => {
            "No stone in reach: tear it down and build a quarry by other rocks, or a granite mine in the mountains (send a geologist first; miners need food)."
        }
        B::Fishery => "No fish in reach.",
        B::Hunter => "No game in reach.",
        B::Farm => "No room for fields in reach.",
        B::Forester => "No room to plant trees in reach.",
        // (a mine never quite stops: its finds grow rare)
        B::GraniteMine | B::CoalMine | B::IronMine | B::GoldMine => {
            "Its ore is running low: it finds something ever less often. Build a new mine where a geologist finds more."
        }
        _ => "Nothing to do in reach.",
    }
}

/// The game speed as a multiple of normal (50 ms per frame), e.g. "1.5×".
fn speed_label(ms: f64) -> String {
    let x = format!("{:.2}", 50.0 / ms);
    format!("{}×", x.trim_end_matches('0').trim_end_matches('.'))
}

/// Whether the bar needs a second row for the clock and speed: under 900 points of width.
fn two_bar_rows(width_points: f32) -> bool {
    width_points < 900.0
}

/// The bar's clock, truce and speed controls (laid out right to left): the clock and the
/// buttons first, then the status lines in the room they leave, cut short with the whole line on
/// hover (German round 1, #2: in German they ran under the speed buttons and the icons).
fn time_controls(app: &mut App, ui: &mut egui::Ui) {
    ui.label(RichText::new(fmt_time(app.sim.gf().0)).monospace());
    let pause = crate::keys::keyed(crate::i18n::tr("Pause (P)"), egui::Key::P, app.keys.key(crate::keys::Act::Pause));
    if ui.button(if app.paused { "▶" } else { "⏸" }).on_hover_text(pause).clicked() {
        app.toggle_pause();
    }
    if ui.button(crate::i18n::tr("−")).on_hover_text(crate::i18n::tr("Slower")).clicked() {
        app.speed = app.speed.saturating_sub(1);
    }
    ui.label(speed_label(SPEEDS[app.speed]));
    if ui.button(crate::i18n::tr("+")).on_hover_text(crate::i18n::tr("Faster")).clicked() {
        app.speed = (app.speed + 1).min(SPEEDS.len() - 1);
    }
    if app.net.is_none()
        && ui
            .selectable_label(app.session.skip.is_some(), "⏩")
            .on_hover_text(crate::i18n::tr("Fast until something needs you: an attack, a loss, the truce, a pause"))
            .clicked()
    {
        app.skip_quiet();
    }
    for (text, color, hover) in status_lines(app) {
        // (no room left: the line waits for a wider window, rather than covering a button)
        if ui.available_width() < 48.0 {
            break;
        }
        // (a frameless button: Tab reaches it and shows the whole line, as the bar's icons do;
        // accessibility round 3, #5: a cut goal could only be read by hovering)
        let whole = if hover.is_empty() { text.clone() } else { format!("{text}\n{hover}") };
        let r = ui.add(egui::Button::new(RichText::new(&text).color(color)).frame(false).truncate());
        if r.has_focus() {
            r.show_tooltip_text(whole);
        } else {
            r.on_hover_text(whole);
        }
    }
}

/// A replay's speed: slower, the speed, faster, and as fast as possible, which brings back the
/// normal speed when pressed again (owner, 2026-10-27: once fast, there was no way back); and
/// the whole map without fog.
fn replay_controls(app: &mut App, ui: &mut egui::Ui) {
    let fastest = SPEEDS.len() - 1;
    if ui.button(crate::i18n::tr("−")).on_hover_text(crate::i18n::tr("Slower")).clicked() {
        app.speed = app.speed.saturating_sub(1);
    }
    ui.label(speed_label(SPEEDS[app.speed]));
    if ui.button(crate::i18n::tr("+")).on_hover_text(crate::i18n::tr("Faster")).clicked() {
        app.speed = (app.speed + 1).min(fastest);
    }
    let fast = app.speed == fastest;
    let hover = if fast { "Back to normal speed" } else { "As fast as possible" };
    if ui.selectable_label(fast, "⏩").on_hover_text(crate::i18n::tr(hover)).clicked() {
        app.speed = if fast { crate::app::NORMAL_SPEED } else { fastest };
        app.paused = false;
    }
    // the whole map, not only what the recorded player saw
    let reveal = ui
        .selectable_label(app.replay_reveal, crate::i18n::tr("👁 Show all"))
        .on_hover_text(crate::i18n::tr("The whole map: every realm's land, buildings and people"));
    if reveal.clicked() {
        app.replay_reveal = !app.replay_reveal;
        app.minimap = None;
    }
}

/// The bar's status lines, the most urgent first: the truce or peace, the goal, the time left.
fn status_lines(app: &App) -> Vec<(String, Color32, String)> {
    let gf = app.sim.gf().0;
    let gold = Color32::from_rgb(250, 205, 120);
    let mut out = Vec::new();
    // (a follow says it's on, and how it ends: a11y round 3, #7)
    if let Some(f) = app.session.follow.and_then(|f| app.sim.figures().get(f)) {
        out.push((
            crate::i18n::show(&crate::i18n::msg("Following: {1} (Esc stops)", &[&job_name(f.job)])),
            Color32::from_rgb(180, 210, 250),
            crate::i18n::tr("The view goes with them; moving the map stops it too").to_string(),
        ));
    }
    if let Some(until) = app.truce_until().filter(|u| *u > gf) {
        out.push((
            format!("{} {}", crate::i18n::tr("Truce"), fmt_time(until - gf)),
            Color32::LIGHT_GREEN,
            crate::i18n::tr("Until then your rivals leave you alone").to_string(),
        ));
    }
    // a free game's peace time and time limit (game design round 1, #6); not while watching a
    // replay, which keeps no options (its commands are as filtered; net round 5, #6)
    let watching = app.playback.is_some();
    if !watching && app.args.peace == crate::app::PEACE_FOR_GOOD && app.peace_until() > gf {
        out.push((
            crate::i18n::tr("Peace for good").to_string(),
            Color32::LIGHT_GREEN,
            crate::i18n::tr("Nobody attacks in this game").to_string(),
        ));
    } else if !watching && app.peace_until() > gf {
        out.push((
            format!("{} {}", crate::i18n::tr("Peace"), fmt_time(app.peace_until() - gf)),
            Color32::LIGHT_GREEN,
            crate::i18n::tr("Until then nobody can attack").to_string(),
        ));
    }
    // the mission's goal and how far it is (game design round 1, #1: it showed only in the
    // briefing); after it, how many rivals are left
    if let Some(o) = app.goal() {
        let obj = if app.session.goal_done { crate::missions::Objective::DefeatAll } else { o };
        let p = crate::missions::progress(obj, &app.sim, app.player);
        out.push((crate::i18n::show(&p.line), gold, String::new()));
    }
    if app.args.coins_goal > 0 && !watching {
        let have = crate::app::coins_minted(&app.sim).get(app.player as usize).copied().unwrap_or(0);
        let line = crate::i18n::msg(
            "Goal: mint {1} gold coins first ({2} so far)",
            &[&app.args.coins_goal.to_string(), &have.to_string()],
        );
        out.push((crate::i18n::show(&line), gold, String::new()));
    }
    if app.args.time_limit > 0 && !watching {
        let left = crate::app::minutes(app.args.time_limit).saturating_sub(gf);
        out.push((
            format!("{} {}", crate::i18n::tr("Time left"), fmt_time(left)),
            Color32::WHITE,
            crate::i18n::tr("Then the side with the most land wins").to_string(),
        ));
    }
    out
}

/// The end screen's column `c`: the statistics' categories, but the gold column counts the
/// coins minted (in store it read 0-4 for every realm: coins go to promotions as they come;
/// game design round 1, #4).
fn end_heading(c: usize) -> &'static str {
    if rr_sim::Sample::CATEGORIES[c] == "Gold" { "Coins minted" } else { rr_sim::Sample::CATEGORIES[c] }
}

/// The briefing's word on the truce binding both sides (one rival has no allies: new-player
/// round 12, #9).
fn truce_both_ways(rivals: usize) -> &'static str {
    if rivals == 1 {
        "The truce holds both ways: if you attack your rival, it fights back at once."
    } else {
        "The truce holds both ways: if you attack a rival, it and its allies fight back at once."
    }
}

/// The end screen's table: every player's last numbers.
fn end_table(app: &App, ui: &mut egui::Ui) {
    egui::Grid::new("final").striped(true).show(ui, |ui| {
        ui.label("");
        for c in 0..rr_sim::Sample::CATEGORIES.len() {
            ui.label(RichText::new(crate::i18n::tr(end_heading(c))).strong());
        }
        ui.end_row();
        let land = app.sim.land_by_player();
        for (i, p) in app.sim.players().iter().enumerate() {
            ui.label(RichText::new(&p.setup.name).color(crate::render::player_text_color(i as u8)));
            for c in 0..rr_sim::Sample::CATEGORIES.len() {
                ui.label(end_value(p, c, land[i]).to_string());
            }
            ui.end_row();
        }
    });
}

/// (land as the verdict counts it, live: the last sample lagged up to 30 s, and the time limit's
/// "AI 1 holds the most land" stood over a table giving us more; new-player round 12, #1)
fn end_value(p: &rr_sim::Player, c: usize, land: u32) -> u32 {
    if rr_sim::Sample::CATEGORIES[c] == "Gold" {
        p.stats.produced_by_ware[rr_rules::Ware::Coins.index()]
    } else if rr_sim::Sample::CATEGORIES[c] == "Land" {
        land
    } else {
        p.stats.samples.last().copied().unwrap_or_default().value(c)
    }
}

pub fn fmt_time(gf: u32) -> String {
    let s = gf as u64 * 50 / 1000;
    format!("{:02}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
}

/// The unread news in the post office, up to 99 (answers to the player's own actions aren't).
fn unread_news(app: &App) -> usize {
    let read = app.session.read_until;
    app.messages.iter().filter(|m| m.gf > read && m.kind != crate::app::MSG_GENERAL).count().min(99)
}

#[allow(clippy::too_many_lines)] // to split (architect round 2, #8: no new long ones)
pub fn panels(app: &mut App, ctx: &egui::Context) {
    egui::TopBottomPanel::bottom("bar").show(ctx, |ui| {
        // wooden planks behind the buttons, a gold line on top
        if let Some(t) = app.tex.as_ref() {
            let r = ui.max_rect().expand2(Vec2::new(8.0, 4.0));
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(r.width() / 256.0, r.height() / 64.0));
            ui.painter().image(t.wood.id(), r, uv, Color32::WHITE);
            ui.painter().line_segment(
                [r.left_top(), r.right_top()],
                egui::Stroke::new(2.0_f32, Color32::from_rgb(170, 130, 64)),
            );
        }
        // the original's bar: map, building help, main selection, post office
        // the name shows on hover, and on keyboard focus too (Tab)
        let big = |ui: &mut egui::Ui, on: bool, icon: &str, tip: &str| -> bool {
            let r =
                ui.add(egui::Button::new(RichText::new(icon).size(22.0)).selected(on).min_size(Vec2::new(46.0, 34.0)));
            if r.has_focus() {
                r.show_tooltip_text(tip);
                r.clicked()
            } else {
                r.on_hover_text(tip).clicked()
            }
        };
        ui.horizontal(|ui| {
            // the four icons stay centred whatever else the bar shows
            let left = ui.min_rect().width();
            ui.add_space((ui.ctx().screen_rect().width() / 2.0 - 110.0 - left).max(0.0));
            // (the labels name the keys as the player set them: a11y round 3, #3)
            let key = |a: crate::keys::Act| app.keys.key(a);
            let keyed = |t: &str, d: egui::Key, a: crate::keys::Act| crate::keys::keyed(crate::i18n::tr(t), d, key(a));
            let (map_tip, bq_tip, main_tip, post_tip) = (
                keyed("Overview map (L)", egui::Key::L, crate::keys::Act::Map),
                keyed("Show where buildings fit (Space)", egui::Key::Space, crate::keys::Act::BuildHelp),
                keyed("Main selection (M)", egui::Key::M, crate::keys::Act::Main),
                keyed("Messages (N)", egui::Key::N, crate::keys::Act::Messages),
            );
            if big(ui, app.ui.map, "🗺", &map_tip) {
                app.ui.map = !app.ui.map;
            }
            if big(ui, app.show_bq, "🏠", &bq_tip) {
                app.show_bq = !app.show_bq;
            }
            if big(ui, app.ui.main, "☰", &main_tip) {
                app.ui.main = !app.ui.main;
            }
            // news only: answers to the player's own actions (saved, loaded, not possible) aren't
            let unread = unread_news(app);
            let post = if unread > 0 && !app.ui.messages { format!("✉{unread}") } else { "✉".to_string() };
            if big(ui, app.ui.messages, &post, &post_tip) {
                app.ui.messages = !app.ui.messages;
            }
            if !two_bar_rows(ui.ctx().screen_rect().width()) {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| time_controls(app, ui));
            }
        });
        // a narrow screen (or a big UI scale): the clock and speed on a row of their own, not
        // on top of the icons (a11y round 2, #3)
        if two_bar_rows(ui.ctx().screen_rect().width()) {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| time_controls(app, ui));
        }
    });
    WINDOW_ROOM.with(|r| r.set(Some(ctx.available_rect())));
    // watching a replay: where we are, and the controls
    if let Some((rep, _)) = app.playback.as_ref() {
        let (now, end) = (app.sim.gf().0, rep.final_gf.max(1));
        let mut menu = false;
        egui::Area::new(egui::Id::new("replay_bar")).anchor(egui::Align2::CENTER_TOP, Vec2::new(0.0, 8.0)).show(
            ctx,
            |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("⏵ {}", crate::i18n::tr("Replay"))).color(Color32::LIGHT_BLUE));
                        ui.add(egui::ProgressBar::new(now as f32 / end as f32).desired_width(220.0).text(format!(
                            "{} / {}",
                            fmt_time(now),
                            fmt_time(end)
                        )));
                        if ui.button(if app.paused { "▶" } else { "⏸" }).clicked() {
                            app.paused = !app.paused;
                        }
                        replay_controls(app, ui);
                        // jump ahead: as fast as the machine runs, the picture follows
                        if ui.button("+10 min").on_hover_text(crate::i18n::tr("Skip ten minutes")).clicked() {
                            app.replay_skip = Some(now + crate::app::minutes(10));
                            app.paused = false;
                        }
                        if ui.button("⏭").on_hover_text(crate::i18n::tr("To the end")).clicked() {
                            app.replay_skip = Some(end);
                            app.paused = false;
                        }
                        if app.replay_skip.is_some() {
                            ui.spinner();
                        }
                        if ui.button(crate::i18n::tr("🚪 Menu")).clicked() {
                            menu = true;
                        }
                    });
                });
            },
        );
        if menu {
            app.playback = None;
            app.in_menu = true;
        }
    }
    if app.waiting {
        egui::Area::new(egui::Id::new("net_wait")).anchor(egui::Align2::CENTER_TOP, Vec2::new(0.0, 12.0)).show(
            ctx,
            |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.label(RichText::new(crate::i18n::tr("Waiting for the other players…")).color(Color32::YELLOW));
                });
            },
        );
    }
    toast(app, ctx);
    first_steps_panel(app, ctx);
    bq_legend(app, ctx);
    // road building: a hint floating above the bar
    if app.session.road.is_some() {
        egui::Area::new(egui::Id::new("road_hint")).anchor(egui::Align2::CENTER_BOTTOM, Vec2::new(0.0, -52.0)).show(
            ctx,
            |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    if let Some(h) = &app.road_hint {
                        ui.label(RichText::new(h).color(Color32::from_rgb(255, 110, 90)));
                    }
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(crate::i18n::tr("Building a road: click nodes, a flag to finish"))
                                .color(Color32::YELLOW),
                        );
                        if ui.button(crate::i18n::tr("Flag here")).clicked() {
                            app.finish_road();
                        }
                        if ui.button(crate::i18n::tr("Cancel")).clicked() {
                            app.session.road = None;
                        }
                    });
                });
            },
        );
    }
    // main selection: everything else, as in the original
    let mut open = app.ui.main;
    // just opened: Tab starts in it, not at the bar (a11y round 3, #8)
    let was_open = egui::Id::new("main_was_open");
    let focus_first = open && !ctx.data(|d| d.get_temp::<bool>(was_open).unwrap_or(false));
    ctx.data_mut(|d| d.insert_temp(was_open, open));
    win(crate::i18n::tr("Main selection"))
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_BOTTOM, Vec2::new(0.0, -54.0))
        .show(ctx, |ui| {
            let mut first = focus_first;
            let mut toggle = |ui: &mut egui::Ui, on: &mut bool, label: &str, tip: &str| {
                let r = ui.add_sized([150.0, 28.0], egui::SelectableLabel::new(*on, label));
                if std::mem::take(&mut first) {
                    r.request_focus();
                }
                if r.on_hover_text(tip).clicked() {
                    *on = !*on;
                }
            };
            egui::Grid::new("main_sel").num_columns(3).show(ui, |ui| {
                toggle(
                    ui,
                    &mut app.ui.inventory,
                    crate::i18n::tr("📦 Inventory"),
                    crate::i18n::tr("Wares and settlers in your warehouses"),
                );
                toggle(
                    ui,
                    &mut app.ui.distribution,
                    crate::i18n::tr("⚖ Distribution"),
                    crate::i18n::tr("Which buildings get scarce wares"),
                );
                toggle(
                    ui,
                    &mut app.ui.transport,
                    crate::i18n::tr("🚚 Transport"),
                    crate::i18n::tr("Order in which carriers move wares"),
                );
                ui.end_row();
                toggle(
                    ui,
                    &mut app.ui.tools,
                    crate::i18n::tr("🔨 Tools"),
                    crate::i18n::tr("What the metalworks makes"),
                );
                toggle(ui, &mut app.ui.military, crate::i18n::tr("⚔ Military"), crate::i18n::tr("Military settings"));
                toggle(ui, &mut app.ui.stats, crate::i18n::tr("📈 Statistics"), crate::i18n::tr("Compare the players"));
                ui.end_row();
                toggle(
                    ui,
                    &mut app.show_prod,
                    crate::i18n::tr("% Productivity"),
                    crate::i18n::tr("Productivity and construction progress (O)"),
                );
                toggle(ui, &mut app.ui.options, crate::i18n::tr("⚙ Options"), crate::i18n::tr("Sound and display"));
                toggle(ui, &mut app.ui.help, crate::i18n::tr("❓ Help"), crate::i18n::tr("Controls"));
                ui.end_row();
                toggle(
                    ui,
                    &mut app.ui.buildings,
                    crate::i18n::tr("🏠 Buildings"),
                    crate::i18n::tr("How many of each building you have"),
                );
                let mut surrender = false;
                toggle(ui, &mut surrender, crate::i18n::tr("⚑ Give up"), crate::i18n::tr("Give up this game"));
                if surrender {
                    app.ui.surrender = true;
                }
                toggle(
                    ui,
                    &mut app.show_names,
                    crate::i18n::tr("🏷 Names"),
                    crate::i18n::tr("Building names on the map (C)"),
                );
                ui.end_row();
                if ui
                    .add_sized([150.0, 28.0], egui::Button::new(crate::i18n::tr("💾 Save / load")))
                    .on_hover_text(crate::i18n::tr("Quick save F5, quick load F9"))
                    .clicked()
                {
                    app.ui.saves = !app.ui.saves;
                }
                let can_save = crate::app::save_refused(app.playback.is_some()).is_none();
                if ui
                    .add_enabled(
                        can_save,
                        egui::Button::new(crate::i18n::tr("⚡ Quick save")).min_size(Vec2::new(150.0, 28.0)),
                    )
                    .on_hover_text("F5")
                    .clicked()
                {
                    app.quick_save();
                }
                if ui.add_sized([150.0, 28.0], egui::Button::new(crate::i18n::tr("🚪 Menu"))).clicked() {
                    app.in_menu = true;
                }
                ui.end_row();
            });
        });
    app.ui.main = open;
    let mut open = app.ui.saves;
    win(crate::i18n::tr("Save / load")).open(&mut open).resizable(false).show(ctx, |ui| saves(app, ui));
    app.ui.saves = open;
}

fn saves(app: &mut App, ui: &mut egui::Ui) {
    let id = egui::Id::new("save_name");
    let mut name = ui.data(|d| d.get_temp::<String>(id)).unwrap_or_default();
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut name).hint_text(crate::i18n::tr("Name")).desired_width(180.0));
        let can_save = crate::app::save_refused(app.playback.is_some()).is_none();
        if ui.add_enabled(can_save, egui::Button::new(crate::i18n::tr("💾 Save"))).clicked() && !name.trim().is_empty()
        {
            app.save_named(&name);
        }
    });
    ui.data_mut(|d| d.insert_temp(id, name));
    ui.separator();
    let list = App::list_saves();
    if list.is_empty() {
        ui.label(RichText::new(crate::i18n::tr("No saved games yet.")).weak());
    }
    let mut load = None;
    egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
        for e in list {
            ui.horizontal(|ui| {
                if ui.button(crate::i18n::tr("📂 Load")).clicked() {
                    load = Some(e.path.clone());
                }
                ui.label(&e.name);
                if let Some(m) =
                    crate::app::save_mission(&e.path, e.modified).as_deref().and_then(crate::missions::mission)
                {
                    ui.label(RichText::new(crate::i18n::tr(m.title)).small().strong());
                }
                ui.label(RichText::new(e.info()).small());
            });
        }
    });
    if let Some(p) = load {
        app.load_from(&p);
        app.ui.saves = false;
    }
}

/// Right-click on a window closes it (as in the original). Windows are areas whose layer id is
/// the window id: their title, or the action window's own id.
fn right_click_close(app: &mut App, ctx: &egui::Context) {
    let Some(pos) = ctx.input(|i| {
        (i.pointer.secondary_released() && !i.pointer.is_decidedly_dragging())
            .then(|| i.pointer.interact_pos())
            .flatten()
    }) else {
        return;
    };
    let Some(layer) = ctx.layer_id_at(pos) else { return };
    type Flag = fn(&mut App) -> &mut bool;
    let windows: [(&str, Flag); 13] = [
        ("Inventory", |a| &mut a.ui.inventory),
        ("Distribution of goods", |a| &mut a.ui.distribution),
        ("Transport", |a| &mut a.ui.transport),
        ("Tools", |a| &mut a.ui.tools),
        ("Military", |a| &mut a.ui.military),
        ("Statistics", |a| &mut a.ui.stats),
        ("Messages", |a| &mut a.ui.messages),
        ("Map", |a| &mut a.ui.map),
        ("Options", |a| &mut a.ui.options),
        ("Help", |a| &mut a.ui.help),
        ("Main selection", |a| &mut a.ui.main),
        ("Save / load", |a| &mut a.ui.saves),
        ("Buildings", |a| &mut a.ui.buildings),
    ];
    for (title, flag) in windows {
        if layer.id == egui::Id::new(crate::i18n::tr(title)) {
            *flag(app) = false;
        }
    }
    if let Some(a) = app.ui.action
        && layer.id == egui::Id::new(("action", a.node.x, a.node.y))
    {
        app.ui.action = None;
        app.session.selected = None;
    }
    app.ui.kept.retain(|p| layer.id != egui::Id::new(("action", p.x, p.y)));
}

/// Sound, display and play options (in the game and on the title screen).
/// The options' keys: each action with its key; click, then press the new one (a11y gap:
/// every key was fixed).
fn keys_section(app: &mut App, ui: &mut egui::Ui) {
    use crate::keys::{ACTS, Act};
    let note_id = egui::Id::new("keys_note");
    if let Some(a) = app.rebinding {
        ui.label(
            RichText::new(crate::i18n::show(&crate::i18n::msg(
                "Press a key for: {1} (Esc keeps the old one)",
                &[crate::i18n::tr(crate::keys::label(a))],
            )))
            .color(Color32::from_rgb(240, 210, 140)),
        );
        let key = ui.ctx().input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Key { key, pressed: true, .. } => Some(*key),
                _ => None,
            })
        });
        if let Some(k) = key {
            let note = if k == egui::Key::Escape {
                String::new()
            } else {
                match app.keys.set(a, k) {
                    Ok(()) => String::new(),
                    Err(Some(other)) => crate::i18n::show(&crate::i18n::msg(
                        "{1} is the key of: {2}",
                        &[k.name(), crate::i18n::tr(crate::keys::label(other))],
                    )),
                    Err(None) => crate::i18n::show(&crate::i18n::msg(
                        "{1} can't be used: the game needs it (scrolling, cancelling, the map cursor, Tab, the action window)",
                        &[k.name()],
                    )),
                }
            };
            ui.data_mut(|d| d.insert_temp(note_id, note));
            app.rebinding = None;
        }
    }
    if let Some(n) = ui.data(|d| d.get_temp::<String>(note_id)).filter(|n| !n.is_empty()) {
        ui.label(RichText::new(n).small().color(Color32::LIGHT_RED));
    }
    egui::Grid::new("keys").num_columns(2).show(ui, |ui| {
        for (a, _, label, _) in ACTS {
            ui.label(crate::i18n::tr(label));
            let waiting = app.rebinding == Some(a);
            let text = if waiting { "…".to_string() } else { crate::keys::key_name(app.keys.key(a)).to_string() };
            if ui.add(egui::Button::new(RichText::new(text).monospace()).selected(waiting)).clicked() {
                app.rebinding = Some(a);
            }
            ui.end_row();
        }
    });
    if ui.button(crate::i18n::tr("All keys as they were")).clicked() {
        app.keys = crate::keys::Keys::default();
        app.rebinding = None::<Act>;
    }
}

pub fn options_window(app: &mut App, ctx: &egui::Context) {
    let mut open = app.ui.options;
    let main_key = app.keys.key(crate::keys::Act::Main);
    let full_key = app.keys.key(crate::keys::Act::Fullscreen);
    // (it scrolls when the screen is short: at UI scale 2 the keys and languages were cut off;
    // a11y round 3, #1)
    tall(win(crate::i18n::tr("Options"))).open(&mut open).resizable(false).show(ctx, |ui| {
        if let Some(a) = app.audio.as_mut() {
            let pct = |v: f64| format!("{:.0} %", v * 100.0);
            for (v, name) in [
                (&mut a.master, "Volume"),
                (&mut a.music_volume, "Music"),
                (&mut a.sfx_volume, "Sounds"),
                (&mut a.ambience, "Ambience"),
            ] {
                ui.add(egui::Slider::new(v, 0.0..=1.0).custom_formatter(|v, _| pct(v)).text(crate::i18n::tr(name)));
            }
            ui.checkbox(&mut a.muted, crate::keys::keyed(crate::i18n::tr("Mute (Ctrl+M)"), egui::Key::M, main_key));
            ui.checkbox(&mut a.mute_in_background, crate::i18n::tr("Silent while the window is in the background"));
            ui.horizontal(|ui| {
                ui.label(format!("♫ {}", a.track_title()));
                if ui.small_button("⏭").on_hover_text(crate::i18n::tr("Next track")).clicked() {
                    a.next_track();
                }
            });
            ui.checkbox(&mut a.shuffle, crate::i18n::tr("Shuffle"));
        } else {
            ui.label(crate::i18n::tr("No audio device."));
        }
        let mut scale = ctx.zoom_factor();
        let old = scale;
        if ui.add(egui::Slider::new(&mut scale, 1.0..=3.0).step_by(0.25).text(crate::i18n::tr("UI scale"))).changed() {
            // (the interface grows, the map doesn't: a11y round 2, #3)
            ctx.set_zoom_factor(scale);
            app.cam.ui_rescaled(old, scale);
        }
        ui.checkbox(&mut app.auto_road, crate::i18n::tr("Start a road after placing a building"));
        ui.checkbox(&mut app.tips, crate::i18n::tr("Tips for beginners (in the messages)"));
        ui.checkbox(&mut app.pause_on_attack, crate::i18n::tr("Pause when attacked"));
        ui.checkbox(&mut app.calm_map, crate::i18n::tr("Calm map (no swaying, birds or sparkles)"));
        if ui.checkbox(&mut app.fog_of_war, crate::i18n::tr("Fog of war (land out of sight is dimmed)")).changed() {
            app.minimap = None;
        }
        ui.collapsing(crate::i18n::tr("Keys"), |ui| keys_section(app, ui));
        // each language by its own name, so it can be found without reading the current one
        ui.horizontal(|ui| {
            ui.label("🌐");
            for (de, name) in [(false, "English"), (true, "Deutsch")] {
                if ui.selectable_label(crate::i18n::is_german() == de, name).clicked() {
                    crate::i18n::set_german(de);
                }
            }
        });
        if ui.button(crate::keys::keyed(crate::i18n::tr("Toggle fullscreen (F11)"), egui::Key::F11, full_key)).clicked()
        {
            let full = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!full));
        }
    });
    app.ui.options = open;
}

#[allow(clippy::too_many_lines)] // to split (architect round 2, #8: no new long ones)
pub fn windows(app: &mut App, ctx: &egui::Context) {
    right_click_close(app, ctx);
    building_windows(app, ctx);
    let mut open = app.ui.inventory;
    win(crate::i18n::tr("Inventory")).open(&mut open).default_width(360.0).show(ctx, |ui| inventory(app, ui));
    app.ui.inventory = open;
    let mut open = app.ui.distribution;
    win(crate::i18n::tr("Distribution of goods")).open(&mut open).show(ctx, |ui| distribution(app, ui));
    app.ui.distribution = open;
    let mut open = app.ui.transport;
    win(crate::i18n::tr("Transport")).open(&mut open).show(ctx, |ui| transport(app, ui));
    app.ui.transport = open;
    let mut open = app.ui.tools;
    win(crate::i18n::tr("Tools")).open(&mut open).show(ctx, |ui| tools(app, ui));
    app.ui.tools = open;
    let mut open = app.ui.military;
    win(crate::i18n::tr("Military")).open(&mut open).show(ctx, |ui| military(app, ui));
    app.ui.military = open;
    let mut open = app.ui.buildings;
    let mut go = None;
    tall(win(crate::i18n::tr("Buildings"))).open(&mut open).show(ctx, |ui| go = building_overview(app, ui));
    app.ui.buildings = open;
    if let Some(p) = go {
        app.cam.center_on(&app.sim, p, app.screen);
        open_building_at(app, p);
    }
    if app.ui.surrender {
        win(crate::i18n::tr("Give up")).anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO).resizable(false).show(
            ctx,
            |ui| {
                ui.label(crate::i18n::tr("Really give up this game?"));
                ui.horizontal(|ui| {
                    if ui.button(crate::i18n::tr("Yes")).clicked() {
                        app.cmd(Cmd::Surrender);
                        app.ui.surrender = false;
                    }
                    if ui.button(crate::i18n::tr("No")).clicked() {
                        app.ui.surrender = false;
                    }
                });
            },
        );
    }
    let mut open = app.ui.stats;
    // (big enough for the chart and its legend: the markers had no key on screen)
    tall(win(crate::i18n::tr("Statistics")))
        .open(&mut open)
        .default_size([620.0, 470.0])
        .show(ctx, |ui| stats(app, ui));
    app.ui.stats = open;
    let mut open = app.ui.messages;
    tall(win(crate::i18n::tr("Messages"))).open(&mut open).default_width(320.0).show(ctx, |ui| messages(app, ui));
    app.ui.messages = open;
    let mut open = app.ui.map;
    win(crate::i18n::tr("Map")).open(&mut open).resizable(false).show(ctx, |ui| minimap(app, ui));
    app.ui.map = open;
    options_window(app, ctx);
    if app.session.ask_load {
        let (mut yes, mut no) = (false, false);
        win(crate::i18n::tr("Load the quick save?")).show(ctx, |ui| {
            ui.label(crate::i18n::tr("What happened since the last save is lost."));
            ui.horizontal(|ui| {
                yes = ui.button(crate::i18n::tr("Load")).clicked();
                no = ui.button(crate::i18n::tr("Cancel")).clicked();
            });
        });
        if yes {
            app.quick_load();
        } else if no || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            app.session.ask_load = false;
        }
    }
    let mut open = app.ui.help;
    tall(win(crate::i18n::tr("Help"))).open(&mut open).show(ctx, |ui| {
        ui.label(crate::i18n::tr("Left click: select a spot, flag or building. Pick an action in its window."));
        ui.label(crate::i18n::tr(
            "Road: choose 'Build road' at a flag, then click nodes. Click a flag to finish, or 'Flag here'.",
        ));
        // the keys as a table by group, not one sentence of forty (a11y round 2, #7)
        egui::Grid::new("help_keys").num_columns(2).spacing(Vec2::new(16.0, 2.0)).striped(true).show(ui, |ui| {
            for (group, keys) in HELP_KEYS {
                ui.label(RichText::new(crate::i18n::tr(group)).strong());
                ui.end_row();
                for (key, what) in *keys {
                    ui.label(RichText::new(help_key(&app.keys, key, what)).monospace());
                    ui.label(crate::i18n::tr(what));
                    ui.end_row();
                }
            }
        });
        ui.label(crate::i18n::tr(
            "Without a mouse: Enter shows a cursor on the map. Arrows move it, Home / PageUp / End / PageDown go diagonally. Enter clicks there. In the action window 1-9 build, F places a flag, T shows the next tab. Backspace takes back a road step. Ctrl+M: mute.",
        ));
        ui.label(crate::i18n::tr(
            "Goal: defeat all other players by taking their military buildings and castles.",
        ));
        ui.separator();
        ui.label(RichText::new(crate::i18n::tr("How the economy works")).strong());
        for line in [
            "Wood goes to a sawmill and becomes boards; stones from a quarry: every building needs boards and stones.",
            "Food (fish, meat, bread) feeds the mines; coal and iron make tools and weapons.",
            "Beer, a sword and a shield make a soldier; gold coins promote soldiers.",
        ] {
            ui.label(format!("• {}", crate::i18n::tr(line)));
        }
        ui.separator();
        ui.label(RichText::new(crate::i18n::tr("Why is it slow?")).strong());
        for line in [
            "Every road between two flags has one carrier. Long roads with few flags carry little: put a flag every two or three steps.",
            "Wares piling up at a flag mean its roads are busy: build a second road around it.",
            "Far from the castle everything takes long: a storehouse in the middle of your land keeps wares near.",
            "A building that waits for wares says so in its window (click it); O shows how well buildings work.",
        ] {
            ui.label(format!("• {}", crate::i18n::tr(line)));
        }
    });
    app.ui.help = open;
    if let Some(msg) = app.session.game_over.clone() {
        win(crate::i18n::tr("Game over"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.heading(crate::i18n::show(&msg));
                // a won mission: where the story goes from here (story round 1)
                story_text(ui, app.args.mission.as_deref().filter(|_| app.session.epilogue), |t| t.epilogue);
                ui.label(format!("{} {}", crate::i18n::tr("Time played:"), fmt_time(app.sim.gf().0)));
                end_table(app, ui);
                // the game's whole story, not only its last numbers
                if ui.button(crate::i18n::tr("Show statistics")).clicked() {
                    app.ui.stats = true;
                }
                if app.session.net_lost && ui.button(crate::i18n::tr("Continue against the computer")).clicked() {
                    app.continue_alone();
                }
                // a mission's goal reached: play on toward conquering all, or the next mission
                if app.session.goal_reached && ui.button(crate::i18n::tr("Continue: conquer all")).clicked() {
                    app.session.goal_reached = false;
                    app.session.game_over = None;
                }
                next_mission_button(app, ui);
                if ui.button(crate::i18n::tr("Back to the menu")).clicked() {
                    app.in_menu = true;
                }
            });
    }
    if app.ui.briefing
        && let Some(m) = app.args.mission.as_deref().and_then(crate::missions::mission)
    {
        // modal: nothing else to click until the mission begins
        egui::Modal::new(egui::Id::new("briefing")).show(ctx, |ui| {
            ui.set_max_width(480.0);
            // the text scrolls, Begin stays in reach (a11y round 3, #1: at UI scale 2 the title
            // and Begin were off the screen)
            let room = (ctx.screen_rect().height() - 120.0).max(120.0);
            egui::ScrollArea::vertical().max_height(room).show(ui, |ui| {
            ui.heading(crate::i18n::tr(m.title));
            ui.add_space(4.0);
            // the story so far (story round 1)
            story_text(ui, Some(m.key), |t| t.prologue);
            ui.label(crate::i18n::tr(m.briefing));
            ui.add_space(6.0);
            ui.label(RichText::new(crate::i18n::tr(m.goal)).strong());
            // who stands how (game design round 1, #2: not always all five against you)
            for (stance, head) in crate::missions::CAST_HEADS {
                let names: Vec<&str> =
                    m.rivals.iter().zip(m.cast_at(crate::ai_level(&app.args))).filter(|(_, s)| *s == stance).map(|(r, _)| crate::i18n::tr(r.0)).collect();
                if !names.is_empty() {
                    ui.label(format!("{} {}", crate::i18n::tr(head), names.join(", ")));
                }
            }
            // who comes by sea (game design round 1, #7)
            if let Some(p) = m.invader_at(crate::ai_level(&app.args))
                && let Some(r) = m.rivals.get(p as usize - 1)
            {
                ui.label(
                    RichText::new(crate::i18n::show(&crate::i18n::msg(
                        "{1} knows your coast: expect ships within the first hour.",
                        &[crate::i18n::tr(r.0)],
                    )))
                    .color(Color32::from_rgb(240, 190, 120)),
                );
            }
            // (an hour against easy rivals: new-player round 7, #2)
            // (only with rivals that come: the first mission's neighbour keeps the peace for good)
            if let Some(gf) = app.truce_until() {
                ui.label(crate::i18n::show(&crate::i18n::msg(
                    "Your rivals leave you alone for the first {1} minutes (see the clock at the bottom right). Use them to build up and to man your borders.",
                    &[&(gf / crate::app::MINUTE).to_string()],
                )));
            }
            // (both ways: game design round 1, #8)
            ui.label(crate::i18n::tr(truce_both_ways(m.rivals.len())));
            // what normal rivals do (new-player round 9, #3: the same briefing as easy)
            if m.far {
                ui.label(RichText::new(crate::i18n::tr(
                    "Your rivals live far away, across the sea or beyond the desert: the war comes late. Grow toward them, or build a harbor, a shipyard and an expedition to reach them sooner.",
                )).color(Color32::from_rgb(240, 190, 120)));
            } else if crate::ai_level(&app.args) != rr_ai::Level::Easy
                && m.cast_at(crate::ai_level(&app.args)).iter().any(|s| s.hostile())
            {
                // (not where everyone keeps the peace: the first valley said both; new-player
                // round 11, #3)
                ui.label(RichText::new(crate::i18n::tr(
                    "These rivals send every soldier they can spare. After the truce they attack a post every few minutes: keep your border posts full, and have beer, swords and shields ready.",
                )).color(Color32::from_rgb(240, 190, 120)));
            }
            ui.add_space(6.0);
            ui.separator();
            ui.label(RichText::new(crate::i18n::tr("How to start")).strong());
            for line in [
                "Click the ground near your castle and choose a building; a road starts from its flag — click the castle to connect it.",
                "First woodcutters, a forester at the edge of the woods (away from the castle: it plants trees all round it), a quarry and a sawmill; then guardhouses to grow your land.",
                "Space shows where you can build, O how well buildings work. Right-click on a window or the map closes the window.",
            ] {
                ui.label(format!("• {}", crate::i18n::tr(line)));
            }
            });
            ui.add_space(6.0);
            if ui.button(crate::i18n::tr("Begin")).clicked() {
                app.ui.briefing = false;
                app.paused = false;
            }
        });
    } else if app.ui.briefing
        && let Some(m) = app.session.map_mission.clone()
    {
        // a map's own mission, in its author's words
        egui::Modal::new(egui::Id::new("map_briefing")).show(ctx, |ui| {
            ui.set_max_width(480.0);
            if !m.title.is_empty() {
                ui.heading(&m.title);
            }
            if !m.briefing.is_empty() {
                ui.add_space(4.0);
                ui.label(&m.briefing);
            }
            if !m.goal.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new(&m.goal).strong());
            }
            if m.truce_minutes > 0 {
                ui.label(crate::i18n::show(&crate::i18n::msg(
                    "Your rivals leave you alone for the first {1} minutes.",
                    &[&m.truce_minutes.to_string()],
                )));
            }
            ui.add_space(6.0);
            if ui.button(crate::i18n::tr("Begin")).clicked() {
                app.ui.briefing = false;
                app.paused = false;
            }
        });
    }
}

#[allow(clippy::too_many_lines)] // to split (architect round 2, #8: no new long ones)
fn action_window(app: &mut App, ctx: &egui::Context) {
    app.session.work_area = None;
    let Some(action) = app.ui.action else { return };
    let node = action.node;
    let me = app.player;
    let sim = &app.sim;
    let world = sim.world();
    let mut open = true;
    let mut cmds: Vec<Cmd> = Vec::new();
    let mut start_road = None;
    let mut work_area = None;
    let auto_road = app.auto_road;
    // with the keyboard cursor: 1-9 pick a building, F a flag, T the next tab
    let (digit, key_flag, key_tab) = if app.session.cursor.is_some() && !ctx.wants_keyboard_input() {
        ctx.input(|i| {
            let digits = [
                egui::Key::Num1,
                egui::Key::Num2,
                egui::Key::Num3,
                egui::Key::Num4,
                egui::Key::Num5,
                egui::Key::Num6,
                egui::Key::Num7,
                egui::Key::Num8,
                egui::Key::Num9,
            ];
            (digits.iter().position(|k| i.key_pressed(*k)), i.key_pressed(egui::Key::F), i.key_pressed(egui::Key::T))
        })
    } else {
        (None, false, false)
    };
    let keyboard = app.session.cursor.is_some();
    // building pictures for the build tabs (the original shows pictures, cost on hover)
    let nation = sim.players()[me as usize].setup.nation as u8;
    let thumbs: std::collections::HashMap<B, (egui::TextureId, Vec2)> = app
        .tex
        .as_ref()
        .map(|t| {
            t.buildings
                .iter()
                .filter(|((_, n), _)| *n == nation)
                .map(|((b, _), (tex, w, h, _, _))| {
                    let s = 46.0 / h.max(*w * 0.8).max(1.0);
                    (*b, (tex.id(), Vec2::new(w * s, h * s)))
                })
                .collect()
        })
        .unwrap_or_default();
    let mut close = false;
    let mut new_tab = None;
    let mut follow = None;
    let mut attack = app.ui.attack;
    let title: String = match world.obj(node) {
        NodeObj::Building(_) => sim
            .building_at(node)
            .map_or(crate::i18n::tr("Building").to_string(), |b| building_name(sim.building(b).kind).to_string()),
        NodeObj::Flag(_) => crate::i18n::tr("Flag").into(),
        _ => crate::i18n::tr("Action").into(),
    };
    // a window per node, so it opens next to each new click instead of where the last one was
    let wid = egui::Id::new(("action", node.x, node.y));
    let click = anchored(ctx, wid, ctx.input(|i| i.pointer.hover_pos()).unwrap_or_default(), ctx.screen_rect());
    win(title)
        .id(wid)
        .open(&mut open)
        .default_pos(click.0)
        .pivot(click.1)
        // egui sizes a new window as 600 wide at first and keeps that on the screen, which
        // would push windows near the right edge over the click
        .default_width(300.0)
        // never under the bottom bar
        .constrain_to(ctx.screen_rect().with_max_y(ctx.screen_rect().max.y - 48.0))
        .resizable(false)
        .show(ctx, |ui| {
            if let Some(b) = sim.building_at(node) {
                building_info(app, ui, b, &mut cmds, &mut close, &mut follow, &mut attack);
                return;
            }
            if let Some(f) = sim.flag_at(node) {
                let fl = sim.flag(f);
                if fl.player != me {
                    ui.label(crate::i18n::tr("An enemy flag."));
                    return;
                }
                ui.label(format!("{} {}", fl.wares.len(), crate::i18n::tr("wares waiting")));
                ui.horizontal(|ui| {
                    if ui.button(crate::i18n::tr("➡ Build road")).clicked() {
                        start_road = Some(node);
                    }
                    if ui.button(crate::i18n::tr("✖ Remove flag")).clicked() {
                        cmds.push(Cmd::RemoveFlag { at: node });
                        close = true;
                    }
                });
                ui.horizontal(|ui| {
                    if ui
                        .button(crate::i18n::tr("⛏ Geologist"))
                        .on_hover_text(crate::i18n::tr("Search the nearby mountains for resources"))
                        .clicked()
                    {
                        cmds.push(Cmd::SendGeologist { flag: node });
                        close = true;
                    }
                    if ui.button(crate::i18n::tr("👁 Scout")).clicked() {
                        cmds.push(Cmd::SendScout { flag: node });
                        close = true;
                    }
                });
                return;
            }
            // an expedition waiting for a target can land on a free harbor spot
            if world.is_harbor_spot(node) {
                let ready = sim.buildings().iter().find(|(id, b)| {
                    b.player == me && sim.expedition_ready(*id) && sim.harbor_sea(b.pos) == sim.harbor_sea(node)
                });
                if let Some((_, h)) = ready
                    && world.owner(node).is_none()
                    && sim.colony_spot_free(me, node)
                    && ui.button(crate::i18n::tr("⛵ Send the expedition here")).clicked()
                {
                    cmds.push(Cmd::SendExpedition { harbor: h.pos, to: node });
                    close = true;
                }
            }
            if world.owner(node) != Some(me) {
                ui.label(crate::i18n::tr("You can only build on your own land."));
                return;
            }
            let bq = world.bq_for(node, me);
            if bq == Bq::Nothing {
                ui.label(crate::i18n::tr("Nothing can be built here."));
                if let Some(r) = sim.road_at(node)
                    && sim.road(r).player == me
                {
                    ui.label(crate::i18n::tr("(on a road — too close to a flag for another one)"));
                    let road = sim.road(r);
                    if ui.button(crate::i18n::tr("✖ Remove road")).clicked() {
                        cmds.push(Cmd::RemoveRoad { flag: sim.flag(road.from).pos, dir: road.dirs[0] });
                        close = true;
                    }
                }
                return;
            }
            let flag_label = if keyboard {
                format!("{} (F)", crate::i18n::tr("🚩 Place flag"))
            } else {
                crate::i18n::tr("🚩 Place flag").to_string()
            };
            if ui.button(flag_label).clicked() || key_flag {
                cmds.push(Cmd::PlaceFlag { at: node });
                close = true;
            }
            if let Some(r) = sim.road_at(node)
                && sim.road(r).player == me
                && ui.button(crate::i18n::tr("✖ Remove road")).clicked()
            {
                let road = sim.road(r);
                cmds.push(Cmd::RemoveRoad { flag: sim.flag(road.from).pos, dir: road.dirs[0] });
                close = true;
            }
            let tabs: Vec<(&str, Vec<B>)> = buildable_lists(bq);
            // no military building within 4 nodes of another one (as in the original): shown, but
            // disabled with the reason
            let military_near = sim.military_building_near(node, me);
            if tabs.is_empty() {
                if bq == Bq::Flag {
                    ui.label(crate::i18n::tr(
                        "Only a flag fits here: a building needs more room (too close to another building, a tree, rocks or a slope).",
                    ));
                }
                return;
            }
            let mut tab = action.tab.min(tabs.len() - 1);
            if key_tab {
                tab = (tab + 1) % tabs.len();
            }
            ui.horizontal(|ui| {
                for (i, (name, _)) in tabs.iter().enumerate() {
                    // the same marker as on the map
                    bq_mark(ui, tab_bq(name));
                    if ui.selectable_label(tab == i, crate::i18n::tr(name)).clicked() {
                        tab = i;
                    }
                }
            });
            new_tab = Some(tab);
            // on a mountain: what the geologists found here, or how to find out
            if tabs[tab].0 == "Mines" {
                let found = found_near(sim, node);
                if found.is_empty() {
                    ui.label(
                        RichText::new(crate::i18n::tr(
                            "Not sure what's inside? Place a flag on the mountain and send a geologist from it.",
                        ))
                        .small()
                        .color(Color32::LIGHT_YELLOW),
                    );
                } else {
                    let list: Vec<String> =
                        found.iter().map(|(m, a)| format!("{} ({a})", crate::i18n::tr(building_key(*m)))).collect();
                    ui.label(
                        RichText::new(format!("{} {}", crate::i18n::tr("Geologists found here:"), list.join(", ")))
                            .small()
                            .color(Color32::LIGHT_YELLOW),
                    );
                }
            }
            // a greyed-out military building says why, without hovering
            if military_near && tabs[tab].1.iter().any(|b| rr_sim::is_frontier_military(*b)) {
                ui.label(
                    RichText::new(crate::i18n::tr(
                        "Military buildings are greyed out: they keep 4 nodes from the castle and from each other. Try further out.",
                    ))
                    .color(Color32::LIGHT_YELLOW),
                );
                // (the lookout tower stays lit: it only watches; new-player round 11, #8)
                if tabs[tab].1.contains(&B::LookoutTower) {
                    ui.label(
                        RichText::new(crate::i18n::tr(
                            "The lookout tower isn't one: it holds no soldiers and claims no land, it only watches far.",
                        ))
                        .color(Color32::LIGHT_YELLOW),
                    );
                }
            }
            egui::Grid::new("blds").num_columns(3).spacing([6.0, 6.0]).show(ui, |ui| {
                for (i, b) in tabs[tab].1.iter().enumerate() {
                    let def = sim.rules().building(*b);
                    let cost = if def.stones > 0 {
                        format!(
                            "{} {}, {} {}",
                            def.boards,
                            crate::i18n::tr("boards"),
                            def.stones,
                            crate::i18n::tr("stones")
                        )
                    } else {
                        format!("{} {}", def.boards, crate::i18n::tr("boards"))
                    };
                    let label = if keyboard && i < 9 {
                        format!("{} {}", i + 1, building_name(*b))
                    } else {
                        building_name(*b).to_string()
                    };
                    let tip = {
                        let ins: Vec<String> = def.inputs.iter().map(|w| ware_name(*w)).collect();
                        let out = def.output.map(ware_name).unwrap_or_else(|| "—".into());
                        let worker = def.worker.map(job_name).unwrap_or_else(|| "—".into());
                        format!(
                            "{}\n{cost}\n{} {worker}\n{} {}\n{} {out}",
                            building_name(*b),
                            crate::i18n::tr("Worker:"),
                            crate::i18n::tr(if def.is_military() { "Promotes soldiers with:" } else { "Needs:" }),
                            if ins.is_empty() { "—".into() } else { ins.join(", ") },
                            crate::i18n::tr("Makes:")
                        )
                    };
                    let blocked = military_near && rr_sim::is_frontier_military(*b);
                    let button = ui
                        .add_enabled_ui(!blocked, |ui| {
                            let r = picture_button(ui, thumbs.get(b).copied(), &label);
                            // workshops look alike: their product in the corner, as on the map
                            if let Some(w) = crate::render::sign_ware(*b) {
                                let c = r.rect.right_top() + Vec2::new(-13.0, 13.0);
                                parchment_disc(ui, c, 10.0);
                                crate::sprites::ware_icon(ui.painter(), c, 15.0, w);
                            }
                            r
                        })
                        .inner
                        .on_hover_text(tip)
                        .on_disabled_hover_text(crate::i18n::tr(
                            "Too close to the castle or another military building: military buildings keep 4 nodes apart.",
                        ));
                    if button.hovered() {
                        work_area = sim.work_radius(*b).map(|r| (node, r));
                    }
                    if button.clicked() || digit == Some(i) && !blocked {
                        cmds.push(Cmd::PlaceBuilding { at: node, kind: *b });
                        if auto_road {
                            start_road = Some(sim.world().grid().neighbor(node, rr_core::Direction::SE));
                        }
                        close = true;
                    }
                    if i % 3 == 2 {
                        ui.end_row();
                    }
                }
            });
        });
    app.session.work_area = work_area;
    app.ui.attack = attack;
    if follow.is_some() {
        app.session.follow = follow;
    }
    if let Some(tab) = new_tab {
        app.ui.action = Some(Action { node, tab });
    }
    for c in cmds {
        app.cmd(c);
    }
    if let Some(n) = start_road {
        app.session.road = Some((n, Vec::new()));
        close = true;
    }
    if !open || close {
        app.ui.action = None;
        app.session.selected = None;
    }
}

/// How to do a First steps line (shown when hovering it).
fn first_step_hint(what: &str) -> Option<&'static str> {
    Some(match what {
        "Two woodcutters" => "Near trees: Space shows where buildings fit.",
        "A forester" => {
            "Next to the woodcutters, away from the land you still want to build on: without one the woods run out."
        }
        "A quarry" => "Next to grey rocks.",
        "A sawmill" => "A medium spot (the bigger house marker): it turns wood into boards.",
        "Three military buildings" => "At the edge of your land: each one makes it bigger and guards it.",
        "A fishery or a hunter" => "A fishery by the water, a hunter near forests with game.",
        "A mine" => {
            "On a mountain inside your land (the dark triangles); send a geologist first to find out what is inside."
        }
        // (and the next steps: new-player round 10, #7)
        "A farm and a well" => {
            "A farm needs open land around it for its fields; grain and water feed the mill, the brewery and the pigs."
        }
        "A mill and a bakery" => "Grain makes flour, flour and water make bread: bread feeds the miners.",
        "A coal mine and an iron mine" => {
            "On mountains where a geologist found coal and iron; miners need food (fish, meat or bread)."
        }
        "A brewery" => "Grain and water make beer. Beer, a sword and a shield make a soldier.",
        "An iron smelter and an armory" => {
            "Iron ore and coal make iron; iron and coal make swords and shields in the armory."
        }
        "Take a rival post: click it, then ⚔ Attack" => {
            "Once you have soldiers to spare: a rival post near your border, weaker than what you can send."
        }
        _ => return None,
    })
}

/// What geologists' signs within 2 of `at` show: the mine for each find and the most it says,
/// richest first (new-player round 7, #5).
pub fn found_near(sim: &rr_sim::Sim, at: rr_core::MapCoord) -> Vec<(B, u8)> {
    let g = sim.world().grid();
    let mut found: Vec<(B, u8)> = Vec::new();
    for q in g.within(at, 2) {
        if let rr_world::NodeObj::Sign(code) = sim.world().obj(q) {
            let mine = match code >> 4 {
                1 => B::CoalMine,
                2 => B::IronMine,
                3 => B::GoldMine,
                4 => B::GraniteMine,
                _ => continue,
            };
            let amount = code & 0x0f;
            match found.iter_mut().find(|(m, _)| *m == mine) {
                Some(e) => e.1 = e.1.max(amount),
                None => found.push((mine, amount)),
            }
        }
    }
    found.sort_by_key(|(m, a)| (std::cmp::Reverse(*a), m.name()));
    found
}

/// The tab the build window opens on: the largest the spot allows, the one its marker shows
/// (new-player round 6, #7).
pub fn first_tab(bq: Bq) -> usize {
    buildable_lists(bq).len().saturating_sub(1)
}

/// The tab for `wanted` (the First steps' next building) if the spot allows it, else `first_tab`.
pub fn tab_for(bq: Bq, wanted: Option<B>) -> usize {
    let lists = buildable_lists(bq);
    wanted.and_then(|w| lists.iter().position(|(_, l)| l.contains(&w))).unwrap_or_else(|| first_tab(bq))
}

fn buildable_lists(bq: Bq) -> Vec<(&'static str, Vec<B>)> {
    use B::*;
    let small = vec![Woodcutter, Forester, Quarry, Fishery, Hunter, Well, Barracks, Guardhouse, LookoutTower];
    let medium = vec![
        Sawmill,
        Slaughterhouse,
        Mill,
        Bakery,
        Brewery,
        IronSmelter,
        Armory,
        Metalworks,
        Mint,
        Storehouse,
        Watchtower,
        Shipyard,
        Catapult,
    ];
    let large = vec![Farm, PigFarm, DonkeyBreeder, Fortress];
    let mines = vec![CoalMine, IronMine, GoldMine, GraniteMine];
    let mut v = Vec::new();
    match bq {
        Bq::Mine => v.push(("Mines", mines)),
        Bq::Hut => v.push(("Small", small)),
        Bq::House => {
            v.push(("Small", small));
            v.push(("Medium", medium));
        }
        Bq::Castle => {
            v.push(("Small", small));
            v.push(("Medium", medium));
            v.push(("Large", large));
        }
        Bq::Harbor => {
            let mut large = large;
            large.insert(0, Harbor);
            v.push(("Small", small));
            v.push(("Medium", medium));
            v.push(("Large", large));
        }
        _ => {}
    }
    v
}

fn tab_bq(name: &str) -> Bq {
    match name {
        "Small" => Bq::Hut,
        "Medium" => Bq::House,
        "Mines" => Bq::Mine,
        _ => Bq::Castle,
    }
}

/// A building-help marker as drawn on the map, as a small picture in the interface.
fn bq_mark(ui: &mut egui::Ui, bq: Bq) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(16.0, 18.0), egui::Sense::hover());
    crate::sprites::bq_icon(ui.painter(), r.center_bottom() - Vec2::new(0.0, 2.0), 1.1, bq);
}

/// A short note at the top of the screen for three seconds.
/// The first game's goals in a corner, ticked off as they are reached; gone once all are done
/// or a game was won (new-player round 6, #5).
/// The help's keys, by group: (key, what it does).
/// A help row's keys as the player has set them (the table holds the defaults).
/// (Key names and the default column go through `tr`: German round 1, #6.)
fn help_key(keys: &crate::keys::Keys, default: &str, what: &str) -> String {
    use crate::keys::{ACTS, Act};
    let k = |a: Act| crate::keys::key_name(keys.key(a));
    match what {
        "Zoom" => crate::i18n::fill("Wheel, {1} / shift-{1}", &[k(Act::Zoom)]),
        "Speed" => format!("{} / {}", k(Act::Faster), k(Act::Slower)),
        "Quick save / load" => format!("{} / {}", k(Act::QuickSave), k(Act::QuickLoad)),
        "Mute" => crate::i18n::fill("Ctrl+{1}", &[k(Act::Main)]),
        _ => ACTS.iter().find(|a| a.2 == what).map_or(crate::i18n::tr(default).to_string(), |a| k(a.0).to_string()),
    }
}

/// How close the enemy is, in plain words, for the garrison line (the border of the land isn't
/// the same as a border with an enemy).
const GARRISON_AREAS: [&str; 4] = ["no enemy near", "an enemy within reach", "a harbor to guard", "an enemy close by"];

/// The garrison line: how many soldiers the military settings send, one sentence per count
/// (German round 1, #3: it read "Die Militäreinstellungen schicken 2: …").
fn garrison_cap_line(cap: usize, area: &str) -> String {
    if cap == 1 {
        crate::i18n::fill(
            "The military settings send 1 soldier here ({1}). Change it in Main selection, Military.",
            &[area],
        )
    } else {
        crate::i18n::fill(
            "The military settings send {1} soldiers here ({2}). Change it in Main selection, Military.",
            &[&cap.to_string(), area],
        )
    }
}

const HELP_KEYS: &[(&str, &[(&str, &str)])] = &[
    (
        "Camera",
        &[
            ("Right drag, arrows, WASD", "Scroll"),
            ("Wheel, Z / shift-Z", "Zoom"),
            ("H", "To the castle"),
            ("G", "To the latest alert"),
        ],
    ),
    ("Windows", &[("I", "Inventory"), ("L", "Map"), ("M", "Main selection"), ("N", "Messages")]),
    ("Showing", &[("Space", "Building help"), ("O", "Productivity and crowded flags"), ("C", "Building names")]),
    (
        "Game",
        &[
            ("P", "Pause"),
            ("+ / -", "Speed"),
            ("Esc", "Cancel"),
            ("F5 / F9", "Quick save / load"),
            ("F11", "Fullscreen"),
            ("Ctrl+M", "Mute"),
            ("F1", "Help"),
        ],
    ),
];

/// Where a game's earned ticks are kept: one game's, not the next one's too (QA round 7, #3: a
/// new game opened with six of seven steps ticked).
fn steps_key(title: &str, game: u32) -> egui::Id {
    egui::Id::new(("steps_done", title, game))
}

/// Steps with every tick earned before kept (`mask`: one bit per step), and the new mask.
fn sticky_steps(steps: Vec<(&'static str, bool)>, mask: u64) -> (Vec<(&'static str, bool)>, u64) {
    let mut m = mask;
    let out = steps
        .into_iter()
        .enumerate()
        .map(|(i, (w, done))| {
            let done = done || m & (1 << i) != 0;
            if done {
                m |= 1 << i;
            }
            (w, done)
        })
        .collect();
    (out, m)
}

/// The step whose hint shows under it without hovering: the first one not done (a11y round 2,
/// #7: hints on hover only were never seen by keyboard and touch players).
fn step_with_hint(steps: &[(&str, bool)]) -> Option<usize> {
    steps.iter().position(|(_, done)| !*done)
}

fn first_steps_panel(app: &App, ctx: &egui::Context) {
    // (after a first win still in a mission, Next steps only: new-player round 9, #3)
    let mission = app.args.mission.is_some();
    if !app.tips || app.won_once && !mission || app.in_menu || app.playback.is_some() || app.editor.is_some() {
        return;
    }
    // (RR_NO_STEPS: clean pictures for the README)
    if std::env::var_os("RR_NO_STEPS").is_some() {
        return;
    }
    // First steps, then Next steps (new-player round 7, #7)
    let mut steps = crate::app::first_steps(&app.sim, app.player);
    let mut title = "First steps";
    if app.won_once || steps.iter().all(|(_, done)| *done) {
        // (worked out every 100 game frames: with nobody in reach it compares every post of
        // ours with every rival's, each drawn frame; perf round 3, #10)
        // (and at once when a building is placed or lost: a tick 5 s late reads as not counted)
        let key = egui::Id::new(("next_steps", app.game_no));
        let at = (app.sim.gf().0 / 100, app.sim.buildings().len());
        steps = match ctx.data(|d| d.get_temp::<((u32, usize), Vec<(&'static str, bool)>)>(key)) {
            Some((then, steps)) if then == at => steps,
            _ => {
                let steps = crate::app::next_steps(&app.sim, app.player);
                ctx.data_mut(|d| d.insert_temp(key, (at, steps.clone())));
                steps
            }
        };
        title = "Next steps";
    }
    // (a tick once earned stays: kept per title for this session)
    let key = steps_key(title, app.game_no);
    let mask = ctx.data(|d| d.get_temp::<u64>(key)).unwrap_or(0);
    let (steps, mask) = sticky_steps(steps, mask);
    ctx.data_mut(|d| d.insert_temp(key, mask));
    if steps.iter().all(|(_, done)| *done) {
        return;
    }
    egui::Area::new(egui::Id::new("first_steps")).anchor(egui::Align2::RIGHT_TOP, Vec2::new(-8.0, 40.0)).show(
        ctx,
        |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_max_width(250.0);
                egui::CollapsingHeader::new(crate::i18n::tr(title)).default_open(true).show(ui, |ui| {
                    let now = step_with_hint(&steps);
                    for (k, (what, done)) in steps.into_iter().enumerate() {
                        let mark = if done { "✔" } else { "·" };
                        let text = egui::RichText::new(format!("{mark} {}", crate::i18n::tr(what)));
                        let r = ui.label(if done { text.weak() } else { text });
                        if let Some(hint) = first_step_hint(what) {
                            if now == Some(k) {
                                // (wrapped to the panel: a long hint ran past its edge)
                                ui.add(
                                    // (a no-break space: italics lean past the measured width)
                                    egui::Label::new(
                                        RichText::new(format!("{}\u{a0}", crate::i18n::tr(hint))).italics().size(12.0),
                                    )
                                    .wrap(),
                                );
                            } else {
                                r.on_hover_text(crate::i18n::tr(hint));
                            }
                        }
                    }
                });
            });
        },
    );
}

/// An alert on screen: it stays 12 s of running game (a pause stops its clock), and a note
/// never replaces it (a11y round 2, #5: 6 s of wall time, gone at the next "Saved").
pub struct Alert {
    pub text: String,
    pub at: MapCoord,
    shown: f32,
}

impl Alert {
    pub const SECS: f32 = 12.0;

    pub fn new(text: String, at: MapCoord) -> Self {
        Alert { text, at, shown: 0.0 }
    }

    /// `dt` seconds more on screen; false once it has been shown long enough.
    pub fn tick(&mut self, dt: f32, paused: bool) -> bool {
        if !paused {
            self.shown += dt;
        }
        self.shown < Self::SECS
    }
}

/// The end screen's way on to the campaign's next mission, once this one is won.
fn next_mission_button(app: &mut App, ui: &mut egui::Ui) {
    let won = app.args.mission.as_ref().is_some_and(|k| app.won_missions.contains(k));
    if let Some(next) = app.args.mission.as_deref().and_then(crate::missions::next_mission).filter(|_| won)
        && ui.button(format!("{} {}", crate::i18n::tr("Next mission:"), crate::i18n::tr(next.title))).clicked()
    {
        app.start_game(crate::Args {
            mission: Some(next.key.into()),
            seed: app.menu.seed,
            ai_level: app.menu.ai_level,
            goods: app.menu.goods,
            ..Default::default()
        });
    }
}

/// A part of a mission's story (`rr_game::story`), in italics: `part` of the tale of `mission`.
fn story_text(ui: &mut egui::Ui, mission: Option<&str>, part: fn(&rr_game::story::Tale) -> &'static str) {
    if let Some(t) = mission.and_then(rr_game::story::tale) {
        ui.set_max_width(460.0);
        ui.label(RichText::new(crate::i18n::tr(part(t))).italics());
        ui.add_space(4.0);
    }
}

/// Whether the "paused" banner shows: in a game, not over the menu or the briefing.
fn pause_banner(paused: bool, in_menu: bool, briefing: bool) -> bool {
    paused && !in_menu && !briefing
}

/// How strong the ring on a building that Go there went to is, `t` seconds on (3 s).
pub fn mark_alpha(t: f32) -> f32 {
    (1.0 - t / 3.0).clamp(0.0, 1.0)
}

/// The kinds of floating notice: one frame (the windows' wood and gold edge), a stripe each
/// (art director round 4, #7: five notices in five styles).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Notice {
    Military,
    Note,
    Pause,
}

impl Notice {
    fn stripe(self) -> Color32 {
        match self {
            Notice::Military => Color32::from_rgb(200, 60, 40),
            Notice::Note => Color32::from_rgb(230, 190, 90),
            Notice::Pause => Color32::from_rgb(150, 140, 120),
        }
    }
}

/// A notice's frame with its stripe down the left side.
fn notice(ui: &mut egui::Ui, kind: Notice, body: impl FnOnce(&mut egui::Ui)) {
    let frame = egui::Frame::window(ui.style()).inner_margin(egui::Margin { left: 14, right: 10, top: 6, bottom: 6 });
    let r = frame.show(ui, body).response.rect;
    let stripe =
        egui::Rect::from_min_max(r.left_top() + Vec2::new(3.0, 4.0), egui::pos2(r.left() + 7.0, r.bottom() - 4.0));
    ui.painter().rect_filled(stripe, 2.0, kind.stripe());
}

/// How dark a pause makes the map (alpha of black over it).
pub const PAUSE_DIM: u8 = 70;

/// A pause you can see (new-player round 10, #9: only the corner's ▶ changed): the map dims
/// (drawn with the map) and a banner at the top, over the windows.
fn paused_banner(app: &App, ctx: &egui::Context) {
    if !pause_banner(app.paused, app.in_menu, app.ui.briefing) || app.playback.is_some() {
        return;
    }
    // (below an alert, never over its Go there: a11y round 3, #10)
    let y = if app.session.alert.is_some() { 150.0 } else { 100.0 };
    egui::Area::new(egui::Id::new("paused"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_TOP, Vec2::new(0.0, y))
        .show(ctx, |ui| {
            notice(ui, Notice::Pause, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(crate::i18n::tr("Paused")).size(22.0).strong());
                    if app.session.paused_by_attack {
                        ui.label(crate::i18n::tr("You are under attack (\"Pause when attacked\" in the Options)."));
                    }
                    let k = crate::keys::key_name(app.keys.key(crate::keys::Act::Pause));
                    ui.label(RichText::new(crate::i18n::show(&crate::i18n::msg("{1} continues", &[k]))).small());
                });
            });
        });
}

fn alert_toast(app: &mut App, ctx: &egui::Context) {
    let dt = ctx.input(|i| i.stable_dt).min(0.5);
    let paused = app.paused;
    let Some(alert) = app.session.alert.as_mut() else { return };
    if !alert.tick(dt, paused) {
        app.session.alert = None;
        return;
    }
    let (text, at) = (alert.text.clone(), alert.at);
    let go_label =
        crate::keys::keyed(crate::i18n::tr("Go there (G)"), egui::Key::G, app.keys.key(crate::keys::Act::ToAlert));
    let mut go = false;
    let mut close = false;
    egui::Area::new(egui::Id::new("alert")).anchor(egui::Align2::CENTER_TOP, Vec2::new(0.0, 56.0)).show(ctx, |ui| {
        notice(ui, Notice::Military, |ui| {
            // (wrapped narrow: at a big UI scale it lay over the steps panel)
            ui.set_max_width(460.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(text).strong());
                go = ui.button(&go_label).clicked();
                close = ui.button("✖").clicked();
            });
        });
    });
    if go {
        app.cam.center_on(&app.sim, at, app.screen);
        app.mark = Some((at, ctx.input(|i| i.time)));
        open_building_at(app, at);
    }
    if go || close {
        app.session.alert = None;
    }
    ctx.request_repaint();
}

/// How long a note stays, in seconds: 3 (6 with a place to go to), and long enough to read at
/// about 200 words a minute (a11y round 3, #4: the 31-word truce advice had 3 s).
fn toast_secs(text: &str, place: bool) -> f64 {
    let base: f64 = if place { 6.0 } else { 3.0 };
    let words = text.split_whitespace().count() as f64;
    base.max(1.5 + 0.3 * words)
}

fn toast(app: &mut App, ctx: &egui::Context) {
    // (a faded mark is dropped here: the map view borrows the app while it draws)
    if app.mark.is_some_and(|(_, since)| mark_alpha((ctx.input(|i| i.time) - since) as f32) <= 0.0) {
        app.mark = None;
    }
    paused_banner(app, ctx);
    alert_toast(app, ctx);
    let Some(text) = app.session.toast.clone() else { return };
    let now = ctx.input(|i| i.time);
    let mut since = *app.session.toast_since.get_or_insert(now);
    // a pause stops its clock (a11y round 3, #4: notes ran out while the game stood)
    if app.paused {
        since += ctx.input(|i| i.stable_dt).min(0.5) as f64;
        app.session.toast_since = Some(since);
    }
    // news with a place stays longer and offers to go there; long notes longer still
    let at = app.session.toast_at;
    if now - since > toast_secs(&text, at.is_some()) {
        app.session.toast = None;
        app.session.toast_since = None;
        app.session.toast_at = None;
        return;
    }
    let mut go = false;
    egui::Area::new(egui::Id::new("toast")).anchor(egui::Align2::CENTER_TOP, Vec2::new(0.0, 12.0)).show(ctx, |ui| {
        notice(ui, Notice::Note, |ui| {
            ui.horizontal(|ui| {
                ui.label(text);
                if at.is_some() && ui.button(crate::i18n::tr("Go there")).clicked() {
                    go = true;
                }
            });
        });
    });
    if go && let Some(p) = at {
        app.cam.center_on(&app.sim, p, app.screen);
        open_building_at(app, p);
        app.session.toast = None;
        app.session.toast_at = None;
    }
    ctx.request_repaint();
}

/// What the building-help markers mean (shown while they are).
fn bq_legend(app: &App, ctx: &egui::Context) {
    if !(app.show_bq || app.session.road.is_some()) || app.in_menu {
        return;
    }
    egui::Area::new(egui::Id::new("bq_legend")).anchor(egui::Align2::LEFT_BOTTOM, Vec2::new(8.0, -48.0)).show(
        ctx,
        |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                for (bq, what) in [
                    (Bq::Flag, "Flag"),
                    (Bq::Hut, "Small building"),
                    (Bq::House, "Medium building"),
                    (Bq::Castle, "Large building"),
                    (Bq::Mine, "Mine"),
                ] {
                    ui.horizontal(|ui| {
                        bq_mark(ui, bq);
                        ui.label(RichText::new(crate::i18n::tr(what)).small());
                    });
                }
            });
        },
    );
}

/// Why nobody can attack: no military building in reach (the headquarters only defends), or
/// the ones in reach keep their last soldier (new-player round 6, #8).
fn no_attack_reason(posts_in_reach: usize) -> &'static str {
    if posts_in_reach == 0 {
        "Attacks start from your military buildings nearby (barracks, guardhouse, watchtower, fortress); the castle's soldiers only defend. Build one close to it."
    } else {
        "Your military buildings near it keep their last soldier at home. Send them more soldiers, or raise the attack setting (Military)."
    }
}

/// Whether a value worked out at game frame `gf_then`, `now - then` seconds ago, still
/// serves: at the same game frame, or within a fifth of a second (at high speed every drawn
/// frame is a new game frame).
fn still_fresh(then: Option<(u32, f64)>, gf: u32, now: f64) -> bool {
    then.is_some_and(|(at, t)| at == gf || now - t < 0.2)
}

/// How many soldiers the attack window offers: the count chosen last, else all that may go,
/// within what may go now.
fn attack_choice(chosen: Option<u8>, avail: usize) -> u8 {
    let most = avail.clamp(1, 255) as u8;
    chosen.unwrap_or(most).clamp(1, most)
}

/// Rough odds of an attack from the hit points on both sides (a guide, not a forecast: fights
/// go one by one, and chance decides each).
fn odds(ours: u32, theirs: u32) -> &'static str {
    if ours * 10 >= theirs * 13 {
        "Rough odds: in your favour."
    } else if ours * 13 > theirs * 10 {
        "Rough odds: even."
    } else {
        "Rough odds: against you."
    }
}

/// Soldiers' ranks as counts, the highest first: "2× General, 1× Private".
fn ranks_line(ranks: &[u8]) -> String {
    (0..5u8)
        .rev()
        .filter_map(|r| {
            let n = ranks.iter().filter(|x| **x == r).count();
            (n > 0).then(|| format!("{n}× {}", job_name(rr_rules::Job::SOLDIERS[r as usize])))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The attack's odds under its slider: who would go, who holds the building, and roughly how
/// it may end (genre lessons, PLAN §27, #2: S2's attacks were "send all and hope").
fn attack_odds(ui: &mut egui::Ui, sim: &rr_sim::Sim, bd: &rr_sim::Building, party: &[u8]) {
    let defenders: Vec<u8> = match &bd.inventory {
        Some(inv) => (0..5u8)
            .flat_map(|r| std::iter::repeat_n(r, inv.job(rr_rules::Job::SOLDIERS[r as usize]) as usize))
            .collect(),
        None => bd.soldiers.iter().filter_map(|s| sim.figure(*s).job.soldier_rank()).collect(),
    };
    let hp = |r: &[u8]| r.iter().map(|x| sim.rules().military.hp[*x as usize] as u32).sum::<u32>();
    // (body size: the most important lines of the window were its smallest; a11y round 3, #6)
    ui.label(RichText::new(format!("{} {}", crate::i18n::tr("You send:"), ranks_line(party))));
    let held = if defenders.is_empty() { crate::i18n::tr("nobody").to_string() } else { ranks_line(&defenders) };
    ui.label(RichText::new(format!("{} {held}", crate::i18n::tr("They hold:"))));
    ui.label(RichText::new(crate::i18n::tr(odds(hp(party), hp(&defenders)))).color(Color32::from_rgb(240, 210, 140)));
}

/// The attack part of a rival's post or warehouse: how many soldiers, strong or weak ones, the
/// truce question, the odds (architect round 3, #10: out of `building_info`).
fn attack_panel(
    app: &App,
    ui: &mut egui::Ui,
    b: rr_sim::BuildingId,
    cmds: &mut Vec<Cmd>,
    close: &mut bool,
    attack: &mut AttackPanel,
) {
    let sim = &app.sim;
    let bd = sim.building(b);
    let me = app.player;
    // one search of the posts in reach, again when the game moved on, at most five times a
    // second (perf-2 #2: every drawn frame took 24-32 ms; perf-3 #5: three searches a game
    // frame, a new one each drawn frame above normal speed)
    let key = egui::Id::new(("attack_plan", b));
    let now = ui.input(|i| i.time);
    let stored = ui.data(|d| d.get_temp::<(u32, f64, rr_sim::AttackPlan)>(key));
    let plan = match stored {
        Some((gf, at, plan)) if still_fresh(Some((gf, at)), sim.gf().0, now) => plan,
        _ => {
            let plan = sim.attack_plan(me, bd.pos);
            ui.data_mut(|d| d.insert_temp(key, (sim.gf().0, now, plan.clone())));
            plan
        }
    };
    // (and the posts in reach when nobody can go: QA round 6, #4)
    let (avail, posts) = (plan.available, plan.posts);
    if avail == 0 {
        ui.label(crate::i18n::tr(no_attack_reason(posts)));
        return;
    }
    if plan.by_sea {
        ui.label(crate::i18n::tr("⛵ By sea: soldiers from the posts by your harbor sail over and land nearby."));
    }
    // the count chosen last time, kept between attacks (new-player round 11, #1: it was
    // read from a field nobody wrote, so every attack sent one soldier)
    let mut n = attack_choice(attack.count, avail);
    ui.horizontal(|ui| {
        ui.radio_value(&mut attack.weak, false, crate::i18n::tr("Strong soldiers"));
        ui.radio_value(&mut attack.weak, true, crate::i18n::tr("Weak soldiers"));
    });
    let app_strong = !attack.weak;
    // during a truce the attack ends it: asked first (game design round 1, #8)
    ui.horizontal(|ui| {
        if ui
            .add(
                egui::Slider::new(&mut n, 1..=avail.min(255) as u8)
                    .text(format!("{} {avail}", crate::i18n::tr("soldiers, available:"))),
            )
            .changed()
        {
            attack.count = Some(n);
        }
        match app.truce_with(bd.player) {
            Some(_) => {
                if ui
                    .button(crate::i18n::tr("⚔ Attack! (ends the truce)"))
                    .on_hover_text(crate::i18n::tr("Your rivals keep the truce as long as you do"))
                    .clicked()
                {
                    attack.asking = Some(b);
                }
            }
            None => {
                if ui.button(crate::i18n::tr("⚔ Attack!")).clicked() {
                    cmds.push(Cmd::Attack { target: bd.pos, soldiers: n, strong: app_strong });
                    *close = true;
                }
            }
        }
    });
    if let Some(truce) = app.truce_with(bd.player)
        && attack.asking == Some(b)
    {
        // (a peaceful rival's truce lasts for good: no countdown to show)
        let text = match truce {
            crate::app::Truce::ForGood => {
                crate::i18n::tr("This rival keeps the peace as long as you do. Attacking ends it, with its allies too.")
                    .to_string()
            }
            crate::app::Truce::Until(until) => crate::i18n::show(&crate::i18n::msg(
                "The truce runs {1} more. Attacking ends it, with this rival and its allies.",
                &[&fmt_time(until.saturating_sub(sim.gf().0))],
            )),
        };
        ui.label(RichText::new(text).color(Color32::from_rgb(240, 190, 120)));
        ui.horizontal(|ui| {
            if ui.button(crate::i18n::tr("End the truce and attack")).clicked() {
                cmds.push(Cmd::Attack { target: bd.pos, soldiers: n, strong: app_strong });
                attack.asking = None;
                *close = true;
            }
            if ui.button(crate::i18n::tr("Keep the truce")).clicked() {
                attack.asking = None;
            }
        });
    }
    attack_odds(ui, sim, bd, plan.party(n, app_strong));
}

/// A harbor's expedition (what it has gathered of what it takes along) and ships, and the free
/// harbor spots across its sea that we have explored, each a click away on the map (owner,
/// 2026-10-27: "make harbors meaningful": nothing showed where an expedition could go).
fn harbor_panel(app: &App, ui: &mut egui::Ui, b: rr_sim::BuildingId, cmds: &mut Vec<Cmd>) {
    let (sim, me) = (&app.sim, app.player);
    let bd = sim.building(b);
    let load = sim.expedition_load();
    match &bd.expedition {
        None => {
            if ui
                .button(crate::i18n::tr("⛵ Prepare an expedition"))
                .on_hover_text(crate::i18n::tr(
                    "A ship takes a builder, the harbor's materials and a starter kit to a free harbor spot on another coast: a new colony.",
                ))
                .clicked()
            {
                cmds.push(Cmd::StartExpedition { harbor: bd.pos });
            }
        }
        Some(e) if e.ship.is_some() && sim.expedition_ready(b) => {
            ui.label(
                RichText::new(crate::i18n::tr("Expedition ready: click a free harbor spot on another coast."))
                    .color(Color32::LIGHT_GREEN),
            );
            // or into the unknown: the nearest free spot on this sea, known or not (as the
            // computer players sail; on an archipelago nothing is known at first)
            let g = sim.world().grid();
            let nearest = sim
                .harbor_sea(bd.pos)
                .map(|sea| sim.free_harbor_spots(me, sea))
                .unwrap_or_default()
                .into_iter()
                .min_by_key(|p| (g.distance(bd.pos, *p), p.y, p.x));
            if let Some(to) = nearest
                && ui.button(crate::i18n::tr("⛵ Sail to the nearest free coast")).clicked()
            {
                cmds.push(Cmd::SendExpedition { harbor: bd.pos, to });
            }
        }
        Some(e) => {
            ui.label(format!(
                "{} {}/{} {}, {}/{} {}, {} {}",
                crate::i18n::tr("Expedition:"),
                e.boards,
                load.0,
                crate::i18n::tr("boards"),
                e.stones,
                load.1,
                crate::i18n::tr("stones"),
                crate::i18n::tr("builder"),
                if e.builder { "✔" } else { "…" }
            ));
            if e.boards == load.0 && e.stones == load.1 && e.builder {
                ui.label(crate::i18n::tr("Waiting for a ship."));
            }
        }
    }
    let ships = sim.ships().iter().filter(|(_, s)| s.player == me && s.docked == Some(b)).count();
    ui.label(format!("{} {ships}", crate::i18n::tr("Ships here:")));
    // a ship explores the coasts still unseen (RTTR exploration expedition)
    let unseen = sim.harbor_sea(bd.pos).map_or(0, |sea| sim.unexplored_spots(me, sea, bd.pos).len());
    if unseen > 0
        && sim.idle_ship_for(me, b).is_some()
        && ui
            .button(crate::i18n::tr("🔭 Explore the coasts"))
            .on_hover_text(crate::i18n::tr(
                "A ship sails to coasts you haven't seen yet, looking far around, and comes back.",
            ))
            .clicked()
    {
        cmds.push(Cmd::StartExploration { harbor: bd.pos });
    }
    let spots = known_free_spots(app, bd.pos);
    if spots.is_empty() {
        ui.label(
            RichText::new(crate::i18n::tr("No free harbor spot known across this sea: explore its coasts.")).small(),
        );
        return;
    }
    ui.label(crate::i18n::fill("Free harbor spots across the sea: {1}", &[&spots.len().to_string()]));
    ui.horizontal_wrapped(|ui| {
        for (i, p) in spots.iter().take(6).enumerate() {
            if ui.small_button(format!("⚓ {}", i + 1)).on_hover_text(crate::i18n::tr("Show on the map")).clicked() {
                ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(INSET_GO), *p));
            }
        }
    });
}

/// The free harbor spots on the sea of the harbor at `harbor` that we have explored, nearest first.
pub(crate) fn known_free_spots(app: &App, harbor: MapCoord) -> Vec<MapCoord> {
    let sim = &app.sim;
    let Some(sea) = sim.harbor_sea(harbor) else { return Vec::new() };
    let g = sim.world().grid();
    let mut spots: Vec<MapCoord> = sim
        .free_harbor_spots(app.player, sea)
        .into_iter()
        .filter(|p| sim.visibility(app.player, *p) != rr_sim::Visibility::Unexplored)
        .collect();
    spots.sort_by_key(|p| (g.distance(harbor, *p), p.y, p.x));
    spots
}

/// The temp-data key of a building window's live view clicked: the node the map goes to.
pub const INSET_GO: &str = "inset_go";

/// The building and the land around it, live: its people, wares and smoke, the fog as on the map
/// (a picture in picture; the owner's playtest, 2026-10-27: a still picture of the building read
/// as a view that didn't work). A click takes the map there.
fn building_view(app: &App, ui: &mut egui::Ui, at: MapCoord) {
    let Some(tex) = app.tex.as_ref() else { return };
    let size = Vec2::new(ui.available_width().max(260.0), 150.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let view = app.inset_view(rect, at);
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, Color32::from_rgb(10, 20, 40));
    view.draw(&painter, tex);
    view.draw_shots(&painter, &app.session.shots);
    view.draw_priorities(&painter);
    painter.rect_stroke(
        rect,
        4.0,
        egui::Stroke::new(1.5_f32, Color32::from_rgb(120, 90, 50)),
        egui::StrokeKind::Inside,
    );
    if resp.on_hover_text(crate::i18n::tr("Show on the map")).clicked() {
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(INSET_GO), at));
    }
    // (the map behind keeps moving: draw the view again next frame)
    ui.ctx().request_repaint();
}

/// "Belongs to" a player, by name (never translated).
fn owner_line(sim: &rr_sim::Sim, p: u8) -> String {
    crate::i18n::fill("Belongs to {1}", &[&rr_game::plain(&sim.players()[p as usize].setup.name)])
}

#[allow(clippy::too_many_lines)] // to split (architect round 2, #8: no new long ones)
fn building_info(
    app: &App,
    ui: &mut egui::Ui,
    b: rr_sim::BuildingId,
    cmds: &mut Vec<Cmd>,
    close: &mut bool,
    follow: &mut Option<rr_sim::FigId>,
    attack: &mut AttackPanel,
) {
    let sim = &app.sim;
    let bd = sim.building(b);
    let def = sim.rules().building(bd.kind);
    let me = app.player;
    building_view(app, ui, bd.pos);
    if bd.player != me {
        ui.label(owner_line(sim, bd.player));
        if sim.are_allies(bd.player, me) {
            ui.label(crate::i18n::tr("An ally."));
        } else if (def.is_military() || bd.kind.is_warehouse()) && !sim.is_attackable(me, b) {
            ui.label(crate::i18n::tr("Not attackable: nobody has moved in yet, or you can't see it."));
        } else if def.is_military() || bd.kind.is_warehouse() {
            attack_panel(app, ui, b, cmds, close, attack);
        }
        return;
    }
    match &bd.state {
        BState::Site(s) => {
            let pct = sim.site_progress_percent(b);
            ui.add(egui::ProgressBar::new(pct as f32 / 100.0).text(format!(
                "{} {}",
                crate::i18n::tr("Under construction:"),
                crate::i18n::pct(pct)
            )));
            ui.label(format!(
                "{} {}/{}  {} {}/{}",
                crate::i18n::tr("Boards"),
                s.used_boards + s.boards,
                def.boards,
                crate::i18n::tr("Stones"),
                s.used_stones + s.stones,
                def.stones
            ));
            if s.planing {
                ui.label(crate::i18n::tr("Waiting for the planer to level the ground."));
            } else if s.builder.is_none() {
                ui.label(crate::i18n::tr("Waiting for a builder."));
            }
        }
        BState::Ready => {
            if bd.kind == B::Harbor {
                harbor_panel(app, ui, b, cmds);
            }
            if let Some(inv) = &bd.inventory {
                ui.label(format!("{} {}", crate::i18n::tr("Settlers:"), inv.job(Job::Helper)));
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("Soldiers:"));
                    for (r, j) in Job::SOLDIERS.iter().enumerate() {
                        if inv.job(*j) > 0 {
                            rank_icon(ui, r as u8);
                            ui.label(inv.job(*j).to_string()).on_hover_text(job_name(*j));
                        }
                    }
                });
                egui::Grid::new("wh_wares").num_columns(6).striped(true).show(ui, |ui| {
                    let mut n = 0;
                    for w in Ware::ALL {
                        if inv.ware(*w) == 0 {
                            continue;
                        }
                        icon(ui, *w);
                        ui.label(format!("{} {}", ware_name(*w), inv.ware(*w)));
                        n += 1;
                        if n % 3 == 0 {
                            ui.end_row();
                        }
                    }
                });
                // RTTR: soldiers kept at home per rank, to defend the warehouse
                ui.collapsing(crate::i18n::tr("Reserve soldiers"), |ui| {
                    egui::Grid::new("wh_reserve").num_columns(4).show(ui, |ui| {
                        for (r, j) in Job::SOLDIERS.iter().enumerate() {
                            let n = bd.reserve[r];
                            rank_icon(ui, r as u8);
                            ui.label(job_name(*j));
                            ui.horizontal(|ui| {
                                if ui.small_button("−").clicked() && n > 0 {
                                    cmds.push(Cmd::SetReserve { at: bd.pos, rank: r as u8, count: n - 1 });
                                }
                                ui.label(RichText::new(n.to_string()).monospace());
                                if ui.small_button("+").clicked() && n < rr_sim::MAX_RESERVE {
                                    cmds.push(Cmd::SetReserve { at: bd.pos, rank: r as u8, count: n + 1 });
                                }
                            });
                            ui.end_row();
                        }
                    });
                });
                // the original's warehouse settings: click a ware to cycle what happens to it
                ui.collapsing(crate::i18n::tr("Stock settings"), |ui| {
                    ui.label(
                        RichText::new(crate::i18n::tr("· normal  ⬇ collect here  ⛔ don't store  ⬆ move out")).small(),
                    );
                    egui::Grid::new("wh_policy").spacing([6.0, 4.0]).show(ui, |ui| {
                        for (i, w) in INVENTORY_ORDER.iter().enumerate() {
                            let p = bd.stock_policy.get(w.index()).copied().unwrap_or_default();
                            let mark = match p {
                                rr_sim::StockPolicy::Normal => "·",
                                rr_sim::StockPolicy::Collect => "⬇",
                                rr_sim::StockPolicy::Stop => "⛔",
                                rr_sim::StockPolicy::MoveOut => "⬆",
                            };
                            let r = ui
                                .horizontal(|ui| {
                                    icon(ui, *w);
                                    ui.small_button(mark)
                                })
                                .inner
                                .on_hover_text(ware_name(*w));
                            if r.clicked() {
                                let next = match p {
                                    rr_sim::StockPolicy::Normal => rr_sim::StockPolicy::Collect,
                                    rr_sim::StockPolicy::Collect => rr_sim::StockPolicy::Stop,
                                    rr_sim::StockPolicy::Stop => rr_sim::StockPolicy::MoveOut,
                                    rr_sim::StockPolicy::MoveOut => rr_sim::StockPolicy::Normal,
                                };
                                cmds.push(Cmd::SetStockPolicy { at: bd.pos, ware: *w, policy: next });
                            }
                            if i % 6 == 5 {
                                ui.end_row();
                            }
                        }
                    });
                });
                ui.collapsing(crate::i18n::tr("Settler settings"), |ui| {
                    egui::Grid::new("wh_people").spacing([6.0, 4.0]).show(ui, |ui| {
                        for (i, j) in Job::ALL.iter().enumerate() {
                            let p = bd.people_policy.get(j.index()).copied().unwrap_or_default();
                            let mark = match p {
                                rr_sim::StockPolicy::Normal => "·",
                                rr_sim::StockPolicy::Collect => "⬇",
                                rr_sim::StockPolicy::Stop => "⛔",
                                rr_sim::StockPolicy::MoveOut => "⬆",
                            };
                            let r = ui
                                .horizontal(|ui| {
                                    ui.label(RichText::new(job_name(*j)).small());
                                    ui.small_button(mark)
                                })
                                .inner;
                            if r.clicked() {
                                let next = match p {
                                    rr_sim::StockPolicy::Normal => rr_sim::StockPolicy::Collect,
                                    rr_sim::StockPolicy::Collect => rr_sim::StockPolicy::Stop,
                                    rr_sim::StockPolicy::Stop => rr_sim::StockPolicy::MoveOut,
                                    rr_sim::StockPolicy::MoveOut => rr_sim::StockPolicy::Normal,
                                };
                                cmds.push(Cmd::SetPeoplePolicy { at: bd.pos, job: *j, policy: next });
                            }
                            if i % 3 == 2 {
                                ui.end_row();
                            }
                        }
                    });
                });
                ui.collapsing(crate::i18n::tr("Settlers"), |ui| {
                    egui::Grid::new("wh_jobs").num_columns(2).show(ui, |ui| {
                        for (i, j) in Job::ALL.iter().filter(|j| inv.job(**j) > 0).enumerate() {
                            ui.label(format!("{} {}", job_name(*j), inv.job(*j)));
                            if i % 2 == 1 {
                                ui.end_row();
                            }
                        }
                    });
                });
            } else if def.is_military() {
                let m = def.military.as_ref().unwrap();
                ui.label(format!(
                    "{} {} / {}  ({} {})",
                    crate::i18n::tr("Soldiers:"),
                    bd.soldiers.len(),
                    m.soldiers,
                    crate::i18n::tr("wanted"),
                    bd.garrison_wanted
                ));
                // the military settings cap the garrison by how close the enemy is: say so
                let cap = sim.required_troops(b);
                if cap < bd.garrison_wanted as usize {
                    // how close the enemy is, in plain words (the border of the land isn't the
                    // same as a border with an enemy)
                    let area = GARRISON_AREAS[sim.frontier_distance(b).min(3)];
                    ui.label(RichText::new(garrison_cap_line(cap, area)).small().color(Color32::LIGHT_YELLOW));
                }
                ui.horizontal(|ui| {
                    for s in &bd.soldiers {
                        let f = sim.figure(*s);
                        let r = ui.scope(|ui| rank_icon(ui, f.job.soldier_rank().unwrap_or(0))).response;
                        r.on_hover_text(job_name(f.job));
                    }
                    if !bd.soldiers.is_empty() {
                        ui.label(RichText::new(crate::i18n::tr("(one mark per soldier, by rank)")).small());
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button(crate::i18n::tr("−")).on_hover_text(crate::i18n::tr("Fewer soldiers")).clicked() {
                        cmds.push(Cmd::SetGarrisonWanted { at: bd.pos, wanted: bd.garrison_wanted.saturating_sub(1) });
                    }
                    if ui.button(crate::i18n::tr("+")).on_hover_text(crate::i18n::tr("More soldiers")).clicked() {
                        cmds.push(Cmd::SetGarrisonWanted { at: bd.pos, wanted: bd.garrison_wanted + 1 });
                    }
                    // promoted soldiers inland do nothing: send the strongest home, from where
                    // the front gets it (RTTR's military building window; item 307)
                    if bd.soldiers.len() > 1
                        && ui
                            .button(crate::i18n::tr("⬆ Send the strongest home"))
                            .on_hover_text(crate::i18n::tr(
                                "Your strongest soldier here goes to the castle and from there to where soldiers are wanted",
                            ))
                            .clicked()
                    {
                        cmds.push(Cmd::SendHome { at: bd.pos });
                    }
                    // (watching the village: a post has no worker to follow, but soldiers)
                    if let Some(s) = bd.soldiers.first()
                        && ui
                            .small_button(crate::i18n::tr("Follow a soldier"))
                            .on_hover_text(crate::i18n::tr("The view goes with the soldier; move the map to stop"))
                            .clicked()
                    {
                        *follow = Some(*s);
                        *close = true;
                    }
                    let label = crate::i18n::tr(if bd.coins_stopped { "Gold: off" } else { "Gold: on" });
                    if ui
                        .button(label)
                        .on_hover_text(crate::i18n::tr(
                            "Gold coins promote the soldiers here; switch deliveries on or off",
                        ))
                        .clicked()
                    {
                        cmds.push(Cmd::ToggleProduction { at: bd.pos });
                    }
                });
                ui.label(format!("{} {}/{}", crate::i18n::tr("Coins:"), bd.stock[0], m.coins)).on_hover_text(
                    crate::i18n::tr("Each gold coin promotes one soldier of every rank present here by one rank."),
                );
            } else {
                match bd.worker {
                    Some(_) if bd.kind == rr_rules::BuildingType::LookoutTower => {
                        ui.label(crate::i18n::tr("The scout keeps watch: the tower sees far around."));
                    }
                    Some(_) => {
                        let pct = bd.productivity();
                        ui.add(egui::ProgressBar::new(pct as f32 / 100.0).text(format!(
                            "{} {}",
                            crate::i18n::tr("Productivity:"),
                            crate::i18n::pct(pct)
                        )));
                    }
                    None => {
                        ui.label(crate::i18n::tr("Waiting for a worker."));
                    }
                };
                // watching the village (genre lessons, PLAN §27, #7)
                if let Some(w) = bd.worker
                    && ui
                        .small_button(crate::i18n::tr("Follow the worker"))
                        .on_hover_text(crate::i18n::tr("The view goes with the worker; move the map to stop"))
                        .clicked()
                {
                    *follow = Some(w);
                    *close = true;
                }
                ui.horizontal(|ui| {
                    for (i, w) in def.inputs.iter().enumerate() {
                        icon(ui, *w);
                        ui.label(format!("{}/{}", bd.stock[i], def.slots_per_input)).on_hover_text(ware_name(*w));
                    }
                    if let Some(out) = def.output {
                        ui.label("➡");
                        icon(ui, out);
                        ui.label(ware_name(out));
                    }
                });
                // what holds it up, by name (genre lessons, PLAN §27, #4)
                if let Some(why) = building_cause(sim, b) {
                    ui.label(RichText::new(why).color(Color32::LIGHT_RED));
                }
                if bd.kind == B::Shipyard {
                    ui.horizontal(|ui| {
                        for (ships, label) in [(false, "Boats"), (true, "Ships")] {
                            if ui.selectable_label(bd.build_ships == ships, crate::i18n::tr(label)).clicked()
                                && bd.build_ships != ships
                            {
                                cmds.push(Cmd::SetShipyardMode { at: bd.pos, ships });
                            }
                        }
                    });
                }
                if def.worker.is_some() {
                    let label =
                        crate::i18n::tr(if bd.stopped { "▶ Start production" } else { "⏸ Stop production" });
                    if ui.button(label).clicked() {
                        cmds.push(Cmd::ToggleProduction { at: bd.pos });
                    }
                }
            }
        }
    }
    priority_and_demolish(ui, bd, cmds, close);
    let _ = Size::Hut;
}

/// A building window's last row: the player's priority (sites and workshops, not warehouses:
/// they ask for nothing), then demolishing, which asks first.
fn priority_and_demolish(ui: &mut egui::Ui, bd: &rr_sim::Building, cmds: &mut Vec<Cmd>, close: &mut bool) {
    if !(bd.is_ready() && bd.inventory.is_some()) {
        let on = bd.priority();
        let label = crate::i18n::tr(if on { "★ Priority: on" } else { "☆ Priority: off" });
        let r = ui.selectable_label(on, label).on_hover_text(crate::i18n::tr(
            "Built and supplied before your other buildings, and gets its people first.",
        ));
        if r.clicked() {
            cmds.push(Cmd::SetPriority { at: bd.pos, on: !on });
        }
    }
    if bd.kind != B::Headquarters {
        // like the original: demolishing asks first
        let id = egui::Id::new(("demolish", bd.pos.x, bd.pos.y));
        let asking = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
        if !asking {
            if ui.button(crate::i18n::tr("🔥 Demolish")).clicked() {
                ui.data_mut(|d| d.insert_temp(id, true));
            }
        } else {
            ui.horizontal(|ui| {
                ui.label(RichText::new(crate::i18n::tr("Really demolish?")).color(Color32::LIGHT_RED));
                if ui.button(crate::i18n::tr("Yes")).clicked() {
                    cmds.push(Cmd::RemoveBuilding { at: bd.pos });
                    ui.data_mut(|d| d.remove::<bool>(id));
                    *close = true;
                }
                if ui.button(crate::i18n::tr("No")).clicked() {
                    ui.data_mut(|d| d.remove::<bool>(id));
                }
            });
        }
    }
}

/// Wares in the order a player thinks of them: building materials, food, metal, war, tools.
const INVENTORY_ORDER: &[Ware] = {
    use Ware::*;
    &[
        Wood, Boards, Stones, Grain, Flour, Water, Fish, Meat, Ham, Bread, Beer, Coal, IronOre, Iron, Gold, Coins,
        Sword, Shield, Boat, Axe, Saw, PickAxe, Hammer, Shovel, Crucible, RodAndLine, Scythe, Cleaver, Rollingpin, Bow,
        Tongs,
    ]
};

/// Where a ware on the way goes: a building site, or a building of a kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Way {
    Site,
    To(B),
}

/// Goals counted, most first (`None`, no goal yet, last).
fn group_ways(goals: impl Iterator<Item = Option<Way>>) -> Vec<(Option<Way>, u32)> {
    let mut counts: std::collections::BTreeMap<Option<Way>, u32> = Default::default();
    for g in goals {
        *counts.entry(g).or_default() += 1;
    }
    let mut out: Vec<(Option<Way>, u32)> = counts.into_iter().collect();
    out.sort_by_key(|(g, n)| (g.is_none(), std::cmp::Reverse(*n), *g));
    out
}

/// The player's wares on the way, by kind and where they go (PLAN §27, #5: "where are my boards
/// going?"), shown when hovering a ware in the inventory.
fn ways_by_ware(sim: &rr_sim::Sim, me: u8) -> std::collections::BTreeMap<rr_rules::Ware, Vec<(Option<Way>, u32)>> {
    let mut goals: std::collections::BTreeMap<rr_rules::Ware, Vec<Option<Way>>> = Default::default();
    for (_, w) in sim.wares().iter().filter(|(_, w)| w.player == me) {
        let way =
            w.goal.and_then(|b| sim.buildings().get(b)).map(|b| if b.is_ready() { Way::To(b.kind) } else { Way::Site });
        goals.entry(w.kind).or_default().push(way);
    }
    goals.into_iter().map(|(k, g)| (k, group_ways(g.into_iter()))).collect()
}

/// A ware's hover in the inventory: its name, and where those on the way go.
fn ware_tip(w: rr_rules::Ware, ways: &[(Option<Way>, u32)]) -> String {
    let mut tip = ware_name(w);
    let total: u32 = ways.iter().map(|(_, n)| n).sum();
    if total > 0 {
        tip += &format!("\n{}", crate::i18n::fill("On the way: {1}", &[&total.to_string()]));
    }
    for (way, n) in ways {
        let n = n.to_string();
        tip += "\n";
        tip += &match way {
            Some(Way::Site) => crate::i18n::fill("{1} to building sites", &[&n]),
            Some(Way::To(b)) => crate::i18n::fill("{1} for: {2}", &[&n, building_key(*b)]),
            None => crate::i18n::fill("{1} without a goal yet", &[&n]),
        };
    }
    tip
}

/// A tooltip over a whole cell, above the labels in it (they took the hover, so a cell's own
/// tooltip never showed).
fn hover_tip(ui: &mut egui::Ui, id: impl std::hash::Hash, rect: egui::Rect, tip: String) {
    ui.interact(rect, ui.id().with(id), egui::Sense::hover()).on_hover_text(tip);
}

fn inventory(app: &App, ui: &mut egui::Ui) {
    let inv = app.sim.player_inventory(app.player);
    let ways = ways_by_ware(&app.sim, app.player);
    let tip = |w: rr_rules::Ware| ware_tip(w, ways.get(&w).map_or(&[], |v| v.as_slice()));
    let id = egui::Id::new("inventory_tab");
    let mut tab = ui.data(|d| d.get_temp::<usize>(id)).unwrap_or(0);
    ui.horizontal(|ui| {
        for (i, n) in ["Wares", "Settlers", "Soldiers"].iter().enumerate() {
            if ui.selectable_label(tab == i, crate::i18n::tr(n)).clicked() {
                tab = i;
            }
        }
    });
    ui.data_mut(|d| d.insert_temp(id, tab));
    let names_id = egui::Id::new("inventory_names");
    // (names shown unless switched off: a11y round 2, #10, tools differ by a few pixels)
    let mut names = ui.data(|d| d.get_temp::<bool>(names_id)).unwrap_or(true);
    if tab == 0 {
        ui.checkbox(&mut names, crate::i18n::tr("With names"));
        ui.data_mut(|d| d.insert_temp(names_id, names));
    }
    ui.separator();
    if tab == 0 && names {
        egui::Grid::new("inv_names").spacing([10.0, 4.0]).show(ui, |ui| {
            for (i, w) in INVENTORY_ORDER.iter().enumerate() {
                let r = ui.horizontal(|ui| {
                    icon_sized(ui, *w, 18.0);
                    let n = inv.ware(*w);
                    let txt = RichText::new(format!("{n:>3}")).monospace();
                    ui.label(if n == 0 { txt.weak() } else { txt.strong() });
                    ui.label(ware_name(*w));
                });
                hover_tip(ui, ("inv_tip", i), r.response.rect, tip(*w));
                if i % 3 == 2 {
                    ui.end_row();
                }
            }
        });
        return;
    }
    // an icon grid like the original's; empty entries are dimmed
    // settler names need wider cells than ware pictures (no breaks inside a word)
    let width = if tab == 1 { 96.0 } else { 52.0 };
    let cell = |ui: &mut egui::Ui, n: u32, draw: &dyn Fn(&mut egui::Ui), tip: String| {
        let r = ui.vertical_centered(|ui| {
            ui.set_width(width);
            draw(ui);
            let txt = RichText::new(n.to_string()).monospace();
            ui.label(if n == 0 { txt.weak() } else { txt.strong() });
        });
        hover_tip(ui, ("inv_cell", r.response.rect.min.x as i32, r.response.rect.min.y as i32), r.response.rect, tip);
    };
    egui::Grid::new(("inv", tab)).spacing([4.0, 6.0]).show(ui, |ui| match tab {
        0 => {
            for (i, w) in INVENTORY_ORDER.iter().enumerate() {
                cell(ui, inv.ware(*w), &|ui| icon_sized(ui, *w, 28.0), tip(*w));
                if i % 7 == 6 {
                    ui.end_row();
                }
            }
        }
        1 => {
            let people: Vec<Job> = Job::ALL.iter().copied().filter(|j| j.soldier_rank().is_none()).collect();
            for (i, j) in people.iter().enumerate() {
                cell(
                    ui,
                    inv.job(*j),
                    &|ui| {
                        ui.label(job_name(*j));
                    },
                    job_name(*j),
                );
                if i % 4 == 3 {
                    ui.end_row();
                }
            }
        }
        _ => {
            for (r, j) in Job::SOLDIERS.iter().enumerate() {
                cell(ui, inv.job(*j), &|ui| rank_icon(ui, r as u8), job_name(*j));
            }
        }
    });
}

/// A soldier's rank as chevrons (one per rank above private), the original shows the rank too.
fn rank_icon(ui: &mut egui::Ui, rank: u8) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(26.0, 18.0), egui::Sense::hover());
    let p = ui.painter();
    let c = r.center();
    p.circle_filled(c + Vec2::new(0.0, -2.0), 5.0, Color32::from_rgb(200, 60, 50));
    for k in 0..rank {
        let y = 6.0 - k as f32 * 2.5;
        p.line_segment(
            [c + Vec2::new(-5.0, y - 1.5), c + Vec2::new(0.0, y + 1.0)],
            egui::Stroke::new(1.4_f32, Color32::GOLD),
        );
        p.line_segment(
            [c + Vec2::new(0.0, y + 1.0), c + Vec2::new(5.0, y - 1.5)],
            egui::Stroke::new(1.4_f32, Color32::GOLD),
        );
    }
}

fn distribution(app: &mut App, ui: &mut egui::Ui) {
    let entries = app.sim.players()[app.player as usize].settings.distribution.clone();
    // one tab per ware that has more than one kind of consumer (as in the original)
    let mut tabs: Vec<Ware> = Vec::new();
    for (w, _, _) in &entries {
        if !tabs.contains(w) && entries.iter().filter(|e| e.0 == *w).count() > 1 {
            tabs.push(*w);
        }
    }
    let id = egui::Id::new("distribution_tab");
    let mut tab = ui.data(|d| d.get_temp::<usize>(id)).unwrap_or(0).min(tabs.len().saturating_sub(1));
    let title = |w: Ware| if w == Ware::Fish { crate::i18n::tr("Food").to_string() } else { ware_name(w) };
    ui.horizontal_wrapped(|ui| {
        for (i, w) in tabs.iter().enumerate() {
            if ui.selectable_label(tab == i, title(*w)).clicked() {
                tab = i;
            }
        }
    });
    ui.data_mut(|d| d.insert_temp(id, tab));
    ui.separator();
    let Some(&ware) = tabs.get(tab) else { return };
    for (w, b, v) in entries.into_iter().filter(|e| e.0 == ware) {
        let mut val = v;
        let name = if b == B::Headquarters { crate::i18n::tr("Construction") } else { building_name(b) };
        if ui.add(egui::Slider::new(&mut val, 0..=10).text(name)).changed() {
            app.cmd(Cmd::SetDistribution { ware: w, building: b, weight: val });
        }
    }
}

/// The transport window's entries: wares that always share a priority (RTTR's standard
/// transport groups, as in the original's window).
const TRANSPORT_GROUPS: &[(&str, &[Ware])] = {
    use Ware::*;
    &[
        ("Coins", &[Coins]),
        ("Weapons", &[Sword, Shield]),
        ("Beer", &[Beer]),
        ("Iron", &[Iron]),
        ("Gold", &[Gold]),
        ("Iron ore", &[IronOre]),
        ("Coal", &[Coal]),
        ("Boards", &[Boards]),
        ("Stones", &[Stones]),
        ("Wood", &[Wood]),
        ("Water", &[Water]),
        ("Food", &[Fish, Meat, Bread, Grain, Flour, Ham]),
        ("Tools", &[Tongs, Hammer, Axe, Saw, PickAxe, Shovel, Crucible, RodAndLine, Scythe, Cleaver, Rollingpin, Bow]),
        ("Boats", &[Boat]),
    ]
};

fn transport(app: &mut App, ui: &mut egui::Ui) {
    let order = app.sim.players()[app.player as usize].settings.transport_order.clone();
    let pos = |w: &Ware| order.iter().position(|x| x == w).unwrap_or(usize::MAX);
    // groups in the order of their highest ware
    let mut groups: Vec<usize> = (0..TRANSPORT_GROUPS.len()).collect();
    groups.sort_by_key(|g| TRANSPORT_GROUPS[*g].1.iter().map(pos).min());
    ui.label(crate::i18n::tr("Carriers take the higher wares first."));
    let mut moved: Option<Vec<usize>> = None;
    let n = groups.len();
    for (i, g) in groups.iter().enumerate() {
        let (name, wares) = TRANSPORT_GROUPS[*g];
        ui.horizontal(|ui| {
            let mv = |to: usize| {
                let mut v = groups.clone();
                let x = v.remove(i);
                v.insert(to, x);
                v
            };
            ui.add_enabled_ui(i > 0, |ui| {
                if ui.small_button("⏫").on_hover_text(crate::i18n::tr("Top")).clicked() {
                    moved = Some(mv(0));
                }
                if ui.small_button("⬆").on_hover_text(crate::i18n::tr("Up")).clicked() {
                    moved = Some(mv(i - 1));
                }
            });
            ui.add_enabled_ui(i + 1 < n, |ui| {
                if ui.small_button("⬇").on_hover_text(crate::i18n::tr("Down")).clicked() {
                    moved = Some(mv(i + 1));
                }
                if ui.small_button("⏬").on_hover_text(crate::i18n::tr("Bottom")).clicked() {
                    moved = Some(mv(n - 1));
                }
            });
            icon(ui, wares[0]);
            ui.label(crate::i18n::tr(name));
        });
    }
    if ui.button(crate::i18n::tr("Default")).clicked() {
        moved = Some((0..TRANSPORT_GROUPS.len()).collect());
    }
    if let Some(v) = moved {
        let order: Vec<Ware> = v.iter().flat_map(|g| TRANSPORT_GROUPS[*g].1.iter().copied()).collect();
        app.cmd(Cmd::SetTransportOrder { order });
    }
}

fn tools(app: &mut App, ui: &mut egui::Ui) {
    let tools = app.sim.players()[app.player as usize].settings.tools.clone();
    let mut changed = false;
    let mut vals = tools.clone();
    for (w, v) in vals.iter_mut() {
        changed |= ui.add(egui::Slider::new(v, 0..=10).text(ware_name(*w))).changed();
    }
    if changed {
        app.cmd(Cmd::SetTools { values: vals });
    }
}

fn military(app: &mut App, ui: &mut egui::Ui) {
    let mut m = app.sim.players()[app.player as usize].settings.military;
    let names = [
        "Recruits (fewer … more)",
        "Defense (weak … strong soldiers first)",
        "Defenders sent to help",
        "Attackers per building",
        "Troops in the interior",
        "Troops in the middle",
        "Troops near harbors",
        "Troops at the border",
    ];
    let mut changed = false;
    for (i, n) in names.iter().enumerate() {
        changed |=
            ui.add(egui::Slider::new(&mut m[i], 0..=rr_sim::MILITARY_SCALE[i]).text(crate::i18n::tr(n))).changed();
    }
    if changed {
        app.cmd(Cmd::SetMilitary { values: m });
    }
}

fn stats(app: &mut App, ui: &mut egui::Ui) {
    let mut cats: Vec<&str> = rr_sim::Sample::CATEGORIES.to_vec();
    cats.push("Your buildings");
    cats.push("Your wares");
    let own = cats.len() - 2;
    let wares_tab = cats.len() - 1;
    ui.horizontal_wrapped(|ui| {
        for (i, c) in cats.iter().enumerate() {
            if ui.selectable_label(app.ui.stats_cat == i, crate::i18n::tr(c)).clicked() {
                app.ui.stats_cat = i;
            }
        }
    });
    let players = app.sim.players();
    if app.ui.stats_cat == wares_tab {
        ware_stats(app, ui);
        return;
    }
    if app.ui.stats_cat == own {
        let _ = building_overview(app, ui);
        return;
    }
    let cat = app.ui.stats_cat;
    let pick = |s: &rr_sim::Sample| s.value(cat);
    // time range: 15 minutes, 1 hour, 4 hours or everything (one sample per 30 s)
    let rid = egui::Id::new("stats_range");
    let mut range = ui.data(|d| d.get_temp::<usize>(rid)).unwrap_or(3);
    ui.horizontal(|ui| {
        for (i, n) in ["15 min", "1 h", "4 h", "all"].iter().enumerate() {
            if ui.selectable_label(range == i, crate::i18n::tr(n)).clicked() {
                range = i;
            }
        }
    });
    ui.data_mut(|d| d.insert_temp(rid, range));
    let keep = [30usize, 120, 480, usize::MAX][range.min(3)];
    let window = |s: &[rr_sim::Sample]| -> Vec<rr_sim::Sample> { s[s.len().saturating_sub(keep)..].to_vec() };
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().max(420.0), 200.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, CHART_BG);
    let max =
        players.iter().flat_map(|p| window(&p.stats.samples).into_iter().map(|s| pick(&s))).max().unwrap_or(1).max(1);
    painter.text(
        rect.left_top() + Vec2::new(4.0, 2.0),
        egui::Align2::LEFT_TOP,
        max.to_string(),
        egui::FontId::proportional(11.0),
        Color32::from_gray(200),
    );
    let max = max as f32;
    // the viewer's line last, so it lies on top (art director round 3, #7)
    let me = app.player as usize;
    let order: Vec<usize> = (0..players.len()).filter(|i| *i != me).chain(std::iter::once(me)).collect();
    for i in order {
        let Some(p) = players.get(i) else { continue };
        let samples = window(&p.stats.samples);
        let n = samples.len();
        if n < 2 {
            continue;
        }
        let pts: Vec<egui::Pos2> = samples
            .iter()
            .enumerate()
            .map(|(k, s)| {
                egui::pos2(
                    rect.left() + k as f32 / (n - 1) as f32 * rect.width(),
                    // (headroom: a line at the peak stays under the label, not on the edge)
                    rect.bottom() - pick(s) as f32 / max * (rect.height() - 16.0),
                )
            })
            .collect();
        // a dark edge, and a colour light enough for the dark chart (blue, red and purple were
        // under 3:1)
        let w: f32 = if i == me { 3.0 } else { 2.0 };
        painter.add(egui::Shape::line(pts.clone(), egui::Stroke::new(w + 1.5, Color32::from_black_alpha(140))));
        let col = chart_color(crate::render::player_color(i as u8));
        painter.add(egui::Shape::line(pts.clone(), egui::Stroke::new(w, col)));
        for k in marks_along(pts.len(), 6, i) {
            draw_marker(&painter, pts[k], marker(i as u8), col);
        }
    }
    // when: the shown samples' first and last game time (item 304)
    if let Some(p) = players.iter().find(|p| p.stats.samples.len() >= 2) {
        let w = window(&p.stats.samples);
        time_axis(ui, rect, w[0].gf, w[w.len() - 1].gf);
        ui.add_space(14.0);
    }
    let me = app.player;
    let allied = allied_rivals(&app.sim, me);
    egui::Grid::new("st").striped(true).show(ui, |ui| {
        ui.label(crate::i18n::tr("Player"));
        for h in rr_sim::Sample::CATEGORIES {
            let r = ui.label(crate::i18n::tr(h));
            if h == "Military" {
                r.on_hover_text(crate::i18n::tr("Soldiers counted by rank: a general counts 5."));
            } else if h == "Gold" {
                r.on_hover_text(crate::i18n::tr("Gold coins minted so far."));
            }
        }
        ui.end_row();
        for (i, p) in players.iter().enumerate() {
            let s = p.stats.samples.last().copied().unwrap_or_default();
            let name = if !p.alive {
                format!("{} ({})", p.setup.name, crate::i18n::tr("defeated"))
            } else if allied.contains(&(i as u8)) {
                format!("{} ({})", p.setup.name, crate::i18n::tr("rival league"))
            } else {
                p.setup.name.clone()
            };
            // a colour chip, then the name in plain text (coloured text is hard to read on brown)
            ui.horizontal(|ui| {
                // (the line's marker as the chip: a shape, not only a colour)
                let (r, _) = ui.allocate_exact_size(Vec2::new(12.0, 12.0), egui::Sense::hover());
                draw_marker(
                    ui.painter(),
                    r.center(),
                    marker(i as u8),
                    chart_color(crate::render::player_color(i as u8)),
                );
                ui.label(name);
            });
            for c in 0..rr_sim::Sample::CATEGORIES.len() {
                ui.label(s.value(c).to_string());
            }
            ui.end_row();
        }
        // rivals allied against you count together (new-player round 8, #4: "Military 52" for
        // each of five allies against one looked like an even fight)
        if allied.len() >= 2 {
            ui.label(RichText::new(crate::i18n::tr("Rival league together")).strong());
            for v in together(players, &allied) {
                ui.label(RichText::new(v.to_string()).strong());
            }
            ui.end_row();
        }
    });
    soldier_summary(&app.sim, me, ui);
}

/// Where your soldiers are (the castle's only defend the castle), and how many of each rank:
/// the chart counts them weighted (a private 1, a general 5), which read as a head count.
fn soldier_summary(sim: &rr_sim::Sim, me: u8, ui: &mut egui::Ui) {
    let (home, out) = soldiers_home_and_out(sim, me);
    ui.label(crate::i18n::show(&crate::i18n::msg(
        "Your soldiers: {1} in warehouses (they defend only those), {2} in military buildings.",
        &[&home.to_string(), &out.to_string()],
    )));
    let ranks = soldiers_by_rank(sim, me);
    let list: Vec<String> = Job::SOLDIERS.iter().zip(ranks).map(|(j, n)| format!("{} {n}", job_name(*j))).collect();
    let points: u32 = ranks.iter().zip(1u32..).map(|(n, w)| n * w).sum();
    ui.label(crate::i18n::show(&crate::i18n::msg(
        "By rank: {1}. Together {2} soldiers with those on the way, {3} points in the chart.",
        &[&list.join(" · "), &ranks.iter().sum::<u32>().to_string(), &points.to_string()],
    )));
}

/// The player's soldiers of each rank, private to general: in warehouses, in military
/// buildings and on the way (the chart's military curve counts the same soldiers).
fn soldiers_by_rank(sim: &rr_sim::Sim, me: u8) -> [u32; 5] {
    let mut ranks = [0u32; 5];
    let inv = sim.player_inventory(me);
    for (r, j) in Job::SOLDIERS.iter().enumerate() {
        ranks[r] += inv.job(*j);
    }
    for (_, f) in sim.figures().iter().filter(|(_, f)| f.player == me) {
        if let Some(r) = f.job.soldier_rank() {
            ranks[r as usize] += 1;
        }
    }
    ranks
}

/// The shape that marks a player's line in the chart and its row in the table (a11y round 2,
/// #1: colour alone left four rivals the same grey for deutan players).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Marker {
    Circle,
    Square,
    Triangle,
    Diamond,
    Cross,
    Plus,
    Ring,
    Down,
}

fn marker(player: u8) -> Marker {
    use Marker::*;
    [Circle, Square, Triangle, Diamond, Cross, Plus, Ring, Down][player as usize % 8]
}

/// The samples of a line of `n` that carry its marker: about every `every`-th of the width,
/// staggered by player so marks of lines lying together don't cover each other, and the last.
fn marks_along(n: usize, every: usize, player: usize) -> Vec<usize> {
    if n < 2 {
        return Vec::new();
    }
    let step = (n / every.max(1)).max(1);
    let first = (step / 2 + player * step / 8) % step;
    let mut v: Vec<usize> = (first..n - 1).step_by(step).collect();
    v.push(n - 1);
    v
}

fn draw_marker(painter: &egui::Painter, at: egui::Pos2, m: Marker, fill: Color32) {
    let r = 5.0_f32;
    let edge = egui::Stroke::new(1.2_f32, Color32::from_black_alpha(200));
    let poly = |pts: Vec<egui::Pos2>| egui::Shape::convex_polygon(pts, fill, edge);
    let v = |x: f32, y: f32| at + Vec2::new(x, y);
    match m {
        Marker::Circle => {
            painter.circle(at, r, fill, edge);
        }
        Marker::Ring => {
            painter.circle_stroke(at, r, egui::Stroke::new(2.0_f32, fill));
        }
        Marker::Square => {
            painter.rect(
                egui::Rect::from_center_size(at, Vec2::splat(2.0 * r - 1.0)),
                0.0,
                fill,
                edge,
                egui::StrokeKind::Middle,
            );
        }
        Marker::Triangle => {
            painter.add(poly(vec![v(0.0, -r), v(r, r * 0.8), v(-r, r * 0.8)]));
        }
        Marker::Down => {
            painter.add(poly(vec![v(-r, -r * 0.8), v(r, -r * 0.8), v(0.0, r)]));
        }
        Marker::Diamond => {
            painter.add(poly(vec![v(0.0, -r), v(r, 0.0), v(0.0, r), v(-r, 0.0)]));
        }
        Marker::Cross | Marker::Plus => {
            let s = egui::Stroke::new(2.2_f32, fill);
            let (a, b) = if m == Marker::Cross { (v(-r, -r), v(r, r)) } else { (v(-r, 0.0), v(r, 0.0)) };
            let (c, d) = if m == Marker::Cross { (v(-r, r), v(r, -r)) } else { (v(0.0, -r), v(0.0, r)) };
            painter.line_segment([a, b], s);
            painter.line_segment([c, d], s);
        }
    }
}

/// The statistics chart's background.
const CHART_BG: Color32 = Color32::from_rgb(40, 30, 20);

/// WCAG contrast of two colours.
fn contrast(a: Color32, b: Color32) -> f32 {
    let lum = |c: Color32| {
        let ch = |v: u8| {
            let v = v as f32 / 255.0;
            if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * ch(c.r()) + 0.7152 * ch(c.g()) + 0.0722 * ch(c.b())
    };
    let (x, y) = (lum(a), lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// A player colour lightened (towards white) until it stands out 3:1 on the chart.
fn chart_color(c: Color32) -> Color32 {
    let mut k = 0.0;
    loop {
        let mix = |v: u8| (v as f32 + (255.0 - v as f32) * k).round() as u8;
        let out = Color32::from_rgb(mix(c.r()), mix(c.g()), mix(c.b()));
        if contrast(out, CHART_BG) >= 3.0 || k >= 1.0 {
            return out;
        }
        k += 0.05;
    }
}

/// The living rivals allied with each other against `me` (a team of two or more that isn't
/// mine).
fn allied_rivals(sim: &rr_sim::Sim, me: u8) -> Vec<u8> {
    let n = sim.players().len() as u8;
    crate::missions::rivals(sim, me)
        .into_iter()
        .filter(|p| (0..n).any(|q| q != *p && q != me && sim.are_allies(*p, q)))
        .collect()
}

/// The latest values of `who` summed, per statistics category.
fn together(players: &[rr_sim::Player], who: &[u8]) -> Vec<u32> {
    (0..rr_sim::Sample::CATEGORIES.len())
        .map(|c| who.iter().map(|p| players[*p as usize].stats.samples.last().map_or(0, |s| s.value(c))).sum())
        .collect()
}

/// Soldiers in `me`'s warehouses and in its military buildings.
fn soldiers_home_and_out(sim: &rr_sim::Sim, me: u8) -> (u32, u32) {
    let mine = sim.buildings().iter().filter(|(_, b)| b.player == me);
    let (mut home, mut out) = (0, 0);
    for (_, b) in mine {
        match &b.inventory {
            Some(inv) => home += rr_rules::Job::SOLDIERS.iter().map(|j| inv.job(*j)).sum::<u32>(),
            None => out += b.soldiers.len() as u32,
        }
    }
    (home, out)
}

/// The building overview: a picture per kind with how many stand (and are being built) and
/// how well they work.
/// What holds a finished workplace up, if anything (for the overview's problem list).
fn building_problem(sim: &rr_sim::Sim, b: rr_sim::BuildingId) -> Option<&'static str> {
    let bd = sim.building(b);
    let def = sim.rules().building(bd.kind);
    if !bd.is_ready() {
        // a site nobody can reach gets no builder and no material
        let connected = sim.flag(bd.flag).roads.iter().any(|r| r.is_some());
        return (!connected).then_some("No road to this building site.");
    }
    // warehouses and military buildings have no workplace to be held up in
    def.worker?;
    if bd.stopped {
        Some("Stopped.")
    } else if bd.worker.is_none() {
        Some("No worker yet: a settler and the right tool are needed.")
    } else if bd.exhausted {
        Some(idle_reason(bd.kind))
    } else if !def.inputs.is_empty() && (0..def.inputs.len()).all(|i| bd.stock[i] == 0) {
        Some("Waiting for wares.")
    } else if bd.attempts >= 16 && bd.productivity() < 25 {
        Some("Works little (under 25%): too few wares, or its work is far away.")
    } else {
        None
    }
}

/// What holds a building up, said by name (genre lessons, PLAN §27, #4: players of deep
/// settlers-likes most want to know *why*): a finished workplace without a road, with a full
/// flag, or waiting for named wares; otherwise the overview's reasons.
pub(crate) fn building_cause(sim: &rr_sim::Sim, b: rr_sim::BuildingId) -> Option<String> {
    let bd = sim.building(b);
    let def = sim.rules().building(bd.kind);
    if bd.is_ready() && def.worker.is_some() && !bd.stopped {
        let flag = sim.flag(bd.flag);
        if !flag.roads.iter().any(Option::is_some) {
            return Some(crate::i18n::tr("Not connected: no road leads from its flag.").into());
        }
        if flag.wares.len() >= sim.rules().economy.flag_capacity as usize {
            return Some(
                crate::i18n::tr("Its flag is full: the carriers don't take the wares away fast enough. Another road or a donkey helps.")
                    .into(),
            );
        }
        let empty: Vec<String> =
            def.inputs.iter().enumerate().filter(|(i, _)| bd.stock[*i] == 0).map(|(_, w)| ware_name(*w)).collect();
        let stuck = if def.use_one_each { !empty.is_empty() } else { empty.len() == def.inputs.len() };
        if bd.worker.is_some() && !def.inputs.is_empty() && stuck {
            return Some(crate::i18n::show(&crate::i18n::msg("Waiting for: {1}.", &[&empty.join(", ")])));
        }
    }
    building_problem(sim, b).map(|why| crate::i18n::tr(why).to_string())
}

/// Counts and productivity per kind, then the buildings that are held up (click: go there).
fn building_overview(app: &App, ui: &mut egui::Ui) -> Option<MapCoord> {
    let nation = app.sim.players()[app.player as usize].setup.nation as u8;
    egui::Grid::new("bcount").spacing([8.0, 8.0]).show(ui, |ui| {
        let mut k = 0;
        for b in B::ALL {
            let mine: Vec<_> =
                app.sim.buildings().iter().filter(|(_, x)| x.player == app.player && x.kind == *b).collect();
            if mine.is_empty() {
                continue;
            }
            let ready = mine.iter().filter(|(_, x)| x.is_ready()).count();
            let prod: Vec<u32> =
                mine.iter().filter(|(_, x)| x.is_ready() && x.attempts > 0).map(|(_, x)| x.productivity()).collect();
            let cell = ui.vertical_centered(|ui| {
                ui.set_width(76.0);
                if let Some((tex, w, h, _, _)) = app.tex.as_ref().and_then(|t| t.buildings.get(&(*b, nation))) {
                    let s = 40.0 / h.max(*w * 0.8).max(1.0);
                    ui.add(egui::Image::new((tex.id(), Vec2::new(w * s, h * s))));
                }
                ui.label(RichText::new(building_name(*b)).small());
                let sites = mine.len() - ready;
                let n = if sites > 0 { format!("{ready} (+{sites})") } else { ready.to_string() };
                ui.label(RichText::new(n).strong());
                if !prod.is_empty() {
                    let avg = prod.iter().sum::<u32>() / prod.len() as u32;
                    ui.add(egui::ProgressBar::new(avg as f32 / 100.0).desired_width(60.0).text(crate::i18n::pct(avg)));
                }
            });
            let tip = format!(
                "{}: {ready} {}, {} {}",
                building_name(*b),
                crate::i18n::tr("finished"),
                mine.len() - ready,
                crate::i18n::tr("under construction")
            );
            hover_tip(ui, ("overview", k), cell.response.rect, tip);
            k += 1;
            if k % 6 == 0 {
                ui.end_row();
            }
        }
    });
    let problems: Vec<_> = app
        .sim
        .buildings()
        .iter()
        .filter(|(_, x)| x.player == app.player)
        .filter_map(|(id, x)| building_cause(&app.sim, id).map(|why| (x.kind, x.pos, why)))
        .collect();
    if problems.is_empty() {
        return None;
    }
    ui.separator();
    ui.label(RichText::new(crate::i18n::tr("Held up")).strong());
    let mut go = None;
    egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
        for (kind, pos, why) in problems {
            ui.horizontal(|ui| {
                if ui.small_button("➡").on_hover_text(crate::i18n::tr("Go there")).clicked() {
                    go = Some(pos);
                }
                ui.label(building_name(kind));
                ui.label(RichText::new(why).small().color(Color32::LIGHT_RED));
            });
        }
    });
    go
}

/// The wares by kind, for the statistics buttons (item 304: 31 small buttons in one heap).
const WARE_GROUPS: [(&str, &[Ware]); 5] = {
    use Ware::*;
    [
        ("Food", &[Fish, Meat, Ham, Grain, Flour, Bread, Water, Beer]),
        ("Building", &[Wood, Boards, Stones, Boat]),
        ("Mining", &[Coal, IronOre, Iron, Gold, Coins]),
        ("Weapons", &[Sword, Shield]),
        ("Tools", &[Tongs, Hammer, Axe, Saw, PickAxe, Shovel, Crucible, RodAndLine, Scythe, Cleaver, Rollingpin, Bow]),
    ]
};

/// Production per 30 s sample folded into at most `n` bars (sums), and the samples per bar.
/// Folded from the newest end: a short bar is the oldest one, not the latest (QA round 5, #5:
/// production seemed to drop at the right edge).
fn bins(per: &[u32], n: usize) -> (Vec<u32>, usize) {
    let k = per.len().div_ceil(n.max(1)).max(1);
    let mut bars: Vec<u32> = per.rchunks(k).map(|c| c.iter().sum()).collect();
    bars.reverse();
    (bars, k)
}

/// A time axis under a chart: the first and the last sample's game time, and the middle.
fn time_axis(ui: &egui::Ui, rect: egui::Rect, first: u32, last: u32) {
    let p = ui.painter();
    let font = egui::FontId::proportional(11.0);
    let c = Color32::from_gray(190);
    let y = rect.bottom() + 2.0;
    p.text(egui::pos2(rect.left(), y), egui::Align2::LEFT_TOP, fmt_time(first), font.clone(), c);
    p.text(egui::pos2(rect.center().x, y), egui::Align2::CENTER_TOP, fmt_time((first + last) / 2), font.clone(), c);
    p.text(egui::pos2(rect.right(), y), egui::Align2::RIGHT_TOP, fmt_time(last), font, c);
}

/// The original's ware statistics: how much of a ware you made over time.
fn ware_stats(app: &App, ui: &mut egui::Ui) {
    let id = egui::Id::new("ware_stats");
    let mut sel = ui.data(|d| d.get_temp::<usize>(id)).unwrap_or(Ware::Sword.index());
    egui::Grid::new("ware_groups").num_columns(2).show(ui, |ui| {
        for (group, wares) in WARE_GROUPS {
            ui.label(RichText::new(crate::i18n::tr(group)).weak());
            ui.horizontal_wrapped(|ui| {
                for w in wares {
                    let r = ui.add(egui::Button::new(ware_name(*w)).selected(sel == w.index()).small());
                    if r.clicked() {
                        sel = w.index();
                    }
                }
            });
            ui.end_row();
        }
    });
    ui.data_mut(|d| d.insert_temp(id, sel));
    let samples = &app.sim.players()[app.player as usize].stats.samples;
    let per: Vec<u32> = samples.windows(2).map(|s| s[1].by_ware[sel].saturating_sub(s[0].by_ware[sel])).collect();
    // at most 40 bars: a long game's half minutes are mostly 0 or 1
    let (bars, k) = bins(&per, 40);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().max(420.0), 160.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, CHART_BG);
    let max = bars.iter().copied().max().unwrap_or(1).max(1) as f32;
    let n = bars.len().max(1) as f32;
    for (i, v) in bars.iter().enumerate() {
        let x0 = rect.left() + i as f32 / n * rect.width();
        let x1 = rect.left() + (i + 1) as f32 / n * rect.width() - 1.0;
        let y = rect.bottom() - *v as f32 / max * (rect.height() - 16.0);
        painter.rect_filled(
            egui::Rect::from_min_max(egui::pos2(x0, y), egui::pos2(x1.max(x0 + 1.0), rect.bottom())),
            0.0,
            Color32::from_rgb(220, 180, 90),
        );
    }
    painter.text(
        rect.left_top() + Vec2::new(4.0, 2.0),
        egui::Align2::LEFT_TOP,
        max.to_string(),
        egui::FontId::proportional(11.0),
        Color32::from_gray(200),
    );
    if let (Some(a), Some(b)) = (samples.first(), samples.last()) {
        time_axis(ui, rect, a.gf, b.gf);
    }
    ui.add_space(14.0);
    let total = samples.last().map_or(0, |s| s.by_ware[sel]);
    if total == 0 {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            crate::i18n::tr("None made yet"),
            egui::FontId::proportional(14.0),
            Color32::from_gray(150),
        );
    }
    // a sample is 30 s: a bar of k samples covers k / 2 minutes
    let span = if k % 2 == 0 { crate::i18n::fill("{1} min", &[&(k / 2).to_string()]) } else { format!("{} s", k * 30) };
    ui.label(format!(
        "{}: {} {total} · {} {span}",
        ware_name(Ware::ALL[sel]),
        crate::i18n::tr("Made so far:"),
        crate::i18n::tr("one bar:")
    ));
}

fn messages(app: &mut App, ui: &mut egui::Ui) {
    // reading the post office marks everything read
    app.session.read_until = app.sim.gf().0;
    if app.messages.is_empty() {
        ui.label(RichText::new(crate::i18n::tr("No messages.")).weak());
        return;
    }
    if ui.small_button(crate::i18n::tr("🗑 Delete all")).clicked() {
        app.messages.clear();
        return;
    }
    // the original sorts its post: show one kind at a time
    let id = egui::Id::new("post_tab");
    let mut tab = ui.data(|d| d.get_temp::<u8>(id)).unwrap_or(99);
    ui.horizontal(|ui| {
        for (k, n) in [
            (99, "All"),
            (crate::app::MSG_MILITARY, "Military"),
            (crate::app::MSG_ECONOMY, "Economy"),
            (crate::app::MSG_TIP, "Tips"),
            (crate::app::MSG_GENERAL, "Other"),
        ] {
            if ui.selectable_label(tab == k, crate::i18n::tr(n)).clicked() {
                tab = k;
            }
        }
    });
    ui.data_mut(|d| d.insert_temp(id, tab));
    let (mut jump, mut delete) = (None, None);
    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
        for (i, crate::app::Message { gf, text: txt, at, kind }) in app.messages.iter().enumerate() {
            if tab != 99 && *kind != tab {
                continue;
            }
            // kept in English, shown in the language chosen now
            let txt = crate::i18n::show(txt);
            ui.horizontal(|ui| {
                // one message thrown away (owner's playtest; RTTR's post office has it)
                if ui.small_button("✖").on_hover_text(crate::i18n::tr("Delete this message")).clicked() {
                    delete = Some(i);
                }
                ui.label(RichText::new(fmt_time(*gf)).monospace().weak());
                if let Some(p) = at {
                    if ui.link(txt).clicked() {
                        jump = Some(*p);
                    }
                } else {
                    ui.label(txt);
                }
            });
        }
    });
    if let Some(i) = delete {
        app.messages.remove(i);
    }
    if let Some(p) = jump {
        // show the spot where this window doesn't cover it, and the building there
        let screen = ui.ctx().screen_rect();
        let at = free_spot(ui.min_rect().expand(12.0), screen);
        app.cam.show_at(&app.sim, p, at.to_vec2());
        open_building_at(app, p);
    }
}

/// A building's window for the news of `p`, as a click on it would open it (owner's
/// playtest: a message about an exhausted mine only moved the map). Nothing for a spot
/// without a building, or one the fog hides.
pub(crate) fn open_building_at(app: &mut App, p: MapCoord) -> bool {
    let seen = app.sim.visibility(app.player, p) != rr_sim::Visibility::Unexplored;
    if app.sim.building_at(p).is_none() || !seen {
        return false;
    }
    app.session.selected = Some(p);
    let tab = first_tab(app.sim.world().bq_for(p, app.player));
    set_action(app, Action { node: p, tab });
    true
}

impl Ui {
    /// Esc with no road and no action window: the open lists close, and the kept buildings'
    /// windows.
    pub fn close_lists(&mut self) {
        for open in [
            &mut self.main,
            &mut self.saves,
            &mut self.buildings,
            &mut self.inventory,
            &mut self.distribution,
            &mut self.transport,
            &mut self.military,
            &mut self.tools,
            &mut self.stats,
            &mut self.messages,
            &mut self.help,
            &mut self.options,
            &mut self.map,
        ] {
            *open = false;
        }
        self.kept.clear();
    }
}

/// How many buildings' windows stay open beside the action window.
const KEPT_WINDOWS: usize = 6;

/// Open the action window at `a.node`; the building window open until now stays open (kept),
/// the oldest kept one closing when there are too many.
pub(crate) fn set_action(app: &mut App, a: Action) {
    if let Some(old) = app.ui.action
        && old.node != a.node
        && app.sim.building_at(old.node).is_some()
    {
        app.ui.kept.retain(|p| *p != old.node);
        app.ui.kept.push(old.node);
        if app.ui.kept.len() > KEPT_WINDOWS {
            app.ui.kept.remove(0);
        }
    }
    app.ui.kept.retain(|p| *p != a.node);
    app.ui.action = Some(a);
}

/// The kept buildings' windows, then the action window (a holy place's own window there).
fn building_windows(app: &mut App, ctx: &egui::Context) {
    kept_windows(app, ctx);
    match app.ui.action.and_then(|a| rr_world::holy_god(app.sim.world().obj(a.node)).map(|g| (a.node, g))) {
        Some((node, god)) => holy_window(app, ctx, node, god),
        None => action_window(app, ctx),
    }
}

/// What a god accepts as an offering, if anything (Juno of Veii: bread).
pub fn offering(god: u8) -> Option<rr_rules::Ware> {
    match god {
        2 => Some(rr_rules::Ware::Bread),
        // (the sacred geese of the Capitol)
        3 => Some(rr_rules::Ware::Grain),
        // (Neptune's shrine on the cape)
        6 => Some(rr_rules::Ware::Fish),
        _ => None,
    }
}

/// How many of a ware one offering gives.
const OFFER_N: u32 = 5;

/// A holy place's window: what it is and who holds it (as the hover says), its god's favour,
/// and where the god takes offerings, a button to offer them.
fn holy_window(app: &mut App, ctx: &egui::Context, node: MapCoord, god: u8) {
    let lines = crate::hover::lines(&app.sim, app.player, node, app.session.favour);
    let title = lines.first().cloned().unwrap_or_default();
    let mut open = true;
    let mut cmd = None;
    egui::Window::new(title)
        .id(egui::Id::new("holy_window"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_pos(free_spot(
            egui::Rect::from_center_size(ctx.screen_rect().center(), Vec2::splat(10.0)),
            ctx.screen_rect(),
        ))
        .show(ctx, |ui| {
            ui.set_max_width(320.0);
            for l in lines.iter().skip(1) {
                ui.label(l);
            }
            let Some(ware) = offering(god) else { return };
            ui.separator();
            ui.label(crate::i18n::show(&crate::i18n::msg("Favour: {1} of 100", &[&app.session.favour.to_string()])));
            let held = app.sim.world().holy_holder(node) == rr_world::Holder::Player(app.player);
            let have = app.sim.player_inventory(app.player).ware(ware);
            let label =
                crate::i18n::msg("Offer {1} {2}", &[&OFFER_N.to_string(), rr_game::ware_name_en(ware).as_str()]);
            let why = if !held {
                crate::i18n::tr("Your land must lie all around it first.")
            } else {
                crate::i18n::tr("From your warehouses, the castle first.")
            };
            let b = ui.add_enabled(held && have >= OFFER_N, egui::Button::new(crate::i18n::show(&label)));
            if b.on_hover_text(why).on_disabled_hover_text(why).clicked() {
                cmd = Some(Cmd::Offer { at: node, ware, n: OFFER_N });
            }
            ui.label(
                RichText::new(crate::i18n::show(&crate::i18n::msg("In store: {1}", &[&have.to_string()]))).small(),
            );
        });
    if let Some(c) = cmd {
        app.cmd(c);
    }
    if !open {
        app.ui.action = None;
    }
}

/// The buildings' windows kept open (`Ui::kept`): each where it stood, until closed or its
/// building is gone.
fn kept_windows(app: &mut App, ctx: &egui::Context) {
    let (mut cmds, mut follow, mut attack, mut closed) = (Vec::new(), None, app.ui.attack, Vec::new());
    for p in app.ui.kept.clone() {
        let Some(b) = app.sim.building_at(p) else {
            closed.push(p);
            continue;
        };
        let (mut open, mut close) = (true, false);
        win(building_name(app.sim.building(b).kind))
            .id(egui::Id::new(("action", p.x, p.y)))
            .open(&mut open)
            .default_width(300.0)
            .constrain_to(ctx.screen_rect().with_max_y(ctx.screen_rect().max.y - 48.0))
            .resizable(false)
            .show(ctx, |ui| building_info(app, ui, b, &mut cmds, &mut close, &mut follow, &mut attack));
        if !open || close {
            closed.push(p);
        }
    }
    app.ui.kept.retain(|p| !closed.contains(p));
    app.ui.attack = attack;
    if follow.is_some() {
        app.session.follow = follow;
    }
    for c in cmds {
        app.cmd(c);
    }
}

/// Middle of the wider free strip of the screen beside `window`.
fn free_spot(window: egui::Rect, screen: egui::Rect) -> egui::Pos2 {
    let left = window.left() - screen.left();
    let right = screen.right() - window.right();
    let x = if right >= left { (window.right() + screen.right()) / 2.0 } else { (screen.left() + window.left()) / 2.0 };
    egui::pos2(x, screen.center().y)
}

/// The minimap's colour for a ground colour: snow-white grounds become a mid grey, so the
/// players' colours (cyan, light yellow) show on it (art director round 4, #4).
pub(crate) fn minimap_ground(c: u32) -> u32 {
    let (r, g, b) = ((c >> 16) & 255, (c >> 8) & 255, c & 255);
    if (r * 2126 + g * 7152 + b * 722) / 10_000 > 190 { 0xa4a8a4 } else { c }
}

fn minimap_image(app: &App) -> egui::ColorImage {
    let sim = &app.sim;
    let w = sim.world();
    let g = w.grid();
    let (mw, mh) = (g.width as usize, g.height as usize);
    // twice as wide: odd rows shift by half a node, as on the map (no sheared coasts)
    let iw = mw * 2;
    let mut px = vec![Color32::BLACK; iw * mh];
    for y in 0..mh {
        for x in 0..mw {
            let p = MapCoord::new(x as u16, y as u16);
            let c = minimap_ground(rr_world::terrain::color(w.landscape, w.terrain(p, rr_world::Tri::Rsu)));
            let (mut r, mut gg, mut b) = (((c >> 16) & 255) as f32, ((c >> 8) & 255) as f32, (c & 255) as f32);
            // relief from the slope toward the light, not from the height
            let se = g.neighbor(p, rr_core::Direction::SE);
            let shade = (1.0 + (w.altitude(p) as f32 - w.altitude(se) as f32) * 0.04).clamp(0.75, 1.2);
            r *= shade;
            gg *= shade;
            b *= shade;
            if matches!(w.obj(p), NodeObj::Tree { .. }) {
                (r, gg, b) = (r * 0.4 + 47.0 * 0.6, gg * 0.4 + 90.0 * 0.6, b * 0.4 + 28.0 * 0.6);
            }
            // (owned water keeps the sea's colour: blue realms read as lakes and shallows; art
            // director round 3, #6)
            let water = rr_world::terrain::is_water(w.terrain(p, rr_world::Tri::Rsu));
            if let Some(o) = w.owner(p).filter(|_| !water && app.visibility(p) != rr_sim::Visibility::Unexplored) {
                let pc = crate::render::player_color(o);
                let built = matches!(w.obj(p), NodeObj::Building(_) | NodeObj::Flag(_)) || w.is_on_road(p);
                let border = g.neighbors(p).iter().any(|n| w.owner(*n) != Some(o));
                let k = match (built && !app.ui.mm_hide_buildings, app.ui.mm_hide_territory) {
                    (true, _) => 0.8,
                    (false, true) => 0.0,
                    (false, false) if border => 0.9,
                    (false, false) => 0.15,
                };
                r = r * (1.0 - k) + pc.r() as f32 * k;
                gg = gg * (1.0 - k) + pc.g() as f32 * k;
                b = b * (1.0 - k) + pc.b() as f32 * k;
            }
            let fog = match app.visibility(p) {
                rr_sim::Visibility::Visible => 1.0,
                rr_sim::Visibility::Fog if app.fog_of_war => 0.55,
                rr_sim::Visibility::Fog => 1.0,
                rr_sim::Visibility::Unexplored => 0.0,
            };
            let mut col =
                Color32::from_rgb((r * fog).min(255.0) as u8, (gg * fog).min(255.0) as u8, (b * fog).min(255.0) as u8);
            // a mission's map is a known continent: its coasts show faintly where nobody of
            // ours has been
            if fog == 0.0 && app.args.mission.is_some() {
                // (warm greys: unknown water no longer the navy of the sea)
                col = if water { Color32::from_rgb(40, 37, 33) } else { Color32::from_rgb(70, 64, 56) };
            }
            let x0 = 2 * x + (y & 1);
            px[y * iw + x0 % iw] = col;
            px[y * iw + (x0 + 1) % iw] = col;
        }
    }
    egui::ColorImage { size: [iw, mh], pixels: px }
}

fn minimap(app: &mut App, ui: &mut egui::Ui) {
    // layers, as in the original's map window
    let mut changed = false;
    ui.horizontal(|ui| {
        let mut territory = !app.ui.mm_hide_territory;
        let mut buildings = !app.ui.mm_hide_buildings;
        changed |= ui.checkbox(&mut territory, crate::i18n::tr("Territory")).changed();
        changed |= ui.checkbox(&mut buildings, crate::i18n::tr("Buildings and roads")).changed();
        app.ui.mm_hide_territory = !territory;
        app.ui.mm_hide_buildings = !buildings;
    });
    if changed {
        app.minimap = None;
    }
    let gf = app.sim.gf().0;
    // every 100 game frames, and at high speed at most once a second (perf round 3, #10: ten
    // rebuilds and texture uploads a second at 50×)
    let built = egui::Id::new("minimap_built");
    let now = ui.input(|i| i.time);
    let second = ui.data(|d| d.get_temp::<f64>(built)).is_none_or(|t| now - t >= 1.0 || now < t);
    let stale = app.minimap.as_ref().is_none_or(|(_, at)| gf >= at + 100 && second || gf < *at);
    if stale {
        let img = minimap_image(app);
        let tex = ui.ctx().load_texture("minimap", img, egui::TextureOptions::NEAREST);
        app.minimap = Some((tex, gf));
        ui.data_mut(|d| d.insert_temp(built, now));
    }
    let g = app.sim.world().grid();
    // zoom: 1x, 2x or 4x around the view; it starts so that the explored land fills the window
    if app.ui.mm_zoom == 0 {
        let me = app.player;
        let seen = g.coords().filter(|p| app.sim.visibility(me, *p) != rr_sim::Visibility::Unexplored).count();
        let share = seen as f32 / g.len() as f32;
        app.ui.mm_zoom = if share < 0.06 {
            4
        } else if share < 0.25 {
            2
        } else {
            1
        };
    }
    ui.horizontal(|ui| {
        for z in [1u8, 2, 4] {
            if ui.selectable_label(app.ui.mm_zoom == z, format!("{z}×")).clicked() {
                app.ui.mm_zoom = z;
            }
        }
    });
    let note = if app.args.mission.is_some() {
        // (dark, not grey: the winter's snow is a light grey on the minimap; QA round 7, #7)
        "Dark: land nobody of yours has seen yet. Dots: where your rivals rule."
    } else {
        "Black: land nobody of yours has seen yet."
    };
    ui.label(RichText::new(crate::i18n::tr(note)).small());
    let last = crate::app::holdouts(&app.sim, app.player);
    if !last.is_empty() {
        let mut who: Vec<u8> = last.iter().map(|(p, _)| *p).collect();
        who.dedup();
        let names: Vec<&str> = who.iter().map(|p| app.sim.players()[*p as usize].setup.name.as_str()).collect();
        ui.label(crate::i18n::show(&crate::i18n::msg(
            "Rings mark the posts and warehouses of your last rivals ({1}).",
            &[&names.join(", ")],
        )));
    }
    let Some((tex, _)) = &app.minimap else { return };
    let k = app.ui.mm_zoom as f32;
    let scale = (300.0 / g.width as f32).min(260.0 / g.height as f32 * 2.0) * k;
    let size = Vec2::new(g.width as f32 * scale, g.height as f32 * scale * 0.5) / k;
    let view_nodes =
        Vec2::new(app.screen.x / (crate::render::TW * app.cam.zoom), app.screen.y / (crate::render::TH * app.cam.zoom));
    let cam_node = Vec2::new(app.cam.offset.x / crate::render::TW, app.cam.offset.y / crate::render::TH);
    let (w, h) = (g.width as f32, g.height as f32);
    // the part of the map shown, centred on the view, kept inside the texture
    let half = 0.5 / k;
    let centre = Vec2::new(
        ((cam_node.x + view_nodes.x / 2.0).rem_euclid(w) / w).clamp(half, 1.0 - half),
        ((cam_node.y + view_nodes.y / 2.0).rem_euclid(h) / h).clamp(half, 1.0 - half),
    );
    let uv0 = centre - Vec2::splat(half);
    let uv = egui::Rect::from_min_size(egui::pos2(uv0.x, uv0.y), Vec2::splat(1.0 / k));
    let resp = ui.add(egui::Image::new((tex.id(), size)).uv(uv).sense(egui::Sense::click_and_drag()));
    let to_px = |v: Vec2| {
        resp.rect.min
            + Vec2::new(
                v.x.rem_euclid(w) * scale - uv0.x * w * scale,
                v.y.rem_euclid(h) * scale * 0.5 - uv0.y * h * scale * 0.5,
            )
    };
    // a mission's rivals: where their realms lie (the briefing names them)
    if app.args.mission.is_some() {
        for (i, pl) in app.sim.players().iter().enumerate() {
            if i as u8 == app.player || !pl.alive {
                continue;
            }
            let Some(at) = app.sim.hq_pos(i as u8) else { continue };
            let px = to_px(Vec2::new(at.x as f32, at.y as f32));
            if resp.rect.contains(px) {
                let painter = ui.painter_at(resp.rect);
                painter.circle_filled(px, 3.0, crate::render::player_color(i as u8));
                let name = pl.setup.name.split(" of ").last().unwrap_or(&pl.setup.name).to_string();
                // kept inside the map ("Graec" was cut at the edge)
                let galley =
                    painter.layout_no_wrap(name, egui::FontId::proportional(11.0), Color32::from_rgb(240, 225, 190));
                let size = galley.size();
                let x = (px.x - size.x / 2.0).clamp(resp.rect.left() + 2.0, resp.rect.right() - size.x - 2.0);
                let y = (px.y - 5.0 - size.y).max(resp.rect.top() + 1.0);
                painter.galley(egui::pos2(x, y), galley, Color32::from_rgb(240, 225, 190));
            }
        }
    }
    // the last rivals' posts and warehouses, through the fog (game design round 1, #5)
    let painter = ui.painter_at(resp.rect);
    // (bigger, and each rival named at its first ring: told apart by colour alone, they were
    // hard to read; a11y round 3, #9)
    let mut named: Vec<u8> = Vec::new();
    for (p, at) in crate::app::holdouts(&app.sim, app.player) {
        let px = to_px(Vec2::new(at.x as f32, at.y as f32));
        painter.circle_stroke(px, 6.0, egui::Stroke::new(3.0_f32, Color32::BLACK));
        painter.circle_stroke(px, 6.0, egui::Stroke::new(2.0_f32, crate::render::player_color(p)));
        if !named.contains(&p) && resp.rect.contains(px) {
            named.push(p);
            let name = &app.sim.players()[p as usize].setup.name;
            let short = name.split(" of ").next().unwrap_or(name);
            painter.text(
                px + Vec2::new(8.0, -8.0),
                egui::Align2::LEFT_BOTTOM,
                short,
                egui::FontId::proportional(12.0),
                Color32::from_rgb(240, 225, 190),
            );
        }
    }
    let r = egui::Rect::from_min_size(to_px(cam_node), Vec2::new(view_nodes.x * scale, view_nodes.y * scale * 0.5));
    ui.painter_at(resp.rect).rect_stroke(r, 0.0, egui::Stroke::new(1.5_f32, Color32::WHITE), egui::StrokeKind::Outside);
    if (resp.clicked() || resp.dragged())
        && let Some(p) = resp.interact_pointer_pos()
    {
        let rel = p - resp.rect.min;
        let nx = (uv0.x * w + rel.x / scale) as u16 % g.width;
        let ny = ((uv0.y * h + rel.y / (scale * 0.5)) as u16 % g.height) & !1;
        app.cam.center_on(&app.sim, MapCoord::new(nx, ny), app.screen);
    }
}

/// A fixed-size button: the picture on top, the name underneath (the build tabs).
fn picture_button(ui: &mut egui::Ui, pic: Option<(egui::TextureId, Vec2)>, label: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(92.0, 74.0), egui::Sense::click());
    let vis = ui.style().interact(&resp);
    ui.painter().rect_filled(rect, 4.0, vis.weak_bg_fill);
    if let Some((id, size)) = pic {
        let k = (52.0 / size.y).min(80.0 / size.x).min(1.0);
        let r = egui::Rect::from_center_size(rect.center_top() + Vec2::new(0.0, 30.0), size * k);
        ui.painter().image(id, r, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
    }
    ui.painter().text(
        rect.center_bottom() - Vec2::new(0.0, 4.0),
        egui::Align2::CENTER_BOTTOM,
        label,
        egui::FontId::proportional(11.0),
        vis.text_color(),
    );
    resp
}

/// A ware pictogram of a given size, on a light parchment disc (dark wares like coal stay
/// readable on the dark windows).
fn icon_sized(ui: &mut egui::Ui, w: Ware, size: f32) {
    let (r, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    parchment_disc(ui, r.center(), size * 0.5);
    crate::sprites::ware_icon(ui.painter(), r.center(), size * 0.78, w);
}

/// A ware pictogram as a UI widget.
fn icon(ui: &mut egui::Ui, w: Ware) {
    let (r, _) = ui.allocate_exact_size(Vec2::splat(22.0), egui::Sense::hover());
    parchment_disc(ui, r.center(), 11.0);
    crate::sprites::ware_icon(ui.painter(), r.center(), 18.0, w);
}

fn parchment_disc(ui: &egui::Ui, c: egui::Pos2, r: f32) {
    ui.painter().circle(
        c,
        r,
        Color32::from_rgb(216, 196, 154),
        egui::Stroke::new(1.0_f32, Color32::from_rgb(70, 52, 30)),
    );
}

#[cfg(test)]
mod tests {

    /// Owner's playtest: a message's place opens the building there (an exhausted mine, an
    /// attacked post), as a click on it would; an empty spot only moves the map.
    #[test]
    fn a_message_opens_its_building() {
        let args = crate::Args { players: 2, size: 64, seed: 3, ..Default::default() };
        let mut app = App::headless(args);
        let hq = app.sim.building(app.sim.players()[0].hq.expect("a castle")).pos;
        assert!(open_building_at(&mut app, hq));
        assert_eq!(app.session.selected, Some(hq));
        assert_eq!(app.ui.action.as_ref().map(|a| a.node), Some(hq));
        let empty = app.sim.world().grid().neighbor(hq, rr_core::Direction::E);
        let empty = app.sim.world().grid().neighbor(empty, rr_core::Direction::E);
        app.session.selected = None;
        assert!(!open_building_at(&mut app, empty), "no building there");
        assert_eq!(app.session.selected, None);
    }
    /// Owner's playtest 2026-10-27: as in the original, several buildings' windows stay open.
    /// Opening another one keeps the last; a spot's build window doesn't stay; at most six.
    #[test]
    fn building_windows_stay_open() {
        let args = crate::Args { players: 2, size: 64, seed: 3, ..Default::default() };
        let mut app = App::headless(args);
        let hq = app.sim.building(app.sim.players()[0].hq.expect("a castle")).pos;
        let rival = app.sim.building(app.sim.players()[1].hq.expect("a castle")).pos;
        let g = app.sim.world().grid();
        let empty = g.neighbor(g.neighbor(hq, rr_core::Direction::E), rr_core::Direction::E);
        set_action(&mut app, Action { node: hq, tab: 0 });
        set_action(&mut app, Action { node: rival, tab: 0 });
        assert_eq!(app.ui.kept, vec![hq], "the castle's window stays");
        set_action(&mut app, Action { node: empty, tab: 0 });
        assert_eq!(app.ui.kept, vec![hq, rival]);
        set_action(&mut app, Action { node: hq, tab: 0 });
        assert_eq!(app.ui.kept, vec![rival], "the empty spot's window went; the castle's is the action now");
        // never more than six
        for x in 0..10 {
            let p = MapCoord::new(x, 1);
            app.ui.kept.retain(|q| *q != p);
            app.ui.kept.push(p);
        }
        app.ui.kept.truncate(KEPT_WINDOWS - 1);
        set_action(&mut app, Action { node: rival, tab: 0 });
        set_action(&mut app, Action { node: empty, tab: 0 });
        assert!(app.ui.kept.len() <= KEPT_WINDOWS);
    }

    /// QA round 7, #3: ticks belong to one game.
    #[test]
    fn ticks_belong_to_one_game() {
        assert_ne!(steps_key("First steps", 1), steps_key("First steps", 2));
        assert_eq!(steps_key("First steps", 1), steps_key("First steps", 1));
    }

    /// Art director round 4, #7: every notice in one frame, told apart by a stripe of its own.
    #[test]
    fn notices_share_a_frame_and_differ_by_stripe() {
        let kinds = [Notice::Military, Notice::Note, Notice::Pause];
        for (i, a) in kinds.iter().enumerate() {
            for b in &kinds[i + 1..] {
                assert_ne!(a.stripe(), b.stripe());
            }
        }
        const { assert!(PAUSE_DIM > 0) };
    }

    /// Art director round 4, #6: headings (strong text) stand out from the text, and read.
    #[test]
    fn headings_stand_out() {
        let ctx = egui::Context::default();
        setup_style(&ctx);
        let v = ctx.style().visuals.clone();
        let (strong, text) = (v.strong_text_color(), v.text_color());
        // (white beside the cream text looked the same; headings are the title's gold)
        assert!(text.b() as i32 - strong.b() as i32 >= 50, "{strong:?} vs {text:?}");
        assert!(contrast(strong, v.window_fill) >= 4.5);
    }

    /// New-player round 10, #9: a pause is seen; Go there marks the building for a while.
    #[test]
    fn a_pause_and_a_mark_are_seen() {
        assert!(pause_banner(true, false, false));
        assert!(!pause_banner(true, true, false), "not in the menu");
        assert!(!pause_banner(true, false, true), "not over the briefing");
        assert!(!pause_banner(false, false, false));
        assert!(mark_alpha(0.1) > 0.5 && mark_alpha(4.0) == 0.0);
    }

    /// New-player round 10, #7: every next step has a hint, and a tick stays once earned (a
    /// burned well unticked "A farm and a well" without a word).
    #[test]
    fn next_steps_have_hints_and_ticks_stay() {
        for what in [
            "A farm and a well",
            "A mill and a bakery",
            "A coal mine and an iron mine",
            "A brewery",
            "An iron smelter and an armory",
            "Take a rival post: click it, then ⚔ Attack",
        ] {
            let hint = first_step_hint(what).unwrap_or_else(|| panic!("no hint for {what}"));
            assert!(crate::i18n::has_german(hint), "{hint}");
        }
        let (steps, mask) = sticky_steps(vec![("a", true), ("b", false)], 0);
        assert_eq!((steps[0].1, mask), (true, 1));
        let (steps, _) = sticky_steps(vec![("a", false), ("b", false)], mask);
        assert!(steps[0].1, "a tick once earned stays");
    }

    /// New-player round 10, #6: a key opens the help, and the help says which.
    #[test]
    fn the_help_has_a_key() {
        assert!(HELP_KEYS.iter().any(|(_, keys)| keys.iter().any(|(k, w)| *k == "F1" && *w == "Help")));
    }

    /// a11y round 2, #10: faint text (zero counts, message times, done steps) reads at 4.5:1 on
    /// the windows (it was about 3:1).
    #[test]
    fn faint_text_is_readable() {
        let ctx = egui::Context::default();
        setup_style(&ctx);
        let v = ctx.style().visuals.clone();
        let c = contrast(v.weak_text_color(), v.window_fill);
        assert!(c >= 4.5, "{c:.2}");
    }

    /// a11y round 2, #7: the current step's hint shows without hovering; every help key has a
    /// German text.
    #[test]
    fn hints_and_keys_are_readable() {
        assert_eq!(step_with_hint(&[("a", true), ("b", false), ("c", false)]), Some(1));
        assert_eq!(step_with_hint(&[("a", true)]), None);
        for (group, keys) in HELP_KEYS {
            assert!(crate::i18n::has_german(group), "{group}");
            for (key, what) in *keys {
                assert!(crate::i18n::has_german(what), "{what}");
                // German round 1, #6: a key column with words in it ("Space", "Wheel") too
                if key.chars().filter(|c| c.is_alphabetic()).count() > 2 {
                    assert!(crate::i18n::has_german(key), "{key}");
                }
            }
        }
        for a in crate::keys::ACTS {
            let name = a.3.name();
            if name.chars().count() > 3 {
                assert!(crate::i18n::has_german(name), "{name}");
            }
        }
    }

    /// German round 1, #3/#6/#7: the garrison line is one sentence per count, its areas are
    /// translated, and the ranks are the classic German ones.
    #[test]
    fn garrison_line_and_ranks_in_german() {
        for area in GARRISON_AREAS {
            assert!(crate::i18n::has_german(area), "{area}");
        }
        assert!(garrison_cap_line(1, "no enemy near").contains("1 soldier here (no enemy near)"));
        assert!(garrison_cap_line(2, "no enemy near").contains("2 soldiers here (no enemy near)"));
        let one = crate::i18n::german(
            "The military settings send 1 soldier here ({1}). Change it in Main selection, Military.",
        );
        assert!(one.unwrap().starts_with("Laut Militäreinstellungen kommt 1 Soldat"));
        let ranks: Vec<_> =
            rr_rules::Job::SOLDIERS.iter().map(|j| crate::i18n::german(&job_name_en(*j)).unwrap()).collect();
        assert_eq!(ranks, ["Gefreiter", "Obergefreiter", "Unteroffizier", "Offizier", "General"]);
    }

    /// a11y round 2, #5: an alert's clock stops in a pause and runs 12 s of play.
    #[test]
    fn alerts_wait_for_the_player() {
        let mut a = Alert::new("We are under attack!".into(), MapCoord::new(1, 1));
        for _ in 0..100 {
            assert!(a.tick(1.0, true), "paused: still shown");
        }
        assert!(a.tick(6.0, false));
        assert!(!a.tick(6.5, false));
    }

    /// a11y round 2, #3: at 2x on a 1366-wide screen (683 points) the pause button lay on the
    /// messages button.
    #[test]
    fn a_narrow_bar_gets_two_rows() {
        assert!(two_bar_rows(1366.0 / 2.0));
        assert!(!two_bar_rows(1280.0));
    }

    /// a11y round 2, #1: the chart's lines are told apart by a shape, not by colour alone
    /// (four of six rivals turned the same grey for deutan players).
    #[test]
    fn every_player_has_its_own_marker() {
        let shapes: Vec<Marker> = (0..8).map(marker).collect();
        for a in 0..8 {
            for b in a + 1..8 {
                assert_ne!(shapes[a], shapes[b], "players {a} and {b}");
            }
        }
        // a few marks along the line and one at its end
        let at = marks_along(120, 6, 0);
        assert!(at.len() >= 4 && at.last() == Some(&119), "{at:?}");
        assert_ne!(at[0], marks_along(120, 6, 3)[0], "staggered");
        assert!(marks_along(1, 6, 0).len() <= 1);
    }

    /// New-player round 12, #1: the end table's land is the verdict's (live), not the last
    /// sample's; the other columns stay the statistics'.
    #[test]
    fn the_end_table_counts_land_as_the_verdict() {
        let mut m = rr_core::MapData::flat(48, 48, 8);
        m.hq = vec![Some(MapCoord::new(12, 12))];
        let setup = rr_sim::GameSetup { players: vec![Default::default()], ..Default::default() };
        let sim = rr_sim::Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &m, &setup).unwrap();
        let p = &sim.players()[0];
        assert!(p.stats.samples.is_empty(), "no sample yet: it read 0");
        let land = sim.land_by_player()[0];
        assert!(land > 0);
        assert_eq!(end_value(p, 0, land), land);
        assert_eq!(rr_sim::Sample::CATEGORIES[0], "Land");
    }

    /// PLAN §27, #5: "where are my boards going?": the wares on the way by where they go, most
    /// first; building sites together, the ones without a goal last.
    #[test]
    fn wares_on_the_way_by_goal() {
        use rr_rules::BuildingType as B;
        let goals = [Some(Way::Site), Some(Way::To(B::Sawmill)), Some(Way::Site), None, Some(Way::Site)];
        let ways = group_ways(goals.into_iter());
        assert_eq!(ways, vec![(Some(Way::Site), 3), (Some(Way::To(B::Sawmill)), 1), (None, 1)]);
        assert!(group_ways(std::iter::empty()).is_empty());
    }

    #[test]
    fn worked_out_once_per_game_frame() {
        assert!(still_fresh(Some((5, 1.0)), 5, 9.0), "the same game frame");
        assert!(still_fresh(Some((4, 1.0)), 5, 1.1), "a new frame within a fifth of a second");
        assert!(!still_fresh(Some((4, 1.0)), 5, 1.3));
        assert!(!still_fresh(None, 5, 1.0));
    }

    use super::*;

    /// Art director round 3, #7: every player's chart line stands out 3:1 on the chart.
    #[test]
    fn chart_lines_have_contrast() {
        for c in crate::render::PLAYER_COLORS {
            assert!(contrast(chart_color(c), CHART_BG) >= 3.0, "{c:?}");
        }
    }

    /// Architect round 3, #7: what the UI hands the app is typed state, not egui memory under a
    /// name (a type mismatch there reads as a silent `None`; item 499 sent one soldier per attack).
    #[test]
    fn no_game_state_in_egui_memory() {
        let sources = [include_str!("ui.rs"), include_str!("app.rs")];
        for key in ["follow", "attack_count", "attack_weak", "break_truce"] {
            let named = format!("Id::new({:?}", key);
            let tuple = format!("Id::new(({:?}", key);
            assert!(sources.iter().all(|s| !s.contains(&named) && !s.contains(&tuple)), "{key}");
        }
        // a new game (a fresh `Ui`) asks no truce question and sends all soldiers
        let panel = Ui::default().attack;
        assert_eq!((panel.asking, attack_choice(panel.count, 7)), (None, 7));
    }

    /// New-player round 8, #4: in a campaign the allied rivals are marked and summed.
    #[test]
    fn allied_rivals_count_together() {
        let mut m = rr_core::MapData::flat(64, 64, 8);
        m.hq = vec![
            Some(rr_core::MapCoord::new(10, 10)),
            Some(rr_core::MapCoord::new(40, 10)),
            Some(rr_core::MapCoord::new(25, 40)),
        ];
        let setup = rr_sim::GameSetup::campaign(1, 3);
        let mut sim = rr_sim::Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &m, &setup).unwrap();
        for _ in 0..700 {
            sim.step(&[]);
        }
        assert_eq!(allied_rivals(&sim, 0), vec![1, 2]);
        let sum = together(sim.players(), &[1, 2]);
        let land = rr_sim::Sample::CATEGORIES.iter().position(|c| *c == "Land").unwrap();
        let each: u32 = (1..3).map(|p| sim.players()[p].stats.samples.last().unwrap().value(land)).sum();
        assert_eq!(sum[land], each);
        let (home, out) = soldiers_home_and_out(&sim, 0);
        assert!(home > 0 && out == 0, "{home} {out}");
        // a free game with every player alone: nobody is allied
        let free = rr_sim::GameSetup { seed: 1, players: vec![Default::default(); 3], ..Default::default() };
        let sim = rr_sim::Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &m, &free).unwrap();
        assert!(allied_rivals(&sim, 0).is_empty());
    }

    /// Item 304: every ware has one group in the statistics.
    #[test]
    fn every_ware_has_one_statistics_group() {
        for w in Ware::ALL {
            let n = WARE_GROUPS.iter().filter(|(_, ws)| ws.contains(w)).count();
            assert_eq!(n, 1, "{w:?}");
        }
    }

    /// Item 304: a long game's half minutes are summed into at most 40 bars.
    #[test]
    fn production_is_binned() {
        let (b, k) = bins(&[1; 100], 40);
        assert_eq!((b.len(), k), (34, 3));
        assert_eq!(b.iter().sum::<u32>(), 100);
        // the partial bar is the oldest, the newest is whole
        assert_eq!((b[0], b[33]), (1, 3));
        assert_eq!(bins(&[2, 3], 40), (vec![2, 3], 1));
        assert_eq!(bins(&[], 40), (vec![], 1));
    }

    /// New-player round 7, #5: the Mines tab tells what geologists found by the spot.
    #[test]
    fn the_mines_tab_reads_the_signs() {
        let mut m = rr_core::MapData::flat(64, 64, 8);
        m.hq = vec![Some(rr_core::MapCoord::new(20, 20))];
        let setup = rr_sim::GameSetup { seed: 1, players: vec![Default::default()], ..Default::default() };
        let mut sim = rr_sim::Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &m, &setup).unwrap();
        let at = rr_core::MapCoord::new(30, 30);
        assert!(found_near(&sim, at).is_empty());
        let g = sim.world().grid();
        sim.debug_set_obj(g.neighbor(at, rr_core::Direction::E), rr_world::NodeObj::Sign(0x13));
        sim.debug_set_obj(g.neighbor(at, rr_core::Direction::W), rr_world::NodeObj::Sign(0x25));
        sim.debug_set_obj(g.neighbor(at, rr_core::Direction::NW), rr_world::NodeObj::Sign(0x00));
        let found = found_near(&sim, at);
        assert_eq!(found, vec![(B::IronMine, 5), (B::CoalMine, 3)], "most first, the empty sign ignored");
    }

    /// New-player round 6, #8: the attack window says why nobody can attack.
    #[test]
    fn why_nobody_can_attack() {
        assert!(no_attack_reason(0).contains("castle"));
        assert!(no_attack_reason(2).contains("last soldier"));
    }

    /// New-player round 6, #7: the build window opens on the tab its marker shows.
    #[test]
    fn the_build_window_opens_on_the_largest_tab() {
        assert_eq!(buildable_lists(Bq::House)[first_tab(Bq::House)].0, "Medium");
        assert_eq!(buildable_lists(Bq::Castle)[first_tab(Bq::Castle)].0, "Large");
        assert_eq!(buildable_lists(Bq::Hut)[first_tab(Bq::Hut)].0, "Small");
        assert_eq!(first_tab(Bq::Flag), 0);
        // new-player round 12, #8: the First steps' next building's tab, where the spot allows it
        let tab = |bq, b| buildable_lists(bq)[tab_for(bq, b)].0;
        assert_eq!(tab(Bq::Castle, Some(B::Woodcutter)), "Small");
        assert_eq!(tab(Bq::Castle, Some(B::Sawmill)), "Medium");
        assert_eq!(tab(Bq::Hut, Some(B::Sawmill)), "Small", "not on this spot: as before");
        assert_eq!(tab(Bq::Castle, None), "Large");
    }

    /// a11y roast #2: no text below 11 px.
    #[test]
    fn small_print_is_readable() {
        let ctx = egui::Context::default();
        setup_style(&ctx);
        let size = ctx.style().text_styles[&egui::TextStyle::Small].size;
        assert!(size >= 11.0, "{size}");
    }

    /// New-player roast #10: the overview names what holds buildings up.
    #[test]
    fn the_overview_says_why_a_building_idles() {
        use rr_core::Direction;
        let mut map = rr_core::MapData::flat(64, 64, 8);
        map.hq = vec![Some(MapCoord::new(20, 20))];
        let setup = rr_sim::GameSetup { seed: 1, players: vec![Default::default()], ..Default::default() };
        let mut sim = rr_sim::Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &map, &setup).unwrap();
        let g = sim.world().grid();
        let at = MapCoord::new(24, 20);
        let cmd = |cmd| rr_sim::PlayerCommand { player: 0, cmd };
        sim.step(&[cmd(Cmd::PlaceBuilding { at, kind: B::Woodcutter })]);
        let b = sim.building_at(at).unwrap();
        assert_eq!(building_problem(&sim, b), Some("No road to this building site."));
        let from = sim.flag(sim.building(b).flag).pos;
        let dirs = sim.road_path(0, from, g.neighbor(MapCoord::new(20, 20), Direction::SE), &[]).unwrap();
        sim.step(&[cmd(Cmd::BuildRoad { from, dirs })]);
        assert_eq!(building_problem(&sim, b), None, "a site is no problem");
        for _ in 0..40_000 {
            sim.step(&[]);
        }
        assert_eq!(building_problem(&sim, b), Some(idle_reason(B::Woodcutter)));
        sim.step(&[cmd(Cmd::ToggleProduction { at })]);
        assert_eq!(building_problem(&sim, b), Some("Stopped."));
    }

    /// New-player round 11, #1: the attack sends as many soldiers as the slider says (it was
    /// read from a count nobody wrote, so every attack sent one); first time, all that may go.
    #[test]
    fn the_attack_sends_what_the_slider_says() {
        assert_eq!(attack_choice(None, 21), 21);
        assert_eq!(attack_choice(Some(5), 21), 5);
        assert_eq!(attack_choice(Some(30), 21), 21, "no more than may go");
        assert_eq!(attack_choice(Some(0), 3), 1);
        assert_eq!(attack_choice(None, 400), 255);
    }

    /// A11y round 3, #4: a note stays long enough to read (the 31-word truce advice had 3 s).
    #[test]
    fn notes_last_long_enough_to_read() {
        assert_eq!(toast_secs("Saved.", false), 3.0);
        assert_eq!(toast_secs("Saved.", true), 6.0);
        let long = vec!["word"; 31].join(" ");
        assert!(toast_secs(&long, false) >= 9.0, "{}", toast_secs(&long, false));
    }

    /// Genre lessons (PLAN §27, #2): rough odds before an attack, from the hit points of who
    /// would go against who holds the building; and the ranks said as counts.
    #[test]
    fn the_odds_are_rough_but_fair() {
        assert_eq!(odds(21, 12), "Rough odds: in your favour.");
        assert_eq!(odds(12, 12), "Rough odds: even.");
        assert_eq!(odds(9, 14), "Rough odds: against you.");
        assert_eq!(odds(5, 0), "Rough odds: in your favour.", "nobody at home");
        let names = ranks_line(&[4, 0, 4, 2]);
        assert_eq!(
            names,
            format!(
                "2× {}, 1× {}, 1× {}",
                job_name(rr_rules::Job::General),
                job_name(rr_rules::Job::Sergeant),
                job_name(rr_rules::Job::Private)
            )
        );
    }

    /// Genre lessons (PLAN §27, #4): the window says what holds a building up, by name: the
    /// ware it waits for, a full flag, a missing road.
    #[test]
    fn the_window_names_the_cause() {
        use rr_core::Direction;
        let mut map = rr_core::MapData::flat(64, 64, 8);
        map.hq = vec![Some(MapCoord::new(20, 20))];
        let setup = rr_sim::GameSetup { seed: 1, players: vec![Default::default()], ..Default::default() };
        let mut sim = rr_sim::Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &map, &setup).unwrap();
        let g = sim.world().grid();
        let at = MapCoord::new(24, 20);
        let cmd = |cmd| rr_sim::PlayerCommand { player: 0, cmd };
        sim.step(&[cmd(Cmd::PlaceBuilding { at, kind: B::Sawmill })]);
        let b = sim.building_at(at).unwrap();
        let from = sim.flag(sim.building(b).flag).pos;
        let dirs = sim.road_path(0, from, g.neighbor(MapCoord::new(20, 20), Direction::SE), &[]).unwrap();
        sim.step(&[cmd(Cmd::BuildRoad { from, dirs: dirs.clone() })]);
        while !sim.building(b).is_ready() || sim.building(b).worker.is_none() {
            sim.step(&[]);
            assert!(sim.gf().0 < 30_000, "the sawmill wasn't built");
        }
        // no wood anywhere: it names the wood
        let wood = sim.player_inventory(0).ware(rr_rules::Ware::Wood);
        sim.debug_take_ware(0, rr_rules::Ware::Wood, wood);
        for _ in 0..3000 {
            sim.step(&[]);
        }
        let wood_name = ware_name(rr_rules::Ware::Wood);
        assert_eq!(building_cause(&sim, b), Some(format!("Waiting for: {wood_name}.")));
        // a full flag comes first
        sim.debug_drop_ware(0, from, rr_rules::Ware::Boards, 8);
        assert!(
            building_cause(&sim, b).is_some_and(|c| c.starts_with("Its flag is full")),
            "{:?}",
            building_cause(&sim, b)
        );
        // no road at all
        sim.step(&[cmd(Cmd::RemoveRoad { flag: from, dir: dirs[0] })]);
        assert!(
            building_cause(&sim, b).is_some_and(|c| c.starts_with("Not connected")),
            "{:?}",
            building_cause(&sim, b)
        );
    }

    /// New-player round 2, A: the window stays on the side chosen at the click, wherever the
    /// pointer goes afterwards.
    #[test]
    fn the_window_side_is_fixed_at_the_click() {
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1280.0, 800.0));
        let id = egui::Id::new("w");
        let first = anchored(&ctx, id, egui::pos2(500.0, 400.0), screen);
        assert_eq!(first, beside(egui::pos2(500.0, 400.0), screen));
        for x in [100.0, 380.0, 700.0, 1200.0] {
            assert_eq!(anchored(&ctx, id, egui::pos2(x, 300.0), screen), first);
        }
        // another window gets its own place
        assert_ne!(anchored(&ctx, egui::Id::new("v"), egui::pos2(1200.0, 300.0), screen), first);
    }

    /// New-player round 2, D: a message's spot is shown beside the window, not under it.
    #[test]
    fn a_jump_lands_beside_the_window() {
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1280.0, 800.0));
        for window in [
            egui::Rect::from_min_size(egui::pos2(100.0, 100.0), Vec2::new(500.0, 400.0)),
            egui::Rect::from_min_size(egui::pos2(500.0, 100.0), Vec2::new(500.0, 400.0)),
            egui::Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(1000.0, 300.0)),
        ] {
            let p = free_spot(window, screen);
            assert!(!window.contains(p) && screen.contains(p), "{window:?} -> {p:?}");
        }
    }

    /// New-player roast #11: the speed reads as a multiple of normal, not milliseconds.
    #[test]
    fn speed_reads_as_a_multiple() {
        let labels: Vec<String> = SPEEDS.iter().map(|ms| speed_label(*ms)).collect();
        assert_eq!(labels, ["0.5×", "0.75×", "1×", "1.5×", "2×", "3×", "10×", "50×"]);
    }

    /// New-player roast #4: a window opens beside the clicked spot, on the side with room.
    #[test]
    fn windows_open_beside_the_click() {
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1280.0, 800.0));
        for click in
            [egui::pos2(200.0, 400.0), egui::pos2(1100.0, 400.0), egui::pos2(640.0, 60.0), egui::pos2(900.0, 700.0)]
        {
            let (pos, pivot) = beside(click, screen);
            // a window of the usual size, placed by egui at pos with this pivot
            let size = Vec2::new(280.0, 330.0);
            let min = pivot.align_size_within_rect(size, egui::Rect::from_center_size(pos, Vec2::ZERO)).min;
            let mut r = egui::Rect::from_min_size(min, size);
            // egui keeps windows on the screen
            r = r.translate(Vec2::new(
                (screen.left() - r.left()).max(0.0) + (screen.right() - r.right()).min(0.0),
                (screen.top() - r.top()).max(0.0) + (screen.bottom() - r.bottom()).min(0.0),
            ));
            let spot = egui::Rect::from_center_size(click + Vec2::new(0.0, -20.0), Vec2::new(70.0, 80.0));
            assert!(!r.intersects(spot), "{click:?}: window {r:?} covers the spot");
            // and away from the middle (the castle) when it fits
            if (click.x - 640.0).abs() > 50.0 && click.x > 375.0 && click.x < 905.0 {
                assert!(!r.contains(screen.center()), "{click:?}: window {r:?} covers the middle");
            }
        }
    }
}
