//! Controller support. `Pad` polls gilrs into a `Frame` snapshot; everything
//! else is pure mapping, so tests and the gamepad smoke run need no hardware.
//! Arena play maps sticks to movement and look, and remappable buttons
//! (`PadBindings`) to game actions. Menus use a virtual cursor that sends
//! ordinary egui pointer events, so every existing screen works without a
//! separate focus-navigation layer. The D-pad jumps the cursor between the
//! controls each screen reports (`Target`).
use egui::{Event, Modifiers, PointerButton, Pos2, Rect};
use glam::Vec2;
use serde::{Deserialize, Serialize};

/// Controller buttons the game uses, named by position (Xbox: A B X Y).
/// The triggers count as buttons once pulled past `TRIGGER_THRESHOLD`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Button {
    South,
    East,
    West,
    North,
    Start,
    LeftBumper,
    RightBumper,
    LeftTrigger,
    RightTrigger,
    LeftStick,
    RightStick,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
}
impl Button {
    /// Buttons that can be bound to actions, with their Xbox labels. Start
    /// and the D-pad are reserved for pausing, powers and the menu cursor.
    pub const BINDABLE: [(Button, &'static str); 10] = [
        (Button::South, "A"),
        (Button::East, "B"),
        (Button::West, "X"),
        (Button::North, "Y"),
        (Button::LeftBumper, "LB"),
        (Button::RightBumper, "RB"),
        (Button::LeftTrigger, "LT"),
        (Button::RightTrigger, "RT"),
        (Button::LeftStick, "LS"),
        (Button::RightStick, "RS"),
    ];
    pub fn label(self) -> &'static str {
        Self::BINDABLE
            .iter()
            .find(|(b, _)| *b == self)
            .map_or(match self {
                Button::Start => "START",
                _ => "D-PAD",
            }, |(_, label)| label)
    }
    /// Why this button can't be bound, if it can't.
    pub fn refusal(self) -> Option<&'static str> {
        match self {
            Button::Start => Some("Start always pauses."),
            Button::DPadUp | Button::DPadDown | Button::DPadLeft | Button::DPadRight => {
                Some("The D-pad chooses powers and moves the menu cursor.")
            }
            _ => None,
        }
    }
}
const BUTTONS: [(gilrs::Button, Button); 13] = [
    (gilrs::Button::South, Button::South),
    (gilrs::Button::East, Button::East),
    (gilrs::Button::West, Button::West),
    (gilrs::Button::North, Button::North),
    (gilrs::Button::Start, Button::Start),
    (gilrs::Button::LeftTrigger, Button::LeftBumper),
    (gilrs::Button::RightTrigger, Button::RightBumper),
    (gilrs::Button::LeftThumb, Button::LeftStick),
    (gilrs::Button::RightThumb, Button::RightStick),
    (gilrs::Button::DPadUp, Button::DPadUp),
    (gilrs::Button::DPadDown, Button::DPadDown),
    (gilrs::Button::DPadLeft, Button::DPadLeft),
    (gilrs::Button::DPadRight, Button::DPadRight),
];

/// One update's controller state. Stick y is positive when pushed up.
#[derive(Clone, Default, Debug)]
pub struct Frame {
    pub left: Vec2,
    pub right: Vec2,
    pub left_trigger: f32,
    pub right_trigger: f32,
    pub held: Vec<Button>,
    pub pressed: Vec<Button>,
    pub released: Vec<Button>,
    /// The controller driving the game disconnected this update.
    pub lost: bool,
}
impl Frame {
    pub fn held(&self, b: Button) -> bool {
        self.held.contains(&b)
    }
    pub fn pressed(&self, b: Button) -> bool {
        self.pressed.contains(&b)
    }
    pub fn released(&self, b: Button) -> bool {
        self.released.contains(&b)
    }
    /// Held, counting a trigger pulled past its threshold.
    pub fn down(&self, b: Button) -> bool {
        let pulled = match b {
            Button::LeftTrigger => self.left_trigger > TRIGGER_THRESHOLD,
            Button::RightTrigger => self.right_trigger > TRIGGER_THRESHOLD,
            _ => false,
        };
        pulled || self.held(b)
    }
    /// The first button pressed this update, for binding.
    pub fn first_pressed(&self) -> Option<Button> {
        self.pressed.first().copied()
    }
    /// Whether the player used the controller this update: a button press, a
    /// trigger pull or a stick past its deadzone. Resting-stick drift doesn't count.
    pub fn deliberate(&self) -> bool {
        !self.pressed.is_empty()
            || self.left_trigger.max(self.right_trigger) > TRIGGER_THRESHOLD
            || self.left.length().max(self.right.length()) > STICK_DEADZONE
    }
}

pub const STICK_DEADZONE: f32 = 0.2;
pub const TRIGGER_THRESHOLD: f32 = 0.35;
/// Turn rate in radians per second at full deflection and default sensitivity.
pub const LOOK_SPEED: f32 = 3.4;
/// Virtual cursor speed in egui points per second at full deflection.
pub const CURSOR_SPEED: f32 = 1100.;
const DPAD_NUDGE: f32 = 0.45;

/// Radial deadzone, rescaled so movement starts smoothly at its edge.
pub fn radial(v: Vec2, dead: f32) -> Vec2 {
    let length = v.length();
    if length <= dead {
        return Vec2::ZERO;
    }
    v / length * ((length.min(1.) - dead) / (1. - dead))
}
/// Squared response: fine control near the centre, full speed at the rim.
fn curve(v: Vec2) -> Vec2 {
    v * v.length()
}

