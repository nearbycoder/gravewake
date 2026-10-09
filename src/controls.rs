//! Rebindable keyboard and mouse actions.
//!
//! Bindings store physical keys (positions), so they survive layout changes,
//! plus the character the key last produced, so labels match the player's
//! keyboard: on AZERTY the default forward key reads Z, not W.
use crate::gamepad::PadAction;
use serde::{Deserialize, Serialize};
use winit::{event::MouseButton, keyboard::KeyCode};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Action {
    Forward,
    Back,
    Left,
    Right,
    Sprint,
    Dodge,
    Reload,
    Melee,
    Bolt,
    Fire,
}
impl Action {
    /// In the Keyboard page's order: movement and sprint, then fire and the
    /// one-shot actions.
    pub const ALL: [Action; 10] = [
        Action::Forward,
        Action::Back,
        Action::Left,
        Action::Right,
        Action::Sprint,
        Action::Fire,
        Action::Dodge,
        Action::Reload,
        Action::Melee,
        Action::Bolt,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Action::Forward => "Forward",
            Action::Back => "Back",
            Action::Left => "Left",
            Action::Right => "Right",
            Action::Sprint => "Sprint",
            Action::Dodge => "Dodge",
            Action::Reload => "Reload",
            Action::Melee => "Melee",
            Action::Bolt => "Ember Bolt",
            Action::Fire => "Fire",
        }
    }
    fn index(self) -> usize {
        Self::ALL.iter().position(|a| *a == self).unwrap()
    }
    /// The remappable controller action that does the same thing, if any.
    /// Movement stays on the left stick.
    pub fn pad_action(self) -> Option<PadAction> {
        match self {
            Action::Forward | Action::Back | Action::Left | Action::Right => None,
            Action::Sprint => Some(PadAction::Sprint),
            Action::Dodge => Some(PadAction::Dodge),
            Action::Reload => Some(PadAction::Reload),
            Action::Melee => Some(PadAction::Melee),
            Action::Bolt => Some(PadAction::Bolt),
            Action::Fire => Some(PadAction::Fire),
        }
    }
}

/// The input the on-screen prompts describe: whichever the player used last.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Device {
    #[default]
    Keyboard,
    Controller,
    /// The browser build's on-screen controls (`touch.rs`).
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    Touch,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Trigger {
    Key(KeyCode),
    Mouse(MouseButton),
}

