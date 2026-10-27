//! The map editor (PLAN §21): paint terrain, raise and lower the land, place trees, stones and
//! resources, set the headquarters and harbor spots, save `.rrmap` files and try a map out.

use crate::app::App;
use eframe::egui::{self, Color32, RichText, Vec2};
use rr_core::{Animal, Landscape, MapCoord, MapData, MapObject, Resource};
use rr_sim::{GameSetup, Sim};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Terrain,
    Raise,
    Lower,
    Flatten,
    Smooth,
    Trees,
    Stones,
    Clear,
    Decoration,
    Animals,
    Resource,
    Headquarters,
    Harbor,
}

/// Terrain ids with their names for a landscape, in the order of the palette (the same ids
/// look different in each landscape; names after RTTR's terrain descriptions).
fn terrains(l: Landscape) -> &'static [(u8, &'static str)] {
    match l {
        Landscape::Greenland => &[
            (8, "Meadow"),
            (9, "Meadow (2)"),
            (10, "Meadow (3)"),
            (15, "Flower meadow"),
            (14, "Steppe"),
            (0, "Savannah"),
            (18, "Mountain meadow"),
            (4, "Desert"),
            (7, "Desert (2)"),
            (3, "Swamp"),
            (5, "Water"),
            (6, "Shallow water"),
            (19, "Reef water"),
            (1, "Mountain"),
            (11, "Mountain (2)"),
            (12, "Mountain (3)"),
            (13, "Mountain (4)"),
            (34, "Flat mountain"),
            (2, "Snow"),
            (16, "Lava"),
            (20, "Lava (2)"),
            (21, "Lava (3)"),
            (22, "Lava (4)"),
        ],
        Landscape::Wasteland => &[
            (8, "Pasture"),
            (9, "Pasture (2)"),
            (10, "Pasture (3)"),
            (15, "Flower pasture"),
            (14, "Light steppe"),
            (0, "Dark steppe"),
            (18, "Alpine pasture"),
            (4, "Wasteland"),
            (7, "Wasteland (2)"),
            (5, "Moor"),
            (6, "Shallow moor"),
            (19, "Reef moor"),
            (1, "Mountain"),
            (11, "Mountain (2)"),
            (12, "Mountain (3)"),
            (13, "Mountain (4)"),
            (34, "Flat mountain"),
            (2, "Lava with few stones"),
            (3, "Lava with many stones"),
            (16, "Lava"),
            (17, "Lava (2)"),
            (20, "Lava (3)"),
            (21, "Lava (4)"),
            (22, "Lava (5)"),
        ],
        Landscape::Winter => &[
            (8, "Tundra"),
            (9, "Tundra (2)"),
            (10, "Tundra (3)"),
            (14, "Tundra (4)"),
            (15, "Flower tundra"),
            (0, "Taiga"),
            (18, "Snow"),
            (4, "Ice"),
            (7, "Ice (2)"),
            (2, "Ice floe"),
            (3, "Ice floes"),
            (5, "Water"),
            (6, "Shallow water"),
            (19, "Reef water"),
            (1, "Mountain"),
            (11, "Mountain (2)"),
            (12, "Mountain (3)"),
            (13, "Mountain (4)"),
            (34, "Flat mountain"),
            (16, "Lava"),
            (20, "Lava (2)"),
            (21, "Lava (3)"),
            (22, "Lava (4)"),
        ],
    }
}

/// Decorations the editor places: original object ids as the game draws them (bushes and
/// flowers can be walked over, rocks block a node).
const DECORATIONS: &[(u8, &str)] = &[(0x0c, "Bushes"), (0x02, "Flowers"), (0x16, "Rocks")];

/// Resources the brush can lay down (kind, name).
const RESOURCES: &[(u8, &str)] =
    &[(0, "Nothing"), (1, "Coal"), (2, "Iron ore"), (3, "Gold"), (4, "Granite"), (5, "Fish"), (6, "Water")];

pub struct Editor {
    pub map: MapData,
    pub tool: Tool,
    pub brush: u16,
    pub terrain: u8,
    pub resource: u8,
    pub amount: u8,
    pub slot: usize,
    pub name: String,
    /// The preview simulation must be rebuilt; when it last was.
    pub stale: bool,
    pub previewed: Option<std::time::Instant>,
    pub message: String,
    /// The last node painted while dragging (so a drag paints each node once).
    pub last: Option<MapCoord>,
    pub new_size: u16,
    pub new_landscape: Landscape,
    /// Earlier states of the map, newest last (one per click or brush stroke), and the states
    /// undone since (for redo).
    pub undo: Vec<MapData>,
    pub redo: Vec<MapData>,
    /// The map as last saved or opened: changes since then are not on disk.
    pub saved: MapData,
    /// The file the map was last saved to; another existing file is only replaced on a second
    /// Save.
    pub file: Option<std::path::PathBuf>,
    replace_ok: Option<std::path::PathBuf>,
    /// Asked: throw away the unsaved changes for a new map, or to leave?
    pub confirm: Option<Confirm>,
    /// Draw the resources in the ground (coal, ores, granite, fish, water).
    pub show_res: bool,
    /// The height Flatten levels to during this stroke (taken where the stroke starts).
    flat_to: Option<u8>,
    /// Trees: one kind (0..=8) or a mix; how dense (1 a quarter of the nodes .. 4 all).
    pub tree_kind: Option<u8>,
    pub density: u8,
    /// Granite pile size 1..=6.
    pub stone_size: u8,
    /// One kind of animal, or a mix.
    pub animal: Option<Animal>,
    /// The decoration placed: an original object id (see `DECORATIONS`).
    pub decor: u8,
    /// Map files to open (path, label), read when the list is first shown.
    files: Option<Vec<(std::path::PathBuf, String)>>,
    /// The map's own mission (briefing, goal, truce, names), saved next to it.
    pub mission: crate::missions::MapMission,
    /// Draw the game's made-up scenery (bushes, stumps, boulders) too: off, only the map's own
    /// objects show (map maker round 1, #9).
    pub show_scenery: bool,
    /// What the last save changed beyond the file (slots renumbered), for its message.
    pub save_note: Option<String>,
    /// The "Player names" field's own text while the mission is edited.
    pub names_text: Option<String>,
    /// The mission as last saved or opened (for unsaved changes).
    pub saved_mission: crate::missions::MapMission,
    /// Why the map's mission file couldn't be read: then Save leaves it alone.
    pub mission_unreadable: Option<String>,
    /// Terrain one triangle at a time (for clean coasts and mountain edges).
    pub tri: bool,
    pub last_tri: Option<(MapCoord, bool)>,
}

/// An action that would lose unsaved changes, waiting for a yes.
pub enum Confirm {
    New(Box<MapData>),
    Open(std::path::PathBuf),
    Leave,
}

impl Editor {
    pub fn new(map: MapData) -> Self {
        let name = if map.name.is_empty() { crate::i18n::tr("My map").to_string() } else { map.name.clone() };
        Self {
            saved: map.clone(),
            redo: Vec::new(),
            file: None,
            replace_ok: None,
            confirm: None,
            show_res: true,
            flat_to: None,
            tree_kind: None,
            density: 2,
            stone_size: 5,
            animal: None,
            decor: DECORATIONS[0].0,
            files: None,
            tri: false,
            mission: Default::default(),
            mission_unreadable: None,
            saved_mission: Default::default(),
            names_text: None,
            save_note: None,
            show_scenery: false,
            last_tri: None,
            map,
            tool: Tool::Terrain,
            brush: 1,
            terrain: 8,
            resource: 1,
            amount: 5,
            slot: 0,
            name,
            stale: true,
            previewed: None,
            message: String::new(),
            last: None,
            new_size: 96,
            new_landscape: Landscape::Greenland,
            undo: Vec::new(),
        }
    }

