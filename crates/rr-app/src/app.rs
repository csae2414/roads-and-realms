//! The game loop: real time to game frames, input, AI players, and the screen.

use crate::Args;
use crate::render::{Camera, View};
use crate::tex::TerrainTextures;
use crate::ui::{self, Action, Ui};
use eframe::egui::{self, Key, PointerButton, Pos2, Vec2};
use rr_core::{Direction, MapCoord};
use rr_game::breaks_peace;
use rr_sim::{Cmd, Event, PlayerCommand, Sim};
use std::collections::VecDeque;
use std::time::Instant;

/// Milliseconds per game frame for the speed settings (the original's 80/60/50/40/30 ms + fast).
/// Post office categories.
pub const MSG_GENERAL: u8 = 0;
pub const MSG_MILITARY: u8 = 1;
pub const MSG_ECONOMY: u8 = 2;
pub const MSG_TIP: u8 = 3;

/// Milliseconds per game frame: 0.5×, 0.75×, 1× (normal), 1.5×, 2×, 3×, 10× and 50×.
pub const SPEEDS: [f64; 8] = [100.0, 50.0 / 0.75, 50.0, 50.0 / 1.5, 25.0, 50.0 / 3.0, 5.0, 1.0];

pub struct App {
    pub sim: Sim,
    pub args: Args,
    pub cam: Camera,
    pub tex: Option<TerrainTextures>,
    pub player: u8,
    pub speed: usize,
    pub paused: bool,
    acc: f64,
    last: Instant,
    start: Instant,
    pub show_bq: bool,
    pub show_prod: bool,
    pub show_names: bool,
    /// Why the last click of the road tool failed (shown in the road bar).
    pub road_hint: Option<String>,
    /// Fog of war instead of the original's classic exploration.
    pub fog_of_war: bool,
    /// Since when each carrier has stood idle on its road (the frame), for the idle motions
    /// (owner, 2026-10-27; drawing only, not saved).
    pub idle: std::collections::HashMap<rr_sim::FigId, u32>,
    /// What the map's look remembers: footprints, fought-over ground (`ambience::Life`).
    pub life: crate::ambience::Life,
    /// RR_FRAMES=DIR:COUNT:FPS with RR_GALLERY: the gallery frame by frame into DIR, its clock
    /// stepped 1/FPS a frame (the site's animation page, `tools/animations.sh`).
    frames: Option<FrameRun>,
    /// Beginner tips: on, and which ones were shown.
    pub tips: bool,
    /// A game was won (kept in the options).
    pub won_once: bool,
    /// The missions won (keys), kept in the settings.
    pub won_missions: Vec<String>,
    /// The best run per mission and level, kept in the settings.
    pub best: Vec<crate::settings::Best>,
    /// The keys, as the player set them (a11y: every key was fixed).
    pub keys: crate::keys::Keys,
    /// The action waiting for a new key in the options, if any.
    pub rebinding: Option<crate::keys::Act>,
    /// A save being checked and written on its own thread (perf round 3, #4: the trial load
    /// doubled the stall of every autosave, 15.9 ms on 256²).
    saving: Option<PendingSave>,
    /// Counts the games played this session (each new, loaded or joined one), so what the
    /// interface keeps for a game isn't carried into the next.
    pub game_no: u32,
    /// The building Go there went to, and when (a ring marks it for a while).
    pub mark: Option<(MapCoord, f64)>,
    pub pause_on_attack: bool,
    /// The "calm map" option (`Settings::calm_map`).
    pub calm_map: bool,
    /// A running LAN game.
    /// A LAN game (kept alive by its own thread while the window isn't drawn).
    pub net: Option<rr_net::LiveLockstep>,
    /// A LAN lobby (hosting or joining) while in the menu.
    pub lobby: Option<crate::netui::Lobby>,
    /// Waiting for other players this frame.
    pub waiting: bool,
    /// The map editor, when it is open.
    pub editor: Option<crate::editor::Editor>,
    /// The editor of the map being tried out, to go back to (with its undo history).
    pub tested: Option<Box<crate::editor::Editor>>,
    /// Size of the map view, updated every frame.
    pub screen: Vec2,
    /// Continue with a road from a new building's flag (off in the original and RTTR).
    pub auto_road: bool,
    /// Pan keys held, by position: A, D, W, S.
    pan_held: [bool; 4],
    /// The options as last written to disk, and whether `$LANG` asks for German (None: options
    /// are not kept, as in screenshot runs).
    stored: Option<(crate::settings::Settings, bool)>,
    pub ui: Ui,
    /// The post office: (frame, text, where, category `MSG_*`).
    pub messages: Messages,
    pub ais: Vec<rr_ai::Ai>,
    shot_state: u32,
    drag_from: Option<Pos2>,
    pub audio: Option<rr_audio::Audio>,
    pub in_menu: bool,
    pub menu: crate::menu::MenuState,
    pub minimap: Option<(egui::TextureHandle, u32)>,
    /// Records every command of the running game.
    pub recorder: Option<rr_sim::Replay>,
    /// Watching a replay: the recording and the next command index.
    pub playback: Option<(rr_sim::Replay, usize)>,
    /// Watching: run ahead to this frame as fast as the machine allows.
    pub replay_skip: Option<u32>,
    /// Watching: the whole map, every realm's land, buildings and people, not only what the
    /// recorded player saw (owner, 2026-10-27).
    pub replay_reveal: bool,
    /// What belongs to the game being played: replaced as a whole by a new game, a load, a
    /// network start or a replay (architect round 3, #6: reset field by field, some forgotten).
    pub session: Session,
}

