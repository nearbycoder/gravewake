//! On-screen touch controls for the browser build on phones and tablets,
//! which have no keyboard, mouse or controller. They show in the arena while
//! touch is the input the player last used (`controls::Device::Touch`), and
//! the menus are tapped as egui sees each tap as a click.
//!
//! - The left part of the screen is a floating stick: it appears where the
//!   thumb lands and moves the hunter; pushed to its rim, the hunter sprints.
//! - Dragging anywhere else looks around, as the mouse does.
//! - Buttons on the right: Fire (held; dragging it also aims), Reload,
//!   Dodge, Melee and Ember Bolt; Pause at the top right.
//!
//! Positions are egui points, laid out clear of the notch and the home
//! indicator by the page's safe-area insets.
// Only the browser build feeds it touches; the desktop runs its tests.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
use egui::{Pos2, Rect, Vec2, pos2, vec2};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Button {
    Fire,
    Reload,
    Dodge,
    Melee,
    Bolt,
    Pause,
}
impl Button {
    pub const ALL: [Button; 6] = [
        Button::Fire,
        Button::Reload,
        Button::Dodge,
        Button::Melee,
        Button::Bolt,
        Button::Pause,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Button::Fire => "FIRE",
            Button::Reload => "RELOAD",
            Button::Dodge => "DODGE",
            Button::Melee => "MELEE",
            Button::Bolt => "BOLT",
            Button::Pause => "II",
        }
    }
}

/// The stick's travel, in points; past `SPRINT` of it the hunter sprints.
pub const STICK_RADIUS: f32 = 56.;
const SPRINT: f32 = 0.92;
/// Radians turned per point dragged at the default mouse sensitivity: a
/// drag across half a phone held sideways turns about 120°.
const LOOK_PER_POINT: f32 = 0.005;
/// A Fire press that moves further than this also aims.
const FIRE_AIM_SLOP: f32 = 6.;

/// Where everything sits on the current screen.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Layout {
    pub screen: Rect,
    /// Safe-area insets: top, right, bottom, left.
    pub insets: [f32; 4],
}
impl Layout {
    pub fn new(screen: Rect, insets: [f32; 4]) -> Self {
        Self { screen, insets }
    }
    fn safe(&self) -> Rect {
        let [top, right, bottom, left] = self.insets;
        Rect::from_min_max(
            self.screen.min + vec2(left, top),
            self.screen.max - vec2(right, bottom),
        )
    }
    /// A button's centre and radius. Each is at least 44 points across.
    pub fn button(&self, button: Button) -> (Pos2, f32) {
        let safe = self.safe();
        // Smaller screens get slightly smaller buttons, never below 44 pt.
        let k = (safe.height() / 390.).clamp(0.85, 1.25);
        let fire = pos2(safe.max.x - 78. * k, safe.max.y - 82. * k);
        match button {
            Button::Fire => (fire, 46. * k),
            // An arc above and left of Fire, clear of the weapon panel
            // that the HUD draws to Fire's left.
            Button::Reload => (fire + vec2(0., -82.) * k, 29. * k),
            Button::Dodge => (fire + vec2(-84., -48.) * k, 29. * k),
            Button::Melee => (fire + vec2(-74., -122.) * k, 29. * k),
            Button::Bolt => (fire + vec2(-150., -70.) * k, 29. * k),
            Button::Pause => (pos2(safe.max.x - 34., safe.min.y + 34.), 24.),
        }
    }
    pub fn button_at(&self, p: Pos2) -> Option<Button> {
        Button::ALL.into_iter().find(|&b| {
            let (centre, radius) = self.button(b);
            // A little forgiving beyond the drawn edge.
            centre.distance(p) <= radius + 6.
        })
    }
    /// Touches starting here drive the stick; elsewhere they look around.
    pub fn in_stick_zone(&self, p: Pos2) -> bool {
        p.x < self.screen.min.x + self.screen.width() * 0.42
    }
}

#[derive(Clone, Copy, Debug)]
enum Role {
    Stick {
        origin: Pos2,
        at: Pos2,
    },
    Look {
        last: Pos2,
    },
    Button {
        button: Button,
        start: Pos2,
        last: Pos2,
        aiming: bool,
    },
}
#[derive(Clone, Copy, Debug)]
struct Finger {
    id: u64,
    role: Role,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Started,
    Moved,
    Ended,
}

