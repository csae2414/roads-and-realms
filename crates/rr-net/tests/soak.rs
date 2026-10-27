//! A LAN game with computer players and two humans who click around at random: both machines
//! must end with the same game.
use rr_core::{Direction, MapCoord, Pcg32};
use rr_net::{ClientLobby, HostLobby, Lockstep};
use rr_rules::{BuildingType, Rules};
use rr_sim::{Cmd, GameSetup, PlayerCommand, PlayerSetup, Sim};
use std::sync::Arc;

fn random_cmd(sim: &Sim, rng: &mut Pcg32, me: u8) -> Cmd {
    let g = sim.world().grid();
    let hq = sim.players()[me as usize].hq.map(|h| sim.building(h).pos).unwrap_or(MapCoord::new(10, 10));
    let near = |rng: &mut Pcg32| {
        MapCoord::new(
            (hq.x as i32 + rng.below(21) as i32 - 10).rem_euclid(g.width as i32) as u16,
            (hq.y as i32 + rng.below(21) as i32 - 10).rem_euclid(g.height as i32) as u16,
        )
    };
    match rng.below(5) {
        0 => Cmd::PlaceFlag { at: near(rng) },
        1 => Cmd::PlaceBuilding { at: near(rng), kind: BuildingType::Woodcutter },
        2 => Cmd::BuildRoad { from: near(rng), dirs: vec![Direction::E; 1 + rng.below(4) as usize] },
        3 => Cmd::PlaceBuilding { at: near(rng), kind: BuildingType::Guardhouse },
        _ => Cmd::RemoveFlag { at: near(rng) },
    }
}

#[test]
fn a_lan_game_with_ais_stays_identical() {
    let map =
        rr_mapgen::generate(&rr_mapgen::GenParams { width: 80, height: 80, players: 4, seed: 9, ..Default::default() });
    let setup = GameSetup { seed: 9, players: (0..4).map(|_| PlayerSetup::default()).collect(), ..Default::default() };
    let ais_spec = [(2u8, 1u8), (3, 2)];
    let mut lobby = HostLobby::open(0).unwrap();
    let mut client = ClientLobby::join(("127.0.0.1", lobby.port()), "guest").unwrap();
    for _ in 0..400 {
        lobby.poll("host").unwrap();
        if !lobby.clients.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let host = lobby.start(&rr_formats::rrmap::write(&map), &setup, &ais_spec).unwrap();
    let (guest, (_, bytes, setup2, ais2)) = loop {
        match client.poll().unwrap() {
            Ok(x) => break x,
            Err(c) => client = c,
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    };
    let rules = Arc::new(Rules::classic());
    let make_ais = |spec: &[(u8, u8)]| -> Vec<rr_ai::Ai> {
        spec.iter()
            .map(|(p, l)| rr_ai::Ai::with_level(*p, if *l == 2 { rr_ai::Level::Hard } else { rr_ai::Level::Normal }))
            .collect()
    };
    let mut machines: Vec<(Lockstep, Sim, Vec<rr_ai::Ai>, Pcg32, u8)> = vec![
        (host, Sim::new(rules.clone(), &map, &setup).unwrap(), make_ais(&ais_spec), Pcg32::new(1, 1), 0),
        (
            guest,
            Sim::new(rules, &rr_formats::rrmap::read(&bytes).unwrap(), &setup2).unwrap(),
            make_ais(&ais2),
            Pcg32::new(2, 2),
            1,
        ),
    ];
    let end = 6000;
    let mut stalls = 0;
    while machines.iter().any(|m| m.1.gf().0 < end) {
        let mut progressed = false;
        for (net, sim, ais, rng, me) in machines.iter_mut() {
            // the guest runs up to three frames per round: the two machines drift apart in time
            let steps = if *me == 1 { 3 } else { 1 };
            for _ in 0..steps {
                if sim.gf().0 >= end {
                    break;
                }
                if rng.chance(5) {
                    net.submit(random_cmd(sim, rng, *me));
                }
                let gf = sim.gf().0;
                match net.commands_for(gf, || sim.sync_hash()).unwrap() {
                    Some(mut cmds) => {
                        for ai in ais.iter_mut() {
                            cmds.extend(ai.think(sim));
                        }
                        let cmds: Vec<PlayerCommand> = cmds;
                        sim.step(&cmds);
                        progressed = true;
                    }
                    None => break,
                }
            }
        }
        if !progressed {
            stalls += 1;
            assert!(stalls < 50_000, "stuck");
            std::thread::sleep(std::time::Duration::from_micros(100));
        }
    }
    assert_eq!(machines[0].1.sync_hash(), machines[1].1.sync_hash(), "the machines went apart");
    let buildings = machines[0].1.buildings().len();
    assert!(buildings > 10, "the AIs built something ({buildings})");
}
