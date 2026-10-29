//! Title screen: painted background, the Europe mission, free games, loading.

use crate::app::App;
use eframe::egui::{self, Align2, Color32, RichText, Vec2};

pub struct MenuState {
    pub players: usize,
    pub size: u16,
    pub seed: u64,
    pub ai_level: u8,
    pub goods: rr_sim::StartGoods,
    pub allied: bool,
    /// The credits window.
    pub credits: bool,
    /// Why the chosen map file can't be played.
    pub map_msg: String,
    /// The slot you play on a map file, and the chosen file's player count (path, players).
    pub me: u8,
    map_players: Option<(String, usize)>,
    /// LAN games: my name, the host's address, the last network message.
    pub net_name: String,
    pub net_addr: String,
    pub net_msg: String,
    /// A note at the top of the menu (why a load was refused).
    pub note: String,
    /// Free game: (nation, team) per player.
    pub table: [(u8, u8); 6],
    /// A level per player slot (255: the Opponents setting).
    pub levels: [u8; 6],
    pub landscape: rr_core::Landscape,
    /// An archipelago: every realm on its own island.
    pub islands: bool,
    /// Win with this share of the land (0, 60 or 75 %).
    pub land_goal: u32,
    /// Or win with this many gold coins in store first (0: no such goal).
    pub coins_goal: u32,
    /// Peace time and time limit, in minutes (game design round 1, #6).
    pub peace: u32,
    pub time_limit: u32,
    pub map_path: String,
    /// Map files found (path, "name · size · players"); None: look again.
    pub maps: Option<Vec<(std::path::PathBuf, String)>>,
    bg: Option<egui::TextureHandle>,
    /// A pick that would end the running network game, waiting for a yes (net round 4, #2).
    leave_pending: Option<Picks>,
    /// The quick load waits for a yes: the game behind the menu has unsaved play (QA round 7,
    /// #1).
    load_pending: bool,
}

/// Map files (.wld/.swd) in the usual places: $RR_MAPS, the user's maps folder, a reference checkout.
/// Map files: the editor's folder (which follows `XDG_DATA_HOME`), and `$RR_MAPS` (for example
/// the maps of your own copy of the original, or RTTR's).
pub fn find_maps() -> Vec<(std::path::PathBuf, String)> {
    let mut dirs: Vec<std::path::PathBuf> = vec![crate::editor::maps_dir()];
    if let Ok(d) = std::env::var("RR_MAPS") {
        dirs.push(d.into());
    }
    // the developer's RTTR checkout, in debug builds only
    #[cfg(debug_assertions)]
    dirs.push(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../reference/s25client/data/RTTR/MAPS"));
    let mut out = Vec::new();
    fn walk(d: &std::path::Path, out: &mut Vec<std::path::PathBuf>, depth: u32) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() && depth < 3 {
                walk(&p, out, depth + 1);
            } else if p.extension().is_some_and(|x| ["wld", "swd", "rrmap"].iter().any(|e| x.eq_ignore_ascii_case(e))) {
                out.push(p);
            }
        }
    }
    for d in dirs {
        walk(&d, &mut out, 0);
    }
    out.sort();
    out.dedup();
    out.into_iter()
        .map(|p| {
            let file = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let label = match std::fs::read(&p).ok().and_then(|b| rr_formats::rrmap::read_any(&b).ok()) {
                Some(m) => map_label(&file, &m),
                None => file,
            };
            (p, label)
        })
        .collect()
}

/// A map file in the list: file, size, players.
fn map_label(file: &str, m: &rr_core::MapData) -> String {
    format!("{file} · {}×{} · {} {}", m.grid.width, m.grid.height, m.players(), crate::i18n::tr("players"))
}

impl Default for MenuState {
    fn default() -> Self {
        Self {
            players: 3,
            size: 112,
            seed: 1,
            ai_level: 1,
            goods: rr_sim::StartGoods::Normal,
            allied: false,
            credits: false,
            map_msg: String::new(),
            me: 0,
            map_players: None,
            net_name: std::env::var("USER").unwrap_or_else(|_| "Player".into()),
            net_addr: String::new(),
            net_msg: String::new(),
            note: String::new(),
            table: [(0, 0), (1, 0), (2, 0), (3, 0), (0, 0), (1, 0)],
            levels: [255; 6],
            landscape: rr_core::Landscape::Greenland,
            islands: false,
            land_goal: 0,
            coins_goal: 0,
            peace: 0,
            time_limit: 0,
            map_path: String::new(),
            maps: None,
            bg: None,
            leave_pending: None,
            load_pending: false,
        }
    }
}

/// The menu's backdrop: a scene of the game itself (art director round 4, #9: a painted
/// picture promised another game), made with tools/uitest.sh from the Europe mission at
/// minute 30.
const MENU_BACKDROP: &str = "menu_village.jpg";

