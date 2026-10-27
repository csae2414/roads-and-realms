//! LAN games in lockstep. The simulation is deterministic, so players only exchange their
//! commands: time is cut into turns of `TURN_GF` frames, commands given during turn T run at
//! the start of turn T + `DELAY`, and the host gathers everybody's commands for a turn into
//! one bundle that every machine applies in the same order. Computer players run on every
//! machine alike (they are deterministic too). Now and then the clients send a hash of their
//! game, and the host stops the game if they differ.
//!
//! Wire format: a little-endian u32 length, then the message as postcard.

use rr_sim::{Cmd, GameSetup, PlayerCommand, PlayerId};
use std::collections::BTreeMap;
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};

/// Frames per turn (200 ms at normal speed).
pub const TURN_GF: u32 = 4;
/// Turns between giving a command and running it (hides the network delay).
pub const DELAY: u32 = 2;
/// Turns between two hash checks.
pub const HASH_EVERY: u32 = 50;
/// The default port.
pub const PORT: u16 = 7712;

/// This machine's address in the local network, for the host to show to joiners. Asks the
/// system which address it would send from (a UDP "connect" sends nothing).
pub fn lan_addresses() -> Vec<std::net::IpAddr> {
    let probe = |bind: &str, to: &str| {
        let s = std::net::UdpSocket::bind(bind).ok()?;
        s.connect(to).ok()?;
        let ip = s.local_addr().ok()?.ip();
        (!ip.is_loopback() && !ip.is_unspecified()).then_some(ip)
    };
    // documentation addresses: never reached, only routed
    [probe("0.0.0.0:0", "192.0.2.1:9"), probe("[::]:0", "[2001:db8::1]:9")].into_iter().flatten().collect()
}
/// The message format (raise the base for changes to the messages themselves).
/// (Raised with every change of the messages: 3 100 with `Msg::Left`, QA round 7, #4.)
pub const PROTOCOL: u32 = 3_100 + rr_sim::SAVE_VERSION;

/// What a lockstep game needs to be the same on every machine: the simulation and the computer
/// players (they run everywhere), their versions and the rules.
pub fn build_id() -> u64 {
    let mut h = rr_rules::Rules::classic().hash();
    for v in [rr_sim::SAVE_VERSION, rr_sim::SIM_VERSION, rr_ai::AI_VERSION] {
        h = (h ^ v as u64).wrapping_mul(0x100_0000_01b3);
    }
    // and the sources that decide how the game plays (build.rs), so a forgotten version bump
    // can't let two different builds play together
    (h ^ source_hash()).wrapping_mul(0x100_0000_01b3)
}

/// The hash of the game's sources and rules data this build was made from (build.rs).
pub fn source_hash() -> u64 {
    u64::from_str_radix(env!("RR_SOURCE_HASH"), 16).unwrap_or(0)
}
/// The biggest message accepted (a map is a few hundred kilobytes).
const MAX_MSG: usize = 8 << 20;
/// A peer silent this long is gone (a running game sends a turn every 200 ms).
const SILENCE: std::time::Duration = std::time::Duration::from_secs(30);
/// Longest player name.
const MAX_NAME: usize = 32;
/// Before a peer has said hello it may send only this much (a hello is under 100 bytes).
const HELLO_MSG: usize = 1024;
/// What a joined player may send in one message: a turn of commands (a bundle of seven such
/// turns stays far below `MAX_MSG`).
const PLAYER_MSG: usize = 256 * 1024;
/// Commands one player may give in one turn.
const MAX_TURN_CMDS: usize = 64;
/// Messages taken from one peer in one poll (the rest wait: a flood can't freeze the host).
const MSGS_PER_POLL: usize = 64;
/// Connections waiting to say hello, and players besides the host (seven in all).
const MAX_WAITING: usize = 8;
pub const MAX_GUESTS: usize = 6;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum Msg {
    Hello {
        name: String,
        protocol: u32,
        /// [`build_id`]: only the same build may play together.
        build: u64,
    },
    /// Host to clients: who is in the lobby.
    Lobby {
        names: Vec<String>,
    },
    /// Host to a client: the game starts; `you` is the client's player, `map` a `.rrmap`.
    Start {
        you: PlayerId,
        map: Vec<u8>,
        setup: GameSetup,
        ais: Vec<(PlayerId, u8)>,
    },
    /// Client to host: my commands for `turn`.
    Turn {
        turn: u32,
        cmds: Vec<Cmd>,
    },
    /// Host to all: everybody's commands for `turn`, in the order to run them.
    Bundle {
        turn: u32,
        cmds: Vec<PlayerCommand>,
    },
    /// Client to host: my game's hash at the start of `turn`.
    Hash {
        turn: u32,
        hash: u64,
    },
    /// Host to all: the games differ; the game ends.
    Desync {
        turn: u32,
    },
    Bye,
    /// Host to the others: a player is out of the game (left, or its game went apart: `desync`).
    Left {
        player: PlayerId,
        desync: bool,
    },
}

