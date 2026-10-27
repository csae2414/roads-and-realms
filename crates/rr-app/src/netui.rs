//! LAN games from the title menu: host a lobby or join one, then start together.

use crate::app::App;
use eframe::egui::{self, RichText};

pub enum Lobby {
    Host(rr_net::HostLobby),
    Client(rr_net::ClientLobby),
}

/// The "Network game" part of the title menu.
pub fn menu_section(app: &mut App, ui: &mut egui::Ui) {
    ui.collapsing(crate::i18n::tr("Network game (LAN)"), |ui| {
        ui.horizontal(|ui| {
            ui.label(crate::i18n::tr("Your name"));
            ui.text_edit_singleline(&mut app.menu.net_name);
        });
        if ui.button(crate::i18n::tr("Host a game")).clicked() {
            match rr_net::HostLobby::open(rr_net::PORT) {
                Ok(l) => app.lobby = Some(Lobby::Host(l)),
                Err(e) => app.menu.net_msg = e,
            }
        }
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut app.menu.net_addr)
                    .hint_text(crate::i18n::tr("host address"))
                    .desired_width(130.0),
            );
            if ui.button(crate::i18n::tr("Join")).clicked() {
                let addr = (app.menu.net_addr.trim().to_string(), rr_net::PORT);
                match rr_net::ClientLobby::join(addr, &app.menu.net_name) {
                    Ok(l) => app.lobby = Some(Lobby::Client(l)),
                    Err(e) => app.menu.net_msg = e,
                }
            }
        });
        if !app.menu.net_msg.is_empty() {
            ui.label(RichText::new(&app.menu.net_msg).small());
        }
    });
}

/// The lobby window while hosting or joining; starts the game.
pub fn lobby_window(app: &mut App, ctx: &egui::Context) {
    let Some(lobby) = app.lobby.take() else { return };
    let name = app.menu.net_name.clone();
    match lobby {
        Lobby::Host(mut l) => {
            if let Err(e) = l.poll(&name) {
                app.menu.net_msg = e;
            }
            let mut start = false;
            let mut cancel = false;
            egui::Window::new(crate::i18n::tr("Hosting a network game"))
                .collapsible(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ctx, |ui| {
                    static ADDRS: std::sync::OnceLock<Vec<std::net::IpAddr>> = std::sync::OnceLock::new();
                    let addrs = ADDRS.get_or_init(rr_net::lan_addresses);
                    if addrs.is_empty() {
                        ui.label(format!("{} {}", crate::i18n::tr("Port"), l.port()));
                    }
                    for a in addrs {
                        ui.label(RichText::new(format!("{}: {}", crate::i18n::tr("Others join at"), a)).strong());
                    }
                    // the build: only the same one can join (compare it on every machine)
                    ui.label(
                        RichText::new(format!("{} {:04x}", crate::i18n::tr("Build id"), rr_net::build_id() >> 48))
                            .small(),
                    );
                    ui.label(RichText::new(crate::i18n::tr("Players:")).small());
                    for (i, n) in l.names(&name).iter().enumerate() {
                        ui.label(RichText::new(n).color(crate::render::player_text_color(i as u8)));
                    }
                    // what a network game takes from the menu, and what it doesn't (net round 5, #2
                    // and #7: it dropped peace, limits, goals and levels without a word)
                    ui.label(
                        RichText::new(crate::i18n::tr(
                            "The map, landscape and start goods: the menu's free game; the other slots go to computer players at normal level.",
                        ))
                        .small(),
                    );
                    ui.label(
                        RichText::new(crate::i18n::tr(
                            "Not in network games: peace time, time limit, land and coin goals, the Players table's levels, allied computer players.",
                        ))
                        .small()
                        .color(egui::Color32::LIGHT_YELLOW),
                    );
                    if crate::rules().hash() != rr_rules::Rules::classic().hash() {
                        ui.label(
                            RichText::new(crate::i18n::tr(
                                "Network games play the classic rules, not those of --rules.",
                            ))
                            .small()
                            .color(egui::Color32::LIGHT_YELLOW),
                        );
                    }
                    ui.horizontal(|ui| {
                        start = ui.button(crate::i18n::tr("Start")).clicked();
                        cancel = ui.button(crate::i18n::tr("Cancel")).clicked();
                    });
                });
            if cancel {
                return;
            }
            if start {
                host_start(app, l, &name);
                return;
            }
            app.lobby = Some(Lobby::Host(l));
        }
        Lobby::Client(l) => {
            let names = l.names.clone();
            match l.poll() {
                Err(e) => app.menu.net_msg = e,
                Ok(Ok((net, (you, map, setup, ais)))) => match rr_formats::rrmap::read(&map) {
                    Ok(map) => app.start_net_game(map, setup, ais, you, net),
                    // (leave, so the others don't wait 30 s for us: net round 4, #7)
                    Err(e) => {
                        let mut net = net;
                        net.leave();
                        app.menu.net_msg = e;
                    }
                },
                Ok(Err(l)) => {
                    let mut cancel = false;
                    egui::Window::new(crate::i18n::tr("Joined a network game"))
                        .collapsible(false)
                        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                        .show(ctx, |ui| {
                            for (i, n) in names.iter().enumerate() {
                                ui.label(RichText::new(n).color(crate::render::player_text_color(i as u8)));
                            }
                            ui.label(crate::i18n::tr("Waiting for the host to start…"));
                            cancel = ui.button(crate::i18n::tr("Cancel")).clicked();
                        });
                    if !cancel {
                        app.lobby = Some(Lobby::Client(l));
                    }
                }
            }
        }
    }
}

/// The host starts: a random map from the menu's free-game settings, one slot per person and
/// computer players in the rest.
fn host_start(app: &mut App, lobby: rr_net::HostLobby, name: &str) {
    let names = lobby.names(name);
    let humans = names.len();
    let total = app.menu.players.max(humans);
    let map = rr_mapgen::generate(&rr_mapgen::GenParams {
        width: app.menu.size,
        height: app.menu.size,
        players: total,
        seed: app.menu.seed,
        landscape: app.menu.landscape,
        islands: app.menu.islands,
        ..Default::default()
    });
    let nations = [rr_sim::Nation::Romans, rr_sim::Nation::Vikings, rr_sim::Nation::Nubians, rr_sim::Nation::Japanese];
    let players = (0..total)
        .map(|i| rr_sim::PlayerSetup {
            name: names.get(i).cloned().unwrap_or_else(|| format!("{} {i}", crate::i18n::tr("AI"))),
            nation: nations[app.menu.table[i % 6].0 as usize % 4],
            team: (app.menu.table[i % 6].1 > 0).then_some(app.menu.table[i % 6].1),
        })
        .collect();
    let setup = rr_sim::GameSetup { seed: app.menu.seed, players, start_goods: app.menu.goods };
    let ais: Vec<(u8, u8)> = (humans..total).map(|p| (p as u8, app.menu.ai_level)).collect();
    match lobby.start(&rr_formats::rrmap::write(&map), &setup, &ais) {
        Ok(net) => app.start_net_game(map, setup, ais, 0, net),
        Err(e) => app.menu.net_msg = e,
    }
}