/// What the touch controls ask for this frame.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Frame {
    pub forward: f32,
    pub right: f32,
    pub sprint: bool,
    /// Fire held, or tapped since the last frame.
    pub fire: bool,
    /// Yaw (positive turns right) and pitch (positive looks up), radians,
    /// at the default sensitivity, not yet inverted.
    pub look: Vec2,
    pub dodge: bool,
    pub reload: bool,
    pub melee: bool,
    pub bolt: bool,
    pub pause: bool,
}

#[derive(Default)]
pub struct Touch {
    fingers: Vec<Finger>,
    look: Vec2,
    pressed: Vec<Button>,
    pub layout: Option<Layout>,
}
impl Touch {
    /// One touch point changed. Touches are only taken as controls while
    /// `controls` is true (the arena); otherwise they're forgotten, and egui
    /// handles them as taps.
    pub fn event(&mut self, id: u64, phase: Phase, p: Pos2, controls: bool) {
        let Some(layout) = self.layout.filter(|_| controls) else {
            self.fingers.clear();
            return;
        };
        match phase {
            Phase::Started => {
                self.fingers.retain(|f| f.id != id);
                let role = if let Some(button) = layout.button_at(p) {
                    self.pressed.push(button);
                    Role::Button {
                        button,
                        start: p,
                        last: p,
                        aiming: false,
                    }
                } else if layout.in_stick_zone(p) {
                    Role::Stick { origin: p, at: p }
                } else {
                    Role::Look { last: p }
                };
                self.fingers.push(Finger { id, role });
            }
            Phase::Moved => {
                let Some(finger) = self.fingers.iter_mut().find(|f| f.id == id) else {
                    return;
                };
                match &mut finger.role {
                    Role::Stick { at, .. } => *at = p,
                    Role::Look { last } => {
                        self.look += p - *last;
                        *last = p;
                    }
                    Role::Button {
                        button: Button::Fire,
                        start,
                        last,
                        aiming,
                    } => {
                        *aiming |= start.distance(p) > FIRE_AIM_SLOP;
                        if *aiming {
                            self.look += p - *last;
                        }
                        *last = p;
                    }
                    Role::Button { .. } => {}
                }
            }
            Phase::Ended => self.fingers.retain(|f| f.id != id),
        }
    }
    /// Whether a finger is on a control.
    pub fn active(&self) -> bool {
        !self.fingers.is_empty()
    }
    /// Let go of everything (focus lost, or the arena left).
    pub fn release(&mut self) {
        self.fingers.clear();
        self.look = Vec2::ZERO;
        self.pressed.clear();
    }
    /// The stick's origin and knob, while a thumb is on it.
    pub fn stick(&self) -> Option<(Pos2, Pos2)> {
        self.fingers.iter().find_map(|f| match f.role {
            Role::Stick { origin, at } => {
                let offset = at - origin;
                let offset = offset * (STICK_RADIUS / offset.length().max(STICK_RADIUS));
                Some((origin, origin + offset))
            }
            _ => None,
        })
    }
    pub fn held(&self, button: Button) -> bool {
        self.fingers
            .iter()
            .any(|f| matches!(f.role, Role::Button { button: b, .. } if b == button))
    }
    /// This frame's input; one-shot presses and the look drag are consumed.
    pub fn frame(&mut self) -> Frame {
        let mut frame = Frame::default();
        if let Some((origin, knob)) = self.stick() {
            let raw = self.fingers.iter().find_map(|f| match f.role {
                Role::Stick { origin, at } => Some((at - origin).length()),
                _ => None,
            });
            let v = (knob - origin) / STICK_RADIUS;
            // A small dead zone, so resting the thumb doesn't walk.
            if v.length() > 0.12 {
                frame.forward = -v.y;
                frame.right = v.x;
            }
            frame.sprint = raw.unwrap_or(0.) >= STICK_RADIUS * SPRINT;
        }
        let pressed = std::mem::take(&mut self.pressed);
        let now = |b: Button| pressed.contains(&b);
        frame.fire = self.held(Button::Fire) || now(Button::Fire);
        frame.reload = now(Button::Reload);
        frame.dodge = now(Button::Dodge);
        frame.melee = now(Button::Melee);
        frame.bolt = now(Button::Bolt);
        frame.pause = now(Button::Pause);
        let look = std::mem::take(&mut self.look);
        frame.look = vec2(look.x, -look.y) * LOOK_PER_POINT;
        frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phone() -> Touch {
        Touch {
            // An iPhone held sideways, with its notch on the left.
            layout: Some(Layout::new(
                Rect::from_min_size(Pos2::ZERO, vec2(844., 390.)),
                [0., 0., 21., 47.],
            )),
            ..Default::default()
        }
    }

    #[test]
    fn buttons_are_large_enough_apart_and_on_screen() {
        for size in [vec2(844., 390.), vec2(667., 375.), vec2(1194., 834.)] {
            let layout = Layout::new(Rect::from_min_size(Pos2::ZERO, size), [0., 47., 21., 47.]);
            let safe = layout.safe();
            for a in Button::ALL {
                let (centre, radius) = layout.button(a);
                assert!(radius * 2. >= 44., "{a:?} is {} points across", radius * 2.);
                assert!(
                    safe.shrink(radius - 0.5).contains(centre),
                    "{a:?} leaves the safe area on {size:?}"
                );
                assert!(
                    !layout.in_stick_zone(centre),
                    "{a:?} is in the stick's half"
                );
                for b in Button::ALL.into_iter().filter(|&b| b != a) {
                    let (other, r) = layout.button(b);
                    assert!(
                        centre.distance(other) >= radius + r + 4.,
                        "{a:?} overlaps {b:?} on {size:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_stick_moves_and_sprints_at_its_rim() {
        let mut t = phone();
        t.event(1, Phase::Started, pos2(150., 280.), true);
        t.event(1, Phase::Moved, pos2(150., 252.), true);
        let f = t.frame();
        assert!(
            (f.forward - 0.5).abs() < 0.01 && f.right.abs() < 0.01 && !f.sprint,
            "{f:?}"
        );
        t.event(1, Phase::Moved, pos2(150. + STICK_RADIUS * 2., 280.), true);
        let f = t.frame();
        assert!((f.right - 1.).abs() < 0.01 && f.sprint, "{f:?}");
        t.event(1, Phase::Ended, pos2(0., 0.), true);
        assert_eq!(t.frame(), Frame::default());
    }

    #[test]
    fn several_fingers_move_look_and_fire_at_once() {
        let mut t = phone();
        let layout = t.layout.unwrap();
        let (fire, _) = layout.button(Button::Fire);
        t.event(1, Phase::Started, pos2(150., 280.), true);
        t.event(1, Phase::Moved, pos2(150., 230.), true);
        t.event(2, Phase::Started, pos2(500., 120.), true);
        t.event(2, Phase::Moved, pos2(540., 110.), true);
        t.event(3, Phase::Started, fire, true);
        let f = t.frame();
        assert!(f.forward > 0.8 && f.fire, "{f:?}");
        assert!(
            f.look.x > 0. && f.look.y > 0.,
            "dragging right and up turns right and looks up: {f:?}"
        );
        // Held fire keeps firing; the look drag was consumed.
        let f = t.frame();
        assert!(f.fire && f.look == Vec2::ZERO);
        // Dragging the fire button aims too.
        t.event(3, Phase::Moved, fire + vec2(-30., 0.), true);
        assert!(t.frame().look.x < 0.);
        t.event(3, Phase::Ended, fire, true);
        assert!(!t.frame().fire);
    }

    #[test]
    fn a_quick_tap_still_counts_once() {
        let mut t = phone();
        let layout = t.layout.unwrap();
        for button in [
            Button::Fire,
            Button::Reload,
            Button::Dodge,
            Button::Melee,
            Button::Bolt,
            Button::Pause,
        ] {
            let (at, _) = layout.button(button);
            t.event(7, Phase::Started, at, true);
            t.event(7, Phase::Ended, at, true);
            let f = t.frame();
            let got = [f.fire, f.reload, f.dodge, f.melee, f.bolt, f.pause];
            assert_eq!(got.iter().filter(|&&b| b).count(), 1, "{button:?}: {f:?}");
            assert_eq!(t.frame(), Frame::default(), "{button:?} fired twice");
        }
    }

    #[test]
    fn touches_outside_the_arena_are_left_to_the_menus() {
        let mut t = phone();
        t.event(1, Phase::Started, pos2(150., 280.), false);
        t.event(1, Phase::Moved, pos2(150., 200.), false);
        assert_eq!(t.frame(), Frame::default());
        assert!(t.stick().is_none());
    }
}