/// A connection that sends and receives whole messages without blocking.
pub struct Conn {
    stream: TcpStream,
    buf: Vec<u8>,
    /// The other side closed the connection.
    eof: bool,
    /// When a message last arrived.
    heard: std::time::Instant,
    /// How long the other side may be silent before it counts as gone.
    silence: std::time::Duration,
    /// Where the next message starts in `buf` (consumed bytes are dropped in batches).
    start: usize,
    /// The biggest message accepted from this peer.
    limit: usize,
    /// A send failed: the peer is dropped at the next poll.
    dead: bool,
    /// Host side: when this player's last turn came (only turns count as alive in a game).
    last_turn: std::time::Instant,
}

impl Conn {
    /// Silent for longer than a running game ever is (turns flow every 200 ms)?
    fn silent(&self) -> bool {
        self.heard.elapsed() > self.silence
    }

    pub fn new(stream: TcpStream) -> std::io::Result<Self> {
        stream.set_nonblocking(true)?;
        stream.set_nodelay(true)?;
        // a peer that stops reading must not freeze us for long
        stream.set_write_timeout(Some(std::time::Duration::from_secs(3)))?;
        Ok(Self {
            stream,
            buf: Vec::new(),
            eof: false,
            heard: std::time::Instant::now(),
            silence: SILENCE,
            start: 0,
            limit: MAX_MSG,
            dead: false,
            last_turn: std::time::Instant::now(),
        })
    }

    pub fn send(&mut self, m: &Msg) -> Result<(), String> {
        let body = postcard::to_stdvec(m).map_err(|e| e.to_string())?;
        let mut out = (body.len() as u32).to_le_bytes().to_vec();
        out.extend(body);
        // a short blocking write (with a timeout): messages are small
        self.stream.set_nonblocking(false).map_err(|e| e.to_string())?;
        let r = self.stream.write_all(&out).map_err(|e| e.to_string());
        self.stream.set_nonblocking(true).map_err(|e| e.to_string())?;
        // a peer that can't take what it is sent is gone (else every send blocks again)
        self.dead |= r.is_err();
        r
    }

    /// The next whole message, if one has arrived.
    pub fn recv(&mut self) -> Result<Option<Msg>, String> {
        let mut chunk = [0u8; 65536];
        // drop what was consumed (in one go, not after every message: a flood of small
        // messages made that quadratic)
        if self.start > 0 && (self.start >= self.buf.len() / 2 || self.buf.len() - self.start < 4) {
            self.buf.drain(..self.start);
            self.start = 0;
        }
        // read what is there, but never more than one biggest message ahead
        while !self.eof && self.buf.len() - self.start <= self.limit + 4 {
            match self.stream.read(&mut chunk) {
                Ok(0) => self.eof = true,
                Ok(n) => self.buf.extend_from_slice(&chunk[..n]),
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(e) => return Err(e.to_string()),
            }
        }
        let left = || Err("the other side left".to_string());
        let b = &self.buf[self.start..];
        if b.len() < 4 {
            return if self.eof { left() } else { Ok(None) };
        }
        let len = u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize;
        if len > self.limit {
            return Err(format!("a message of {len} bytes is too big"));
        }
        if b.len() < 4 + len {
            return if self.eof { left() } else { Ok(None) };
        }
        let m = postcard::from_bytes(&b[4..4 + len]).map_err(|e| e.to_string())?;
        self.start += 4 + len;
        self.heard = std::time::Instant::now();
        Ok(Some(m))
    }
}

