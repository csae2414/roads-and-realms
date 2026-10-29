//! Music and sound effects: a music playlist plus positional sound effects for game events.
//! Assets are CC0 (see `assets/PROVENANCE.md`).

use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, Source};
use rr_core::{Grid, MapCoord};
use rr_sim::{Event, Sim, SoundCue};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

/// The effects: key and its file variants in `assets/audio/sfx`.
const SFX: [(&str, &[&str]); 47] = [
    ("chop", &["chop"]),
    ("treefall", &["treefall"]),
    ("hammer", &["hammer0", "hammer1", "hammer2"]),
    ("anvil", &["anvil0", "anvil1", "anvil2"]),
    ("deer", &["deer0", "deer1", "deer2"]),
    ("refuse", &["refuse"]),
    ("fire", &["fire"]),
    ("bray", &["bray0", "bray1", "bray2"]),
    ("news", &["news"]),
    ("horn", &["horn"]),
    ("collapse", &["collapse"]),
    ("thump", &["thump"]),
    ("pick", &["pick0", "pick1", "pick2"]),
    ("saw", &["saw"]),
    ("sword", &["sword0", "sword1", "sword2"]),
    ("death", &["death0", "death1", "death2"]),
    ("build", &["build"]),
    ("fanfare", &["fanfare"]),
    ("alarm", &["alarm"]),
    ("splash", &["splash0", "splash1"]),
    ("bow", &["bow"]),
    ("scythe", &["scythe"]),
    ("dig", &["dig"]),
    ("creak", &["creak"]),
    ("door", &["door"]),
    ("knife", &["knife"]),
    ("bucket", &["bucket"]),
    ("click", &["click"]),
    ("built", &["built"]),
    ("lost", &["lost"]),
    ("victory", &["victory"]),
    ("defeat", &["defeat"]),
    ("sheep", &["sheep"]),
    ("chirp", &["chirp"]),
    ("duck", &["duck0", "duck1", "duck2"]),
    ("stag", &["stag0", "stag1", "stag2"]),
    ("fox", &["fox0", "fox1", "fox2"]),
    ("launch", &["launch"]),
    ("news_good", &["news_good"]),
    ("news_bad", &["news_bad"]),
    ("bear", &["bear0", "bear1", "bear2"]),
    ("dock", &["dock"]),
    ("catapult", &["catapult"]),
    ("brew", &["brew"]),
    ("pig", &["pig"]),
    ("promote", &["promote"]),
    ("setdown", &["setdown0", "setdown1", "setdown2"]),
];

/// The effect for a sim sound cue.
fn cue_key(c: SoundCue) -> &'static str {
    match c {
        SoundCue::TreeFall => "treefall",
        SoundCue::Chop => "chop",
        SoundCue::Hammer => "hammer",
        SoundCue::Anvil => "anvil",
        SoundCue::Fire => "fire",
        SoundCue::Pick => "pick",
        SoundCue::Saw => "saw",
        SoundCue::Sword => "sword",
        SoundCue::Death => "death",
        SoundCue::Build => "build",
        SoundCue::Fanfare => "fanfare",
        SoundCue::Launch => "launch",
        SoundCue::Dock => "dock",
        SoundCue::Catapult => "catapult",
        SoundCue::Thump => "thump",
        SoundCue::Brew => "brew",
        SoundCue::Pig => "pig",
        SoundCue::Bray => "bray",
        SoundCue::Promote => "promote",
        SoundCue::Splash => "splash",
        SoundCue::Shot => "bow",
        SoundCue::Scythe => "scythe",
        SoundCue::Dig => "dig",
        SoundCue::Creak => "creak",
        SoundCue::Door => "door",
        SoundCue::Knife => "knife",
        SoundCue::Bucket => "bucket",
    }
}

pub struct Audio {
    /// Kept for as long as the game sounds (dropped, the device closes).
    _stream: OutputStream,
    music: Sink,
    /// Looping ambience: birds over land, waves by the open sea, a stream by small waters and
    /// shores, wind in the trees.
    birds: Sink,
    waves: Sink,
    brook: Sink,
    wind: Sink,
    /// Where each bed sounds from, -1 left .. 1 right (waves, stream, birds, wind), read by
    /// its source as it plays.
    bed_pan: [Arc<AtomicU32>; 4],
    /// When the next bird or animal may call.
    next_call: Instant,
    tracks: Vec<Arc<[u8]>>,
    names: Vec<&'static str>,
    /// Short loop files, played several times over with a fade-out.
    loops: Vec<bool>,
    list: Playlist,
    /// Jukebox: play the tracks in random order.
    pub shuffle: bool,
    sfx: Vec<(&'static str, Vec<Clip>)>,
    /// The effects' bus (soft-clipped into the stream).
    fx: rodio::mixer::Mixer,
    /// The view centre, the map, and half the view's width in nodes (sounds fade out a little
    /// beyond the screen edge and pan fully there).
    listener: Option<(MapCoord, Grid, f32)>,
    gate: Gate,
    /// Slider positions 0..1 (see `level_gain`).
    pub master: f32,
    pub music_volume: f32,
    pub sfx_volume: f32,
    pub ambience: f32,
    /// How hard a gust blows over the view's middle now (0..1): the wind in the trees swells
    /// with it, as the trees bend on the screen.
    pub gust: f32,
    /// The music makes room under a jingle or the alarm until then (sound designer #2).
    duck_until: Option<Instant>,
    /// The music's duck level now (eased toward 0.5 under a jingle, 1 otherwise).
    duck: f32,
    /// After a fade: quiet this long before the next track (after victory or defeat).
    hold_after_fade: Option<Duration>,
    /// A skip in progress: the track fades out from then.
    fading_since: Option<Instant>,
    last_update: Option<Instant>,
    /// Fog of war on: only land in sight is heard (else all explored land).
    pub fog_of_war: bool,
    /// For the per-play variation of effects (`vary`, `next_variant`).
    rand: u32,
    last_variant: Vec<(&'static str, usize)>,
    pub muted: bool,
    /// Silent while the window is in the background (when `mute_in_background`).
    pub background: bool,
    pub mute_in_background: bool,
    pick: u32,
    /// The view's zoom (1 normal; zoomed out the realm sounds farther away) and the game's
    /// speed (1 normal), both set by the app.
    pub zoom: f32,
    pub speed: f32,
    /// When each work sound last played (thinned at high speed).
    work_last: Vec<(&'static str, Instant)>,
    next_crackle: Instant,
    /// Sounds waiting out their little delay: when, which, volume, pan.
    later: Vec<(Instant, &'static str, f32, f32)>,
    next_carry: Instant,
    /// The game frame of the last work beats (beats fall between it and now).
    beat_gf: u32,
    /// Building sites heard hammering lately (one hammer per site every 4 s).
    sites: Vec<(MapCoord, Instant)>,
}

/// Where the assets are: `$RR_ASSETS`, next to the executable, or the source tree.
pub fn assets_dir() -> PathBuf {
    if let Ok(p) = std::env::var("RR_ASSETS") {
        return PathBuf::from(p);
    }
    if let Ok(exe) = std::env::current_exe() {
        for dir in exe.ancestors().skip(1).take(4) {
            let p = dir.join("assets");
            if p.join("audio").is_dir() {
                return p;
            }
        }
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

fn load(p: &Path) -> Option<Arc<[u8]>> {
    std::fs::read(p).ok().map(Arc::from)
}

/// A decoder that knows its length: from a bare cursor rodio gives Ogg files none (sound
/// designer 2 #7, #8: the fair loop was cut at a 15 s guess, the beds' random start was dead).
fn decode(data: Arc<[u8]>) -> Option<Decoder<Cursor<Arc<[u8]>>>> {
    let len = data.len() as u64;
    Decoder::builder().with_data(Cursor::new(data)).with_byte_len(len).build().ok()
}

/// A source that never claims an empty first span: on a fresh Ogg decoder that claim makes
/// rodio's `Buffered` end at once, so the beds and the fair loop were silent from the start
/// (QA round 6, #1).
struct Primed<S>(S);

impl<S: Source> Iterator for Primed<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        self.0.next()
    }
}

impl<S: Source> Source for Primed<S> {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> rodio::ChannelCount {
        self.0.channels()
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        self.0.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.0.total_duration()
    }
}

/// An effect decoded once, mono, at its file's rate.
#[derive(Clone)]
pub struct Clip {
    data: Arc<[f32]>,
    rate: u32,
}

/// Decode an effect file into a clip (mono: the channels averaged, as the game plays them).
fn clip(data: Arc<[u8]>) -> Option<Clip> {
    let src = decode(data)?;
    let rate = src.sample_rate();
    let samples: Vec<f32> = MonoMix::new(src).collect();
    (!samples.is_empty()).then(|| Clip { data: samples.into(), rate })
}

/// A clip playing: a plain source with no span to report (a fresh decoder's empty first span
/// made the mixer drop every effect: sound designer round 3, #1, no effect had ever played).
struct ClipSource {
    clip: Clip,
    pos: usize,
}

impl Iterator for ClipSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let s = *self.clip.data.get(self.pos)?;
        self.pos += 1;
        Some(s)
    }
}

impl Source for ClipSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> rodio::ChannelCount {
        1
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        self.clip.rate
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f64(self.clip.data.len() as f64 / f64::from(self.clip.rate)))
    }
}

/// The effects' own bus: a mixer whose sum passes a soft clip before it reaches the stream (a
/// battle at full sliders peaked at +3.4 dBFS: sound designer round 3, #9).
fn effects_bus(rate: u32) -> (rodio::mixer::Mixer, impl Source<Item = f32> + Send + 'static) {
    let (mixer, out) = rodio::mixer::mixer(2, rate);
    (mixer, SoftClipped(out))
}

/// A source with `soft_clip` on every sample.
struct SoftClipped<S>(S);

impl<S: Source> Iterator for SoftClipped<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        self.0.next().map(soft_clip)
    }
}

impl<S: Source> Source for SoftClipped<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.0.current_span_len()
    }

    fn channels(&self) -> rodio::ChannelCount {
        self.0.channels()
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        self.0.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.0.total_duration()
    }
}

/// An effect as it goes into the mixer: a little higher or lower, panned, at `volume`.
fn effect_chain(clip: Clip, pitch: f32, pan: f32, volume: f32) -> impl Source<Item = f32> + Send + 'static {
    let (l, r) = pan_gains(pan);
    rodio::source::ChannelVolume::new(ClipSource { clip, pos: 0 }.speed(pitch), vec![l * volume, r * volume])
}

/// A bed looped for ever, starting `start` into its loop.
fn bed_source(src: Decoder<Cursor<Arc<[u8]>>>, start: Duration) -> impl Source<Item = f32> {
    Primed(src).buffered().repeat_infinite().skip_duration(start)
}

/// The fair loop repeated for `total`, faded over its last seconds.
fn fair_loop(src: Decoder<Cursor<Arc<[u8]>>>, total: Duration) -> impl Source<Item = f32> + use<> {
    let (channels, rate) = (src.channels(), src.sample_rate());
    let all: Vec<f32> = Primed(src).collect();
    // whole frames of the 8-bar body, and as many whole bodies as fit the minute, so every
    // seam falls on the beat and the count is whole frames (sound designer round 3, #3)
    let frames = (FAIR_BODY.as_secs_f64() * f64::from(rate)).round() as usize;
    let body: Arc<[f32]> =
        all[..(frames * channels as usize).min(all.len() / channels as usize * channels as usize)].into();
    let bodies = ((total.as_secs_f64() / FAIR_BODY.as_secs_f64()).floor() as usize).max(1);
    let len = body.len() * bodies;
    LoopSource { body, channels, rate, pos: 0, len }
}