    pub fn blank(size: u16, landscape: Landscape) -> MapData {
        let mut m = MapData::flat(size, size, 8);
        m.landscape = landscape;
        m.hq = vec![None; 7];
        m
    }

    /// The map as a simulation without players, for drawing.
    pub fn preview(&self) -> Sim {
        let mut m = self.map.clone();
        m.hq.clear();
        Sim::new(crate::rules(), &m, &GameSetup::default()).expect("a map without players")
    }

    /// Remember the map before a click or a brush stroke (as many steps as fit in about 64 MB,
    /// at least 30).
    pub fn checkpoint(&mut self) {
        self.flat_to = None;
        self.undo.push(self.map.clone());
        self.redo.clear();
        let nodes = self.map.altitude.len().max(1);
        let keep = (64 * 1024 * 1024 / (nodes * 12)).max(30);
        if self.undo.len() > keep {
            self.undo.remove(0);
        }
    }

    pub fn undo(&mut self) {
        if let Some(m) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.map, m));
            self.stale = true;
        }
    }

    pub fn redo(&mut self) {
        if let Some(m) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.map, m));
            self.stale = true;
        }
    }

    /// Changes not saved yet?
    pub fn dirty(&self) -> bool {
        // (a mission edit too: it was lost without a question; QA round 8, #12)
        self.map != self.saved || self.mission != self.saved_mission
    }

    /// Paint the current terrain on one triangle: `rsu` the one pointing up below the node, else
    /// the one pointing down to its right.
    pub fn paint_triangle(&mut self, p: MapCoord, rsu: bool) {
        let i = self.map.grid.index(p);
        if rsu {
            self.map.terrain_rsu[i] = self.terrain;
        } else {
            self.map.terrain_usd[i] = self.terrain;
        }
        self.stale = true;
    }

    /// Use the current tool at `p`.
    pub fn apply(&mut self, p: MapCoord) {
        let g = self.map.grid;
        // what the land is like around each node, for tools that only fit some of it
        let world = rr_world::World::new(&self.map);
        let all_water = |q: MapCoord| world.terrains_around(q).iter().all(|t| rr_world::terrain::is_water(*t));
        let any_water = |q: MapCoord| world.terrains_around(q).iter().any(|t| rr_world::terrain::is_water(*t));
        let mountain = |q: MapCoord| world.terrains_around(q).iter().any(|t| rr_world::terrain::is_mountain(*t));
        // Raise and Lower shape the slopes around the brush too (one step per node)
        let reach = match self.tool {
            Tool::Raise | Tool::Lower => (self.brush + 24).min(g.width.min(g.height) / 2 - 1),
            _ => self.brush,
        };
        let nodes: Vec<MapCoord> = g.within(p, reach).collect();
        let base = self.map.altitude[g.index(p)];
        let flat = *self.flat_to.get_or_insert(base);
        // heights before this step, so Smooth reads the old land, not what it just changed
        let before = self.map.altitude.clone();
        for (k, q) in nodes.iter().enumerate() {
            let i = g.index(*q);
            match self.tool {
                Tool::Terrain => {
                    self.map.terrain_rsu[i] = self.terrain;
                    self.map.terrain_usd[i] = self.terrain;
                }
                // hills, not mesas: the brush is the top, which moves by one; around it the land
                // follows only where it would be steeper than one step per node
                Tool::Raise => {
                    let d = (g.distance(p, *q) as i32 - self.brush as i32).max(0);
                    if (before[i] as i32) < base as i32 + 1 - d {
                        self.map.altitude[i] = (before[i] + 1).min(60);
                    }
                }
                Tool::Lower => {
                    let d = (g.distance(p, *q) as i32 - self.brush as i32).max(0);
                    if before[i] as i32 > base as i32 - 1 + d {
                        self.map.altitude[i] = before[i].saturating_sub(1);
                    }
                }
                Tool::Flatten => self.map.altitude[i] = flat,
                Tool::Smooth => {
                    let sum: u32 =
                        g.neighbors(*q).iter().map(|n| before[g.index(*n)] as u32).sum::<u32>() + 2 * before[i] as u32;
                    self.map.altitude[i] = ((sum + 4) / 8) as u8;
                }
                Tool::Trees => {
                    if self.map.objects[i] == MapObject::None && dense(*q, self.density) && !any_water(*q) {
                        let kind = self.tree_kind.unwrap_or(((q.x as u32 * 7 + q.y as u32 * 13) % 9) as u8);
                        self.map.objects[i] = MapObject::Tree(kind);
                    }
                }
                Tool::Stones => {
                    if self.map.objects[i] == MapObject::None && dense(*q, self.density) && !any_water(*q) {
                        let size = self.stone_size.clamp(1, 6);
                        self.map.objects[i] = MapObject::Granite { kind: (q.x % 2) as u8, size };
                    }
                }
                Tool::Decoration => {
                    if self.map.objects[i] == MapObject::None && dense(*q, self.density) && !any_water(*q) {
                        self.map.objects[i] = MapObject::Decoration(self.decor);
                    }
                }
                Tool::Animals => {
                    // a few animals, not a herd on every node; ducks by the water
                    let kinds = [Animal::Rabbit, Animal::Fox, Animal::Stag, Animal::Deer, Animal::Sheep];
                    let kind = self.animal.unwrap_or(kinds[(q.x as usize + q.y as usize) % kinds.len()]);
                    let place = if kind == Animal::Duck { any_water(*q) } else { !all_water(*q) };
                    if self.map.animals[i].is_none() && (q.x as usize * 5 + q.y as usize * 3 + k) % 7 == 0 && place {
                        self.map.animals[i] = Some(kind);
                    }
                }
                Tool::Clear => {
                    self.map.objects[i] = MapObject::None;
                    self.map.animals[i] = None;
                }
                Tool::Resource => {
                    let a = self.amount.clamp(1, 7);
                    // ores and granite only in mountains, fish only in water (as mines and
                    // fishers can reach them); water and "none" anywhere
                    let fits = match self.resource {
                        1..=4 => mountain(*q),
                        5 => any_water(*q),
                        _ => true,
                    };
                    if !fits {
                        continue;
                    }
                    self.map.resources[i] = match self.resource {
                        1 => Resource::Coal(a),
                        2 => Resource::Iron(a),
                        3 => Resource::Gold(a),
                        4 => Resource::Granite(a),
                        5 => Resource::Fish(a),
                        6 => Resource::Water,
                        _ => Resource::None,
                    };
                }
                Tool::Headquarters | Tool::Harbor => {}
            }
        }
        match self.tool {
            Tool::Headquarters => {
                if self.map.hq.len() < 7 {
                    self.map.hq.resize(7, None);
                }
                // a headquarters sits on an even row (as the original's maps have them)
                let at = MapCoord::new(p.x, p.y & !1);
                self.map.hq[self.slot] = if self.map.hq[self.slot] == Some(at) { None } else { Some(at) };
            }
            Tool::Harbor => {
                if let Some(i) = self.map.harbors.iter().position(|h| *h == p) {
                    self.map.harbors.remove(i);
                } else if any_water(p) && !all_water(p) {
                    self.map.harbors.push(p);
                } else {
                    self.message = crate::i18n::tr("A harbor spot goes on the coast.").into();
                }
            }
            _ => {}
        }
        self.stale = true;
    }

    /// Write the map in the original's `.swd` format next to the editor's maps.
    pub fn export_swd(&mut self) -> Result<std::path::PathBuf, String> {
        let clean: String = self.name.chars().filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_')).collect();
        let clean = clean.trim();
        if clean.is_empty() {
            return Err(crate::i18n::tr("Give the map a name.").into());
        }
        let dir = maps_dir();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let p = dir.join(format!("{clean}.swd"));
        let mut m = self.map.clone();
        m.name = clean.to_string();
        m.hq = m.hq.iter().flatten().map(|p| Some(*p)).collect();
        crate::app::write_safely(&p, &rr_formats::wld::write(&m)).map_err(|e| e.to_string())?;
        Ok(p)
    }

    /// Save and say how it went in the panel, with what still keeps the map from being played.
    pub fn save_with_message(&mut self) {
        self.message = match self.save() {
            Ok(p) => {
                let mut m = format!("{} {}", crate::i18n::tr("Saved to"), p.display());
                for problem in map_problems(&self.map) {
                    m += "\n⚠ ";
                    m.push_str(&problem);
                }
                if let Some(n) = &self.save_note {
                    m += &format!("\n⚠ {n}");
                }
                if let Some(e) = &self.mission_unreadable {
                    m += &format!("\n⚠ {e}\n{}", crate::i18n::tr(MISSION_KEPT));
                }
                m
            }
            Err(e) => e,
        };
    }

    /// Read the mission of the map at `path` (kept unread, and untouched, if it is broken).
    pub fn open_mission(&mut self, path: &std::path::Path) {
        self.names_text = None;
        match crate::missions::MapMission::read(path) {
            Ok(m) => {
                self.mission = m.unwrap_or_default();
                self.saved_mission = self.mission.clone();
                self.mission_unreadable = None;
            }
            Err(e) => {
                self.mission = Default::default();
                self.saved_mission = Default::default();
                self.message = format!("⚠ {e}\n{}", crate::i18n::tr(MISSION_KEPT));
                self.mission_unreadable = Some(e);
            }
        }
    }

    pub fn save(&mut self) -> Result<std::path::PathBuf, String> {
        self.save_in(&maps_dir())
    }

    /// Save into `dir` (the maps folder; tests use their own).
    pub fn save_in(&mut self, dir: &std::path::Path) -> Result<std::path::PathBuf, String> {
        let clean: String = self.name.chars().filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_')).collect();
        let clean = clean.trim();
        if clean.is_empty() {
            return Err(crate::i18n::tr("Give the map a name.").into());
        }
        self.map.name = clean.to_string();
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let p = dir.join(format!("{clean}.rrmap"));
        // another map of that name is only replaced when asked twice
        if p.exists() && self.file.as_ref() != Some(&p) && self.replace_ok.as_ref() != Some(&p) {
            self.replace_ok = Some(p);
            return Err(crate::i18n::tr("A map with this name exists. Save again to replace it.").into());
        }
        // players are numbered without gaps in a map file; the mission's names move with their
        // slots, and the editor says so (map maker round 1, #5: Carl's slot 3 became slot 2,
        // and his name went to another)
        let mut m = self.map.clone();
        m.hq = m.hq.iter().flatten().map(|p| Some(*p)).collect();
        let kept: Vec<usize> = (0..self.map.hq.len()).filter(|i| self.map.hq[*i].is_some()).collect();
        let moved: Vec<String> = kept
            .iter()
            .enumerate()
            .filter(|(new, old)| *new != **old)
            .map(|(new, old)| {
                crate::i18n::show(&crate::i18n::msg(
                    "Slot {1} is now slot {2}.",
                    &[&(old + 1).to_string(), &(new + 1).to_string()],
                ))
            })
            .collect();
        self.save_note = None;
        if !moved.is_empty() {
            if !self.mission.names.is_empty() {
                let names = &self.mission.names;
                self.mission.names = kept.iter().map(|i| names.get(*i).cloned().unwrap_or_default()).collect();
                self.names_text = None;
            }
            // the goal's slot and the human's move too (QA round 8, #2: they kept the old number)
            let new_slot = |old: u8| kept.iter().position(|k| *k == old as usize).map(|n| n as u8);
            if let Some(crate::missions::Objective::DefeatRival(p)) = self.mission.objective {
                self.mission.objective = new_slot(p).map(crate::missions::Objective::DefeatRival);
            }
            if let Some(h) = self.mission.human {
                self.mission.human = new_slot(h);
            }
            self.save_note =
                Some(format!("{} {}", crate::i18n::tr("Players renumbered without gaps."), moved.join(" ")));
        }
        crate::app::write_safely(&p, &rr_formats::rrmap::write(&m)).map_err(|e| e.to_string())?;
        // the mission goes next to the map (or away, when there is none)
        let mp = crate::missions::MapMission::path_for(&p);
        if self.mission_unreadable.is_some() {
            // a mission file we couldn't read stays as it is (map maker round 1, #3)
        } else if self.mission.is_empty() {
            let _ = std::fs::remove_file(&mp);
        } else {
            crate::app::write_safely(&mp, self.mission.to_json().as_bytes()).map_err(|e| e.to_string())?;
        }
        self.saved = self.map.clone();
        self.saved_mission = self.mission.clone();
        self.file = Some(p.clone());
        Ok(p)
    }

    /// Can a game start on this map? (Every headquarters needs room for a castle.)
    pub fn check(&self) -> Result<(), String> {
        let mut m = self.map.clone();
        m.hq = m.hq.iter().flatten().map(|p| Some(*p)).collect();
        let setup = GameSetup { players: vec![Default::default(); m.hq.len()], ..Default::default() };
        Sim::new(crate::rules(), &m, &setup).map(|_| ()).map_err(|e| match e {
            rr_sim::SetupError::HqNotBuildable(i) => {
                format!("{} {}", crate::i18n::tr("No room for a headquarters of player"), i + 1)
            }
            e => e.to_string(),
        })
    }

    /// Players the map has room for (headquarters set).
    pub fn players(&self) -> usize {
        self.map.hq.iter().flatten().count()
    }
}