/// The host's lobby: people join until the host starts the game.
pub struct HostLobby {
    listener: TcpListener,
    pub clients: Vec<(Conn, String)>,
    /// Connections that have not said hello yet, with the time they came.
    waiting: Vec<(Conn, std::time::Instant)>,
}

impl HostLobby {
    pub fn open(port: u16) -> Result<Self, String> {
        let listener = TcpListener::bind(("0.0.0.0", port)).map_err(|e| e.to_string())?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        Ok(Self { listener, clients: Vec::new(), waiting: Vec::new() })
    }

    pub fn port(&self) -> u16 {
        self.listener.local_addr().map_or(0, |a| a.port())
    }

    /// Take in new players, drop the ones who left, and tell everybody who is there. Never
    /// waits: a connection that says nothing is dropped after a few seconds.
    pub fn poll(&mut self, host_name: &str) -> Result<(), String> {
        while let Ok((s, _)) = self.listener.accept() {
            // a few at a time, and only small messages until they say hello
            if self.waiting.len() < MAX_WAITING
                && let Ok(mut c) = Conn::new(s)
            {
                c.limit = HELLO_MSG;
                self.waiting.push((c, std::time::Instant::now()));
            }
        }
        let mut changed = false;
        let mut still = Vec::new();
        for (mut c, since) in std::mem::take(&mut self.waiting) {
            match c.recv() {
                Ok(Some(Msg::Hello { name, protocol, build }))
                    if protocol == PROTOCOL && build == build_id() && self.clients.len() < MAX_GUESTS =>
                {
                    let name: String = name.chars().filter(|ch| !ch.is_control()).take(MAX_NAME).collect();
                    c.limit = PLAYER_MSG;
                    self.clients.push((c, name));
                    changed = true;
                }
                Ok(Some(Msg::Hello { .. })) => {
                    let _ = c.send(&Msg::Bye);
                }
                Ok(None) if since.elapsed() < std::time::Duration::from_secs(3) => still.push((c, since)),
                _ => {}
            }
        }
        self.waiting = still;
        // people who leave the lobby (any error counts for that connection only)
        let before = self.clients.len();
        self.clients.retain_mut(|(c, _)| matches!(c.recv(), Ok(None) | Ok(Some(Msg::Lobby { .. }))));
        changed |= self.clients.len() != before;
        if changed {
            let names = self.names(host_name);
            self.clients.retain_mut(|(c, _)| c.send(&Msg::Lobby { names: names.clone() }).is_ok());
        }
        Ok(())
    }

    pub fn names(&self, host_name: &str) -> Vec<String> {
        std::iter::once(host_name.to_string()).chain(self.clients.iter().map(|c| c.1.clone())).collect()
    }

    /// Start: the host is player 0, the clients 1, 2, …; `setup` has a slot for each.
    pub fn start(mut self, map: &[u8], setup: &GameSetup, ais: &[(PlayerId, u8)]) -> Result<Lockstep, String> {
        let mut conns = Vec::new();
        for (i, (mut c, _)) in self.clients.drain(..).enumerate() {
            let you = i as PlayerId + 1;
            c.send(&Msg::Start { you, map: map.to_vec(), setup: setup.clone(), ais: ais.to_vec() })?;
            // the game's clock starts now, not when they joined the lobby
            c.last_turn = std::time::Instant::now();
            conns.push((c, you));
        }
        let mut humans: Vec<PlayerId> = vec![0];
        humans.extend(conns.iter().map(|c| c.1));
        Ok(Lockstep::new(
            0,
            Role::Host {
                conns,
                humans,
                pending: BTreeMap::new(),
                hashes: BTreeMap::new(),
                their_hashes: BTreeMap::new(),
                apart: Vec::new(),
                next_bundle: DELAY,
            },
        ))
    }
}