/// The fair tune's 8 bars: 4 phrases of 3.692 s (its file holds a tail and a fade after them).
const FAIR_BODY: Duration = Duration::from_millis(14_768);

/// A body played over and over for `len` samples, faded over its last seconds.
struct LoopSource {
    body: Arc<[f32]>,
    channels: rodio::ChannelCount,
    rate: rodio::SampleRate,
    pos: usize,
    len: usize,
}

impl Iterator for LoopSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.pos >= self.len || self.body.is_empty() {
            return None;
        }
        let s = self.body[self.pos % self.body.len()];
        let per_sec = f64::from(self.rate) * f64::from(self.channels);
        let t = Duration::from_secs_f64(self.pos as f64 / per_sec);
        let total = Duration::from_secs_f64(self.len as f64 / per_sec);
        self.pos += 1;
        Some(s * loop_gain(t, total))
    }
}

impl Source for LoopSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> rodio::ChannelCount {
        self.channels
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        self.rate
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f64(self.len as f64 / (f64::from(self.rate) * f64::from(self.channels))))
    }
}

/// Where a bed starts this session: anywhere in its loop.
fn bed_start(len: Duration, nanos: u32) -> Duration {
    let ms = (len.as_millis() as u64).max(1);
    Duration::from_millis((nanos as u64).wrapping_mul(2_654_435_761) % ms)
}

/// The looped track's gain `t` into its `total`: full, then a fade over the last 4 s (the
/// whole minute was one ramp down).
fn loop_gain(t: Duration, total: Duration) -> f32 {
    let left = total.saturating_sub(t).as_secs_f32();
    (left / 4.0).min(1.0)
}

/// The music's duck level a step `dt` seconds on toward `target`: down in about 50 ms, up
/// over about 800 ms (it was a hard step every frame).
fn ease_duck(level: f32, target: f32, dt: f32) -> f32 {
    let tau = if target < level { 0.05 } else { 0.8 };
    level + (target - level) * (1.0 - (-dt / tau).exp())
}

impl Audio {
    pub fn new() -> Result<Self, String> {
        let mut stream = OutputStreamBuilder::open_default_stream().map_err(|e| e.to_string())?;
        stream.log_on_drop(false);
        let dir = assets_dir().join("audio");
        let mut tracks: Vec<Arc<[u8]>> = Vec::new();
        let mut names = Vec::new();
        let mut loops = Vec::new();
        // (at war: the feast's drums and the exploration's drive, sound designer 2 #7)
        let mut war = Vec::new();
        for (file, title, is_loop) in [
            ("bards", "The Bard's Tale (RandomMind)", false),
            ("inn", "The Old Tower Inn (RandomMind)", false),
            ("feast", "King's Feast (RandomMind)", false),
            ("fair", "Medieval Fair (Woli34)", true),
            ("minstrel", "Minstrel Dance (RandomMind)", false),
            ("harvest", "Harvest Season (RandomMind)", false),
            ("exploration", "Exploration (RandomMind)", false),
        ] {
            if let Some(d) = load(&dir.join("music").join(format!("{file}.ogg"))) {
                // (three of them: two alternated for an hour; sound designer round 3, #8)
                if matches!(file, "feast" | "exploration" | "minstrel") {
                    war.push(tracks.len());
                }
                tracks.push(d);
                names.push(title);
                loops.push(is_loop);
            }
        }
        let sfx = SFX
            .iter()
            .map(|(k, files)| {
                (
                    *k,
                    files
                        .iter()
                        .filter_map(|f| load(&dir.join("sfx").join(format!("{f}.ogg"))).and_then(clip))
                        .collect(),
                )
            })
            .collect();
        let (fx, fx_out) = effects_bus(44_100);
        stream.mixer().add(fx_out);
        let music = Sink::connect_new(stream.mixer());
        music.set_volume(0.35);
        let bed_pan: [Arc<AtomicU32>; 4] = std::array::from_fn(|_| Arc::new(AtomicU32::new(0f32.to_bits())));
        let ambient = |name: &str, pan: &Arc<AtomicU32>| {
            let sink = Sink::connect_new(stream.mixer());
            sink.set_volume(0.0);
            if let Some(data) = load(&dir.join("ambient").join(format!("{name}.ogg")))
                && let Some(src) = decode(data)
            {
                // (the beds are seamless loops, tools/ambience.py; each starts somewhere else)
                let len = src.total_duration().unwrap_or(Duration::from_secs(40));
                let nanos =
                    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
                let start = bed_start(len, nanos ^ name.len() as u32);
                sink.append(Panned::new(MonoMix::new(bed_source(src, start)), pan.clone()));
            }
            sink
        };
        let (birds, waves, brook, wind) = (
            ambient("birds", &bed_pan[2]),
            ambient("waves", &bed_pan[0]),
            ambient("stream", &bed_pan[1]),
            ambient("wind", &bed_pan[3]),
        );
        Ok(Self {
            _stream: stream,
            music,
            birds,
            waves,
            brook,
            wind,
            bed_pan,
            next_call: Instant::now(),
            list: Playlist { n: tracks.len(), war, ..Default::default() },
            loops,
            tracks,
            names,
            shuffle: false,
            sfx,
            fx,
            listener: None,
            gate: Gate::default(),
            master: 1.0,
            music_volume: DEFAULT_MUSIC,
            sfx_volume: DEFAULT_SFX,
            ambience: DEFAULT_AMBIENCE,
            gust: 0.0,
            muted: false,
            background: false,
            mute_in_background: true,
            pick: 0,
            beat_gf: 0,
            zoom: 1.0,
            speed: 1.0,
            work_last: Vec::new(),
            next_crackle: Instant::now(),
            later: Vec::new(),
            next_carry: Instant::now(),
            sites: Vec::new(),
            duck_until: None,
            duck: 1.0,
            hold_after_fade: None,
            fading_since: None,
            last_update: None,
            fog_of_war: false,
            rand: 0x9e37_79b9,
            last_variant: Vec::new(),
        })
    }

    /// Jukebox: the title of the track playing now.
    pub fn track_title(&self) -> &str {
        self.list.playing.and_then(|i| self.names.get(i).copied()).unwrap_or("…")
    }

    /// The realm is at war (attacked lately): short pauses, driving tracks.
    pub fn set_war(&mut self, at_war: bool) {
        let playing_war = self.list.playing.is_some_and(|t| self.list.war.contains(&t));
        // a war that starts during a calm track: it fades, and a war track follows soon
        if war_onset(self.list.tense, at_war, playing_war) && self.list.playing.is_some() {
            self.fading_since.get_or_insert(Instant::now());
            self.hold_after_fade = Some(Duration::from_secs(5));
        }
        if at_war && !self.list.tense {
            self.list.war_since = Some(Instant::now());
        } else if !at_war {
            self.list.war_since = None;
        }
        self.list.tense = at_war;
    }

    /// Jukebox: skip to the next track.
    /// The next track, after a one-second fade of this one (not a hard cut).
    pub fn next_track(&mut self) {
        self.fading_since.get_or_insert(Instant::now());
    }

    /// The node at the center of the view; sounds fade with distance from it.
    pub fn set_listener(&mut self, at: MapCoord, grid: Grid, half_width: f32) {
        self.listener = Some((at, grid, half_width.max(4.0)));
    }

    pub fn play(&mut self, key: &str, volume: f32) {
        // after victory or defeat the music fades and the land is heard a while (sound
        // designer 2 #7: the cheerful playlist went straight on)
        if matches!(key, "victory" | "defeat") {
            self.fading_since.get_or_insert(Instant::now());
            self.hold_after_fade = Some(Duration::from_secs(20));
        }
        self.play_panned(key, volume, 0.0);
    }