/// Where the editor keeps maps (the menu lists them too).
/// The mission's goal the game checks, and the human's slot (map maker round 1, #2).
fn mission_goal(ui: &mut egui::Ui, m: &mut crate::missions::MapMission) {
    use crate::missions::Objective as O;
    let kinds: [(&str, Option<O>); 6] = [
        ("only the text above", None),
        ("defeat all", Some(O::DefeatAll)),
        ("defeat the player in slot", Some(O::DefeatRival(2))),
        ("hold this % of the land", Some(O::HoldLand(40))),
        ("survive this many minutes", Some(O::Survive(60))),
        ("mint this many gold coins", Some(O::Make(rr_rules::Ware::Coins, 30))),
    ];
    let same =
        |a: Option<O>, b: Option<O>| a.map(|o| std::mem::discriminant(&o)) == b.map(|o| std::mem::discriminant(&o));
    let current = kinds.iter().find(|(_, o)| same(*o, m.objective)).map_or(kinds[0].0, |k| k.0);
    ui.horizontal(|ui| {
        ui.label(crate::i18n::tr("Goal the game checks:"));
        egui::ComboBox::from_id_salt("ed_goal").selected_text(crate::i18n::tr(current)).show_ui(ui, |ui| {
            for (name, o) in kinds {
                if ui.selectable_label(same(o, m.objective), crate::i18n::tr(name)).clicked() && !same(o, m.objective) {
                    m.objective = o;
                }
            }
        });
        match &mut m.objective {
            Some(O::DefeatRival(p)) => {
                // (shown from 1, as the slots in the editor)
                let mut slot = *p as u32 + 1;
                ui.add(egui::DragValue::new(&mut slot).range(2..=8));
                *p = (slot - 1) as u8;
            }
            Some(O::HoldLand(n) | O::Survive(n) | O::Make(_, n)) => {
                ui.add(egui::DragValue::new(n).range(1..=600));
            }
            _ => {}
        }
    });
    ui.horizontal(|ui| {
        let mut fixed = m.human.is_some();
        if ui.checkbox(&mut fixed, crate::i18n::tr("The human plays slot")).changed() {
            m.human = fixed.then_some(0);
        }
        if let Some(h) = &mut m.human {
            let mut slot = *h as u32 + 1;
            ui.add(egui::DragValue::new(&mut slot).range(1..=8));
            *h = (slot - 1) as u8;
        }
    });
}