/// A client waiting in someone's lobby.
pub struct ClientLobby {
    conn: Conn,
    pub names: Vec<String>,
}

/// What a client gets when the host starts: its player, the map file, the setup, the AIs.
pub type StartInfo = (PlayerId, Vec<u8>, GameSetup, Vec<(PlayerId, u8)>);

impl ClientLobby {
    pub fn join(addr: impl ToSocketAddrs, name: &str) -> Result<Self, String> {
        let addr = addr.to_socket_addrs().map_err(|e| e.to_string())?.next().ok_or("no address")?;
        let s = TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(5)).map_err(|e| e.to_string())?;
        let mut conn = Conn::new(s).map_err(|e| e.to_string())?;
        conn.send(&Msg::Hello { name: name.into(), protocol: PROTOCOL, build: build_id() })?;
        Ok(Self { conn, names: Vec::new() })
    }

    /// The game, once the host starts it.
    pub fn poll(mut self) -> Result<Result<(Lockstep, StartInfo), Self>, String> {
        while let Some(m) = self.conn.recv()? {
            match m {
                Msg::Lobby { names } => self.names = names,
                Msg::Start { you, map, setup, ais } => {
                    let ls = Lockstep::new(you, Role::Client { conn: self.conn });
                    return Ok(Ok((ls, (you, map, setup, ais))));
                }
                Msg::Bye => return Err("the host closed the game".into()),
                _ => {}
            }
        }
        Ok(Err(self))
    }
}

enum Role {
    Host {
        /// Each client's connection and player.
        conns: Vec<(Conn, PlayerId)>,
        /// The players still in the game (the host first).
        humans: Vec<PlayerId>,
        /// turn -> player -> commands
        pending: BTreeMap<u32, BTreeMap<PlayerId, Vec<Cmd>>>,
        /// turn -> the host's own hash
        hashes: BTreeMap<u32, u64>,
        /// turn -> the clients' hashes that came before the host's (one per player)
        their_hashes: BTreeMap<u32, BTreeMap<PlayerId, u64>>,
        /// Players whose game went apart from the host's: they are out.
        apart: Vec<PlayerId>,
        /// The first turn not bundled yet.
        next_bundle: u32,
    },
    Client {
        conn: Conn,
    },
}

/// One player's side of a running LAN game.
pub struct Lockstep {
    pub me: PlayerId,
    role: Role,
    /// Bundles ready to run, by turn.
    bundles: BTreeMap<u32, Vec<PlayerCommand>>,
    /// My commands given during this turn.
    local: Vec<Cmd>,
    /// The next turn my commands go to.
    next_seal: u32,
    /// Players who left the game since the last look (the host sees it, and tells the guests).
    pub left: Vec<PlayerId>,
    /// Of those who left: the players whose game went apart from the host's (a desync, not a
    /// quit; LAN round 3, #5).
    pub desynced: Vec<PlayerId>,
    /// Turns bundled so far, as far as this machine knows (the others' progress).
    reached: u32,
    /// The last check turn whose hash went out.
    last_hash: u32,
}

impl Lockstep {
    /// Test hook: send a sync hash for any turn, as a misbehaving peer could.
    #[doc(hidden)]
    pub fn debug_send_hash(&mut self, turn: u32, hash: u64) {
        if let Role::Client { conn } = &mut self.role {
            let _ = conn.send(&Msg::Hash { turn, hash });
        }
    }

    /// Test hook: send an empty command turn for any turn number.
    #[doc(hidden)]
    pub fn debug_send_turn(&mut self, turn: u32) {
        if let Role::Client { conn } = &mut self.role {
            let _ = conn.send(&Msg::Turn { turn, cmds: Vec::new() });
        }
    }