/// The state of the one game being played (`App::session`), fresh for each game.
#[derive(Default)]
pub struct Session {
    pub pending: Vec<PlayerCommand>,
    pub selected: Option<MapCoord>,
    /// The settler the camera follows, until the player moves the map (genre lessons, PLAN §27,
    /// #7: watching the village).
    pub follow: Option<rr_sim::FigId>,
    pub road: Option<(MapCoord, Vec<Direction>)>,
    /// The work area to show while a building is being chosen (node, radius).
    pub work_area: Option<(MapCoord, u16)>,
    pub tips_shown: u32,
    /// The player was told that no military building fits and nothing can attack.
    pub dead_end_told: bool,
    /// When the player was last attacked (for advice that fits; new-player round 8, #3).
    pub last_attacked: Option<u32>,
    /// The last capture of ours (gf, workshops burned with it so far, where) for one message.
    pub burn_fold: Option<(u32, u32, Option<MapCoord>)>,
    /// When "nothing in reach" was last said, per kind.
    pub nothing_told: Vec<(rr_rules::BuildingType, u32)>,
    /// When the player was last told the rivals hold far more land.
    pub behind_told: u32,
    /// Fast through a quiet stretch (`App::skip_quiet`): the speed before, and from which frame
    /// news ends it.
    pub skip: Option<(usize, u32)>,
    /// Castles taken by an attack this game: (whose, by whom), for the castle goal.
    pub castles_lost: Vec<(u8, u8)>,
    /// The small stories told this game (`new_stories` bits; they travel in the save), and the
    /// campaign story's beats (`rr_game::story::beat_bit`).
    pub stories: u32,
    /// The story's deeds done this game (`rr_game::story::deed_bit`; in the save).
    pub deeds: u32,
    /// The chapter's god's favour with us, 0-100 (Concordia's; in the save).
    pub favour: u32,
    /// For the story's beats: we captured a rival's building, we lost a military one.
    pub story_captured: bool,
    pub story_lost: bool,
    /// The mission was won this game: the end screen tells its epilogue.
    pub epilogue: bool,
    /// Catapult stones in the air (drawn only; the sim decides the hit as they leave).
    pub shots: Vec<crate::render::Shot>,
    /// Where the note's news happened (for its Go there button).
    pub toast_at: Option<MapCoord>,
    /// When an attack last brought the game back to normal speed.
    pub slowed_at: Option<u32>,
    /// The player's attacks under way: (target, frame sent), to tell how they ended.
    pub my_attacks: Vec<(MapCoord, u32)>,
    /// The network game was lost (host gone, desync): it may go on against the computer.
    pub net_lost: bool,
    /// When the idle-stock tip was last given.
    pub idle_told: Option<u32>,
    /// A short note shown at the top for a few seconds (saved, loaded), and since when.
    pub toast: Option<String>,
    pub toast_since: Option<f64>,
    /// The latest alert (an attack, a loss): kept apart from notes, so "Saved" can't replace it.
    pub alert: Option<crate::ui::Alert>,
    /// Where the latest alert was (G goes there).
    pub last_alert: Option<MapCoord>,
    /// The mission's goal is met (from then on the game goes for "defeat all").
    pub goal_done: bool,
    /// The goal was just reached: the end window offers to go on.
    pub goal_reached: bool,
    /// The game paused itself for an attack (the banner says why: a11y round 3, #10).
    pub paused_by_attack: bool,
    /// The game frame of the last save or load (F9 asks when much would be lost).
    pub saved_gf: Option<u32>,
    /// F9 is waiting for a yes.
    pub ask_load: bool,
    /// The mission that came with the map file being played, if any.
    pub map_mission: Option<crate::missions::MapMission>,
    /// Messages up to this frame have been read.
    pub read_until: u32,
    /// The keyboard cursor on the map (Enter starts it, arrows move it, Enter clicks).
    pub cursor: Option<MapCoord>,
    pub game_over: Option<String>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext, args: Args) -> Self {
        let (mut sim, recorder) = crate::new_game_with_replay(&args).unwrap_or_else(|e| crate::fail(&e));
        let recorder = (args.gf == 0).then_some(recorder);
        // (the carriers' idle times kept through the run, so a screenshot shows their motions)
        let mut idle = std::collections::HashMap::new();
        let mut life = crate::ambience::Life::default();
        if args.gf > 0 {
            let mut ais: Vec<rr_ai::Ai> = (0..sim.players().len() as u8).map(rr_ai::Ai::new).collect();
            for _ in 0..args.gf {
                let mut cmds = Vec::new();
                for ai in ais.iter_mut() {
                    cmds.extend(ai.think(&sim));
                }
                sim.step(&cmds);
                track_idle(&mut idle, &sim);
                life.track(&sim, 0);
            }
        }
        let ais = make_ais(&args, sim.players().len());
        let mut cam = Camera { offset: Vec2::ZERO, zoom: args.zoom.unwrap_or(1.35) };
        let me = args.me;
        if let Some(p) = home_view(&sim, me) {
            cam.center_on(&sim, p, Vec2::new(1280.0, 760.0));
        }
        if args.focus.as_deref() == Some("ship")
            && let Some((_, s)) = sim.ships().iter().next()
        {
            let p = s.pos;
            cam.center_on(&sim, p, Vec2::new(1280.0, 760.0));
        }
        crate::ui::setup_style(&cc.egui_ctx);
        let menu = !args.explicit || args.menu;
        let view_player =
            if args.focus.as_deref() == Some("ship") { sim.ships().iter().next().map(|(_, s)| s.player) } else { None };
        let mut app = Self::assemble(sim, args, cam, ais);
        // (here, not in `assemble`: a headless app plays no sound; architect round 3, #2)
        app.audio = if app.args.screenshot.is_none() { rr_audio::Audio::new().ok() } else { None };
        app.player = me;
        app.in_menu = menu;
        app.recorder = recorder;
        app.idle = idle;
        app.life = life;
        if app.args.screenshot.is_none() {
            let s = crate::settings::Settings::load();
            app.apply_settings(&s, &cc.egui_ctx);
            app.stored = Some((s, crate::i18n::is_german()));
            if let Some(de) = app.stored.as_ref().and_then(|(s, _)| s.german) {
                crate::i18n::set_german(de);
            }
        }
        if let Some(p) = view_player {
            app.player = p;
        }
        // a game started from the command line begins as one from the menu (not a screenshot
        // run, whose view would sit under the briefing)
        if !menu && app.args.screenshot.is_none() {
            app.begin_game();
        }
        app
    }

    /// A game without a window or sound, for tests: started as from the command line.
    #[cfg(test)]
    pub(crate) fn headless(args: Args) -> Self {
        let sim = crate::new_game(&args);
        let ais = make_ais(&args, sim.players().len());
        let mut app = Self::assemble(sim, args, Camera { offset: Vec2::ZERO, zoom: 1.0 }, ais);
        app.player = app.args.me;
        app.in_menu = false;
        app.begin_game();
        app
    }

    pub fn assemble(sim: Sim, args: Args, cam: Camera, ais: Vec<rr_ai::Ai>) -> Self {
        let auto_road = args.auto_road;
        Self {
            sim,
            args,
            cam,
            tex: None,
            player: 0,
            speed: 2,
            paused: false,
            acc: 0.0,
            last: Instant::now(),
            start: Instant::now(),
            show_bq: false,
            show_prod: false,
            show_names: false,
            road_hint: None,
            fog_of_war: false,
            idle: Default::default(),
            life: Default::default(),
            frames: FrameRun::from_env(),
            editor: None,
            tested: None,
            net: None,
            tips: true,
            won_once: false,
            won_missions: Vec::new(),
            best: Vec::new(),
            keys: Default::default(),
            rebinding: None,
            saving: None,
            game_no: 0,
            mark: None,
            pause_on_attack: false,
            calm_map: false,
            lobby: None,
            waiting: false,
            screen: Vec2::new(1280.0, 760.0),
            auto_road,
            stored: None,
            pan_held: [false; 4],
            ui: Ui::default(),
            messages: VecDeque::new(),
            ais,
            shot_state: 0,
            drag_from: None,
            audio: None,
            in_menu: false,
            menu: Default::default(),
            minimap: None,
            recorder: None,
            playback: None,
            replay_skip: None,
            replay_reveal: false,
            session: Session::default(),
        }
    }

    /// Replace the running game.
    pub fn start_game(&mut self, args: Args) {
        self.save_replay();
        if let Some(n) = self.net.take() {
            let _ = n.with(|l| {
                l.leave();
                Ok(())
            });
        }
        let (sim, recorder) = match crate::new_game_with_replay(&args) {
            Ok(g) => g,
            Err(e) => {
                // back to the menu with the reason (a map file that can't be played)
                self.menu.map_msg = e;
                self.in_menu = true;
                return;
            }
        };
        self.player = args.me;
        self.recorder = Some(recorder);
        self.playback = None;
        self.ais = make_ais(&args, sim.players().len());
        if let Some(p) = home_view(&sim, args.me) {
            self.cam.center_on(&sim, p, self.screen);
        }
        self.tex = None;
        self.minimap = None;
        self.reset_game_ui();
        self.sim = sim;
        self.args = args;
        self.messages.clear();
        self.session.tips_shown = 0;
        self.session.game_over = None;
        self.session.road = None;
        self.session.selected = None;
        self.ui = Ui::default();
        self.in_menu = false;
        self.begin_game();
    }

    /// What a new game gets before its first frame, from the menu or the command line (map
    /// maker round 1, #4: `--map FILE` skipped the map's mission): the map's mission and its
    /// truce, the newcomer's military defaults, and the briefing.
    fn begin_game(&mut self) {
        self.session.map_mission =
            self.args.map.as_deref().and_then(|p| crate::missions::MapMission::load(std::path::Path::new(p)));
        // the map's mission: the computer players leave you alone for a while
        if let Some(m) = self.session.map_mission.as_ref().filter(|m| m.truce_minutes > 0) {
            for ai in &mut self.ais {
                ai.truce = Some((self.args.me, minutes(m.truce_minutes)));
            }
        }
        // until a game is won, the human's buildings with an enemy in reach (the middle) are
        // manned like the border ones (new-player round 7, #1: one soldier each by default)
        // (in a mission always: the second campaign's normal rivals hit posts of one soldier;
        // new-player round 9, #3)
        if (!self.won_once || self.args.mission.is_some()) && !self.args.auto {
            let values = self.sim.players()[self.player as usize].settings.military;
            let mission = self.args.mission.as_deref().and_then(crate::missions::mission);
            self.cmd(Cmd::SetMilitary { values: newcomer_military(values, mission) });
        }
        let briefed = self.args.mission.is_some() || self.session.map_mission.is_some();
        self.paused = briefed;
        self.ui.briefing = briefed;
    }

    pub fn cmd(&mut self, cmd: Cmd) {
        if std::env::var_os("RR_LOG").is_some() {
            eprintln!("gf {} cmd {:?}", self.sim.gf().0, cmd);
        }
        // peace first: a refused attack breaks no truce (architect round 3, #1: the truce ended
        // before the peace refused the attack)
        if breaks_peace(&cmd, self.sim.gf().0, self.peace_until()) {
            if self.args.peace == PEACE_FOR_GOOD {
                self.note(crate::i18n::tr("Peace for good: nobody attacks in this game.").into());
            } else if !matches!(cmd, Cmd::Attack { .. }) {
                let left = crate::ui::fmt_time(self.peace_until() - self.sim.gf().0);
                self.note(crate::i18n::msg("Peace time: no catapults for {1}.", &[&left]));
            } else {
                let left = crate::ui::fmt_time(self.peace_until() - self.sim.gf().0);
                self.note(crate::i18n::msg("Peace time: no attacks for {1}.", &[&left]));
            }
            return;
        }
        // an attack ends the truce with that rival and its allies (the attack panel asked first)
        if let Cmd::Attack { target, .. } = cmd
            && let Some(rival) = self.sim.building_at(target).map(|b| self.sim.building(b).player)
        {
            self.break_truce(rival);
            self.session.my_attacks.push((target, self.sim.gf().0));
        }
        // (the click when it worked comes with the sim's Placed event, the refusal with its own)
        match self.net.as_ref() {
            Some(net) => {
                let _ = net.with(|l| {
                    l.submit(cmd);
                    Ok(())
                });
            }
            None => {
                // in a pause the order waits, drawn as planned (new-player round 12, #4: the clicks
                // seemed to do nothing, so a newcomer clicked again)
                if self.paused
                    && matches!(cmd, Cmd::PlaceBuilding { .. } | Cmd::BuildRoad { .. } | Cmd::PlaceFlag { .. })
                {
                    // (the pause key as the player set it: German round 2, #10)
                    let key = crate::keys::key_name(self.keys.key(crate::keys::Act::Pause));
                    self.note(crate::i18n::fill(PLANNED_NOTE, &[key]));
                }
                self.session.pending.push(PlayerCommand { player: self.player, cmd });
            }
        }
    }

    /// The next First step's building, while the First steps show (tips on, nothing won yet).
    fn next_first_step(&self) -> Option<rr_rules::BuildingType> {
        if !self.tips || self.won_once {
            return None;
        }
        first_steps(&self.sim, self.player)
            .into_iter()
            .find(|(_, done)| !done)
            .and_then(|(s, _)| first_step_building(s))
    }

    /// Tips for a new player, each shown once, only while it still applies.
    /// Until when the computer players spare this player (a mission's truce), if they do.
    /// When a free game's peace time ends (0: none; only in games on this machine).
    pub fn peace_until(&self) -> u32 {
        if self.net.is_some() { 0 } else { peace_gf(self.args.peace) }
    }

    /// Until when the rival `p` spares this player (a mission's truce), if it still does.
    pub fn truce_with(&self, p: u8) -> Option<Truce> {
        truce_against(&self.ais, self.player, p, self.sim.gf().0).map(Truce::of)
    }

    pub fn truce_until(&self) -> Option<u32> {
        truce_end(&self.ais, self.player)
    }

    /// From minute 20, every few minutes: if the rivals hold far more land, say so and what to do
    /// (at most every 15 minutes).
    /// Under pressure: attacked in the last 10 minutes, or fewer than 5 soldiers in store.
    /// Attacked in the last 10 minutes.
    fn attacked_lately(&self) -> bool {
        let gf = self.sim.gf().0;
        self.session.last_attacked.is_some_and(|a| gf < a + minutes(10))
    }

    fn standing_check(&mut self) {
        let me = self.player as usize;
        let gf = self.sim.gf().0;
        if self.playback.is_some() || gf < minutes(20) || gf < self.session.behind_told + minutes(15) {
            return;
        }
        let by_player = crate::missions::land_by_player(&self.sim);
        let land = |p: usize| by_player[p];
        // (not those that keep the peace for good: the first valley's neighbour filled it, and
        // the messages nagged every 15 minutes; new-player round 11, #4)
        let rival_ids: Vec<u8> = crate::missions::rivals(&self.sim, me as u8)
            .into_iter()
            .filter(|p| self.truce_with(*p) != Some(Truce::ForGood))
            .collect();
        let rivals: Vec<u32> = rival_ids.iter().map(|p| land(*p as usize)).collect();
        let one = rivals.len() == 1;
        if let Some(k) = behind(land(me), &rivals) {
            self.session.behind_told = gf;
            let msg = crate::i18n::msg(
                if one { "Your rival holds {1}× your land. {2}" } else { "Your rivals hold {1}× your land. {2}" },
                &[
                    &k.to_string(),
                    standing_advice(self.attacked_lately(), self.sim.player_inventory(self.player).soldiers() < 5),
                ],
            );
            self.messages.push_front(Message::new(gf, msg, None, MSG_MILITARY));
            // and in numbers: their military buildings against ours
            let military = |p: u8| {
                self.sim
                    .buildings()
                    .iter()
                    .filter(|(_, b)| b.player == p && b.kind != rr_rules::BuildingType::Headquarters)
                    .filter(|(_, b)| self.sim.rules().building(b.kind).is_military())
                    .count()
            };
            let others: Vec<usize> = rival_ids.iter().copied().map(military).collect();
            if !others.is_empty() {
                let each = others.iter().sum::<usize>() / others.len();
                let numbers = crate::i18n::msg(
                    if one {
                        "You have {1} military buildings, your rival {2}."
                    } else {
                        "You have {1} military buildings, your rivals about {2} each."
                    },
                    &[&military(me as u8).to_string(), &each.to_string()],
                );
                self.messages.push_front(Message::new(gf, numbers, None, MSG_MILITARY));
            }
        }
    }

    /// The player attacked `rival` (or hit it with a catapult): a truce with it and its allies
    /// ends now, and a message says so.
    fn break_truce(&mut self, rival: u8) {
        if self.truce_with(rival).is_none() {
            return;
        }
        let gf = self.sim.gf().0;
        let ended = end_truce(&mut self.ais, &self.sim, self.player, rival, gf);
        let names: Vec<String> = ended.iter().map(|p| self.sim.players()[*p as usize].setup.name.clone()).collect();
        let msg = crate::i18n::msg("You broke the truce: {1} will fight back.", &[&rr_game::plain(&names.join(", "))]);
        self.messages.push_front(Message::new(gf, msg, None, MSG_GENERAL));
        // ("the truce is over" would only repeat it: new-player round 11, #7)
        self.session.tips_shown |= TRUCE_OVER_TOLD;
    }

    /// The player's attacks that ended without taking their target: told once (new-player
    /// round 11, #7: after an attack no message said how it went; a capture has its own news).
    fn attack_news(&mut self) {
        let gf = self.sim.gf().0;
        let me = self.player;
        let mut ended = Vec::new();
        self.session.my_attacks.retain(|(at, sent)| {
            if gf < sent + 200 || !attack_failed(&self.sim, me, *at) {
                return gf < sent + minutes(60);
            }
            ended.push(*at);
            false
        });
        for at in ended {
            let Some(b) = self.sim.building_at(at) else { continue };
            let bd = self.sim.building(b);
            let name = rr_game::plain(&self.sim.players()[bd.player as usize].setup.name);
            let msg = crate::i18n::msg(
                "The attack on {1}'s {2} failed: its defenders held.",
                &[&name, crate::ui::building_key(bd.kind)],
            );
            self.messages.push_front(Message::new(gf, msg, Some(at), MSG_MILITARY));
        }
    }

    /// Warn before the truce ends, and when it does (once each; the flags travel in the save).
    fn truce_news(&mut self) {
        const WARNED: u32 = TRUCE_WARNED;
        const OVER: u32 = TRUCE_OVER_TOLD;
        let Some(until) = self.truce_until() else { return };
        let gf = self.sim.gf().0;
        let (msg, flag) = if gf >= until {
            ("The truce is over: your rivals may attack now.", OVER)
        } else if gf + minutes(5) >= until {
            // advice that fits: buildings that could hold more soldiers, then the border setting
            let short = undermanned(&self.sim, self.player);
            let border = self.sim.players()[self.player as usize].settings.military[7];
            if short > 0 {
                let msg = crate::i18n::msg(undermanned_template(short), &[&short.to_string()]);
                if self.session.tips_shown & WARNED == 0 {
                    self.session.tips_shown |= WARNED;
                    // heard too, not only read (a11y round 3, #4)
                    if let Some(a) = self.audio.as_mut() {
                        a.play("news_bad", 0.7);
                    }
                    self.note(crate::i18n::show(&msg));
                    self.messages.push_front(Message::new(gf, msg, None, MSG_MILITARY));
                }
                return;
            } else {
                (truce_warning(has_posts(&self.sim, self.player), border < rr_sim::MILITARY_SCALE[7]), WARNED)
            }
        } else {
            return;
        };
        if self.session.tips_shown & flag == 0 {
            self.session.tips_shown |= flag;
            self.messages.push_front(Message::new(gf, msg.to_string(), None, MSG_MILITARY));
            if flag == OVER {
                // the end of the truce is news you hear
                if let Some(a) = self.audio.as_mut() {
                    a.play(rr_audio::TRUCE_END, 0.7);
                }
                self.note(crate::i18n::tr(msg).into());
            }
        }
    }

    /// No military building fits anywhere, none is being built, and no soldier can attack: say
    /// so once (the mission can't be won from here) instead of leaving the player wondering.
    fn dead_end_check(&mut self) {
        let me = self.player;
        if self.playback.is_some() || !self.sim.players()[me as usize].alive {
            return;
        }
        let sim = &self.sim;
        let building_one = sim
            .buildings()
            .iter()
            .any(|(_, b)| b.player == me && !b.is_ready() && rr_sim::is_frontier_military(b.kind));
        let stuck = !building_one
            && !sim.military_spot_exists(me)
            && !sim.buildings().iter().any(|(id, b)| sim.is_attackable(me, id) && sim.attackers_for(me, b.pos) > 0);
        if stuck && !self.session.dead_end_told {
            let msg = "No room is left for a military building, and none of your soldiers can attack. From here the land can't be won back: try the mission again (Main selection, Give up).";
            self.messages.push_front(Message::new(self.sim.gf().0, msg.to_string(), None, MSG_MILITARY));
        }
        self.session.dead_end_told = stuck;
    }

    fn beginner_tips(&mut self) {
        if !self.tips || self.playback.is_some() {
            return;
        }
        use rr_rules::BuildingType as B;
        let me = self.player;
        let gf = self.sim.gf().0;
        let count = |k: B| self.sim.buildings().iter().filter(|(_, b)| b.player == me && b.kind == k).count();
        let unconnected = self.sim.buildings().iter().any(|(_, b)| {
            // unconnected for a minute: not just placed with its road still being drawn
            b.player == me
                && !b.is_ready()
                && gf >= b.placed() + 1200
                && self.sim.flag(b.flag).roads.iter().all(|r| r.is_none())
        });
        let military = self
            .sim
            .buildings()
            .iter()
            .filter(|(_, b)| {
                b.player == me && b.kind != B::Headquarters && self.sim.rules().building(b.kind).is_military()
            })
            .count();
        let boards = self.sim.player_inventory(me).ware(rr_rules::Ware::Boards);
        let hq = self.sim.hq_pos(me);
        let g = self.sim.world().grid();
        let forester_near_hq = hq.is_some_and(|h| {
            self.sim
                .buildings()
                .iter()
                .any(|(_, b)| b.player == me && b.kind == B::Forester && g.distance(b.pos, h) <= 7)
        });
        let stones = self.sim.player_inventory(me).ware(rr_rules::Ware::Stones);
        let quarry_works =
            self.sim.buildings().iter().any(|(_, b)| b.player == me && b.kind == B::Quarry && b.productivity() > 0);
        let harbor = self.sim.buildings().iter().any(|(_, b)| b.player == me && b.kind == B::Harbor && b.is_ready());
        let boat_yard =
            self.sim.buildings().iter().any(|(_, b)| b.player == me && b.kind == B::Shipyard && !b.build_ships);
        let food = count(B::Fishery) + count(B::Hunter) + count(B::Bakery) + count(B::Slaughterhouse);
        // transport is the bottleneck newcomers never see coming
        let jammed = self.sim.flags().iter().any(|(_, f)| f.player == me && f.wares.len() >= 5);
        let sites = self.sim.buildings().iter().filter(|(_, b)| b.player == me && !b.is_ready()).count();
        let inv = self.sim.player_inventory(me);
        let weapons_wait = inv.ware(rr_rules::Ware::Sword) > 0
            && inv.ware(rr_rules::Ware::Shield) > 0
            && inv.ware(rr_rules::Ware::Beer) == 0
            && count(B::Brewery) == 0;
        let tips: [(bool, &str); 12] = [
            (
                gf >= 300 && count(B::Woodcutter) == 0,
                "Tip: build a woodcutter near trees. Click a spot with a house marker (Space shows where buildings fit).",
            ),
            (
                unconnected && gf >= 600,
                "Tip: a new building needs a road. Click its flag, choose 'Build road' and click your way to another flag.",
            ),
            (gf >= 3600 && count(B::Quarry) == 0, "Tip: stones come from a quarry built close to rocks."),
            (
                gf >= 6000 && count(B::Sawmill) == 0,
                "Tip: a sawmill turns wood into boards, which every building needs.",
            ),
            (gf >= 9000 && military == 0, "Tip: guardhouses and barracks near the border make your land bigger."),
            (
                gf >= minutes(15) && food == 0,
                "Tip: next, food: mines need it for coal, iron and gold. Build a fishery by the water or a hunter, later farms with a mill and a bakery.",
            ),
            (
                gf >= 6000 && stones < 8 && !quarry_works,
                "Tip: stones are running low. Build a quarry next to rocks, or a granite mine in the mountains (send a geologist first).",
            ),
            (
                harbor && boat_yard,
                "Tip: your shipyard builds boats for river roads. Switch it to Ships in its window to sail to other shores.",
            ),
            (
                forester_near_hq,
                "Tip: a forester plants trees on all free land around it. Keep foresters away from where you still want to build.",
            ),
            (
                gf >= 6000 && boards < 4,
                "Tip: boards are running low. More woodcutters, foresters and a second sawmill help.",
            ),
            (
                gf >= 3000 && jammed,
                "Tip: wares pile up at a flag: its roads are too long or too busy. Put more flags on long roads (each part gets its own carrier), or build a second road.",
            ),
            // new-player round 8, #6: 17 swords and 16 shields and not one new soldier
            (
                weapons_wait,
                "Tip: swords and shields wait, but there is no beer: a brewery (grain and water) turns them into soldiers.",
            ),
        ];
        // "build a ..." tips are for players who never built one, not for one who just lost it:
        // woodcutter (0), quarry (2), sawmill (3), guardhouse (4)
        for (bit, had) in
            [(0, count(B::Woodcutter) > 0), (2, count(B::Quarry) > 0), (3, count(B::Sawmill) > 0), (4, military > 0)]
        {
            if had {
                self.session.tips_shown |= 1 << bit;
            }
        }
        // material lying idle: again every 20 minutes while it does, with numbers (new-player
        // round 7, #7: one tip at minute 5, then silence for two hours)
        if idle_stock(boards, stones, sites) && idle_tip_due(gf, self.session.idle_told) {
            self.session.idle_told = Some(gf);
            let def = self.sim.rules().building(B::Guardhouse);
            let n = (boards / def.boards.max(1) as u32).min(stones / def.stones.max(1) as u32);
            // (no guardhouses while no soldier waits to man one: round 8, #3)
            // (with fewer than 5 soldiers waiting, no guardhouses: they would stand empty; round 9)
            let template = if self.sim.player_inventory(me).soldiers() < 5 {
                "{1} boards and {2} stones lie in store and nothing is being built. No soldier waits to man a new military building: brew beer, make swords and shields, and build workshops meanwhile."
            } else {
                "{1} boards and {2} stones lie in store and nothing is being built: enough for {3} guardhouses. Place more buildings, military ones at the border first."
            };
            let msg = crate::i18n::msg(template, &[&boards.to_string(), &stones.to_string(), &n.to_string()]);
            self.note(crate::i18n::show(&msg));
            self.messages.push_front(Message::new(gf, msg, None, MSG_TIP));
            return;
        }
        const { assert!(LIST_TIPS < 32 && CASTLE_TIP == 1 << LIST_TIPS) };
        debug_assert!(tips.len() <= LIST_TIPS, "the tips list runs into the named bits");
        for (i, (due, text)) in tips.iter().enumerate() {
            if *due && self.session.tips_shown & (1 << i) == 0 {
                self.session.tips_shown |= 1 << i;
                let msg = crate::i18n::msg("💡 {1}", &[text]);
                // shown at the top too, not only in the list
                self.note(crate::i18n::show(&msg));
                self.messages.push_front(Message::new(gf, msg, None, MSG_TIP));
                // one at a time
                return;
            }
        }
    }

    /// Leave a running network game (the others play on).
    pub fn leave_net(&mut self) {
        if let Some(n) = self.net.take() {
            let _ = n.with(|l| {
                l.leave();
                Ok(())
            });
        }
    }

    /// Start a LAN game: everybody got the same map and setup; `me` is this machine's player.
    pub fn start_net_game(
        &mut self,
        map: rr_core::MapData,
        setup: rr_sim::GameSetup,
        ais: Vec<(u8, u8)>,
        me: u8,
        net: rr_net::Lockstep,
    ) {
        self.save_replay();
        // what the host sent must make sense before anything is built from it
        let n = setup.players.len();
        let valid = (me as usize) < n
            && map.players() >= n
            && ais.iter().all(|(p, _)| (*p as usize) < n && *p != me)
            && ais.iter().map(|a| a.0).collect::<std::collections::BTreeSet<_>>().len() == ais.len();
        if !valid {
            let mut net = net;
            net.leave();
            self.menu.net_msg = crate::i18n::tr("The host sent a game that can't be played.").into();
            return;
        }
        let rules = std::sync::Arc::new(rr_rules::Rules::classic());
        let sim = match Sim::new(rules.clone(), &map, &setup) {
            Ok(s) => s,
            Err(e) => {
                // say goodbye, so the others don't wait for us (net round 4, #7), and say why
                // where it is seen: the menu
                let mut net = net;
                net.leave();
                self.menu.net_msg = format!("{} {e}", crate::i18n::tr("The host sent a game that can't be played."));
                return;
            }
        };
        self.recorder = Some(rr_sim::Replay::new(&rules, &map, &setup));
        self.playback = None;
        self.ais = ais.iter().map(|(p, l)| rr_ai::Ai::with_level(*p, crate::level(*l))).collect();
        self.player = me;
        if let Some(p) = home_view(&sim, me) {
            self.cam.center_on(&sim, p, self.screen);
        }
        self.tex = None;
        self.minimap = None;
        self.reset_game_ui();
        self.sim = sim;
        self.args = Args::default();
        // (no mission of an earlier game: net round 4, #4, one peer automated its harbors)
        self.session.map_mission = None;
        self.net = Some(rr_net::LiveLockstep::new(net));
        self.messages.clear();
        self.session.game_over = None;
        self.session.road = None;
        self.session.selected = None;
        self.ui = Ui::default();
        self.in_menu = false;
        self.paused = false;
    }

    fn advance(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last).as_secs_f64().min(0.25);
        self.last = now;
        // a network game never pauses and goes on after this player's game is over (the
        // others are still playing)
        if self.net.is_none() && (self.paused || self.session.game_over.is_some()) {
            return;
        }
        if self.replay_skip.is_some() {
            self.replay_run_ahead(now);
            return;
        }
        self.acc += dt * 1000.0 / SPEEDS[self.speed];
        // a network game far behind the others (its window was hidden) runs without its pace
        // until it has caught up, within 30 ms a frame; the others wait for it meanwhile (net
        // round 4, #1: it replayed the gap at normal speed)
        let gf0 = self.sim.gf().0;
        let ahead = self.net.as_ref().and_then(|n| n.with(|l| Ok(l.queued_ahead(gf0))).ok()).unwrap_or(0);
        let catching_up = catch_up(ahead);
        if catching_up {
            self.acc = self.acc.max(f64::from(ahead * rr_net::TURN_GF));
        }
        let mut steps = 0;
        self.waiting = false;
        while self.acc >= 1.0 && steps < if catching_up { 20_000 } else { 400 } {
            if catching_up && now.elapsed().as_millis() >= 30 {
                break;
            }
            self.acc -= 1.0;
            steps += 1;
            if !self.step_frame() {
                break;
            }
        }
        if steps == 400 {
            self.acc = 0.0;
        }
        self.quiet_check();
    }

    /// One game frame: its commands, the step, the news. False when this screen frame stops
    /// here: waiting for the network, the network game ended, or an attack paused the game.
    fn step_frame(&mut self) -> bool {
        let gf = self.sim.gf().0;
        let cmds = if self.net.is_some() {
            match self.net_commands(gf) {
                Some(cmds) => cmds,
                None => return false,
            }
        } else if self.playback.is_some() {
            self.replay_commands(gf)
        } else {
            self.local_commands(gf)
        };
        let out = self.sim.step(&cmds);
        self.handle_events(out.events);
        // an attack that paused the game stops it here, not after the frame's other steps
        // (QA round 7, #8: up to 12.5 s of game time at top speed)
        if stops_the_frame(self.paused, self.net.is_some()) {
            self.acc = 0.0;
            return false;
        }
        if autosave_due(self.sim.gf().0) && self.playback.is_none() && self.session.game_over.is_none() {
            if self.net.is_none() {
                self.save_to(&Self::autosave_path());
            } else {
                self.autosave_lan();
            }
        }
        self.periodic_news();
        true
    }

    /// A LAN game's commands for frame `gf`: everybody's from the network, the computer
    /// players' from here as on every other machine. `None`: none yet (waiting for the others)
    /// or the network game ended.
    fn net_commands(&mut self, gf: u32) -> Option<Vec<PlayerCommand>> {
        let net = self.net.as_ref()?;
        let sim = &self.sim;
        let (res, left, desynced) = match net.with(|l| {
            Ok((l.commands_for(gf, || sim.sync_hash()), std::mem::take(&mut l.left), std::mem::take(&mut l.desynced)))
        }) {
            Ok(x) => x,
            Err(e) => (Err(e), Vec::new(), Vec::new()),
        };
        // a desync is not a quit: say so, and leave a report to compare
        // (the host, which compared the games, leaves a report; a guest only hears of
        // another's: QA round 7, #9)
        let host = net.with(|l| Ok(l.is_host())).unwrap_or(false);
        for p in &desynced {
            let report = if host {
                write_desync_report(&Self::desync_dir(), &self.sim, self.player)
                    .map(|p| p.display().to_string())
                    .unwrap_or_default()
            } else {
                String::new()
            };
            let msg = format!("{} {} {report}", crate::i18n::tr(desync_line(host)), p + 1);
            self.messages.push_front(Message::new(gf, msg, None, MSG_GENERAL));
        }
        for p in left.into_iter().filter(|p| !desynced.contains(p)) {
            // (by name: "a player left the game: 2" said little)
            let name = self.sim.players().get(p as usize).map_or_else(|| (p + 1).to_string(), |x| x.setup.name.clone());
            let msg = format!("{} {name}", crate::i18n::tr("A player left the game:"));
            self.messages.push_front(Message::new(gf, msg, None, MSG_GENERAL));
        }
        match res {
            Ok(Some(mut cmds)) => {
                for ai in self.ais.iter_mut() {
                    cmds.extend(ai.think(&self.sim));
                }
                if let Some(r) = self.recorder.as_mut() {
                    r.record(gf, &cmds);
                }
                Some(cmds)
            }
            Ok(None) => {
                // waiting for the other players
                self.acc = self.acc.min(1.0);
                self.waiting = true;
                None
            }
            Err(e) => {
                // after a desync, a report to compare with the host's
                let e = if e.contains("apart")
                    && let Ok(p) = write_desync_report(&Self::desync_dir(), &self.sim, self.player)
                {
                    format!("{e} ({} {})", crate::i18n::tr("report:"), p.display())
                } else {
                    e
                };
                self.session.game_over = Some(format!("{} {e}", crate::i18n::tr("Network game ended:")));
                self.net = None;
                // the game here is whole: it can go on against the computer
                self.session.net_lost = true;
                None
            }
        }
    }

    /// Watching: the recorded commands of frame `gf` (input and AIs are ignored).
    fn replay_commands(&mut self, gf: u32) -> Vec<PlayerCommand> {
        let Some((rep, i)) = self.playback.as_mut() else { return Vec::new() };
        self.session.pending.clear();
        let start = *i;
        while *i < rep.commands.len() && rep.commands[*i].0 == gf {
            *i += 1;
        }
        if gf >= rep.final_gf {
            self.paused = true;
            if !rep.verify(&self.sim) {
                self.session.toast = Some(crate::i18n::tr(REPLAY_DIFFERS).into());
                self.session.toast_since = None;
            }
        }
        rep.commands[start..*i].iter().map(|c| c.1.clone()).collect()
    }

    /// A game on this machine: the player's commands and the AIs', none attacking in peace time.
    fn local_commands(&mut self, gf: u32) -> Vec<PlayerCommand> {
        let mut cmds = std::mem::take(&mut self.session.pending);
        for ai in self.ais.iter_mut() {
            cmds.extend(ai.think(&self.sim));
        }
        // peace time: nobody's attack goes out (game design round 1, #6)
        let peace = self.peace_until();
        cmds.retain(|c| !breaks_peace(&c.cmd, gf, peace));
        if let Some(r) = self.recorder.as_mut() {
            r.record(gf, &cmds);
        }
        cmds
    }

    /// The news and checks that look at the game now and then.
    fn periodic_news(&mut self) {
        if self.sim.gf().0.is_multiple_of(100) {
            self.beginner_tips();
            self.attack_news();
        }
        if self.sim.gf().0.is_multiple_of(600) {
            self.dead_end_check();
        }
        if self.sim.gf().0.is_multiple_of(100) {
            self.truce_news();
            self.story_news();
        }
        if self.sim.gf().0.is_multiple_of(3600) {
            self.standing_check();
        }
    }

    /// Fast through a quiet stretch: top speed until the first news or a pause, then the speed
    /// of before (genre lessons, PLAN §27, #8: big maps without pacing drag).
    pub fn skip_quiet(&mut self) {
        let prev = self.session.skip.map_or(self.speed, |(prev, _)| prev);
        self.session.skip = Some((prev, self.sim.gf().0));
        self.speed = SPEEDS.len() - 1;
    }

    /// Ends a fast stretch at military news (an attack, a loss, the truce), a pause or the
    /// game's end, or when the player picked a speed.
    pub fn quiet_check(&mut self) {
        let Some((prev, from)) = self.session.skip else { return };
        // (military news only: "finished", finds and tips ended it within seconds in a growing
        // realm; veteran round 12, #6)
        let news = self.messages.iter().take_while(|m| m.gf > from).any(|m| m.kind == MSG_MILITARY);
        let chosen = self.speed != SPEEDS.len() - 1;
        if news || chosen || self.paused || self.session.game_over.is_some() {
            if !chosen {
                self.speed = prev;
            }
            self.session.skip = None;
        }
    }

    /// A castle fell: kept for the castle goal; if it was the goal's and another realm took it,
    /// the player hears that the mission now asks for all (new-player round 12, #7).
    fn castle_razed(&mut self, player: u8, by: u8) {
        self.session.castles_lost.push((player, by));
        if let Some(crate::missions::Objective::DefeatRival(p)) = self.objective()
            && p == player
            && !self.sim.are_allies(by, self.player)
        {
            let name = |q: u8| rr_game::plain(&self.sim.players()[q as usize].setup.name);
            let msg = crate::i18n::msg(CASTLE_TAKEN_BY_OTHERS, &[&name(player), &name(by)]);
            self.note(crate::i18n::show(&msg));
            self.messages.push_front(Message::new(self.sim.gf().0, msg, None, MSG_MILITARY));
        }
    }

    /// Stones thrown this frame that we can see leave or land, for the map to draw in flight
    /// (playtest 2026-10-27: the catapult threw, but nothing flew).
    fn catapult_shots(&mut self, events: &[Event]) {
        let gf = self.sim.gf().0;
        self.session.shots.retain(|s| !s.done(gf));
        let (me, fog) = (self.player, self.fog_of_war);
        for e in events {
            if let Event::CatapultThrow { from, to, hit } = *e
                && [from, to].iter().any(|p| rr_audio::audible(self.sim.visibility(me, *p), fog))
            {
                self.session.shots.push(crate::render::Shot { from, to, hit, gf });
            }
        }
    }

    /// What the campaign story's beats wait for in this frame's events: a capture of ours, a
    /// military building of ours lost.
    fn story_facts(&mut self, events: &[Event]) {
        for e in events {
            if let Event::Offered { player, n, .. } = e
                && *player == self.player
            {
                self.session.favour = (self.session.favour + OFFER_FAVOUR * n).min(100);
            }
        }
        let me = self.player;
        for e in events {
            match e {
                Event::BuildingCaptured { by, .. } if *by == me => self.session.story_captured = true,
                Event::BuildingCaptured { player, .. } if *player == me => self.session.story_lost = true,
                Event::Razed { player, .. } if *player == me => self.session.story_lost = true,
                Event::BuildingLost { player, kind, .. }
                    if *player == me && self.sim.rules().building(*kind).is_military() =>
                {
                    self.session.story_lost = true
                }
                _ => {}
            }
        }
    }

    /// The campaign story's beats due now, in the news and as a note (story round 1: the
    /// missions had briefings, but no story).
    fn tale_news(&mut self) {
        let Some(tale) = self.args.mission.as_deref().and_then(rr_game::story::tale) else { return };
        let (sim, me) = (&self.sim, self.player);
        let rivals = (0..sim.players().len() as u8)
            .filter(|p| {
                *p != me
                    && sim
                        .buildings()
                        .iter()
                        .any(|(_, b)| b.player == *p && sim.visibility(me, b.pos) == rr_sim::Visibility::Visible)
            })
            .collect();
        let seen = rr_game::story::Seen {
            minute: sim.gf().0 / MINUTE,
            rivals,
            captured: self.session.story_captured,
            lost: self.session.story_lost,
            minted: coins_minted(sim).get(me as usize).copied().unwrap_or(0),
            ship: sim.ships().iter().any(|(_, s)| s.player == me),
            holy: sim
                .world()
                .holy_places()
                .iter()
                .any(|(p, _)| sim.world().holy_holder(*p) == rr_world::Holder::Player(me)),
            favour: self.session.favour,
        };
        // Concordia's favour grows where the lands meet at her shrine
        let near = rr_game::story::concordia_near(sim, me) || rr_game::story::mars_drill(sim, me);
        self.session.favour = rr_game::story::favour_step(self.session.favour, near, sim.gf().0);
        let deeds = rr_game::story::due_deeds(tale, &seen, self.session.deeds);
        for (bit, line) in rr_game::story::due(tale, &seen, self.session.stories) {
            self.session.stories |= bit;
            self.note(crate::i18n::tr(line).into());
            self.messages.push_front(Message::new(self.sim.gf().0, line.into(), None, MSG_GENERAL));
        }
        for (bit, deed) in deeds {
            self.session.deeds |= bit;
            self.do_deed(deed);
        }
    }

    /// What a story's deed does in the game (`rr_game::story::Deed`).
    fn do_deed(&mut self, deed: rr_game::story::Deed) {
        use rr_game::story::Deed;
        match deed {
            Deed::HandOver { to, line } => {
                if let Some(at) = rr_game::story::hand_over_spot(&self.sim, self.player, to) {
                    self.cmd(Cmd::HandOver { at, to });
                    self.note(crate::i18n::tr(line).into());
                    self.messages.push_front(Message::new(self.sim.gf().0, line.into(), Some(at), MSG_GENERAL));
                }
            }
            Deed::Bless { n, recruit, line } => {
                if let Some(at) = rr_game::story::held_holy_place(&self.sim, self.player) {
                    self.cmd(Cmd::Bless { at, n, recruit });
                    self.note(crate::i18n::tr(line).into());
                    self.messages.push_front(Message::new(self.sim.gf().0, line.into(), Some(at), MSG_GENERAL));
                }
            }
            Deed::War { with } => {
                for ai in self.ais.iter_mut().filter(|a| a.player == with) {
                    ai.truce = None;
                }
            }
        }
    }

    /// Small stories in the news, each once a game (genre lessons, PLAN §27, #7: watching the
    /// village): the first coin, the first general, the first ship.
    fn story_news(&mut self) {
        if self.playback.is_none() {
            self.tale_news();
        }
        if self.playback.is_some() || self.session.stories & 0b111 == 0b111 {
            return;
        }
        let (sim, me) = (&self.sim, self.player);
        let general = rr_rules::Job::General;
        let now = Stories {
            coin: coins_minted(sim).get(me as usize).is_some_and(|c| *c > 0),
            general: sim.player_inventory(me).job(general) > 0
                || sim
                    .buildings()
                    .iter()
                    .any(|(_, b)| b.player == me && b.soldiers.iter().any(|s| sim.figure(*s).job == general)),
            ship: sim.ships().iter().any(|(_, s)| s.player == me),
        };
        for (bit, text, kind) in new_stories(&now, self.session.stories) {
            self.session.stories |= bit;
            self.note(crate::i18n::tr(text).into());
            self.messages.push_front(Message::new(self.sim.gf().0, text.into(), None, kind));
        }
    }

    /// Watching a replay: step on towards `replay_skip` for at most 30 ms this frame (the screen
    /// keeps moving), without sounds or messages on the way.
    fn replay_run_ahead(&mut self, start: Instant) {
        let Some(target) = self.replay_skip else { return };
        let Some((rep, i)) = self.playback.as_mut() else {
            self.replay_skip = None;
            return;
        };
        // (a replay saved before its end doesn't know where it ends: 0)
        let end = if rep.final_gf > 0 { target.min(rep.final_gf) } else { target };
        while self.sim.gf().0 < end && start.elapsed().as_millis() < 30 {
            let gf = self.sim.gf().0;
            let from = *i;
            while *i < rep.commands.len() && rep.commands[*i].0 == gf {
                *i += 1;
            }
            let cmds: Vec<_> = rep.commands[from..*i].iter().map(|c| c.1.clone()).collect();
            self.sim.step(&cmds);
        }
        if self.sim.gf().0 >= end {
            self.replay_skip = None;
            self.minimap = None;
            if rep.final_gf > 0 && end >= rep.final_gf {
                self.paused = true;
                if !rep.verify(&self.sim) {
                    self.session.toast = Some(crate::i18n::tr(REPLAY_DIFFERS).into());
                    self.session.toast_since = None;
                }
            }
        }
    }

    fn handle_events(&mut self, events: Vec<Event>) {
        let me = self.player;
        // (land lost with a capture of ours is told with the capture)
        let captured_now = events.iter().any(|e| matches!(e, Event::BuildingCaptured { player, .. } if *player == me));
        self.story_facts(&events);
        self.catapult_shots(&events);
        // workshops that burn with land a rival took in the same frame: one line with the capture,
        // not a "Well lost" each (new-player round 8, #2)
        for e in events {
            if let Event::Razed { player, kind: rr_rules::BuildingType::Headquarters, by, .. } = e {
                self.castle_razed(player, by);
            }
            // our catapult hitting a rival under truce breaks it, as an attack does (veteran
            // round 11, #2: the peaceful rivals were shot empty and never answered)
            if let Event::CatapultHit { player, by, .. } = &e
                && *by == me
            {
                self.break_truce(*player);
            }
            // workshops burning with land a rival just took: one line with the capture, counted
            // up (they burn a frame or more after it; new-player rounds 8 and 9, #2/#6)
            if let Event::BuildingLost { player, kind, .. } = &e
                && *player == me
                && !self.sim.rules().building(*kind).is_military()
                && let Some((at_gf, n, at)) = self.session.burn_fold
                && in_burn_window(at_gf, self.sim.gf().0)
            {
                self.session.burn_fold = Some((at_gf, n + 1, at));
                let line = crate::i18n::msg(burned_template(n + 1), &[&(n + 1).to_string()]);
                self.messages.retain(|m| !(m.gf == at_gf && is_burned_line(&m.text)));
                self.messages.push_front(Message::new(at_gf, line, at, MSG_MILITARY));
                continue;
            }
            if std::env::var_os("RR_LOG").is_some() && !matches!(e, Event::Sound { .. }) {
                eprintln!("gf {} event {:?}", self.sim.gf().0, e);
            }
            // (work sounds only from land we may hear: not from the unexplored)
            let heard = match &e {
                Event::Sound { at, .. } => rr_audio::audible(self.sim.visibility(me, *at), self.fog_of_war),
                Event::BuildingFinished { player, at, .. } if *player != me => {
                    rr_audio::audible(self.sim.visibility(me, *at), self.fog_of_war)
                }
                _ => true,
            };
            if heard && let Some(a) = self.audio.as_mut() {
                a.on_event(&e, me);
            }
            let kind = match &e {
                Event::UnderAttack { .. }
                | Event::BuildingCaptured { .. }
                | Event::PlayerDefeated { .. }
                | Event::LandLost { .. }
                | Event::CatapultHit { .. } => MSG_MILITARY,
                Event::BuildingLost { kind, .. } if self.sim.rules().building(*kind).is_military() => MSG_MILITARY,
                Event::CommandRejected { .. } => MSG_GENERAL,
                _ => MSG_ECONOMY,
            };
            let (txt, at) = match &e {
                Event::CatapultHit { player, at, .. } if *player == me => {
                    // the first hit of a minute as a toast with Go there (new-player round 8,
                    // #8: hits only reached the list, so nobody saw where the catapult stood)
                    let gf = self.sim.gf().0;
                    if self.session.last_attacked.is_none_or(|a| gf >= a + MINUTE) {
                        self.session.last_attacked = Some(gf);
                        self.alert("A catapult is firing upon us!".to_string(), *at, true);
                    }
                    ("A catapult is firing upon us!".to_string(), Some(*at))
                }
                Event::BuildingFinished { player, at, kind } if *player == me => {
                    (self.finished_news(*at, *kind), Some(*at))
                }
                Event::BuildingLost { player, at, kind } if *player == me => (self.lost_news(*at, *kind), Some(*at)),
                Event::UnderAttack { player, at, .. } if *player == me => {
                    self.session.last_attacked = Some(self.sim.gf().0);
                    let txt = military_news(&self.sim, me, &e).unwrap_or_else(|| "We are under attack!".into());
                    self.alert(txt.clone(), *at, true);
                    (txt, Some(*at))
                }
                Event::MineExhausted { player, at } if *player == me => ("A mine is exhausted".to_string(), Some(*at)),
                Event::MineRunningLow { player, at } if *player == me => {
                    ("A mine is running low".to_string(), Some(*at))
                }
                Event::GeologistFound { player, at, what } if *player == me => {
                    let txt = match ui::resource_key(*what) {
                        (k, Some(a)) => crate::i18n::msg("Geologist found {1} ({2})", &[k, &a.to_string()]),
                        (k, None) => crate::i18n::msg("Geologist found {1}", &[k]),
                    };
                    (txt, Some(*at))
                }
                Event::BuildingCaptured { player, at, by } if *player == me || *by == me => {
                    // the first loss: what the castle's soldiers do and don't (new-player round
                    // 8, #2: 41 sat in the castle while four guardhouses fell next to it)
                    if *player == me && self.session.tips_shown & CASTLE_TIP == 0 {
                        self.session.tips_shown |= CASTLE_TIP;
                        if let Some(tip) = castle_tip(&self.sim, me) {
                            self.messages.push_front(Message::new(self.sim.gf().0, tip, None, MSG_TIP));
                        }
                    }
                    let txt = military_news(&self.sim, me, &e).unwrap_or_else(|| {
                        if *by == me { "We captured a building!" } else { "A building was captured by the enemy" }
                            .into()
                    });
                    self.alert(txt.clone(), *at, false);
                    (txt, Some(*at))
                }
                // land a neighbour's new post took, even with nothing on it (veteran round 12, #4:
                // only a burned building was told; RTTR: "Lost land by this building")
                Event::LandLost { player, to, nodes, at } if *player == me && !captured_now => {
                    let who = rr_game::plain(&self.sim.players()[*to as usize].setup.name);
                    (crate::i18n::msg(LAND_LOST, &[&nodes.to_string(), &who]), Some(*at))
                }
                Event::PlayerDefeated { player } => {
                    if *player == me {
                        self.session.game_over = Some(crate::i18n::tr("You have been defeated.").into());
                    }
                    (
                        crate::i18n::msg(
                            "{1} has been defeated",
                            &[&rr_game::plain(&self.sim.players()[*player as usize].setup.name)],
                        ),
                        None,
                    )
                }
                // (an answer to the player's own click: not when a replay repeats the click)
                Event::CommandRejected { player, reason, .. } if *player == me && self.playback.is_none() => {
                    let why = match reason {
                        rr_sim::RejectReason::NotYourTerritory => "not your land",
                        rr_sim::RejectReason::NotBuildable => "no room to build there",
                        rr_sim::RejectReason::NoFlag => "there is no flag",
                        rr_sim::RejectReason::BadRoad => "the road can't go there",
                        rr_sim::RejectReason::NotFound => "nothing there to do it with",
                        rr_sim::RejectReason::NotAllowed => "not allowed",
                    };
                    (crate::i18n::msg("Not possible: {1}", &[why]), None)
                }
                Event::NothingInReach { player, at, kind } if *player == me => {
                    // once per kind in 10 minutes (new-player round 9, #9: two farms said "no room
                    // for fields" 29 times in 90 minutes and pushed the military news away)
                    let gf = self.sim.gf().0;
                    let last = self.session.nothing_told.iter().find(|(k, _)| k == kind).map(|(_, t)| *t);
                    if !nothing_due(last, gf) {
                        continue;
                    }
                    self.session.nothing_told.retain(|(k, _)| k != kind);
                    self.session.nothing_told.push((*kind, gf));
                    (crate::i18n::msg("{1}: {2}", &[ui::building_key(*kind), ui::idle_reason(*kind)]), Some(*at))
                }
                Event::NoResources { player, at } if *player == me => {
                    ("The expedition found the harbor spot taken".to_string(), Some(*at))
                }
                _ => continue,
            };
            if matches!(&e, Event::BuildingCaptured { player, .. } if *player == me) {
                self.session.burn_fold = Some((self.sim.gf().0, 0, at));
            }
            // news without a sound of its own gets a chime, only with a message shown (sound
            // designer 2 #2: the chime rang for news the throttle had dropped)
            if let Some((key, v)) = rr_audio::chime_for(&e, me)
                && let Some(a) = self.audio.as_mut()
            {
                a.play(key, v);
            }
            // a refusal is seen, not only heard (a11y round 2, #6)
            self.note_refusal(&e, me, &txt);
            self.messages.push_front(Message::new(self.sim.gf().0, txt, at, kind));
            self.messages.truncate(80);
        }
        self.check_outcome(me);
    }

    /// A mission is won: ticked in the menu, and its time and score kept if they are the best
    /// at this level (game design round 1, #4).
    fn mission_won(&mut self) {
        let Some(key) = self.args.mission.clone() else { return };
        self.session.epilogue = true;
        // a gentle chapter ends in peace, for good (the Sabines: the women stood between)
        if crate::missions::mission(&key).is_some_and(|m| m.gentle) {
            let me = self.player;
            for ai in self.ais.iter_mut() {
                ai.truce = Some((me, u32::MAX));
            }
        }
        if !self.won_missions.contains(&key) {
            self.won_missions.push(key.clone());
        }
        let me = self.player;
        let share = crate::missions::land_share(&self.sim, me);
        let players = self.sim.players();
        let vanquished =
            (0..players.len()).filter(|p| !players[*p].alive && !self.sim.are_allies(*p as u8, me)).count() as u32;
        let minutes = self.sim.gf().0 / MINUTE;
        let score = crate::settings::score(share, vanquished, minutes);
        let b = crate::settings::Best { mission: key, level: self.args.ai_level.min(2), minutes, score };
        if crate::settings::record_best(&mut self.best, b) {
            let msg = crate::i18n::msg(
                "Your best on this mission: {1} minutes, score {2}.",
                &[&minutes.to_string(), &score.to_string()],
            );
            self.messages.push_front(Message::new(self.sim.gf().0, msg, None, MSG_GENERAL));
        }
    }

    /// The goal of this game: a campaign mission's, or a map mission's (map maker round 1, #2).
    pub fn objective(&self) -> Option<crate::missions::Objective> {
        match self.args.mission.as_deref().and_then(crate::missions::mission) {
            // (Concordia's favour at 50: one building fewer to take; never none)
            Some(m) => Some(match m.objective {
                crate::missions::Objective::Vanquish(n) if self.session.favour >= FAVOUR_EASIER => {
                    crate::missions::Objective::Vanquish(n.saturating_sub(1).max(1))
                }
                o => o,
            }),
            None => self.session.map_mission.as_ref().and_then(|m| m.objective),
        }
    }

    /// Whether a goal ends the game this frame (`outcome`), and what that sets: the end screen,
    /// the sound, "won once", a mission ticked.
    /// The goal still to play for: the objective, unless it is a castle another realm took (then
    /// defeating all is left; new-player round 12, #7).
    pub fn goal(&self) -> Option<crate::missions::Objective> {
        let me = self.player;
        self.objective()
            .filter(|o| castle_goal_ours(Some(*o), &self.session.castles_lost, |p| self.sim.are_allies(p, me)))
    }

    fn check_outcome(&mut self, me: u8) {
        if self.session.game_over.is_some() || self.playback.is_some() {
            return;
        }
        let Some(o) = outcome(&self.sim, me, &self.args, self.goal(), self.session.goal_done) else { return };
        self.session.game_over = Some(o.text);
        if o.kind == Ends::Goal {
            (self.session.goal_done, self.session.goal_reached) = (true, true);
        }
        if let Ends::Goal | Ends::Won { .. } = o.kind {
            if let Some(a) = self.audio.as_mut() {
                a.play("victory", 0.9);
            }
            self.won_once = true;
        }
        if let Ends::Goal | Ends::Won { mission: true } = o.kind {
            self.mission_won();
        }
    }

    /// The whole map is shown, without fog or clouds: in the editor, a replay with "show all",
    /// or a headless screenshot with RR_REVEAL.
    pub fn sees_all(&self) -> bool {
        self.editor.is_some()
            || (self.playback.is_some() && self.replay_reveal)
            || std::env::var_os("RR_REVEAL").is_some()
    }

    /// What the player sees of `p` (all of it, when `sees_all`).
    pub fn visibility(&self, p: MapCoord) -> rr_sim::Visibility {
        if self.sees_all() { rr_sim::Visibility::Visible } else { self.sim.visibility(self.player, p) }
    }

    /// A small live view of the map around `at`, in `rect` (the building window's picture in
    /// picture; playtest 2026-10-27).
    pub(crate) fn inset_view(&self, rect: egui::Rect, at: MapCoord) -> View<'_> {
        let mut view = self.view(rect, self.start.elapsed().as_secs_f32());
        // the building a little below the middle: its picture rises above its node
        let centre = crate::render::world_pos_unwrapped(&self.sim, at.x as i32, at.y as i32) - Vec2::new(0.0, 22.0);
        view.cam = Camera { offset: centre - rect.size() / 2.0, zoom: 1.0 };
        (view.show_bq, view.road_preview, view.work_area, view.selected) = (false, None, None, None);
        view.planned.clear();
        view
    }

    /// RR_OPEN's windows (headless screenshots, the manual's pictures): a window by name,
    /// `stats:N` a statistics tab, `hq` our castle's window, `kind:Sawmill` our first building of
    /// a kind, `prio:Sawmill` gives it priority, `build` the build window at a free spot near
    /// the castle, `bq` / `prod` / `names`
    /// the overlays, `play` closes the briefing.
    fn debug_open(&mut self, v: &str) {
        let (name, arg) = v.split_once(':').unwrap_or((v, ""));
        let me = self.player;
        let first = |sim: &Sim, f: &dyn Fn(rr_rules::BuildingType) -> bool| {
            sim.buildings().iter().find(|(_, b)| b.player == me && b.is_ready() && f(b.kind)).map(|(_, b)| b.pos)
        };
        let hq = first(&self.sim, &|k| k == rr_rules::BuildingType::Headquarters);
        match name {
            "stats" => {
                self.ui.stats = true;
                self.ui.stats_cat = arg.parse().unwrap_or(0);
            }
            "inventory" => self.ui.inventory = true,
            "buildings" => self.ui.buildings = true,
            "map" => self.ui.map = true,
            "options" => self.ui.options = true,
            "main" => self.ui.main = true,
            "saves" => self.ui.saves = true,
            "distribution" => self.ui.distribution = true,
            "transport" => self.ui.transport = true,
            "military" => self.ui.military = true,
            "tools" => self.ui.tools = true,
            "messages" => self.ui.messages = true,
            "help" => self.ui.help = true,
            "bq" => self.show_bq = true,
            "prod" => self.show_prod = true,
            "names" => self.show_names = true,
            "play" => self.ui.briefing = false,
            "hq" => {
                if let Some(at) = hq {
                    ui::open_building_at(self, at);
                }
            }
            "kind" => match first(&self.sim, &|k| format!("{k:?}") == arg) {
                Some(at) => {
                    ui::open_building_at(self, at);
                }
                None => eprintln!("RR_OPEN: no {arg} of ours"),
            },
            // the map on the nearest free harbor spot across our first harbor's sea
            "spot" => {
                let h = first(&self.sim, &|k| k == rr_rules::BuildingType::Harbor);
                if let Some(p) = h.and_then(|h| ui::known_free_spots(self, h).first().copied()) {
                    self.cam.center_on(&self.sim, p, self.screen);
                }
            }
            // our first building of a kind gets the player's priority (one frame on)
            "prio" => {
                if let Some(at) = first(&self.sim, &|k| format!("{k:?}") == arg) {
                    let out = self.sim.step(&[PlayerCommand { player: me, cmd: Cmd::SetPriority { at, on: true } }]);
                    self.handle_events(out.events);
                }
            }
            "build" => {
                let w = self.sim.world();
                let spot = hq.and_then(|h| {
                    let mut near: Vec<MapCoord> = w.grid().within(h, 6).collect();
                    near.sort_by_key(|p| (w.grid().distance(h, *p), p.y, p.x));
                    near.into_iter().find(|p| w.bq_for(*p, me) == rr_world::Bq::Castle)
                });
                if let Some(node) = spot {
                    self.cam.center_on(&self.sim, node, self.screen);
                    self.ui.action = Some(ui::Action { node, tab: arg.parse().unwrap_or(0) });
                }
            }
            _ => eprintln!("RR_OPEN: no window {name}"),
        }
    }

    /// The free harbor spots to mark on the map: while an expedition of ours is under way, a
    /// harbor's window is open (in `open`) or the building help is on.
    fn harbor_spots_shown(&self, open: &[MapCoord]) -> Vec<MapCoord> {
        let me = self.player;
        let harbors: Vec<&rr_sim::Building> = self
            .sim
            .buildings()
            .iter()
            .map(|(_, b)| b)
            .filter(|b| b.player == me && b.kind == rr_rules::BuildingType::Harbor && b.is_ready())
            .collect();
        let active = self.show_bq || harbors.iter().any(|h| h.expedition.is_some() || open.contains(&h.pos));
        if !active {
            return Vec::new();
        }
        let mut seen_seas = Vec::new();
        let mut spots = Vec::new();
        for h in harbors {
            let sea = self.sim.harbor_sea(h.pos);
            if sea.is_some() && !seen_seas.contains(&sea) {
                seen_seas.push(sea);
                spots.extend(ui::known_free_spots(self, h.pos));
            }
        }
        spots
    }

    /// A click on a building window's live view takes the map there (`ui::INSET_GO`).
    fn inset_jump(&mut self, ctx: &egui::Context) {
        if let Some(at) = ctx.data_mut(|d| d.remove_temp::<MapCoord>(egui::Id::new(ui::INSET_GO))) {
            self.cam.center_on(&self.sim, at, self.screen);
            self.session.follow = None;
        }
    }

    fn view<'a>(&'a self, rect: egui::Rect, time: f32) -> View<'a> {
        View {
            sim: &self.sim,
            cam: self.cam,
            rect,
            render_gf: self.sim.gf().0 as f32 - 1.0 + self.acc as f32,
            player: self.player,
            show_bq: self.show_bq || self.session.road.is_some(),
            show_prod: self.show_prod,
            show_names: self.show_names,
            selected: self.session.selected,
            road_preview: self.session.road.clone(),
            planned: if self.paused {
                crate::render::planned_marks(&self.session.pending.iter().map(|c| c.cmd.clone()).collect::<Vec<_>>())
            } else {
                Vec::new()
            },
            work_area: self.session.work_area,
            time,
            fog_of_war: self.fog_of_war,
            idle: Some(&self.idle),
            // (RR_REVEAL: the whole map, for headless screenshots)
            reveal_all: self.sees_all(),
            scenery: self.editor.as_ref().is_none_or(|e| e.show_scenery),
            fog_cache: Default::default(),
            life: Some(&self.life),
        }
    }

    /// Play without a mouse: Enter puts a cursor in the middle of the view, the arrows (up and
    /// down zig-zag along a column), Home / PageUp / End / PageDown (the diagonals) or the
    /// number pad without Num Lock move it, Enter clicks there, Backspace takes back a road
    /// step, Esc ends it. In the action window 1-9 build, F places a flag, T shows the next tab.
    fn cursor_input(&mut self, ctx: &egui::Context, rect: egui::Rect, typing: bool) {
        // keys belong to a focused button or field in a window
        if typing || ctx.memory(|m| m.focused()).is_some() {
            return;
        }
        let (enter, back, keys) = ctx.input(|i| {
            let keys: Vec<Key> = i
                .events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Key { key, pressed: true, modifiers, .. } if !modifiers.command => Some(*key),
                    _ => None,
                })
                .collect();
            (i.key_pressed(Key::Enter), i.key_pressed(Key::Backspace), keys)
        });
        let g = self.sim.world().grid();
        let Some(mut c) = self.session.cursor else {
            if enter {
                self.session.cursor = Some(self.view(rect, 0.0).pick(rect.center()));
            }
            return;
        };
        for k in keys {
            if let Some(n) = cursor_step(g, c, k) {
                c = n;
            }
        }
        self.session.cursor = Some(c);
        // the view follows the cursor
        let at = self.view(rect, 0.0).node_screen(c);
        if !rect.shrink2(rect.size() * 0.15).contains(at) {
            self.cam.center_on(&self.sim, c, self.screen);
        }
        if back && let Some((_, dirs)) = self.session.road.as_mut() {
            dirs.pop();
        }
        if enter {
            self.click_node(c);
        }
    }

    /// A quick load asks first: play since the last save would be lost (not in a replay or a
    /// network game).
    fn load_asks_first(&self) -> bool {
        load_needs_asking(self.sim.gf().0, self.session.saved_gf) && self.playback.is_none() && self.net.is_none()
    }

    #[allow(clippy::too_many_lines)] // to split (architect round 2, #8: no new long ones)
    fn map_input(&mut self, ctx: &egui::Context, resp: &egui::Response, rect: egui::Rect) {
        // keys go to a text field while one has focus
        let typing = ctx.wants_keyboard_input();
        // W/A/S/D by where they sit on the keyboard, whatever the layout
        ctx.input(|i| {
            if !i.focused {
                self.pan_held = [false; 4];
            }
            for e in &i.events {
                if let egui::Event::Key { key, physical_key, pressed, .. } = e
                    && let Some(j) =
                        [Key::A, Key::D, Key::W, Key::S].iter().position(|k| *k == physical_key.unwrap_or(*key))
                {
                    self.pan_held[j] = *pressed && !typing;
                }
            }
        });
        let (scroll, keys) = ctx.input(|i| {
            let mut k = Vec2::ZERO;
            if typing {
                return (i.raw_scroll_delta, k);
            }
            for d in [Key::ArrowLeft, Key::ArrowRight, Key::ArrowUp, Key::ArrowDown] {
                // with the keyboard cursor the arrows move the cursor
                if i.key_down(d) && self.session.cursor.is_none() {
                    k += pan_dir(d).unwrap_or_default();
                }
            }
            for (j, d) in [Key::A, Key::D, Key::W, Key::S].into_iter().enumerate() {
                if self.pan_held[j] {
                    k += pan_dir(d).unwrap_or_default();
                }
            }
            (i.raw_scroll_delta, k)
        });
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        self.cam.offset += keys * 700.0 * dt / self.cam.zoom;
        // following a settler: the camera glides after it; moving the map ends it
        if keys != Vec2::ZERO || resp.dragged_by(PointerButton::Secondary) || resp.dragged_by(PointerButton::Middle) {
            self.session.follow = None;
        }
        if let Some(f) = self.session.follow {
            match self.sim.figures().get(f) {
                Some(fig) => {
                    let mut to = self.cam;
                    to.center_on(&self.sim, fig.pos, rect.size());
                    self.cam.offset = follow_step(self.cam.offset, to.offset, dt);
                }
                None => self.session.follow = None,
            }
        }
        if resp.hovered() && scroll.y != 0.0 {
            let mouse = ctx.input(|i| i.pointer.hover_pos()).unwrap_or(rect.center());
            self.cam.zoom_at(if scroll.y > 0.0 { 1.12 } else { 1.0 / 1.12 }, mouse - rect.min);
        }
        // the editor paints while the left button is dragged (each node once); one undo step
        // per stroke
        if resp.drag_started_by(PointerButton::Primary)
            && let Some(ed) = self.editor.as_mut()
        {
            ed.checkpoint();
            ed.last = None;
            ed.last_tri = None;
        }
        // one triangle at a time: by the pointer's triangle, not its node
        if self.editor.as_ref().is_some_and(|ed| ed.tri && ed.tool == crate::editor::Tool::Terrain)
            && (resp.dragged_by(PointerButton::Primary) || resp.clicked())
            && let Some(pos) = resp.interact_pointer_pos()
        {
            let tri = crate::editor::triangle_at(&self.view(rect, 0.0), pos);
            if let Some(ed) = self.editor.as_mut()
                && let Some((q, rsu)) = tri
                && ed.last_tri != tri
            {
                if resp.clicked() {
                    ed.checkpoint();
                }
                ed.paint_triangle(q, rsu);
                ed.last_tri = tri;
            }
            return;
        }
        if self.editor.is_some()
            && resp.dragged_by(PointerButton::Primary)
            && let Some(pos) = resp.interact_pointer_pos()
        {
            let node = self.view(rect, 0.0).pick(pos);
            if let Some(ed) = self.editor.as_mut()
                && ed.last != Some(node)
                && !matches!(ed.tool, crate::editor::Tool::Headquarters | crate::editor::Tool::Harbor)
            {
                ed.apply(node);
                ed.last = Some(node);
            }
        }
        // right or middle drag scrolls (like the original's right-drag)
        if resp.drag_started_by(PointerButton::Secondary) || resp.drag_started_by(PointerButton::Middle) {
            self.drag_from = resp.interact_pointer_pos();
        }
        if (resp.dragged_by(PointerButton::Secondary) || resp.dragged_by(PointerButton::Middle))
            && let Some(p) = resp.interact_pointer_pos()
            && let Some(f) = self.drag_from
        {
            self.cam.offset -= (p - f) / self.cam.zoom;
            self.drag_from = Some(p);
        }
        // right-click on the map: stop building a road, or close the action window
        if resp.clicked_by(PointerButton::Secondary) {
            if self.session.road.is_some() {
                self.session.road = None;
            } else {
                self.ui.action = None;
                self.session.selected = None;
            }
        }
        if resp.clicked()
            && let Some(pos) = resp.interact_pointer_pos()
        {
            // a click on a building's picture means that building
            let node = self.view(rect, 0.0).pick_click(pos);
            self.click_node(node);
        }
        self.cursor_input(ctx, rect, typing);
        let (mut save, mut load, mut fullscreen, mut to_hq) = (false, false, false, false);
        let mut to_alert = false;
        let keys = self.keys.clone();
        ctx.input(|i| {
            // (and not while a key is being chosen in the options)
            if typing || self.rebinding.is_some() {
                return;
            }
            // a letter typed on a pan key (Z on AZERTY's W) pans and does nothing else
            let shadowed = shadowed_by_pan(&i.events);
            // the editor's preview has no players: only view keys work there
            let editing = self.editor.is_some();
            let pressed = |i: &egui::InputState, k: Key| {
                i.key_pressed(k) && !shadowed.contains(&k) && (!editing || editor_key(&keys, k))
            };
            use crate::keys::Act;
            let k = |a: Act| keys.key(a);
            // RTTR's keys: H headquarters, I inventory, L map, M main selection, N messages,
            // Z / shift-Z zoom
            if pressed(i, k(Act::ToCastle)) {
                to_hq = true;
            }
            // G: to the latest alert (a11y round 2, #5: "Go there" was a mouse button only)
            if pressed(i, k(Act::ToAlert)) {
                to_alert = true;
            }
            if pressed(i, k(Act::Inventory)) {
                self.ui.inventory = !self.ui.inventory;
            }
            if pressed(i, k(Act::Map)) {
                self.ui.map = !self.ui.map;
            }
            if pressed(i, k(Act::Main)) && i.modifiers.command {
                if let Some(a) = self.audio.as_mut() {
                    a.muted = !a.muted;
                }
            } else if pressed(i, k(Act::Main)) {
                self.ui.main = !self.ui.main;
            }
            if pressed(i, k(Act::Messages)) {
                self.ui.messages = !self.ui.messages;
            }
            if pressed(i, k(Act::Help)) {
                self.ui.help = !self.ui.help;
            }
            if pressed(i, k(Act::Names)) {
                self.show_names = !self.show_names;
            }
            if pressed(i, k(Act::Zoom)) && !i.modifiers.command {
                // around the middle of the screen (the corner lost sight of the castle)
                let f = if i.modifiers.shift { 0.8 } else { 1.25 };
                self.cam.zoom_at(f, self.screen / 2.0);
            }
            if pressed(i, k(Act::BuildHelp)) {
                self.show_bq = !self.show_bq;
            }
            if pressed(i, k(Act::Productivity)) {
                self.show_prod = !self.show_prod;
            }
            if pressed(i, k(Act::Pause)) {
                self.toggle_pause();
            }
            if pressed(i, k(Act::Fullscreen)) {
                fullscreen = true;
            }
            if pressed(i, k(Act::QuickSave)) {
                save = true;
            }
            if pressed(i, k(Act::QuickLoad)) {
                load = true;
            }
            if pressed(i, Key::Escape) {
                // a follow ends too (a11y round 3, #7)
                self.session.follow = None;
                // the road or the action window first; with neither, the open lists close
                if self.session.road.is_none() && self.ui.action.is_none() {
                    self.ui.close_lists();
                }
                if self.session.road.is_none() && self.ui.action.is_none() {
                    self.session.cursor = None;
                }
                self.session.road = None;
                self.session.selected = None;
                self.ui.action = None;
            }
            // (= is + without shift on many keyboards)
            if pressed(i, k(Act::Faster)) || k(Act::Faster) == Key::Plus && pressed(i, Key::Equals) {
                self.speed = (self.speed + 1).min(SPEEDS.len() - 1);
            }
            if pressed(i, k(Act::Slower)) {
                self.speed = self.speed.saturating_sub(1);
            }
        });
        // (a jump ends a follow: it pulled the view back at once; QA round 8, #9)
        if to_hq || to_alert {
            self.session.follow = None;
        }
        if to_hq && let Some(p) = home_view(&self.sim, self.player) {
            self.cam.center_on(&self.sim, p, self.screen);
        }
        if to_alert && let Some(p) = self.session.last_alert {
            self.cam.center_on(&self.sim, p, self.screen);
            self.session.alert = None;
            self.mark = Some((p, ctx.input(|i| i.time)));
        }
        if save {
            self.quick_save();
        }
        if fullscreen {
            let full = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!full));
        }
        if load {
            if self.load_asks_first() {
                self.session.ask_load = true;
            } else {
                self.quick_load();
            }
        }
        // in the editor F5 / Ctrl+S save the map, Ctrl+Z undoes, Ctrl+Y or Ctrl+Shift+Z redoes
        if let Some(ed) = self.editor.as_mut()
            && !typing
        {
            let (save, undo, redo) = ctx.input(|i| {
                let c = i.modifiers.command;
                (
                    i.key_pressed(Key::F5) || c && i.key_pressed(Key::S),
                    c && !i.modifiers.shift && i.key_pressed(Key::Z),
                    c && (i.key_pressed(Key::Y) || i.modifiers.shift && i.key_pressed(Key::Z)),
                )
            });
            if save {
                ed.save_with_message();
            }
            if undo {
                ed.undo();
            }
            if redo {
                ed.redo();
            }
        }
    }

    fn click_node(&mut self, node: MapCoord) {
        // (a node from another map, e.g. a cursor kept over a load: nothing there)
        if !self.sim.world().grid().contains(node) {
            return;
        }
        if let Some(ed) = self.editor.as_mut() {
            ed.checkpoint();
            ed.apply(node);
            ed.last = Some(node);
            return;
        }
        let g = self.sim.world().grid();
        if let Some((start, mut dirs)) = self.session.road.clone() {
            let mut node = self.sim.road_click_target(node);
            self.road_hint = None;
            let mut end = start;
            for d in &dirs {
                end = g.neighbor(end, *d);
            }
            // clicking the end again finishes the road with a flag there (as in the original)
            if node == end {
                if dirs.len() >= 2 {
                    self.finish_road();
                } else {
                    self.road_hint =
                        Some(crate::i18n::tr("A flag can't stand right next to another flag.").to_string());
                }
                return;
            }
            // clicking the previous node undoes a step
            if let Some(last) = dirs.last().copied()
                && g.neighbor(end, last.opposite()) == node
            {
                dirs.pop();
                self.session.road = Some((start, dirs));
                return;
            }
            let mut visited: Vec<MapCoord> = vec![start];
            let mut p = start;
            for d in &dirs {
                p = g.neighbor(p, *d);
                visited.push(p);
            }
            let step = Direction::ALL.iter().copied().find(|d| g.neighbor(end, *d) == node);
            let ext = match step {
                Some(d)
                    if self.sim.road_step_ok(self.player, end, d, self.sim.flag_at(node).is_some()).is_ok()
                        && !visited.contains(&node) =>
                {
                    Some(vec![d])
                }
                _ => self.sim.road_path(self.player, end, node, &visited),
            };
            // aiming at a flag of a network we don't belong to yet: when that network passes
            // close by, join it there instead of running a road of our own all the way
            let (mut ext, target) = (ext, self.sim.flag_at(node));
            let foreign =
                target.is_some_and(|t| self.sim.flag_at(start).is_none_or(|s| self.sim.flag_distance(s, t).is_none()));
            if foreign
                && let Some((goal, j)) = self.sim.road_join_path(self.player, end, node, &visited)
                && prefer_join(ext.as_ref().map(Vec::len), j.len())
            {
                node = goal;
                ext = Some(j);
            }
            let Some(ext) = ext else {
                self.road_trouble(end, node, &visited);
                return;
            };
            dirs.extend(ext);
            if self.sim.flag_at(node).is_some() {
                self.build_road(start, dirs);
                self.session.road = None;
            } else if self.sim.road_at(node).is_some() && flag_fits(&self.sim, node) {
                // meeting another road: a flag there joins them
                self.cmd(Cmd::PlaceFlag { at: node });
                self.build_road(start, dirs);
                self.session.road = None;
            } else if self.sim.road_at(node).is_some() {
                // no flag fits here: the road stays under way, and the player hears why
                // (new-player round 7, #4: it used to fail silently)
                self.note(
                    crate::i18n::tr(
                        "A flag can't go here, next to another flag: click that flag, or one step further on.",
                    )
                    .into(),
                );
                self.session.road = Some((start, dirs));
            } else {
                self.session.road = Some((start, dirs));
            }
            return;
        }
        self.session.selected = Some(node);
        let tab = ui::tab_for(self.sim.world().bq_for(node, self.player), self.next_first_step());
        ui::set_action(self, Action { node, tab });
    }

    fn apply_settings(&mut self, s: &crate::settings::Settings, ctx: &egui::Context) {
        if let Some(a) = self.audio.as_mut() {
            a.master = s.master;
            a.music_volume = s.music;
            a.sfx_volume = s.sfx;
            a.ambience = s.ambience;
            a.mute_in_background = s.mute_in_background;
            a.shuffle = s.shuffle;
        }
        let old = ctx.zoom_factor();
        ctx.set_zoom_factor(s.ui_scale);
        self.cam.ui_rescaled(old, s.ui_scale);
        self.fog_of_war = s.fog_of_war;
        self.pause_on_attack = s.pause_on_attack;
        self.calm_map = s.calm_map;
        self.tips = s.tips;
        // on by default; switched off in the options or with --no-auto-road
        self.auto_road &= s.auto_road;
        self.won_once = s.won_once;
        self.won_missions = s.won_missions.clone();
        self.best = s.best.clone();
        self.keys = crate::keys::Keys::from_overrides(&s.keys);
        self.menu.ai_level = crate::settings::first_level(s.won_once);
    }

    /// The options as they are now; the language is kept only once it differs from `$LANG`.
    fn settings_now(
        &self,
        ctx: &egui::Context,
        prev: &crate::settings::Settings,
        lang_de: bool,
    ) -> crate::settings::Settings {
        let mut s = prev.clone();
        if let Some(a) = self.audio.as_ref() {
            s.master = a.master;
            s.music = a.music_volume;
            s.sfx = a.sfx_volume;
            s.ambience = a.ambience;
            s.mute_in_background = a.mute_in_background;
            s.shuffle = a.shuffle;
        }
        s.ui_scale = ctx.zoom_factor();
        s.fog_of_war = self.fog_of_war;
        s.pause_on_attack = self.pause_on_attack;
        s.calm_map = self.calm_map;
        s.tips = self.tips;
        s.auto_road = self.auto_road;
        s.won_once |= self.won_once;
        for m in &self.won_missions {
            if !s.won_missions.contains(m) {
                s.won_missions.push(m.clone());
            }
        }
        for b in &self.best {
            crate::settings::record_best(&mut s.best, b.clone());
        }
        s.keys = self.keys.overrides();
        let de = crate::i18n::is_german();
        if s.german.is_some() || de != lang_de {
            s.german = Some(de);
        }
        s
    }

    /// Write the options when they changed (not while a slider is being dragged).
    fn keep_settings(&mut self, ctx: &egui::Context) {
        let Some((prev, lang_de)) = self.stored.as_ref() else { return };
        if ctx.input(|i| i.pointer.any_down()) {
            return;
        }
        let now = self.settings_now(ctx, prev, *lang_de);
        if now != *prev {
            now.store();
            self.stored = Some((now, *lang_de));
        }
    }

    pub fn save_path() -> std::path::PathBuf {
        // (Linux: $XDG_DATA_HOME or ~/.local/share; Windows: %APPDATA%)
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("APPDATA").map(std::path::PathBuf::from))
            .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share")))
            .unwrap_or_else(|| ".".into());
        base.join("roads-and-realms").join("quick.rrsave")
    }

    /// Where desync reports go (next to the saves).
    pub fn desync_dir() -> std::path::PathBuf {
        Self::save_path().with_file_name("desync")
    }

    /// Folder of the named save games.
    pub fn saves_dir() -> std::path::PathBuf {
        Self::save_path().with_file_name("saves")
    }

    /// Named save games, newest first.
    pub fn list_saves() -> Vec<SaveEntry> {
        // (read at most twice a second: the menu and the window ask every drawn frame, and each
        // read opens every save; a finished save reads it again at once)
        SAVE_LIST.with(|kept| {
            let mut kept = kept.borrow_mut();
            if let Some((at, list)) = kept.as_ref()
                && at.elapsed().as_millis() < 500
            {
                return list.clone();
            }
            let list = list_saves_in(&Self::saves_dir(), &Self::saves_beside());
            *kept = Some((Instant::now(), list.clone()));
            list
        })
    }

    /// The saves kept beside the named ones: the quick save (new-player round 12, #5: after F5
    /// the list said "No saved games yet") and the autosaves (QA round 7, #2).
    fn saves_beside() -> [std::path::PathBuf; 3] {
        [Self::save_path(), Self::autosave_path(), Self::save_path().with_file_name(LAN_AUTOSAVE)]
    }

    pub fn quick_save(&mut self) {
        self.save_to(&Self::save_path());
    }

    pub fn quick_load(&mut self) {
        self.session.ask_load = false;
        self.load_from(&Self::save_path());
    }

    /// Where the game saves itself every ten minutes (apart from the quick save).
    pub fn autosave_path() -> std::path::PathBuf {
        Self::save_path().with_file_name("autosave.rrsave")
    }

    /// Network games can't be saved or loaded (the others would not follow).
    fn no_net(&mut self) -> bool {
        if self.net.is_some() {
            let msg = crate::i18n::tr("Not in a network game.").to_string();
            // (seen at once, not only in the list: net round 4, #3)
            self.note(msg.clone());
            // (in the menu a note isn't drawn: there it goes to the menu's own line)
            self.menu.note = format!("{msg} {}", crate::i18n::tr("Leave the network game first."));
            self.messages.push_front(Message::new(self.sim.gf().0, msg, None, MSG_GENERAL));
        }
        self.net.is_none()
    }

    /// Save under a name (letters, digits, spaces, - and _ are kept).
    pub fn save_named(&mut self, name: &str) {
        let clean: String =
            name.chars().filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_')).collect::<String>();
        let clean = clean.trim();
        if clean.is_empty() {
            return;
        }
        self.save_to(&Self::saves_dir().join(format!("{clean}.rrsave")));
    }

    /// P and the pause button: a network game can't be paused (the others play on), so it says
    /// so instead of showing "Paused" while play goes on (net round 4, #3).
    pub fn toggle_pause(&mut self) {
        if !can_pause(self.net.is_some()) {
            self.note(crate::i18n::tr("A network game can't be paused: the others play on.").into());
            return;
        }
        // the briefing holds the game until Begin (new-player round 11, #9: P ran the clock
        // behind it)
        if self.ui.briefing {
            return;
        }
        self.paused = !self.paused;
        self.session.paused_by_attack = false;
    }

    /// The news of a building of ours finished; a harbor or storehouse in a mission keeps two
    /// soldiers at once (new-player round 10, #3: rivals took ten empty harbors with one man).
    fn finished_news(&mut self, at: MapCoord, kind: rr_rules::BuildingType) -> String {
        let mission =
            plays_a_mission(self.args.mission.is_some(), self.session.map_mission.is_some(), self.net.is_some());
        let reserve = self.sim.building_at(at).map(|b| self.sim.building(b).reserve).unwrap_or_default();
        if let Some(count) = default_reserve(kind, mission, reserve) {
            self.cmd(Cmd::SetReserve { at, rank: 0, count });
            let tip = crate::i18n::tr(
                "💡 Two soldiers stay in this warehouse to defend it: rivals attack empty warehouses first. Change it in its window (Reserve).",
            );
            self.messages.push_front(Message::new(self.sim.gf().0, tip.into(), Some(at), MSG_TIP));
        }
        crate::i18n::msg("{1} finished", &[ui::building_key(kind)])
    }

    /// The news of a building of ours lost at `at`; a warehouse's loss is an alert too, naming
    /// the rival whose soldiers stand by it.
    fn lost_news(&mut self, at: MapCoord, kind: rr_rules::BuildingType) -> String {
        let me = self.player;
        let g = self.sim.world().grid();
        let mut near = [0u32; 8];
        for (_, f) in self.sim.figures().iter() {
            if f.player != me && f.job.soldier_rank().is_some() && g.distance(f.pos, at) <= 4 {
                near[f.player as usize % 8] += 1;
            }
        }
        let by = (0..8).filter(|p| near[*p] > 0).max_by_key(|p| near[*p]);
        let name = by.and_then(|p| self.sim.players().get(p).map(|x| x.setup.name.clone()));
        let last = !self.sim.buildings().iter().any(|(_, b)| b.player == me && b.kind.is_warehouse() && b.is_ready());
        // a workshop on land that a rival's new post took (new-player round 12, #2: a peaceful
        // neighbour's watchtower burned a hut, told as "Hunter lost")
        if !lost_is_alert(kind)
            && let Some(o) = self.sim.world().owner(at).filter(|o| *o != me)
        {
            let owner = rr_game::plain(&self.sim.players()[o as usize].setup.name);
            return crate::i18n::msg(LAND_TAKEN, &[ui::building_key(kind), &owner]);
        }
        let txt = lost_text(kind, name.as_deref(), last);
        // an alert when a rival's soldiers took it; taking down one's own is no alarm (QA
        // round 7, #5)
        if lost_is_alert(kind) && name.is_some() {
            self.alert(txt.clone(), at, false);
        }
        txt
    }

    /// Your own refused order: its reason as a note too (muted or deaf players got no sign that
    /// a click failed; the text went to a tab the unread counter leaves out).
    fn note_refusal(&mut self, e: &Event, me: u8, txt: &str) {
        if refusal_shown(e, me) {
            self.note(crate::i18n::show(txt));
        }
    }

    /// Show a short note at the top of the screen (restarts its three seconds).
    pub fn note(&mut self, text: String) {
        self.session.toast = Some(text);
        self.session.toast_since = None;
        self.session.toast_at = None;
    }

    /// Everything of the interface that belongs to one game, cleared whenever the game is
    /// replaced (new game, load, network start, replay; QA round 4, #2 and #7). A load then
    /// restores what its save carries.
    fn reset_game_ui(&mut self) {
        // (a save of the last game is told there, not in this one: QA round 9, #2)
        self.finish_save(true);
        self.game_no = self.game_no.wrapping_add(1);
        self.session = Session::default();
        // (the map's remembered life is that game's: its footprints on a smaller loaded map
        // crashed the game, eval of 2026-10-29)
        self.life = crate::ambience::Life::default();
        self.menu.note.clear();
        // (a load under the briefing kept it, and the loaded game stood frozen; QA round 8, #1)
        self.ui.briefing = false;
        self.ui.action = None;
        self.ui.kept.clear();
        // (the truce question belongs to that game's post; the count and strong or weak are the
        // player's habit and stay: QA round 9, #3)
        self.ui.attack.asking = None;
    }

    /// Build a road the player drew, with flags along it when that option is on.
    fn build_road(&mut self, start: MapCoord, dirs: Vec<rr_core::Direction>) {
        let flags = if self.auto_road { auto_flags(&self.sim, start, &dirs) } else { Vec::new() };
        self.cmd(Cmd::BuildRoad { from: start, dirs });
        for at in flags {
            self.cmd(Cmd::PlaceFlag { at });
        }
    }

    /// Go on alone after a lost network game: computer players take over the other humans'
    /// realms (LAN round 3, #3); it is then a normal game that can be saved.
    /// A network game saves itself too (net round 4, #5: a crash lost everything): each
    /// machine its own file, where computer players take the other humans' realms, so it goes
    /// on as a game of one.
    fn autosave_lan(&mut self) {
        let ais = lan_save_ais(&self.sim, &self.ais, self.player, crate::ai_level(&self.args));
        let meta = SaveMeta {
            behind_told: self.session.behind_told,
            stories: self.session.stories,
            deeds: self.session.deeds,
            favour: self.session.favour,
            castles_lost: self.session.castles_lost.clone(),
            level: Some(self.args.ai_level),
            player: Some(self.player),
            ..Default::default()
        };
        let extra = encode_ui(&self.messages, self.session.tips_shown, self.session.read_until, &meta);
        let path = Self::save_path().with_file_name(LAN_AUTOSAVE);
        let bytes = rr_ai::save_game_with(&self.sim, &ais, &extra);
        self.write_in_background(&path, bytes, true);
    }

    pub fn continue_alone(&mut self) {
        let alive: Vec<bool> = self.sim.players().iter().map(|p| p.alive).collect();
        let ai_players: Vec<u8> = self.ais.iter().map(|a| a.player).collect();
        let level = crate::ai_level(&self.args);
        for p in takeover_slots(&alive, self.player, &ai_players) {
            self.ais.push(rr_ai::Ai::with_level(p, level));
        }
        self.session.net_lost = false;
        self.session.game_over = None;
        self.waiting = false;
        self.recorder = None;
    }

    /// Military news you can't miss: a note with "Go there" and, at the first attack of a
    /// fight, the game back at normal speed (at most once a minute).
    fn alert(&mut self, stored: String, at: MapCoord, attacked: bool) {
        if self.playback.is_some() {
            return;
        }
        self.session.alert = Some(crate::ui::Alert::new(crate::i18n::show(&stored), at));
        self.session.last_alert = Some(at);
        if attacked {
            let gf = self.sim.gf().0;
            (self.speed, self.session.slowed_at) = slow_for_attack(self.speed, self.session.slowed_at, gf);
            if self.pause_on_attack && self.net.is_none() {
                self.paused = true;
                self.session.paused_by_attack = true;
            }
        }
    }

    pub fn save_to(&mut self, p: &std::path::Path) {
        if !self.no_net() {
            return;
        }
        // a replay has no computer players: saving it would overwrite a real game with a free win
        if let Some(why) = save_refused(self.playback.is_some()) {
            self.note(crate::i18n::tr(why).into());
            return;
        }
        let _ = std::fs::create_dir_all(p.parent().unwrap_or(std::path::Path::new(".")));
        let meta = SaveMeta {
            mission: self.args.mission.clone(),
            map: self.args.map.clone(),
            land_goal: self.args.land_goal,
            behind_told: self.session.behind_told,
            stories: self.session.stories,
            deeds: self.session.deeds,
            favour: self.session.favour,
            castles_lost: self.session.castles_lost.clone(),
            peace: self.args.peace,
            time_limit: self.args.time_limit,
            coins_goal: self.args.coins_goal,
            goal_done: self.session.goal_done,
            level: Some(self.args.ai_level),
            player: Some(self.player),
        };
        let extra = encode_ui(&self.messages, self.session.tips_shown, self.session.read_until, &meta);
        let bytes = rr_ai::save_game_with(&self.sim, &self.ais, &extra);
        self.write_in_background(p, bytes, false);
    }

    /// Check that `bytes` load and write them to `p`, on a thread of its own; the result is told
    /// when it is done (`finish_save`). One save at a time: a second waits for the first.
    fn write_in_background(&mut self, p: &std::path::Path, bytes: Vec<u8>, lan: bool) {
        self.finish_save(true);
        let path = p.to_path_buf();
        let thread = std::thread::spawn(move || {
            // a save that wouldn't load never replaces one that does (net round 5, #1)
            loads_back(&bytes).map_err(SaveFail::Unloadable)?;
            write_safely(&path, &bytes).map_err(|e| SaveFail::Write(e.to_string()))
        });
        self.saving =
            Some(PendingSave { thread, path: p.to_path_buf(), gf: self.sim.gf().0, game_no: self.game_no, lan });
    }

    /// Tell how a save went, once it is done (`wait`: wait for it; before a load, at exit).
    pub(crate) fn finish_save(&mut self, wait: bool) -> Option<String> {
        if !self.saving.as_ref().is_some_and(|s| wait || s.thread.is_finished()) {
            return None;
        }
        let done = self.saving.take()?;
        let result = done.thread.join().unwrap_or_else(|_| Err(SaveFail::Write("the save thread failed".into())));
        let name = done.path.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        SAVE_LIST.with(|kept| kept.borrow_mut().take());
        // (told in the game it was made in: a save finished after a new game started marked the
        // new one saved; QA round 9, #2)
        let failed = match result {
            // a short note, not a post-office entry; the name, not the whole path
            // (a save of the player's own counts for F9's question, the autosave doesn't: QA
            // round 7, #1, it reset the clock, and F9 then loaded an old quick save)
            Ok(()) => {
                if !done.lan && done.game_no == self.game_no && marks_play_saved(&done.path, &Self::autosave_path()) {
                    self.session.saved_gf = Some(done.gf);
                }
                self.note(format!("{} {name}", crate::i18n::tr("Game saved:")));
                return None;
            }
            // (the LAN autosave stays quiet when it fails, as before)
            Err(_) if done.lan => return None,
            Err(SaveFail::Unloadable(e)) => {
                format!("{} {e}", crate::i18n::tr("Not saved, the game kept the last good save:"))
            }
            Err(SaveFail::Write(e)) => format!("{} {e}", crate::i18n::tr("Save failed:")),
        };
        self.messages.push_front(Message::new(self.sim.gf().0, failed.clone(), None, MSG_GENERAL));
        Some(failed)
    }

    pub fn load_from(&mut self, p: &std::path::Path) {
        if !self.no_net() {
            return;
        }
        // (a save still being written is the one to load; if it failed, the loaded game is told)
        let failed = self.finish_save(true);
        // (with --rules, a network game's autosave was made with the classic rules: tried with
        // those too; net round 5, #7)
        let res = std::fs::read(p).map_err(|e| e.to_string()).and_then(|b| {
            rr_ai::load_game_with(crate::rules(), &b)
                .or_else(|e| rr_ai::load_game_with(std::sync::Arc::new(rr_rules::Rules::classic()), &b).map_err(|_| e))
        });
        match res {
            Ok((sim, ais, extra)) => {
                self.reset_game_ui();
                self.sim = sim;
                // the messages and tips of that game (older saves have none)
                let (messages, tips, read, meta) = decode_ui(&extra).unwrap_or_default();
                (self.messages, self.session.tips_shown, self.session.read_until) = (messages, tips, read);
                // and which game it is: a mission stays a mission (its map, rivals, goal)
                self.args.mission = meta.mission;
                self.args.land_goal = meta.land_goal;
                (self.args.peace, self.args.time_limit) = (meta.peace, meta.time_limit);
                self.args.coins_goal = meta.coins_goal;
                if let Some(l) = meta.level {
                    self.args.ai_level = l;
                }
                self.session.map_mission =
                    meta.map.as_deref().and_then(|p| crate::missions::MapMission::load(std::path::Path::new(p)));
                self.args.map = meta.map;
                self.session.behind_told = meta.behind_told;
                self.session.stories = meta.stories;
                self.session.deeds = meta.deeds;
                self.session.favour = meta.favour;
                self.session.castles_lost = meta.castles_lost;
                // a loaded game can't be replayed from its start
                self.recorder = None;
                self.playback = None;
                // the slot the save names, else the one no computer plays (net round 5, #4)
                let n = self.sim.players().len();
                self.player = meta.player.filter(|p| (*p as usize) < n).unwrap_or_else(|| human_player(n, &ais));
                self.ais = ais;
                arm_invader(&mut self.ais, &self.args);
                self.session.goal_done = meta.goal_done;
                self.session.road = None;
                self.ui.action = None;
                self.ui.kept.clear();
                self.session.game_over = None;
                // the map may differ: new textures and minimap
                self.tex = None;
                self.minimap = None;
                self.in_menu = false;
                self.editor = None;
                if let Some(pos) = home_view(&self.sim, self.player) {
                    self.cam.center_on(&self.sim, pos, self.screen);
                }
                self.session.saved_gf = Some(self.sim.gf().0);
                self.note(crate::i18n::tr("Game loaded").into());
                if let Some(m) = failed {
                    self.messages.push_front(Message::new(self.sim.gf().0, m, None, MSG_GENERAL));
                }
            }
            Err(e) => {
                let why = load_error(&e, !p.exists());
                // in the menu the reason shows there, and the menu stays (new-player round 6, #6)
                if self.in_menu {
                    self.menu.map_msg = why.clone();
                }
                self.messages.push_front(Message::new(self.sim.gf().0, why, None, MSG_GENERAL));
            }
        }
    }

    /// Write the running game's replay (called when a new game starts and on exit).
    pub fn save_replay(&mut self) {
        let Some(mut r) = self.recorder.take() else { return };
        if self.sim.gf().0 < 100 {
            return;
        }
        r.finish(&self.sim);
        let p = crate::replay_path();
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        let _ = write_safely(&p, &r.to_bytes());
    }

    /// Watch the last game again from the start.
    pub fn watch_replay(&mut self) -> bool {
        // the game in the background first: it is the last game now (it read the file before
        // and played the one before it, or nothing; owner's question 2026-10-27)
        if self.net.is_none() {
            self.save_replay();
        }
        // (the replay must open before a network game is left for it: net round 4, #2)
        let Some(rep) = std::fs::read(crate::replay_path()).ok().and_then(|b| rr_sim::Replay::from_bytes(&b)) else {
            return false;
        };
        let Ok(sim) =
            rep.play_setup(crate::rules()).or_else(|_| rep.play_setup(std::sync::Arc::new(rr_rules::Rules::classic())))
        else {
            return false;
        };
        self.leave_net();
        self.save_replay();
        self.reset_game_ui();
        self.sim = sim;
        self.ais.clear();
        self.tex = None;
        self.minimap = None;
        self.messages.clear();
        self.session.game_over = None;
        self.ui = Ui::default();
        self.in_menu = false;
        self.paused = false;
        if let Some(p) = home_view(&self.sim, 0) {
            self.cam.center_on(&self.sim, p, self.screen);
        }
        self.playback = Some((rep, 0));
        self.replay_skip = None;
        true
    }

    /// Tell the player why the road tool can't reach `to`.
    fn road_trouble(&mut self, from: MapCoord, to: MapCoord, avoid: &[MapCoord]) {
        use rr_sim::RoadTrouble as T;
        let msg = match self.sim.road_trouble(self.player, from, to, avoid) {
            T::NotYourLand => "No road there: not your land.",
            T::Blocked => "No road there: something is in the way.",
            T::TooFar => "Too far for one road: click a node closer by.",
            T::CrossesRoad(_) => "A road is in the way: click a node of it to put a flag there and join.",
            T::NoWay => "No way for a road there. Right-click or Cancel ends road building.",
        };
        self.road_hint = Some(crate::i18n::tr(msg).to_string());
    }

    /// Finish the road being drawn at its current end (placing a flag there).
    pub fn finish_road(&mut self) {
        if let Some((start, dirs)) = self.session.road.take()
            && dirs.len() >= 2
        {
            self.build_road(start, dirs);
        }
    }

    /// The node a click would pick, under the pointer, its building-size marker, and the
    /// hover's lines about it once the pointer rests (`HOVER_WORDS_SECS`).
    fn pointer_marks(&self, view: &View<'_>, painter: &egui::Painter, resp: &egui::Response) {
        // the node a click would pick, under the pointer (a11y round 2, #8: nothing showed
        // it, so a shaky hand couldn't check a click before making it); a screenshot has no
        // pointer, only the virtual screen's in the middle
        if let Some(pos) = resp.hover_pos().filter(|_| !resp.dragged() && self.args.screenshot.is_none()) {
            let node = view.pick_click(pos);
            let at = view.node_screen(node);
            // what fits there, as the space bar's marker for this one point (owner, 2026-10-27:
            // "I thought you would just show an icon"); the circle only where no marker shows
            // the point (a building, a flag, land not ours: "a circle has no real use")
            let w = self.sim.world();
            let bq =
                if w.owner(node) == Some(self.player) { w.bq_for(node, self.player) } else { rr_world::Bq::Nothing };
            if bq == rr_world::Bq::Nothing {
                let r = (8.0 * self.cam.zoom).max(5.0);
                painter.circle_stroke(at, r, egui::Stroke::new(1.5_f32, egui::Color32::from_white_alpha(150)));
            } else if !view.show_bq {
                crate::sprites::bq_icon(painter, at, self.cam.zoom, bq);
            }
            // and, once the pointer has rested a while longer, what is there in words
            let rested = resp.ctx.input(|i| i.pointer.time_since_last_movement()) >= HOVER_WORDS_SECS;
            let lines = crate::hover::lines(&self.sim, self.player, node, self.session.favour);
            // (not over the window of the very point it would describe)
            let own_window = self.ui.action.is_some_and(|a| a.node == node);
            if rested && self.session.road.is_none() && !lines.is_empty() && !own_window {
                hover_box(&resp.ctx, pos, &lines);
            }
        }
    }

    fn screenshot_logic(&mut self, ctx: &egui::Context) {
        if let Some(f) = self.frames.as_mut() {
            f.step(ctx);
            return;
        }
        // F12 saves a screenshot at any time (to $RR_SHOT_DIR or the working directory)
        let mut trigger = ctx.input(|i| i.key_pressed(Key::F12));
        // UI tests: touching $RR_SHOT_DIR/trigger takes a screenshot
        if let Ok(dir) = std::env::var("RR_SHOT_DIR") {
            let f = std::path::Path::new(&dir).join("trigger");
            if f.exists() {
                let _ = std::fs::remove_file(&f);
                trigger = true;
            }
        }
        if trigger {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        if self.args.screenshot.is_some() {
            self.shot_state += 1;
            // RR_OPEN=stats[:tab],... opens windows for headless screenshots of them
            if self.shot_state == 1
                && let Ok(v) = std::env::var("RR_OPEN")
            {
                for name in v.split(',') {
                    self.debug_open(name);
                }
            }
            if self.shot_state == 8 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
            if self.shot_state > 200 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        let shot = ctx.input(|i| {
            i.events
                .iter()
                .find_map(|e| if let egui::Event::Screenshot { image, .. } = e { Some(image.clone()) } else { None })
        });
        if let Some(img) = shot {
            let path = match &self.args.screenshot {
                Some(p) => p.clone(),
                None => {
                    let dir = std::env::var("RR_SHOT_DIR").unwrap_or_else(|_| ".".into());
                    self.shot_state += 1;
                    format!("{dir}/shot_{:03}_gf{}.png", self.shot_state, self.sim.gf().0)
                }
            };
            let [w, h] = img.size;
            let mut buf = image::RgbaImage::new(w as u32, h as u32);
            for (i, px) in img.pixels.iter().enumerate() {
                buf.put_pixel((i % w) as u32, (i / w) as u32, image::Rgba(px.to_array()));
            }
            match buf.save(&path) {
                Ok(()) => eprintln!("saved {path} ({w}x{h}) at gf {}", self.sim.gf().0),
                Err(e) => eprintln!("screenshot {path}: {e}"),
            }
            if self.args.screenshot.is_some() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
}

impl eframe::App for App {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.finish_save(true);
        self.save_replay();
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // the map view: the window minus the bottom bar
        self.screen = ctx.screen_rect().size() - Vec2::new(0.0, 40.0);
        focus_ring(ctx);
        if let Some(a) = self.audio.as_mut() {
            a.background = !ctx.input(|i| i.focused);
        }
        self.keep_settings(ctx);
        self.finish_save(false);
        if self.tex.is_none() {
            self.tex = Some(TerrainTextures::new(ctx, self.sim.world().landscape));
        }
        if self.in_menu {
            // a network game runs on behind the menu
            if self.net.is_some() {
                self.advance();
            }
            crate::menu::show(self, ctx);
            if let Some(a) = self.audio.as_mut() {
                a.update(&self.sim, self.player);
            }
            self.screenshot_logic(ctx);
            // a network game runs on behind the menu; else the menu only needs a slow tick
            if self.net.is_some() || self.lobby.is_some() {
                ctx.request_repaint();
            } else {
                ctx.request_repaint_after(IDLE_TICK);
            }
            return;
        }
        // the map editor shows its map, no game runs
        // while a brush is dragged the preview follows ten times a second, not every frame
        // (each rebuild is a whole game setup)
        let dragging = ctx.input(|i| i.pointer.primary_down());
        if let Some(ed) = self.editor.as_mut()
            && ed.stale
            && (!dragging || ed.previewed.is_none_or(|t| t.elapsed().as_millis() >= 100))
        {
            ed.stale = false;
            ed.previewed = Some(Instant::now());
            let landscape = ed.map.landscape;
            self.sim = ed.preview();
            self.minimap = None;
            if self.tex.as_ref().is_some_and(|t| t.landscape != landscape) {
                self.tex = Some(TerrainTextures::new(ctx, landscape));
            }
        }
        if self.args.screenshot.is_none() && self.editor.is_none() {
            self.advance();
        }
        let time = self.start.elapsed().as_secs_f32();
        if self.editor.is_none() {
            ui::panels(self, ctx);
        }
        self.inset_jump(ctx);
        let mut heard = (Vec::new(), 0.0);
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| {
            let rect = ui.max_rect();
            let resp = ui.allocate_rect(rect, egui::Sense::click_and_drag());
            self.map_input(ctx, &resp, rect);
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(10, 20, 40));
            track_idle(&mut self.idle, &self.sim);
            self.life.calm = self.calm_map;
            self.life.track(&self.sim, self.session.favour);
            // (RR_FRAMES: the map's clock is the film's too, so each frame is the same each run)
            let view = self.view(rect, self.frames.as_ref().map_or(time, |f| f.idx as f32 / f.fps));
            if let Some(tex) = &self.tex {
                let t = Instant::now();
                view.draw(&painter, tex);
                heard = crate::ambience::heard(&view);
                view.draw_shots(&painter, &self.session.shots);
                view.draw_priorities(&painter);
                let open: Vec<MapCoord> =
                    self.ui.action.iter().map(|a| a.node).chain(self.ui.kept.iter().copied()).collect();
                view.draw_catapult_reach(&painter, &open);
                view.draw_harbor_spots(&painter, &self.harbor_spots_shown(&open));
                // RR_GALLERY=1: every animal and figure drawn big over the map (headless
                // screenshots for art work)
                if std::env::var_os("RR_GALLERY").is_some() {
                    let t = self.frames.as_ref().map_or(time, |f| f.idx as f32 / f.fps);
                    let names = crate::sprites::gallery(&painter, rect, t);
                    if let Some(f) = self.frames.as_ref().filter(|f| f.idx == 0) {
                        let _ = std::fs::write(format!("{}/cells.txt", f.dir), names.join("\n"));
                    }
                }
                // RR_DRAW_TIMES=1: how long building the map's shapes takes (for tuning)
                if std::env::var_os("RR_DRAW_TIMES").is_some() {
                    eprintln!("draw {:.2} ms", t.elapsed().as_secs_f64() * 1000.0);
                }
            }
            // a pause dims the map (art director round 4, #7: a bright, moving map read as
            // nothing happened)
            if self.paused && self.net.is_none() && self.playback.is_none() && !self.ui.briefing {
                painter.rect_filled(rect, 0.0, egui::Color32::from_black_alpha(crate::ui::PAUSE_DIM));
            }
            // the building Go there went to: a ring that fades (new-player round 10, #9)
            if let Some((p, since)) = self.mark {
                let a = crate::ui::mark_alpha((ctx.input(|i| i.time) - since) as f32);
                if a > 0.0 {
                    let at = view.node_screen(p);
                    let c = egui::Color32::from_rgba_unmultiplied(255, 80, 60, (a * 255.0) as u8);
                    painter.circle_stroke(at, 28.0 * self.cam.zoom.max(0.6), egui::Stroke::new(3.0_f32, c));
                    ctx.request_repaint();
                }
            }
            self.pointer_marks(&view, &painter, &resp);
            if let Some(c) = self.session.cursor {
                let at = view.node_screen(c);
                let r = 10.0 * self.cam.zoom;
                painter.circle_stroke(at, r, egui::Stroke::new(4.0_f32, egui::Color32::BLACK));
                painter.circle_stroke(at, r, egui::Stroke::new(2.0_f32, egui::Color32::WHITE));
            }
            crate::editor::overlay(self, &painter, &view);
        });
        if self.editor.is_some() {
            crate::editor::panel(self, ctx);
            self.screenshot_logic(ctx);
            // the editor changes on input, which repaints by itself
            ctx.request_repaint_after(IDLE_TICK);
            return;
        }
        ui::windows(self, ctx);
        // (war music for 3 minutes after an attack, not the alarm's 10: it covered most of a
        // long game; sound designer round 3, #8)
        let at_war = self.session.last_attacked.is_some_and(|a| self.sim.gf().0 < a + minutes(3));
        if let Some(a) = self.audio.as_mut() {
            let center = ctx.screen_rect().center();
            let node = crate::render::View {
                sim: &self.sim,
                cam: self.cam,
                rect: ctx.screen_rect(),
                render_gf: 0.0,
                player: self.player,
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
                reveal_all: false,
                scenery: true,
                fog_cache: Default::default(),
                life: None,
            }
            .pick(center);
            // half the view's width in nodes: the zoom decides how far one hears
            let half = self.screen.x / 2.0 / (crate::render::TW * self.cam.zoom);
            a.set_listener(node, self.sim.world().grid(), half);
            a.zoom = self.cam.zoom;
            a.set_war(at_war);
            a.speed = (SPEEDS[NORMAL_SPEED] / SPEEDS[self.speed]) as f32;
            a.fog_of_war = self.fog_of_war;
            a.play_life(&heard.0, heard.1);
            a.update(&self.sim, self.player);
        }
        self.screenshot_logic(ctx);
        // a paused game redraws on input and on a slow tick (no full-speed redraws for nothing)
        if self.paused && self.net.is_none() {
            ctx.request_repaint_after(IDLE_TICK);
        } else {
            ctx.request_repaint();
        }
    }
}