/// The players' names from the field's text: comma-separated, trimmed; none if all are empty.
fn parse_names(text: &str) -> Vec<String> {
    let names: Vec<String> = text.split(',').map(|n| n.trim().to_string()).collect();
    if names.iter().all(String::is_empty) { Vec::new() } else { names }
}

/// What the editor does with a mission file it can't read.
const MISSION_KEPT: &str = "The mission file can't be read: it is left as it is, and the mission fields stay empty. Fix the file, or remove it.";

pub fn maps_dir() -> std::path::PathBuf {
    App::save_path().with_file_name("maps")
}

/// The editor's tool window.
#[allow(clippy::too_many_lines)] // to split (architect round 2, #8: no new long ones)
pub fn panel(app: &mut App, ctx: &egui::Context) {
    let Some(ed) = app.editor.as_mut() else { return };
    let mut leave = false;
    let mut test = None;
    let mut new_map: Option<(MapData, Option<std::path::PathBuf>)> = None;
    let mut open: Option<std::path::PathBuf> = None;
    egui::Window::new(crate::i18n::tr("Map editor"))
        .collapsible(false)
        .resizable(false)
        .default_pos(egui::pos2(16.0, 16.0))
        .show(ctx, |ui| {
            egui::Grid::new("ed_names").num_columns(2).show(ui, |ui| {
                ui.label(crate::i18n::tr("Name"));
                ui.text_edit_singleline(&mut ed.name);
                ui.end_row();
                ui.label(crate::i18n::tr("Author"));
                ui.text_edit_singleline(&mut ed.map.author);
                ui.end_row();
            });
            ui.label(
                RichText::new(format!(
                    "{}×{} · {} {}",
                    ed.map.grid.width,
                    ed.map.grid.height,
                    ed.players(),
                    crate::i18n::tr("players")
                ))
                .weak(),
            );
            ui.separator();
            let tools = [
                (Tool::Terrain, "Terrain"),
                (Tool::Raise, "Raise"),
                (Tool::Lower, "Lower"),
                (Tool::Flatten, "Flatten"),
                (Tool::Smooth, "Smooth"),
                (Tool::Trees, "Trees"),
                (Tool::Stones, "Stones"),
                (Tool::Clear, "Clear"),
                (Tool::Decoration, "Decoration"),
                (Tool::Animals, "Animals"),
                (Tool::Resource, "Resources"),
                (Tool::Headquarters, "Headquarters"),
                (Tool::Harbor, "Harbor spot"),
            ];
            egui::Grid::new("ed_tools").show(ui, |ui| {
                for (i, (t, n)) in tools.iter().enumerate() {
                    ui.selectable_value(&mut ed.tool, *t, crate::i18n::tr(n));
                    if i % 3 == 2 {
                        ui.end_row();
                    }
                }
            });
            ui.add(egui::Slider::new(&mut ed.brush, 0..=6).text(crate::i18n::tr("brush")));
            match ed.tool {
                Tool::Terrain => {
                    ui.checkbox(&mut ed.tri, crate::i18n::tr("One triangle at a time"));
                    egui::Grid::new("ed_terrain").show(ui, |ui| {
                        for (i, (id, n)) in terrains(ed.map.landscape).iter().enumerate() {
                            let c = rr_world::terrain::color(ed.map.landscape, *id);
                            let col = Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8);
                            ui.horizontal(|ui| {
                                let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), egui::Sense::hover());
                                ui.painter().rect_filled(r, 2.0, col);
                                ui.selectable_value(&mut ed.terrain, *id, crate::i18n::tr(n));
                            });
                            if i % 3 == 2 {
                                ui.end_row();
                            }
                        }
                    });
                }
                Tool::Trees | Tool::Stones => {
                    if ed.tool == Tool::Trees {
                        ui.horizontal_wrapped(|ui| {
                            ui.selectable_value(&mut ed.tree_kind, None, crate::i18n::tr("mixed"));
                            for k in 0..9u8 {
                                ui.selectable_value(&mut ed.tree_kind, Some(k), format!("{}", k + 1));
                            }
                        });
                    } else {
                        ui.add(egui::Slider::new(&mut ed.stone_size, 1..=6).text(crate::i18n::tr("pile size")));
                    }
                    ui.add(egui::Slider::new(&mut ed.density, 1..=4).text(crate::i18n::tr("density")));
                }
                Tool::Decoration => {
                    ui.horizontal_wrapped(|ui| {
                        for (id, n) in DECORATIONS {
                            ui.selectable_value(&mut ed.decor, *id, crate::i18n::tr(n));
                        }
                    });
                    ui.add(egui::Slider::new(&mut ed.density, 1..=4).text(crate::i18n::tr("density")));
                }
                Tool::Animals => {
                    ui.horizontal_wrapped(|ui| {
                        ui.selectable_value(&mut ed.animal, None, crate::i18n::tr("mixed"));
                        for (a, n) in [
                            (Animal::Rabbit, "Rabbit"),
                            (Animal::Fox, "Fox"),
                            (Animal::Stag, "Stag"),
                            (Animal::Deer, "Deer"),
                            (Animal::Duck, "Duck"),
                            (Animal::Sheep, "Sheep"),
                            (Animal::PolarBear, "Polar bear"),
                        ] {
                            ui.selectable_value(&mut ed.animal, Some(a), crate::i18n::tr(n));
                        }
                    });
                }
                Tool::Resource => {
                    ui.horizontal_wrapped(|ui| {
                        for (k, n) in RESOURCES {
                            ui.selectable_value(&mut ed.resource, *k, crate::i18n::tr(n));
                        }
                    });
                    ui.add(egui::Slider::new(&mut ed.amount, 1..=7).text(crate::i18n::tr("amount")));
                }
                Tool::Headquarters => {
                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::tr("Player"));
                        for s in 0..7 {
                            let set = ed.map.hq.get(s).copied().flatten().is_some();
                            let label = RichText::new(format!("{}", s + 1)).color(if set {
                                crate::render::player_color(s as u8)
                            } else {
                                Color32::GRAY
                            });
                            ui.selectable_value(&mut ed.slot, s, label);
                        }
                    });
                    ui.label(RichText::new(crate::i18n::tr("Click to set, click again to remove.")).small());
                }
                _ => {}
            }
            ui.horizontal(|ui| {
                ui.checkbox(&mut ed.show_res, crate::i18n::tr("Show resources"));
                ui.checkbox(&mut ed.show_scenery, crate::i18n::tr("Show the game's scenery")).on_hover_text(
                    crate::i18n::tr("Bushes, stumps and boulders the game draws; they aren't in the map"),
                );
                let space = crate::keys::keyed(
                    crate::i18n::tr("Where buildings fit (Space)"),
                    egui::Key::Space,
                    app.keys.key(crate::keys::Act::BuildHelp),
                );
                ui.checkbox(&mut app.show_bq, space);
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(crate::i18n::tr("💾 Save")).on_hover_text("F5, Ctrl+S").clicked() {
                    ed.save_with_message();
                }
                if ui
                    .button(crate::i18n::tr("▶ Try it"))
                    .on_hover_text(crate::i18n::tr("Save, then play it against computer players"))
                    .clicked()
                {
                    // check first, so a map that can't be played isn't saved over a good one
                    let checked = if ed.players() >= 1 { ed.check() } else { Ok(()) };
                    match checked.and_then(|_| ed.save()) {
                        Ok(p) if ed.players() >= 1 => test = Some(p),
                        Ok(_) => ed.message = crate::i18n::tr("Set at least one headquarters first.").into(),
                        Err(e) => ed.message = e,
                    }
                }
                if ui
                    .button(crate::i18n::tr("Export .swd"))
                    .on_hover_text(crate::i18n::tr(
                        "The original's format, for Return to the Roots (fish amounts are lost)",
                    ))
                    .clicked()
                {
                    ed.message = match ed.export_swd() {
                        Ok(p) => format!("{} {}", crate::i18n::tr("Saved to"), p.display()),
                        Err(e) => e,
                    };
                }
                if ui
                    .add_enabled(!ed.undo.is_empty(), egui::Button::new(crate::i18n::tr("⟲ Undo")))
                    .on_hover_text("Ctrl+Z")
                    .clicked()
                {
                    ed.undo();
                }
                if ui
                    .add_enabled(!ed.redo.is_empty(), egui::Button::new(crate::i18n::tr("⟳ Redo")))
                    .on_hover_text("Ctrl+Y")
                    .clicked()
                {
                    ed.redo();
                }
                if ui.button(crate::i18n::tr("🚪 Menu")).clicked() {
                    if ed.dirty() {
                        ed.confirm = Some(Confirm::Leave);
                    } else {
                        leave = true;
                    }
                }
            });
            ui.collapsing(crate::i18n::tr("New map"), |ui| {
                ui.add(egui::Slider::new(&mut ed.new_size, 32..=256).step_by(16.0).text(crate::i18n::tr("map size")));
                ui.horizontal(|ui| {
                    for (l, n) in [
                        (Landscape::Greenland, "Greenland"),
                        (Landscape::Wasteland, "Wasteland"),
                        (Landscape::Winter, "Winter world"),
                    ] {
                        ui.selectable_value(&mut ed.new_landscape, l, crate::i18n::tr(n));
                    }
                });
                if ui.button(crate::i18n::tr("Start a new map")).clicked() {
                    let m = Editor::blank(ed.new_size, ed.new_landscape);
                    if ed.dirty() {
                        ed.confirm = Some(Confirm::New(Box::new(m)));
                    } else {
                        new_map = Some((m, None));
                    }
                }
            });
            ui.collapsing(crate::i18n::tr("Mission"), |ui| {
                ui.label(RichText::new(crate::i18n::tr("Shown before a game on this map; saved next to it.")).small());
                let m = &mut ed.mission;
                egui::Grid::new("ed_mission").num_columns(2).show(ui, |ui| {
                    ui.label(crate::i18n::tr("Title"));
                    ui.text_edit_singleline(&mut m.title);
                    ui.end_row();
                    ui.label(crate::i18n::tr("Goal"));
                    ui.text_edit_singleline(&mut m.goal);
                    ui.end_row();
                });
                ui.label(crate::i18n::tr("Briefing"));
                ui.add(egui::TextEdit::multiline(&mut m.briefing).desired_rows(3).desired_width(300.0));
                ui.add(egui::Slider::new(&mut m.truce_minutes, 0..=60).text(crate::i18n::tr("minutes of peace")));
                mission_goal(ui, m);
                // (the field keeps its own text: rebuilt from the names each frame, "Aster,B"
                // came back as "Aster, B" and the cursor typed into the wrong name; map maker
                // round 1, #10)
                let names = ed.names_text.get_or_insert_with(|| m.names.join(", "));
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("Player names"));
                    if ui.text_edit_singleline(names).changed() {
                        m.names = parse_names(names);
                    }
                });
            });
            let open_list = ui.collapsing(crate::i18n::tr("Open a map"), |ui| {
                let files = ed.files.get_or_insert_with(crate::menu::find_maps).clone();
                if files.is_empty() {
                    ui.label(crate::i18n::tr("No map files yet."));
                }
                egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                    for (p, label) in files {
                        if ui.selectable_label(ed.file.as_ref() == Some(&p), label).clicked() {
                            if ed.dirty() {
                                ed.confirm = Some(Confirm::Open(p));
                            } else {
                                open = Some(p);
                            }
                        }
                    }
                });
            });
            // look again each time the list is opened
            if open_list.fully_closed() {
                ed.files = None;
            }
            if ed.confirm.is_some() {
                ui.label(
                    RichText::new(crate::i18n::tr("The changes since the last save will be lost."))
                        .color(egui::Color32::from_rgb(255, 200, 90)),
                );
                ui.horizontal(|ui| {
                    if ui.button(crate::i18n::tr("Discard them")).clicked() {
                        match ed.confirm.take() {
                            Some(Confirm::New(m)) => new_map = Some((*m, None)),
                            Some(Confirm::Open(p)) => open = Some(p),
                            Some(Confirm::Leave) => leave = true,
                            None => {}
                        }
                    }
                    if ui.button(crate::i18n::tr("Cancel")).clicked() {
                        ed.confirm = None;
                    }
                });
            }
            if !ed.message.is_empty() {
                ui.label(RichText::new(&ed.message).color(egui::Color32::from_rgb(255, 200, 90)));
            }
        });
    if let Some(p) = open {
        match std::fs::read(&p).map_err(|e| e.to_string()).and_then(|b| rr_formats::rrmap::read_any(&b)) {
            Ok(m) => {
                let ours = p.extension().is_some_and(|e| e == "rrmap");
                new_map = Some((m, ours.then_some(p)));
            }
            Err(e) => ed_message(app, format!("{}: {e}", p.display())),
        }
    }
    if let Some((m, file)) = new_map {
        let mut ed = Editor::new(m);
        if let Some(f) = file.as_deref() {
            ed.open_mission(f);
        }
        ed.file = file;
        app.editor = Some(ed);
        app.tex = None;
        app.minimap = None;
    }
    if leave {
        app.editor = None;
        app.in_menu = true;
        // maps saved meanwhile show up in the menu's list
        app.menu.maps = None;
    }
    if let Some(p) = test {
        let ed = app.editor.take();
        let players = rr_formats::rrmap::read_any(&std::fs::read(&p).unwrap_or_default()).map_or(2, |m| m.players());
        app.menu.map_path = p.to_string_lossy().to_string();
        // as the menu would start it: its level, seed and goods, and the mission's human slot
        // (QA round 8, #7: easy, seed 0 and slot 1 whatever the mission said)
        let me = ed.as_ref().and_then(|e| e.mission.human).unwrap_or(0).min(players.saturating_sub(1) as u8);
        app.start_game(crate::Args {
            map: Some(p.to_string_lossy().to_string()),
            players,
            me,
            seed: app.menu.seed,
            ai_level: app.menu.ai_level,
            goods: app.menu.goods,
            ..Default::default()
        });
        // the title menu offers the way back
        app.tested = ed.map(Box::new);
    }
}