    fn new(me: PlayerId, role: Role) -> Self {
        Self {
            me,
            role,
            bundles: BTreeMap::new(),
            local: Vec::new(),
            next_seal: DELAY,
            left: Vec::new(),
            desynced: Vec::new(),
            reached: 0,
            last_hash: 0,
        }
    }

    /// How long a peer may say nothing before it counts as gone (tests use short ones).
    pub fn set_silence(&mut self, d: std::time::Duration) {
        match &mut self.role {
            Role::Client { conn } => conn.silence = d,
            Role::Host { conns, .. } => conns.iter_mut().for_each(|(c, _)| c.silence = d),
        }
    }

    pub fn is_host(&self) -> bool {
        matches!(self.role, Role::Host { .. })
    }

    /// A command of this player; it runs `DELAY` turns from now on every machine.
    pub fn submit(&mut self, cmd: Cmd) {
        self.local.push(cmd);
    }

    /// The commands to run at frame `gf`, or `None` while still waiting for the others.
    /// Call this before every step; `hash` gives the game's hash for the checks.
    pub fn commands_for(&mut self, gf: u32, hash: impl FnOnce() -> u64) -> Result<Option<Vec<PlayerCommand>>, String> {
        self.poll()?;
        if !gf.is_multiple_of(TURN_GF) {
            return Ok(Some(Vec::new()));
        }
        let turn = gf / TURN_GF;
        // at each turn start, my commands so far go out for a later turn (once per turn)
        if self.next_seal <= turn + DELAY {
            let cmds = std::mem::take(&mut self.local);
            let t = self.next_seal;
            self.next_seal += 1;
            self.seal(t, cmds)?;
        }
        // (also for turns the keep-alive sealed while the window wasn't drawn)
        if turn > 0 && turn.is_multiple_of(HASH_EVERY) && turn > self.last_hash {
            self.last_hash = turn;
            let h = hash();
            self.send_hash(turn, h)?;
        }
        if turn < DELAY {
            return Ok(Some(Vec::new()));
        }
        self.poll()?;
        Ok(self.bundles.remove(&turn))
    }

    /// How many turns' bundles are waiting beyond the turn after `gf`'s: a game that far
    /// behind should run without its usual pace until it has caught up (net round 4, #1).
    pub fn queued_ahead(&self, gf: u32) -> u32 {
        let next = gf / TURN_GF + 1;
        self.bundles.range(next..).count() as u32
    }

    /// Keep this player in the game while its window isn't drawn (LAN round 3, #4: a hidden or
    /// dragged window froze everyone, then was dropped): read the network and seal empty turns,
    /// never further ahead than the others have got. Valid lockstep: the player gives no
    /// commands. The game here catches up when `commands_for` is called again.
    pub fn keep_alive(&mut self) -> Result<(), String> {
        self.poll()?;
        while self.next_seal <= self.reached + DELAY {
            let t = self.next_seal;
            self.next_seal += 1;
            self.seal(t, Vec::new())?;
        }
        Ok(())
    }

    fn seal(&mut self, turn: u32, cmds: Vec<Cmd>) -> Result<(), String> {
        let me = self.me;
        match &mut self.role {
            Role::Client { conn } => conn.send(&Msg::Turn { turn, cmds }),
            Role::Host { pending, .. } => {
                pending.entry(turn).or_default().insert(me, cmds);
                self.bundle_ready()
            }
        }
    }

    fn send_hash(&mut self, turn: u32, hash: u64) -> Result<(), String> {
        match &mut self.role {
            Role::Client { conn } => conn.send(&Msg::Hash { turn, hash }),
            Role::Host { hashes, their_hashes, apart, .. } => {
                hashes.insert(turn, hash);
                // clients that got there first: whoever differs from the host is out (the
                // game goes on for the others)
                for (player, h) in their_hashes.remove(&turn).unwrap_or_default() {
                    if h != hash {
                        apart.push(player);
                    }
                }
                // old hashes are no longer needed
                hashes.retain(|t, _| *t + 4 * HASH_EVERY > turn);
                Ok(())
            }
        }
    }

