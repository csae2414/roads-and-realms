use rr_core::{MapCoord, MapData};
use rr_net::{ClientLobby, HostLobby, Lockstep};
use rr_rules::Rules;
use rr_sim::{Cmd, GameSetup, PlayerSetup, Sim};
use std::sync::Arc;

#[derive(Clone, Copy, Default)]
struct Case {
    /// Change the guest's game behind the network's back.
    tamper: bool,
    /// Guest steps per host step (> 1: the guest runs ahead whenever it can).
    guest_speed: u32,
    /// The guest leaves at this frame.
    guest_leaves: Option<u32>,
}

#[test]
fn two_players_stay_in_step() {
    run(Case { guest_speed: 1, ..Default::default() });
}

/// A game that drifts apart is noticed at the next hash check.
#[test]
fn a_desync_is_detected() {
    run(Case { tamper: true, guest_speed: 1, ..Default::default() });
}

/// ...also when the guest gets to the check before the host.
#[test]
fn a_desync_is_detected_when_the_guest_runs_ahead() {
    run(Case { tamper: true, guest_speed: 3, ..Default::default() });
}

/// A player who leaves doesn't end the game for the others.
#[test]
fn the_host_plays_on_when_a_guest_leaves() {
    run(Case { guest_speed: 1, guest_leaves: Some(300), ..Default::default() });
}

