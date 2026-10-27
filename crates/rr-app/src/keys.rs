//! The keys a player can change (a11y gap: every key was fixed). The table holds each action's
//! default; the settings keep only the keys that differ from it.

use eframe::egui::Key;

/// What a key does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    ToCastle,
    ToAlert,
    Inventory,
    Map,
    Main,
    Messages,
    Help,
    Names,
    Zoom,
    BuildHelp,
    Productivity,
    Pause,
    Fullscreen,
    QuickSave,
    QuickLoad,
    Faster,
    Slower,
}

/// Every action: its name in the settings file, its label (the help window's text) and its
/// default key (RTTR's where it has one).
pub const ACTS: [(Act, &str, &str, Key); 17] = [
    (Act::ToCastle, "to_castle", "To the castle", Key::H),
    (Act::ToAlert, "to_alert", "To the latest alert", Key::G),
    (Act::Inventory, "inventory", "Inventory", Key::I),
    (Act::Map, "map", "Map", Key::L),
    (Act::Main, "main", "Main selection", Key::M),
    (Act::Messages, "messages", "Messages", Key::N),
    (Act::Help, "help", "Help", Key::F1),
    (Act::Names, "names", "Building names", Key::C),
    (Act::Zoom, "zoom", "Zoom", Key::Z),
    (Act::BuildHelp, "build_help", "Building help", Key::Space),
    (Act::Productivity, "productivity", "Productivity and crowded flags", Key::O),
    (Act::Pause, "pause", "Pause", Key::P),
    (Act::Fullscreen, "fullscreen", "Fullscreen", Key::F11),
    (Act::QuickSave, "quick_save", "Quick save", Key::F5),
    (Act::QuickLoad, "quick_load", "Quick load", Key::F9),
    (Act::Faster, "faster", "Faster", Key::Plus),
    (Act::Slower, "slower", "Slower", Key::Minus),
];

/// Keys no action may take: Escape cancels, the arrows and WASD scroll, Enter, Backspace, Home,
/// End, PageUp and PageDown work the map cursor, Tab moves the focus, and 1-9, F and T work the
/// action window (a11y round 3, #2: Tab and F were accepted, and did two things at once).
pub fn reserved(k: Key) -> bool {
    matches!(
        k,
        Key::Escape
            | Key::Enter
            | Key::Backspace
            | Key::Tab
            | Key::Home
            | Key::End
            | Key::PageUp
            | Key::PageDown
            | Key::F
            | Key::T
            | Key::Num1
            | Key::Num2
            | Key::Num3
            | Key::Num4
            | Key::Num5
            | Key::Num6
            | Key::Num7
            | Key::Num8
            | Key::Num9
            | Key::ArrowUp
            | Key::ArrowDown
            | Key::ArrowLeft
            | Key::ArrowRight
            | Key::W
            | Key::A
            | Key::S
            | Key::D
    )
}

#[derive(Clone, Debug, PartialEq)]
pub struct Keys {
    keys: Vec<(Act, Key)>,
}

impl Default for Keys {
    fn default() -> Self {
        Keys { keys: ACTS.iter().map(|(a, _, _, k)| (*a, *k)).collect() }
    }
}

impl Keys {
    pub fn key(&self, a: Act) -> Key {
        self.keys.iter().find(|(x, _)| *x == a).map_or(Key::Escape, |(_, k)| *k)
    }

    /// The action bound to `k`, if any.
    pub fn act(&self, k: Key) -> Option<Act> {
        self.keys.iter().find(|(_, x)| *x == k).map(|(a, _)| *a)
    }

    /// Bind `a` to `k`; refused if the key is reserved or another action has it (that one).
    pub fn set(&mut self, a: Act, k: Key) -> Result<(), Option<Act>> {
        if reserved(k) {
            return Err(None);
        }
        if let Some(other) = self.act(k).filter(|o| *o != a) {
            return Err(Some(other));
        }
        if let Some(e) = self.keys.iter_mut().find(|(x, _)| *x == a) {
            e.1 = k;
        }
        Ok(())
    }