    /// Read the network; the host also turns complete turns into bundles.
    fn poll(&mut self) -> Result<(), String> {
        match &mut self.role {
            Role::Client { conn } => {
                while let Some(m) = conn.recv()? {
                    match m {
                        Msg::Bundle { turn, cmds } => {
                            // only turns that can still come up (a host can't fill our memory;
                            // a window not drawn for minutes still fits)
                            if self.bundles.len() >= 4096 {
                                return Err("the host sends more than a game needs".into());
                            }
                            self.reached = self.reached.max(turn.saturating_add(1));
                            self.bundles.insert(turn, cmds);
                        }
                        Msg::Desync { turn } => return Err(format!("the games went apart at turn {turn}")),
                        Msg::Bye => return Err("the host left the game".into()),
                        // who else is out (net round 4, #6: guests were never told)
                        Msg::Left { player, desync } => {
                            if !self.left.contains(&player) {
                                self.left.push(player);
                            }
                            if desync && !self.desynced.contains(&player) {
                                self.desynced.push(player);
                            }
                        }
                        _ => {}
                    }
                }
                if conn.silent() {
                    return Err("the host does not answer".into());
                }
                Ok(())
            }
            Role::Host { conns, humans, pending, hashes, their_hashes, apart, next_bundle } => {
                let mut gone = Vec::new();
                for (i, (c, player)) in conns.iter_mut().enumerate() {
                    let mut taken = 0;
                    loop {
                        if taken >= MSGS_PER_POLL {
                            break;
                        }
                        taken += 1;
                        match c.recv() {
                            Ok(Some(Msg::Turn { turn, cmds })) => {
                                // only turns that can still run, and no more commands than a
                                // player can mean; anything else is dropped
                                // (peer numbers: saturating, so no turn a peer sends can overflow;
                                // LAN round 3, #1)
                                if (*next_bundle..=next_bundle.saturating_add(DELAY + 1)).contains(&turn) {
                                    let mut cmds = cmds;
                                    cmds.truncate(MAX_TURN_CMDS);
                                    pending.entry(turn).or_default().insert(*player, cmds);
                                    c.last_turn = std::time::Instant::now();
                                }
                            }
                            // hashes only for check turns near now, one per player
                            Ok(Some(Msg::Hash { turn, hash }))
                                if turn.is_multiple_of(HASH_EVERY)
                                    && turn.saturating_add(4 * HASH_EVERY) > *next_bundle
                                    && turn < next_bundle.saturating_add(2 * HASH_EVERY) =>
                            {
                                match hashes.get(&turn) {
                                    Some(h) if *h != hash => apart.push(*player),
                                    Some(_) => {}
                                    // the host gets there later: compare then
                                    None => {
                                        their_hashes.entry(turn).or_default().insert(*player, hash);
                                    }
                                }
                            }
                            Ok(Some(Msg::Bye)) | Err(_) => {
                                gone.push(i);
                                break;
                            }
                            Ok(Some(_)) => {}
                            Ok(None) => {
                                // talking is not playing: only turns keep a player in the game
                                if c.silent() || c.last_turn.elapsed() > c.silence || c.dead {
                                    gone.push(i);
                                }
                                break;
                            }
                        }
                    }
                }
                // a game that went apart from the host's ends for that player only
                for player in std::mem::take(apart) {
                    if !self.desynced.contains(&player) {
                        self.desynced.push(player);
                    }
                    if let Some(i) = conns.iter().position(|c| c.1 == player) {
                        let turn = *next_bundle;
                        let _ = conns[i].0.send(&Msg::Desync { turn });
                        if !gone.contains(&i) {
                            gone.push(i);
                        }
                    }
                }
                gone.sort_unstable();
                // a player who leaves is out; the game goes on for the others
                for i in gone.into_iter().rev() {
                    let (_, player) = conns.remove(i);
                    humans.retain(|h| *h != player);
                    for by in pending.values_mut() {
                        by.remove(&player);
                    }
                    self.left.push(player);
                    let desync = self.desynced.contains(&player);
                    for (c, _) in conns.iter_mut() {
                        let _ = c.send(&Msg::Left { player, desync });
                    }
                }
                their_hashes.retain(|t, _| *t + 4 * HASH_EVERY > *next_bundle);
                self.bundle_ready()
            }
        }
    }