/// How often an idle screen (menu, editor, paused game) is redrawn without input.
const IDLE_TICK: std::time::Duration = std::time::Duration::from_millis(100);

/// Join a network close by (a road of `join` steps) rather than build the road to the flag
/// that was clicked (`direct` steps, if possible at all)? When it is at most half as long.
fn prefer_join(direct: Option<usize>, join: usize) -> bool {
    direct.is_none_or(|d| join * 2 <= d)
}

/// Where a key moves the keyboard cursor: left/right along the row, up/down along the column
/// (odd rows sit half a node to the right, so the step alternates), the diagonals on Home /
/// PageUp / End / PageDown or the number pad.
fn cursor_step(g: rr_core::Grid, p: MapCoord, k: Key) -> Option<MapCoord> {
    let odd = p.y & 1 == 1;
    let d = match k {
        Key::ArrowLeft => Direction::W,
        Key::ArrowRight => Direction::E,
        Key::ArrowUp => {
            if odd {
                Direction::NW
            } else {
                Direction::NE
            }
        }
        Key::ArrowDown => {
            if odd {
                Direction::SW
            } else {
                Direction::SE
            }
        }
        // (the number pad's 7, 9, 1, 3 without Num Lock)
        Key::Home => Direction::NW,
        Key::PageUp => Direction::NE,
        Key::End => Direction::SW,
        Key::PageDown => Direction::SE,
        _ => return None,
    };
    Some(g.neighbor(p, d))
}