/// Controller actions that can be remapped. Movement and look stay on the
/// sticks; pausing stays on Start.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum PadAction {
    Fire,
    Sprint,
    Dodge,
    Reload,
    Melee,
    Bolt,
}
impl PadAction {
    pub const ALL: [PadAction; 6] = [
        PadAction::Fire,
        PadAction::Sprint,
        PadAction::Dodge,
        PadAction::Reload,
        PadAction::Melee,
        PadAction::Bolt,
    ];
    pub fn name(self) -> &'static str {
        match self {
            PadAction::Fire => "Fire",
            PadAction::Sprint => "Sprint",
            PadAction::Dodge => "Dodge",
            PadAction::Reload => "Reload",
            PadAction::Melee => "Melee",
            PadAction::Bolt => "Ember Bolt",
        }
    }
    fn index(self) -> usize {
        Self::ALL.iter().position(|a| *a == self).unwrap()
    }
}

#[derive(Serialize, Deserialize)]
struct StoredPad {
    action: PadAction,
    buttons: Vec<Button>,
}

/// A main and a second button per action. Every action keeps a main button,
/// and no button does two things.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(from = "Vec<StoredPad>", into = "Vec<StoredPad>")]
pub struct PadBindings([[Option<Button>; 2]; 6]);
impl Default for PadBindings {
    fn default() -> Self {
        use Button::*;
        Self([
            [Some(RightTrigger), None],
            [Some(LeftTrigger), Some(LeftStick)],
            [Some(South), None],
            [Some(West), None],
            [Some(East), Some(RightBumper)],
            [Some(North), Some(LeftBumper)],
        ])
    }
}
impl From<Vec<StoredPad>> for PadBindings {
    // A damaged or badly hand-edited entry restores the defaults rather than
    // leave an action without a button.
    fn from(stored: Vec<StoredPad>) -> Self {
        let mut bindings = Self::default();
        for entry in stored {
            if entry.buttons.is_empty()
                || entry.buttons.len() > 2
                || entry.buttons.iter().any(|b| b.refusal().is_some())
            {
                return Self::default();
            }
            bindings.0[entry.action.index()] = [Some(entry.buttons[0]), entry.buttons.get(1).copied()];
        }
        if bindings.has_duplicates() {
            return Self::default();
        }
        bindings
    }
}
impl From<PadBindings> for Vec<StoredPad> {
    fn from(bindings: PadBindings) -> Self {
        PadAction::ALL
            .iter()
            .map(|&action| StoredPad {
                action,
                buttons: bindings.buttons(action).collect(),
            })
            .collect()
    }
}
impl PadBindings {
    /// The action's main button, then its second, if any.
    pub fn buttons(&self, action: PadAction) -> impl Iterator<Item = Button> {
        self.0[action.index()].into_iter().flatten()
    }
    pub fn slot(&self, action: PadAction, slot: usize) -> Option<Button> {
        self.0[action.index()][slot]
    }
    /// The main button's label, as prompts show it.
    pub fn label(&self, action: PadAction) -> &'static str {
        self.buttons(action).next().map_or("?", Button::label)
    }
    fn find(&self, button: Button) -> Option<(PadAction, usize)> {
        PadAction::ALL.into_iter().find_map(|action| {
            (0..2).find(|&slot| self.slot(action, slot) == Some(button)).map(|slot| (action, slot))
        })
    }
    fn has_duplicates(&self) -> bool {
        let all: Vec<Button> = self.0.iter().flatten().flatten().copied().collect();
        all.iter().enumerate().any(|(i, b)| all[..i].contains(b))
    }
    /// Put `button` in `action`'s `slot`. If another action's slot held it,
    /// that slot takes this slot's old button (or empties), and the other
    /// action is returned. Refused if the button is reserved, or if an
    /// action would be left without a button.
    pub fn assign(
        &mut self,
        action: PadAction,
        slot: usize,
        button: Button,
    ) -> Result<Option<PadAction>, String> {
        if let Some(reason) = button.refusal() {
            return Err(reason.into());
        }
        let mut next = *self;
        let old = next.0[action.index()][slot];
        let displaced = next.find(button).filter(|&found| found != (action, slot));
        if let Some((other, other_slot)) = displaced {
            next.0[other.index()][other_slot] = old;
        }
        next.0[action.index()][slot] = Some(button);
        // A main slot that emptied takes its action's second button.
        for pair in &mut next.0 {
            if pair[0].is_none() {
                pair.swap(0, 1);
            }
        }
        if let Some(bare) = PadAction::ALL.into_iter().find(|&a| next.buttons(a).next().is_none()) {
            return Err(format!(
                "{} is {}'s only button.",
                button.label(),
                bare.name()
            ));
        }
        *self = next;
        Ok(displaced.map(|(other, _)| other).filter(|&other| other != action))
    }
    fn held(&self, frame: &Frame, action: PadAction) -> bool {
        self.buttons(action).any(|b| frame.down(b))
    }
    fn pressed(&self, frame: &Frame, action: PadAction) -> bool {
        self.buttons(action).any(|b| frame.pressed(b))
    }
}

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Arena {
    pub forward: f32,
    pub right: f32,
    pub sprint: bool,
    pub fire: bool,
    /// Yaw (positive turns right) and pitch (positive looks up) in rad/s.
    pub look: Vec2,
    pub dodge: bool,
    pub reload: bool,
    pub melee: bool,
    pub spell: bool,
    pub pause: bool,
    /// D-pad left/up/right picks soul power 1/2/3 while leveling.
    pub power: Option<usize>,
}
pub fn arena(frame: &Frame, bindings: &PadBindings) -> Arena {
    let movement = radial(frame.left, STICK_DEADZONE);
    Arena {
        forward: movement.y,
        right: movement.x,
        sprint: bindings.held(frame, PadAction::Sprint),
        fire: bindings.held(frame, PadAction::Fire),
        look: curve(radial(frame.right, STICK_DEADZONE)) * LOOK_SPEED,
        dodge: bindings.pressed(frame, PadAction::Dodge),
        reload: bindings.pressed(frame, PadAction::Reload),
        melee: bindings.pressed(frame, PadAction::Melee),
        spell: bindings.pressed(frame, PadAction::Bolt),
        pause: frame.pressed(Button::Start),
        power: [Button::DPadLeft, Button::DPadUp, Button::DPadRight]
            .iter()
            .position(|b| frame.pressed(*b)),
    }
}