fn load_bg(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let path = rr_audio::assets_dir().join("art").join(MENU_BACKDROP);
    let img = image::open(path).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let ci = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    Some(ctx.load_texture("menu_bg", ci, egui::TextureOptions::LINEAR))
}

pub fn show(app: &mut App, ctx: &egui::Context) {
    if app.menu.bg.is_none() {
        app.menu.bg = load_bg(ctx);
    }
    egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| {
        let rect = ui.max_rect();
        if let Some(bg) = &app.menu.bg {
            // cover the screen, keep the aspect
            let s = bg.size_vec2();
            let k = (rect.width() / s.x).max(rect.height() / s.y);
            let r = egui::Rect::from_center_size(rect.center(), s * k);
            ui.painter().image(
                bg.id(),
                r,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        } else {
            ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(40, 60, 30));
        }
    });
    let mut start: Option<crate::Args> = None;
    let mut load = false;
    let mut open_editor = false;
    let mut load_path: Option<std::path::PathBuf> = None;
    let mut replay = false;
    // two columns when the screen is wide enough, and less room above on low screens
    let wide = ctx.screen_rect().width() >= 820.0;
    let top = if ctx.screen_rect().height() >= 800.0 { 90.0 } else { 30.0 };
    let room = ctx.screen_rect().height() - top - 20.0;
    let short = !wide || ctx.screen_rect().height() < 760.0;
    egui::Window::new(crate::i18n::tr("title"))
        .title_bar(false)
        .resizable(false)
        .anchor(Align2::CENTER_TOP, Vec2::new(0.0, top))
        // long menus scroll instead of running off the screen, on low or narrow screens; there
        // the menu takes the whole height (a scrolling window grew to about 430 points only and
        // cut off the free game's start button)
        .vscroll(short)
        .min_height(if short { room } else { 0.0 })
        .max_height(room)
        .default_height(if short { room } else { 0.0 })
        .frame(egui::Frame::window(&ctx.style()).fill(Color32::from_rgba_unmultiplied(50, 36, 22, 235)))
        .show(ctx, |ui| {
            ui.set_width(if wide { 660.0 } else { 340.0 });
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new(crate::i18n::tr("Roads & Realms")).size(34.0).color(Color32::from_rgb(250, 220, 140)),
                );
                ui.label(RichText::new(crate::i18n::tr("build · carry · conquer")).italics());
                ui.add_space(12.0);
                let mut p = Picks::default();
                if wide {
                    // two columns: play on the left, continue / load / more on the right
                    ui.horizontal_top(|ui| {
                        ui.allocate_ui_with_layout(
                            Vec2::new(340.0, 0.0),
                            egui::Layout::top_down(egui::Align::Center),
                            |ui| play_column(app, ui, &mut p),
                        );
                        ui.separator();
                        ui.allocate_ui_with_layout(
                            Vec2::new(300.0, 0.0),
                            egui::Layout::top_down(egui::Align::Center),
                            |ui| more_column(app, ui, ctx, &mut p),
                        );
                    });
                } else {
                    play_column(app, ui, &mut p);
                    ui.separator();
                    more_column(app, ui, ctx, &mut p);
                }
                // a network game ends with these: ask first (one click ended it for everyone)
                if app.net.is_some() && p.leaves_the_game() {
                    app.menu.leave_pending = Some(p);
                } else {
                    (start, load, open_editor, load_path, replay) =
                        (p.start, p.load, p.open_editor, p.load_path, p.replay);
                }
            });
        });
    if let Some(p) = ask_leave(app, ctx) {
        (start, load, open_editor, load_path, replay) = (p.start, p.load, p.open_editor, p.load_path, p.replay);
    }
    crate::ui::options_window(app, ctx);
    let mut open = app.menu.credits;
    egui::Window::new(crate::i18n::tr("Credits"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            ui.label(RichText::new("Roads & Realms").size(20.0));
            ui.label(crate::i18n::tr(
                "A non-commercial remake in the spirit of the 1996 classic. No original game data is included.",
            ));
            ui.separator();
            for line in [
                "Some rules and timings come from Return to the Roots (GPL), with thanks to its authors.",
                "Music: RandomMind and Woli34 (OpenGameArt, CC0). Ambience: Spring Spring, RandomMind (CC0).",
                "Sounds: Kenney (kenney.nl, CC0).",
                "Coastlines: Natural Earth (public domain).",
                "Title painting, the Roman buildings, the holy places and the ships: painted with an AI image tool from our own prompts; all other art: our own code.",
                "Code: GPL-3.0-or-later. Art pack: CC-BY-SA-4.0.",
            ] {
                ui.label(crate::i18n::tr(line));
            }
        });
    app.menu.credits = open;
    crate::netui::lobby_window(app, ctx);
    if open_editor {
        // the game in the background ends here: keep its replay, leave a network game
        app.leave_net();
        app.save_replay();
        // edit the chosen map file, or start from a blank meadow
        let path = std::path::PathBuf::from(&app.menu.map_path);
        let map = std::fs::read(&path).ok().and_then(|b| rr_formats::rrmap::read_any(&b).ok());
        let from_file = map.is_some() && path.extension().is_some_and(|e| e == "rrmap");
        let mut ed = crate::editor::Editor::new(
            map.unwrap_or_else(|| crate::editor::Editor::blank(96, rr_core::Landscape::Greenland)),
        );
        // saving goes back to the file it came from without asking
        if from_file {
            ed.open_mission(&path);
            ed.file = Some(path);
        }
        app.editor = Some(ed);
        app.in_menu = false;
        app.tex = None;
    }
    if let Some(a) = start {
        app.start_game(a);
    }
    // F9 loads the quick save here too
    // the player's quick-load key, not while a key is being chosen (QA round 8, #8)
    let quick_load = app.keys.key(crate::keys::Act::QuickLoad);
    load |= app.rebinding.is_none() && ctx.input(|i| i.key_pressed(quick_load)) && App::save_path().exists();
    load = ask_load(app, ctx, load);
    if load {
        // a loaded game closes the menu; a failed one leaves it open with the reason
        app.quick_load();
    }
    if let Some(p) = load_path {
        app.load_from(&p);
    }
    // (a replay that can't play says so: the button did nothing, owner 2026-10-27)
    if replay && !app.watch_replay() {
        app.menu.note = crate::i18n::tr(REPLAY_UNPLAYABLE).into();
    }
}