/// Keys that can be bound, with their stored names and default labels.
const KEYS: &[(KeyCode, &str, &str)] = &[
    (KeyCode::KeyA, "KeyA", "A"),
    (KeyCode::KeyB, "KeyB", "B"),
    (KeyCode::KeyC, "KeyC", "C"),
    (KeyCode::KeyD, "KeyD", "D"),
    (KeyCode::KeyE, "KeyE", "E"),
    (KeyCode::KeyF, "KeyF", "F"),
    (KeyCode::KeyG, "KeyG", "G"),
    (KeyCode::KeyH, "KeyH", "H"),
    (KeyCode::KeyI, "KeyI", "I"),
    (KeyCode::KeyJ, "KeyJ", "J"),
    (KeyCode::KeyK, "KeyK", "K"),
    (KeyCode::KeyL, "KeyL", "L"),
    (KeyCode::KeyM, "KeyM", "M"),
    (KeyCode::KeyN, "KeyN", "N"),
    (KeyCode::KeyO, "KeyO", "O"),
    (KeyCode::KeyP, "KeyP", "P"),
    (KeyCode::KeyQ, "KeyQ", "Q"),
    (KeyCode::KeyR, "KeyR", "R"),
    (KeyCode::KeyS, "KeyS", "S"),
    (KeyCode::KeyT, "KeyT", "T"),
    (KeyCode::KeyU, "KeyU", "U"),
    (KeyCode::KeyV, "KeyV", "V"),
    (KeyCode::KeyW, "KeyW", "W"),
    (KeyCode::KeyX, "KeyX", "X"),
    (KeyCode::KeyY, "KeyY", "Y"),
    (KeyCode::KeyZ, "KeyZ", "Z"),
    (KeyCode::Digit0, "Digit0", "0"),
    (KeyCode::Digit1, "Digit1", "1"),
    (KeyCode::Digit2, "Digit2", "2"),
    (KeyCode::Digit3, "Digit3", "3"),
    (KeyCode::Digit4, "Digit4", "4"),
    (KeyCode::Digit5, "Digit5", "5"),
    (KeyCode::Digit6, "Digit6", "6"),
    (KeyCode::Digit7, "Digit7", "7"),
    (KeyCode::Digit8, "Digit8", "8"),
    (KeyCode::Digit9, "Digit9", "9"),
    (KeyCode::Space, "Space", "SPACE"),
    (KeyCode::ShiftLeft, "ShiftLeft", "SHIFT"),
    (KeyCode::ShiftRight, "ShiftRight", "R-SHIFT"),
    (KeyCode::ControlLeft, "ControlLeft", "CTRL"),
    (KeyCode::ControlRight, "ControlRight", "R-CTRL"),
    (KeyCode::AltLeft, "AltLeft", "ALT"),
    (KeyCode::AltRight, "AltRight", "R-ALT"),
    (KeyCode::Tab, "Tab", "TAB"),
    (KeyCode::CapsLock, "CapsLock", "CAPS"),
    (KeyCode::Enter, "Enter", "ENTER"),
    (KeyCode::Backspace, "Backspace", "BACKSPACE"),
    (KeyCode::Backquote, "Backquote", "`"),
    (KeyCode::Minus, "Minus", "-"),
    (KeyCode::Equal, "Equal", "="),
    (KeyCode::BracketLeft, "BracketLeft", "["),
    (KeyCode::BracketRight, "BracketRight", "]"),
    (KeyCode::Backslash, "Backslash", "\\"),
    (KeyCode::Semicolon, "Semicolon", ";"),
    (KeyCode::Quote, "Quote", "'"),
    (KeyCode::Comma, "Comma", ","),
    (KeyCode::Period, "Period", "."),
    (KeyCode::Slash, "Slash", "/"),
    (KeyCode::IntlBackslash, "IntlBackslash", "<"),
    (KeyCode::ArrowUp, "ArrowUp", "UP"),
    (KeyCode::ArrowDown, "ArrowDown", "DOWN"),
    (KeyCode::ArrowLeft, "ArrowLeft", "LEFT"),
    (KeyCode::ArrowRight, "ArrowRight", "RIGHT"),
    (KeyCode::Insert, "Insert", "INSERT"),
    (KeyCode::Delete, "Delete", "DELETE"),
    (KeyCode::Home, "Home", "HOME"),
    (KeyCode::End, "End", "END"),
    (KeyCode::PageUp, "PageUp", "PAGE UP"),
    (KeyCode::PageDown, "PageDown", "PAGE DOWN"),
    (KeyCode::Numpad0, "Numpad0", "NUM 0"),
    (KeyCode::Numpad1, "Numpad1", "NUM 1"),
    (KeyCode::Numpad2, "Numpad2", "NUM 2"),
    (KeyCode::Numpad3, "Numpad3", "NUM 3"),
    (KeyCode::Numpad4, "Numpad4", "NUM 4"),
    (KeyCode::Numpad5, "Numpad5", "NUM 5"),
    (KeyCode::Numpad6, "Numpad6", "NUM 6"),
    (KeyCode::Numpad7, "Numpad7", "NUM 7"),
    (KeyCode::Numpad8, "Numpad8", "NUM 8"),
    (KeyCode::Numpad9, "Numpad9", "NUM 9"),
    (KeyCode::NumpadEnter, "NumpadEnter", "NUM ENTER"),
    (KeyCode::F1, "F1", "F1"),
    (KeyCode::F2, "F2", "F2"),
    (KeyCode::F3, "F3", "F3"),
    (KeyCode::F4, "F4", "F4"),
    (KeyCode::F5, "F5", "F5"),
    (KeyCode::F9, "F9", "F9"),
    (KeyCode::F10, "F10", "F10"),
    (KeyCode::F12, "F12", "F12"),
];
const BUTTONS: &[(MouseButton, &str, &str)] = &[
    (MouseButton::Left, "MouseLeft", "LEFT MOUSE"),
    (MouseButton::Right, "MouseRight", "RIGHT MOUSE"),
    (MouseButton::Middle, "MouseMiddle", "MIDDLE MOUSE"),
    (MouseButton::Back, "MouseBack", "MOUSE 4"),
    (MouseButton::Forward, "MouseForward", "MOUSE 5"),
];
/// Keys with fixed jobs: pause/back, the three presentation toggles and
/// fullscreen. The left mouse button can be bound, and still clicks menus.
const RESERVED: &[(KeyCode, &str)] = &[
    (KeyCode::Escape, "Escape"),
    (KeyCode::F6, "F6"),
    (KeyCode::F7, "F7"),
    (KeyCode::F8, "F8"),
    (KeyCode::F11, "F11"),
];