    /// What the map's drawn-only life makes heard this frame (a fish's splash, a duck): each at
    /// its place, as loud as the game's own sounds there; and the gust over the view.
    pub fn play_life(&mut self, sounds: &[(&'static str, MapCoord, f32)], gust: f32) {
        self.gust = gust;
        for (key, at, volume) in sounds {
            let (v, pan) = (self.volume_at(*at) * volume, self.pan_at(*at));
            self.play_panned(key, v, pan);
        }
    }

    /// Play a sound; `pan` -1 is left, 1 is right.
    fn play_panned(&mut self, key: &str, volume: f32, pan: f32) {
        // (a sound not loaded spends no slot of the gate: the bears' did)
        let Some((_, variants)) = self.sfx.iter().find(|(k, _)| *k == key) else { return };
        if volume <= HEARD || !self.gate.admit(key, Instant::now()) {
            return;
        }
        if variants.is_empty() {
            return;
        }
        if ducks_music(key, volume) {
            self.duck_until = Some(Instant::now() + Duration::from_millis(1500));
        }
        // a variant other than the last one, a little higher or lower, a little louder or softer
        let (name, n) = (self.sfx.iter().find(|(k, _)| *k == key).map(|(k, _)| *k).unwrap_or(""), variants.len());
        let last = self.last_variant.iter().find(|(k, _)| *k == name).map(|(_, i)| *i);
        let k = next_variant(n, last, &mut self.rand);
        match self.last_variant.iter_mut().find(|(kk, _)| *kk == name) {
            Some(e) => e.1 = k,
            None => self.last_variant.push((name, k)),
        }
        let data = variants[k].clone();
        let (pitch, gain_db) = if JINGLE_KEYS.contains(&key) { (1.0, 0.0) } else { vary(&mut self.rand) };
        let v = volume * self.gain(self.sfx_volume) * 10f32.powf(gain_db / 20.0);
        self.fx.add(effect_chain(data, pitch, pan, v));
    }

    fn volume_at(&self, at: MapCoord) -> f32 {
        match self.listener {
            Some((l, g, half)) => (1.0 - g.distance(l, at) as f32 / (half * 1.4)).max(0.0) * zoom_gain(self.zoom),
            None => 0.5,
        }
    }

    /// Left/right position of a sound relative to the view centre.
    fn pan_at(&self, at: MapCoord) -> f32 {
        let Some((l, g, half)) = self.listener else { return 0.0 };
        let w = g.width as i32;
        let mut dx = at.x as i32 - l.x as i32;
        if dx > w / 2 {
            dx -= w;
        } else if dx < -w / 2 {
            dx += w;
        }
        (dx as f32 / half).clamp(-1.0, 1.0) * 0.8
    }

    pub fn on_event(&mut self, e: &Event, me: u8) {
        match e {
            Event::Sound { at, sound } => {
                let now = Instant::now();
                let (v, pan) = (self.volume_at(*at), self.pan_at(*at));
                // (only what is heard takes a slot: QA round 6, #3)
                if v <= HEARD {
                    return;
                }
                if *sound == SoundCue::Hammer && !site_due(&mut self.sites, *at, now) {
                    return;
                }
                let key = cue_key(*sound);
                if is_work(*sound) && !work_slot(&mut self.work_last, key, self.speed, v, now) {
                    return;
                }
                if *sound == SoundCue::Sword {
                    self.pick = self.pick.wrapping_mul(1103515245).wrapping_add(12345);
                    let roll = ((self.pick >> 8) & 0xffff) as f32 / 65536.0;
                    self.play_later(key, v, pan, roll);
                    return;
                }
                self.play_panned(key, v, pan);
            }
            Event::BuildingFinished { at, .. } if finished_elsewhere(e, me).is_some() => {
                let (key, v) = finished_elsewhere(e, me).unwrap_or(("build", 0.0));
                let (d, pan) = (self.volume_at(*at), self.pan_at(*at));
                self.play_panned(key, d * v, pan);
            }
            _ => {
                if let Some((key, v)) = news_sound(e, me) {
                    self.play(key, v);
                }
            }
        }
    }

    pub fn update(&mut self, sim: &Sim, me: u8) {
        // fades follow time, not the frame rate (paused, the app draws far fewer frames)
        let now = Instant::now();
        let dt = self.last_update.map_or(0.016, |t| now.duration_since(t).as_secs_f32()).min(0.5);
        self.last_update = Some(now);
        let k = 1.0 - (-dt / 0.4).exp();
        let fog = self.fog_of_war;
        self.play_due();
        // a listener of another map (the game was replaced since it was set: a mission started
        // from the title menu, whose sound runs before the new game's first view) waits for the
        // next one; it asked the new, smaller map about its old places and crashed (owner,
        // 2026-10-27)
        if self.listener.is_some_and(|(_, g, _)| g != sim.world().grid()) {
            self.listener = None;
        }
        // ambience follows what is around the view centre, over the whole view
        if let Some((l, g, half)) = self.listener {
            let w = sim.world();
            let mut around = Around::default();
            let mut trees: Vec<MapCoord> = Vec::new();
            let mut fires: Vec<MapCoord> = Vec::new();
            for p in g.within(l, bed_radius(half)) {
                around.n += 1.0;
                // (only land one may hear, from where it lies: sound designer 2 #10)
                let (v, pan) = (sim.visibility(me, p), self.pan_at(p));
                let heard = hear_weight(v, fog);
                if heard == 0.0 {
                    continue;
                }
                let ts = w.terrains_around(p);
                let wet = ts.iter().filter(|t| rr_world::terrain::is_water(**t)).count();
                if wet == 6 {
                    around.add(Bed::Waves, heard, pan);
                } else if wet > 0 {
                    around.add(Bed::Stream, heard, pan);
                } else if ts.iter().all(|t| rr_world::terrain::is_vital(*t)) {
                    around.add(Bed::Birds, heard, pan);
                }
                if matches!(w.obj(p), rr_world::NodeObj::Fire) {
                    fires.push(p);
                }
                if matches!(w.obj(p), rr_world::NodeObj::Tree { .. }) {
                    around.add(Bed::Wind, heard, pan);
                    trees.push(p);
                }
            }
            for (h, p) in self.bed_pan.iter().zip(bed_pans(&around)) {
                let old = f32::from_bits(h.load(Ordering::Relaxed));
                h.store((old + (p - old) * k).to_bits(), Ordering::Relaxed);
            }
            self.work_beats(sim, me, l, g, half);
            self.crackle(&fires, now);
            self.carriers(sim, me, l, g, half, now);
            let mix = ambience_mix(&around, w.landscape);
            let amb = self.gain(self.ambience);
            let ease = |s: &Sink, target: f32| s.set_volume(s.volume() + (target - s.volume()) * k);
            ease(&self.waves, amb * mix[0]);
            ease(&self.brook, amb * mix[1]);
            ease(&self.birds, amb * mix[2]);
            ease(&self.wind, amb * mix[3] * (1.0 + 0.8 * self.gust));
            // now and then a bird in the trees, or a sheep in view, calls
            if now >= self.next_call {
                self.pick = self.pick.wrapping_mul(1103515245).wrapping_add(12345);
                // every 15-40 s (sound designer #6: every ~16 s near the castle was too often)
                let wait = 15 + (self.pick >> 16) % 26;
                self.next_call = now + Duration::from_secs(wait as u64);
                // an animal in view that has a voice (rabbits are quiet)
                let near: Vec<&rr_sim::Figure> = sim
                    .figures()
                    .iter()
                    .filter(|(_, f)| g.distance(f.pos, l) <= half as u32 && audible(sim.visibility(me, f.pos), fog))
                    .map(|(_, f)| f)
                    .collect();
                let wild: Vec<(MapCoord, &'static str)> = near
                    .iter()
                    .filter_map(|f| match f.kind {
                        rr_sim::FigKind::Animal { species } => animal_call(species, w.landscape).map(|k| (f.pos, k)),
                        _ => None,
                    })
                    .collect();
                let tame: Vec<(MapCoord, &'static str)> =
                    near.iter().filter_map(|f| figure_call(f.job).map(|k| (f.pos, k))).collect();
                let pick = (self.pick >> 8) as usize;
                if let Some((at, key)) = pick_call(&wild, &tame, pick / 2).filter(|_| pick % 2 == 0) {
                    let (v, pan) = (self.volume_at(at), self.pan_at(at));
                    self.play_panned(key, v * 0.6, pan);
                } else if around.trees > 6.0 && !trees.is_empty() {
                    // from a tree in view
                    let at = trees[(pick / 2) % trees.len()];
                    let (v, pan) = (self.volume_at(at), self.pan_at(at));
                    let snow = if w.landscape == rr_core::Landscape::Winter { 0.4 } else { 1.0 };
                    self.play_panned("chirp", v * 0.35 * snow * (around.trees / around.n * 4.0).min(1.0), pan);
                }
            }
        }
        if self.music.empty() {
            let rnd = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.subsec_nanos() as usize);
            if let Some(t) = self.list.on_empty(Instant::now(), self.shuffle, rnd)
                && let Some(src) = decode(self.tracks[t].clone())
            {
                if self.loops[t] {
                    // faded in once, not at every repeat, and out over its last seconds
                    let looped = fair_loop(src, Duration::from_secs(60));
                    self.music.append(looped.fade_in(Duration::from_secs(2)));
                } else {
                    self.music.append(src.fade_in(Duration::from_secs(2)));
                }
            }
        }
        // 6 dB down under a jingle or the alarm
        let target = if self.duck_until.is_some_and(|t| Instant::now() < t) { 0.5 } else { 1.0 };
        self.duck = ease_duck(self.duck, target, dt);
        let duck = self.duck;
        // a skip fades the track out over a second, then the next starts
        let fade = match self.fading_since {
            Some(t) if t.elapsed() >= Duration::from_secs(1) => {
                self.fading_since = None;
                match self.hold_after_fade.take() {
                    Some(d) => self.list.hold(Instant::now(), d),
                    None => self.list.skip(),
                }
                self.music.clear();
                self.music.play();
                1.0
            }
            Some(t) => 1.0 - t.elapsed().as_secs_f32(),
            None => 1.0,
        };
        self.music.set_volume(self.gain(self.music_volume) * duck * fade);
    }

    /// Fires in view crackle as long as they burn (a burning ruin stood 3 minutes in silence
    /// after one crackle: sound designer 2 #6): every 1.5-3 s from one of them, louder with more.
    fn crackle(&mut self, fires: &[MapCoord], now: Instant) {
        if fires.is_empty() || now < self.next_crackle {
            return;
        }
        self.pick = self.pick.wrapping_mul(1103515245).wrapping_add(12345);
        self.next_crackle = now + Duration::from_millis(1500 + (self.pick >> 16) as u64 % 1500);
        let at = fires[(self.pick >> 8) as usize % fires.len()];
        let (d, pan) = (self.volume_at(at), self.pan_at(at));
        self.play_panned("fire", d * fire_volume(fires.len()), pan);
    }

    /// Now and then a carrier in view sets its load down.
    fn carriers(&mut self, sim: &Sim, me: u8, l: MapCoord, g: Grid, half: f32, now: Instant) {
        if now < self.next_carry || !work_admitted(self.speed, None, now) {
            return;
        }
        let fog = self.fog_of_war;
        let near: Vec<MapCoord> = sim
            .figures()
            .iter()
            .filter(|(_, f)| matches!(f.kind, rr_sim::FigKind::Carrier { .. }) && f.carry.is_some())
            .filter(|(_, f)| g.distance(f.pos, l) <= half as u32 && audible(sim.visibility(me, f.pos), fog))
            .map(|(_, f)| f.pos)
            .collect();
        self.pick = self.pick.wrapping_mul(1103515245).wrapping_add(12345);
        let roll = ((self.pick >> 8) & 0xffff) as f32 / 65536.0;
        self.next_carry = now + carry_wait(near.len(), roll);
        if near.is_empty() {
            return;
        }
        self.pick = self.pick.wrapping_mul(1103515245).wrapping_add(12345);
        let at = near[(self.pick >> 8) as usize % near.len()];
        let (d, pan) = (self.volume_at(at), self.pan_at(at));
        self.play_panned("setdown", d * 0.6, pan);
    }

    /// The trades at work in view keep on sounding through their cycle: the sim cues only the
    /// start of a 47 s smithy cycle (sound designer 2 #4).
    fn work_beats(&mut self, sim: &Sim, me: u8, l: MapCoord, g: Grid, half: f32) {
        let now = sim.gf().0;
        let from = std::mem::replace(&mut self.beat_gf, now);
        // (none at 10x and faster, like the sim's work cues)
        if now <= from || now - from > 200 || !work_admitted(self.speed, None, Instant::now()) {
            return;
        }
        let fog = self.fog_of_war;
        let beats: Vec<(MapCoord, &'static str, f32)> = sim
            .figures()
            .iter()
            .filter(|(_, f)| f.inside.is_some() && g.distance(f.pos, l) <= half as u32)
            .filter_map(|(id, f)| {
                let rr_sim::Motion::Work { start, .. } = f.motion else { return None };
                let b = work_rhythm(f.job)?;
                let seed = id.index() as u32;
                (from + 1..=now).any(|t| t > start && beat_at(&b, t - start, seed)).then_some((f.pos, b.key, b.volume))
            })
            .filter(|(p, _, _)| audible(sim.visibility(me, *p), fog))
            .collect();
        // at speed the beats come faster with the game: keep one in `speed`, and through the
        // work slots like the sim's cues (sound designer round 3, #4: 410 plays a minute at 3x)
        let wall = Instant::now();
        for (at, key, v) in beats {
            self.pick = self.pick.wrapping_mul(1103515245).wrapping_add(12345);
            let roll = ((self.pick >> 8) & 0xffff) as f32 / 65536.0;
            let (d, pan) = (self.volume_at(at), self.pan_at(at));
            // (and beats too faint to hear don't take a slot: a miner's picks at -48 LUFS did)
            if d * v < 0.1
                || !beat_kept(self.speed, roll)
                || !work_slot(&mut self.work_last, key, self.speed, d * v, wall)
            {
                continue;
            }
            self.play_later(key, d * v, pan, roll);
        }
    }

    /// Play a sound a little later (see `play_delay`); `update` plays it when it is due.
    fn play_later(&mut self, key: &'static str, volume: f32, pan: f32, roll: f32) {
        self.later.push((Instant::now() + play_delay(roll), key, volume, pan));
    }

    /// The sounds whose little delay is over.
    fn play_due(&mut self) {
        let now = Instant::now();
        let due: Vec<(Instant, &'static str, f32, f32)> = self.later.iter().copied().filter(|l| l.0 <= now).collect();
        self.later.retain(|l| l.0 > now);
        for (_, key, v, pan) in due {
            self.play_panned(key, v, pan);
        }
    }

    /// The gain of a channel slider, after the master slider, mute and focus.
    fn gain(&self, channel: f32) -> f32 {
        if self.muted || self.background && self.mute_in_background {
            return 0.0;
        }
        level_gain(self.master) * level_gain(channel)
    }
}

/// How a trade sounds through its work cycle: `burst` beats `step` frames apart, then a rest.
#[derive(Clone, Copy, Debug)]
pub struct Rhythm {
    pub key: &'static str,
    pub volume: f32,
    pub burst: u32,
    pub step: u32,
    pub rest: u32,
}

/// The rhythm of a trade working inside, if it has one (frames at 20 a second).
pub fn work_rhythm(job: rr_rules::Job) -> Option<Rhythm> {
    use rr_rules::Job;
    let r = |key, volume, burst, step, rest| Some(Rhythm { key, volume, burst, step, rest });
    match job {
        // a smith strikes in bursts of four, a breath between
        Job::Armorer | Job::Metalworker | Job::Minter => r("anvil", 0.5, 4, 18, 70),
        Job::IronFounder => r("anvil", 0.3, 2, 24, 120),
        Job::Carpenter => r("saw", 0.5, 2, 30, 50),
        Job::Miller => r("creak", 0.4, 1, 1, 90),
        // a pick underground: fewer, softer
        Job::Miner => r("pick", 0.3, 3, 24, 90),
        Job::Butcher => r("knife", 0.4, 2, 16, 120),
        _ => None,
    }
}

/// Whether a beat falls `t` frames into the work; `seed` keeps two smithies out of step.
pub fn beat_at(r: &Rhythm, t: u32, seed: u32) -> bool {
    let cycle = r.burst * r.step + r.rest;
    // (the first beat a moment in: the sim cues the start itself)
    let shifted = t + seed.wrapping_mul(2_654_435_761) % cycle;
    let (c, k) = (shifted / cycle, shifted % cycle);
    // each cycle a burst one longer or shorter and a step up to 15 % off: a smith, not a
    // metronome (sound designer round 3, #5)
    let h = (c ^ seed.rotate_left(7)).wrapping_mul(2_654_435_761) >> 16;
    let burst = (r.burst + h % 3).saturating_sub(1).max(1);
    let step = (r.step * (85 + (h >> 4) % 31) / 100).max(1);
    k >= step && k <= burst * step && k % step == 0
}

/// How long to wait for the next carrier's set-down: `carry_interval`, 35 % either way.
pub fn carry_wait(carriers: usize, roll: f32) -> Duration {
    carry_interval(carriers).mul_f32(0.65 + 0.7 * roll)
}

/// A little delay before a clash or a beat plays (`roll` from 0 to 1): up to 120 ms, so two
/// duels don't strike in the same frame and fights don't sit on the sim's 250 ms grid.
pub fn play_delay(roll: f32) -> Duration {
    Duration::from_millis((roll.clamp(0.0, 1.0) * 120.0) as u64)
}

/// How often a carrier in view is heard putting a load down: a lone one now and then, a busy
/// town every 1.5 s (1 000-2 000 carriers by minute 60 were silent: sound designer 2 #6).
pub fn carry_interval(carriers: usize) -> Duration {
    Duration::from_millis((6000 / (carriers.max(1) as u64)).clamp(1500, 5000))
}

/// The crackle of fires in view: one fire soft, a burning town full.
pub fn fire_volume(fires: usize) -> f32 {
    (0.35 + 0.1 * fires as f32).min(0.8)
}

/// Which call sounds: wild animals before donkeys three to one (late in the game the calls were
/// all one bray: sound designer 2 #6). `pick` is a random number.
fn pick_call<T: Copy>(wild: &[T], tame: &[T], pick: usize) -> Option<T> {
    let from_wild = !wild.is_empty() && (tame.is_empty() || pick % 4 != 0);
    let list = if from_wild { wild } else { tame };
    (!list.is_empty()).then(|| list[(pick / 4) % list.len()])
}

/// Zoomed out, the land is farther away: -3 dB per halving of the zoom (zoom 0.5 heard twice
/// the cues at the same level, +5-6 dB in all: sound designer 2 #9).
pub fn zoom_gain(zoom: f32) -> f32 {
    zoom.clamp(0.01, 1.0).sqrt()
}

/// Whether a work sound plays at this game speed: all of them up to 2x, one per sound every
/// 300 ms up to 5x, none faster (news, battle and the interface stay).
pub fn work_admitted(speed: f32, last: Option<Instant>, now: Instant) -> bool {
    if speed <= 2.0 {
        true
    } else if speed <= 5.0 {
        last.is_none_or(|t| now.duration_since(t) >= Duration::from_millis(300))
    } else {
        false
    }
}

/// Whether a work beat plays at this game speed (`roll` from 0 to 1): all up to 1.5x, then one
/// in `speed`, so a smithy's tempo stays that of a smith.
fn beat_kept(speed: f32, roll: f32) -> bool {
    speed <= 1.5 || roll < 1.0 / speed
}

/// Whether war tracks are due this far into a war: 15 minutes of them, then 10 of calm ones,
/// and so on (a siege of an hour drummed for an hour).
fn war_music_due(into_war: Duration) -> bool {
    into_war.as_secs() % (25 * 60) < 15 * 60
}

/// Whether the music should give way now: war just began and a calm track is playing.
fn war_onset(was: bool, now: bool, playing_war: bool) -> bool {
    !was && now && !playing_war
}

/// Below this volume a sound isn't played at all.
const HEARD: f32 = 0.02;

/// Whether a work sound heard at `volume` plays now at this speed, and if so its slot is taken:
/// one per sound every 300 ms at 3-5x. A sound out of hearing takes no slot.
fn work_slot(
    last: &mut Vec<(&'static str, Instant)>,
    key: &'static str,
    speed: f32,
    volume: f32,
    now: Instant,
) -> bool {
    if volume <= HEARD {
        return false;
    }
    let prev = last.iter().find(|(k, _)| *k == key).map(|(_, t)| *t);
    if !work_admitted(speed, prev, now) {
        return false;
    }
    last.retain(|(k, _)| *k != key);
    last.push((key, now));
    true
}

/// The sim's cues of the economy at work (thinned at speed), not battle or ships.
pub fn is_work(c: SoundCue) -> bool {
    !matches!(
        c,
        SoundCue::Sword
            | SoundCue::Death
            | SoundCue::Fire
            | SoundCue::Fanfare
            | SoundCue::Launch
            | SoundCue::Dock
            | SoundCue::Catapult
            | SoundCue::Thump
            | SoundCue::Promote
    )
}

/// Sounds that pass however busy the realm is: news, stingers and the interface.
const PRIORITY_KEYS: [&str; 11] =
    ["victory", "defeat", "built", "lost", "fanfare", "horn", "click", "refuse", "news", "news_good", "news_bad"];

/// One hammer per building site every 4 s: a site hammers every 2.85 s, and in the first half
/// hour that was 57-70 % of all heard (sound designer 2 #4).
fn site_due(sites: &mut Vec<(MapCoord, Instant)>, at: MapCoord, now: Instant) -> bool {
    sites.retain(|(_, t)| now.duration_since(*t) < Duration::from_secs(4));
    if sites.iter().any(|(p, _)| *p == at) {
        return false;
    }
    sites.push((at, now));
    true
}

/// Left and right gains for a position -1 (left) .. 1 (right), at equal power everywhere, so a
/// sound at the screen edge is as loud as one in the middle (both 0.71 there).
pub fn pan_gains(pan: f32) -> (f32, f32) {
    let theta = (pan.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
    (theta.cos(), theta.sin())
}

/// A slider position 0..1 as a gain: 0 is silent, the rest spans -40 dB to 0 dB, so the slider
/// moves evenly through what the ear hears.
pub fn level_gain(pos: f32) -> f32 {
    if pos <= 0.0 {
        return 0.0;
    }
    10f32.powf((pos.min(1.0) - 1.0) * 40.0 / 20.0)
}

/// The call of an animal, if it has one (sheep recorded, the others synthesised).
fn animal_call(a: rr_core::Animal, land: rr_core::Landscape) -> Option<&'static str> {
    use rr_core::Animal::*;
    match a {
        // no stag rut and no ducks quacking in the snow (sound designer 2 #5)
        Stag | Duck if land == rr_core::Landscape::Winter => None,
        Sheep => Some("sheep"),
        Duck => Some("duck"),
        Stag => Some("stag"),
        // a short bark: the rutting bellow is the stag's
        Deer => Some("deer"),
        Fox => Some("fox"),
        // a low growl (item 335)
        PolarBear => Some("bear"),
        Rabbit => None,
    }
}

/// The call of a working figure, if it has one: pack donkeys bray now and then.
fn figure_call(job: rr_rules::Job) -> Option<&'static str> {
    (job == rr_rules::Job::PackDonkey).then_some("bray")
}

/// What lies around the view centre: nodes, open water, shore, green land, trees.
#[derive(Default)]
struct Around {
    n: f32,
    water: f32,
    shore: f32,
    green: f32,
    trees: f32,
    /// Per bed (waves, stream, birds, wind): the sum of weight × pan, and of the weights.
    pan: [f32; 4],
    weight: [f32; 4],
}

#[derive(Clone, Copy)]
enum Bed {
    Waves,
    Stream,
    Birds,
    Wind,
}

impl Around {
    /// A node of a bed's land, heard with weight `w` (0 unexplored), lying at `pan`.
    fn add(&mut self, bed: Bed, w: f32, pan: f32) {
        match bed {
            Bed::Waves => self.water += w,
            Bed::Stream => self.shore += w,
            Bed::Birds => self.green += w,
            Bed::Wind => self.trees += w,
        }
        self.pan[bed as usize] += w * pan;
        self.weight[bed as usize] += w;
    }
}

/// Where each bed sounds from: the mean position of its land in view.
fn bed_pans(a: &Around) -> [f32; 4] {
    std::array::from_fn(|i| if a.weight[i] > 0.0 { (a.pan[i] / a.weight[i]).clamp(-0.8, 0.8) } else { 0.0 })
}

/// How much a node counts for the beds: in sight fully, remembered land half, unexplored
/// (or fogged land under fog of war) not at all.
fn hear_weight(v: rr_sim::Visibility, fog_of_war: bool) -> f32 {
    match v {
        _ if !audible(v, fog_of_war) => 0.0,
        rr_sim::Visibility::Visible => 1.0,
        _ => 0.5,
    }
}

/// How loud each ambience loop plays for what is around: waves, stream, birds, wind.
fn ambience_mix(a: &Around, land: rr_core::Landscape) -> [f32; 4] {
    let n = a.n.max(1.0);
    // the North: few birds and the wind over all open land; the wasteland: thinner birds, some
    // wind (sound designer 2 #5)
    let (birds, open_wind) = match land {
        rr_core::Landscape::Winter => (0.3, 0.6),
        rr_core::Landscape::Wasteland => (0.6, 0.3),
        rr_core::Landscape::Greenland => (1.0, 0.0),
    };
    let land_share = 1.0 - (a.water / n).min(1.0);
    [
        (a.water / n * 2.5).min(1.0),
        // a river or a lake shore without much open water: the stream
        ((a.shore / n * 4.0).min(1.0) * (1.0 - (a.water / n * 2.0).min(1.0))).max(0.0) * BED_CAP[1],
        (a.green / n * 1.5).min(1.0) * BED_CAP[2] * birds,
        (a.trees / n * 3.0).min(1.0).max(open_wind * land_share) * BED_CAP[3],
    ]
}

/// The most each bed plays at (of its file level): waves, stream, birds, wind.
const BED_CAP: [f32; 4] = [1.0, 0.95, 0.9, 0.85];
/// The levels `tools/loudness.py` gives the files (integrated LUFS).
const MUSIC_LUFS: f32 = -18.0;
const AMBIENT_LUFS: f32 = -24.0;
/// News sounds that make the music step back.
const JINGLE_KEYS: [&str; 7] = ["alarm", "built", "lost", "fanfare", "victory", "defeat", "horn"];
/// The end of the truce: a horn, not the alarm of an attack.
pub const TRUCE_END: &str = "horn";
/// News without a sound of its own: a soft chime for a nag ("nothing in reach").
pub const NEWS_CUE: &str = "news";
/// The level `tools/loudness.py` gives the interface cues (LUFS M-max).
const UI_LUFS: f32 = -22.0;
const CLICK_VOLUME: f32 = 0.5;
const REFUSE_VOLUME: f32 = 0.6;
const CHIME_VOLUME: f32 = 0.6;

/// The chime for news without a sound of its own: a find rises, a loss falls, a nag is soft.
pub fn chime_for(e: &Event, me: u8) -> Option<(&'static str, f32)> {
    match e {
        Event::GeologistFound { player, .. } if *player == me => Some(("news_good", CHIME_VOLUME)),
        Event::MineExhausted { player, .. } if *player == me => Some(("news_bad", CHIME_VOLUME)),
        Event::NothingInReach { player, .. } if *player == me => Some((NEWS_CUE, CHIME_VOLUME)),
        _ => None,
    }
}

/// The interface cues as heard at the default sliders, in LUFS: click, refusal, chime.
pub fn default_cue_levels() -> [f32; 3] {
    let db = |g: f32| 20.0 * g.log10();
    let at = |v: f32| UI_LUFS + db(level_gain(DEFAULT_SFX)) + db(v);
    [at(CLICK_VOLUME), at(REFUSE_VOLUME), at(CHIME_VOLUME)]
}

/// Music and the birds bed (the land's usual sound) as heard at the default sliders, in LUFS.
pub fn default_bus_levels() -> (f32, f32) {
    let db = |g: f32| 20.0 * g.log10();
    let music = MUSIC_LUFS + db(level_gain(DEFAULT_MUSIC));
    let beds = AMBIENT_LUFS + db(level_gain(DEFAULT_AMBIENCE)) + db(BED_CAP[2]);
    (music, beds)
}

/// The default slider positions (the options start here).
pub const DEFAULT_MUSIC: f32 = 0.75;
pub const DEFAULT_SFX: f32 = 0.9;
pub const DEFAULT_AMBIENCE: f32 = 0.7;

/// The music: a track, then 40-90 s of quiet (ambience only), then the next one, so hours of
/// play don't wear the few tracks out.
#[derive(Default)]
struct Playlist {
    n: usize,
    /// The track playing now; None during a pause.
    playing: Option<usize>,
    last: Option<usize>,
    quiet_until: Option<Instant>,
    /// The driving tracks, and whether the realm is at war (attacked lately): then the pause
    /// is 10-20 s and one of those comes next.
    war: Vec<usize>,
    tense: bool,
    /// When this war began (for the rests of a long war's music).
    war_since: Option<Instant>,
}

impl Playlist {
    /// The music ran out: the track to start now, if the pause is over.
    fn on_empty(&mut self, now: Instant, shuffle: bool, rnd: usize) -> Option<usize> {
        if self.n == 0 {
            return None;
        }
        let war = self.tense
            && !self.war.is_empty()
            && self.war_since.is_none_or(|s| war_music_due(now.saturating_duration_since(s)));
        if self.playing.take().is_some() {
            let pause = if war { 10 + rnd % 11 } else { 40 + rnd % 51 };
            self.quiet_until = Some(now + Duration::from_secs(pause as u64));
            return None;
        }
        if self.quiet_until.is_some_and(|t| now < t) {
            return None;
        }
        self.quiet_until = None;
        // (the rest of a long war: calm tracks only)
        let calm: Vec<usize> = (0..self.n).filter(|t| !self.war.contains(t) && Some(*t) != self.last).collect();
        let t = match self.last {
            Some(l) if war => {
                let others: Vec<usize> = self.war.iter().copied().filter(|w| *w != l).collect();
                let from = if others.is_empty() { &self.war } else { &others };
                from[rnd % from.len()]
            }
            Some(_) if self.tense && !calm.is_empty() => calm[rnd % calm.len()],
            // the session opens with any track (sound designer #7: always the same opener)
            None => rnd % self.n,
            // any other track than the one just played
            Some(l) if shuffle && self.n > 1 => (l + 1 + rnd % (self.n - 1)) % self.n,
            Some(l) => (l + 1) % self.n,
        };
        self.last = Some(t);
        self.playing = Some(t);
        Some(t)
    }

    /// Quiet for `d` from `now`, then the next track.
    fn hold(&mut self, now: Instant, d: Duration) {
        self.playing = None;
        self.quiet_until = Some(now + d);
    }

    /// The player wants the next track at once.
    fn skip(&mut self) {
        self.playing = None;
        self.quiet_until = None;
    }
}

/// How many sounds may start at once: at most four in 150 ms, so a battle doesn't roar. The
/// alarm skips that limit, as it must be heard, but sounds at most every few seconds.
#[derive(Default)]
struct Gate {
    recent: Vec<Instant>,
    alarm: Option<Instant>,
    news: Vec<(&'static str, Instant)>,
}

const ALARM_EVERY_MS: u128 = 4000;

/// The news jingles that must not repeat in a burst (a lost border takes many buildings along),
/// and how many ms apart they may sound.
const NEWS: [(&str, u128); 3] = [("lost", 3000), ("built", 1000), ("fanfare", 1000)];

/// The jingle for news of the player's own realm: (sound, volume).
/// Whether a sound at land of this visibility is heard (sound designer #8: one heard the
/// woodcutters of unexplored land): in sight always, explored land without fog of war, never
/// unexplored land.
pub fn audible(v: rr_sim::Visibility, fog_of_war: bool) -> bool {
    match v {
        rr_sim::Visibility::Visible => true,
        rr_sim::Visibility::Fog => !fog_of_war,
        rr_sim::Visibility::Unexplored => false,
    }
}

/// How far round the view centre the beds listen: the whole view (a little beyond its edge).
pub fn bed_radius(half_width: f32) -> u16 {
    (half_width * 1.2).clamp(6.0, 30.0) as u16
}

/// A small xorshift step (sound variation needs no quality, just no pattern).
fn rand_next(r: &mut u32) -> u32 {
    *r ^= *r << 13;
    *r ^= *r >> 17;
    *r ^= *r << 5;
    *r
}

/// A play's pitch (0.92-1.08, tape-style) and gain change (within 2 dB), so a sound heard
/// again and again doesn't sound the same each time (sound designer #4).
pub fn vary(r: &mut u32) -> (f32, f32) {
    let u = |r: &mut u32| (rand_next(r) % 10_001) as f32 / 10_000.0;
    (0.92 + 0.16 * u(r), -2.0 + 4.0 * u(r))
}

/// The variant to play of `n`: never the one played last time (when there is a choice).
pub fn next_variant(n: usize, last: Option<usize>, r: &mut u32) -> usize {
    if n <= 1 {
        return 0;
    }
    match last {
        Some(l) if l < n => (l + 1 + rand_next(r) as usize % (n - 1)) % n,
        _ => rand_next(r) as usize % n,
    }
}

/// A source mixed down to one channel by averaging (sound designer #1: rodio's ChannelVolume
/// adds a stereo file's channels up, so stereo effects played 3 dB hotter than mono ones).
pub struct MonoMix<S: Source> {
    inner: S,
}

impl<S: Source> MonoMix<S> {
    pub fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S: Source> Iterator for MonoMix<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let n = self.inner.channels().max(1);
        let mut sum = 0.0;
        for k in 0..n {
            match self.inner.next() {
                Some(s) => sum += s,
                None if k == 0 => return None,
                None => break,
            }
        }
        Some(soft_clip(sum / n as f32))
    }
}

/// Untouched up to 0.8, then rounded off toward 1: an effect pushed over full scale by the
/// sliders and the pan bends instead of cracking (sound designer 2 #11: no limiter at all).
pub fn soft_clip(x: f32) -> f32 {
    const KNEE: f32 = 0.8;
    let a = x.abs();
    if a <= KNEE { x } else { x.signum() * (KNEE + (1.0 - KNEE) * ((a - KNEE) / (1.0 - KNEE)).tanh()) }
}

impl<S: Source> Source for MonoMix<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len().map(|l| l / self.inner.channels().max(1) as usize)
    }

    fn channels(&self) -> rodio::ChannelCount {
        1
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        self.inner.total_duration()
    }
}

/// A mono source played in stereo from a position that may move while it plays (the beds),
/// at equal power, as the effects are (a mono file plays at its own power).
pub struct Panned<S: Source> {
    inner: S,
    pan: Arc<AtomicU32>,
    right: Option<f32>,
}

impl<S: Source> Panned<S> {
    fn new(inner: S, pan: Arc<AtomicU32>) -> Self {
        Self { inner, pan, right: None }
    }
}

impl<S: Source> Iterator for Panned<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if let Some(r) = self.right.take() {
            return Some(r);
        }
        let s = self.inner.next()?;
        // (equal power, as the effects: the √2 made every bed 3 dB hotter than the level
        // constants assume; sound designer round 3, #2)
        let (l, r) = pan_gains(f32::from_bits(self.pan.load(Ordering::Relaxed)));
        self.right = Some(s * r);
        Some(s * l)
    }
}

impl<S: Source> Source for Panned<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len().map(|l| l * 2)
    }

    fn channels(&self) -> rodio::ChannelCount {
        2
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        self.inner.total_duration()
    }
}