/// A connection that never says hello (or closes at once) doesn't disturb the lobby.
#[test]
fn the_lobby_survives_silent_connections() {
    let mut lobby = HostLobby::open(0).unwrap();
    let port = lobby.port();
    let silent = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    drop(std::net::TcpStream::connect(("127.0.0.1", port)).unwrap());
    let _guest = ClientLobby::join(("127.0.0.1", port), "guest").unwrap();
    let t0 = std::time::Instant::now();
    for _ in 0..200 {
        lobby.poll("host").unwrap();
        if !lobby.clients.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(lobby.names("host"), vec!["host".to_string(), "guest".to_string()]);
    assert!(t0.elapsed() < std::time::Duration::from_secs(2), "polling must not wait for silent peers");
    drop(silent);
}

fn connect() -> (Lockstep, Lockstep, MapData, GameSetup, MapData, GameSetup) {
    let mut map = MapData::flat(64, 64, 8);
    map.hq = vec![Some(MapCoord::new(14, 20)), Some(MapCoord::new(46, 20))];
    let setup =
        GameSetup { seed: 3, players: vec![PlayerSetup::default(), PlayerSetup::default()], ..Default::default() };
    let mut lobby = HostLobby::open(0).unwrap();
    let port = lobby.port();
    let mut client = ClientLobby::join(("127.0.0.1", port), "guest").unwrap();
    for _ in 0..200 {
        lobby.poll("host").unwrap();
        if !lobby.clients.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(lobby.names("host"), vec!["host".to_string(), "guest".to_string()]);
    let host = lobby.start(&rr_formats::rrmap::write(&map), &setup, &[]).unwrap();
    let (guest, (you, map_bytes, setup2, _)) = loop {
        match client.poll().unwrap() {
            Ok(x) => break x,
            Err(c) => client = c,
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    };
    assert_eq!(you, 1);
    let map2 = rr_formats::rrmap::read(&map_bytes).unwrap();
    (host, guest, map, setup, map2, setup2)
}

fn run(case: Case) {
    let (mut host, mut guest, map, setup, map2, setup2) = connect();
    let rules = Arc::new(Rules::classic());
    let mut a = Sim::new(rules.clone(), &map, &setup).unwrap();
    let mut b = Sim::new(rules, &map2, &setup2).unwrap();
    let (fa, fb) = (MapCoord::new(18, 22), MapCoord::new(42, 22));
    let end = 1200;
    let mut stalls = 0;
    let mut guest_on = true;
    let mut desync = false;
    while a.gf().0 < end || (guest_on && b.gf().0 < end) {
        let mut progressed = false;
        if a.gf().0 < end {
            if a.gf().0 == 40 {
                host.submit(Cmd::PlaceFlag { at: fa });
            }
            match host.commands_for(a.gf().0, || a.sync_hash()) {
                Ok(Some(cmds)) => {
                    a.step(&cmds);
                    progressed = true;
                }
                Ok(None) => {}
                Err(e) => {
                    assert!(case.tamper && e.contains("apart"), "{e}");
                    desync = true;
                    break;
                }
            }
        }
        for _ in 0..case.guest_speed {
            if !guest_on || b.gf().0 >= end {
                break;
            }
            if Some(b.gf().0) == case.guest_leaves {
                guest.leave();
                guest_on = false;
                break;
            }
            if b.gf().0 == 77 {
                guest.submit(Cmd::PlaceFlag { at: fb });
                if case.tamper {
                    // something only this machine sees
                    b.debug_add_ware(1, rr_rules::Ware::Gold, 1);
                }
            }
            match guest.commands_for(b.gf().0, || b.sync_hash()) {
                Ok(Some(cmds)) => {
                    b.step(&cmds);
                    progressed = true;
                }
                Ok(None) => break,
                Err(e) => {
                    assert!(case.tamper && e.contains("apart"), "{e}");
                    desync = true;
                    guest_on = false;
                    break;
                }
            }
        }
        if desync && !case.tamper {
            panic!("desync");
        }
        if !progressed {
            stalls += 1;
            assert!(stalls < 20_000, "stuck at {} / {}", a.gf().0, b.gf().0);
            std::thread::sleep(std::time::Duration::from_micros(200));
        }
    }
    if case.tamper {
        assert!(desync, "the desync went unnoticed");
        // robustness roast #8: the player whose game went apart is out; the host plays on
        assert_eq!(host.left, vec![1]);
        // LAN round 3, #5: and the host knows it was a desync, not a quit
        assert_eq!(host.desynced, vec![1]);
        return;
    }
    if let Some(at) = case.guest_leaves {
        assert_eq!(a.gf().0, end, "the host played on");
        assert_eq!(host.left, vec![1], "the host noticed");
        assert!(at < end);
        return;
    }
    assert!(a.flag_at(fa).is_some() && a.flag_at(fb).is_some(), "both flags on the host");
    assert_eq!(a.sync_hash(), b.sync_hash(), "both games are the same");
}

/// New-player roast #12: the host can tell joiners where to connect.
#[test]
fn the_host_knows_its_lan_address() {
    for ip in rr_net::lan_addresses() {
        assert!(!ip.is_loopback() && !ip.is_unspecified(), "{ip}");
    }
}

/// With three players, one frozen guest stalls the game. The host drops the frozen one after
/// the silence limit and the game goes on; the waiting guest must not give up on the host first
/// (it heard the host after the frozen guest's last turn, so the host's limit runs out first).
#[test]
fn a_frozen_guest_is_dropped_and_the_others_play_on() {
    let mut lobby = HostLobby::open(0).unwrap();
    let port = lobby.port();
    let mut a = Some(ClientLobby::join(("127.0.0.1", port), "a").unwrap());
    std::thread::sleep(std::time::Duration::from_millis(20));
    let mut b = Some(ClientLobby::join(("127.0.0.1", port), "b").unwrap());
    for _ in 0..400 {
        lobby.poll("host").unwrap();
        if lobby.clients.len() == 2 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(lobby.clients.len(), 2);
    let setup = GameSetup { seed: 1, players: vec![PlayerSetup::default(); 3], ..Default::default() };
    let mut host = lobby.start(b"map", &setup, &[]).unwrap();
    let started = |l: &mut Option<ClientLobby>| loop {
        match l.take().unwrap().poll().unwrap() {
            Ok((ls, _)) => break ls,
            Err(c) => *l = Some(c),
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    };
    let mut ga = started(&mut a);
    let mut gb = started(&mut b);
    let silence = std::time::Duration::from_millis(600);
    host.set_silence(silence);
    ga.set_silence(silence);
    let (mut gf_h, mut gf_a, mut gf_b) = (0u32, 0u32, 0u32);
    let t0 = std::time::Instant::now();
    while t0.elapsed() < silence * 4 {
        // b plays along for a while, then freezes
        if gf_b < 60 && gb.commands_for(gf_b, || 0).unwrap().is_some() {
            gf_b += 1;
        }
        if host.commands_for(gf_h, || 0).unwrap().is_some() {
            gf_h += 1;
        }
        match ga.commands_for(gf_a, || 0) {
            Ok(Some(_)) => gf_a += 1,
            Ok(None) => {}
            Err(e) => panic!("guest a gave up at gf {gf_a}: {e}"),
        }
        std::thread::sleep(std::time::Duration::from_millis(3));
    }
    assert_eq!(host.left, vec![2], "the frozen guest is out");
    assert!(gf_a > 100 && gf_h > 100, "the game went on past b's freeze: {gf_h} / {gf_a}");
    // net round 4, #6: the other guest is told too
    assert_eq!(ga.left, vec![2], "guest a learns that b left");
}

/// QA round 3, #5: a build with other rules (even with the same save format) can't join.
#[test]
fn a_different_build_cant_join() {
    let mut lobby = HostLobby::open(0).unwrap();
    let port = lobby.port();
    let s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    let mut c = rr_net::Conn::new(s).unwrap();
    c.send(&rr_net::Msg::Hello { name: "old".into(), protocol: rr_net::PROTOCOL, build: rr_net::build_id() ^ 1 })
        .unwrap();
    let _good = ClientLobby::join(("127.0.0.1", port), "new").unwrap();
    for _ in 0..200 {
        lobby.poll("host").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        if !lobby.clients.is_empty() {
            break;
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(50));
    lobby.poll("host").unwrap();
    assert_eq!(lobby.names("host"), vec!["host".to_string(), "new".to_string()]);
}

/// Robustness roast #6, #13: at most six guests, and nobody can send big messages before
/// saying hello.
#[test]
fn the_lobby_has_limits() {
    use std::io::Write;
    let mut lobby = HostLobby::open(0).unwrap();
    let port = lobby.port();
    // a stranger announces 8 MB before any hello: dropped, the lobby goes on
    let mut rude = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    rude.write_all(&(8u32 << 20).to_le_bytes()).unwrap();
    let _ = rude.write_all(&vec![0u8; 64 * 1024]);
    let guests: Vec<_> = (0..8).map(|i| ClientLobby::join(("127.0.0.1", port), &format!("g{i}")).unwrap()).collect();
    for _ in 0..300 {
        lobby.poll("host").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(lobby.clients.len(), rr_net::MAX_GUESTS);
    drop(guests);
}

/// LAN round 3, #1: a guest's message with a turn number near the end of the range used to
/// overflow the host's check and crash it. Whatever a peer sends, the host doesn't panic.
#[test]
fn odd_turn_numbers_dont_crash_the_host() {
    let (mut host, mut guest, _, _, _, _) = connect();
    // (hashes are checked on every HASH_EVERY-th turn: the largest such numbers too)
    let top = u32::MAX / rr_net::HASH_EVERY * rr_net::HASH_EVERY;
    for turn in [u32::MAX, u32::MAX - 1, top, top - rr_net::HASH_EVERY, u32::MAX / 2, 0] {
        guest.debug_send_hash(turn, 42);
        guest.debug_send_turn(turn);
    }
    for gf in 0..200 {
        let _ = host.commands_for(gf, || 0);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

/// LAN round 3, #2: the build id carries a hash of the game's sources, not only the version
/// numbers (which someone may forget to raise).
#[test]
fn the_build_id_knows_the_sources() {
    assert_ne!(rr_net::source_hash(), 0);
    let mut versions_only = rr_rules::Rules::classic().hash();
    for v in [rr_sim::SAVE_VERSION, rr_sim::SIM_VERSION, rr_ai::AI_VERSION] {
        versions_only = (versions_only ^ v as u64).wrapping_mul(0x100_0000_01b3);
    }
    assert_ne!(rr_net::build_id(), versions_only);
}

/// LAN round 3, #4: a window that isn't drawn (hidden, minimised, dragged) keeps its player in
/// the game: the keep-alive seals empty turns for it, nobody waits, nobody is dropped, and when
/// the window comes back its game catches up.
#[test]
fn a_hidden_window_keeps_its_player_in_the_game() {
    let mut lobby = HostLobby::open(0).unwrap();
    let port = lobby.port();
    let mut a = Some(ClientLobby::join(("127.0.0.1", port), "a").unwrap());
    std::thread::sleep(std::time::Duration::from_millis(20));
    let mut b = Some(ClientLobby::join(("127.0.0.1", port), "b").unwrap());
    for _ in 0..400 {
        lobby.poll("host").unwrap();
        if lobby.clients.len() == 2 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let setup = GameSetup { seed: 1, players: vec![PlayerSetup::default(); 3], ..Default::default() };
    let mut host = lobby.start(b"map", &setup, &[]).unwrap();
    let started = |l: &mut Option<ClientLobby>| loop {
        match l.take().unwrap().poll().unwrap() {
            Ok((ls, _)) => break ls,
            Err(c) => *l = Some(c),
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    };
    let mut ga = started(&mut a);
    let mut gb = started(&mut b);
    let silence = std::time::Duration::from_millis(600);
    host.set_silence(silence);
    ga.set_silence(silence);
    gb.set_silence(silence);
    let (mut gf_h, mut gf_a, mut gf_b) = (0u32, 0u32, 0u32);
    let t0 = std::time::Instant::now();
    while t0.elapsed() < silence * 4 {
        // b plays along for a while, then its window stops drawing: only the keep-alive runs
        if gf_b < 60 {
            if gb.commands_for(gf_b, || 0).unwrap().is_some() {
                gf_b += 1;
            }
        } else {
            gb.keep_alive().unwrap();
        }
        if host.commands_for(gf_h, || 0).unwrap().is_some() {
            gf_h += 1;
        }
        if ga.commands_for(gf_a, || 0).unwrap().is_some() {
            gf_a += 1;
        }
        std::thread::sleep(std::time::Duration::from_millis(3));
    }
    assert!(host.left.is_empty(), "b was dropped: {:?}", host.left);
    assert!(gf_h > 200 && gf_a > 200, "the others waited for b: {gf_h} / {gf_a}");
    // b's window is back: it knows how far behind it is (net round 4, #1: the app replayed the
    // gap at normal speed while everybody waited), and catches up with the turns sealed for it
    gb.keep_alive().unwrap();
    let behind = gb.queued_ahead(gf_b);
    assert!(behind * rr_net::TURN_GF + 60 >= gf_h.min(gf_a) - gf_b, "b sees {behind} turns ahead");
    let target = gf_h.min(gf_a);
    let t1 = std::time::Instant::now();
    while gf_b < target && t1.elapsed() < std::time::Duration::from_secs(5) {
        if gb.commands_for(gf_b, || 0).unwrap().is_some() {
            gf_b += 1;
        } else {
            let _ = host.commands_for(gf_h, || 0);
            let _ = ga.commands_for(gf_a, || 0);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
    assert!(gf_b >= target, "b didn't catch up: {gf_b} of {target}");
}

/// QA round 7, #4: the protocol moved on with `Msg::Left` (a peer of the build before it ended
/// its game when anybody left).
#[test]
fn the_protocol_knows_the_left_message() {
    const { assert!(rr_net::PROTOCOL >= 3_100 + rr_sim::SAVE_VERSION) };
}