    /// The keys that differ from the defaults, as (action, key name) for the settings file.
    pub fn overrides(&self) -> Vec<(String, String)> {
        ACTS.iter()
            .filter(|(a, _, _, k)| self.key(*a) != *k)
            .map(|(a, id, _, _)| ((*id).to_string(), self.key(*a).name().to_string()))
            .collect()
    }

    /// The defaults with the settings file's keys; unknown names and doubled keys are skipped.
    pub fn from_overrides(o: &[(String, String)]) -> Keys {
        let mut keys = Keys::default();
        for (id, name) in o {
            if let (Some((a, ..)), Some(k)) = (ACTS.iter().find(|x| x.1 == id), Key::from_name(name)) {
                let _ = keys.set(*a, k);
            }
        }
        keys
    }
}

/// A translated label that names a default key, with the key as set: "Pause (P)" says "(K)"
/// once Pause is on K.
pub fn keyed(text: &str, default: Key, now: Key) -> String {
    let (default, now) = (key_name(default), key_name(now));
    text.replace(&format!("({default})"), &format!("({now})")).replace(&format!("+{default})"), &format!("+{now})"))
}

/// A key's name in the current language ("Leertaste" for Space in German).
pub fn key_name(k: Key) -> &'static str {
    crate::i18n::tr(k.name())
}

/// The label of an action.
pub fn label(a: Act) -> &'static str {
    ACTS.iter().find(|x| x.0 == a).map_or("", |x| x.2)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A key can be changed, kept in the settings, and can't take another action's key or one the
    /// game needs (Escape, the scroll keys).
    #[test]
    fn keys_can_be_changed() {
        let mut k = Keys::default();
        assert_eq!(k.key(Act::Pause), Key::P);
        assert!(k.overrides().is_empty(), "the defaults are not written");
        assert_eq!(k.set(Act::Pause, Key::H), Err(Some(Act::ToCastle)));
        assert_eq!(k.set(Act::Pause, Key::Escape), Err(None));
        assert_eq!(k.set(Act::Pause, Key::W), Err(None));
        // a11y round 3, #2: and none the game uses otherwise (Tab moves focus, F places a flag
        // in the action window, Home moves the map cursor)
        for used in [Key::Tab, Key::F, Key::T, Key::Num1, Key::Num9, Key::Home, Key::End, Key::PageUp, Key::PageDown] {
            assert_eq!(k.set(Act::Pause, used), Err(None), "{used:?}");
        }
        assert_eq!(k.set(Act::Pause, Key::K), Ok(()));
        assert_eq!(k.act(Key::K), Some(Act::Pause));
        assert_eq!(k.overrides(), vec![("pause".to_string(), "K".to_string())]);
        assert_eq!(Keys::from_overrides(&k.overrides()), k);
        // a broken or doubled entry keeps the default
        let odd = vec![("pause".into(), "NoSuchKey".into()), ("map".into(), "H".into()), ("zz".into(), "K".into())];
        assert_eq!(Keys::from_overrides(&odd), Keys::default());
    }

    #[test]
    fn labels_name_the_keys_as_set() {
        assert_eq!(keyed("Pause (P)", Key::P, Key::K), "Pause (K)");
        assert_eq!(keyed("Mute (Ctrl+M)", Key::M, Key::B), "Mute (Ctrl+B)");
        assert_eq!(keyed("Pause (P)", Key::P, Key::P), "Pause (P)");
    }

    /// No two actions share a default key, and none is reserved.
    #[test]
    fn the_defaults_are_sound() {
        for (i, a) in ACTS.iter().enumerate() {
            assert!(!reserved(a.3), "{:?}", a.0);
            assert!(ACTS[i + 1..].iter().all(|b| b.3 != a.3), "{:?}", a.0);
        }
    }
}