/// The menu's word when the last game's replay can't be played.
const REPLAY_UNPLAYABLE: &str =
    "The last game's replay can't be played: it was recorded with another version of the game, or the file is damaged.";

/// The question before a pick ends the running network game: the pick once answered yes.
fn ask_leave(app: &mut App, ctx: &egui::Context) -> Option<Picks> {
    let p = app.menu.leave_pending.take()?;
    let host = app.net.as_ref().and_then(|n| n.with(|l| Ok(l.is_host())).ok()).unwrap_or(false);
    let (mut yes, mut no) = (false, false);
    egui::Modal::new(egui::Id::new("leave_net")).show(ctx, |ui| {
        ui.label(crate::i18n::tr(leaving_question(host)));
        ui.horizontal(|ui| {
            yes = ui.button(crate::i18n::tr("Leave the game")).clicked();
            no = ui.button(crate::i18n::tr("Cancel")).clicked();
        });
    });
    if yes {
        app.leave_net();
        return Some(p);
    }
    if !no {
        app.menu.leave_pending = Some(p);
    }
    None
}

/// The quick load from the menu: asked first when the game behind the menu has more than a
/// minute of unsaved play, as F9 is (QA round 7, #1). True when it is to load now.
fn ask_load(app: &mut App, ctx: &egui::Context, load: bool) -> bool {
    if load
        && crate::app::load_needs_asking(app.sim.gf().0, app.session.saved_gf)
        && app.playback.is_none()
        && app.net.is_none()
    {
        app.menu.load_pending = true;
        return false;
    }
    if !app.menu.load_pending {
        return load;
    }
    let (mut yes, mut no) = (false, false);
    egui::Modal::new(egui::Id::new("menu_load")).show(ctx, |ui| {
        ui.heading(crate::i18n::tr("Load the quick save?"));
        ui.label(crate::i18n::tr("What happened since the last save is lost."));
        ui.horizontal(|ui| {
            yes = ui.button(crate::i18n::tr("Load")).clicked();
            no = ui.button(crate::i18n::tr("Cancel")).clicked();
        });
    });
    if yes || no {
        app.menu.load_pending = false;
    }
    yes
}

/// The question before a network game is left: the host's leaving ends it for everyone.
fn leaving_question(host: bool) -> &'static str {
    if host {
        "Leave the network game? You are the host: the game ends for everyone, and nobody can come back."
    } else {
        "Leave the network game? The others play on without you, and you can't come back."
    }
}

/// What the title screen asked for this frame.
#[derive(Default)]
struct Picks {
    start: Option<crate::Args>,
    load: bool,
    open_editor: bool,
    load_path: Option<std::path::PathBuf>,
    replay: bool,
}

impl Picks {
    /// Whether this pick replaces the running game.
    fn leaves_the_game(&self) -> bool {
        self.start.is_some() || self.open_editor || self.replay
    }
}