/// A bright ring around the widget with keyboard focus (egui's own outline is faint on the
/// brown windows). Drawn where it was last frame.
fn focus_ring(ctx: &egui::Context) {
    let Some(r) = ctx.memory(|m| m.focused()).and_then(|id| ctx.read_response(id)) else { return };
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("focus_ring")));
    painter.rect_stroke(
        r.rect.expand(2.0),
        3.0,
        egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(255, 235, 110)),
        egui::StrokeKind::Outside,
    );
}

/// The keys that work in the map editor too: zoom, fullscreen, Esc, Space (building room). The game's keys (H to the
/// headquarters, F5 quick save, F9 quick load...) would act on the editor's player-less preview.
fn editor_key(keys: &crate::keys::Keys, k: Key) -> bool {
    use crate::keys::Act;
    k == Key::Escape || [Act::Zoom, Act::Fullscreen, Act::BuildHelp].iter().any(|a| keys.key(*a) == k)
}

/// Where a player's view starts: the headquarters, or once it is lost the player's oldest
/// building, or the middle of the map (never the black corner at 0,0).
fn home_view(sim: &Sim, player: u8) -> Option<MapCoord> {
    // RR_LOOK=x,y looks at that node instead (headless screenshots of a given place)
    if let Some(p) = std::env::var("RR_LOOK").ok().and_then(|v| {
        let (x, y) = v.split_once(',')?;
        Some(MapCoord::new(x.trim().parse().ok()?, y.trim().parse().ok()?))
    }) {
        return Some(p);
    }
    sim.hq_pos(player)
        .or_else(|| sim.buildings().iter().find(|(_, b)| b.player == player).map(|(_, b)| b.pos))
        // somewhere the player has seen: the middle of their explored land
        .or_else(|| {
            let g = sim.world().grid();
            let seen: Vec<MapCoord> =
                g.coords().filter(|p| sim.visibility(player, *p) != rr_sim::Visibility::Unexplored).collect();
            let n = seen.len() as u32;
            (n > 0).then(|| {
                let (sx, sy) = seen.iter().fold((0u32, 0u32), |a, p| (a.0 + p.x as u32, a.1 + p.y as u32));
                let mid = MapCoord::new((sx / n) as u16, (sy / n) as u16);
                // the explored node nearest that middle (the middle itself may be unexplored)
                *seen.iter().min_by_key(|p| g.distance(**p, mid)).unwrap_or(&mid)
            })
        })
        .or_else(|| {
            let g = sim.world().grid();
            Some(MapCoord::new(g.width / 2, g.height / 2))
        })
}