/// Markers for what the map holds beyond terrain: headquarters and harbor spots.
pub fn overlay(app: &App, painter: &egui::Painter, view: &crate::render::View) {
    let Some(ed) = app.editor.as_ref() else { return };
    let z = app.cam.zoom;
    for (s, hq) in ed.map.hq.iter().enumerate() {
        if let Some(p) = hq {
            let c = view.node_screen(*p);
            let col = crate::render::player_color(s as u8);
            painter.rect_filled(
                egui::Rect::from_center_size(c - Vec2::new(0.0, 14.0 * z), Vec2::splat(18.0 * z)),
                3.0,
                col,
            );
            painter.text(
                c - Vec2::new(0.0, 14.0 * z),
                egui::Align2::CENTER_CENTER,
                format!("{}", s + 1),
                egui::FontId::proportional(13.0 * z),
                Color32::WHITE,
            );
        }
    }
    for h in &ed.map.harbors {
        let c = view.node_screen(*h);
        painter.circle_stroke(c, 7.0 * z, egui::Stroke::new(2.0_f32, Color32::from_rgb(120, 200, 255)));
        painter.text(c, egui::Align2::CENTER_CENTER, "⚓", egui::FontId::proportional(11.0 * z), Color32::WHITE);
    }
    // building room as any player would have it on their own land, and the ground's riches
    let g = ed.map.grid;
    let clip = painter.clip_rect().expand(20.0);
    let world = view.sim.world();
    for p in g.coords() {
        let c = view.node_screen(p);
        if !clip.contains(c) {
            continue;
        }
        if app.show_bq {
            crate::sprites::bq_icon(painter, c, z, world.bq(p));
        }
        if ed.show_res
            && let Some((col, amount)) = resource_mark(ed.map.resources[g.index(p)])
        {
            let r = (1.5 + amount as f32 * 0.5) * z;
            painter.circle_filled(c + Vec2::new(0.0, 4.0 * z), r + 1.0, Color32::BLACK);
            painter.circle_filled(c + Vec2::new(0.0, 4.0 * z), r, col);
        }
    }
    // what is under the pointer
    if let Some(pos) = painter.ctx().pointer_hover_pos()
        && painter.clip_rect().contains(pos)
    {
        let p = view.pick(pos);
        let i = g.index(p);
        // the brush: its rim
        if !matches!(ed.tool, Tool::Headquarters | Tool::Harbor) {
            for q in g.ring(p, ed.brush) {
                let c = view.node_screen(q);
                painter.circle_stroke(c, 2.5 * z, egui::Stroke::new(1.5_f32, Color32::from_white_alpha(200)));
            }
            painter.circle_stroke(view.node_screen(p), 3.0 * z, egui::Stroke::new(1.5_f32, Color32::WHITE));
        }
        let terrain =
            |t: u8| terrains(ed.map.landscape).iter().find(|(k, _)| *k == t).map_or("?", |(_, n)| crate::i18n::tr(n));
        let res = match ed.map.resources[i] {
            Resource::None => String::new(),
            r => {
                let (k, a) = crate::ui::resource_key(r);
                match a {
                    Some(a) => format!(" · {} {a}", crate::i18n::tr(k)),
                    None => format!(" · {}", crate::i18n::tr(k)),
                }
            }
        };
        let text = format!(
            "{},{} · {} {} · {}{res}",
            p.x,
            p.y,
            crate::i18n::tr("height"),
            ed.map.altitude[i],
            terrain(ed.map.terrain_rsu[i]),
        );
        let at = pos + Vec2::new(16.0, 18.0);
        let galley = painter.layout_no_wrap(text, egui::FontId::proportional(13.0), Color32::WHITE);
        painter.rect_filled(
            egui::Rect::from_min_size(at, galley.size()).expand(3.0),
            3.0,
            Color32::from_black_alpha(190),
        );
        painter.galley(at, galley, Color32::WHITE);
    }
}