impl Trigger {
    fn name(self) -> &'static str {
        match self {
            Trigger::Key(code) => KEYS.iter().find(|k| k.0 == code).map(|k| k.1),
            Trigger::Mouse(button) => BUTTONS.iter().find(|b| b.0 == button).map(|b| b.1),
        }
        .unwrap_or("Unbound")
    }
    fn parse(name: &str) -> Option<Self> {
        KEYS.iter()
            .find(|k| k.1 == name)
            .map(|k| Trigger::Key(k.0))
            .or_else(|| {
                BUTTONS
                    .iter()
                    .find(|b| b.1 == name)
                    .map(|b| Trigger::Mouse(b.0))
            })
    }
    fn default_label(self) -> &'static str {
        match self {
            Trigger::Key(code) => KEYS.iter().find(|k| k.0 == code).map(|k| k.2),
            Trigger::Mouse(button) => BUTTONS.iter().find(|b| b.0 == button).map(|b| b.2),
        }
        .unwrap_or("?")
    }
    /// Keys whose default label is one character may show the layout's own.
    fn takes_glyph(self) -> bool {
        matches!(self, Trigger::Key(_)) && self.default_label().chars().count() == 1
    }
    /// Why this input can't be bound, if it can't.
    pub fn refusal(self) -> Option<String> {
        match self {
            Trigger::Key(code) => {
                if let Some((_, name)) = RESERVED.iter().find(|r| r.0 == code) {
                    Some(format!("{name} is reserved."))
                } else if !KEYS.iter().any(|k| k.0 == code) {
                    Some("That key can't be bound.".into())
                } else {
                    None
                }
            }
            Trigger::Mouse(button) => (!BUTTONS.iter().any(|b| b.0 == button))
                .then(|| "That mouse button can't be bound.".into()),
        }
    }
}