/// The best run of a mission at the chosen level, for its hover (empty if none).
fn best_line(app: &App, key: &str) -> String {
    let Some(b) = crate::settings::best_of(&app.best, key, app.menu.ai_level) else { return String::new() };
    let limits = crate::missions::mission(key).map_or(crate::missions::WORLD_MEDALS, |m| m.medals);
    let medal = crate::settings::medal(b.minutes, limits, app.menu.ai_level);
    let mut line = crate::i18n::msg(
        "Your best at this level: {1} minutes, score {2}",
        &[&b.minutes.to_string(), &b.score.to_string()],
    );
    if !medal.is_empty() {
        line = format!("{line} ({})", crate::i18n::tr(medal));
    }
    format!("\n{}", crate::i18n::show(&line))
}

/// Who fights you in a mission at the chosen level, and which variant of its map the seed
/// picks (game design round 1, #4: the variants hid behind the free game's seed), for its hover.
fn cast_line(app: &App, key: &str) -> String {
    let level = crate::level(app.menu.ai_level);
    let variant = crate::i18n::show(&crate::i18n::msg(
        "Map variant {1} (the seed below picks another)",
        &[&app.menu.seed.to_string()],
    ));
    crate::missions::mission(key)
        .map_or(String::new(), |m| format!("\n{}\n{variant}", crate::missions::cast_line(m, level)))
}

/// The campaign: the first valley, Europe, then the numbered missions (won ones ticked).
fn campaign_buttons(app: &App, ui: &mut egui::Ui, p: &mut Picks) {
    let won = |k: &str| if app.won_missions.iter().any(|m| m == k) { " ✔" } else { "" };
    // the small first mission before Europe (genre lessons, PLAN §27, #3)
    let first = crate::missions::mission("first").expect("mission");
    // the bigger button for the mission to play next: the first valley until it is won
    // (new-player round 11, #5: the teaching mission was the slim one)
    let learning = won("first").is_empty();
    let (big, slim) = ([260.0, 34.0], [260.0, 26.0]);
    let label = format!("▶  {}{}", crate::i18n::tr(first.title), won("first"));
    let label = if learning { RichText::new(label).size(18.0) } else { RichText::new(label) };
    if ui
        .add_sized(if learning { big } else { slim }, egui::Button::new(label))
        .on_hover_text(format!(
            "{}\n{}{}{}",
            crate::i18n::tr(first.hint),
            crate::i18n::tr(first.goal),
            cast_line(app, "first"),
            best_line(app, "first")
        ))
        .clicked()
    {
        p.start = Some(crate::Args {
            mission: Some("first".into()),
            seed: app.menu.seed,
            ai_level: app.menu.ai_level,
            goods: app.menu.goods,
            ..Default::default()
        });
    }
    ui.add_space(4.0);
    let europe = format!("⚔  {}{}", crate::i18n::tr("The Europe mission"), won("europe"));
    let europe = if learning { RichText::new(europe) } else { RichText::new(europe).size(18.0) };
    if ui
        .add_sized(if learning { slim } else { big }, egui::Button::new(europe))
        .on_hover_text(format!(
            "{}\n{}{}{}",
            crate::i18n::tr("You hold Italy. Five rulers share Europe with you."),
            crate::i18n::tr(crate::missions::mission("europe").expect("mission").hint),
            cast_line(app, "europe"),
            best_line(app, "europe")
        ))
        .clicked()
    {
        p.start = Some(crate::Args {
            mission: Some("europe".into()),
            seed: app.menu.seed,
            ai_level: app.menu.ai_level,
            goods: app.menu.goods,
            ..Default::default()
        });
    }
    ui.add_space(6.0);
    ui.separator();
    // wraps when the names are long (German)
    ui.horizontal_wrapped(|ui| {
        // numbered in the campaign's order, each with its own line, won ones ticked
        // (numbered from Europe, the first campaign mission: the first steps come before it)
        for (n, key) in crate::missions::CAMPAIGN.iter().enumerate().skip(2) {
            let m = crate::missions::mission(key).expect("mission");
            let label = format!("{} · {}{}", n, crate::i18n::tr(m.title), won(key));
            let hover = format!(
                "{}\n{}{}{}",
                crate::i18n::tr(m.hint),
                crate::i18n::tr(m.goal),
                cast_line(app, key),
                best_line(app, key)
            );
            if ui.button(label).on_hover_text(hover).clicked() {
                p.start = Some(crate::Args {
                    mission: Some((*key).into()),
                    seed: app.menu.seed,
                    ai_level: app.menu.ai_level,
                    goods: app.menu.goods,
                    ..Default::default()
                });
            }
        }
    });
}