/// The screen direction of a pan key (arrows, or W/A/S/D by position).
fn pan_dir(k: Key) -> Option<Vec2> {
    Some(match k {
        Key::ArrowLeft | Key::A => Vec2::new(-1.0, 0.0),
        Key::ArrowRight | Key::D => Vec2::new(1.0, 0.0),
        Key::ArrowUp | Key::W => Vec2::new(0.0, -1.0),
        Key::ArrowDown | Key::S => Vec2::new(0.0, 1.0),
        _ => return None,
    })
}

/// Letters typed this frame on the W/A/S/D positions of another layout (AZERTY gives Z, Q):
/// those keys pan, so their letters are no hotkeys.
fn shadowed_by_pan(events: &[egui::Event]) -> Vec<Key> {
    events
        .iter()
        .filter_map(|e| match e {
            egui::Event::Key { key, physical_key: Some(p), .. }
                if key != p && matches!(p, Key::W | Key::A | Key::S | Key::D) =>
            {
                Some(*key)
            }
            _ => None,
        })
        .collect()
}

#[cfg(test)]
use rr_ai::MISSION_TRUCE;

/// The computer players of a new game (all but the human's slot unless the computer plays it
/// too). In a mission they spare player 0 at first, so the first guardhouses can stand.
fn make_ais(args: &crate::Args, players: usize) -> Vec<rr_ai::Ai> {
    let level = crate::ai_level(args);
    let mut ais: Vec<rr_ai::Ai> = (0..players as u8)
        .filter(|p| args.auto || *p != args.me)
        .map(|p| {
            let mut ai = rr_ai::Ai::with_level(p, crate::level_of(args, p));
            // a mission's truce: an hour on easy, half an hour else, for good with a peaceful rival
            if let Some(m) = args.mission.as_deref().and_then(crate::missions::mission)
                && !args.auto
                && let Some(t) = rr_game::mission_truce(m, level, p)
            {
                ai.truce = Some((0, t.gf()));
            }
            ai
        })
        .collect();
    arm_invader(&mut ais, args);
    ais
}

/// The computer players as a new game gets them, for whole-game tests of the missions.
#[cfg(test)]
pub(crate) fn make_ais_for_test(args: &crate::Args, players: usize) -> Vec<rr_ai::Ai> {
    make_ais(args, players)
}

/// The hover's words in a small box by the pointer, on the side with room (our own, not egui's
/// tooltip: that one stayed away after a click until the pointer left the map, and ran off the
/// screen's edge; owner, 2026-10-27). The first line, what stands there, in bold.
fn hover_box(ctx: &egui::Context, pos: egui::Pos2, lines: &[String]) {
    let screen = ctx.screen_rect();
    let (left, up) = (pos.x > screen.right() - 300.0, pos.y > screen.bottom() - 150.0);
    let pivot = match (left, up) {
        (false, false) => egui::Align2::LEFT_TOP,
        (true, false) => egui::Align2::RIGHT_TOP,
        (false, true) => egui::Align2::LEFT_BOTTOM,
        (true, true) => egui::Align2::RIGHT_BOTTOM,
    };
    let at = pos + Vec2::new(if left { -14.0 } else { 18.0 }, if up { -14.0 } else { 20.0 });
    egui::Area::new(egui::Id::new("map_hover"))
        .order(egui::Order::Tooltip)
        .interactable(false)
        .pivot(pivot)
        .fixed_pos(at)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_max_width(260.0);
                for (i, l) in lines.iter().enumerate() {
                    let text = egui::RichText::new(l);
                    ui.label(if i == 0 && lines.len() > 1 { text.strong() } else { text.small() });
                }
            });
        });
}

/// The favour one offered ware gives its god (Juno: 5 breads, 20 points), raised as the
/// offering's event comes (`ui::holy_window` makes the offering).
pub const OFFER_FAVOUR: u32 = 4;

/// The favour at which Concordia makes the chapter's goal one building smaller.
pub const FAVOUR_EASIER: u32 = 50;

/// How long the pointer rests on the map before the hover's words show (the marker shows at
/// once; the words at egui's half second came up on every pause of the mouse).
const HOVER_WORDS_SECS: f32 = 1.2;

/// A gallery filmed frame by frame (`App::frames`).
struct FrameRun {
    dir: String,
    count: u32,
    fps: f32,
    /// The frame being drawn; frames before `WARM` let the window settle.
    idx: u32,
    warm: u32,
    pending: bool,
}

impl FrameRun {
    const WARM: u32 = 8;

    fn from_env() -> Option<Self> {
        let v = std::env::var("RR_FRAMES").ok()?;
        let mut it = v.rsplitn(3, ':');
        let fps = it.next()?.parse().ok()?;
        let count = it.next()?.parse().ok()?;
        let dir = it.next()?.to_string();
        Some(Self { dir, count, fps, idx: 0, warm: 0, pending: false })
    }

    /// Ask for this frame's picture, save the one that came, close after the last.
    fn step(&mut self, ctx: &egui::Context) {
        ctx.request_repaint();
        if self.warm < Self::WARM {
            self.warm += 1;
            return;
        }
        let shot = ctx.input(|i| {
            i.events
                .iter()
                .find_map(|e| if let egui::Event::Screenshot { image, .. } = e { Some(image.clone()) } else { None })
        });
        if let Some(img) = shot {
            let [w, h] = img.size;
            let mut buf = image::RgbaImage::new(w as u32, h as u32);
            for (i, px) in img.pixels.iter().enumerate() {
                buf.put_pixel((i % w) as u32, (i / w) as u32, image::Rgba(px.to_array()));
            }
            let _ = buf.save(format!("{}/frame_{:04}.png", self.dir, self.idx));
            self.idx += 1;
            self.pending = false;
            if self.idx >= self.count {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        if !self.pending && self.idx < self.count {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            self.pending = true;
        }
    }
}

/// Keep `idle` to the carriers standing idle on their roads now, each with the frame it began
/// (a carrier that moves or takes a ware starts over).
pub(crate) fn track_idle(idle: &mut std::collections::HashMap<rr_sim::FigId, u32>, sim: &rr_sim::Sim) {
    let gf = sim.gf().0;
    let waiting = |f: &rr_sim::Figure| {
        matches!(f.kind, rr_sim::FigKind::Carrier { .. })
            && f.step == rr_sim::Step::CIdle
            && f.motion == rr_sim::Motion::Idle
            && f.carry.is_none()
    };
    idle.retain(|id, _| sim.figures().get(*id).is_some_and(waiting));
    for (id, f) in sim.figures().iter() {
        if waiting(f) {
            idle.entry(id).or_insert(gf);
        }
    }
}

/// The human's military settings at the start of a game they haven't mastered yet: the
/// middle manned like the border (new-player round 7, #1); in a gentle mission whose rivals all
/// keep the peace, one soldier a building everywhere, as nobody will attack (the Roman
/// campaign's first chapter: full garrisons spent the 52 soldiers of the start at 30 % of the
/// land, there being no ore for weapons).
pub(crate) fn newcomer_military(mut values: [u8; 8], mission: Option<&crate::missions::Mission>) -> [u8; 8] {
    use crate::missions::Objective as O;
    let peace = mission.is_some_and(|m| m.gentle && m.cast.iter().all(|s| *s == crate::missions::Stance::Peaceful));
    // (only where the goal is land: the Sabines' chapter has its war)
    if peace && mission.is_some_and(|m| matches!(m.objective, O::HoldMapLand(_) | O::HoldLand(_))) {
        values[4] = 0;
        values[5] = 0;
        values[7] = 0;
    } else {
        values[5] = values[5].max(values[7]);
    }
    values
}

/// A mission's invader knows the player's coast, and its rivals' land cap (not saved with the
/// AIs: set again on load).
fn arm_invader(ais: &mut [rr_ai::Ai], args: &crate::Args) {
    let invader =
        args.mission.as_deref().and_then(crate::missions::mission).and_then(|m| m.invader_at(crate::ai_level(args)));
    // (and the free game's peace time, which they know of: veteran round 11, #3)
    let peace = if args.mission.is_none() { peace_gf(args.peace) } else { 0 };
    // a mission's rivals leave room for its land goal (not saved with the AIs either)
    let cap = args.mission.as_deref().and_then(crate::missions::mission).map_or(0, |m| m.land_cap);
    for ai in ais.iter_mut() {
        ai.invade = (Some(ai.player) == invader && !args.auto).then_some(0);
        ai.peace_until = peace;
        ai.land_cap = (cap > 0).then_some(cap);
    }
}

/// When the truce with player `me` ends: the last of the rivals that come at all (a peaceful
/// rival's truce lasts for good and isn't one to count down: the briefing said 1789569 minutes).
pub(crate) fn truce_end(ais: &[rr_ai::Ai], me: u8) -> Option<u32> {
    ais.iter()
        .filter_map(|a| a.truce)
        .filter(|(p, _)| *p == me)
        .filter_map(|(_, u)| match Truce::of(u) {
            Truce::Until(u) => Some(u),
            Truce::ForGood => None,
        })
        .max()
}

pub use rr_game::Truce;

/// Until when the computer player `rival` spares player `me`, if that truce still runs at `gf`.
fn truce_against(ais: &[rr_ai::Ai], me: u8, rival: u8, gf: u32) -> Option<u32> {
    ais.iter()
        .find(|a| a.player == rival)
        .and_then(|a| a.truce)
        .filter(|(p, until)| *p == me && gf < *until)
        .map(|(_, until)| until)
}

/// The player `me` attacks `rival`: the truce of the rival and its allies with `me` ends now
/// (game design round 1, #8: the truce bound only the rivals). Who it ended for.
fn end_truce(ais: &mut [rr_ai::Ai], sim: &rr_sim::Sim, me: u8, rival: u8, gf: u32) -> Vec<u8> {
    let mut ended = Vec::new();
    for i in 0..ais.len() {
        let p = ais[i].player;
        if sim.are_allies(p, rival) && truce_against(ais, me, p, gf).is_some() {
            ais[i].truce = Some((me, gf));
            ended.push(p);
        }
    }
    ended
}

/// One frame of the camera following a settler: a glide toward it, or a jump when it is far
/// (across the map's wrap, or after a ship's voyage).
fn follow_step(cur: Vec2, target: Vec2, dt: f32) -> Vec2 {
    if (target - cur).length() > 1500.0 { target } else { cur + (target - cur) * (dt * 4.0).min(1.0) }
}

pub(crate) use rr_game::{MINUTE, minutes};

/// Peace for good, as a peace time in minutes: nobody attacks in the whole game.
pub(crate) const PEACE_FOR_GOOD: u32 = u32::MAX;

/// The frame a peace of `minutes` ends.
pub(crate) fn peace_gf(minutes: u32) -> u32 {
    if minutes == PEACE_FOR_GOOD { u32::MAX } else { self::minutes(minutes) }
}

/// How a goal ends the game: a mission's goal met (it may go on), won (counted as a mission
/// won, or a free game's coins or time), or lost.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Ends {
    Goal,
    Won { mission: bool },
    Lost,
}

/// What a goal decided this frame, and the game's last word (a stored message).
pub(crate) struct Outcome {
    pub kind: Ends,
    pub text: String,
}

/// The one goal engine (architect round 3, #5): the mission's own goal first; then the free
/// game's coins (whoever first minted them: RTTR's economy mode in small, PLAN §27, #12) and time
/// limit (the side with the most land: RTTR's tournament modes); then all rivals beaten, or the
/// land share (60 or 75 %: three quarters alone was never reached in 40 probe games, design
/// round 1, #6). Not for replays or a game already over: the caller asks.
pub(crate) fn outcome(
    sim: &rr_sim::Sim,
    me: u8,
    args: &crate::Args,
    objective: Option<crate::missions::Objective>,
    goal_done: bool,
) -> Option<Outcome> {
    use crate::i18n::{msg, tr};
    let alive = sim.players()[me as usize].alive;
    let ends = |kind, text: String| Some(Outcome { kind, text });
    if !goal_done && alive && objective.is_some_and(|o| crate::missions::progress(o, sim, me).met) {
        return ends(Ends::Goal, tr("Goal reached! The mission is won.").into());
    }
    let name = |w: u8| rr_game::plain(&sim.players()[w as usize].setup.name);
    if args.coins_goal > 0
        && let Some(w) = first_to(&coins_minted(sim), args.coins_goal)
    {
        let n = args.coins_goal.to_string();
        return if sim.are_allies(w, me) {
            ends(Ends::Won { mission: false }, msg("You minted {1} gold coins first: victory.", &[&n]))
        } else {
            ends(Ends::Lost, msg("{1} minted {2} gold coins first.", &[&name(w), &n]))
        };
    }
    if args.time_limit > 0
        && sim.gf().0 >= minutes(args.time_limit)
        && let Some((w, tie)) = most_land(sim)
    {
        return if tie {
            ends(Ends::Lost, tr("Time is up! The land is shared equally: a draw.").into())
        } else if sim.are_allies(w, me) && alive {
            ends(Ends::Won { mission: false }, tr("Time is up! You hold the most land: victory.").into())
        } else {
            ends(Ends::Lost, msg("Time is up! {1} holds the most land.", &[&name(w)]))
        };
    }
    if !checks_outcome(false, false, sim.players().len()) || !alive {
        return None;
    }
    if crate::missions::rivals(sim, me).is_empty() {
        return ends(Ends::Won { mission: true }, tr("Victory! All enemies are defeated.").into());
    }
    if args.land_goal > 0 && crate::missions::land_share(sim, me) >= args.land_goal {
        let goal = args.land_goal.to_string();
        return ends(Ends::Won { mission: true }, msg("Victory! You hold {1} % of the land.", &[&goal]));
    }
    None
}

/// The first player whose count reached `goal`, if any.
fn first_to(counts: &[u32], goal: u32) -> Option<u8> {
    counts.iter().position(|c| *c >= goal).map(|p| p as u8)
}

/// What the small stories in the news look for (`App::story_news`).
struct Stories {
    coin: bool,
    general: bool,
    ship: bool,
}

/// The stories due now: (bit, text, kind) of those that happened and weren't told (`told`).
fn new_stories(now: &Stories, told: u32) -> Vec<(u32, &'static str, u8)> {
    [
        (
            1,
            now.coin,
            "The first gold coin is minted: coins promote the soldiers of the military buildings they reach.",
            MSG_ECONOMY,
        ),
        (
            2,
            now.general,
            "A general at last, your strongest soldier. Send generals where the fighting is.",
            MSG_MILITARY,
        ),
        (
            4,
            now.ship,
            "Your first ship is on the water: harbors can now ship wares across the sea and send out expeditions.",
            MSG_ECONOMY,
        ),
    ]
    .into_iter()
    .filter(|(bit, happened, _, _)| *happened && told & bit == 0)
    .map(|(bit, _, text, kind)| (bit, text, kind))
    .collect()
}

/// Whether `me` has a military building besides the castle (built or being built).
fn has_posts(sim: &rr_sim::Sim, me: u8) -> bool {
    sim.buildings().iter().any(|(_, b)| {
        b.player == me && b.kind != rr_rules::BuildingType::Headquarters && sim.rules().building(b.kind).is_military()
    })
}

/// The five-minute truce warning when no building lacks soldiers: no posts at all yet (new-player
/// round 12, #3: it said the border buildings were full), the border setting low, or all full.
fn truce_warning(posts: bool, border_low: bool) -> &'static str {
    if !posts {
        "The truce ends in 5 minutes, and you have no military building yet: build guardhouses or barracks toward your rivals."
    } else if border_low {
        "The truce ends in 5 minutes. Man your border buildings: Main selection, Military, troops at the border."
    } else {
        "The truce ends in 5 minutes. Your border buildings already take all the soldiers they can: watchtowers and fortresses hold more, and beer, swords and shields make new soldiers."
    }
}

/// The gold coins each player has minted so far.
pub(crate) fn coins_minted(sim: &rr_sim::Sim) -> Vec<u32> {
    sim.players().iter().map(|p| p.stats.produced_by_ware[rr_rules::Ware::Coins.index()]).collect()
}

// The bits of `tips_shown` (they travel in the save): the tips list takes bits 0 to
// `LIST_TIPS - 1`, then the named ones (architect round 3, #9).
const LIST_TIPS: usize = 29;
/// The castle tip.
const CASTLE_TIP: u32 = 1 << 29;
/// "The truce ends in 5 minutes" was told.
const TRUCE_WARNED: u32 = 1 << 30;
/// "The truce is over" was told (or needs no telling: the player broke it).
const TRUCE_OVER_TOLD: u32 = 1 << 31;

/// Has an attack of `me` on the building at `at` ended without taking it: the building still
/// someone else's, and no soldier of ours on the way to it or waiting at it.
fn attack_failed(sim: &rr_sim::Sim, me: u8, at: MapCoord) -> bool {
    let Some(t) = sim.building_at(at) else { return false };
    if sim.building(t).player == me {
        return false;
    }
    !sim.figures().iter().any(|(_, f)| {
        f.player == me
            && matches!(
                f.kind,
                rr_sim::FigKind::Soldier(
                    rr_sim::SoldierTask::Attack { target, .. } | rr_sim::SoldierTask::AtTarget { target, .. }
                ) if target == t
            )
    })
}

thread_local! {
    /// The save list as last read (`App::list_saves`).
    static SAVE_LIST: std::cell::RefCell<Option<(Instant, Vec<SaveEntry>)>> = const { std::cell::RefCell::new(None) };
}

/// A save on its way to its file (`App::write_in_background`).
struct PendingSave {
    thread: std::thread::JoinHandle<Result<(), SaveFail>>,
    path: std::path::PathBuf,
    /// The game frame saved, and of which game (`App::game_no`).
    gf: u32,
    game_no: u32,
    /// The LAN autosave: quiet when it fails.
    lan: bool,
}

/// Why a save didn't reach its file.
enum SaveFail {
    /// It wouldn't load: the last good save stays.
    Unloadable(String),
    Write(String),
}

/// Would these save bytes load again? (Checked before a save replaces a file: a broken save
/// once replaced the last good autosave; net round 5, #1.)
fn loads_back(bytes: &[u8]) -> Result<(), String> {
    rr_ai::load_game_with(crate::rules(), bytes).map(|_| ())
}

/// With one or two rivals left of more, where their posts and warehouses stand (game design round 1,
/// #5: a beaten realm's last storehouse took an hour to find); with more, nothing.
pub(crate) fn holdouts(sim: &rr_sim::Sim, me: u8) -> Vec<(u8, MapCoord)> {
    let all: Vec<u8> = (0..sim.players().len() as u8).filter(|p| !sim.are_allies(*p, me)).collect();
    let rivals = crate::missions::rivals(sim, me);
    // (only once a rival has fallen: two rivals from the start still leave a map to explore)
    if !(1..=2).contains(&rivals.len()) || rivals.len() == all.len() {
        return Vec::new();
    }
    sim.buildings()
        .iter()
        .filter(|(_, b)| {
            rivals.contains(&b.player)
                && b.is_ready()
                && (rr_sim::is_frontier_military(b.kind) || b.kind.is_warehouse())
        })
        .map(|(_, b)| (b.player, b.pos))
        .collect()
}

/// A player of the side (allies together) holding the most land, if anybody holds any.
/// Also whether another side holds as much: a draw (QA round 9, #5: a tie went to the lower slot).
fn most_land(sim: &rr_sim::Sim) -> Option<(u8, bool)> {
    let land = crate::missions::land_by_player(sim);
    let n = land.len() as u8;
    let side = |p: u8| (0..n).filter(|q| sim.are_allies(p, *q)).map(|q| land[q as usize]).sum::<u32>();
    let w = (0..n).filter(|p| side(*p) > 0).max_by_key(|p| (side(*p), std::cmp::Reverse(*p)))?;
    Some((w, (0..n).any(|q| !sim.are_allies(q, w) && side(q) == side(w))))
}

/// The slot no computer player plays: the human's (a loaded game knows it from its AIs).
fn human_player(players: usize, ais: &[rr_ai::Ai]) -> u8 {
    (0..players as u8).find(|p| !ais.iter().any(|a| a.player == *p)).unwrap_or(0)
}

/// How many times more land the median rival holds, when it's at least twice as much.
fn behind(mine: u32, rivals: &[u32]) -> Option<u32> {
    if mine == 0 || rivals.is_empty() {
        return None;
    }
    let mut r = rivals.to_vec();
    r.sort_unstable();
    let median = r[r.len() / 2];
    (median >= 2 * mine).then_some(median / mine)
}

/// A line of the post office: when, the stored text (`i18n::msg`), where (for Go there), and
/// its kind (`MSG_*`, the tabs).
#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    pub gf: u32,
    pub text: String,
    pub at: Option<MapCoord>,
    pub kind: u8,
}

impl Message {
    pub fn new(gf: u32, text: String, at: Option<MapCoord>, kind: u8) -> Self {
        Message { gf, text, at, kind }
    }
}

type Messages = VecDeque<Message>;

/// The interface's part of a save: messages, tips shown and how far they were read.
/// Said when a replay ends in another game than the one recorded.
const REPLAY_DIFFERS: &str = "This replay played differently from the recorded game.";

/// Where `write_safely` puts the new file before it replaces `p`.
pub fn temp_path(p: &std::path::Path) -> std::path::PathBuf {
    let mut name = p.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    p.with_file_name(name)
}

/// Write a file whole or not at all (QA round 4, #5): into a temp file, synced, then renamed
/// over the old one. A full disk or a crash midway leaves the old file as it was.
pub fn write_safely(p: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let tmp = temp_path(p);
    let res = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        std::fs::rename(&tmp, p)
    })();
    if res.is_err() && tmp.is_file() {
        let _ = std::fs::remove_file(&tmp);
    }
    res
}

/// Where flags go along a new road from `start` (new-player round 7, #6: one-click roads were
/// long stretches with one carrier): every second node where one fits, never next to the road's
/// ends or to each other.
pub fn auto_flags(sim: &rr_sim::Sim, start: MapCoord, dirs: &[rr_core::Direction]) -> Vec<MapCoord> {
    let g = sim.world().grid();
    let mut nodes = vec![start];
    for d in dirs {
        nodes.push(g.neighbor(*nodes.last().expect("start"), *d));
    }
    let n = nodes.len() - 1;
    let mut out: Vec<MapCoord> = Vec::new();
    let mut last = 0;
    for (i, p) in nodes.iter().enumerate().take(n.saturating_sub(1)).skip(2) {
        // (the ground must take a flag too: no slope or water that refuses it)
        let ground = sim.world().bq_for(*p, sim.world().owner(*p).unwrap_or(0)).allows(rr_world::Bq::Flag);
        if i >= last + 2 && ground && flag_fits(sim, *p) && out.iter().all(|f| g.distance(*f, *p) >= 2) {
            out.push(*p);
            last = i;
        }
    }
    out
}

/// Whether a flag can stand at `p` as far as other flags go (none on a neighbouring node).
pub fn flag_fits(sim: &rr_sim::Sim, p: MapCoord) -> bool {
    let g = sim.world().grid();
    sim.flag_at(p).is_none() && g.neighbors(p).iter().all(|q| sim.flag_at(*q).is_none())
}

/// Ready military buildings (not the headquarters) holding fewer soldiers than they could
/// (new-player round 7, #1: the truce warning said they were full).
pub fn undermanned(sim: &rr_sim::Sim, me: u8) -> usize {
    sim.buildings()
        .iter()
        .filter(|(_, b)| b.player == me && b.is_ready() && b.kind != rr_rules::BuildingType::Headquarters)
        .filter_map(|(_, b)| sim.rules().building(b.kind).military.as_ref().map(|m| (b, m.soldiers as usize)))
        .filter(|(b, cap)| b.soldiers.len() < *cap)
        .count()
}

/// Write what this machine knows at a desync (LAN round 3, #5): the build, the frame, the sync
/// hash and the whole state, for comparing with the other machine's report.
pub fn write_desync_report(dir: &std::path::Path, sim: &rr_sim::Sim, me: u8) -> std::io::Result<std::path::PathBuf> {
    std::fs::create_dir_all(dir)?;
    let gf = sim.gf().0;
    let p = dir.join(format!("{gf}-p{me}.txt"));
    let text = format!(
        "build {:016x}\nframe {gf}\nplayer {me}\nsync hash {:016x}\n\n{}",
        rr_net::build_id(),
        sim.sync_hash(),
        sim.dump_string()
    );
    write_safely(&p, text.as_bytes())?;
    Ok(p)
}