/// Whether a sound makes the music step back: a jingle that is heard, not one at the edge of
/// hearing (sound designer 2 #1).
fn ducks_music(key: &str, volume: f32) -> bool {
    // (not for one's own quiet "built" chime, under the music already: the music ducked 54-102
    // times an hour; sound designer round 3, #7)
    JINGLE_KEYS.contains(&key) && key != "built" && volume > 0.3
}

/// Somebody else's building finished in view: a quiet "done" where it stands, no jingle.
pub fn finished_elsewhere(e: &Event, me: u8) -> Option<(&'static str, f32)> {
    match e {
        Event::BuildingFinished { player, .. } if *player != me => Some(("build", 0.4)),
        _ => None,
    }
}

pub fn news_sound(e: &Event, me: u8) -> Option<(&'static str, f32)> {
    Some(match e {
        Event::BuildingFinished { player, kind, .. } if *player == me => {
            use rr_rules::BuildingType as B;
            if matches!(kind, B::Barracks | B::Guardhouse | B::Watchtower | B::Fortress | B::Harbor) {
                ("fanfare", 0.6)
            } else {
                ("built", 0.4)
            }
        }
        Event::UnderAttack { player, .. } if *player == me => ("alarm", 0.8),
        Event::BuildingCaptured { by, .. } if *by == me => ("fanfare", 0.8),
        Event::BuildingCaptured { player, .. } | Event::BuildingLost { player, .. } if *player == me => ("lost", 0.7),
        Event::PlayerDefeated { player } if *player == me => ("defeat", 0.9),
        // a dull "uh-uh" for a refused command, and a click only for what was done
        // (UI at -22 LUFS: the click 4 LU over the birds and 4 under the music, sound
        // designer 2 #2)
        Event::CommandRejected { player, .. } if *player == me => ("refuse", REFUSE_VOLUME),
        Event::Placed { player } if *player == me => ("click", CLICK_VOLUME),
        // tearing down your own building: timber falling, no lament (sound designer #5)
        Event::BuildingRemoved { player, .. } if *player == me => ("collapse", 0.5),
        // (the stone lands as a positional thump for all in earshot: sound designer 2 #6)
        _ => return None,
    })
}

