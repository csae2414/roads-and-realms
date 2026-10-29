//! Roads & Realms: the playable game.
//!
//! `rr-app [--map FILE] [--seed N] [--players N] [--size N] [--screenshot OUT.png --gf N]`

// (on Windows a window of its own, without a console beside it)
#![cfg_attr(windows, windows_subsystem = "windows")]

mod ambience;
mod animals;
mod app;
mod editor;
mod hover;
mod i18n;
mod idle;
mod keys;
mod menu;
mod missions;
mod netui;
mod render;
mod settings;
mod sprites;
mod tex;
mod ui;

use rr_core::MapData;
use rr_rules::Rules;
use rr_sim::{GameSetup, Nation, PlayerSetup, Sim};
use std::sync::Arc;

#[derive(Clone, Debug, Default)]
pub struct Args {
    pub map: Option<String>,
    pub seed: u64,
    pub players: usize,
    pub size: u16,
    pub screenshot: Option<String>,
    pub gf: u32,
    pub auto: bool,
    /// A world-campaign mission ("europe", "mediterranean", "northamerica", "fareast", "north").
    pub mission: Option<String>,
    /// Any game option given on the command line: skip the menu.
    pub explicit: bool,
    pub menu: bool,
    pub zoom: Option<f32>,
    /// 0 easy, 1 normal, 2 hard
    pub ai_level: u8,
    /// Screenshot helper: "ship" centres the view on the first ship.
    pub focus: Option<String>,
    /// Start a road from a new building's flag right after placing it.
    pub auto_road: bool,
    /// What every headquarters starts with.
    pub goods: rr_sim::StartGoods,
    /// Free games: greenland, wasteland or winter.
    pub landscape: rr_core::Landscape,
    /// Free games: an archipelago, every realm on its own island.
    pub islands: bool,
    /// Free games: (nation, team) per player; team 0 means none.
    pub player_table: Vec<(u8, u8)>,
    /// Free games: the level per player slot (0 easy, 1 normal, 2 hard; else `ai_level`).
    pub levels: Vec<u8>,
    /// Free games: the computer players are allies against you.
    pub allied_ais: bool,
    /// Free games: win with three quarters of all land instead of total victory.
    /// Free games: win with this share of all land, in percent (0: no such goal).
    pub land_goal: u32,
    /// Free games: minutes of peace at the start, for everybody (game design round 1, #6).
    pub peace: u32,
    /// Free games: minutes until the side with the most land wins; 0 for none (RTTR's
    /// tournament modes).
    pub time_limit: u32,
    /// Free games: whoever first has this many gold coins in store wins (0: no such goal).
    pub coins_goal: u32,
    /// The player slot the human plays (map files; 0 otherwise).
    pub me: u8,
}