    /// Host: every complete turn becomes a bundle, sent to all and kept for itself.
    fn bundle_ready(&mut self) -> Result<(), String> {
        let Role::Host { conns, humans, pending, next_bundle, .. } = &mut self.role else { return Ok(()) };
        loop {
            let turn = *next_bundle;
            let complete = pending.get(&turn).is_some_and(|by| humans.iter().all(|h| by.contains_key(h)));
            if !complete {
                return Ok(());
            }
            let by = pending.remove(&turn).expect("complete turn");
            // in player order, so every machine runs them alike
            let cmds: Vec<PlayerCommand> = by
                .into_iter()
                .flat_map(|(player, cmds)| cmds.into_iter().map(move |cmd| PlayerCommand { player, cmd }))
                .collect();
            // a client that can't take the bundle is dropped at the next poll
            for (c, _) in conns.iter_mut() {
                let _ = c.send(&Msg::Bundle { turn, cmds: cmds.clone() });
            }
            self.bundles.insert(turn, cmds);
            *next_bundle += 1;
            self.reached = *next_bundle;
        }
    }

    /// Leave the game.
    pub fn leave(&mut self) {
        match &mut self.role {
            Role::Client { conn } => {
                let _ = conn.send(&Msg::Bye);
            }
            Role::Host { conns, .. } => {
                for (c, _) in conns.iter_mut() {
                    let _ = c.send(&Msg::Bye);
                }
            }
        }
    }
}

/// A running LAN game that stays alive while the window isn't drawn (LAN round 3, #4): a
/// thread calls `keep_alive` whenever the game hasn't been asked for a second.
pub struct LiveLockstep {
    inner: std::sync::Arc<std::sync::Mutex<Lockstep>>,
    ui_seen: std::sync::Arc<std::sync::Mutex<std::time::Instant>>,
    error: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl LiveLockstep {
    pub fn new(l: Lockstep) -> Self {
        use std::sync::{Arc, Mutex, atomic::AtomicBool, atomic::Ordering};
        let inner = Arc::new(Mutex::new(l));
        let ui_seen = Arc::new(Mutex::new(std::time::Instant::now()));
        let error = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let (i, u, e, s) = (inner.clone(), ui_seen.clone(), error.clone(), stop.clone());
        let thread = std::thread::spawn(move || {
            while !s.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(100));
                let away = u.lock().map(|t| t.elapsed() > std::time::Duration::from_secs(1)).unwrap_or(false);
                if away
                    && let Ok(mut l) = i.lock()
                    && let Err(err) = l.keep_alive()
                {
                    if let Ok(mut slot) = e.lock() {
                        *slot = Some(err);
                    }
                    break;
                }
            }
        });
        Self { inner, ui_seen, error, stop, thread: Some(thread) }
    }

    /// Work with the game (from the UI): this also tells the keep-alive the window is drawn.
    /// An error the keep-alive met comes first.
    pub fn with<R>(&self, f: impl FnOnce(&mut Lockstep) -> Result<R, String>) -> Result<R, String> {
        if let Ok(mut t) = self.ui_seen.lock() {
            *t = std::time::Instant::now();
        }
        if let Some(e) = self.error.lock().ok().and_then(|mut e| e.take()) {
            return Err(e);
        }
        let mut l = self.inner.lock().map_err(|_| "the network game broke down".to_string())?;
        f(&mut l)
    }
}

impl Drop for LiveLockstep {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