/// A control on the current screen that the D-pad can reach, in egui points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    pub rect: Rect,
    /// Where the cursor rests on it: the centre, or a slider's knob.
    pub point: Pos2,
    /// A slider's track ends (x), which left and right step along.
    pub track: Option<(f32, f32)>,
}
impl Target {
    pub fn button(rect: Rect) -> Self {
        Self {
            rect,
            point: rect.center(),
            track: None,
        }
    }
}

/// The D-pad directions in screen space (y down): left, right, up, down.
const DPAD: [(Button, egui::Vec2); 4] = [
    (Button::DPadLeft, egui::vec2(-1., 0.)),
    (Button::DPadRight, egui::vec2(1., 0.)),
    (Button::DPadUp, egui::vec2(0., -1.)),
    (Button::DPadDown, egui::vec2(0., 1.)),
];
/// A held D-pad direction repeats after this long, then at this interval.
pub const DPAD_REPEAT_DELAY: f32 = 0.4;
pub const DPAD_REPEAT_INTERVAL: f32 = 0.12;
/// Left and right move a slider by this share of its track.
pub const SLIDER_STEP: f32 = 1. / 20.;

/// The target the D-pad reaches from `from` in direction `dir`: the nearest
/// within about 50° of it, weighing sideways distance double, or failing
/// that the nearest anywhere on that side. The target under the cursor is
/// skipped.
pub fn neighbour(from: Pos2, dir: egui::Vec2, targets: &[Target]) -> Option<usize> {
    let mut best: Option<(bool, f32, usize)> = None;
    for (i, t) in targets.iter().enumerate() {
        if t.rect.contains(from) {
            continue;
        }
        let v = t.point - from;
        let along = v.dot(dir);
        if along < 1. {
            continue;
        }
        let across = (v.x * dir.y - v.y * dir.x).abs();
        let candidate = (across <= along * 1.2, along + across * 2., i);
        let better = match best {
            None => true,
            Some((cone, score, _)) => {
                (candidate.0 && !cone) || (candidate.0 == cone && candidate.1 < score)
            }
        };
        if better {
            best = Some(candidate);
        }
    }
    best.map(|(_, _, i)| i)
}

/// The control the cursor rests on, for the focus frame: the smallest
/// target containing `at`, so a button on a card wins over the card.
pub fn focused(at: Pos2, targets: &[Target]) -> Option<usize> {
    targets
        .iter()
        .enumerate()
        .filter(|(_, t)| t.rect.contains(at))
        .min_by(|(_, a), (_, b)| a.rect.area().total_cmp(&b.rect.area()))
        .map(|(i, _)| i)
}