/// The command line's usage: every flag `parse_args_from` knows (a test checks it).
const HELP: &str = "rr-app [options]
  --map FILE.wld|.swd|.rrmap   play a map file (its .mission.json too)   --me N   the human's slot
  --europe | --mission first|europe|northamerica|north|fareast|mediterranean
  --seed N  --players N (at most 7)  --size N  --landscape greenland|wasteland|winter  --islands  --goods low|medium|high
  --ai easy|normal|hard  --allied
  --land-goal PERCENT (--three-quarters: 75)  --coins-goal N  --time-limit MINUTES
  --peace MINUTES  --peaceful (for good)
  --auto-road | --no-auto-road  --zoom F  --menu  --auto
  --rules FILE (rules of the classic file's shape; network games keep the classic rules)
  --screenshot OUT.png --gf N  --focus ship  --help";

/// The rules games on this machine play by: the classic ones, or those of `--rules FILE` (map
/// maker round 1, #1). Network games always play the classic rules, which every machine has.
static RULES: std::sync::OnceLock<Arc<Rules>> = std::sync::OnceLock::new();

pub fn rules() -> Arc<Rules> {
    RULES.get_or_init(|| Arc::new(Rules::classic())).clone()
}

fn parse_args() -> Args {
    parse_args_from(std::env::args().skip(1))
}

fn parse_args_from(mut it: impl Iterator<Item = String>) -> Args {
    let mut a = Args { seed: 1, players: 2, size: 96, ai_level: 1, auto_road: true, ..Default::default() };
    let mut me_given = false;
    while let Some(k) = it.next() {
        a.explicit = true;
        match k.as_str() {
            "--map" => a.map = it.next(),
            "--seed" => a.seed = it.next().and_then(|v| v.parse().ok()).unwrap_or(1),
            "--players" => a.players = it.next().and_then(|v| v.parse().ok()).unwrap_or(2),
            "--size" => a.size = it.next().and_then(|v| v.parse().ok()).unwrap_or(96),
            "--screenshot" => a.screenshot = it.next(),
            "--gf" => a.gf = it.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            "--auto" => a.auto = true,
            "--auto-road" => a.auto_road = true,
            "--no-auto-road" => a.auto_road = false,
            "--allied" => a.allied_ais = true,
            "--islands" => a.islands = true,
            "--landscape" => {
                a.landscape = match it.next().as_deref() {
                    Some("wasteland") => rr_core::Landscape::Wasteland,
                    Some("winter") => rr_core::Landscape::Winter,
                    _ => rr_core::Landscape::Greenland,
                }
            }
            "--three-quarters" => a.land_goal = 75,
            "--land-goal" => a.land_goal = it.next().and_then(|v| v.parse().ok()).unwrap_or(0).min(100),
            "--peace" => a.peace = it.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            "--coins-goal" => a.coins_goal = it.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            "--peaceful" => a.peace = crate::app::PEACE_FOR_GOOD,
            "--time-limit" => a.time_limit = it.next().and_then(|v| v.parse().ok()).unwrap_or(0).min(24 * 60),
            "--europe" => a.mission = Some("europe".into()),
            "--mission" => a.mission = it.next(),
            "--rules" => {
                let p = it.next().unwrap_or_default();
                let r = Rules::from_file(std::path::Path::new(&p)).unwrap_or_else(|e| fail(&e));
                let _ = RULES.set(Arc::new(r));
            }
            // (the human's slot on a map file; map maker round 1, #4)
            "--me" => {
                a.me = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                me_given = true;
            }
            "--menu" => a.menu = true,
            "--ai" => {
                a.ai_level = match it.next().as_deref() {
                    Some("easy") => 0,
                    Some("hard") => 2,
                    _ => 1,
                }
            }
            "--goods" => {
                a.goods = match it.next().as_deref() {
                    Some("low") => rr_sim::StartGoods::Low,
                    Some("high") => rr_sim::StartGoods::High,
                    _ => rr_sim::StartGoods::Normal,
                }
            }
            "--focus" => a.focus = it.next(),
            "--zoom" => a.zoom = it.next().and_then(|v| v.parse().ok()),
            "--help" | "-h" => {
                println!("{HELP}");
                std::process::exit(0);
            }
            other => eprintln!("unknown argument {other}"),
        }
    }
    // a map's mission may name the human's slot; --me wins (map maker round 1, #2)
    if !me_given
        && let Some(h) =
            a.map.as_deref().and_then(|p| missions::MapMission::load(std::path::Path::new(p))).and_then(|m| m.human)
    {
        a.me = h;
    }
    // (the map format knows seven player slots; asking for nine used to play seven without a word)
    if a.players > rr_mapgen::MAX_PLAYERS {
        eprintln!("at most {} players, not {}; playing {}", rr_mapgen::MAX_PLAYERS, a.players, rr_mapgen::MAX_PLAYERS);
        a.players = rr_mapgen::MAX_PLAYERS;
    }
    a
}

pub fn load_map(a: &Args) -> MapData {
    if let Some(path) = &a.map {
        // (a message and exit 1, not a panic's backtrace: map maker round 1, #6)
        let bytes = std::fs::read(path).unwrap_or_else(|e| fail(&format!("{path}: {e}")));
        return rr_formats::rrmap::read_any(&bytes).unwrap_or_else(|e| fail(&format!("{path}: {e}")));
    }
    if let Some(m) = a.mission.as_deref().and_then(missions::mission) {
        return rr_game::mission_map(m, a.seed);
    }
    if let Some(r) = a.mission.as_deref().and_then(rr_mapgen::world::region) {
        return rr_mapgen::region::generate(r, a.seed);
    }
    rr_mapgen::generate(&rr_mapgen::GenParams {
        width: a.size,
        height: a.size,
        players: a.players,
        seed: a.seed,
        landscape: a.landscape,
        islands: a.islands,
        ..Default::default()
    })
}

/// The level of the computer player in slot `p`: its own from the Players table, else the
/// menu's (game design round 1, #3).
pub fn level_of(a: &Args, p: u8) -> rr_ai::Level {
    a.levels.get(p as usize).filter(|l| **l <= 2).map_or_else(|| ai_level(a), |l| level(*l))
}

pub fn ai_level(a: &Args) -> rr_ai::Level {
    level(a.ai_level)
}

/// A level from its number (0 easy, 1 normal, 2 hard; anything else normal): the one mapping
/// (architect round 3, #12: it was written three times).
pub fn level(n: u8) -> rr_ai::Level {
    match n {
        0 => rr_ai::Level::Easy,
        2 => rr_ai::Level::Hard,
        _ => rr_ai::Level::Normal,
    }
}

pub fn new_game(a: &Args) -> Sim {
    new_game_with_replay(a).unwrap_or_else(|e| fail(&e)).0
}

/// A new game plus an empty replay recorder for it.
pub fn new_game_with_replay(a: &Args) -> Result<(Sim, rr_sim::Replay), String> {
    let map = load_map(a);
    let nations = [Nation::Romans, Nation::Vikings, Nation::Nubians, Nation::Japanese];
    let players: Vec<PlayerSetup> = if let Some(m) = a.mission.as_deref().and_then(missions::mission) {
        // you and the rivals as the mission casts them: allied against you, each on its own, or
        // peaceful (game design round 1, #2), as many at war as the level says (#3)
        rr_game::mission_players(m, ai_level(a))
            .into_iter()
            .map(|(n, nation, t)| PlayerSetup { name: i18n::tr(n).into(), nation, team: Some(t) })
            .collect()
    } else {
        // a map's own mission may name its players
        let names = a.map.as_deref().and_then(|p| missions::MapMission::load(std::path::Path::new(p))).map(|m| m.names);
        (0..map.players())
            .map(|i| PlayerSetup {
                name: match names.as_ref().and_then(|n| n.get(i)).filter(|n| !n.is_empty()) {
                    Some(n) => n.clone(),
                    None if i == a.me as usize => i18n::tr("You").into(),
                    None => format!("{} {i}", i18n::tr("AI")),
                },
                // the game setup's player table, if given: nation and team (0 = none)
                nation: a.player_table.get(i).map_or(nations[i % 4], |e| nations[e.0 as usize % 4]),
                team: if a.allied_ais {
                    Some(if i == a.me as usize { 0 } else { 1 })
                } else {
                    a.player_table.get(i).and_then(|e| (e.1 > 0).then_some(e.1))
                },
            })
            .collect()
    };
    let rules = rules();
    // (a campaign mission is played from its first slot: --me 1 made you Aldric; QA round 8, #10)
    if a.me != 0 && a.mission.is_some() {
        return Err("--me is for map files; a campaign mission is played from the first slot".into());
    }
    // (a mission has its own goal: a time limit ended the first valley with "victory" on a tie;
    // QA round 9, #5)
    if a.mission.is_some() && (a.time_limit > 0 || a.coins_goal > 0 || a.land_goal > 0) {
        return Err("--time-limit, --coins-goal and --land-goal are for free games; a mission has its own goal".into());
    }
    if a.me as usize >= players.len() {
        return Err(format!(
            "--me {}: this map has {} players (0..={})",
            a.me,
            players.len(),
            players.len().saturating_sub(1)
        ));
    }
    let setup = GameSetup { seed: a.seed, players, start_goods: a.goods };
    let replay = rr_sim::Replay::new(&rules, &map, &setup);
    let sim = Sim::new(rules, &map, &setup).map_err(|e| setup_error_text(&e))?;
    Ok((sim, replay))
}

/// Stop with a message (a game given on the command line can't start).
pub fn fail(e: &str) -> ! {
    eprintln!("{e}");
    std::process::exit(1)
}

/// Why no game can start on a map, if it can't.
pub fn setup_problem(map: &rr_core::MapData) -> Option<String> {
    let n = map.hq.iter().flatten().count();
    if n == 0 {
        return Some(i18n::tr("The map has no headquarters.").into());
    }
    let mut m = map.clone();
    m.hq = m.hq.iter().flatten().map(|p| Some(*p)).collect();
    let setup = GameSetup { players: vec![Default::default(); n], ..Default::default() };
    Sim::new(rules(), &m, &setup).err().map(|e| setup_error_text(&e))
}

fn setup_error_text(e: &rr_sim::SetupError) -> String {
    match e {
        rr_sim::SetupError::HqNotBuildable(i) => {
            format!("{} {}", i18n::tr("No room for a headquarters of player"), i + 1)
        }
        e => e.to_string(),
    }
}

/// Where the last game's replay is kept.
pub fn replay_path() -> std::path::PathBuf {
    app::App::save_path().with_file_name("last.rrreplay")
}

/// The window size before maximising: `RR_WINDOW=WxH` (headless tests) or 1280x800.
fn window_size() -> [f32; 2] {
    std::env::var("RR_WINDOW").ok().and_then(|v| parse_size(&v)).unwrap_or([1280.0, 800.0])
}

fn parse_size(v: &str) -> Option<[f32; 2]> {
    let (w, h) = v.split_once('x')?;
    Some([w.trim().parse().ok()?, h.trim().parse().ok()?])
}

/// The game's id on the desktop: its `.desktop` file and icon carry this name.
const APP_ID: &str = "roads-and-realms";

/// The window's icon (`assets/icon`, the launcher's picture); none if the file is missing.
fn window_icon() -> Option<eframe::egui::IconData> {
    let path = rr_audio::assets_dir().join("icon").join("roads-and-realms-256.png");
    let img = image::open(path).ok()?.into_rgba8();
    let (width, height) = img.dimensions();
    Some(eframe::egui::IconData { rgba: img.into_raw(), width, height })
}

/// A failure that ends the game before or while it runs: written to `error.log` beside the
/// saves, and on Windows shown in a message box (the game has no console there, so a failure
/// to open the window said nothing at all; owner's Windows test, 2026-10-27).
fn report_fatal(what: &str) {
    let log = app::App::save_path().with_file_name("error.log");
    if let Some(dir) = log.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&log, format!("{what}\n"));
    eprintln!("{what}");
    let text = format!("Roads & Realms could not run:\n\n{what}\n\n(also written to {})", log.display());
    message_box(&text);
}

