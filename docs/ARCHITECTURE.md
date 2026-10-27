# Architecture

Status: **v0.3**, 2026-10-23. It replaces v0.2 and includes the findings of the architecture, netcode, QA and legal reviews.

Crate prefix: **`rr-`** ("Roads & Realms", working title). `s2-*` was dropped because it collides with Google's S2 geometry crates and leans on the trademark.

## 1. Principles

1. **The simulation is the domain.** Everything else (rendering, UI, audio, network, file formats, AI) is an adapter around it. Dependencies point **inward** only.
2. **The simulation is a pure function of (rules, map, setup, command log).** It uses no clock, no I/O, no floats, no threads, no iteration over hash maps, and no globals. The [determinism contract](#7-determinism-contract) is binding.
3. **Data supplies the numbers; code supplies the behavior.** Values live in TOML. Branching behavior lives in Rust.
4. **One way in, one way out.** The only input is `PlayerCommand` (stamped with `execute_gf`). The outputs are a borrowed read-only `View` and a stream of `Event`s.
5. **Headless first.** Every feature works and is tested without a window.
6. **Crate boundaries are the architecture.** A wrong `use` must not compile, and CI checks the dependency table.

## 2. Layers and crates

```
 Composition roots   rr-app (winit loop, pacing, scenes)   rr-editor   rr-cli
 ──────────────────────────────────────────────────────────────────────────────
 Adapters            rr-render   rr-ui   rr-audio   rr-net   rr-assets
 ──────────────────────────────────────────────────────────────────────────────
 Application         rr-session  (no clock: advance_to(gf), command sources,
                                  recorder, autosave hooks, sync hashes)
 ──────────────────────────────────────────────────────────────────────────────
 Ports / infra       rr-persist (postcard envelope)   rr-ai   rr-formats
 ──────────────────────────────────────────────────────────────────────────────
 Domain              rr-sim     (economy, transport, units, military, nature,
                                 seafaring, scheduler, step)
                     rr-world   (grid layers, terrain, BQ, territory, MapData)
                     rr-protocol(Command, PlayerCommand, Event, Rejected)
                     rr-rules   (typed rule definitions, validated, hashed)
                     rr-core    (ids, arenas, coords, Direction, Gf, RNG,
                                 StableHash, int math)
```

| Crate | License | May depend on |
|---|---|---|
| `rr-core` | MIT OR Apache-2.0 | std |
| `rr-rules` | GPL-3.0+ | core, serde, toml |
| `rr-protocol` | GPL-3.0+ | core, rules, serde |
| `rr-world` | GPL-3.0+ | core, rules |
| `rr-sim` | GPL-3.0+ | core, rules, protocol, world |
| `rr-ai` | GPL-3.0+ | sim (`View`, `AiObservation`), protocol |
| `rr-formats` | GPL-3.0+ (see the legal review: libsiedler2 is a reference, so it is not a clean room) | core, world (`MapData`) |
| `rr-assets` | GPL-3.0+ | formats, image |
| `rr-persist` | GPL-3.0+ | sim, protocol, postcard |
| `rr-session` | GPL-3.0+ | sim, protocol, persist, ai |
| `rr-render` | GPL-3.0+ | sim (view types), world, assets, wgpu |
| `rr-ui` | GPL-3.0+ | render, protocol, sim (views), assets |
| `rr-audio` | GPL-3.0+ | protocol, assets, kira, rustysynth |
| `rr-net` | GPL-3.0+ | protocol |
| `rr-app`, `rr-cli` | GPL-3.0+ | anything |
| `rr-editor` | GPL-3.0+ | world, formats, render, egui (no sim) |

Enforced by: `cargo-deny` (licenses, advisories, bans), a `tools/check_deps.py` script over `cargo metadata` against this table, and `reuse lint`.

## 3. Domain

### 3.1 `rr-core`
- `Gf(u32)` game frame. `MapCoord { x: u16, y: u16 }`. `Direction` (S2 order W, NW, NE, E, SE, SW). `Grid` with **wrapping** neighbor math, and ring and radius iterators in a fixed order.
- `Id<T> { idx: u32, gen: u32 }` and `Arena<T>` (dense storage with a free list; iteration in index order; the free list is part of the state and gets serialized).
- RNG: `Pcg32`, implemented in-house so the algorithm can never change underneath us. Randomness is **derived per use**: `rng_for(seed, gf, entity, Purpose::AnimalWalk)`. That way one new random call doesn't shift every later draw. A debug ring buffer records the last 1,000 RNG calls with their call sites.
- `trait StableHash { fn stable_hash(&self, h: &mut Xxh3); }` is implemented by hand or with a small derive. It is **never** computed by hashing serialized bytes, and never uses `DefaultHasher`.
- Integer helpers: `div_round_half_up`, `percent_of`. No `Fixed` type, because the sim doesn't need fractions. Rendering interpolation uses floats in the adapters.
- `MapData`: a neutral map description, produced by loaders and consumed by `World::new`.

### 3.2 `rr-rules`
Typed, validated definitions loaded from `data/rules/*.toml`: `WareDef, JobDef, BuildingDef, TerrainDef, NatureDef, MilitaryDef, StartingGoods, Defaults`, and `Program`s (see 3.5).
- Float values are **rejected** at load time. Percentages are integers, from 0 to 100 or in per-mille.
- The **rule-set hash** is part of every save, replay and lobby.
- Each value can carry `source = "rttr:BuildingConsts.h@<sha>"` or `"orig:measured:2026-11-02"`. This feeds the provenance table in Appendix A.

### 3.3 `rr-world`
Node layers as structure-of-arrays: altitude, terrain RSU/LSD, road bits, object ref, resource, owner, border, shading, BQ cache per player, fog per player. Also `Territory` (recomputed by dirty region) and `Bq` (recomputed by dirty region).
- A debug check compares both against a full recompute every 500 GF.
- `rr-world` knows nothing about economy or units, so the editor can use it on its own.

### 3.4 `rr-sim`
```
rr-sim/src/
  lib.rs        Sim, step(), View<'a>, AiObservation
  ctx.rs        Ctx<'a>: split borrows of all subsystems for one unit update
  schedule.rs   timing wheel (GF buckets, insertion-ordered) + far-future heap
  transport/    flags, roads, route cache (per-player road_epoch), carriers, donkeys, boats
  economy/      buildings, construction, requests/offers, distributor, warehouses
  units/        per-kind arenas: Carrier, Worker, Soldier, Geologist, Scout, Animal, Ship
  programs/     job program interpreter (compiled Op list)
  military/     garrisons, attacks, fights, capture
  nature/       trees, fields, stones, fish, animals
  seafaring/    harbors, ships, expeditions, sea routes
  players/      per-player settings and statistics
  hash.rs       per-subsystem StableHash + cheap sync hash
  invariants.rs the QA invariants (debug: every GF; release: on demand)
```

**Borrowing pattern.** No subsystem gets `&mut Sim`. `step()` destructures `self` into a `Ctx`:
```rust
pub(crate) struct Ctx<'a> {
    pub world: &'a mut World, pub transport: &'a mut Transport, pub econ: &'a mut Economy,
    pub military: &'a mut Military, pub nature: &'a mut Nature, pub sched: &'a mut Scheduler,
    pub rules: &'a Rules, pub gf: Gf, pub seed: u64, pub effects: &'a mut Vec<Effect>,
}
```
Units are updated one at a time with `&mut Unit` plus `&mut Ctx`. Effects that can't be applied right away, such as "destroy building X" during a fight, are pushed to `effects`. They are drained in push order before the next phase of the tick.

**Tick order (one GF), documented and tested:**
1. Validate and apply the commands for this GF, sorted by `(player, seq)`. A rejected command emits `Event::CommandRejected` and leaves the state untouched.
2. Dispatch the due timer events of the bucket in insertion order. Events scheduled for the *current* GF during dispatch run **next GF**. Events for dead ids are skipped (generation check).
3. Drain effects.
4. Territory and BQ dirty-region recompute. This happens **before** distribution, so lost buildings get no wares.
5. Distributor pass for each player with `gf % K == player % K`, to spread the load.
6. Statistics every N GF.
7. Emit events for the frontend.

**Routing (performance and determinism).**
- Each player has a `road_epoch` that increments on any flag or road change.
- Distances between flags are cached per (source flag) with Dial's algorithm (an integer bucket queue) and invalidated by the epoch.
- The distributor runs one multi-source Dijkstra per ware type per pass, not one per offer × request.
- A ware re-plans only when it reaches a flag *and* the epoch has changed.
- If the original has a congestion term (to be verified in RTTR), it is quantized.
- Free walking (off roads) uses a bounded A* with a wrapping hex-distance heuristic and the tie-break `(f, g, node_index)`.

### 3.5 Job programs
Programs are compiled at load time into a validated `Vec<Op>`:
```rust
enum Op { Wait(Gf), FindObject { kind: ObjKind, radius: u8, else_goto: Label }, WalkTo(Target),
          Anim { id: AnimId, dur: Gf, sound: Option<SoundId> }, Transform { target: Target, into: ObjKind },
          Carry(WareId), WalkHome, DropAtFlag, ConsumeInputs, Produce(WareId),
          JumpIf { cond: Cond, to: Label }, Goto(Label), Native(NativeBehavior) }
```
- Unit state is `(ProgramId, pc: u16, locals: [u32; 4])`, which saves cleanly.
- **Rule of thumb:** if a behavior needs more than about 2 conditions, it is `Native` and written in Rust. That covers fights, ships, carriers and hunters chasing animals.

### 3.6 Public API
```rust
impl Sim {
    pub fn new(rules: Arc<Rules>, map: &MapData, setup: &GameSetup) -> Result<Self, SetupError>;
    pub fn builder(rules: Arc<Rules>) -> SimBuilder;      // tests: place buildings/roads directly
    pub fn step(&mut self, cmds: &[PlayerCommand]) -> StepOutput;  // exactly one GF
    pub fn view(&self, fog: Option<PlayerId>) -> View<'_>;         // concrete borrowed view, no dyn
    pub fn observe(&self, p: PlayerId) -> AiObservation;           // owned, Arc'd layers, thread-safe
    pub fn sync_hash(&self) -> u64;   // cheap, every network frame
    pub fn full_hash(&self) -> SubsystemHashes;  // per subsystem, every ~10 s / on demand
    pub fn dump(&self, w: &mut dyn Write);       // diffable text dump for desync hunting
}
```
For the renderer, figure motion is exposed as `Motion::Walk { from, to, start: Gf, dur: Gf }`. The renderer computes `t = (render_gf - start) / dur` with a fractional `render_gf`, so it needs no snapshots.

## 4. Application layer: `rr-session`
```rust
pub trait CommandSource { fn poll(&mut self, upto: Gf) -> Vec<PlayerCommand>; } // every cmd has execute_gf
pub struct Session { sim: Sim, sources: Vec<Box<dyn CommandSource>>, recorder: Recorder, .. }
impl Session { pub fn advance_to(&mut self, gf: Gf) -> Result<Vec<Event>, Stall>; }
```
- **No wall clock.** `rr-app` turns real time and game speed into a target GF.
- **AI contract.** The AI reads `observe()` at GF *n* (it may run on a worker thread). Its commands are stamped `execute_gf = n + L`. If they haven't arrived by then, `advance_to` returns `Stall` and the app waits. It never applies late commands.
- AI commands are **recorded like human commands**. A replay never re-runs the AI. The AI has its own RNG.
- Pause, game speed and player drop are commands too.

## 5. Adapters
- **`rr-render`** (wgpu, pinned version): chunked terrain meshes (16×16 nodes, rebuilt when dirty), sprites batched in painter's order per row, atlases, player color through an index mask and a palette uniform, palette-cycling for water and lava. A **CPU reference renderer** writes palette-indexed output for exact tests.
- **`rr-ui`**: immediate-mode, S2-style skin, drawn through `rr-render`'s batcher. Windows return `UiAction` (`Open(WindowId)`, `Close`, `Issue(Command)`). The vertical slice uses **egui** first, and the S2 skin is a later milestone. Debug overlays stay egui.
- **`rr-audio`**: kira. `Event::Sound` is positioned by node. Music uses XMIDI converted to MIDI, played through rustysynth with a freely licensed SoundFont (GeneralUser GS; check its license).
- **`rr-net`**: lockstep over TCP. The host only orders commands; every client validates them identically.
  - Network frame of 4 GF, with lag adapted to measured RTT.
  - Sync hash per network frame, full hash every ~10 s, a text dump on mismatch.
  - Reconnect and late join: the host saves at a network-frame boundary and transfers the save, then the client fast-forwards.
  - The size of a road path in a command is capped.
- **`rr-assets`**: `AssetSource` packs: `OriginalPack` (the user's install), `OpenPack` (our art, see [ART.md](ART.md)) and `PlaceholderPack`. `LayeredSource` lets packs override each other per key.
- **`rr-formats`**: readers for WLD/SWD, LST, BOB, BBM/LBM, IDX/DAT, and an SWD writer for exporting to RTTR. Fuzzed with `cargo fuzz`.
- **`rr-persist`**: postcard inside our envelope `{magic, save_version, sim_version, rules_hash, build_id, payload}`. Every sim enum has explicit discriminants. The timer wheel is serialized in canonical order, including `next_seq` and the arena free lists. Test: `full_hash(load(save(s))) == full_hash(s)`, then 10k GF from both must match.

## 6. Workspace
```
roads-and-realms/
├── Cargo.toml          workspace; [workspace.lints]; profile.dev.package."*" opt-level=3; rr-sim opt-level=1;
│                       profile.release overflow-checks=true for rr-sim (and lto)
├── clippy.toml         disallowed_types/methods (HashMap iteration, Instant::now, thread::spawn, f32/f64 in sim)
├── deny.toml  REUSE.toml  LICENSES/
├── crates/rr-*/
├── data/rules/  data/reference/  data/maps/  data/art/  data/i18n/{en,de}.ftl
├── tools/mapgen/europe/  tools/art/  tools/rttr-oracle/ (C++ driver, GPL)  tools/check_deps.py
├── tests/scenarios/  tests/replays/
├── docs/
```

## 7. Determinism contract
- [ ] No `f32`/`f64`, no iteration over `HashMap`/`HashSet`, no `usize` in state, hashes or the wire format, no `std::time`, no threads, no `rand` in `rr-core`, `rr-rules`, `rr-world` or `rr-sim`. Enforced by `clippy.toml` and a CI grep.
- [ ] Overflow is explicit (`checked_`, `saturating_` or `wrapping_`), and release builds of the sim run with overflow checks on.
- [ ] Every sort uses a total key that ends with an id. Iteration is in index or key order.
- [ ] RNG is derived per use. The UI and AI never touch the sim's RNG.
- [ ] Every input, including AI, pause and speed, is a logged command with `execute_gf`.
- [ ] Timer semantics (bucket order, same-GF insertion goes to the next GF, dead-id skip) are specified and tested.
- [ ] The tick phase order and the iteration order inside each phase are documented (3.4) and tested.
- [ ] `StableHash`: a sync hash per network frame, a full hash per subsystem periodically.
- [ ] Save → load → continue gives the same result bit for bit as an uninterrupted run (CI).
- [ ] CI cross-checks Linux and Windows, x86_64, aarch64 and i686, and debug and release.
- [ ] Desync tooling: dump, structural diff, RNG call-site log, `rr-cli replay bisect`.
- [ ] Goldens are tagged with the rule hash. `--bless` requires a changelog line and shows a semantic diff.

## 8. Cross-cutting
- Errors: `thiserror` in libraries, `anyhow` in binaries. The sim never panics on input.
- Logging: `tracing`. Logging in the sim sits behind a feature flag and must never change behavior.
- Localization: Fluent (`data/i18n/en.ftl`, `de.ftl`) from day one.
- Testing tools: `cargo nextest`, `proptest`, `insta`, `iai-callgrind`, `cargo fuzz`.
- Performance budgets on Europe with 6 AIs, mid-game, headless:
  - ≥ 1,000 GF/s,
  - p99 tick < 2 ms and no tick > 10 ms,
  - RSS < 500 MB,
  - save/load < 1 s.

## 9. As built (2026-10-23) — deviations from the plan above

The first implementation run made pragmatic choices; they are recorded here so the plan stays honest.

| Plan | As built | Why / when to revisit |
|---|---|---|
| 16 crates incl. `rr-protocol`, `rr-persist`, `rr-session`, `rr-render`, `rr-ui`, `rr-net`, `rr-editor` | 12 crates: `rr-core`, `rr-rules`, `rr-world`, `rr-formats`, `rr-mapgen`, `rr-sim`, `rr-ai`, `rr-game` (missions, casts, goals, truces and the peace filter, the campaign's story in `story.rs`, without egui: the app and `rr-cli` build their games from it), `rr-audio`, `rr-net`, `rr-cli`, `rr-app` (the editor is a module of `rr-app`) | Persistence and replays live in `rr-sim` (`save`, `load`, `Replay`); render + UI in `rr-app`. Split when multiplayer/editor start. |
| `Ctx<'a>` split borrows | `impl Sim` blocks per module (`transport`, `economy`, `figures`, `military`, `nature`, `territory`, `fog`), ids into arenas | Simpler and fast enough (6 AIs on Europe: ~0.1 ms/GF). |
| Timing wheel | `BTreeMap<(gf, seq), Ev>` scheduler | Same order guarantees; swap if profiling demands. |
| Job programs as `Vec<Op>` | Hand-written state machines in `figures.rs` | Fewer moving parts for v0; programs remain the goal for modding. |
| winit + wgpu renderer | eframe/egui with the glow (OpenGL) backend; terrain as textured egui meshes | Runs under Xvfb/llvmpipe for headless screenshots and UI tests. |
| kira audio | rodio 0.21 | Small API, Ogg Vorbis. |
| Original assets as default pack | Procedural terrain textures; people and animals as lit vector shapes (`animals.rs`, `sprites.rs`); Roman buildings as AI-painted pictures keyed by `tools/art/post.py` (agy and Codex); the rest pre-rendered by `tools/art/prerender.py` | No original files are needed to play. See docs/ART.md §0. |
| Track B Europe after 1.0 | Done now: Natural Earth coastlines, real mountain ranges, land bridges at Dover/Øresund/Messina; plus Mare Nostrum, North America and the Far East | Seafaring exists (ships, expeditions, colonies, overseas supply); bridges stay where the original's maps would let you walk. Japan is reachable only by ship. Ranges and woods fade around starts so every start has ≥ 200 building spots (tested). |
| Multiplayer after 1.0 | `rr-net`: LAN lockstep over TCP. Turns of 4 GF, commands run 2 turns later, the host bundles every player's commands per turn in player order; computer players run on every machine; clients send `sync_hash` every 50 turns and the host stops the game on a mismatch. The host sends the map as `.rrmap`, so float differences in map generation don't matter | No reconnect, no internet relay yet |
| "Cheap" per-frame sync hash | `sync_hash` = hash of the full save (everything but transient events) | Called at the end of soaks and replays only; per-frame network checks need a cheaper rolling hash when multiplayer starts. |
| Map generation shared by peers | Maps are generated from the seed with `f32` maths (`cos`, `sqrt`) | Fine for single player, saves and replays (they store the map). Multiplayer must send the `MapData` instead of the seed. |
| `clippy.toml` and overflow checks (§7) | `crates/rr-sim/clippy.toml` forbids `HashMap`/`HashSet` and float maths; release builds have `overflow-checks = true` | No CI service is set up (no remote); `tools/check.sh` is the gate. |

Performance (2026-10-23, after the review fixes): Europe with 6 AIs, 120 000 GF (100 min of
game time) runs in 11 s on one core, about 0.09 ms per GF, of which the AIs take 40 %. The
simulation still runs one Dijkstra per distribution request and per job assignment
(architecture review #10/#11); there is no route cache yet (`road_epoch` is the hook for one).