/// The realms a computer player takes over when a network game is left to go on alone: every
/// living one but `me` that no computer plays yet.
pub fn takeover_slots(alive: &[bool], me: u8, ai_players: &[u8]) -> Vec<u8> {
    (0..alive.len() as u8).filter(|p| *p != me && alive[*p as usize] && !ai_players.contains(p)).collect()
}

/// Whether this frame looks for victory: a running game of several players, never a replay
/// (architect round 2, #4: watching one could count the last game's player as winning and set
/// the saved "won once").
fn checks_outcome(over: bool, watching_replay: bool, players: usize) -> bool {
    !over && !watching_replay && players > 1
}

/// Why saving isn't possible now (QA round 4, #1), if it isn't.
pub fn save_refused(watching_replay: bool) -> Option<&'static str> {
    watching_replay.then_some("Replays can't be saved.")
}

/// Why a game didn't load, in plain words.
fn load_error(e: &str, missing: bool) -> String {
    if missing {
        crate::i18n::tr("No saved game there yet. Press F5 during a game to make a quick save.").into()
    } else {
        format!("{} {e}", crate::i18n::tr("Load failed:"))
    }
}

/// The idle-stock tip: from minute 5, then every 20 minutes.
fn idle_tip_due(gf: u32, last: Option<u32>) -> bool {
    let min = MINUTE;
    gf >= 5 * min && last.is_none_or(|t| gf >= t + 20 * min)
}

/// The goals after First steps: food for the mines, the mines, weapons and beer for soldiers.
pub fn next_steps(sim: &rr_sim::Sim, me: u8) -> Vec<(&'static str, bool)> {
    use rr_rules::BuildingType as B;
    let mine = |k: B| sim.buildings().iter().filter(|(_, b)| b.player == me && b.kind == k).count();
    vec![
        ("A farm and a well", mine(B::Farm) >= 1 && mine(B::Well) >= 1),
        ("A mill and a bakery", mine(B::Mill) >= 1 && mine(B::Bakery) >= 1),
        ("A coal mine and an iron mine", mine(B::CoalMine) >= 1 && mine(B::IronMine) >= 1),
        // (beer before weapons: weapons without beer make no soldier; new-player round 8, #6)
        ("A brewery", mine(B::Brewery) >= 1),
        ("An iron smelter and an armory", mine(B::IronSmelter) >= 1 && mine(B::Armory) >= 1),
        // the road to victory (new-player round 9, #4: nothing taught attacking); with nobody
        // in reach, finding them comes first (new-player round 10, #4: two hours in Mare Nostrum
        // with no rival in sight)
        if rivals_in_reach(sim, me) {
            (
                "Take a rival post: click it, then ⚔ Attack",
                sim.players()[me as usize].stats.samples.last().is_some_and(|s| s.vanquished > 0),
            )
        } else {
            ("Find your rivals: grow toward them, or reach them by sea (a harbor, a shipyard, an expedition)", false)
        },
    ]
}

/// Whether a rival's post or warehouse lies within 40 nodes of one of ours.
fn rivals_in_reach(sim: &rr_sim::Sim, me: u8) -> bool {
    let g = sim.world().grid();
    let holds =
        |b: &rr_sim::Building| b.is_ready() && (b.kind.is_warehouse() || sim.rules().building(b.kind).is_military());
    let ours: Vec<MapCoord> =
        sim.buildings().iter().filter(|(_, b)| b.player == me && holds(b)).map(|(_, b)| b.pos).collect();
    sim.buildings()
        .iter()
        .filter(|(_, b)| b.player != me && !sim.are_allies(b.player, me) && holds(b))
        .any(|(_, b)| ours.iter().any(|p| g.distance(*p, b.pos) <= 40))
}

/// Building material piles up while little is built (new-player round 6, #5).
/// What to do about being behind: grow, or while under pressure hold what there is (new-player
/// round 8, #3: "Grow" during a collapse spread the last soldiers thinner).
/// Attacked lately: hold; few soldiers in store but no attack: make soldiers first (new-player
/// round 9, #7: "facing your attackers" when nobody had attacked).
fn standing_advice(attacked: bool, few_soldiers: bool) -> &'static str {
    if attacked {
        "Hold what you have: a watchtower or fortress facing your attackers, and no new guardhouses until soldiers wait in store (beer, a sword and a shield make one)."
    } else if few_soldiers {
        "Make soldiers before you grow: beer, a sword and a shield make one in the castle. New military buildings now would stand empty."
    } else {
        "Grow: put military buildings at the edge of your land (Space shows where), keep wood and stone coming, and feed miners for iron and gold."
    }
}

fn idle_stock(boards: u32, stones: u32, sites: usize) -> bool {
    boards > 30 && stones > 30 && sites < 2
}

/// The first game's goals, ticked off as they are reached (new-player round 6, #5).
/// The building of a First step (new-player round 12, #8: the build window opened on Large,
/// a tab away from the woodcutter the steps ask for).
fn first_step_building(step: &str) -> Option<rr_rules::BuildingType> {
    use rr_rules::BuildingType as B;
    match step {
        "Two woodcutters" => Some(B::Woodcutter),
        "A forester" => Some(B::Forester),
        "A quarry" => Some(B::Quarry),
        "A sawmill" => Some(B::Sawmill),
        "Three military buildings" => Some(B::Guardhouse),
        "A fishery or a hunter" => Some(B::Hunter),
        _ => None,
    }
}

pub fn first_steps(sim: &rr_sim::Sim, me: u8) -> Vec<(&'static str, bool)> {
    use rr_rules::BuildingType as B;
    // a step is taken once the building stands or its site has a road (new-player round 11, #6:
    // two woodcutters with no road were ticked for five minutes while nothing was built)
    let counts = |b: &rr_sim::Building| b.is_ready() || sim.flag(b.flag).roads.iter().any(Option::is_some);
    let mine = |k: B| sim.buildings().iter().filter(|(_, b)| b.player == me && b.kind == k && counts(b)).count();
    let military = sim
        .buildings()
        .iter()
        .filter(|(_, b)| {
            b.player == me && b.kind != B::Headquarters && sim.rules().building(b.kind).is_military() && counts(b)
        })
        .count();
    vec![
        ("Two woodcutters", mine(B::Woodcutter) >= 2),
        ("A forester", mine(B::Forester) >= 1),
        ("A quarry", mine(B::Quarry) >= 1),
        ("A sawmill", mine(B::Sawmill) >= 1),
        ("Three military buildings", military >= 3),
        ("A fishery or a hunter", mine(B::Fishery) + mine(B::Hunter) >= 1),
        ("A mine", mine(B::CoalMine) + mine(B::IronMine) + mine(B::GoldMine) + mine(B::GraniteMine) >= 1),
    ]
}

/// The index of normal speed in `SPEEDS`.
pub(crate) const NORMAL_SPEED: usize = 2;

/// The line for a player whose game went apart: the host compared it with its own; a guest
/// only hears of it.
fn desync_line(host: bool) -> &'static str {
    if host {
        "A player's game went apart from ours (desync):"
    } else {
        "A player's game went apart from the host's (desync):"
    }
}

/// Whether a pause set during a frame's steps ends them at once.
fn stops_the_frame(paused: bool, net: bool) -> bool {
    paused && !net
}

/// Whether the game can be paused: not a network game.
fn can_pause(net: bool) -> bool {
    !net
}

/// Whether this game is a single-player mission (a network game never is, whatever was played
/// before).
fn plays_a_mission(mission: bool, map_mission: bool, net: bool) -> bool {
    !net && (mission || map_mission)
}

/// A new harbor or storehouse in a mission keeps two privates at home unless a reserve is set
/// (rivals attack empty warehouses first, RTTR hq_or_harbor_without_soldiers).
fn default_reserve(kind: rr_rules::BuildingType, mission: bool, reserve: [u8; 5]) -> Option<u8> {
    (mission
        && matches!(kind, rr_rules::BuildingType::Harbor | rr_rules::BuildingType::Storehouse)
        && reserve == [0; 5])
        .then_some(2)
}

/// A lost warehouse (castle, harbor, storehouse) is an alert, not one more line: its wares
/// and settlers go with it (new-player round 10, #2).
fn lost_is_alert(kind: rr_rules::BuildingType) -> bool {
    kind.is_warehouse()
}

/// The news of a lost building: who took it if known, and a warning when no warehouse is left.
/// The goal's castle fell to another realm.
const CASTLE_TAKEN_BY_OTHERS: &str = "{1}'s castle fell to {2}: to win the mission now, defeat all rivals.";

/// Whether a castle goal stands (`objective` met by the castle falling): unless the castle fell
/// to a realm not on our side (`ours`); not known (an older save): it stands, as before.
fn castle_goal_ours(
    objective: Option<crate::missions::Objective>,
    lost: &[(u8, u8)],
    ours: impl Fn(u8) -> bool,
) -> bool {
    match objective {
        Some(crate::missions::Objective::DefeatRival(p)) => {
            lost.iter().filter(|(q, _)| *q == p).all(|(_, by)| ours(*by))
        }
        _ => true,
    }
}

/// Land of ours that another realm's post took.
const LAND_LOST: &str = "{1} nodes of our land went to {2}: a post of theirs is nearer.";

/// What an order given in a pause says.
const PLANNED_NOTE: &str = "Planned (⚒): the work starts when the game runs again ({1}).";

/// A building of ours burned because another realm's territory took its land.
const LAND_TAKEN: &str = "{1} burned: {2}'s new territory took its land.";

fn lost_text(kind: rr_rules::BuildingType, by: Option<&str>, last: bool) -> String {
    let k = ui::building_key(kind);
    match (lost_is_alert(kind), by, last) {
        (false, _, _) => crate::i18n::msg("{1} lost", &[k]),
        (true, Some(b), false) => {
            crate::i18n::msg("{1} lost to {2}: its wares and settlers are gone with it.", &[k, b])
        }
        (true, None, false) => crate::i18n::msg("{1} lost: its wares and settlers are gone with it.", &[k]),
        (true, Some(b), true) => crate::i18n::msg(
            "{1} lost to {2}: its wares and settlers are gone with it. You have no warehouse left: nothing new can be built.",
            &[k, b],
        ),
        (true, None, true) => crate::i18n::msg(
            "{1} lost: its wares and settlers are gone with it. You have no warehouse left: nothing new can be built.",
            &[k],
        ),
    }
}

/// The saves in `dir`, and those of `extra` that exist, newest first.
fn list_saves_in(dir: &std::path::Path, extra: &[std::path::PathBuf]) -> Vec<SaveEntry> {
    let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "rrsave")).collect())
        .unwrap_or_default();
    paths.extend(extra.iter().filter(|p| p.is_file()).cloned());
    let mut v: Vec<SaveEntry> = paths.iter().filter_map(|p| save_entry(p)).collect();
    v.sort_by_key(|e| std::cmp::Reverse(e.modified));
    v
}

/// The computer players of a network game's save: its own, and one for every other living
/// human, so the save goes on as a game of one.
fn lan_save_ais(sim: &rr_sim::Sim, ais: &[rr_ai::Ai], me: u8, level: rr_ai::Level) -> Vec<rr_ai::Ai> {
    let alive: Vec<bool> = sim.players().iter().map(|p| p.alive).collect();
    let mut out = ais.to_vec();
    let taken: Vec<u8> = out.iter().map(|a| a.player).collect();
    for p in takeover_slots(&alive, me, &taken) {
        out.push(rr_ai::Ai::with_level(p, level));
    }
    out
}

/// Whether a network game this many turns behind runs to catch up (two turns are the normal
/// lead of the bundles).
fn catch_up(turns_ahead: u32) -> bool {
    turns_ahead > 2
}

/// A network game's own autosave (loaded, it goes on against computer players).
const LAN_AUTOSAVE: &str = "autosave-lan.rrsave";

/// Whether a save to `p` keeps the play since the last save safe for F9's question: any but
/// the autosave (F9 loads the quick save, not the autosave).
fn marks_play_saved(p: &std::path::Path, autosave: &std::path::Path) -> bool {
    p != autosave
}

/// Whether F9 should ask first: more than a minute of play since the last save or load.
pub(crate) fn load_needs_asking(gf: u32, saved: Option<u32>) -> bool {
    gf > saved.unwrap_or(0) + MINUTE
}

/// Every ten minutes of play the game saves itself (a11y round 2, #9: no autosave at all).
fn autosave_due(gf: u32) -> bool {
    gf > 0 && gf.is_multiple_of(minutes(10))
}

/// Whether an event is the player's own refused order (shown as a note as well as heard).
fn refusal_shown(e: &Event, me: u8) -> bool {
    matches!(e, Event::CommandRejected { player, .. } if *player == me)
}

/// An attack on us: back to normal speed if faster, unless that happened in the last minute.
fn slow_for_attack(speed: usize, slowed_at: Option<u32>, gf: u32) -> (usize, Option<u32>) {
    if speed > NORMAL_SPEED && slowed_at.is_none_or(|t| gf >= t + MINUTE) {
        (NORMAL_SPEED, Some(gf))
    } else {
        (speed, slowed_at)
    }
}

/// May "nothing in reach" for a kind be said again (last said at `last`)? Every 10 minutes.
fn nothing_due(last: Option<u32>, gf: u32) -> bool {
    last.is_none_or(|t| gf >= t + minutes(10))
}

/// Does a loss at `gf` belong to a capture at `capture` (the land's workshops burn within a few
/// frames of it)?
fn in_burn_window(capture: u32, gf: u32) -> bool {
    gf >= capture && gf <= capture + 10
}

/// After the first lost military building: the castle's soldiers only defend the castle.
fn castle_tip(sim: &rr_sim::Sim, me: u8) -> Option<String> {
    let hq = sim.players()[me as usize].hq.and_then(|h| sim.buildings().get(h))?;
    let inv = hq.inventory.as_ref()?;
    let n: u32 = rr_rules::Job::SOLDIERS.iter().map(|j| inv.job(*j)).sum();
    // (an empty castle has nothing to explain: QA round 5, #7)
    if n == 0 {
        return None;
    }
    Some(crate::i18n::msg(
        "💡 Your castle holds {1} soldiers, but they only defend the castle. A guardhouse holds 3, a watchtower 6, a fortress 9: put strong buildings where your rivals' land touches yours.",
        &[&n.to_string()],
    ))
}

/// "Germania attacks our Guardhouse", "Germania took our Guardhouse", "We took Guardhouse from
/// Germania" (stored as message templates).
fn military_news(sim: &rr_sim::Sim, me: u8, e: &Event) -> Option<String> {
    let name = |p: u8| rr_game::plain(&sim.players().get(p as usize).map(|p| p.setup.name.clone()).unwrap_or_default());
    let kind_at = |at: MapCoord| sim.building_at(at).map(|b| ui::building_key(sim.building(b).kind));
    match e {
        // with the numbers: how many come, how many are inside (new-player round 8, #2)
        Event::UnderAttack { player, at, by, soldiers } if *player == me => {
            let b = sim.building_at(*at)?;
            let bd = sim.building(b);
            let inside = match &bd.inventory {
                Some(inv) => rr_rules::Job::SOLDIERS.iter().map(|j| inv.job(*j)).sum::<u32>() as usize,
                None => bd.soldiers.len(),
            };
            Some(crate::i18n::msg(
                if *soldiers == 1 {
                    "{1} sends one soldier against our {2} ({4} inside)"
                } else {
                    "{1} sends {3} soldiers against our {2} ({4} inside)"
                },
                &[&name(*by), kind_at(*at)?, &soldiers.to_string(), &inside.to_string()],
            ))
        }
        Event::BuildingCaptured { player, at, by } if *player == me => {
            Some(crate::i18n::msg("{1} took our {2}", &[&name(*by), kind_at(*at)?]))
        }
        Event::BuildingCaptured { player, at, by } if *by == me => {
            Some(crate::i18n::msg("We took {1}'s {2}", &[&name(*player), kind_at(*at)?]))
        }
        _ => None,
    }
}

/// What a save keeps of the game around the simulation: which mission it is, its map file (for
/// the map's own mission), the three-quarters goal and when the "rivals hold more" tip was last
/// given (new-player round 6, #1).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SaveMeta {
    #[serde(deserialize_with = "lenient")]
    pub mission: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub map: Option<String>,
    /// Win with this share of the land, in percent (0: no such goal).
    #[serde(deserialize_with = "lenient")]
    pub land_goal: u32,
    #[serde(deserialize_with = "lenient")]
    pub behind_told: u32,
    #[serde(deserialize_with = "lenient")]
    pub stories: u32,
    #[serde(deserialize_with = "lenient")]
    pub castles_lost: Vec<(u8, u8)>,
    /// The free game's peace time and time limit, in minutes.
    #[serde(deserialize_with = "lenient")]
    pub peace: u32,
    #[serde(deserialize_with = "lenient")]
    pub time_limit: u32,
    #[serde(deserialize_with = "lenient")]
    pub coins_goal: u32,
    /// The goal was reached and the game goes on (QA round 8, #3: announced again on load).
    #[serde(deserialize_with = "lenient")]
    pub goal_done: bool,
    /// The opponents' level of the game (QA round 8, #6: a load took the session's).
    #[serde(deserialize_with = "level_capped")]
    pub level: Option<u8>,
    /// The human's slot (net round 5, #4: a LAN autosave guessed the lowest free one).
    #[serde(deserialize_with = "lenient")]
    pub player: Option<u8>,
    #[serde(deserialize_with = "lenient")]
    pub deeds: u32,
    #[serde(deserialize_with = "lenient")]
    pub favour: u32,
}

/// A save field that doesn't read loses only itself, not the rest (QA round 9, #6: one bad
/// field made a campaign save load as a free game).
fn lenient<'de, D: serde::Deserializer<'de>, T: serde::de::DeserializeOwned + Default>(d: D) -> Result<T, D::Error> {
    let v = <serde_json::Value as serde::Deserialize>::deserialize(d)?;
    Ok(serde_json::from_value(v).unwrap_or_default())
}

/// The level, beyond hard read as hard (as before item 556).
fn level_capped<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<u8>, D::Error> {
    let v = <serde_json::Value as serde::Deserialize>::deserialize(d)?;
    Ok(v.as_u64().map(|l| l.min(2) as u8))
}

fn encode_ui(messages: &Messages, tips_shown: u32, read_until: u32, meta: &SaveMeta) -> Vec<u8> {
    let msgs: Vec<serde_json::Value> =
        messages.iter().map(|m| serde_json::json!([m.gf, m.text, m.at.map(|p| [p.x, p.y]), m.kind])).collect();
    // the meta's fields sit beside the messages (architect round 3, #9: one list, through serde)
    let mut v = serde_json::to_value(meta).unwrap_or_default();
    if let Some(o) = v.as_object_mut() {
        o.insert("messages".into(), msgs.into());
        o.insert("tips_shown".into(), tips_shown.into());
        o.insert("read_until".into(), read_until.into());
    }
    v.to_string().into_bytes()
}

fn decode_ui(bytes: &[u8]) -> Option<(Messages, u32, u32, SaveMeta)> {
    let v: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let num = |v: &serde_json::Value| v.as_u64().map(|n| n as u32);
    let messages = v["messages"]
        .as_array()?
        .iter()
        .filter_map(|m| {
            let at = m[2].as_array().and_then(|p| Some(MapCoord::new(num(&p[0])? as u16, num(&p[1])? as u16)));
            Some(Message::new(num(&m[0])?, m[1].as_str()?.to_string(), at, num(&m[3])? as u8))
        })
        .collect();
    // older saves have none of these, or some (a bad meta loses the meta, not the save)
    let mut meta: SaveMeta = serde_json::from_value(v.clone()).unwrap_or_default();
    if meta.land_goal == 0 && v["three_quarters"].as_bool() == Some(true) {
        meta.land_goal = 75;
    }
    meta.level = meta.level.map(|l| l.min(2));
    Some((messages, num(&v["tips_shown"])?, num(&v["read_until"])?, meta))
}

/// A named save as the save lists show it.
#[derive(Clone)]
pub struct SaveEntry {
    pub name: String,
    pub path: std::path::PathBuf,
    pub modified: std::time::SystemTime,
    /// Game time (from the save's header; `None` for older saves).
    pub gf: Option<u32>,
}

impl SaveEntry {
    /// "date · age" of the file and the time played, e.g. "2026-10-24 · 5 min ago · 01:02:03".
    pub fn info(&self) -> String {
        let secs = self.modified.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let ago = std::time::SystemTime::now().duration_since(self.modified).map(|d| d.as_secs()).unwrap_or(0);
        let mut s = format!("{} · {}", civil_date(secs.saturating_add_signed(utc_offset(secs))), age(ago));
        if let Some(gf) = self.gf {
            s += &format!(" · {}", crate::ui::fmt_time(gf));
        }
        s
    }
}

/// Reads only the first bytes of the file for its game time.
/// The campaign mission a save is of, if any: read once per file and change (the list is drawn
/// every frame; new-player round 12, #9: the rows didn't say which mission).
pub fn save_mission(p: &std::path::Path, modified: std::time::SystemTime) -> Option<String> {
    type Kept = std::collections::HashMap<std::path::PathBuf, (std::time::SystemTime, Option<String>)>;
    thread_local! {
        static KEPT: std::cell::RefCell<Kept> = Default::default();
    }
    if let Some(m) = KEPT.with(|k| k.borrow().get(p).filter(|(t, _)| *t == modified).map(|(_, m)| m.clone())) {
        return m;
    }
    let bytes = std::fs::read(p).ok();
    let mission = bytes.and_then(|b| rr_ai::save_game_extra(&b)).and_then(|x| decode_ui(&x)).and_then(|d| d.3.mission);
    KEPT.with(|k| k.borrow_mut().insert(p.to_path_buf(), (modified, mission.clone())));
    mission
}

pub fn save_entry(p: &std::path::Path) -> Option<SaveEntry> {
    use std::io::Read;
    let meta = std::fs::metadata(p).ok()?;
    let mut head = [0u8; 8];
    let gf = std::fs::File::open(p).ok()?.read_exact(&mut head).ok().and_then(|()| rr_ai::save_game_time(&head));
    Some(SaveEntry {
        name: p.file_stem()?.to_string_lossy().to_string(),
        path: p.to_path_buf(),
        modified: meta.modified().unwrap_or(std::time::UNIX_EPOCH),
        gf,
    })
}

/// The local time zone's offset from UTC at that moment, in seconds.
#[cfg(unix)]
fn utc_offset(secs: u64) -> i64 {
    let t = secs as libc::time_t;
    // SAFETY: localtime_r only writes the tm we hand it.
    unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&t, &mut tm).is_null() { 0 } else { tm.tm_gmtoff as i64 }
    }
}

#[cfg(not(unix))]
fn utc_offset(_secs: u64) -> i64 {
    0
}

/// "YYYY-MM-DD" of seconds since 1970 (UTC; Howard Hinnant's days-to-civil).
fn civil_date(secs: u64) -> String {
    let z = (secs / 86400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// How long ago a save was made, as one template per case so German can say "vor 5 Min." and
/// "vor 1 Tag" (German round 1, #3: it said "5 Min. her" and "1 Tage her").
fn age(secs: u64) -> String {
    let (template, n) = age_template(secs);
    crate::i18n::fill(template, &[&n.to_string()])
}

fn age_template(secs: u64) -> (&'static str, u64) {
    match secs {
        0..60 => ("just now", 0),
        60..3600 => ("{1} min ago", secs / 60),
        3600..86400 => ("{1} h ago", secs / 3600),
        86400..172800 => ("1 day ago", 1),
        _ => ("{1} days ago", secs / 86400),
    }
}

/// The truce warning for buildings short of soldiers, singular for one (German round 1, #4).
fn undermanned_template(short: usize) -> &'static str {
    if short == 1 {
        "The truce ends in 5 minutes. 1 of your military buildings holds fewer soldiers than it could: Main selection, Military, raise the troops in the middle and at the border."
    } else {
        "The truce ends in 5 minutes. {1} of your military buildings hold fewer soldiers than they could: Main selection, Military, raise the troops in the middle and at the border."
    }
}

/// Whether a stored message is a burned-buildings line (by its template, not its English
/// beginning: architect round 3, #12).
fn is_burned_line(stored: &str) -> bool {
    let template = stored.split(rr_game::SEP).next().unwrap_or("");
    template == burned_template(1) || template == burned_template(2)
}

/// The burned-buildings line, singular for one (it read "1 buildings").
fn burned_template(n: u32) -> &'static str {
    if n == 1 { "Burned with the lost land: 1 building" } else { "Burned with the lost land: {1} buildings" }
}