/// The Roman campaign's chapters in a row, numbered I, II, ...: a chapter opens once the one
/// before it is won (docs/CAMPAIGN-ROME.md).
fn rome_buttons(app: &App, ui: &mut egui::Ui, p: &mut Picks) {
    const NUMERALS: [&str; 10] = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"];
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(crate::i18n::tr("The Roman campaign")).strong());
        for (n, key) in crate::missions::ROME.iter().enumerate() {
            let m = crate::missions::mission(key).expect("chapter");
            let won = if app.won_missions.iter().any(|w| w == key) { " ✔" } else { "" };
            let label = format!("{} · {}{won}", NUMERALS[n % 10], crate::i18n::tr(m.title));
            let open = crate::missions::unlocked(key, &app.won_missions);
            let hover = if open {
                format!(
                    "{}\n{}{}{}",
                    crate::i18n::tr(m.hint),
                    crate::i18n::tr(m.goal),
                    cast_line(app, key),
                    best_line(app, key)
                )
            } else {
                crate::i18n::tr("Win the chapter before it first.").to_string()
            };
            let b = ui.add_enabled(open, egui::Button::new(label));
            let b = if open { b.on_hover_text(hover) } else { b.on_disabled_hover_text(hover) };
            if b.clicked() {
                p.start = Some(crate::Args {
                    mission: Some((*key).into()),
                    seed: app.menu.seed,
                    ai_level: app.menu.ai_level,
                    goods: app.menu.goods,
                    ..Default::default()
                });
            }
        }
    });
}

/// Missions, opponents and the free game.
/// Today's map: a seed from the date (UTC), the same on every machine.
fn todays_seed(now: std::time::SystemTime) -> u64 {
    let days = now.duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() / 86_400);
    // (spread out, so the days' maps don't look alike)
    days.wrapping_mul(2_654_435_761) % 1_000_000 + 1
}

/// Today's map whole: the seed and the setup that makes the map (veteran round 12, #5: the seed
/// alone gave friends with another player count or size another map).
fn todays_map(seed: u64) -> (u64, usize, u16, rr_core::Landscape) {
    (seed, 4, 128, rr_core::Landscape::Greenland)
}

/// The free game's landscape, and whether it is an archipelago.
fn landscape_row(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        for (l, n) in [
            (rr_core::Landscape::Greenland, "Greenland"),
            (rr_core::Landscape::Wasteland, "Wasteland"),
            (rr_core::Landscape::Winter, "Winter world"),
        ] {
            if ui.selectable_label(app.menu.landscape == l, crate::i18n::tr(n)).clicked() {
                app.menu.landscape = l;
            }
        }
        ui.checkbox(&mut app.menu.islands, crate::i18n::tr("Islands")).on_hover_text(crate::i18n::tr(
            "Every realm on its own island, with islands of ore and forest between: harbors and expeditions reach the rest.",
        ));
    });
}