#[cfg(windows)]
fn message_box(text: &str) {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn MessageBoxW(hwnd: *mut std::ffi::c_void, text: *const u16, caption: *const u16, kind: u32) -> i32;
    }
    let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let (text, caption) = (wide(text), wide("Roads & Realms"));
    // SAFETY: two NUL-terminated UTF-16 strings that outlive the call; no owner window.
    unsafe {
        MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), 0x10);
    }
}

#[cfg(not(windows))]
fn message_box(_text: &str) {}

/// How the game draws: through OpenGL (glow), as it always has; RR_RENDERER=wgpu (DirectX 12 on
/// Windows, else Vulkan or OpenGL) when OpenGL failed (`main` starts the game again so), or to
/// test it.
fn renderer() -> eframe::Renderer {
    match std::env::var("RR_RENDERER").as_deref() {
        Ok("wgpu") => eframe::Renderer::Wgpu,
        _ => eframe::Renderer::Glow,
    }
}

/// The game once more, with wgpu, when OpenGL could not open the window (a PC with no OpenGL 2
/// driver said "egui_glow requires opengl 2.0+"; owner's Windows test, 2026-10-27): a fresh
/// process, as the window system starts once per process. Its exit code, if it ran.
fn retry_with_wgpu() -> Option<i32> {
    if std::env::var_os("RR_RENDERER").is_some() {
        return None;
    }
    let exe = std::env::current_exe().ok()?;
    let status =
        std::process::Command::new(exe).args(std::env::args_os().skip(1)).env("RR_RENDERER", "wgpu").status().ok()?;
    Some(status.code().unwrap_or(1))
}