/// Stick-driven pointer for menus. `pos` is `None` until the stick, the
/// D-pad or A is used, and again after the real mouse moves.
#[derive(Default, Debug)]
pub struct Cursor {
    pub pos: Option<Pos2>,
    clicking: bool,
    /// How long each D-pad direction has been held, for repeats.
    held_for: [f32; 4],
}
impl Cursor {
    /// D-pad directions that act this update: fresh presses, and held
    /// directions once the repeat delay has passed.
    fn dpad(&mut self, frame: &Frame, dt: f32) -> Vec<egui::Vec2> {
        let mut fired = vec![];
        for (i, (button, dir)) in DPAD.into_iter().enumerate() {
            if frame.pressed(button) {
                self.held_for[i] = 0.;
                fired.push(dir);
            } else if frame.held(button) {
                let before = self.held_for[i];
                self.held_for[i] += dt;
                let ticks = |t: f32| ((t - DPAD_REPEAT_DELAY) / DPAD_REPEAT_INTERVAL).floor();
                if self.held_for[i] >= DPAD_REPEAT_DELAY
                    && (before < DPAD_REPEAT_DELAY || ticks(self.held_for[i]) > ticks(before))
                {
                    fired.push(dir);
                }
            } else {
                self.held_for[i] = 0.;
            }
        }
        fired
    }
    /// Returns egui events for this frame, and whether B or Start asked to go
    /// back. `targets` are the controls the D-pad can reach; with none, the
    /// D-pad nudges the cursor instead.
    pub fn step(
        &mut self,
        frame: &Frame,
        dt: f32,
        screen: Rect,
        targets: &[Target],
    ) -> (Vec<Event>, bool) {
        let mut stick = curve(radial(frame.left, STICK_DEADZONE));
        let mut jumps = vec![];
        if targets.is_empty() {
            for (button, dir) in DPAD {
                if frame.held(button) {
                    stick += Vec2::new(dir.x, -dir.y) * DPAD_NUDGE;
                }
            }
        } else {
            jumps = self.dpad(frame, dt);
        }
        let back = frame.pressed(Button::East) || frame.pressed(Button::Start);
        let press = frame.pressed(Button::South);
        let release =
            self.clicking && (frame.released(Button::South) || !frame.held(Button::South));
        let mut events = vec![];
        if self.pos.is_none() && stick == Vec2::ZERO && !press && jumps.is_empty() {
            return (events, back);
        }
        let mut moved = stick != Vec2::ZERO;
        if self.pos.is_none() && !jumps.is_empty() {
            // The first D-pad press lands on the control nearest the centre.
            jumps.clear();
            let centre = screen.center();
            if let Some(t) = targets
                .iter()
                .min_by(|a, b| (a.point - centre).length().total_cmp(&(b.point - centre).length()))
            {
                self.pos = Some(t.point);
                moved = true;
            }
        }
        let pos = self.pos.get_or_insert(screen.center());
        if stick != Vec2::ZERO {
            let step = egui::vec2(stick.x, -stick.y) * CURSOR_SPEED * dt;
            *pos = (*pos + step).clamp(screen.min, screen.max - egui::vec2(1., 1.));
        }
        let mut slid = false;
        for dir in jumps {
            let here = *pos;
            let slider = targets
                .iter()
                .find(|t| t.track.is_some() && t.rect.contains(here));
            if let (Some(&Target { track: Some((x0, x1)), .. }), true) = (slider, dir.y == 0.) {
                // Step along the slider, then click there to set it.
                pos.x = (pos.x + dir.x * (x1 - x0) * SLIDER_STEP).clamp(x0, x1);
                slid = true;
            } else if let Some(i) = neighbour(here, dir, targets) {
                *pos = targets[i].point;
            } else {
                continue;
            }
            moved = true;
        }
        if moved || press {
            events.push(Event::PointerMoved(*pos));
        }
        if slid && !self.clicking {
            events.push(Self::button(*pos, true));
            events.push(Self::button(*pos, false));
        }
        if press {
            events.push(Self::button(*pos, true));
            self.clicking = true;
        }
        // A quick tap can press and release within one update; egui sees both.
        if release || (press && frame.released(Button::South)) {
            events.push(Self::button(*pos, false));
            self.clicking = false;
        }
        (events, back)
    }
    fn button(pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        }
    }
    /// Hand control back to the mouse.
    pub fn hide(&mut self) {
        self.pos = None;
        self.clicking = false;
        self.held_for = [0.; 4];
    }
}

/// Left-stick deflection that moves the cursor toward `target` in one step,
/// inverting the deadzone and response curve. Used by the gamepad smoke run.
pub fn steer(from: Pos2, target: Pos2, dt: f32) -> Vec2 {
    let delta = target - from;
    let distance = delta.length();
    if distance < 0.5 {
        return Vec2::ZERO;
    }
    let speed = ((distance / dt) / CURSOR_SPEED).min(1.);
    let deflection = STICK_DEADZONE + speed.sqrt() * (1. - STICK_DEADZONE);
    Vec2::new(delta.x, -delta.y) / distance * deflection
}

/// gilrs connection plus the virtual cursor. Device errors disable controllers
/// without affecting keyboard and mouse play.
pub struct Pad {
    gilrs: Option<gilrs::Gilrs>,
    active: Option<gilrs::GamepadId>,
    /// Whether each trigger (left, right) was past its threshold last update.
    triggers: [bool; 2],
    pub cursor: Cursor,
    /// Connection changes to show the player.
    pub notices: Vec<String>,
}
impl Pad {
    pub fn new(enabled: bool) -> Self {
        let gilrs = if enabled {
            match gilrs::Gilrs::new() {
                Ok(gilrs) => Some(gilrs),
                Err(gilrs::Error::NotImplemented(_)) => None,
                Err(e) => {
                    eprintln!("Controller support unavailable: {e}");
                    None
                }
            }
        } else {
            None
        };
        Self {
            gilrs,
            active: None,
            triggers: [false; 2],
            cursor: Cursor::default(),
            notices: vec![],
        }
    }
    pub fn poll(&mut self) -> Frame {
        let mut frame = Frame::default();
        let Some(gilrs) = &mut self.gilrs else {
            return frame;
        };
        let map = |b: gilrs::Button| BUTTONS.iter().find(|(g, _)| *g == b).map(|(_, b)| *b);
        while let Some(gilrs::Event { id, event, .. }) = gilrs.next_event() {
            match event {
                gilrs::EventType::Connected => {
                    self.active.get_or_insert(id);
                    let name = gilrs.gamepad(id).name().to_owned();
                    self.notices
                        .push(format!("CONTROLLER CONNECTED  /  {name}"));
                }
                gilrs::EventType::Disconnected => {
                    if self.active == Some(id) {
                        self.active = None;
                        frame.lost = true;
                    }
                    self.notices.push("CONTROLLER DISCONNECTED".into());
                }
                gilrs::EventType::ButtonPressed(b, _) => {
                    // The most recently used controller drives the game.
                    self.active = Some(id);
                    frame.pressed.extend(map(b));
                }
                gilrs::EventType::ButtonReleased(b, _) if self.active == Some(id) => {
                    frame.released.extend(map(b));
                }
                _ => {}
            }
        }
        if self
            .active
            .is_none_or(|id| !gilrs.gamepad(id).is_connected())
        {
            self.active = gilrs.gamepads().next().map(|(id, _)| id);
        }
        let Some(id) = self.active else {
            return frame;
        };
        let pad = gilrs.gamepad(id);
        frame.left = Vec2::new(
            pad.value(gilrs::Axis::LeftStickX),
            pad.value(gilrs::Axis::LeftStickY),
        );
        frame.right = Vec2::new(
            pad.value(gilrs::Axis::RightStickX),
            pad.value(gilrs::Axis::RightStickY),
        );
        let trigger = |b| {
            pad.button_data(b)
                .map_or(if pad.is_pressed(b) { 1. } else { 0. }, |d| d.value())
        };
        frame.left_trigger = trigger(gilrs::Button::LeftTrigger2);
        frame.right_trigger = trigger(gilrs::Button::RightTrigger2);
        frame.held = BUTTONS
            .iter()
            .filter(|(g, _)| pad.is_pressed(*g))
            .map(|(_, b)| *b)
            .collect();
        trigger_edges(&mut frame, &mut self.triggers);
        frame
    }
}