fn play_column(app: &mut App, ui: &mut egui::Ui, p: &mut Picks) {
    campaign_buttons(app, ui, p);
    rome_buttons(app, ui, p);
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(crate::i18n::tr("Opponents:"));
        for (i, (n, hint)) in [
            ("easy", "The rivals build slowly and attack late: for your first games."),
            ("normal", "The rivals expand fast and attack once the truce is over."),
            ("hard", "The rivals build and attack at full speed."),
        ]
        .iter()
        .enumerate()
        {
            if ui
                .selectable_label(app.menu.ai_level == i as u8, crate::i18n::tr(n))
                .on_hover_text(crate::i18n::tr(hint))
                .clicked()
            {
                app.menu.ai_level = i as u8;
            }
        }
    });
    ui.horizontal(|ui| {
        ui.label(crate::i18n::tr("Starting goods:"));
        for (g, n) in [
            (rr_sim::StartGoods::Low, "low"),
            (rr_sim::StartGoods::Normal, "medium"),
            (rr_sim::StartGoods::High, "high"),
        ] {
            if ui.selectable_label(app.menu.goods == g, crate::i18n::tr(n)).clicked() {
                app.menu.goods = g;
            }
        }
    });
    ui.separator();
    ui.label(crate::i18n::tr("Free game on a random map"));
    ui.add(egui::Slider::new(&mut app.menu.players, 2..=6).text(crate::i18n::tr("players")));
    ui.add(egui::Slider::new(&mut app.menu.size, 64..=192).step_by(16.0).text(crate::i18n::tr("map size")));
    ui.horizontal(|ui| {
        ui.add(egui::DragValue::new(&mut app.menu.seed).prefix(crate::i18n::tr("seed ")));
        // the same map for everyone today (genre lessons, PLAN §27, #13: varied starts to share)
        let today = todays_seed(std::time::SystemTime::now());
        let on = (app.menu.seed, app.menu.players, app.menu.size, app.menu.landscape) == todays_map(today);
        if ui
            .selectable_label(on, crate::i18n::tr("Today's map"))
            .on_hover_text(crate::i18n::tr(
                "The same map for everyone who plays today (4 players, size 128, Greenland): compare your game with friends",
            ))
            .clicked()
        {
            (app.menu.seed, app.menu.players, app.menu.size, app.menu.landscape) = todays_map(today);
        }
    });
    landscape_row(app, ui);
    ui.collapsing(crate::i18n::tr("Players"), |ui| {
        egui::Grid::new("player_table").show(ui, |ui| {
            let nations = ["Romans", "Vikings", "Nubians", "Japanese"];
            for i in 0..app.menu.players {
                let (nation, team) = &mut app.menu.table[i];
                let who =
                    if i == 0 { crate::i18n::tr("You").to_string() } else { format!("{} {i}", crate::i18n::tr("AI")) };
                ui.label(RichText::new(who).color(crate::render::player_text_color(i as u8)));
                egui::ComboBox::from_id_salt(("nation", i))
                    .selected_text(crate::i18n::tr(nations[*nation as usize % 4]))
                    .show_ui(ui, |ui| {
                        for (k, n) in nations.iter().enumerate() {
                            ui.selectable_value(nation, k as u8, crate::i18n::tr(n));
                        }
                    });
                egui::ComboBox::from_id_salt(("team", i))
                    .selected_text(if *team == 0 {
                        crate::i18n::tr("no team").to_string()
                    } else {
                        format!("{} {team}", crate::i18n::tr("Team"))
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(team, 0, crate::i18n::tr("no team"));
                        for k in 1..=3u8 {
                            ui.selectable_value(team, k, format!("{} {k}", crate::i18n::tr("Team")));
                        }
                    });
                // a level per computer player (game design round 1, #3)
                if i > 0 {
                    let names = ["easy", "normal", "hard"];
                    let text =
                        |l: u8| names.get(l as usize).map_or(crate::i18n::tr("as above"), |n| crate::i18n::tr(n));
                    let level = &mut app.menu.levels[i];
                    egui::ComboBox::from_id_salt(("level", i)).selected_text(text(*level)).show_ui(ui, |ui| {
                        for l in [255u8, 0, 1, 2] {
                            ui.selectable_value(level, l, text(l));
                        }
                    });
                }
                ui.end_row();
            }
        });
    });
    ui.checkbox(&mut app.menu.allied, crate::i18n::tr("The computer players are allies against you"));
    ui.horizontal(|ui| {
        ui.label(crate::i18n::tr("Win:"));
        // (land, coins): the economy goal is RTTR's economy mode in small (PLAN §27, #12)
        let text = |(land, coins): (u32, u32)| match (land, coins) {
            (0, 0) => crate::i18n::tr("when all rivals are defeated").to_string(),
            (0, c) => crate::i18n::show(&crate::i18n::msg("first to mint {1} gold coins", &[&c.to_string()])),
            (g, _) => crate::i18n::show(&crate::i18n::msg("with {1} % of the land", &[&g.to_string()])),
        };
        let mut goal = (app.menu.land_goal, app.menu.coins_goal);
        egui::ComboBox::from_id_salt("land_goal").selected_text(text(goal)).show_ui(ui, |ui| {
            for g in [(0, 0), (60, 0), (75, 0), (0, 60)] {
                ui.selectable_value(&mut goal, g, text(g));
            }
        });
        (app.menu.land_goal, app.menu.coins_goal) = goal;
    });
    ui.horizontal(|ui| {
        minutes_choice(
            ui,
            "peace",
            crate::i18n::tr("Peace time"),
            &mut app.menu.peace,
            &[0, 15, 30, 60, crate::app::PEACE_FOR_GOOD],
        );
        minutes_choice(ui, "limit", crate::i18n::tr("Time limit"), &mut app.menu.time_limit, &[0, 30, 60, 90, 120]);
    });
    if ui.add_sized([260.0, 30.0], egui::Button::new(crate::i18n::tr("Start free game"))).clicked() {
        p.start = Some(crate::Args {
            allied_ais: app.menu.allied,
            player_table: app.menu.table[..app.menu.players].to_vec(),
            levels: app.menu.levels[..app.menu.players].to_vec(),
            landscape: app.menu.landscape,
            islands: app.menu.islands,
            land_goal: app.menu.land_goal,
            coins_goal: app.menu.coins_goal,
            peace: app.menu.peace,
            time_limit: app.menu.time_limit,
            players: app.menu.players,
            size: app.menu.size,
            seed: app.menu.seed,
            ai_level: app.menu.ai_level,
            goods: app.menu.goods,
            ..Default::default()
        });
    }
}

/// Continue, load, LAN, map files and the rest.
/// Why something just now couldn't be done here (net round 5, #8: a load during a network game
/// failed with the reason drawn nowhere).
fn menu_note(app: &App, ui: &mut egui::Ui) {
    if !app.menu.note.is_empty() {
        ui.label(RichText::new(&app.menu.note).color(Color32::LIGHT_YELLOW));
    }
}

