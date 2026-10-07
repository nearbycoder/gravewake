//! Controller support. `Pad` polls gilrs into a `Frame` snapshot; everything
//! else is pure mapping, so tests and the gamepad smoke run need no hardware.
//! Arena play maps sticks, triggers and face buttons to game actions. Menus
//! use a virtual cursor that sends ordinary egui pointer events, so every
//! existing screen works without a separate focus-navigation layer.
use egui::{Event, Modifiers, PointerButton, Pos2, Rect};
use glam::Vec2;

/// Controller buttons the game uses, named by position (Xbox: A B X Y).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Button {
    South,
    East,
    West,
    North,
    Start,
    LeftBumper,
    RightBumper,
    LeftStick,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
}
const BUTTONS: [(gilrs::Button, Button); 12] = [
    (gilrs::Button::South, Button::South),
    (gilrs::Button::East, Button::East),
    (gilrs::Button::West, Button::West),
    (gilrs::Button::North, Button::North),
    (gilrs::Button::Start, Button::Start),
    (gilrs::Button::LeftTrigger, Button::LeftBumper),
    (gilrs::Button::RightTrigger, Button::RightBumper),
    (gilrs::Button::LeftThumb, Button::LeftStick),
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
pub fn arena(frame: &Frame) -> Arena {
    let movement = radial(frame.left, STICK_DEADZONE);
    Arena {
        forward: movement.y,
        right: movement.x,
        sprint: frame.left_trigger > TRIGGER_THRESHOLD || frame.held(Button::LeftStick),
        fire: frame.right_trigger > TRIGGER_THRESHOLD,
        look: curve(radial(frame.right, STICK_DEADZONE)) * LOOK_SPEED,
        dodge: frame.pressed(Button::South),
        reload: frame.pressed(Button::West),
        melee: frame.pressed(Button::East) || frame.pressed(Button::RightBumper),
        spell: frame.pressed(Button::North) || frame.pressed(Button::LeftBumper),
        pause: frame.pressed(Button::Start),
        power: [Button::DPadLeft, Button::DPadUp, Button::DPadRight]
            .iter()
            .position(|b| frame.pressed(*b)),
    }
}

/// Stick-driven pointer for menus. `pos` is `None` until the stick or A is
/// used, and again after the real mouse moves.
#[derive(Default, Debug)]
pub struct Cursor {
    pub pos: Option<Pos2>,
    clicking: bool,
}
impl Cursor {
    /// Returns egui events for this frame, and whether B or Start asked to go back.
    pub fn step(&mut self, frame: &Frame, dt: f32, screen: Rect) -> (Vec<Event>, bool) {
        let mut stick = curve(radial(frame.left, STICK_DEADZONE));
        for (button, nudge) in [
            (Button::DPadLeft, Vec2::new(-DPAD_NUDGE, 0.)),
            (Button::DPadRight, Vec2::new(DPAD_NUDGE, 0.)),
            (Button::DPadUp, Vec2::new(0., DPAD_NUDGE)),
            (Button::DPadDown, Vec2::new(0., -DPAD_NUDGE)),
        ] {
            if frame.held(button) {
                stick += nudge;
            }
        }
        let back = frame.pressed(Button::East) || frame.pressed(Button::Start);
        let press = frame.pressed(Button::South);
        let release =
            self.clicking && (frame.released(Button::South) || !frame.held(Button::South));
        let mut events = vec![];
        if self.pos.is_none() && stick == Vec2::ZERO && !press {
            return (events, back);
        }
        let pos = self.pos.get_or_insert(screen.center());
        let moved = stick != Vec2::ZERO;
        if moved {
            let step = egui::vec2(stick.x, -stick.y) * CURSOR_SPEED * dt;
            *pos = (*pos + step).clamp(screen.min, screen.max - egui::vec2(1., 1.));
        }
        if moved || press {
            events.push(Event::PointerMoved(*pos));
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
        frame
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
    #[test]
    fn prompt_labels_name_the_inputs_that_perform_each_action() {
        use crate::controls::Action;
        for action in Action::ALL {
            let mut frame = Frame::default();
            match action.pad_label() {
                "LEFT STICK" => frame.left = Vec2::new(0., 1.),
                "LT" => frame.left_trigger = 1.,
                label => frame.pressed.push(match label {
                    "A" => Button::South,
                    "B" => Button::East,
                    "X" => Button::West,
                    "Y" => Button::North,
                    other => panic!("unexpected label {other}"),
                }),
            }
            let a = arena(&frame);
            let performed = match action {
                Action::Forward | Action::Back | Action::Left | Action::Right => a.forward > 0.9,
                Action::Sprint => a.sprint,
                Action::Dodge => a.dodge,
                Action::Reload => a.reload,
                Action::Melee => a.melee,
                Action::Bolt => a.spell,
            };
            assert!(performed, "{action:?} / {}", action.pad_label());
        }
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
        let a = arena(&frame);
        assert!((a.forward - 1.).abs() < 1e-6 && a.right.abs() < 1e-6);
        assert!(a.sprint && a.fire && a.dodge && a.reload && a.melee && a.spell && a.pause);
        assert!((a.look.x - LOOK_SPEED).abs() < 1e-5 && a.look.y.abs() < 1e-6);
        let idle = arena(&Frame {
            left: Vec2::new(0.1, -0.1),
            right_trigger: 0.2,
            ..Default::default()
        });
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
            assert_eq!(arena(&frame).power, Some(power));
        }
    }
    #[test]
    fn cursor_moves_clamps_clicks_and_goes_back() {
        let mut cursor = Cursor::default();
        let (events, back) = cursor.step(&Frame::default(), 1. / 60., screen());
        assert!(events.is_empty() && !back && cursor.pos.is_none());
        let right = Frame {
            left: Vec2::new(1., 0.),
            ..Default::default()
        };
        let (events, _) = cursor.step(&right, 0.5, screen());
        assert_eq!(cursor.pos, Some(Pos2::new(720. + CURSOR_SPEED * 0.5, 450.)));
        assert!(matches!(events[..], [Event::PointerMoved(_)]));
        for _ in 0..10 {
            cursor.step(&right, 0.5, screen());
        }
        assert_eq!(cursor.pos.unwrap().x, 1439.);
        let press = Frame {
            pressed: vec![Button::South],
            held: vec![Button::South],
            ..Default::default()
        };
        let (events, _) = cursor.step(&press, 1. / 60., screen());
        assert!(matches!(
            events[..],
            [
                Event::PointerMoved(_),
                Event::PointerButton { pressed: true, .. }
            ]
        ));
        let (events, _) = cursor.step(&Frame::default(), 1. / 60., screen());
        assert!(matches!(
            events[..],
            [Event::PointerButton { pressed: false, .. }]
        ));
        let tap = Frame {
            pressed: vec![Button::South],
            released: vec![Button::South],
            ..Default::default()
        };
        let (events, _) = cursor.step(&tap, 1. / 60., screen());
        assert_eq!(events.len(), 3);
        let (_, back) = cursor.step(
            &Frame {
                pressed: vec![Button::East],
                ..Default::default()
            },
            1. / 60.,
            screen(),
        );
        assert!(back);
        cursor.hide();
        assert!(cursor.pos.is_none());
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
            cursor.step(&frame, 1. / 60., screen());
        }
        assert!(
            cursor.pos.unwrap().distance(target) < 1.5,
            "{:?}",
            cursor.pos
        );
    }
}