/// How a resource is marked on the editor's map: colour and amount (None: nothing to show).
fn resource_mark(r: Resource) -> Option<(Color32, u8)> {
    Some(match r {
        Resource::Coal(a) => (Color32::from_rgb(40, 40, 40), a),
        Resource::Iron(a) => (Color32::from_rgb(190, 90, 50), a),
        Resource::Gold(a) => (Color32::from_rgb(250, 210, 40), a),
        Resource::Granite(a) => (Color32::from_rgb(170, 170, 180), a),
        Resource::Fish(a) => (Color32::from_rgb(80, 160, 240), a),
        Resource::Water | Resource::None => return None,
    })
}

fn ed_message(app: &mut App, text: String) {
    if let Some(ed) = app.editor.as_mut() {
        ed.message = text;
    }
}

/// The terrain triangle under a screen point: its node and whether it is the one pointing up.
pub fn triangle_at(view: &crate::render::View, pos: egui::Pos2) -> Option<(MapCoord, bool)> {
    use rr_core::Direction::{E, SE, SW};
    let g = view.sim.world().grid();
    let inside = |a: egui::Pos2, b: egui::Pos2, c: egui::Pos2| {
        let s = |p: egui::Pos2, q: egui::Pos2, r: egui::Pos2| (p.x - r.x) * (q.y - r.y) - (q.x - r.x) * (p.y - r.y);
        let (d1, d2, d3) = (s(pos, a, b), s(pos, b, c), s(pos, c, a));
        !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0))
    };
    let near = view.pick(pos);
    // the triangles of the nodes around (a node's triangles lie below it, so look above too)
    for q in g.within(near, 2) {
        let p = view.node_screen(q);
        let (sw, se, e) = (
            view.node_screen(g.neighbor(q, SW)),
            view.node_screen(g.neighbor(q, SE)),
            view.node_screen(g.neighbor(q, E)),
        );
        if inside(p, sw, se) {
            return Some((q, true));
        }
        if inside(p, e, se) {
            return Some((q, false));
        }
    }
    None
}

/// Is a node part of a brush stroke at this density (1 a quarter of the nodes, 4 all)? A fixed
/// pattern, so going over the same spot again doesn't fill it up.
fn dense(q: MapCoord, density: u8) -> bool {
    let h = (q.x as u32).wrapping_mul(7) ^ (q.y as u32).wrapping_mul(13) ^ ((q.x as u32 * q.y as u32) >> 1);
    h % 4 < density.clamp(1, 4) as u32
}

/// What keeps a map from being played, or makes it play badly (empty when it's fine).
pub fn map_problems(map: &MapData) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(e) = crate::setup_problem(map) {
        out.push(e);
    }
    let world = rr_world::World::new(map);
    let around = |p: MapCoord| world.terrains_around(p);
    let coast = |p: MapCoord| {
        let t = around(p);
        t.iter().any(|t| rr_world::terrain::is_water(*t)) && !t.iter().all(|t| rr_world::terrain::is_water(*t))
    };
    let inland = map.harbors.iter().filter(|h| !coast(**h)).count();
    if inland > 0 {
        out.push(format!("{} {inland}", crate::i18n::tr("Harbor spots not on the coast:")));
    }
    let g = map.grid;
    let wet = g
        .coords()
        .filter(|p| {
            matches!(map.objects[g.index(*p)], MapObject::Tree(_) | MapObject::Granite { .. })
                && around(*p).iter().all(|t| rr_world::terrain::is_water(*t))
        })
        .count();
    if wet > 0 {
        out.push(format!("{} {wet}", crate::i18n::tr("Trees or stones in water:")));
    }
    out.extend(barren_problems(map, &world));
    out
}

/// Where nobody can grow (map maker round 1, #7: a map with no trees or stones by the castles,
/// its harbors on a pond, passed the check; both AIs ran out of stone by minute 33).
/// The few-trees warning, singular for one tree (German round 1, #4: "nur 1 Bäume").
fn trees_template(trees: usize) -> &'static str {
    if trees == 1 {
        "Player {1}: only 1 tree near the castle: little wood to cut."
    } else {
        "Player {1}: only {2} trees near the castle: little wood to cut."
    }
}