fn more_column(app: &mut App, ui: &mut egui::Ui, ctx: &egui::Context, p: &mut Picks) {
    menu_note(app, ui);
    // after trying a map out: back to its editor, undo history and all
    if let Some(name) = app.tested.as_ref().map(|e| e.name.clone())
        && ui
            .add_sized([260.0, 28.0], egui::Button::new(format!("🗺 {}: {name}", crate::i18n::tr("Back to the editor"))))
            .clicked()
    {
        app.leave_net();
        app.save_replay();
        app.editor = app.tested.take().map(|e| *e);
        if let Some(ed) = app.editor.as_mut() {
            ed.stale = true;
        }
        app.in_menu = false;
        app.tex = None;
        app.minimap = None;
    }
    if app.sim.gf().0 > 0
        && app.session.game_over.is_none()
        && ui.add_sized([260.0, 28.0], egui::Button::new(crate::i18n::tr("Continue"))).clicked()
    {
        app.in_menu = false;
    }
    let have_quick = App::save_path().exists();
    if ui
        .add_enabled(have_quick, egui::Button::new(crate::i18n::tr("Load quick save")).min_size(Vec2::new(260.0, 28.0)))
        .on_disabled_hover_text(crate::i18n::tr("No quick save yet. Press F5 during a game to make one."))
        .clicked()
    {
        p.load = true;
    }
    let saves = crate::app::App::list_saves();
    if !saves.is_empty() {
        ui.menu_button(crate::i18n::tr("Load a saved game"), |ui| {
            for e in saves.iter().take(20) {
                if ui.button(format!("{}   ·   {}", e.name, e.info())).clicked() {
                    p.load_path = Some(e.path.clone());
                    ui.close_menu();
                }
            }
        });
    }
    ui.add_space(6.0);
    ui.separator();
    crate::netui::menu_section(app, ui);
    ui.separator();
    ui.label(crate::i18n::tr("Play a map file (.rrmap from the editor, .wld/.swd e.g. from your own copy)"));
    let maps = app.menu.maps.get_or_insert_with(find_maps).clone();
    ui.horizontal(|ui| {
        let current = maps
            .iter()
            .find(|(p, _)| p.to_string_lossy() == app.menu.map_path)
            .map(|(_, l)| l.clone())
            .unwrap_or_default();
        egui::ComboBox::from_id_salt("maps").width(250.0).selected_text(current).show_ui(ui, |ui| {
            for (m, label) in &maps {
                if ui.selectable_label(app.menu.map_path == m.to_string_lossy(), label).clicked() {
                    app.menu.map_path = m.to_string_lossy().to_string();
                }
            }
        });
        if ui.small_button("↻").on_hover_text(crate::i18n::tr("Look for map files again")).clicked() {
            app.menu.maps = None;
        }
    });
    ui.add(
        egui::TextEdit::singleline(&mut app.menu.map_path)
            .hint_text(crate::i18n::tr("/path/to/MAP.WLD"))
            .desired_width(250.0),
    );
    // which slot of the map is yours (read once per chosen file)
    if app.menu.map_players.as_ref().is_none_or(|(p, _)| *p != app.menu.map_path) {
        let n = std::fs::read(&app.menu.map_path)
            .ok()
            .and_then(|b| rr_formats::rrmap::read_any(&b).ok())
            .map_or(0, |m| m.players());
        app.menu.map_players = Some((app.menu.map_path.clone(), n));
    }
    let n = app.menu.map_players.as_ref().map_or(0, |(_, n)| *n);
    if n > 1 {
        ui.horizontal(|ui| {
            ui.label(crate::i18n::tr("You play:"));
            for s in 0..n.min(7) as u8 {
                let label = RichText::new(format!("{}", s + 1)).color(crate::render::player_color(s));
                ui.selectable_value(&mut app.menu.me, s, label);
            }
        });
    }
    if !app.menu.map_path.is_empty()
        && ui.add_sized([260.0, 28.0], egui::Button::new(crate::i18n::tr("Play this map"))).clicked()
    {
        let map = std::fs::read(&app.menu.map_path).ok().and_then(|b| rr_formats::rrmap::read_any(&b).ok());
        app.menu.map_msg = match &map {
            None => crate::i18n::tr("This file is no map that can be read.").into(),
            Some(m) => crate::setup_problem(m).unwrap_or_default(),
        };
        // a broken mission file stops the start, with where it broke (map maker round 1, #3)
        if app.menu.map_msg.is_empty()
            && let Err(e) = crate::missions::MapMission::read(std::path::Path::new(&app.menu.map_path))
        {
            app.menu.map_msg =
                format!("{e}\n{}", crate::i18n::tr("Fix the mission file, or remove it to play without it."));
        }
        if app.menu.map_msg.is_empty() {
            // the free game's player table, allies and objective apply to map files too
            let n = map.as_ref().map_or(2, |m| m.players()).min(app.menu.table.len());
            p.start = Some(crate::Args {
                map: Some(app.menu.map_path.clone()),
                player_table: app.menu.table[..n].to_vec(),
                levels: app.menu.levels[..n].to_vec(),
                allied_ais: app.menu.allied,
                land_goal: app.menu.land_goal,
                coins_goal: app.menu.coins_goal,
                peace: app.menu.peace,
                time_limit: app.menu.time_limit,
                // (the map's mission may name the human's slot: map maker round 1, #2)
                me: crate::missions::MapMission::load(std::path::Path::new(&app.menu.map_path))
                    .and_then(|m| m.human)
                    .unwrap_or(app.menu.me)
                    .min(n.saturating_sub(1) as u8),
                seed: app.menu.seed,
                ai_level: app.menu.ai_level,
                goods: app.menu.goods,
                ..Default::default()
            });
        }
    }
    if !app.menu.map_msg.is_empty() {
        ui.label(RichText::new(&app.menu.map_msg).color(Color32::from_rgb(255, 200, 90)));
    }
    ui.add_space(6.0);
    ui.separator();
    if (crate::replay_path().exists() || app.recorder.is_some() && app.sim.gf().0 >= 100)
        && ui.add_sized([260.0, 28.0], egui::Button::new(crate::i18n::tr("Watch the last game"))).clicked()
    {
        p.replay = true;
    }
    if ui.add_sized([260.0, 28.0], egui::Button::new(crate::i18n::tr("🗺 Map editor"))).clicked() {
        p.open_editor = true;
    }
    if ui.add_sized([260.0, 28.0], egui::Button::new(crate::i18n::tr("⚙ Options"))).clicked() {
        app.ui.options = !app.ui.options;
    }
    if ui.add_sized([260.0, 28.0], egui::Button::new(crate::i18n::tr("Credits"))).clicked() {
        app.menu.credits = !app.menu.credits;
    }
    if ui.add_sized([260.0, 28.0], egui::Button::new(crate::i18n::tr("Quit"))).clicked() {
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
    ui.add_space(8.0);
    ui.label(
        RichText::new(crate::i18n::tr("Music: RandomMind, Woli34 · Sounds: Kenney (CC0) · Coastlines: Natural Earth"))
            .small(),
    );
}

/// A choice of a number of minutes, 0 shown as "none" (the free game's peace time and limit).
fn minutes_choice(ui: &mut egui::Ui, id: &str, label: &str, value: &mut u32, choices: &[u32]) {
    let text = |m: u32| match m {
        0 => crate::i18n::tr("none").to_string(),
        crate::app::PEACE_FOR_GOOD => crate::i18n::tr("for good").to_string(),
        m => crate::i18n::fill("{1} min", &[&m.to_string()]),
    };
    ui.label(label);
    egui::ComboBox::from_id_salt(id).width(70.0).selected_text(text(*value)).show_ui(ui, |ui| {
        for m in choices {
            ui.selectable_value(value, *m, text(*m));
        }
    });
}

#[cfg(test)]
mod leave_tests {
    /// PLAN §27, #13: today's map is the same all day, another tomorrow.
    #[test]
    fn todays_map_changes_with_the_day() {
        let at = |secs: u64| super::todays_seed(std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs));
        let day = 20_000 * 86_400;
        assert_eq!(at(day + 10), at(day + 86_399));
        assert_ne!(at(day), at(day + 86_400));
        assert!(at(day) > 0);
        // the whole map: players, size and landscape too (veteran round 12, #5)
        assert_eq!(super::todays_map(7), (7, 4, 128, rr_core::Landscape::Greenland));
    }

    use super::*;

    /// Art director round 4, #9: the menu shows the game's own look.
    #[test]
    fn the_menu_shows_the_game() {
        assert!(rr_audio::assets_dir().join("art").join(MENU_BACKDROP).is_file());
        assert_ne!(MENU_BACKDROP, "menu_bg.jpg");
    }

    /// Net round 4, #2: a new game, the editor or a replay from the menu ask before ending a
    /// network game; the host is told it ends for everyone.
    #[test]
    fn leaving_a_network_game_asks() {
        assert!(Picks { replay: true, ..Default::default() }.leaves_the_game());
        assert!(Picks { open_editor: true, ..Default::default() }.leaves_the_game());
        assert!(!Picks::default().leaves_the_game());
        assert!(leaving_question(true).contains("everyone") && !leaving_question(false).contains("everyone"));
        for h in [true, false] {
            assert!(crate::i18n::has_german(leaving_question(h)));
        }
    }
}