/// The bindable keys whose label may be the layout's own character.
pub fn glyph_keys() -> impl Iterator<Item = KeyCode> {
    KEYS.iter()
        .map(|k| k.0)
        .filter(|&code| Trigger::Key(code).takes_glyph())
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Binding {
    pub trigger: Trigger,
    /// The character the key produced on the player's layout, if any.
    pub glyph: Option<char>,
}
impl Binding {
    pub fn label(&self) -> String {
        match (self.trigger, self.glyph) {
            (Trigger::Key(_), Some(c)) => c.to_uppercase().collect(),
            (trigger, _) => trigger.default_label().into(),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Stored {
    action: Action,
    input: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    glyph: Option<char>,
}

/// One binding per action. Every action has a distinct input.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(from = "Vec<Stored>", into = "Vec<Stored>")]
pub struct Bindings([Binding; 10]);
impl Default for Bindings {
    fn default() -> Self {
        let key = |code| Binding {
            trigger: Trigger::Key(code),
            glyph: None,
        };
        Self([
            key(KeyCode::KeyW),
            key(KeyCode::KeyS),
            key(KeyCode::KeyA),
            key(KeyCode::KeyD),
            key(KeyCode::ShiftLeft),
            Binding {
                trigger: Trigger::Mouse(MouseButton::Left),
                glyph: None,
            },
            key(KeyCode::Space),
            key(KeyCode::KeyR),
            key(KeyCode::KeyE),
            key(KeyCode::KeyQ),
        ])
    }
}
impl From<Vec<Stored>> for Bindings {
    // Unknown, reserved or duplicate entries mean the file was damaged or
    // hand-edited badly; fall back to defaults rather than lose an action.
    fn from(stored: Vec<Stored>) -> Self {
        let mut bindings = Self::default();
        for entry in stored {
            match Trigger::parse(&entry.input) {
                Some(trigger) if trigger.refusal().is_none() => {
                    bindings.0[entry.action.index()] = Binding {
                        trigger,
                        glyph: entry.glyph.filter(|c| trigger.takes_glyph() && !c.is_control()),
                    };
                }
                _ => return Self::default(),
            }
        }
        if bindings.has_duplicates() {
            return Self::default();
        }
        bindings
    }
}
impl From<Bindings> for Vec<Stored> {
    fn from(bindings: Bindings) -> Self {
        Action::ALL
            .iter()
            .map(|&action| {
                let b = bindings.get(action);
                Stored {
                    action,
                    input: b.trigger.name().into(),
                    glyph: b.glyph,
                }
            })
            .collect()
    }
}
impl Bindings {
    pub fn get(&self, action: Action) -> Binding {
        self.0[action.index()]
    }
    pub fn label(&self, action: Action) -> String {
        self.get(action).label()
    }
    pub fn action(&self, trigger: Trigger) -> Option<Action> {
        Action::ALL
            .iter()
            .copied()
            .find(|&a| self.get(a).trigger == trigger)
    }
    fn has_duplicates(&self) -> bool {
        self.0
            .iter()
            .enumerate()
            .any(|(i, a)| self.0[..i].iter().any(|b| b.trigger == a.trigger))
    }
    /// Bind `action` to `trigger`. An action that already used `trigger`
    /// takes `action`'s old input and is returned.
    pub fn assign(
        &mut self,
        action: Action,
        trigger: Trigger,
        glyph: Option<char>,
    ) -> Result<Option<Action>, String> {
        if let Some(reason) = trigger.refusal() {
            return Err(reason);
        }
        let old = self.get(action);
        let displaced = self.action(trigger).filter(|&a| a != action);
        if let Some(other) = displaced {
            self.0[other.index()] = old;
        }
        self.0[action.index()] = Binding {
            trigger,
            glyph: glyph.filter(|c| trigger.takes_glyph() && !c.is_control() && !c.is_whitespace()),
        };
        Ok(displaced)
    }
    /// Remember the character a bound key produces on this layout.
    /// Returns true if a label changed.
    pub fn learn_glyph(&mut self, code: KeyCode, glyph: char) -> bool {
        if glyph.is_control() || glyph.is_whitespace() || !Trigger::Key(code).takes_glyph() {
            return false;
        }
        let mut changed = false;
        for b in &mut self.0 {
            if b.trigger == Trigger::Key(code) && b.glyph != Some(glyph) {
                b.glyph = Some(glyph);
                changed = true;
            }
        }
        changed
    }
    /// Remember the characters a layout gives (`keymap::Glyphs`) for every
    /// bound key. Returns true if a label changed.
    pub fn learn_layout(&mut self, glyphs: &[(KeyCode, char)]) -> bool {
        glyphs
            .iter()
            .fold(false, |changed, &(code, glyph)| self.learn_glyph(code, glyph) || changed)
    }
    /// "W A S D", or the four current movement labels.
    pub fn movement_label(&self) -> String {
        [Action::Forward, Action::Left, Action::Back, Action::Right]
            .map(|a| self.label(a))
            .join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rebinding_to_a_used_key_swaps_the_two_actions() {
        let mut b = Bindings::default();
        assert_eq!(
            b.assign(Action::Dodge, Trigger::Key(KeyCode::KeyE), Some('e')),
            Ok(Some(Action::Melee))
        );
        assert_eq!(b.get(Action::Dodge).trigger, Trigger::Key(KeyCode::KeyE));
        assert_eq!(b.get(Action::Melee).trigger, Trigger::Key(KeyCode::Space));
        assert_eq!(b.label(Action::Dodge), "E");
        assert_eq!(b.label(Action::Melee), "SPACE");
        // Rebinding to the same input changes nothing else.
        assert_eq!(
            b.assign(Action::Dodge, Trigger::Key(KeyCode::KeyE), None),
            Ok(None)
        );
        assert!(!b.has_duplicates());
        assert_eq!(
            b.assign(Action::Melee, Trigger::Mouse(MouseButton::Right), None),
            Ok(None)
        );
        assert_eq!(
            b.action(Trigger::Mouse(MouseButton::Right)),
            Some(Action::Melee)
        );
        assert_eq!(b.label(Action::Melee), "RIGHT MOUSE");
    }
    #[test]
    fn reserved_and_unknown_inputs_are_refused() {
        let mut b = Bindings::default();
        for code in [KeyCode::Escape, KeyCode::F6, KeyCode::F7, KeyCode::F8, KeyCode::F11] {
            assert!(b.assign(Action::Bolt, Trigger::Key(code), None).is_err());
        }
        assert!(
            b.assign(Action::Bolt, Trigger::Key(KeyCode::MediaPlayPause), None)
                .is_err()
        );
        assert_eq!(b, Bindings::default());
    }
    #[test]
    fn fire_moves_off_the_left_button_and_older_files_keep_it_there() {
        let mut b = Bindings::default();
        for action in Action::ALL {
            assert_eq!(b.action(b.get(action).trigger), Some(action), "{action:?} at its index");
        }
        assert_eq!(b.get(Action::Fire).trigger, Trigger::Mouse(MouseButton::Left));
        assert_eq!(b.label(Action::Fire), "LEFT MOUSE");
        // To a free key: the left button is left unbound.
        assert_eq!(b.assign(Action::Fire, Trigger::Key(KeyCode::KeyF), Some('f')), Ok(None));
        assert_eq!(b.action(Trigger::Key(KeyCode::KeyF)), Some(Action::Fire));
        assert_eq!(b.action(Trigger::Mouse(MouseButton::Left)), None);
        assert_eq!(b.label(Action::Fire), "F");
        // The left button for melee; then fire back on it, swapping with melee.
        assert_eq!(b.assign(Action::Melee, Trigger::Mouse(MouseButton::Left), None), Ok(None));
        assert_eq!(
            b.assign(Action::Fire, Trigger::Mouse(MouseButton::Left), None),
            Ok(Some(Action::Melee))
        );
        assert_eq!(b.get(Action::Melee).trigger, Trigger::Key(KeyCode::KeyF));
        assert!(!b.has_duplicates());
        // Saved by name, and files from before Fire could move keep it on the
        // left button, with their other keys.
        let json = serde_json::to_string(&b).unwrap();
        assert!(json.contains(r#"{"action":"Fire","input":"MouseLeft"}"#), "{json}");
        assert_eq!(serde_json::from_str::<Bindings>(&json).unwrap(), b);
        let older = r#"[{"action":"Reload","input":"KeyT"},{"action":"Melee","input":"MouseRight"}]"#;
        let old: Bindings = serde_json::from_str(older).unwrap();
        assert_eq!(old.get(Action::Fire).trigger, Trigger::Mouse(MouseButton::Left));
        assert_eq!(old.get(Action::Reload).trigger, Trigger::Key(KeyCode::KeyT));
        // A file that put another action on the left button and fire on it too
        // is damaged: defaults.
        let clash = r#"[{"action":"Melee","input":"MouseLeft"}]"#;
        assert_eq!(serde_json::from_str::<Bindings>(clash).unwrap(), Bindings::default());
    }
    #[test]
    fn labels_follow_the_keyboard_layout() {
        let mut b = Bindings::default();
        assert_eq!(b.movement_label(), "W A S D");
        // An AZERTY keyboard produces z and q from the W and A positions.
        assert!(b.learn_glyph(KeyCode::KeyW, 'z'));
        assert!(b.learn_glyph(KeyCode::KeyA, 'q'));
        assert!(!b.learn_glyph(KeyCode::KeyA, 'q'));
        assert!(!b.learn_glyph(KeyCode::KeyM, 'm'), "unbound keys are ignored");
        assert!(!b.learn_glyph(KeyCode::Space, 'x'), "named keys keep their names");
        assert_eq!(b.movement_label(), "Z Q S D");
        assert_eq!(b.get(Action::Forward).trigger, Trigger::Key(KeyCode::KeyW));
    }
    #[test]
    fn bindings_round_trip_and_damaged_files_fall_back_to_defaults() {
        let mut b = Bindings::default();
        b.assign(Action::Sprint, Trigger::Mouse(MouseButton::Back), None)
            .unwrap();
        b.learn_glyph(KeyCode::KeyW, 'z');
        let json = serde_json::to_string(&b).unwrap();
        assert!(json.contains("\"MouseBack\"") && json.contains("\"z\""), "{json}");
        assert_eq!(serde_json::from_str::<Bindings>(&json).unwrap(), b);
        // A partial list keeps defaults for the rest.
        let partial: Bindings =
            serde_json::from_str(r#"[{"action":"Bolt","input":"KeyF"}]"#).unwrap();
        assert_eq!(partial.get(Action::Bolt).trigger, Trigger::Key(KeyCode::KeyF));
        assert_eq!(partial.get(Action::Forward), Bindings::default().get(Action::Forward));
        for damaged in [
            r#"[{"action":"Bolt","input":"KeyW"}]"#,
            r#"[{"action":"Bolt","input":"Escape"}]"#,
            r#"[{"action":"Bolt","input":"NoSuchKey"}]"#,
        ] {
            assert_eq!(
                serde_json::from_str::<Bindings>(damaged).unwrap(),
                Bindings::default(),
                "{damaged}"
            );
        }
    }
}