fn barren_problems(map: &MapData, world: &rr_world::World) -> Vec<String> {
    let g = map.grid;
    let mut out = Vec::new();
    for (i, hq) in map.hq.iter().enumerate() {
        let Some(hq) = *hq else { continue };
        let near = |f: fn(&MapObject) -> bool| g.within(hq, 12).filter(|q| f(&map.objects[g.index(*q)])).count();
        let trees = near(|o| matches!(o, MapObject::Tree(_)));
        let stones = near(|o| matches!(o, MapObject::Granite { .. }));
        if trees < 6 {
            out.push(crate::i18n::show(&crate::i18n::msg(
                trees_template(trees),
                &[&(i + 1).to_string(), &trees.to_string()],
            )));
        }
        if stones == 0 {
            out.push(crate::i18n::show(&crate::i18n::msg(
                "Player {1}: no stones near the castle: no stone to build with.",
                &[&(i + 1).to_string()],
            )));
        }
    }
    // the water bodies, and the harbor spots by each: a spot alone on its water sends no ship
    let water = |p: MapCoord| world.terrains_around(p).iter().all(|t| rr_world::terrain::is_water(*t));
    let mut body = vec![0u32; g.len()];
    let mut next = 0;
    for p in g.coords() {
        if body[g.index(p)] != 0 || !water(p) {
            continue;
        }
        next += 1;
        let mut todo = vec![p];
        body[g.index(p)] = next;
        while let Some(q) = todo.pop() {
            for n in g.neighbors(q) {
                if body[g.index(n)] == 0 && water(n) {
                    body[g.index(n)] = next;
                    todo.push(n);
                }
            }
        }
    }
    let bodies_of = |h: MapCoord| -> Vec<u32> {
        let mut b: Vec<u32> = g.within(h, 2).map(|q| body[g.index(q)]).filter(|b| *b != 0).collect();
        b.sort_unstable();
        b.dedup();
        b
    };
    for h in &map.harbors {
        let mine = bodies_of(*h);
        let shared = map.harbors.iter().any(|o| o != h && bodies_of(*o).iter().any(|b| mine.contains(b)));
        if !shared {
            out.push(crate::i18n::show(&crate::i18n::msg(
                "The harbor spot at {1},{2} has no other harbor spot on its water: no ship can sail anywhere from it.",
                &[&h.x.to_string(), &h.y.to_string()],
            )));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    /// German round 1, #4: one tree is "1 Baum", not "1 Bäume".
    #[test]
    fn one_tree_is_singular() {
        assert!(super::trees_template(1).contains("only 1 tree near"));
        assert!(super::trees_template(3).contains("{2} trees"));
        assert!(crate::i18n::german(super::trees_template(1)).unwrap().contains("nur 1 Baum "));
    }

    use super::{Editor, Tool};
    use rr_core::{Landscape, MapCoord, MapObject};

    /// Map-maker roast #11: tree kind, density, pile size and animal kind are the author's.
    #[test]
    fn objects_as_chosen() {
        let mut ed = Editor::new(Editor::blank(40, Landscape::Greenland));
        let g = ed.map.grid;
        let c = MapCoord::new(20, 20);
        let count = |ed: &Editor| g.within(c, 4).filter(|q| ed.map.objects[g.index(*q)] != MapObject::None).count();
        ed.tool = Tool::Trees;
        ed.brush = 4;
        ed.tree_kind = Some(7);
        ed.density = 1;
        ed.apply(c);
        let sparse = count(&ed);
        assert!(g.within(c, 4).all(|q| matches!(ed.map.objects[g.index(q)], MapObject::None | MapObject::Tree(7))));
        ed.density = 4;
        ed.apply(c);
        assert!(count(&ed) > sparse * 2, "{sparse} {}", count(&ed));
        ed.tool = Tool::Clear;
        ed.apply(c);
        ed.tool = Tool::Stones;
        ed.stone_size = 2;
        ed.apply(c);
        assert!(ed.map.objects.iter().any(|o| *o == MapObject::Granite { kind: 0, size: 2 } || *o == MapObject::Granite { kind: 1, size: 2 }));
        ed.tool = Tool::Animals;
        ed.animal = Some(rr_core::Animal::Fox);
        ed.apply(c);
        assert!(ed.map.animals.iter().flatten().all(|a| *a == rr_core::Animal::Fox));
        assert!(ed.map.animals.iter().flatten().count() > 0);
    }

    /// Map-maker roast #11: decorations can be placed, and only rocks block.
    #[test]
    fn decorations_are_placed() {
        let mut ed = Editor::new(Editor::blank(40, Landscape::Greenland));
        ed.tool = Tool::Decoration;
        ed.brush = 3;
        ed.decor = 0x16;
        ed.apply(MapCoord::new(20, 20));
        assert!(ed.map.objects.contains(&MapObject::Decoration(0x16)));
        let w = rr_world::World::new(&ed.map);
        let rock = ed.map.grid.coords().find(|p| ed.map.objects[ed.map.grid.index(*p)] != MapObject::None).unwrap();
        assert_ne!(w.bq(rock), rr_world::Bq::Castle);
    }

    /// Map-maker roast #10: raising makes a hill with slopes, Flatten keeps the stroke's
    /// height, Smooth evens a cliff.
    #[test]
    fn height_tools_shape_hills() {
        let mut ed = Editor::new(Editor::blank(40, Landscape::Greenland));
        let g = ed.map.grid;
        let c = MapCoord::new(20, 20);
        ed.tool = Tool::Raise;
        ed.brush = 4;
        for _ in 0..6 {
            ed.checkpoint();
            ed.apply(c);
        }
        let h = |ed: &Editor, q: MapCoord| ed.map.altitude[g.index(q)] as i32;
        assert_eq!(h(&ed, c), 16);
        // no step steeper than one between neighbours
        for q in g.within(c, 12) {
            for n in g.neighbors(q) {
                assert!((h(&ed, q) - h(&ed, n)).abs() <= 1, "{q:?} {n:?}");
            }
        }
        // Flatten levels to where the stroke began, even when dragged over the hill
        ed.tool = Tool::Flatten;
        ed.brush = 0;
        ed.checkpoint();
        let low = MapCoord::new(30, 20);
        ed.apply(low);
        ed.apply(c);
        assert_eq!(h(&ed, c), 10);
        // Smooth takes the edge off a cliff
        let mut ed = Editor::new(Editor::blank(40, Landscape::Greenland));
        let q = MapCoord::new(20, 20);
        ed.map.altitude[g.index(q)] = 30;
        ed.tool = Tool::Smooth;
        ed.brush = 0;
        ed.apply(q);
        assert!(h(&ed, q) < 30 && h(&ed, q) > 10);
    }

    /// Map-maker roast #9: every landscape names its own terrains, all of them, in German too.
    #[test]
    fn every_landscape_has_its_terrain_names() {
        for l in [Landscape::Greenland, Landscape::Wasteland, Landscape::Winter] {
            let t = super::terrains(l);
            assert!(t.len() >= 23, "{l:?}");
            for (_, n) in t {
                assert!(crate::i18n::has_german(n), "{n}");
            }
        }
        // the same id is lava in the wasteland and snow on greenland
        let name = |l, id| super::terrains(l).iter().find(|(k, _)| *k == id).map(|(_, n)| *n);
        assert_eq!(name(Landscape::Greenland, 2), Some("Snow"));
        assert_eq!(name(Landscape::Wasteland, 2), Some("Lava with few stones"));
    }

    /// Map-maker roast #4, #5: redo, and what counts as unsaved.
    /// Map maker round 1, #5: saving closes the gaps between headquarters slots; the mission's
    /// names move with them, and the editor says so.
    #[test]
    fn saving_renumbers_names_with_their_slots() {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("rr-editor-{nanos}"));
        let mut ed = Editor::new(rr_core::MapData::flat(32, 32, 8));
        ed.map.hq = vec![Some(rr_core::MapCoord::new(8, 8)), None, Some(rr_core::MapCoord::new(20, 20))];
        ed.name = "Gaps".into();
        ed.mission.names = vec!["Aster".into(), "Brann".into(), "Carl".into()];
        ed.mission.objective = Some(crate::missions::Objective::DefeatRival(2));
        ed.mission.human = Some(2);
        ed.mission.title = "T".into();
        ed.save_in(&dir).expect("saved");
        let back = crate::missions::MapMission::read(&dir.join("Gaps.rrmap")).unwrap().unwrap();
        assert_eq!(back.names, ["Aster", "Carl"], "Carl's slot 3 became slot 2");
        // QA round 8, #2: and the goal and the human slot move with him
        assert_eq!((back.objective, back.human), (Some(crate::missions::Objective::DefeatRival(1)), Some(1)));
        assert!(ed.save_note.as_deref().is_some_and(|n| n.contains('3') && n.contains('2')), "{:?}", ed.save_note);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// QA round 8, #12: a mission edit is an unsaved change.
    #[test]
    fn a_mission_edit_is_unsaved() {
        let mut ed = Editor::new(rr_core::MapData::flat(32, 32, 8));
        assert!(!ed.dirty());
        ed.mission.title = "Lakeside".into();
        assert!(ed.dirty());
    }

    /// Map maker round 1, #10: names typed with plain commas stay as typed.
    #[test]
    fn names_with_plain_commas() {
        use super::parse_names;
        assert_eq!(parse_names("Aster,Brann,Carl"), ["Aster", "Brann", "Carl"]);
        assert_eq!(parse_names(" Aster , Brann"), ["Aster", "Brann"]);
        assert!(parse_names(" , ").is_empty());
    }

    /// Map maker round 1, #3: a mission file the editor couldn't read is left alone on Save
    /// (it was deleted: opening a map with a typo in its mission and saving lost the mission).
    #[test]
    fn save_leaves_an_unreadable_mission_alone() {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("rr-editor-{nanos}"));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let mut ed = Editor::new(rr_core::MapData::flat(32, 32, 8));
        ed.map.hq = vec![Some(rr_core::MapCoord::new(10, 10))];
        ed.name = "Lakeside".into();
        let map = dir.join("Lakeside.rrmap");
        let mission = crate::missions::MapMission::path_for(&map);
        let broken = "{\"title\": \"Lake\",}";
        std::fs::write(&mission, broken).unwrap();
        ed.open_mission(&map);
        assert!(ed.mission_unreadable.is_some());
        ed.save_in(&dir).expect("saved");
        assert_eq!(std::fs::read_to_string(&mission).unwrap(), broken, "the mission file is untouched");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn undo_redo_and_unsaved_changes() {
        let mut ed = Editor::new(Editor::blank(32, Landscape::Greenland));
        assert!(!ed.dirty());
        let p = MapCoord::new(10, 10);
        let g = ed.map.grid;
        ed.tool = Tool::Raise;
        for _ in 0..3 {
            ed.checkpoint();
            ed.apply(p);
        }
        assert!(ed.dirty());
        ed.undo();
        ed.undo();
        assert_eq!(ed.map.altitude[g.index(p)], 11);
        ed.redo();
        assert_eq!(ed.map.altitude[g.index(p)], 12);
        // a new stroke drops what was undone
        ed.checkpoint();
        ed.apply(p);
        assert!(ed.redo.is_empty());
        ed.undo();
        ed.undo();
        ed.undo();
        assert!(!ed.dirty());
    }

    /// Map maker round 1, #7: a map where nobody can grow is named: no trees or stones by a
    /// headquarters, a harbor spot alone on its water (both AIs ran out of stone by minute 33).
    #[test]
    fn barren_maps_are_named() {
        let mut m = rr_core::MapData::flat(48, 48, 8);
        m.hq = vec![Some(MapCoord::new(12, 12)), Some(MapCoord::new(36, 36))];
        let problems = super::map_problems(&m);
        assert!(problems.iter().any(|p| p.contains("trees") && p.contains('1')), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("stones") && p.contains('2')), "{problems:?}");
        // trees and stones by both: no such warning
        let g = m.grid;
        for hq in [MapCoord::new(12, 12), MapCoord::new(36, 36)] {
            for (k, q) in g.ring(hq, 6).enumerate() {
                m.objects[g.index(q)] =
                    if k % 3 == 0 { MapObject::Granite { kind: 0, size: 4 } } else { MapObject::Tree(1) };
            }
        }
        let problems = super::map_problems(&m);
        assert!(!problems.iter().any(|p| p.contains("trees") || p.contains("stones")), "{problems:?}");
        // a pond with one harbor spot
        for q in g.within(MapCoord::new(24, 8), 2) {
            m.terrain_rsu[g.index(q)] = 5;
            m.terrain_usd[g.index(q)] = 5;
        }
        m.harbors = vec![MapCoord::new(24, 11)];
        let problems = super::map_problems(&m);
        assert!(problems.iter().any(|p| p.contains("no other harbor")), "{problems:?}");
    }

    /// Map-maker roast #3, #7: unplayable maps are named, and tools respect the terrain.
    #[test]
    fn broken_maps_are_named_and_water_stays_clear() {
        let mut ed = Editor::new(Editor::blank(40, Landscape::Greenland));
        let g = ed.map.grid;
        assert!(super::map_problems(&ed.map)[0].contains("headquarters"));
        // a lake around (10, 10)
        ed.tool = Tool::Terrain;
        ed.terrain = 5;
        ed.brush = 3;
        ed.apply(MapCoord::new(10, 10));
        for tool in [Tool::Trees, Tool::Stones] {
            ed.tool = tool;
            ed.brush = 1;
            ed.apply(MapCoord::new(10, 10));
        }
        assert!(g.within(MapCoord::new(10, 10), 1).all(|q| ed.map.objects[g.index(q)] == MapObject::None));
        // coal only in mountains
        ed.tool = Tool::Resource;
        ed.resource = 1;
        ed.apply(MapCoord::new(25, 25));
        assert_eq!(ed.map.resources[g.index(MapCoord::new(25, 25))], rr_core::Resource::None);
        // no harbor spot inland, one on the shore
        ed.tool = Tool::Harbor;
        ed.apply(MapCoord::new(30, 30));
        assert!(ed.map.harbors.is_empty());
        ed.apply(MapCoord::new(14, 10));
        assert_eq!(ed.map.harbors.len(), 1, "{}", ed.message);
        // a headquarters in the lake: no game
        ed.tool = Tool::Headquarters;
        ed.slot = 0;
        ed.apply(MapCoord::new(10, 10));
        assert!(crate::setup_problem(&ed.map).is_some());
        // on dry land it's fine
        ed.apply(MapCoord::new(10, 10));
        ed.apply(MapCoord::new(28, 28));
        assert_eq!(crate::setup_problem(&ed.map), None);
    }

    #[test]
    fn tools_change_the_map() {
        let mut ed = Editor::new(Editor::blank(32, Landscape::Greenland));
        let p = MapCoord::new(10, 10);
        let g = ed.map.grid;
        ed.tool = Tool::Terrain;
        ed.terrain = 5;
        ed.brush = 1;
        ed.apply(p);
        assert!(g.within(p, 1).all(|q| ed.map.terrain_rsu[g.index(q)] == 5));
        ed.tool = Tool::Raise;
        ed.apply(p);
        assert_eq!(ed.map.altitude[g.index(p)], 11);
        ed.tool = Tool::Stones;
        ed.brush = 3;
        ed.apply(MapCoord::new(20, 20));
        assert!(ed.map.objects.iter().any(|o| matches!(o, MapObject::Granite { .. })));
        ed.tool = Tool::Clear;
        ed.apply(MapCoord::new(20, 20));
        assert!(ed.map.objects.iter().all(|o| *o == MapObject::None));
        // headquarters: set on an even row, a second click removes it
        ed.tool = Tool::Headquarters;
        ed.slot = 2;
        ed.apply(MapCoord::new(5, 7));
        assert_eq!(ed.map.hq[2], Some(MapCoord::new(5, 6)));
        assert_eq!(ed.players(), 1);
        ed.apply(MapCoord::new(5, 7));
        assert_eq!(ed.players(), 0);
        // a headquarters in the water can't start a game
        ed.apply(MapCoord::new(10, 10));
        assert!(ed.check().is_err());
        ed.apply(MapCoord::new(10, 10));
        ed.apply(MapCoord::new(24, 4));
        assert!(ed.check().is_ok());
        // undo goes back one checkpoint
        let before = ed.map.clone();
        ed.checkpoint();
        ed.tool = Tool::Terrain;
        ed.terrain = 2;
        ed.apply(MapCoord::new(3, 3));
        assert_ne!(ed.map, before);
        ed.undo();
        assert_eq!(ed.map, before);
    }
}