/// Turn analog trigger pulls into presses and releases of the trigger
/// buttons, with `was` remembering last update's state.
fn trigger_edges(frame: &mut Frame, was: &mut [bool; 2]) {
    for (i, (value, button)) in [
        (frame.left_trigger, Button::LeftTrigger),
        (frame.right_trigger, Button::RightTrigger),
    ]
    .into_iter()
    .enumerate()
    {
        let down = value > TRIGGER_THRESHOLD;
        if down {
            frame.held.push(button);
        }
        if down && !was[i] {
            frame.pressed.push(button);
        } else if !down && was[i] {
            frame.released.push(button);
        }
        was[i] = down;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn screen() -> Rect {
        Rect::from_min_size(Pos2::ZERO, egui::vec2(1440., 900.))
    }
    #[test]
    fn deadzone_is_radial_and_rescaled() {
        assert_eq!(radial(Vec2::new(0.15, 0.1), STICK_DEADZONE), Vec2::ZERO);
        let edge = radial(Vec2::new(0.21, 0.), STICK_DEADZONE);
        assert!(edge.x > 0. && edge.x < 0.02);
        assert!((radial(Vec2::new(0., 1.), STICK_DEADZONE).y - 1.).abs() < 1e-6);
        // Over-range diagonals from square gates stay within the unit circle.
        assert!(radial(Vec2::new(1., 1.), STICK_DEADZONE).length() <= 1. + 1e-6);
    }
    /// A frame that presses the button a prompt names.
    fn pressing(label: &str) -> Frame {
        let mut frame = Frame::default();
        if label == "LEFT STICK" {
            frame.left = Vec2::new(0., 1.);
            return frame;
        }
        let button = Button::BINDABLE
            .iter()
            .find(|(_, l)| *l == label)
            .unwrap_or_else(|| panic!("unexpected label {label}"))
            .0;
        match button {
            Button::LeftTrigger => frame.left_trigger = 1.,
            Button::RightTrigger => frame.right_trigger = 1.,
            _ => {}
        }
        frame.pressed.push(button);
        frame.held.push(button);
        frame
    }
    #[test]
    fn prompt_labels_name_the_inputs_that_perform_each_action() {
        use crate::controls::{Action, Device};
        let mut custom = PadBindings::default();
        custom.assign(PadAction::Dodge, 0, Button::RightStick).unwrap();
        custom.assign(PadAction::Fire, 0, Button::RightBumper).unwrap();
        custom.assign(PadAction::Bolt, 0, Button::LeftTrigger).unwrap();
        for bindings in [PadBindings::default(), custom] {
            let mut g = crate::game::Game::new(false);
            g.device = Device::Controller;
            g.prefs.pad_bindings = bindings;
            for action in Action::ALL {
                let label = g.prompt(action);
                let a = arena(&pressing(&label), &bindings);
                let performed = match action {
                    Action::Forward | Action::Back | Action::Left | Action::Right => a.forward > 0.9,
                    Action::Sprint => a.sprint,
                    Action::Dodge => a.dodge,
                    Action::Reload => a.reload,
                    Action::Melee => a.melee,
                    Action::Bolt => a.spell,
                };
                assert!(performed, "{action:?} / {label}");
            }
            assert!(arena(&pressing(g.fire_prompt()), &bindings).fire);
        }
        // Remapped prompts follow the bindings.
        let mut g = crate::game::Game::new(false);
        g.device = Device::Controller;
        g.prefs.pad_bindings = custom;
        assert_eq!(g.prompt(Action::Dodge), "RS");
        assert_eq!(g.prompt(Action::Bolt), "LT");
        assert_eq!(g.fire_prompt(), "RB");
    }
    #[test]
    fn pad_bindings_swap_refuse_reserved_buttons_and_keep_every_action_bound() {
        use Button::*;
        let mut b = PadBindings::default();
        let pair = |b: &PadBindings, a| b.buttons(a).collect::<Vec<_>>();
        // A button in use swaps with the slot being changed.
        assert_eq!(b.assign(PadAction::Dodge, 0, East), Ok(Some(PadAction::Melee)));
        assert_eq!(pair(&b, PadAction::Dodge), [East]);
        assert_eq!(pair(&b, PadAction::Melee), [South, RightBumper]);
        // Taking another action's second button leaves its slot empty.
        assert_eq!(b.assign(PadAction::Fire, 1, LeftStick), Ok(Some(PadAction::Sprint)));
        assert_eq!(pair(&b, PadAction::Fire), [RightTrigger, LeftStick]);
        assert_eq!(pair(&b, PadAction::Sprint), [LeftTrigger]);
        // Moving a button within one action reorders it.
        assert_eq!(b.assign(PadAction::Melee, 0, RightBumper), Ok(None));
        assert_eq!(pair(&b, PadAction::Melee), [RightBumper, South]);
        // An emptied main slot takes the second button.
        assert_eq!(b.assign(PadAction::Reload, 1, RightBumper), Ok(Some(PadAction::Melee)));
        assert_eq!(pair(&b, PadAction::Melee), [South]);
        // Reserved buttons and leaving an action bare are refused unchanged.
        let before = b;
        for button in [Start, DPadUp, DPadDown, DPadLeft, DPadRight] {
            assert!(b.assign(PadAction::Fire, 0, button).is_err(), "{button:?}");
        }
        let bare = b.assign(PadAction::Melee, 1, East).unwrap_err();
        assert_eq!(bare, "B is Dodge's only button.");
        assert_eq!(b, before);
        assert!(!b.has_duplicates());
        // Every bindable button can be bound somewhere.
        for (button, _) in Button::BINDABLE {
            let mut b = PadBindings::default();
            assert!(b.assign(PadAction::Fire, 1, button).is_ok() || b.assign(PadAction::Fire, 0, button).is_ok(), "{button:?}");
        }
    }
    #[test]
    fn pad_bindings_round_trip_and_damaged_entries_restore_defaults() {
        let mut custom = PadBindings::default();
        custom.assign(PadAction::Reload, 1, Button::RightStick).unwrap();
        let json = serde_json::to_string(&custom).unwrap();
        assert_eq!(serde_json::from_str::<PadBindings>(&json).unwrap(), custom);
        for damaged in [
            r#"[{"action":"Fire","buttons":["Start"]}]"#,
            r#"[{"action":"Fire","buttons":[]}]"#,
            r#"[{"action":"Fire","buttons":["South","East","West"]}]"#,
            r#"[{"action":"Fire","buttons":["South"]}]"#,
        ] {
            let loaded: PadBindings = serde_json::from_str(damaged).unwrap();
            assert_eq!(loaded, PadBindings::default(), "{damaged}");
        }
        // Through the settings file: other preferences survive a bad entry,
        // and files from before remapping load the defaults.
        let prefs = crate::game::Preferences::from_json(
            br#"{"volume":0.3,"pad_bindings":[{"action":"Nope","buttons":["South"]}]}"#,
        )
        .unwrap();
        assert_eq!(prefs.volume, 0.3);
        assert_eq!(prefs.pad_bindings, PadBindings::default());
        let legacy = crate::game::Preferences::from_json(br#"{"volume":0.2}"#).unwrap();
        assert_eq!(legacy.pad_bindings, PadBindings::default());
    }
    #[test]
    fn trigger_pulls_become_presses_and_releases() {
        let mut was = [false; 2];
        let mut step = |left: f32, right: f32| {
            let mut frame = Frame {
                left_trigger: left,
                right_trigger: right,
                ..Default::default()
            };
            trigger_edges(&mut frame, &mut was);
            frame
        };
        let light = step(0.2, 0.);
        assert!(light.pressed.is_empty() && light.held.is_empty());
        let pull = step(0.6, 0.);
        assert_eq!(pull.pressed, [Button::LeftTrigger]);
        assert_eq!(pull.first_pressed(), Some(Button::LeftTrigger));
        let hold = step(0.9, 0.5);
        assert_eq!(hold.pressed, [Button::RightTrigger]);
        assert_eq!(hold.held, [Button::LeftTrigger, Button::RightTrigger]);
        let release = step(0.1, 0.5);
        assert_eq!(release.released, [Button::LeftTrigger]);
        assert!(release.pressed.is_empty());
        // The release of the press that chose a slot binds nothing.
        let chosen = Frame {
            released: vec![Button::South],
            ..Default::default()
        };
        assert_eq!(chosen.first_pressed(), None);
    }
    #[test]
    fn only_deliberate_controller_input_switches_prompts() {
        // Resting-stick drift, light trigger pressure and held buttons don't count.
        let drift = Frame {
            left: Vec2::new(0.12, -0.1),
            right: Vec2::new(-0.15, 0.1),
            left_trigger: 0.2,
            right_trigger: 0.3,
            held: vec![Button::South],
            ..Default::default()
        };
        assert!(!drift.deliberate());
        assert!(!Frame::default().deliberate());
        for frame in [
            Frame {
                pressed: vec![Button::Start],
                ..Default::default()
            },
            Frame {
                right_trigger: 0.5,
                ..Default::default()
            },
            Frame {
                left: Vec2::new(0., 0.4),
                ..Default::default()
            },
            Frame {
                right: Vec2::new(-0.3, 0.),
                ..Default::default()
            },
        ] {
            assert!(frame.deliberate(), "{frame:?}");
        }
    }
    #[test]
    fn arena_mapping_covers_every_action() {
        let frame = Frame {
            left: Vec2::new(0., 1.),
            right: Vec2::new(1., 0.),
            left_trigger: 0.8,
            right_trigger: 0.9,
            pressed: vec![
                Button::South,
                Button::West,
                Button::East,
                Button::North,
                Button::Start,
            ],
            ..Default::default()
        };
        let a = arena(&frame, &PadBindings::default());
        assert!((a.forward - 1.).abs() < 1e-6 && a.right.abs() < 1e-6);
        assert!(a.sprint && a.fire && a.dodge && a.reload && a.melee && a.spell && a.pause);
        assert!((a.look.x - LOOK_SPEED).abs() < 1e-5 && a.look.y.abs() < 1e-6);
        let idle = arena(
            &Frame {
                left: Vec2::new(0.1, -0.1),
                right_trigger: 0.2,
                ..Default::default()
            },
            &PadBindings::default(),
        );
        assert_eq!(idle, Arena::default());
        for (button, power) in [
            (Button::DPadLeft, 0),
            (Button::DPadUp, 1),
            (Button::DPadRight, 2),
        ] {
            let frame = Frame {
                pressed: vec![button],
                ..Default::default()
            };
            assert_eq!(arena(&frame, &PadBindings::default()).power, Some(power));
        }
    }
    #[test]
    fn cursor_moves_clamps_clicks_and_goes_back() {
        let mut cursor = Cursor::default();
        let (events, back) = cursor.step(&Frame::default(), 1. / 60., screen(), &[]);
        assert!(events.is_empty() && !back && cursor.pos.is_none());
        let right = Frame {
            left: Vec2::new(1., 0.),
            ..Default::default()
        };
        let (events, _) = cursor.step(&right, 0.5, screen(), &[]);
        assert_eq!(cursor.pos, Some(Pos2::new(720. + CURSOR_SPEED * 0.5, 450.)));
        assert!(matches!(events[..], [Event::PointerMoved(_)]));
        for _ in 0..10 {
            cursor.step(&right, 0.5, screen(), &[]);
        }
        assert_eq!(cursor.pos.unwrap().x, 1439.);
        let press = Frame {
            pressed: vec![Button::South],
            held: vec![Button::South],
            ..Default::default()
        };
        let (events, _) = cursor.step(&press, 1. / 60., screen(), &[]);
        assert!(matches!(
            events[..],
            [
                Event::PointerMoved(_),
                Event::PointerButton { pressed: true, .. }
            ]
        ));
        let (events, _) = cursor.step(&Frame::default(), 1. / 60., screen(), &[]);
        assert!(matches!(
            events[..],
            [Event::PointerButton { pressed: false, .. }]
        ));
        let tap = Frame {
            pressed: vec![Button::South],
            released: vec![Button::South],
            ..Default::default()
        };
        let (events, _) = cursor.step(&tap, 1. / 60., screen(), &[]);
        assert_eq!(events.len(), 3);
        let (_, back) = cursor.step(
            &Frame {
                pressed: vec![Button::East],
                ..Default::default()
            },
            1. / 60.,
            screen(),
            &[],
        );
        assert!(back);
        cursor.hide();
        assert!(cursor.pos.is_none());
    }
    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(Pos2::new(x, y), egui::vec2(w, h))
    }
    fn tap(button: Button) -> Frame {
        Frame {
            pressed: vec![button],
            held: vec![button],
            ..Default::default()
        }
    }
    #[test]
    fn dpad_picks_the_nearest_control_in_its_direction() {
        // A row of three buttons over a wide one, like the pack screen.
        let targets = [
            Target::button(rect(200., 400., 200., 40.)),
            Target::button(rect(600., 400., 200., 40.)),
            Target::button(rect(1000., 400., 200., 40.)),
            Target::button(rect(560., 600., 320., 50.)),
        ];
        let at = |i: usize| targets[i].point;
        let right = egui::vec2(1., 0.);
        let down = egui::vec2(0., 1.);
        assert_eq!(neighbour(at(0), right, &targets), Some(1));
        assert_eq!(neighbour(at(1), right, &targets), Some(2));
        assert_eq!(neighbour(at(2), right, &targets), None);
        assert_eq!(neighbour(at(3), egui::vec2(0., -1.), &targets), Some(1));
        // Nothing straight below the left button: the nearest below it anyway.
        assert_eq!(neighbour(at(0), down, &targets), Some(3));
        assert_eq!(neighbour(at(3), down, &targets), None);
        // Off-centre inside a control, that control is skipped.
        assert_eq!(neighbour(Pos2::new(210., 420.), right, &targets), Some(1));
        // A closer control well off to the side loses to one in line.
        let side = [
            Target::button(rect(400., 380., 40., 40.)),
            Target::button(rect(380., 200., 40., 40.)),
            Target::button(rect(700., 40., 40., 40.)),
        ];
        assert_eq!(neighbour(Pos2::new(400., 700.), egui::vec2(0., -1.), &side), Some(0));
        assert_eq!(neighbour(Pos2::new(420., 400.), egui::vec2(0., -1.), &side), Some(1));
    }
    #[test]
    fn the_focus_frame_marks_the_smallest_control_under_the_cursor() {
        // A card (the equipped card is a target) with a button on it, and a
        // separate button beside it.
        let targets = [
            Target::button(rect(100., 100., 300., 420.)),
            Target::button(rect(140., 440., 220., 40.)),
            Target::button(rect(500., 440., 220., 40.)),
        ];
        assert_eq!(focused(Pos2::new(200., 200.), &targets), Some(0));
        assert_eq!(focused(Pos2::new(250., 460.), &targets), Some(1), "the button on the card");
        assert_eq!(focused(targets[2].point, &targets), Some(2));
        // Between controls, or with none on screen, there's no frame.
        assert_eq!(focused(Pos2::new(450., 460.), &targets), None);
        assert_eq!(focused(Pos2::new(250., 460.), &[]), None);
        // Wherever a D-pad press lands, that control is the one framed.
        let mut cursor = Cursor::default();
        let screen = rect(0., 0., 1440., 900.);
        cursor.step(&tap(Button::DPadRight), 0.016, screen, &targets);
        cursor.step(&Frame::default(), 0.016, screen, &targets);
        let first = focused(cursor.pos.unwrap(), &targets).unwrap();
        assert_eq!(cursor.pos, Some(targets[first].point));
        assert_eq!(first, 2, "the control nearest the centre");
        cursor.step(&tap(Button::DPadLeft), 0.016, screen, &targets);
        let next = focused(cursor.pos.unwrap(), &targets).unwrap();
        assert_eq!(next, 1, "the button on the card, in line, not the card");
        assert_eq!(cursor.pos, Some(targets[next].point));
    }
    #[test]
    fn first_dpad_press_lands_near_the_centre_and_holding_repeats() {
        let targets: Vec<Target> = (0..6)
            .map(|i| Target::button(rect(600., 100. + i as f32 * 120., 240., 40.)))
            .collect();
        let mut cursor = Cursor::default();
        let (events, _) = cursor.step(&tap(Button::DPadDown), 1. / 60., screen(), &targets);
        // Screen centre is y 450: the fourth row (centre 480) is nearest.
        assert_eq!(cursor.pos, Some(targets[3].point));
        assert!(matches!(events[..], [Event::PointerMoved(_)]));
        let hold = Frame {
            held: vec![Button::DPadDown],
            ..Default::default()
        };
        let mut cursor_rows = vec![];
        let mut elapsed = 0.;
        // Hold down for 0.7 s in 10 ms updates.
        for _ in 0..70 {
            cursor.step(&hold, 0.01, screen(), &targets);
            elapsed += 0.01;
            let p = cursor.pos.unwrap();
            let r = targets.iter().position(|t| t.point == p).unwrap();
            if cursor_rows.last().is_none_or(|(_, last)| *last != r) {
                cursor_rows.push((elapsed, r));
            }
        }
        // Repeats at the 0.4 s delay, then every 0.12 s; the last row stops it.
        assert_eq!(cursor_rows[0].1, 3);
        assert_eq!(cursor_rows[1].1, 4);
        assert!((cursor_rows[1].0 - DPAD_REPEAT_DELAY).abs() < 0.015, "{cursor_rows:?}");
        assert_eq!(cursor_rows[2].1, 5);
        assert!(
            (cursor_rows[2].0 - DPAD_REPEAT_DELAY - DPAD_REPEAT_INTERVAL).abs() < 0.015,
            "{cursor_rows:?}"
        );
        assert_eq!(cursor_rows.len(), 3);
        // Releasing and pressing again acts at once.
        cursor.step(&Frame::default(), 0.01, screen(), &targets);
        cursor.step(&tap(Button::DPadUp), 0.01, screen(), &targets);
        assert_eq!(cursor.pos, Some(targets[4].point));
        // A then clicks where the cursor rests.
        let (events, _) = cursor.step(&tap(Button::South), 0.01, screen(), &targets);
        assert!(matches!(
            events[..],
            [Event::PointerMoved(p), Event::PointerButton { pressed: true, pos, .. }]
                if p == targets[4].point && pos == p
        ));
    }
    #[test]
    fn dpad_steps_sliders_and_leaves_them_vertically() {
        let track = (600., 800.);
        let slider = Target {
            rect: rect(588., 285., 224., 30.),
            point: Pos2::new(650., 300.),
            track: Some(track),
        };
        let below = Target::button(rect(600., 400., 200., 40.));
        let targets = [slider, below];
        let mut cursor = Cursor {
            pos: Some(slider.point),
            ..Default::default()
        };
        let (events, _) = cursor.step(&tap(Button::DPadRight), 0.01, screen(), &targets);
        assert_eq!(cursor.pos, Some(Pos2::new(660., 300.)));
        assert!(matches!(
            events[..],
            [
                Event::PointerMoved(_),
                Event::PointerButton { pressed: true, .. },
                Event::PointerButton { pressed: false, .. }
            ]
        ));
        cursor.step(&Frame::default(), 0.01, screen(), &targets);
        for _ in 0..30 {
            cursor.step(&Frame::default(), 0.01, screen(), &targets);
            cursor.step(&tap(Button::DPadLeft), 0.01, screen(), &targets);
        }
        // The cursor stops at the track's end.
        assert_eq!(cursor.pos, Some(Pos2::new(600., 300.)));
        cursor.step(&Frame::default(), 0.01, screen(), &targets);
        let (events, _) = cursor.step(&tap(Button::DPadDown), 0.01, screen(), &targets);
        assert_eq!(cursor.pos, Some(below.point));
        assert!(matches!(events[..], [Event::PointerMoved(_)]));
        // Without targets the D-pad still nudges the cursor.
        let mut cursor = Cursor::default();
        cursor.step(&tap(Button::DPadRight), 0.1, screen(), &[]);
        assert!(cursor.pos.unwrap().x > 720.);
    }
    #[test]
    fn steering_reaches_targets_through_the_response_curve() {
        let mut cursor = Cursor::default();
        let target = Pos2::new(355., 812.);
        for _ in 0..60 {
            let from = cursor.pos.unwrap_or(screen().center());
            let frame = Frame {
                left: steer(from, target, 1. / 60.),
                ..Default::default()
            };
            cursor.step(&frame, 1. / 60., screen(), &[]);
        }
        assert!(
            cursor.pos.unwrap().distance(target) < 1.5,
            "{:?}",
            cursor.pos
        );
    }
}