fn main() -> eframe::Result<()> {
    // a panic, anywhere, leaves its message and place behind
    std::panic::set_hook(Box::new(|info| report_fatal(&info.to_string())));
    i18n::init_from_env();
    let args = parse_args();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Roads & Realms")
            .with_inner_size(window_size())
            // fills small laptop screens (1366x768) without running off them
            .with_maximized(std::env::var_os("RR_WINDOW").is_none())
            .with_min_inner_size([640.0, 400.0])
            // the launcher's name: the desktop matches the window to `roads-and-realms.desktop`
            .with_app_id(APP_ID)
            .with_icon(window_icon().unwrap_or_default()),
        renderer: renderer(),
        ..Default::default()
    };
    let run = eframe::run_native("Roads & Realms", options, Box::new(move |cc| Ok(Box::new(app::App::new(cc, args)))));
    // (no window at all: often no OpenGL 2 driver, a virtual machine or a remote desktop)
    if let Err(e) = &run {
        if let Some(code) = retry_with_wgpu() {
            std::process::exit(code);
        }
        let how = if matches!(renderer(), eframe::Renderer::Wgpu) { "wgpu" } else { "OpenGL" };
        report_fatal(&format!("The window could not be opened ({how}): {e}"));
    }
    run
}

#[cfg(test)]
mod tests {
    use super::*;