#[cfg(test)]
mod tests {
    /// The idle motions (owner, 2026-10-27): a carrier standing on its road with nothing to
    /// carry is tracked from the frame it began, and dropped once it walks or carries.
    #[test]
    fn idle_carriers_are_tracked() {
        use rr_core::{Direction::E, MapCoord};
        let mut m = rr_core::MapData::flat(48, 48, 8);
        m.hq = vec![Some(MapCoord::new(20, 20))];
        let setup = rr_sim::GameSetup { players: vec![Default::default()], ..Default::default() };
        let mut sim = rr_sim::Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &m, &setup).unwrap();
        let from = sim.world().grid().neighbor(MapCoord::new(20, 20), rr_core::Direction::SE);
        let road = rr_sim::Cmd::BuildRoad { from, dirs: vec![E, E, E, E] };
        sim.step(&[rr_sim::PlayerCommand { player: 0, cmd: road }]);
        let mut idle = std::collections::HashMap::new();
        for _ in 0..1500 {
            sim.step(&[]);
            track_idle(&mut idle, &sim);
        }
        assert_eq!(idle.len(), 1, "the road's carrier waits");
        let (id, since) = idle.iter().next().map(|(i, s)| (*i, *s)).unwrap();
        assert!(since < sim.gf().0 - crate::idle::START_GF, "it began long ago");
        assert!(matches!(sim.figures()[id].kind, rr_sim::FigKind::Carrier { .. }));
    }

    /// QA round 7, #9: a guest says whose game went apart, not "from ours".
    #[test]
    fn a_guest_hears_of_another_desync() {
        assert!(desync_line(true).contains("ours") && desync_line(false).contains("host"));
        assert!(crate::i18n::has_german(desync_line(false)));
    }

    /// QA round 7, #8: a pause on attack stops the frame's steps (not in a network game).
    #[test]
    fn a_pause_stops_the_frame() {
        assert!(stops_the_frame(true, false));
        assert!(!stops_the_frame(true, true) && !stops_the_frame(false, false));
    }

    /// QA round 7, #6: the last warehouse's news gives no advice that can't be followed (a
    /// storehouse without a warehouse never gets its boards).
    #[test]
    fn the_last_warehouse_news_is_honest() {
        let t = lost_text(rr_rules::BuildingType::Headquarters, None, true);
        assert!(!t.contains("build a storehouse"), "{t}");
        assert!(t.contains("nothing new can be built"), "{t}");
    }

    /// QA round 7, #2: the autosaves are listed with the saves, so they can be loaded.
    #[test]
    fn the_autosaves_can_be_loaded() {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        let dir = std::env::temp_dir().join(format!("rr-saves-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(dir.join("saves")).expect("a temporary folder");
        let mut m = rr_core::MapData::flat(32, 32, 8);
        m.hq = vec![Some(MapCoord::new(10, 10))];
        let setup = rr_sim::GameSetup { players: vec![Default::default()], ..Default::default() };
        let sim = Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &m, &setup).unwrap();
        let auto = dir.join("autosave.rrsave");
        std::fs::write(&auto, rr_ai::save_game(&sim, &[])).unwrap();
        let listed = list_saves_in(&dir.join("saves"), &[auto.clone(), dir.join("autosave-lan.rrsave")]);
        assert_eq!(listed.iter().map(|e| e.path.clone()).collect::<Vec<_>>(), vec![auto]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// QA round 7, #1: the autosave doesn't count as the player's save for F9's question.
    #[test]
    fn the_autosave_doesnt_silence_f9() {
        let auto = std::path::Path::new("/x/autosave.rrsave");
        assert!(!marks_play_saved(auto, auto));
        assert!(marks_play_saved(std::path::Path::new("/x/quick.rrsave"), auto));
    }

    /// Net round 4, #1: a game behind the others catches up; the normal lead doesn't count.
    #[test]
    fn a_game_behind_catches_up() {
        assert!(!catch_up(0) && !catch_up(2));
        assert!(catch_up(3) && catch_up(500));
    }

    /// Net round 4, #5: a network game's autosave loads as a game of one, the other humans'
    /// realms with computer players.
    #[test]
    fn a_lan_autosave_goes_on_alone() {
        let mut m = rr_core::MapData::flat(64, 64, 8);
        m.hq = vec![Some(MapCoord::new(10, 10)), Some(MapCoord::new(40, 10)), Some(MapCoord::new(25, 40))];
        let setup = rr_sim::GameSetup { seed: 1, players: vec![Default::default(); 3], ..Default::default() };
        let sim = Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &m, &setup).unwrap();
        // two humans (0 and 1) and a computer player (2); this machine is 0
        let ais = lan_save_ais(&sim, &[rr_ai::Ai::new(2)], 0, rr_ai::Level::Hard);
        let bytes = rr_ai::save_game_with(&sim, &ais, &[]);
        let (_, loaded, _) = rr_ai::load_game_with(std::sync::Arc::new(rr_rules::Rules::classic()), &bytes).unwrap();
        let mut players: Vec<u8> = loaded.iter().map(|a| a.player).collect();
        players.sort();
        assert_eq!(players, vec![1, 2]);
        // net round 5, #4: the other human's realm goes to an AI at the game's level
        assert_eq!(loaded.iter().find(|a| a.player == 1).map(|a| a.level), Some(rr_ai::Level::Hard));
    }

    /// Net round 4, #3: a network game can't be paused.
    #[test]
    fn a_network_game_cant_be_paused() {
        assert!(can_pause(false) && !can_pause(true));
    }

    /// Net round 4, #4: a network game is never a mission, so no peer automates its harbors.
    #[test]
    fn a_network_game_is_no_mission() {
        assert!(plays_a_mission(true, false, false) && plays_a_mission(false, true, false));
        assert!(!plays_a_mission(true, true, true));
        assert!(!plays_a_mission(false, false, false));
    }

    /// New-player round 10, #4: with no rival in reach, the last step is to find them, not to
    /// take a post nobody can see.
    #[test]
    fn far_rivals_are_found_first() {
        let sim_with = |a: MapCoord, b: MapCoord| {
            let mut m = rr_core::MapData::flat(128, 128, 8);
            m.hq = vec![Some(a), Some(b)];
            let setup = rr_sim::GameSetup { seed: 1, players: vec![Default::default(); 2], ..Default::default() };
            Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &m, &setup).unwrap()
        };
        let far = next_steps(&sim_with(MapCoord::new(10, 10), MapCoord::new(74, 74)), 0);
        assert!(far.last().unwrap().0.starts_with("Find your rivals"), "{far:?}");
        let near = next_steps(&sim_with(MapCoord::new(10, 10), MapCoord::new(40, 10)), 0);
        assert!(near.last().unwrap().0.starts_with("Take a rival post"), "{near:?}");
    }

    /// New-player round 10, #3: a mission's new harbor keeps two soldiers.
    #[test]
    fn a_new_harbor_keeps_defenders() {
        use rr_rules::BuildingType as B;
        assert_eq!(default_reserve(B::Harbor, true, [0; 5]), Some(2));
        assert_eq!(default_reserve(B::Storehouse, true, [0; 5]), Some(2));
        assert_eq!(default_reserve(B::Harbor, false, [0; 5]), None, "free games keep RTTR's default");
        assert_eq!(default_reserve(B::Harbor, true, [1, 0, 0, 0, 0]), None, "a reserve set stays");
        assert_eq!(default_reserve(B::Farm, true, [0; 5]), None);
    }

    /// New-player round 10, #2: a lost castle was one quiet line like a burned farm.
    #[test]
    fn a_lost_warehouse_is_news() {
        use rr_rules::BuildingType as B;
        let t = lost_text(B::Headquarters, Some("Bjorn"), false);
        assert!(t.contains("Bjorn") && t.contains("{1} lost to {2}"), "{t}");
        let last = lost_text(B::Harbor, None, true);
        assert!(last.contains("no warehouse left"), "{last}");
        assert!(lost_is_alert(B::Headquarters) && lost_is_alert(B::Harbor) && lost_is_alert(B::Storehouse));
        assert!(!lost_is_alert(B::Farm));
    }

    /// a11y round 2, #9: F9 asks before throwing away more than a minute of play; an autosave
    /// every ten minutes.
    #[test]
    fn loading_asks_and_the_game_saves_itself() {
        assert!(!load_needs_asking(1000, None));
        assert!(load_needs_asking(5000, None));
        assert!(!load_needs_asking(5000, Some(4500)));
        assert!(load_needs_asking(9000, Some(4500)));
        assert!(!autosave_due(0));
        assert!(autosave_due(12_000) && autosave_due(24_000) && !autosave_due(12_001));
    }

    /// a11y round 2, #6: your own refused orders show on screen.
    #[test]
    fn refusals_are_seen() {
        let e = |player| Event::CommandRejected {
            player,
            cmd: rr_sim::Cmd::PlaceFlag { at: MapCoord::new(1, 1) },
            reason: rr_sim::RejectReason::NotBuildable,
        };
        assert!(refusal_shown(&e(0), 0));
        assert!(!refusal_shown(&e(1), 0));
    }

    use super::*;

    /// New-player round 2, B: mission rivals spare the newcomer for half an hour.
    #[test]
    fn the_keyboard_cursor_walks_rows_and_columns() {
        let g = rr_core::Grid::new(32, 32);
        let p = MapCoord::new(10, 10);
        assert_eq!(cursor_step(g, p, Key::ArrowLeft), Some(MapCoord::new(9, 10)));
        // two steps up keep the column
        let up = cursor_step(g, p, Key::ArrowUp).unwrap();
        assert_eq!(cursor_step(g, up, Key::ArrowUp), Some(MapCoord::new(10, 8)));
        let down = cursor_step(g, p, Key::ArrowDown).unwrap();
        assert_eq!(cursor_step(g, down, Key::ArrowDown), Some(MapCoord::new(10, 12)));
        assert_eq!(cursor_step(g, p, Key::Home), Some(g.neighbor(p, Direction::NW)));
        assert_eq!(cursor_step(g, p, Key::Q), None);
    }

    /// Map-maker roast #1, #2: H crashed the editor, F5 overwrote the quick save with its map.
    /// Map-maker roast #13: the human can play any slot of a map; a save remembers which.
    #[test]
    fn the_human_plays_the_chosen_slot() {
        let args = crate::Args { me: 2, ..Default::default() };
        let ais = make_ais(&args, 4);
        assert_eq!(ais.iter().map(|a| a.player).collect::<Vec<_>>(), vec![0, 1, 3]);
        assert_eq!(human_player(4, &ais), 2);
        assert_eq!(human_player(3, &make_ais(&crate::Args::default(), 3)), 0);
    }

    #[test]
    fn game_keys_are_off_in_the_editor() {
        let keys = crate::keys::Keys::default();
        for k in [Key::H, Key::F5, Key::F9, Key::I, Key::M, Key::P] {
            assert!(!editor_key(&keys, k), "{k:?}");
        }
        assert!(editor_key(&keys, Key::Z));
    }

    #[test]
    fn azerty_pan_keys_do_not_zoom() {
        let key = |key, physical| egui::Event::Key {
            key,
            physical_key: Some(physical),
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        };
        // AZERTY: the key where W sits types Z (zoom), the A position types Q
        let ev = [key(Key::Z, Key::W), key(Key::Q, Key::A), key(Key::H, Key::H)];
        assert_eq!(shadowed_by_pan(&ev), vec![Key::Z, Key::Q]);
        // on QWERTY nothing is shadowed
        assert!(shadowed_by_pan(&[key(Key::W, Key::W), key(Key::Z, Key::Z)]).is_empty());
        assert_eq!(pan_dir(Key::W), Some(Vec2::new(0.0, -1.0)));
        assert_eq!(pan_dir(Key::Z), None);
    }

    #[test]
    fn mission_rivals_keep_a_truce() {
        let mission = crate::Args { mission: Some("europe".into()), ai_level: 1, ..Default::default() };
        let ais = make_ais(&mission, 6);
        assert_eq!(ais.len(), 5);
        // Europe's cast (game design round 1, #2): Bjorn and Hadrian come after the truce, the
        // three others keep it for good
        let truce = |p: u8| ais.iter().find(|a| a.player == p).and_then(|a| a.truce);
        assert_eq!((truce(4), truce(5)), (Some((0, MISSION_TRUCE)), Some((0, MISSION_TRUCE))));
        assert!((1..=3).all(|p| truce(p) == Some((0, u32::MAX / 2))));
        assert!(make_ais(&crate::Args::default(), 3).iter().all(|a| a.truce.is_none()));
        // new-player round 7, #2: against easy opponents the truce lasts an hour
        let easy = crate::Args { mission: Some("europe".into()), ai_level: 0, ..Default::default() };
        assert!(make_ais(&easy, 6).iter().filter(|a| a.player >= 4).all(|a| a.truce == Some((0, 2 * MISSION_TRUCE))));
        // game design round 1, #7: Egypt comes to Mare Nostrum by sea, on normal, not on easy
        let invaders = |level| {
            let a = crate::Args { mission: Some("mediterranean".into()), ai_level: level, ..Default::default() };
            make_ais(&a, 6).iter().filter(|a| a.invade == Some(0)).map(|a| a.player).collect::<Vec<_>>()
        };
        assert_eq!((invaders(1), invaders(0)), (vec![5], vec![]));
        assert!(make_ais(&mission, 6).iter().all(|a| a.invade.is_none()), "Europe has none");
    }

    /// New-player round 4, #1: the player hears when the rivals hold much more land.
    #[test]
    fn falling_behind_is_noticed() {
        assert_eq!(behind(594, &[1719, 2519, 2000, 1800, 2200]), Some(3));
        assert_eq!(behind(1000, &[1500, 1800]), None, "less than twice: no alarm");
        assert_eq!(behind(500, &[]), None, "no rivals left");
        assert_eq!(behind(0, &[300]), None, "nothing to compare");
    }

    /// New-player round 2, #3: next to a road, a new building joins it.
    #[test]
    fn a_road_joins_a_network_that_passes_close_by() {
        assert!(prefer_join(Some(7), 2), "the road one or two nodes away");
        assert!(prefer_join(None, 9), "no road to the flag at all");
        assert!(!prefer_join(Some(5), 4), "about as far: build what was asked");
    }

    #[test]
    fn messages_and_tips_travel_in_the_save() {
        let mut m = Messages::new();
        m.push_back(Message::new(120, "A mine is exhausted".into(), Some(MapCoord::new(3, 4)), MSG_ECONOMY));
        m.push_back(Message::new(60, "💡 Tip".into(), None, MSG_TIP));
        let (back, tips, read, _) = decode_ui(&encode_ui(&m, 0b101, 90, &SaveMeta::default())).unwrap();
        assert_eq!((back, tips, read), (m, 0b101, 90));
        assert!(decode_ui(b"").is_none());
    }

    /// QA round 4, #5: a save is written whole or not at all; a failed write keeps the old file.
    #[test]
    fn saves_are_written_safely() {
        let dir = std::env::temp_dir().join(format!("rr-safe-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("quick.rrsave");
        write_safely(&p, b"old game").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"old game");
        assert!(!temp_path(&p).exists(), "no temp file left");
        // the temp file can't be made (a folder stands in its way): the old save stays
        std::fs::create_dir_all(temp_path(&p)).unwrap();
        assert!(write_safely(&p, b"new game").is_err());
        assert_eq!(std::fs::read(&p).unwrap(), b"old game");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Architect round 2, #4: no victory while watching a replay.
    #[test]
    fn replays_win_nothing() {
        assert!(checks_outcome(false, false, 3));
        assert!(!checks_outcome(false, true, 3));
        assert!(!checks_outcome(true, false, 3) && !checks_outcome(false, false, 1));
    }

    /// QA round 4, #1: watching a replay, F5 must not overwrite the quick save.
    #[test]
    fn replays_are_not_saved() {
        assert!(save_refused(true).is_some());
        assert!(save_refused(false).is_none());
    }

    /// New-player round 7, #7: the idle-stock tip comes back every 20 minutes while it holds,
    /// and a second checklist follows the first.
    #[test]
    fn the_game_keeps_advising() {
        let min = 60 * 20;
        assert!(idle_tip_due(25 * min, None));
        assert!(!idle_tip_due(30 * min, Some(25 * min)));
        assert!(idle_tip_due(45 * min, Some(25 * min)));
        assert!(!idle_tip_due(4 * min, None), "not in the first minutes");
        let mut m = rr_core::MapData::flat(64, 64, 8);
        m.hq = vec![Some(MapCoord::new(20, 20))];
        let setup = rr_sim::GameSetup { seed: 1, players: vec![Default::default()], ..Default::default() };
        let sim = rr_sim::Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &m, &setup).unwrap();
        let next = next_steps(&sim, 0);
        assert!(next.len() >= 5 && next.iter().all(|(_, done)| !done), "{next:?}");
    }

    /// New-player round 7, #6: a new road gets a flag on every second node where one fits, so
    /// each stretch has its own carrier (RTTR's auto flag).
    #[test]
    fn long_roads_get_flags() {
        let mut m = rr_core::MapData::flat(64, 64, 8);
        m.hq = vec![Some(MapCoord::new(20, 20))];
        let setup = rr_sim::GameSetup { seed: 1, players: vec![Default::default()], ..Default::default() };
        let sim = rr_sim::Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &m, &setup).unwrap();
        let g = sim.world().grid();
        let start = g.neighbor(MapCoord::new(20, 20), rr_core::Direction::SE);
        let dirs = vec![rr_core::Direction::E; 8];
        let flags = auto_flags(&sim, start, &dirs);
        assert!(flags.len() >= 3, "{flags:?}");
        // never next to each other, the start or the end
        let mut nodes = vec![start];
        for d in &dirs {
            nodes.push(g.neighbor(*nodes.last().unwrap(), *d));
        }
        let end = *nodes.last().unwrap();
        for (i, a) in flags.iter().enumerate() {
            assert!(g.distance(*a, start) >= 2 && g.distance(*a, end) >= 2);
            for b in &flags[i + 1..] {
                assert!(g.distance(*a, *b) >= 2);
            }
        }
        assert!(auto_flags(&sim, start, &dirs[..2]).is_empty(), "a short road needs none");
    }

    /// New-player round 7, #4: a road may only join another where a flag fits (not right next
    /// to a flag); elsewhere the road stays under way instead of failing silently.
    #[test]
    fn a_road_joins_only_where_a_flag_fits() {
        let mut m = rr_core::MapData::flat(64, 64, 8);
        m.hq = vec![Some(MapCoord::new(20, 20))];
        let setup = rr_sim::GameSetup { seed: 1, players: vec![Default::default()], ..Default::default() };
        let mut sim = rr_sim::Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &m, &setup).unwrap();
        let g = sim.world().grid();
        let hq_flag = g.neighbor(MapCoord::new(20, 20), rr_core::Direction::SE);
        let e = rr_core::Direction::E;
        sim.step(&[rr_sim::PlayerCommand {
            player: 0,
            cmd: rr_sim::Cmd::BuildRoad { from: hq_flag, dirs: vec![e; 4] },
        }]);
        let one = g.neighbor(hq_flag, e);
        let two = g.neighbor(one, e);
        assert!(sim.road_at(one).is_some() && sim.road_at(two).is_some());
        assert!(!flag_fits(&sim, one), "next to the castle's flag");
        assert!(flag_fits(&sim, two));
    }

    /// New-player round 7, #1: the truce warning knows which military buildings hold fewer
    /// soldiers than they could.
    #[test]
    fn undermanned_buildings_are_counted() {
        let mut m = rr_core::MapData::flat(64, 64, 8);
        m.hq = vec![Some(MapCoord::new(20, 20))];
        let setup = rr_sim::GameSetup { seed: 1, players: vec![Default::default()], ..Default::default() };
        let mut sim = rr_sim::Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &m, &setup).unwrap();
        assert_eq!(undermanned(&sim, 0), 0, "the headquarters doesn't count");
        let at = MapCoord::new(26, 20);
        sim.step(&[rr_sim::PlayerCommand {
            player: 0,
            cmd: rr_sim::Cmd::PlaceBuilding { at, kind: rr_rules::BuildingType::Watchtower },
        }]);
        let flag = sim.flag(sim.building(sim.building_at(at).unwrap()).flag).pos;
        let hq_flag = sim.flag(sim.building(sim.building_at(MapCoord::new(20, 20)).unwrap()).flag).pos;
        let dirs = sim.road_path(0, flag, hq_flag, &[]).unwrap();
        sim.step(&[rr_sim::PlayerCommand { player: 0, cmd: rr_sim::Cmd::BuildRoad { from: flag, dirs } }]);
        for _ in 0..30_000 {
            sim.step(&[]);
            if sim.building(sim.building_at(at).unwrap()).soldiers.len() == 1 {
                break;
            }
        }
        // inland, the default settings send one soldier to a tower that holds six
        assert_eq!(undermanned(&sim, 0), 1);
    }

    /// LAN round 3, #5: a desync leaves a report: the build and the whole state at that frame.
    #[test]
    fn a_desync_report_is_written() {
        let dir = std::env::temp_dir().join(format!("rr-desync-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut m = rr_core::MapData::flat(32, 32, 8);
        m.hq = vec![Some(MapCoord::new(10, 10))];
        let setup = rr_sim::GameSetup { seed: 1, players: vec![Default::default()], ..Default::default() };
        let sim = rr_sim::Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &m, &setup).unwrap();
        let p = write_desync_report(&dir, &sim, 1).unwrap();
        let text = std::fs::read_to_string(&p).unwrap();
        assert!(text.contains(&format!("{:016x}", rr_net::build_id())), "the build is named");
        assert!(text.contains("sync hash"), "the state is there");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// LAN round 3, #3: after a lost connection the computer takes over every other realm that
    /// no computer plays yet (the human's own stays the human's; the dead stay dead).
    #[test]
    fn computer_players_take_over_the_humans_left_behind() {
        assert_eq!(takeover_slots(&[true, true, true, false, true], 1, &[3, 4]), vec![0, 2]);
        assert!(takeover_slots(&[true, true], 0, &[1]).is_empty());
    }

    /// New-player round 6, #6: a missing save is explained, not reported as an OS error.
    #[test]
    fn a_missing_save_is_explained() {
        assert!(load_error("No such file or directory (os error 2)", true).contains("F5"));
        assert!(load_error("bad checksum", false).contains("bad checksum"));
    }

    /// New-player round 6, #5: goals for the first game, and a nudge when material lies idle.
    #[test]
    fn first_steps_tick_off() {
        let mut m = rr_core::MapData::flat(64, 64, 8);
        m.hq = vec![Some(MapCoord::new(20, 20))];
        let setup = rr_sim::GameSetup { seed: 1, players: vec![Default::default()], ..Default::default() };
        let mut sim = rr_sim::Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &m, &setup).unwrap();
        assert!(first_steps(&sim, 0).iter().all(|(_, done)| !done));
        sim.step(&[rr_sim::PlayerCommand {
            player: 0,
            cmd: rr_sim::Cmd::PlaceBuilding { at: MapCoord::new(24, 20), kind: rr_rules::BuildingType::Sawmill },
        }]);
        // new-player round 11, #6: placed is not done; connected (or standing) is
        let sawmill = |sim: &rr_sim::Sim| first_steps(sim, 0).iter().any(|(what, done)| *what == "A sawmill" && *done);
        assert!(!sawmill(&sim), "a site with no road is no step taken");
        let b = sim.building_at(MapCoord::new(24, 20)).unwrap();
        let from = sim.flag(sim.building(b).flag).pos;
        let hq_flag = sim.world().grid().neighbor(MapCoord::new(20, 20), rr_core::Direction::SE);
        let dirs = sim.road_path(0, from, hq_flag, &[]).unwrap();
        sim.step(&[rr_sim::PlayerCommand { player: 0, cmd: rr_sim::Cmd::BuildRoad { from, dirs } }]);
        assert!(sawmill(&sim), "connected: ticked");
        assert!(idle_stock(69, 82, 0));
        assert!(!idle_stock(69, 82, 3) && !idle_stock(20, 82, 0));
    }

    /// New-player round 6, #3: an attack brings a fast game back to normal speed, once a minute.
    #[test]
    fn an_attack_slows_the_game_down() {
        assert_eq!(slow_for_attack(6, None, 1000), (NORMAL_SPEED, Some(1000)));
        assert_eq!(slow_for_attack(6, Some(1000), 1500), (6, Some(1000)), "not again within a minute");
        assert_eq!(slow_for_attack(6, Some(1000), 2200), (NORMAL_SPEED, Some(2200)));
        assert_eq!(slow_for_attack(1, None, 1000), (1, None), "slower than normal stays");
    }

    /// New-player round 6, #3: the news names the attacker and the building.
    #[test]
    fn military_news_names_attacker_and_building() {
        let mut m = rr_core::MapData::flat(64, 64, 8);
        m.hq = vec![Some(MapCoord::new(20, 20)), Some(MapCoord::new(44, 44))];
        let players = (0..2)
            .map(|i| rr_sim::PlayerSetup { name: ["Iberia", "Germania"][i].into(), ..Default::default() })
            .collect();
        let setup = rr_sim::GameSetup { seed: 1, players, ..Default::default() };
        let sim = rr_sim::Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &m, &setup).unwrap();
        let at = MapCoord::new(20, 20);
        let shown = crate::i18n::show(
            &military_news(&sim, 0, &Event::UnderAttack { player: 0, at, by: 1, soldiers: 22 }).unwrap(),
        );
        assert!(shown.contains("Germania") && shown.contains("Castle"), "{shown}");
        // new-player round 8, #2: how many come, how many are inside
        let inside = rr_rules::Job::SOLDIERS.iter().map(|j| sim.player_inventory(0).job(*j)).sum::<u32>();
        assert!(shown.contains("22") && shown.contains(&format!("({inside} ")), "{shown}");
        // round 8, #3: no "Grow" under pressure
        assert!(standing_advice(false, false).starts_with("Grow") && !standing_advice(true, false).contains("Grow"));
        // round 9, #7: no "attackers" when nobody attacked
        assert!(
            !standing_advice(false, true).contains("attackers") && !standing_advice(false, true).starts_with("Grow")
        );
        let tip = crate::i18n::show(&castle_tip(&sim, 0).unwrap());
        assert!(tip.contains(&format!("holds {inside} soldiers")), "{tip}");
        // every tip shows its bulb in German too (QA round 5, #7)
        let stored = castle_tip(&sim, 0).unwrap();
        let template = stored.split('\u{1f}').next().unwrap();
        assert!(crate::i18n::has_german(template));
        let took =
            crate::i18n::show(&military_news(&sim, 1, &Event::BuildingCaptured { player: 0, at, by: 1 }).unwrap());
        assert!(took.contains("Iberia"), "{took}");
    }

    /// New-player round 9, #9: "nothing in reach" once per kind in 10 minutes.
    #[test]
    fn nothing_in_reach_is_said_sparingly() {
        assert!(nothing_due(None, 5));
        assert!(!nothing_due(Some(1000), 1000 + 11_999));
        assert!(nothing_due(Some(1000), 1000 + 12_000));
    }

    /// New-player rounds 8 and 9: losses a few frames after a capture go with it; later ones not.
    #[test]
    fn burned_buildings_go_with_the_capture() {
        assert!(in_burn_window(100, 100) && in_burn_window(100, 101) && in_burn_window(100, 110));
        assert!(!in_burn_window(100, 111) && !in_burn_window(100, 99));
    }

    /// New-player round 8, #6: beer comes before the weapons in Next steps (weapons without
    /// beer make no soldier).
    #[test]
    fn next_steps_brew_before_they_forge() {
        let mut m = rr_core::MapData::flat(48, 48, 8);
        m.hq = vec![Some(MapCoord::new(20, 20))];
        let setup = rr_sim::GameSetup { seed: 1, players: vec![Default::default()], ..Default::default() };
        let sim = rr_sim::Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &m, &setup).unwrap();
        let steps: Vec<&str> = next_steps(&sim, 0).into_iter().map(|s| s.0).collect();
        let at = |t: &str| steps.iter().position(|s| *s == t).unwrap();
        assert!(at("A brewery") < at("An iron smelter and an armory"), "{steps:?}");
        // round 9, #4: the last step leads to the rivals (here alone: finding them, round 10, #4)
        assert!(steps.last().is_some_and(|s| s.starts_with("Find your rivals")), "{steps:?}");
    }

    /// Game design round 1, #8: the truce binds the player too. Attacking a rival ends the truce
    /// with it and its allies, not with the others.
    #[test]
    fn an_attack_ends_the_truce() {
        let mut m = rr_core::MapData::flat(96, 96, 8);
        m.hq = vec![
            Some(MapCoord::new(20, 20)),
            Some(MapCoord::new(60, 20)),
            Some(MapCoord::new(40, 60)),
            Some(MapCoord::new(70, 70)),
        ];
        let team = |t| rr_sim::PlayerSetup { team: Some(t), ..Default::default() };
        let setup =
            rr_sim::GameSetup { seed: 1, players: vec![team(0), team(1), team(1), team(2)], ..Default::default() };
        let sim = rr_sim::Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &m, &setup).unwrap();
        let mut ais: Vec<rr_ai::Ai> = (1..4).map(rr_ai::Ai::new).collect();
        for ai in &mut ais {
            ai.truce = Some((0, MISSION_TRUCE));
        }
        assert_eq!(truce_against(&ais, 0, 2, 100), Some(MISSION_TRUCE));
        assert_eq!(end_truce(&mut ais, &sim, 0, 2, 100), vec![1, 2]);
        assert_eq!(truce_against(&ais, 0, 1, 101), None, "its ally's ended too");
        assert_eq!(truce_against(&ais, 0, 3, 101), Some(MISSION_TRUCE), "the others keep it");
        assert!(end_truce(&mut ais, &sim, 0, 2, 200).is_empty(), "nothing left to end");
    }

    /// Architect round 3, #8: a truce for good is a kind, not a number to compare (two
    /// sentinels, `u32::MAX / 2` written and `u32::MAX / 4` tested).
    #[test]
    fn a_truce_for_good_is_a_kind() {
        for t in [Truce::Until(0), Truce::Until(MISSION_TRUCE), Truce::Until(minutes(24 * 60)), Truce::ForGood] {
            assert_eq!(Truce::of(t.gf()), t);
        }
        let europe = crate::Args { mission: Some("europe".into()), ai_level: 1, ..Default::default() };
        let kinds: Vec<Truce> = make_ais(&europe, 6).iter().filter_map(|a| a.truce).map(|t| Truce::of(t.1)).collect();
        assert!(kinds.contains(&Truce::ForGood) && kinds.contains(&Truce::Until(MISSION_TRUCE)), "{kinds:?}");
    }

    /// The truce to count down is the hostile rivals', not a peaceful one's everlasting truce
    /// (the Europe briefing said "the first 1789569 minutes").
    #[test]
    fn peaceful_rivals_dont_stretch_the_truce() {
        let europe = crate::Args { mission: Some("europe".into()), ai_level: 1, ..Default::default() };
        assert_eq!(truce_end(&make_ais(&europe, 6), 0), Some(MISSION_TRUCE));
        let mut ais = make_ais(&europe, 6);
        for a in &mut ais {
            a.truce = a.truce.filter(|t| Truce::of(t.1) == Truce::ForGood);
        }
        assert_eq!(truce_end(&ais, 0), None, "only peaceful rivals: no countdown");
    }

    /// Game design round 1, #5: with one or two rivals left, their posts and warehouses show
    /// through the fog (the last hour of a won game was a search); with more, none.
    #[test]
    fn the_last_rivals_show() {
        let mut m = rr_core::MapData::flat(96, 96, 8);
        m.hq = vec![
            Some(MapCoord::new(20, 20)),
            Some(MapCoord::new(60, 20)),
            Some(MapCoord::new(40, 60)),
            Some(MapCoord::new(70, 70)),
        ];
        let team = |t| rr_sim::PlayerSetup { team: Some(t), ..Default::default() };
        let setup =
            rr_sim::GameSetup { seed: 1, players: vec![team(0), team(1), team(2), team(3)], ..Default::default() };
        let sim = rr_sim::Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &m, &setup).unwrap();
        assert!(holdouts(&sim, 0).is_empty(), "three rivals: no marks");
        // two rivals from the start: nobody has fallen yet, the map is still to explore
        let mut two = setup.clone();
        two.players[3].team = Some(0);
        let sim2 = rr_sim::Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &m, &two).unwrap();
        assert!(holdouts(&sim2, 0).is_empty(), "two rivals, none fallen: no marks");
        // one of three falls: the two left show
        let mut sim = sim;
        sim.debug_destroy_building(MapCoord::new(70, 70));
        sim.step(&[]);
        assert!(!sim.players()[3].alive);
        assert_eq!(holdouts(&sim, 0), vec![(1, MapCoord::new(60, 20)), (2, MapCoord::new(40, 60))]);
    }

    /// Genre lessons (PLAN §27, #7): following a settler, the camera glides after it, and a jump
    /// across the map (a wrap, a ship) is taken at once.
    #[test]
    fn the_camera_glides_after_a_settler() {
        let (cur, target) = (Vec2::new(0.0, 0.0), Vec2::new(100.0, 0.0));
        let one = follow_step(cur, target, 1.0 / 60.0);
        assert!(one.x > 0.0 && one.x < 20.0, "a glide, not a jump: {one:?}");
        let mut c = cur;
        for _ in 0..120 {
            c = follow_step(c, target, 1.0 / 60.0);
        }
        assert!((c - target).length() < 1.0, "there after two seconds: {c:?}");
        assert_eq!(follow_step(cur, Vec2::new(5000.0, 0.0), 1.0 / 60.0), Vec2::new(5000.0, 0.0));
    }

    /// A small free game without window or sound (architect round 3, #2), the rival under a
    /// truce of half an hour; and the rival's castle.
    fn headless(peace: u32, time_limit: u32) -> (App, MapCoord) {
        let args = crate::Args { players: 2, size: 64, seed: 3, peace, time_limit, ..Default::default() };
        let mut app = App::headless(args);
        for ai in &mut app.ais {
            ai.truce = Some((0, minutes(30)));
        }
        let hq = app.sim.building(app.sim.players()[1].hq.expect("a castle")).pos;
        (app, hq)
    }

    /// Architect round 3, #1 through `App::cmd`: an attack refused in peace breaks no truce;
    /// out of peace it does.
    #[test]
    fn headless_peace_keeps_the_truce() {
        let (mut app, hq) = headless(15, 0);
        let attack = Cmd::Attack { target: hq, soldiers: 1, strong: true };
        app.cmd(attack.clone());
        assert_eq!(app.truce_with(1), Some(Truce::Until(minutes(30))), "refused in peace: the truce holds");
        assert!(app.session.my_attacks.is_empty());
        assert!(!app.session.pending.iter().any(|c| matches!(c.cmd, Cmd::Attack { .. })), "not sent either");
        app.args.peace = 0;
        app.cmd(attack);
        assert_eq!(app.truce_with(1), None, "an attack ends it");
        assert_eq!(app.session.my_attacks.len(), 1);
    }

    /// Story round 1: a mission's beat is told once, when its moment comes (here our first
    /// capture), and a won mission has its epilogue.
    #[test]
    fn headless_the_story_is_told_once() {
        let args = crate::Args { mission: Some("first".into()), seed: 3, ..Default::default() };
        let mut app = App::headless(args);
        let hq = app.sim.building(app.sim.players()[1].hq.expect("a castle")).pos;
        let line =
            |app: &App| app.messages.iter().filter(|m| m.text.starts_with("In the post you took lies a purse")).count();
        app.tale_news();
        assert_eq!(line(&app), 0, "nothing taken yet");
        app.handle_events(vec![rr_sim::Event::BuildingCaptured { player: 1, at: hq, by: 0 }]);
        app.tale_news();
        app.tale_news();
        assert_eq!(line(&app), 1, "told once");
        assert!(!app.session.epilogue);
        app.mission_won();
        assert!(app.session.epilogue);
    }

    /// A catapult of ours hitting the rival ends the truce, as an attack does (game design
    /// round 1, #8), through the app's event handling.
    #[test]
    fn headless_a_catapult_ends_the_truce() {
        let (mut app, hq) = headless(0, 0);
        app.handle_events(vec![rr_sim::Event::CatapultHit { player: 1, at: hq, by: 1 }]);
        assert!(app.truce_with(1).is_some(), "the rival's own hit: nothing of ours");
        app.handle_events(vec![rr_sim::Event::CatapultHit { player: 1, at: hq, by: 0 }]);
        assert_eq!(app.truce_with(1), None);
    }

    /// A stone thrown where we can see is drawn flying, then gone after it lands (playtest
    /// 2026-10-27: nothing flew).
    #[test]
    fn headless_a_thrown_stone_flies_and_lands() {
        let (mut app, _) = headless(0, 0);
        // (our own castle: the rival's lies out of sight, and is drawn nothing of)
        let hq = app.sim.building(app.sim.players()[0].hq.expect("a castle")).pos;
        let to = MapCoord::new(hq.x + 4, hq.y);
        app.handle_events(vec![rr_sim::Event::CatapultThrow { from: hq, to, hit: true }]);
        assert_eq!(app.session.shots.len(), 1);
        assert_eq!((app.session.shots[0].from, app.session.shots[0].to), (hq, to));
        for _ in 0..60 {
            app.sim.step(&[]);
        }
        app.handle_events(Vec::new());
        assert!(app.session.shots.is_empty(), "landed and gone");
    }

    /// Architect round 3, #3: the AIs' peace time is not in their save (`serde(skip)`): a
    /// load sets it again from the game's own peace time, not the session's.
    #[test]
    fn headless_a_load_keeps_the_peace() {
        let (mut app, _) = headless(15, 0);
        for _ in 0..50 {
            app.step_frame();
        }
        let dir = std::env::temp_dir().join(format!("rr-peace-{}", std::process::id()));
        let path = dir.join("peace.rrsave");
        app.save_to(&path);
        // (written on a thread of its own, perf round 3, #4: done before another app loads it,
        // QA round 9, #4)
        app.finish_save(true);
        let (mut other, _) = headless(0, 0);
        other.load_from(&path);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!((other.args.peace, other.sim.gf().0), (15, 50), "the save's game");
        assert!(!other.ais.is_empty() && other.ais.iter().all(|a| a.peace_until == peace_gf(15)));
    }

    /// Perf round 3, #4: a save is checked and written on a thread of its own and told when
    /// done; the file is there then.
    #[test]
    fn headless_a_save_is_written_behind() {
        let (mut app, _) = headless(0, 0);
        let dir = std::env::temp_dir().join(format!("rr-behind-{}", std::process::id()));
        let path = dir.join("behind.rrsave");
        app.save_to(&path);
        app.save_to(&path);
        app.finish_save(true);
        let written = std::fs::read(&path);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(loads_back(&written.expect("written")).is_ok());
        assert!(app.saving.is_none());
        assert!(app.session.toast.as_deref().is_some_and(|t| t.contains("behind")), "{:?}", app.session.toast);
    }

    /// Genre lessons (PLAN §27, #7): small stories in the news, each once a game: the first
    /// coin, the first general, the first ship.
    #[test]
    fn small_stories_are_told_once() {
        let none = Stories { coin: false, general: false, ship: false };
        assert!(new_stories(&none, 0).is_empty());
        let all = Stories { coin: true, general: true, ship: true };
        let told = new_stories(&all, 0);
        assert_eq!(told.len(), 3);
        let mask = told.iter().fold(0, |m, (bit, _, _)| m | bit);
        assert!(new_stories(&all, mask).is_empty(), "once each");
        assert_eq!(new_stories(&Stories { general: true, ..none }, 0).len(), 1);
        for (_, text, _) in told {
            assert!(crate::i18n::has_german(text), "{text}");
        }
    }

    /// New-player round 12, #3: with no military building the warning says build one, not
    /// that the border buildings are full.
    #[test]
    fn the_truce_warning_fits() {
        assert!(truce_warning(false, false).contains("no military building"));
        assert!(truce_warning(false, true).contains("no military building"));
        assert!(truce_warning(true, true).contains("Man your border"));
        assert!(truce_warning(true, false).contains("already take all"));
        for posts in [false, true] {
            assert!(crate::i18n::has_german(truce_warning(posts, false)));
        }
    }

    /// New-player round 12, #7: "take the castle of X" is met when our side takes it, not when
    /// another realm does; then the player is told the goal moved on to conquering all.
    #[test]
    fn headless_only_our_side_takes_the_goal_castle() {
        let (mut app, hq) = headless(0, 0);
        let goal = Some(crate::missions::Objective::DefeatRival(1));
        app.handle_events(vec![rr_sim::Event::Razed {
            player: 1,
            at: hq,
            kind: rr_rules::BuildingType::Headquarters,
            by: 2,
        }]);
        assert_eq!(app.session.castles_lost, vec![(1, 2)]);
        assert!(!castle_goal_ours(goal, &app.session.castles_lost, |p| p == 0), "another realm took it");
        assert!(castle_goal_ours(goal, &[(1, 0)], |p| p == 0), "we took it");
        assert!(castle_goal_ours(goal, &[], |p| p == 0), "unknown (an older save): as before");
        assert!(castle_goal_ours(None, &app.session.castles_lost, |p| p == 0));
    }

    /// New-player round 12, #9: a save row knows its mission, read from the file without
    /// loading the game.
    #[test]
    fn a_save_knows_its_mission() {
        let dir = std::env::temp_dir().join(format!("rr-mission-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("m.rrsave");
        let mut m = rr_core::MapData::flat(32, 32, 8);
        m.hq = vec![Some(MapCoord::new(10, 10))];
        let setup = rr_sim::GameSetup { players: vec![Default::default()], ..Default::default() };
        let sim = rr_sim::Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &m, &setup).unwrap();
        let meta = SaveMeta { mission: Some("europe".into()), ..Default::default() };
        std::fs::write(&path, rr_ai::save_game_with(&sim, &[], &encode_ui(&Messages::new(), 0, 0, &meta))).unwrap();
        let t = std::fs::metadata(&path).unwrap().modified().unwrap();
        let first = save_mission(&path, t);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(first.as_deref(), Some("europe"));
        assert_eq!(save_mission(&path, t).as_deref(), Some("europe"), "kept: the file is gone");
    }

    /// Veteran round 12, #4: land a neighbour's post took is told, with the place; not again
    /// when a capture of ours in the same frame already tells it.
    #[test]
    fn headless_lost_land_is_told() {
        let (mut app, hq) = headless(0, 0);
        let n = app.messages.len();
        app.handle_events(vec![rr_sim::Event::LandLost { player: 0, to: 1, nodes: 12, at: hq }]);
        assert_eq!(app.messages.len(), n + 1);
        let m = app.messages.front().unwrap();
        assert_eq!((m.kind, m.at), (MSG_MILITARY, Some(hq)));
        assert!(crate::i18n::show(&m.text).starts_with("12 nodes of our land went to"), "{}", m.text);
        let n = app.messages.len();
        app.handle_events(vec![
            rr_sim::Event::BuildingCaptured { player: 0, at: hq, by: 1 },
            rr_sim::Event::LandLost { player: 0, to: 1, nodes: 30, at: hq },
        ]);
        assert!(app.messages.iter().take(app.messages.len() - n).all(|m| !m.text.contains("nodes of our land")));
    }

    /// New-player round 12, #4: building in a pause says it is planned, not done.
    #[test]
    fn headless_a_pause_plans_the_order() {
        let (mut app, hq) = headless(0, 0);
        app.paused = true;
        app.session.toast = None;
        app.cmd(Cmd::PlaceFlag { at: MapCoord::new(hq.x + 4, hq.y + 4) });
        assert!(
            app.session.toast.as_deref().is_some_and(|t| t.starts_with("Planned") && t.ends_with("(P).")),
            "{:?}",
            app.session.toast
        );
        assert!(crate::i18n::has_german(PLANNED_NOTE));
        app.paused = false;
        app.session.toast = None;
        app.cmd(Cmd::PlaceFlag { at: MapCoord::new(hq.x + 6, hq.y + 4) });
        assert_eq!(app.session.toast, None, "running: the work itself shows");
    }

    /// New-player round 12, #5: the quick save is in the list, with the autosaves.
    #[test]
    fn the_quick_save_is_listed() {
        let beside = App::saves_beside();
        assert!(beside.contains(&App::save_path()) && beside.contains(&App::autosave_path()));
        let dir = std::env::temp_dir().join(format!("rr-list-{}", std::process::id()));
        let quick = dir.join("quick.rrsave");
        std::fs::create_dir_all(dir.join("saves")).unwrap();
        let mut m = rr_core::MapData::flat(32, 32, 8);
        m.hq = vec![Some(MapCoord::new(10, 10))];
        let setup = rr_sim::GameSetup { players: vec![Default::default()], ..Default::default() };
        let sim = rr_sim::Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &m, &setup).unwrap();
        std::fs::write(&quick, rr_ai::save_game_with(&sim, &[], b"")).unwrap();
        let list = list_saves_in(&dir.join("saves"), std::slice::from_ref(&quick));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(list.len(), 1, "the quick save");
    }

    /// Genre lessons (PLAN §27, #8): fast through the quiet stretches, back to the speed of
    /// before at the first news (or a pause); the player's own speed change ends it too.
    #[test]
    fn headless_fast_until_news() {
        let (mut app, _) = headless(0, 0);
        app.speed = NORMAL_SPEED;
        app.skip_quiet();
        assert_eq!(app.speed, SPEEDS.len() - 1);
        for _ in 0..20 {
            app.step_frame();
            app.quiet_check();
        }
        assert_eq!(app.speed, SPEEDS.len() - 1, "nothing happened yet");
        let gf = app.sim.gf().0;
        app.messages.push_front(Message::new(gf, "finished".into(), None, MSG_ECONOMY));
        app.quiet_check();
        assert_eq!(app.speed, SPEEDS.len() - 1, "a building finished: no reason to stop");
        app.messages.push_front(Message::new(gf + 1, "attacked".into(), None, MSG_MILITARY));
        app.step_frame();
        app.quiet_check();
        assert_eq!((app.speed, app.session.skip), (NORMAL_SPEED, None), "back at the military news");
        // a tip is no news either (it ended a skip at once early in a game)
        app.skip_quiet();
        app.messages.push_front(Message::new(app.sim.gf().0 + 1, "tip".into(), None, MSG_TIP));
        app.step_frame();
        app.quiet_check();
        assert_eq!(app.speed, SPEEDS.len() - 1, "a tip");
        // a note of the player's own (saved, loaded) is no news
        app.messages.push_front(Message::new(app.sim.gf().0 + 1, "saved".into(), None, MSG_GENERAL));
        app.step_frame();
        app.quiet_check();
        assert_eq!(app.speed, SPEEDS.len() - 1);
        app.speed = 3;
        app.quiet_check();
        assert_eq!((app.speed, app.session.skip), (3, None), "the player chose a speed");
    }

    /// Architect round 3, #6: a new game or a load starts from a fresh `Session`; nothing of
    /// the last game's state is left (it was reset field by field, and fields were forgotten).
    #[test]
    fn headless_each_game_has_a_fresh_session() {
        let (mut app, hq) = headless(0, 0);
        let dirty = |app: &mut App| {
            let s = &mut app.session;
            s.toast = Some("old".into());
            s.game_over = Some("over".into());
            s.my_attacks.push((hq, 5));
            s.paused_by_attack = true;
            s.tips_shown = 7;
            s.behind_told = 9;
            s.selected = Some(hq);
            s.last_attacked = Some(3);
        };
        let fresh = |app: &App| {
            let s = &app.session;
            s.game_over.is_none()
                && s.my_attacks.is_empty()
                && !s.paused_by_attack
                && s.selected.is_none()
                && s.last_attacked.is_none()
                && s.behind_told == 0
        };
        dirty(&mut app);
        let args = app.args.clone();
        app.start_game(args);
        assert!(fresh(&app) && app.session.tips_shown == 0 && app.session.toast.is_none(), "a new game");
        let dir = std::env::temp_dir().join(format!("rr-fresh-{}", std::process::id()));
        let path = dir.join("fresh.rrsave");
        app.save_to(&path);
        app.finish_save(true);
        dirty(&mut app);
        app.load_from(&path);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(fresh(&app), "a load (its save carries the tips and the rest)");
    }

    /// QA round 9, #2 and #3: a save still being written belongs to its game (it marked the next
    /// one saved, and F9 then loaded over new play without asking); a load closes the truce
    /// question of the last game's post.
    #[test]
    fn headless_a_save_stays_with_its_game() {
        let (mut app, hq) = headless(0, 0);
        for _ in 0..50 {
            app.step_frame();
        }
        let dir = std::env::temp_dir().join(format!("rr-own-{}", std::process::id()));
        let path = dir.join("own.rrsave");
        app.save_to(&path);
        let args = app.args.clone();
        app.start_game(args);
        assert_eq!(app.session.saved_gf, None, "the new game is not saved");
        assert!(app.saving.is_none());
        let b = app.sim.building_at(hq).expect("the castle");
        app.ui.attack = crate::ui::AttackPanel { count: Some(3), weak: true, asking: Some(b) };
        app.load_from(&path);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(app.sim.gf().0, 50, "loaded");
        assert_eq!(app.ui.attack, crate::ui::AttackPanel { count: Some(3), weak: true, asking: None });
    }

    /// The free game's time limit ends the game through whole frames (`step_frame`): the side
    /// with the most land wins; two untouched castles hold the same: a draw (QA round 9, #5: it
    /// went to the lower slot).
    #[test]
    fn headless_the_time_limit_ends_the_game() {
        let (mut app, _) = headless(0, 1);
        app.ais.clear();
        while app.session.game_over.is_none() && app.sim.gf().0 <= minutes(1) {
            app.step_frame();
        }
        assert_eq!(app.sim.gf().0, minutes(1), "over at the minute");
        assert_eq!(app.session.game_over.as_deref(), Some("Time is up! The land is shared equally: a draw."));
        assert!(!app.won_once);
    }

    /// Architect round 3, #5: one goal engine for every kind of goal, in its order: the
    /// mission's own, the coins, the time limit, all rivals beaten, the land share.
    #[test]
    fn one_goal_engine() {
        use crate::missions::Objective;
        let sim = |teams: [Option<u8>; 2]| {
            let mut m = rr_core::MapData::flat(64, 64, 8);
            m.hq = vec![Some(MapCoord::new(12, 20)), Some(MapCoord::new(44, 20))];
            let mut setup = rr_sim::GameSetup { players: vec![Default::default(); 2], ..Default::default() };
            setup.players[0].team = teams[0];
            setup.players[1].team = teams[1];
            rr_sim::Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &m, &setup).unwrap()
        };
        let free = crate::Args::default();
        let rivals = sim([None, None]);
        let kind = |o: Option<Outcome>| o.map(|o| (o.kind, o.text));
        assert_eq!(kind(outcome(&rivals, 0, &free, None, false)), None, "nothing decided");
        // a mission's goal: once, then the game goes on
        let held = Some(Objective::HoldLand(40));
        assert_eq!(outcome(&rivals, 0, &free, held, false).map(|o| o.kind), Some(Ends::Goal));
        assert_eq!(kind(outcome(&rivals, 0, &free, held, true)), None, "met before: goes on");
        assert_eq!(kind(outcome(&rivals, 0, &free, Some(Objective::HoldLand(60)), false)), None);
        // the land share of the free game
        let land = crate::Args { land_goal: 40, ..Default::default() };
        let won = outcome(&rivals, 0, &land, None, false).unwrap();
        let text = crate::i18n::msg("Victory! You hold {1} % of the land.", &["40"]);
        assert_eq!((won.kind, won.text), (Ends::Won { mission: true }, text));
        assert!(outcome(&rivals, 0, &crate::Args { land_goal: 60, ..Default::default() }, None, false).is_none());
        // nobody left to fight
        let allies = sim([Some(1), Some(1)]);
        assert_eq!(outcome(&allies, 0, &free, None, false).map(|o| o.kind), Some(Ends::Won { mission: true }));
        // the time limit: the side with the most land (a draw goes to the lower slot)
        let mut late = sim([None, None]);
        for _ in 0..minutes(1) {
            late.step(&[]);
        }
        let limit = crate::Args { time_limit: 1, ..Default::default() };
        let land = late.land_by_player();
        let (ours, theirs) = (outcome(&late, 0, &limit, None, false).unwrap(), outcome(&late, 1, &limit, None, false));
        if land[0] == land[1] {
            // (a tie is a draw for both: QA round 9, #5)
            assert_eq!(
                (ours.kind, ours.text.as_str()),
                (Ends::Lost, "Time is up! The land is shared equally: a draw.")
            );
        } else {
            let me_first = land[0] > land[1];
            assert_eq!(ours.kind, if me_first { Ends::Won { mission: false } } else { Ends::Lost });
            assert_eq!(theirs.map(|o| o.kind), Some(if me_first { Ends::Lost } else { Ends::Won { mission: false } }));
        }
    }

    /// Genre lessons (PLAN §27, #12): a peaceful economy game. Peace for good holds every attack
    /// back for ever, and whoever first has the coins wins.
    #[test]
    fn an_economy_game_has_a_winner() {
        let attack = Cmd::Attack { target: MapCoord::new(1, 1), soldiers: 3, strong: true };
        assert!(breaks_peace(&attack, u32::MAX - 1, peace_gf(PEACE_FOR_GOOD)));
        assert_eq!(peace_gf(15), 18_000);
        // coins minted, not in store: the AIs spend theirs on promotions at once (none in store
        // at minute 60 in four games; the best minted 50-59 by minute 120 in peace)
        assert_eq!(first_to(&[12, 59, 3], 60), None);
        assert_eq!(first_to(&[12, 61, 60], 60), Some(1));
    }

    /// Game design round 1, #3: the free game's Players table sets a level per computer player;
    /// unset, the menu's level counts.
    #[test]
    fn each_rival_has_its_level() {
        let args = crate::Args { ai_level: 1, levels: vec![255, 0, 2, 255], ..Default::default() };
        let ais = make_ais(&args, 4);
        let level = |p: u8| ais.iter().find(|a| a.player == p).map(|a| a.level);
        assert_eq!(
            (level(1), level(2), level(3)),
            (Some(rr_ai::Level::Easy), Some(rr_ai::Level::Hard), Some(rr_ai::Level::Normal))
        );
    }

    /// New-player round 11, #7: an attack whose soldiers are all gone and whose target still
    /// stands is told as failed; one still under way, or a target taken, isn't.
    #[test]
    fn a_failed_attack_is_told() {
        let mut m = rr_core::MapData::flat(64, 64, 8);
        m.hq = vec![Some(MapCoord::new(12, 20)), Some(MapCoord::new(40, 20))];
        let setup = rr_sim::GameSetup { seed: 1, players: vec![Default::default(); 2], ..Default::default() };
        let sim = rr_sim::Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &m, &setup).unwrap();
        // nobody of ours is on the way to the rival's castle: that attack is over, and failed
        assert!(attack_failed(&sim, 0, MapCoord::new(40, 20)));
        // our own castle (taken, say) or nothing there: no failure to tell
        assert!(!attack_failed(&sim, 0, MapCoord::new(12, 20)));
        assert!(!attack_failed(&sim, 0, MapCoord::new(30, 30)));
    }

    /// Net round 5, #1: a save is tried before it is written; a good one loads back, garbage
    /// doesn't.
    #[test]
    fn saves_are_tried_before_they_are_written() {
        let mut m = rr_core::MapData::flat(32, 32, 8);
        m.hq = vec![Some(MapCoord::new(10, 10))];
        let setup = rr_sim::GameSetup { seed: 1, players: vec![Default::default()], ..Default::default() };
        let sim = rr_sim::Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &m, &setup).unwrap();
        assert!(loads_back(&rr_ai::save_game_with(&sim, &[], b"")).is_ok());
        assert!(loads_back(b"RRG6 not a save").is_err());
    }

    /// Game design round 1, #6: peace time holds every attack back, then lets them go.
    #[test]
    fn peace_time_holds_attacks_back() {
        let attack = Cmd::Attack { target: MapCoord::new(1, 1), soldiers: 3, strong: true };
        assert!(breaks_peace(&attack, 100, 18_000));
        assert!(!breaks_peace(&attack, 18_000, 18_000));
        assert!(!breaks_peace(&attack, 100, 0), "no peace time");
        assert!(!breaks_peace(&Cmd::Surrender, 100, 18_000), "only attacks");
        // a catapult fires on its own: none is built in peace (it fired in peace, item 460)
        let catapult = Cmd::PlaceBuilding { at: MapCoord::new(1, 1), kind: rr_rules::BuildingType::Catapult };
        assert!(breaks_peace(&catapult, 100, 18_000));
        let farm = Cmd::PlaceBuilding { at: MapCoord::new(1, 1), kind: rr_rules::BuildingType::Farm };
        assert!(!breaks_peace(&farm, 100, 18_000));
    }

    /// Game design round 1, #6: at the time limit the side with the most land wins.
    #[test]
    fn at_the_time_limit_the_most_land_wins() {
        let mut m = rr_core::MapData::flat(96, 96, 8);
        m.hq = vec![Some(MapCoord::new(20, 20)), Some(MapCoord::new(60, 20)), Some(MapCoord::new(40, 60))];
        let team = |t| rr_sim::PlayerSetup { team: Some(t), ..Default::default() };
        let setup = rr_sim::GameSetup { seed: 1, players: vec![team(0), team(1), team(1)], ..Default::default() };
        let sim = rr_sim::Sim::new(std::sync::Arc::new(rr_rules::Rules::classic()), &m, &setup).unwrap();
        let (w, tie) = most_land(&sim).expect("someone holds land");
        assert!(!tie);
        assert!(sim.are_allies(w, 1) && !sim.are_allies(w, 0), "two realms beat one: {w}");
    }

    /// New-player round 6, #1: a saved mission stays a mission when it is loaded again.
    #[test]
    fn the_mission_travels_in_the_save() {
        let meta = SaveMeta {
            mission: Some("europe".into()),
            map: Some("/maps/isle.rrmap".into()),
            land_goal: 60,
            behind_told: 25_200,
            stories: 5,
            castles_lost: vec![(1, 0)],
            peace: 15,
            time_limit: 90,
            coins_goal: 60,
            goal_done: true,
            level: Some(2),
            player: Some(1),
            deeds: 0b11,
            favour: 42,
        };
        let (_, _, _, back) = decode_ui(&encode_ui(&Messages::new(), 0, 0, &meta)).unwrap();
        assert_eq!(back, meta);
        // the three quarters of saves from before are a land goal of 75 % (design round 1, #6)
        let quarters = br#"{"messages": [], "tips_shown": 1, "read_until": 2, "three_quarters": true}"#;
        assert_eq!(decode_ui(quarters).unwrap().3.land_goal, 75);
        // saves from before carry none of it
        let old = br#"{"messages": [], "tips_shown": 1, "read_until": 2}"#;
        assert_eq!(decode_ui(old).unwrap().3, SaveMeta::default());
        // a level beyond hard is hard; a broken meta loses the meta, not the messages
        let high = br#"{"messages": [], "tips_shown": 1, "read_until": 2, "level": 7}"#;
        assert_eq!(decode_ui(high).unwrap().3.level, Some(2));
        let bad = br#"{"messages": [[5, "hi", null, 0]], "tips_shown": 1, "read_until": 2, "peace": "x", "mission": "europe"}"#;
        let (msgs, _, _, meta) = decode_ui(bad).unwrap();
        // (only the broken field is lost: QA round 9, #6)
        assert_eq!((msgs.len(), meta.peace, meta.mission.as_deref()), (1, 0, Some("europe")));
        let far = br#"{"messages": [], "tips_shown": 1, "read_until": 2, "level": 300}"#;
        assert_eq!(decode_ui(far).unwrap().3.level, Some(2));
    }

    #[test]
    fn dates_of_saves() {
        assert_eq!(civil_date(0), "1970-01-01");
        assert_eq!(civil_date(1_792_800_000), "2026-10-24");
        assert_eq!(civil_date(951_782_400), "2000-02-29");
    }

    #[test]
    fn ages_of_saves() {
        assert_eq!(age(30), "just now");
        assert_eq!(age(5 * 60 + 3), "5 min ago");
        assert_eq!(age(3 * 3600 + 100), "3 h ago");
        assert_eq!(age(2 * 86400 + 5), "2 days ago");
        assert_eq!(age(86400 + 5), "1 day ago");
    }

    /// German round 1, #3/#4: whole German sentences, and the singular where a count is 1.
    #[test]
    fn german_ages_and_counts_read_right() {
        let de = |(t, n): (&str, u64)| crate::i18n::german(t).unwrap().replace("{1}", &n.to_string());
        assert_eq!(de(age_template(5 * 60 + 3)), "vor 5 Min.");
        assert_eq!(de(age_template(3600)), "vor 1 Std.");
        assert_eq!(de(age_template(86400 + 5)), "vor 1 Tag");
        assert_eq!(de(age_template(3 * 86400)), "vor 3 Tagen");
        let one = crate::i18n::german(undermanned_template(1)).unwrap();
        assert!(one.contains("Eines deiner Militärgebäude hat ") && !one.contains("{1}"), "{one}");
        assert!(crate::i18n::german(undermanned_template(2)).unwrap().contains("{1} deiner Militärgebäude haben"));
        assert!(undermanned_template(1).contains("1 of your military buildings holds"));
        assert_eq!(burned_template(1), "Burned with the lost land: 1 building");
        let two = crate::i18n::msg(burned_template(2), &["2"]);
        assert!(is_burned_line(&two) && is_burned_line(burned_template(1)) && !is_burned_line("Burned"));
        assert!(burned_template(2).contains("{1} buildings"));
        assert!(crate::i18n::has_german(burned_template(1)));
        let behind = crate::i18n::german("Your rivals hold {1}× your land. {2}").unwrap();
        assert!(behind.contains("{1}-mal so viel Land wie du"), "{behind}");
    }

    #[test]
    fn a_save_list_entry_reads_the_header_only() {
        let dir = std::env::temp_dir().join(format!("rr-saves-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("x.rrsave");
        let mut bytes = b"RRG2".to_vec();
        bytes.extend(72_000u32.to_le_bytes());
        bytes.extend([0u8; 100]);
        std::fs::write(&p, bytes).unwrap();
        let e = save_entry(&p).unwrap();
        assert_eq!(e.name, "x");
        assert_eq!(e.gf, Some(72_000));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