impl Gate {
    fn admit(&mut self, key: &str, now: Instant) -> bool {
        self.recent.retain(|t| now.duration_since(*t).as_millis() < 150);
        if let Some(&(name, gap)) = NEWS.iter().find(|(k, _)| *k == key) {
            if self.news.iter().any(|(k, t)| *k == name && now.duration_since(*t).as_millis() < gap) {
                return false;
            }
            self.news.retain(|(k, _)| *k != name);
            self.news.push((name, now));
        }
        if key == "alarm" {
            if self.alarm.is_some_and(|t| now.duration_since(t).as_millis() < ALARM_EVERY_MS) {
                return false;
            }
            self.alarm = Some(now);
        } else if self.recent.len() >= 4 && !PRIORITY_KEYS.contains(&key) {
            return false;
        }
        self.recent.push(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sound designer #5 (rest): fire crackles, pack donkeys bray.
    #[test]
    fn fire_and_donkeys_are_heard() {
        assert_eq!(cue_key(rr_sim::SoundCue::Fire), "fire");
        assert!(SFX.iter().any(|(k, v)| *k == "fire" && !v.is_empty()));
        assert!(SFX.iter().any(|(k, v)| *k == "bray" && !v.is_empty()));
        assert_eq!(figure_call(rr_rules::Job::PackDonkey), Some("bray"));
    }

    /// Sound designer #8: no sounds from land nobody of ours has seen (or, with fog of war, can
    /// see now), and the beds listen over the whole zoomed view.
    #[test]
    fn heard_only_where_seen_and_over_the_view() {
        use rr_sim::Visibility::*;
        assert!(audible(Visible, true) && audible(Visible, false));
        assert!(!audible(Fog, true) && audible(Fog, false));
        assert!(!audible(Unexplored, true) && !audible(Unexplored, false));
        assert!(bed_radius(20.0) >= 20, "zoomed out the beds hear the screen edge");
        assert!(bed_radius(4.0) >= 6);
    }

    /// Sound designer #7: the session doesn't always open with the same track.
    #[test]
    fn the_first_track_varies() {
        let firsts: std::collections::BTreeSet<usize> = (0..20)
            .map(|rnd| {
                let mut p = Playlist { n: 6, ..Default::default() };
                p.on_empty(Instant::now(), false, rnd).unwrap()
            })
            .collect();
        assert!(firsts.len() > 1, "always track {firsts:?}");
    }

    /// Sound designer #5: one cue per meaning. A refusal is not the mill, a placement not the
    /// news, the truce end not an attack, tearing down your own site not a loss.
    #[test]
    fn one_cue_per_meaning() {
        use rr_rules::BuildingType as B;
        let at = MapCoord::new(1, 1);
        let key = |e: Event| news_sound(&e, 0).map(|s| s.0);
        let refused = key(Event::CommandRejected {
            player: 0,
            cmd: rr_sim::Cmd::PlaceFlag { at },
            reason: rr_sim::RejectReason::NotBuildable,
        });
        let placed = key(Event::Placed { player: 0 });
        let removed = key(Event::BuildingRemoved { player: 0, at, kind: B::Farm });
        let lost = key(Event::BuildingLost { player: 0, at, kind: B::Farm });
        let attacked = key(Event::UnderAttack { player: 0, at, by: 1, soldiers: 5 });
        let all = [refused, placed, removed, lost, attacked];
        for (i, a) in all.iter().enumerate() {
            assert!(a.is_some(), "cue {i} is silent");
            for b in &all[i + 1..] {
                assert_ne!(a, b, "two meanings share {a:?}");
            }
        }
        let work: Vec<&str> = rr_rules::Job::ALL
            .iter()
            .flat_map(|j| [rr_sim::work_cue(*j, false), rr_sim::work_cue(*j, true)])
            .flatten()
            .map(cue_key)
            .collect();
        assert!(!work.contains(&refused.unwrap()), "the refusal sounds like work");
        assert_ne!(TRUCE_END, "alarm");
        assert_ne!(NEWS_CUE, "click");
    }

    /// Sound designer #4: every play of an effect differs a little (pitch 0.92-1.08, gain
    /// within 2 dB), and a sound with variants never repeats the one just played.
    #[test]
    fn plays_vary() {
        let mut r = 12345u32;
        let mut pitches = Vec::new();
        for _ in 0..200 {
            let (p, g) = vary(&mut r);
            assert!((0.92..=1.08).contains(&p) && (-2.0..=2.0).contains(&g), "{p} {g}");
            pitches.push(p);
        }
        let spread =
            pitches.iter().cloned().fold(f32::MIN, f32::max) - pitches.iter().cloned().fold(f32::MAX, f32::min);
        assert!(spread > 0.1, "pitch barely varies: {spread}");
        let mut last = None;
        for _ in 0..100 {
            let k = next_variant(3, last, &mut r);
            assert!(k < 3 && Some(k) != last);
            last = Some(k);
        }
        assert_eq!(next_variant(1, Some(0), &mut r), 0);
        assert_eq!(cue_key(rr_sim::SoundCue::Anvil), "anvil");
        assert_eq!(rr_sim::work_cue(rr_rules::Job::Armorer, false), Some(rr_sim::SoundCue::Anvil));
    }

    /// Sound designer #2: at default sliders the ambience beds sit no more than 10 LU under the
    /// music (they were 12-16 under: the land was silent while a track played).
    #[test]
    fn beds_are_heard_under_the_music() {
        let (music, beds) = default_bus_levels();
        assert!(music - beds <= 10.0, "music {music:.1} LUFS, beds {beds:.1} LUFS");
        assert!(music - beds >= 4.0, "beds must stay under the music");
    }

    /// Sound designer #1: a stereo effect plays as loud as the same sound in mono (rodio's
    /// ChannelVolume adds the channels up instead of averaging them).
    #[test]
    fn stereo_effects_are_mixed_down_not_summed() {
        let tone: Vec<f32> = (0..2000).map(|i| (i as f32 * 0.05).sin() * 0.5).collect();
        let stereo: Vec<f32> = tone.iter().flat_map(|s| [*s, *s]).collect();
        let mono_src = rodio::buffer::SamplesBuffer::new(1, 44_100, tone.clone());
        let stereo_src = rodio::buffer::SamplesBuffer::new(2, 44_100, stereo);
        let a: Vec<f32> = MonoMix::new(mono_src).collect();
        let b: Vec<f32> = MonoMix::new(stereo_src).collect();
        assert_eq!(a.len(), b.len());
        let rms = |v: &[f32]| (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt();
        assert!((rms(&a) / rms(&b) - 1.0).abs() < 0.01, "{} vs {}", rms(&a), rms(&b));
        assert_eq!(MonoMix::new(rodio::buffer::SamplesBuffer::new(2, 44_100, vec![0.0f32; 8])).channels(), 1);
    }
    use std::time::Duration;

    #[test]
    fn the_alarm_gets_through_a_busy_battle_but_not_every_frame() {
        let mut g = Gate::default();
        let t = Instant::now();
        for _ in 0..4 {
            assert!(g.admit("sword", t));
        }
        assert!(!g.admit("sword", t));
        assert!(g.admit("alarm", t));
        assert!(!g.admit("alarm", t + Duration::from_secs(1)));
        assert!(g.admit("alarm", t + Duration::from_secs(5)));
        assert!(g.admit("sword", t + Duration::from_secs(5)));
    }

    /// Every cue has a sound, and every sound file is there.
    #[test]
    fn every_cue_has_a_sound_file() {
        use SoundCue::*;
        for c in [
            TreeFall, Chop, Hammer, Pick, Saw, Sword, Death, Build, Fanfare, Splash, Shot, Scythe, Dig, Creak, Door,
            Knife, Bucket, Anvil, Fire, Launch, Dock, Catapult, Thump, Brew, Pig, Bray, Promote,
        ] {
            assert!(SFX.iter().any(|(k, _)| *k == cue_key(c)), "{c:?}");
        }
        let dir = assets_dir().join("audio").join("sfx");
        for (_, files) in SFX {
            for f in files {
                assert!(dir.join(format!("{f}.ogg")).is_file(), "{f}");
            }
        }
    }

    /// Sound designer 2 #1: your own building is one jingle; another's is a quiet "done" that
    /// doesn't duck the music; a jingle at the edge of hearing doesn't either.
    #[test]
    fn a_finished_building_is_one_sound() {
        use rr_rules::BuildingType as B;
        let at = MapCoord::new(3, 3);
        let mine = Event::BuildingFinished { player: 0, at, kind: B::Woodcutter };
        let theirs = Event::BuildingFinished { player: 1, at, kind: B::Woodcutter };
        assert_eq!(news_sound(&mine, 0), Some(("built", 0.4)));
        assert_eq!(finished_elsewhere(&mine, 0), None);
        assert_eq!(news_sound(&theirs, 0), None);
        let (k, v) = finished_elsewhere(&theirs, 0).unwrap();
        assert!(!ducks_music(k, v));
        assert!(ducks_music("fanfare", 0.6));
        assert!(!ducks_music("fanfare", 0.1));
        // sound designer round 3, #7: not for the quiet "built" chime (41 ducks an hour)
        assert!(!ducks_music("built", 0.4));
        assert!(ducks_music("alarm", 0.8));
    }

    /// Sound designer 2 #2: the click was 4 LU under the birds; now every UI cue is over the
    /// beds and under the music.
    #[test]
    fn interface_cues_are_heard() {
        let (music, beds) = default_bus_levels();
        for (i, l) in default_cue_levels().iter().enumerate() {
            assert!(*l >= beds + 4.0 && *l <= music - 2.0, "cue {i} at {l} (music {music}, beds {beds})");
        }
        let at = MapCoord::new(1, 1);
        let find = Event::GeologistFound { player: 0, at, what: rr_core::Resource::Gold(3) };
        let gone = Event::MineExhausted { player: 0, at };
        let (a, b) = (chime_for(&find, 0).unwrap().0, chime_for(&gone, 0).unwrap().0);
        assert!(a != b && a != NEWS_CUE && b != NEWS_CUE);
        assert_eq!(chime_for(&find, 1), None);
    }

    /// Sound designer 2 #4: a 47 s smithy cycle had one strike; two smithies don't strike in
    /// step; a site hammers at most every 4 s.
    #[test]
    fn work_sounds_through_the_cycle() {
        let r = work_rhythm(rr_rules::Job::Armorer).unwrap();
        let beats = |seed| (1..=940).filter(|t| beat_at(&r, *t, seed)).collect::<Vec<_>>();
        assert!(beats(1).len() >= 10, "{:?}", beats(1));
        assert_ne!(beats(1), beats(2));
        assert!(work_rhythm(rr_rules::Job::Woodcutter).is_none());
        let (mut sites, t, at) = (Vec::new(), Instant::now(), MapCoord::new(5, 5));
        assert!(site_due(&mut sites, at, t));
        assert!(!site_due(&mut sites, at, t + Duration::from_secs(3)));
        assert!(site_due(&mut sites, MapCoord::new(6, 5), t + Duration::from_secs(3)));
        assert!(site_due(&mut sites, at, t + Duration::from_secs(5)));
    }

    /// Sound designer 2 #6: wild animals before donkeys, fires louder in a burning town.
    #[test]
    fn calls_and_fires() {
        let wild_share = (0..400).filter(|p| pick_call(&['w'], &['d'], *p) == Some('w')).count();
        assert!((280..=320).contains(&wild_share), "{wild_share}");
        assert_eq!(pick_call(&[], &['d'], 1), Some('d'));
        assert_eq!(pick_call::<char>(&[], &[], 1), None);
        assert!(fire_volume(1) < fire_volume(6) && fire_volume(50) <= 0.8);
        assert!(carry_interval(1) > carry_interval(3));
        assert_eq!(carry_interval(0), Duration::from_millis(5000));
        assert_eq!(carry_interval(500), Duration::from_millis(1500));
    }

    /// Sound designer 2 #9: zooming out made the realm louder, fast-forward a machine gun, and
    /// the stingers shared the work sounds' gate.
    #[test]
    fn zoom_speed_and_priority() {
        assert!((zoom_gain(0.5) - 0.707).abs() < 0.01);
        assert_eq!(zoom_gain(2.0), 1.0);
        let t = Instant::now();
        assert!(work_admitted(1.0, Some(t), t));
        assert!(!work_admitted(3.0, Some(t), t + Duration::from_millis(100)));
        assert!(work_admitted(3.0, Some(t), t + Duration::from_millis(400)));
        assert!(!work_admitted(10.0, None, t));
        assert!(is_work(SoundCue::Hammer) && !is_work(SoundCue::Sword) && !is_work(SoundCue::Death));
        let mut g = Gate::default();
        for _ in 0..4 {
            assert!(g.admit("sword", t));
        }
        assert!(!g.admit("chop", t));
        assert!(g.admit("victory", t), "the stinger waits for no battle");
        assert!(g.admit("click", t));
    }

    #[test]
    fn effects_bend_instead_of_cracking() {
        assert_eq!(soft_clip(0.5), 0.5);
        assert_eq!(soft_clip(-0.8), -0.8);
        assert!(soft_clip(1.5) < 1.0 && soft_clip(1.5) > 0.95);
        assert!(soft_clip(-3.0) >= -1.0);
        assert!(soft_clip(0.9) > soft_clip(0.85));
    }

    /// QA round 6, #3: at 3-5x a cue out of hearing took the 300 ms slot of its sound, so the
    /// same sound in the middle of the view was dropped.
    #[test]
    fn only_heard_work_takes_the_slot() {
        let (mut last, t) = (Vec::new(), Instant::now());
        assert!(!work_slot(&mut last, "chop", 4.0, 0.0, t), "out of hearing");
        assert!(work_slot(&mut last, "chop", 4.0, 0.8, t + Duration::from_millis(10)), "in view");
        assert!(!work_slot(&mut last, "chop", 4.0, 0.8, t + Duration::from_millis(100)));
        assert!(work_slot(&mut last, "pick", 4.0, 0.8, t + Duration::from_millis(100)));
        assert!(work_slot(&mut last, "chop", 1.0, 0.8, t + Duration::from_millis(101)));
    }

    /// Sound designer round 3, #4: at 3x one beat in three plays.
    #[test]
    fn beats_thin_with_speed() {
        let kept = |speed: f32| (0..3000).filter(|i| beat_kept(speed, *i as f32 / 3000.0)).count();
        assert_eq!(kept(1.0), 3000);
        assert!((950..=1050).contains(&kept(3.0)), "{}", kept(3.0));
    }

    /// Sound designer round 3, #5: no metronomes. Carriers' intervals vary, a smithy's
    /// strikes vary from cycle to cycle, and a play may wait up to 120 ms.
    #[test]
    fn nothing_runs_on_a_metronome() {
        let waits: Vec<f64> = (0..200).map(|i| carry_wait(10, i as f32 / 200.0).as_secs_f64()).collect();
        let mean = waits.iter().sum::<f64>() / waits.len() as f64;
        let sd = (waits.iter().map(|w| (w - mean).powi(2)).sum::<f64>() / waits.len() as f64).sqrt();
        assert!(sd / mean >= 0.2, "carriers' intervals vary by {:.2}", sd / mean);
        let r = work_rhythm(rr_rules::Job::Armorer).unwrap();
        let beats: Vec<u32> = (1..4000).filter(|t| beat_at(&r, *t, 7)).collect();
        let mut gaps: Vec<u32> = beats.windows(2).map(|w| w[1] - w[0]).collect();
        gaps.sort_unstable();
        gaps.dedup();
        assert!(gaps.len() > 3, "the smithy's gaps: {gaps:?}");
        assert!(play_delay(0.0) == Duration::ZERO && play_delay(0.999) <= Duration::from_millis(120));
    }

    /// Sound designer round 3, #8: at war more than two tracks take turns, and a war that
    /// starts during a calm track doesn't wait for its end.
    #[test]
    fn war_music_varies_and_comes_at_once() {
        let mut l = Playlist { n: 7, last: Some(0), war: vec![2, 4, 6], tense: true, ..Default::default() };
        let t = Instant::now();
        let mut seen = std::collections::BTreeSet::new();
        for i in 0..10u64 {
            l.playing = Some(0);
            let _ = l.on_empty(t, true, i as usize * 7);
            if let Some(p) = l.on_empty(t + Duration::from_secs(30), true, (i * i * 7919 + i * 31 + 3) as usize) {
                seen.insert(p);
            }
        }
        assert!(seen.len() >= 3, "{seen:?}");
        assert!(war_onset(false, true, false), "a calm track gives way");
        assert!(!war_onset(true, true, false) && !war_onset(false, true, true) && !war_onset(false, false, false));
    }

    /// Sound designer round 3, gap: a long war doesn't drum for an hour. War tracks play for
    /// 15 minutes, then calm ones for 10, then war again while the war lasts.
    #[test]
    fn war_music_rests_in_a_long_war() {
        assert!(war_music_due(Duration::from_secs(14 * 60)));
        assert!(!war_music_due(Duration::from_secs(16 * 60)));
        assert!(war_music_due(Duration::from_secs(26 * 60)));
        let t = Instant::now();
        let mut l =
            Playlist { n: 7, last: Some(0), war: vec![2, 4, 6], tense: true, war_since: Some(t), ..Default::default() };
        let late = t + Duration::from_secs(20 * 60);
        l.playing = Some(2);
        let _ = l.on_empty(late, true, 3);
        let next = l.on_empty(late + Duration::from_secs(100), true, 3).expect("a track after the pause");
        assert!(!l.war.contains(&next), "a calm track in the rest: {next}");
    }

    /// Sound designer round 3, #9: a battle at full sliders stays under full scale: the effects
    /// meet in one bus whose sum is clipped softly (the per-file clip never bent anything).
    #[test]
    fn the_effects_bus_never_overs() {
        let dir = assets_dir().join("audio").join("sfx");
        let names = [
            "death0", "sword0", "sword1", "sword2", "thump", "alarm", "death1", "death2", "catapult", "fire", "horn",
            "collapse",
        ];
        let (mixer, out) = effects_bus(44_100);
        let (raw, raw_out) = rodio::mixer::mixer(2, 44_100);
        for f in names {
            let c = clip(load(&dir.join(format!("{f}.ogg"))).unwrap()).unwrap();
            mixer.add(effect_chain(c.clone(), 1.0, 0.0, 1.0));
            raw.add(effect_chain(c, 1.0, 0.0, 1.0));
        }
        let raw_peak = raw_out.take(44_100 * 2 * 2).fold(0f32, |a, s| a.max(s.abs()));
        assert!(raw_peak > 1.0, "the plain sum would not over: {raw_peak}");
        let peak = out.take(44_100 * 2 * 2).fold(0f32, |a, s| a.max(s.abs()));
        assert!(peak <= 1.0, "peak {peak}");
        assert!(peak > 0.5, "the bus is silent: {peak}");
    }

    #[test]
    fn panning_keeps_the_power() {
        for pan in [-1.0, -0.8, -0.3, 0.0, 0.5, 1.0] {
            let (l, r) = pan_gains(pan);
            assert!((l * l + r * r - 1.0).abs() < 1e-5, "{pan}");
        }
        assert!(pan_gains(-0.8).0 > pan_gains(-0.8).1);
    }

    #[test]
    fn sliders_are_in_decibels() {
        assert_eq!(level_gain(0.0), 0.0);
        assert!((level_gain(1.0) - 1.0).abs() < 1e-6);
        // half way is -20 dB, a quarter of full scale is -10 dB at 0.75
        assert!((level_gain(0.5) - 0.1).abs() < 1e-4);
        assert!((level_gain(0.75) - 0.316).abs() < 1e-3);
    }

    /// The owner: wind in the forests, rivers, birds.
    #[test]
    fn animals_have_calls() {
        use rr_core::Animal::*;
        use rr_core::Landscape::*;
        // every voiced animal of every landscape is a loaded sound (the bears were mute:
        // sound designer 2 #5)
        for land in [Greenland, Wasteland, Winter] {
            for a in [Sheep, Duck, Stag, Deer, Fox, PolarBear, Rabbit] {
                if let Some(k) = animal_call(a, land) {
                    assert!(SFX.iter().any(|(s, _)| *s == k), "{a:?}");
                }
            }
        }
        assert_eq!(animal_call(Rabbit, Greenland), None);
        assert_eq!(animal_call(PolarBear, Winter), Some("bear"));
        // no stag rut or quacking in the snow
        assert_eq!(animal_call(Stag, Winter), None);
        assert_eq!(animal_call(Duck, Winter), None);
    }

    #[test]
    fn ambience_follows_the_land() {
        let forest = Around { n: 100.0, green: 80.0, trees: 40.0, ..Default::default() };
        let sea = Around { n: 100.0, water: 70.0, shore: 10.0, ..Default::default() };
        let river = Around { n: 100.0, shore: 20.0, green: 50.0, ..Default::default() };
        let g = rr_core::Landscape::Greenland;
        let (f, s, r) = (ambience_mix(&forest, g), ambience_mix(&sea, g), ambience_mix(&river, g));
        assert!(f[3] > 0.5 && f[0] == 0.0, "{f:?}");
        assert!(s[0] == 1.0 && s[1] == 0.0 && s[3] == 0.0, "{s:?}");
        assert!(r[1] > 0.5 && r[0] < 0.1, "{r:?}");
    }

    /// Sound designer 2 #5: the North sounded like a summer meadow.
    #[test]
    fn winter_sounds_of_wind_not_birds() {
        let tundra = Around { n: 100.0, green: 80.0, trees: 5.0, ..Default::default() };
        let summer = ambience_mix(&tundra, rr_core::Landscape::Greenland);
        let winter = ambience_mix(&tundra, rr_core::Landscape::Winter);
        assert!(winter[2] < 0.4 && winter[3] > 0.4, "{winter:?}");
        assert!(summer[2] > 0.7, "{summer:?}");
    }

    /// Sound designer 2 #7, #8: rodio gave no length for the Ogg files, so the fair loop was cut
    /// a beat short and every bed started at 0 s.
    #[test]
    fn files_know_their_length() {
        let data = load(&assets_dir().join("audio").join("music").join("fair.ogg")).unwrap();
        let len = decode(data).unwrap().total_duration().expect("a length");
        assert!((len.as_secs_f32() - 15.48).abs() < 0.05, "{len:?}");
        let wind = load(&assets_dir().join("audio").join("ambient").join("wind.ogg")).unwrap();
        assert!(decode(wind).unwrap().total_duration().is_some_and(|d| d > Duration::from_secs(20)));
        let bed = Duration::from_secs(40);
        assert_ne!(bed_start(bed, 1), bed_start(bed, 2));
        assert!(bed_start(bed, 12345) < bed);
    }

    /// Sound designer 2 #7: the loop faded over its whole minute; the duck was a hard step.
    #[test]
    fn the_loop_fades_only_at_its_end_and_the_duck_ramps() {
        let total = Duration::from_secs(60);
        let g = |s: f32| loop_gain(Duration::from_secs_f32(s), total);
        assert_eq!(g(1.0), 1.0);
        assert_eq!(g(40.0), 1.0);
        assert!(g(58.0) < 0.6 && g(58.0) > 0.0);
        assert_eq!(g(60.0), 0.0);
        // down in about 50 ms, back up over about 800 ms
        assert!(ease_duck(1.0, 0.5, 0.15) < 0.55);
        let up = ease_duck(0.5, 1.0, 0.15);
        assert!(up > 0.5 && up < 0.6, "{up}");
    }

    /// Sound designer 2 #7: a siege sounded like a harvest festival. At war the pause is short
    /// and the driving tracks come next.
    #[test]
    fn war_picks_the_driving_tracks_soon() {
        let t = Instant::now();
        for rnd in 0..20 {
            let mut l = Playlist { n: 5, last: Some(0), war: vec![2, 4], tense: true, ..Default::default() };
            l.playing = Some(0);
            assert_eq!(l.on_empty(t, true, rnd), None);
            let after = l.quiet_until.unwrap().duration_since(t);
            assert!(after <= Duration::from_secs(20), "{after:?}");
            let next = l.on_empty(t + Duration::from_secs(21), true, rnd).unwrap();
            assert!(next == 2 || next == 4, "{next}");
        }
    }

    /// Sound designer 2 #7: after victory or defeat the cheerful playlist went straight on.
    #[test]
    fn the_music_holds_after_a_stinger() {
        let mut l = Playlist { n: 3, ..Default::default() };
        let t = Instant::now();
        l.hold(t, Duration::from_secs(20));
        assert_eq!(l.on_empty(t + Duration::from_secs(10), false, 0), None);
        assert!(l.on_empty(t + Duration::from_secs(21), false, 0).is_some());
    }

    /// Sound designer 2 #10: unexplored sea played waves, and a coast on the left sounded in both
    /// ears alike.
    #[test]
    fn beds_hear_known_land_where_it_lies() {
        let g = rr_core::Landscape::Greenland;
        let mut dark = Around::default();
        for _ in 0..100 {
            dark.add(Bed::Waves, 0.0, -0.6);
        }
        assert_eq!(ambience_mix(&dark, g)[0], 0.0);
        let mut coast = Around::default();
        for i in 0..100 {
            if i < 40 {
                coast.add(Bed::Waves, 1.0, -0.6);
            } else {
                coast.add(Bed::Birds, 1.0, 0.3);
            }
        }
        let pans = bed_pans(&coast);
        assert!(pans[0] < -0.3 && pans[2] > 0.1, "{pans:?}");
        assert!(ambience_mix(&coast, g)[0] > 0.5);
        assert_eq!(hear_weight(rr_sim::Visibility::Unexplored, false), 0.0);
        assert_eq!(hear_weight(rr_sim::Visibility::Fog, false), 0.5);
        assert_eq!(hear_weight(rr_sim::Visibility::Visible, true), 1.0);
    }

    /// QA round 6, #1: the beds as built in `Audio::new` gave no sound at all (rodio's
    /// `Buffered` ends at once on a fresh decoder), and every session started them at 0 s.
    #[test]
    fn beds_are_not_silent() {
        let dir = assets_dir().join("audio").join("ambient");
        for name in ["birds", "waves", "stream", "wind"] {
            let data = load(&dir.join(format!("{name}.ogg"))).expect("bed file");
            let len = decode(data.clone()).unwrap().total_duration().unwrap();
            let src = |start| MonoMix::new(bed_source(decode(data.clone()).unwrap(), start));
            let n = src(bed_start(len, 12345)).take(44_100).count();
            assert_eq!(n, 44_100, "{name}: the bed gives {n} samples");
            let (a, b): (Vec<f32>, Vec<f32>) =
                (src(Duration::ZERO).take(2000).collect(), src(Duration::from_secs(10)).take(2000).collect());
            assert_ne!(a, b, "{name}: the start offset does nothing");
        }
    }

    /// QA round 6, #1: the fair loop as built in `update` gave no sound at all.
    #[test]
    fn fair_loop_is_not_silent() {
        let src = decode(load(&assets_dir().join("audio/music/fair.ogg")).unwrap()).unwrap();
        let n = fair_loop(src, Duration::from_secs(60)).count();
        assert!(n > 44_100 * 2 * 58, "the fair loop gives {n} samples, not a minute");
        // sound designer round 3, #3: whole frames (an odd count swapped left and right for
        // every later track) and whole 8-bar bodies of 14.768 s (the seam stumbled 63 ms)
        assert_eq!(n % 2, 0, "{n} samples: half a frame");
        let frames = n / 2;
        let body = (FAIR_BODY.as_secs_f64() * 44_100.0).round() as usize;
        assert_eq!(frames % body, 0, "{frames} frames aren't whole bodies of {body}");
    }

    /// Sound designer round 3, #2: a bed in the middle plays at its file's power, as an effect
    /// does (the √2 made every bed 3 dB hotter than all the level constants assume).
    #[test]
    fn beds_play_at_their_file_level() {
        let data = load(&assets_dir().join("audio").join("ambient").join("wind.ogg")).unwrap();
        let mono: Vec<f32> =
            MonoMix::new(bed_source(decode(data.clone()).unwrap(), Duration::ZERO)).take(44_100 * 5).collect();
        let pan = Arc::new(AtomicU32::new(0f32.to_bits()));
        let stereo: Vec<f32> = Panned::new(MonoMix::new(bed_source(decode(data).unwrap(), Duration::ZERO)), pan)
            .take(44_100 * 10)
            .collect();
        let p = |v: &[f32]| v.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>();
        let ratio = p(&stereo) / p(&mono);
        assert!((ratio - 1.0).abs() < 0.05, "stereo power {ratio:.2}× the file's");
    }

    /// Sound designer round 3, #1: every effect plays through a real mixer (a fresh decoder
    /// told the mixer its first span was empty, and no effect ever made a sound).
    #[test]
    fn effects_play_through_a_mixer() {
        let dir = assets_dir().join("audio").join("sfx");
        for (key, files) in SFX {
            for f in files {
                let data = load(&dir.join(format!("{f}.ogg"))).expect("file");
                let (mixer, out) = rodio::mixer::mixer(2, 44_100);
                mixer.add(effect_chain(clip(data).expect("decodes"), 1.0, 0.0, 0.7));
                let n = out.take(44_100 * 2 * 4).filter(|s| s.abs() > 1e-4).count();
                assert!(n > 1000, "{key}/{f}: {n} audible samples through the mixer");
            }
        }
    }

    #[test]
    fn tracks_are_separated_by_quiet() {
        let mut l = Playlist { n: 3, ..Default::default() };
        let t = Instant::now();
        assert_eq!(l.on_empty(t, false, 0), Some(0));
        // the track ends: a pause of 40-90 s
        assert_eq!(l.on_empty(t + Duration::from_secs(100), false, 7), None);
        assert_eq!(l.on_empty(t + Duration::from_secs(130), false, 0), None);
        assert_eq!(l.on_empty(t + Duration::from_secs(148), false, 0), Some(1));
        // skipping starts the next one at once
        l.skip();
        assert_eq!(l.on_empty(t + Duration::from_secs(150), false, 0), Some(2));
        assert_eq!(l.on_empty(t + Duration::from_secs(151), false, 0), None);
        assert_eq!(l.playing, None);
    }

    #[test]
    fn shuffle_never_repeats_the_last_track() {
        for rnd in 0..20 {
            let mut l = Playlist { n: 4, last: Some(2), ..Default::default() };
            assert_ne!(l.on_empty(Instant::now(), true, rnd), Some(2));
        }
    }

    #[test]
    fn a_collapsing_border_plays_one_lost_jingle() {
        let mut g = Gate::default();
        let t = Instant::now();
        assert!(g.admit("lost", t));
        assert!(!g.admit("lost", t + Duration::from_millis(500)));
        assert!(g.admit("built", t + Duration::from_millis(500)));
        assert!(g.admit("lost", t + Duration::from_secs(4)));
    }

    #[test]
    fn good_news_rises_bad_news_falls() {
        use rr_rules::BuildingType as B;
        let at = MapCoord::new(1, 1);
        let fin = |kind| Event::BuildingFinished { player: 0, at, kind };
        assert_eq!(news_sound(&fin(B::Woodcutter), 0).map(|s| s.0), Some("built"));
        assert_eq!(news_sound(&fin(B::Guardhouse), 0).map(|s| s.0), Some("fanfare"));
        assert_eq!(news_sound(&fin(B::Guardhouse), 1), None);
        let lost = Event::BuildingLost { player: 0, at, kind: B::Farm };
        assert_eq!(news_sound(&lost, 0).map(|s| s.0), Some("lost"));
        let cap = Event::BuildingCaptured { player: 1, at, by: 0 };
        assert_eq!(news_sound(&cap, 0).map(|s| s.0), Some("fanfare"));
        assert_eq!(news_sound(&cap, 1).map(|s| s.0), Some("lost"));
        assert_eq!(news_sound(&Event::PlayerDefeated { player: 0 }, 0).map(|s| s.0), Some("defeat"));
        // new-player round 6, #9: a refused command answers with a soft "no"
        let no = Event::CommandRejected {
            player: 0,
            cmd: rr_sim::Cmd::PlaceFlag { at },
            reason: rr_sim::RejectReason::NotBuildable,
        };
        assert_eq!(news_sound(&no, 0).map(|s| s.0), Some("refuse"));
        assert_eq!(news_sound(&no, 1), None);
    }
}