    /// QA round 9, #5: a mission has its own goal; the free game's are refused with it.
    #[test]
    fn missions_take_no_free_game_goals() {
        let first = Args { mission: Some("first".into()), seed: 1, ..Default::default() };
        assert!(new_game_with_replay(&first).is_ok());
        for a in [
            Args { time_limit: 1, ..first.clone() },
            Args { coins_goal: 10, ..first.clone() },
            Args { land_goal: 60, ..first.clone() },
        ] {
            assert!(new_game_with_replay(&a).is_err_and(|e| e.contains("free games")));
        }
    }

    /// Map-maker roast #13: a map's mission file names its players.
    #[test]
    fn a_map_mission_names_the_players() {
        let dir = std::env::temp_dir().join(format!("rr-mission-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let map_path = dir.join("duel.rrmap");
        let mut m = rr_core::MapData::flat(48, 48, 8);
        m.hq = vec![Some(rr_core::MapCoord::new(10, 10)), Some(rr_core::MapCoord::new(34, 34))];
        std::fs::write(&map_path, rr_formats::rrmap::write(&m)).unwrap();
        let mission = crate::missions::MapMission {
            names: vec!["Aster".into(), "Brann".into()],
            truce_minutes: 5,
            ..Default::default()
        };
        std::fs::write(crate::missions::MapMission::path_for(&map_path), mission.to_json()).unwrap();
        let args = super::Args { map: Some(map_path.to_string_lossy().into()), players: 2, ..Default::default() };
        let (sim, _) = super::new_game_with_replay(&args).unwrap();
        let names: Vec<&str> = sim.players().iter().map(|p| p.setup.name.as_str()).collect();
        assert_eq!(names, ["Aster", "Brann"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn window_size_from_env_text() {
        assert_eq!(super::parse_size("1366x700"), Some([1366.0, 700.0]));
        assert_eq!(super::parse_size("big"), None);
    }

    /// New-player roast #5: a new building's road starts at once unless switched off.
    #[test]
    fn roads_start_after_placing_by_default() {
        assert!(super::parse_args_from(std::iter::empty()).auto_road);
        assert!(super::parse_args_from(["--auto-road".to_string()].into_iter()).auto_road);
        assert!(!super::parse_args_from(["--no-auto-road".to_string()].into_iter()).auto_road);
    }

    /// The map format knows seven castle slots: asking for nine says so instead of playing seven.
    #[test]
    fn too_many_players_are_named_and_clamped() {
        let n = rr_mapgen::MAX_PLAYERS;
        assert_eq!(super::parse_args_from(["--players".to_string(), "5".to_string()].into_iter()).players, 5);
        assert_eq!(super::parse_args_from(["--players".to_string(), "9".to_string()].into_iter()).players, n);
    }
}

#[cfg(test)]
mod icon_tests {
    /// The launcher's icon is there for the window too (owner: a launcher with a nice icon).
    #[test]
    fn the_window_has_its_icon() {
        let icon = super::window_icon().expect("assets/icon/roads-and-realms-256.png");
        assert_eq!((icon.width, icon.height), (256, 256));
        assert_eq!(icon.rgba.len(), 256 * 256 * 4);
    }
}

#[cfg(test)]
mod help_tests {
    /// Map maker round 1, #11: every flag the parser knows is in --help.
    #[test]
    fn help_names_every_flag() {
        let src = include_str!("main.rs");
        let parser = &src[src.find("fn parse_args_from").unwrap()..src.find("pub fn load_map").unwrap()];
        for part in parser.split('"').skip(1).step_by(2) {
            if part.starts_with("--") {
                assert!(super::HELP.contains(part), "{part} is not in --help");
            }
        }
    }
}
