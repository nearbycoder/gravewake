//! First-run field tips: short notes that explain a mechanic the first time
//! it matters. Each tip shows once; seen tips are saved in `settings.json`.
use crate::controls::Action;
use crate::game::{Game, Mode};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tip {
    Move,
    Reload,
    Souls,
    Hurt,
    Collector,
    Bolt,
}
impl Tip {
    fn bit(self) -> u32 {
        1 << self as u32
    }
    /// The Collector's tip shows in the shop; the rest show in the arena.
    pub fn in_shop(self) -> bool {
        self == Tip::Collector
    }
    fn duration(self) -> f32 {
        if self.in_shop() { 40. } else { 10. }
    }
    pub fn text(self, g: &Game) -> String {
        let key = |action| g.prompt(action);
        match self {
            Tip::Move => format!(
                "{} to move. Hold {} to sprint. {} dodges, and nothing can hurt you mid-dodge.",
                g.movement_prompt(),
                key(Action::Sprint),
                key(Action::Dodge)
            ),
            Tip::Reload => format!(
                "{} reloads before the magazine runs dry. {} strikes in melee when they get close.",
                key(Action::Reload),
                key(Action::Melee)
            ),
            Tip::Souls => "Every kill leaves a green soul. Walk near souls to gather them; each soul level lets you choose a power.".into(),
            Tip::Hurt => "The red arc around the reticle points toward whatever hurt you.".into(),
            Tip::Collector => "The Collector pays you after each descent. Spend gold on weapons, armor, the Hollow Chalice or a pack. Choose Upgrade Card to strengthen your weapon in the Binding.".into(),
            Tip::Bolt => format!(
                "The chalice is bound. Press {} to cast Ember Bolt for 12 mana. Mana refills over time.",
                key(Action::Bolt)
            ),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ActiveTip {
    pub tip: Tip,
    pub life: f32,
}
impl ActiveTip {
    /// Fades in quickly and out over the last second.
    pub fn alpha(&self) -> f32 {
        let shown = self.tip.duration() - self.life;
        (shown / 0.3).min(self.life).clamp(0., 1.)
    }
}

impl Game {
    fn tips_live(&self) -> bool {
        self.tips_enabled && self.prefs.field_tips && self.practice_backup.is_none()
    }
    /// Advance the visible tip and queue any tip whose moment has come.
    pub fn update_tips(&mut self, dt: f32) {
        if let Some(active) = &mut self.tip {
            let shop = matches!(self.mode, Mode::Shop | Mode::Tree | Mode::Pack);
            if active.tip.in_shop() {
                if self.mode == Mode::Arena {
                    self.tip = None;
                } else if shop {
                    active.life -= dt;
                }
            } else if self.mode == Mode::Arena {
                active.life -= dt;
            } else if shop {
                // An arena tip left over at the end of a descent.
                self.tip = None;
            }
            if self.tip.is_some_and(|t| t.life <= 0.) {
                self.tip = None;
            }
        }
        if !self.tips_live() {
            self.tip_queue.clear();
            return;
        }
        let arena = self.mode == Mode::Arena;
        let weapon = &self.run.weapon;
        for (tip, due) in [
            (Tip::Move, arena && self.run.time > 0.4),
            (
                Tip::Reload,
                arena && !weapon.kind.melee() && self.run.ammo * 3 <= weapon.capacity(),
            ),
            (Tip::Souls, arena && !self.run.survival.orbs.is_empty()),
            (Tip::Hurt, arena && !self.damage_marks.is_empty()),
            (Tip::Collector, self.mode == Mode::Shop),
            (Tip::Bolt, self.run.chalice),
        ] {
            if due && self.prefs.tips_seen & tip.bit() == 0 {
                self.prefs.tips_seen |= tip.bit();
                self.tip_queue.push(tip);
                self.save_preferences();
            }
        }
        if self.tip.is_none() {
            // Show the next tip that belongs where the player is now.
            let shop = self.mode == Mode::Shop;
            if let Some(i) = self
                .tip_queue
                .iter()
                .position(|t| (t.in_shop() && shop) || (!t.in_shop() && arena))
            {
                let tip = self.tip_queue.remove(i);
                self.tip = Some(ActiveTip {
                    tip,
                    life: tip.duration(),
                });
            }
        }
    }
    /// The journal's Field tips switch. Turning tips back on shows them all again.
    pub fn set_field_tips(&mut self, on: bool) {
        self.prefs.field_tips = on;
        if on {
            self.prefs.tips_seen = 0;
        } else {
            self.tip = None;
            self.tip_queue.clear();
        }
        self.save_preferences();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{DamageMark, Preferences};
    fn game() -> Game {
        let mut g = Game::new(false);
        g.tips_enabled = true;
        g.new_run();
        g
    }
    fn run_until_tip(g: &mut Game) -> Option<Tip> {
        for _ in 0..120 {
            g.update_tips(1. / 60.);
            if let Some(active) = g.tip {
                return Some(active.tip);
            }
        }
        None
    }
    #[test]
    fn each_tip_appears_once_when_its_moment_comes() {
        let mut g = game();
        g.run.time = 1.;
        assert_eq!(run_until_tip(&mut g), Some(Tip::Move));
        assert!(g.tip.unwrap().tip.text(&g).starts_with("W A S D to move. Hold SHIFT"));
        // Later moments queue behind the visible tip instead of overlapping.
        g.run.ammo = 1;
        g.damage_marks.push(DamageMark {
            bearing: 0.,
            life: 1.,
        });
        g.update_tips(0.);
        assert_eq!(g.tip_queue, vec![Tip::Reload, Tip::Hurt]);
        g.update_tips(11.);
        assert_eq!(g.tip.map(|t| t.tip), Some(Tip::Reload));
        g.update_tips(11.);
        assert_eq!(g.tip.map(|t| t.tip), Some(Tip::Hurt));
        g.update_tips(11.);
        assert_eq!(g.tip, None);
        // A new run doesn't repeat them.
        g.new_run();
        g.run.time = 1.;
        g.run.ammo = 0;
        assert_eq!(run_until_tip(&mut g), None);
    }
    #[test]
    fn the_collector_tip_waits_for_the_shop_and_bolt_follows_the_chalice() {
        let mut g = game();
        g.prefs.tips_seen = Tip::Move.bit();
        g.run.chalice = true;
        g.mode = Mode::Shop;
        g.update_tips(0.);
        assert_eq!(g.tip.map(|t| t.tip), Some(Tip::Collector));
        assert_eq!(g.tip_queue, vec![Tip::Bolt], "the bolt tip waits for the arena");
        g.mode = Mode::Arena;
        g.update_tips(0.);
        assert_eq!(g.tip.map(|t| t.tip), Some(Tip::Bolt));
        assert!(g.tip.unwrap().tip.text(&g).contains("Press Q to cast Ember Bolt"));
    }
    #[test]
    fn the_switch_hides_tips_and_turning_it_on_shows_them_again() {
        let mut g = game();
        g.run.time = 1.;
        assert_eq!(run_until_tip(&mut g), Some(Tip::Move));
        g.set_field_tips(false);
        assert_eq!(g.tip, None);
        g.run.ammo = 0;
        assert_eq!(run_until_tip(&mut g), None);
        g.set_field_tips(true);
        assert_eq!(g.prefs.tips_seen, 0);
        assert_eq!(run_until_tip(&mut g), Some(Tip::Move));
    }
    #[test]
    fn scripted_and_practice_runs_record_no_tips() {
        let mut g = Game::new(false);
        assert!(!g.tips_enabled, "smoke and reviews disable saving and tips");
        g.new_run();
        g.run.time = 1.;
        assert_eq!(run_until_tip(&mut g), None);
        assert_eq!(g.prefs.tips_seen, 0);
        let mut g = game();
        g.practice_backup = Some(g.run.clone());
        g.run.time = 1.;
        assert_eq!(run_until_tip(&mut g), None);
        assert_eq!(g.prefs.tips_seen, 0);
    }
    #[test]
    fn seen_tips_and_the_switch_persist() {
        let prefs = Preferences {
            field_tips: false,
            tips_seen: 0b101,
            ..Preferences::default()
        };
        let bytes = serde_json::to_vec(&prefs).unwrap();
        assert_eq!(Preferences::from_json(&bytes), Some(prefs));
        let old = Preferences::from_json(br#"{"volume":0.5}"#).unwrap();
        assert!(old.field_tips && old.tips_seen == 0);
    }
    #[test]
    fn tips_name_controller_inputs_after_the_controller_is_used() {
        use crate::controls::Device;
        const TIPS: [Tip; 6] = [
            Tip::Move,
            Tip::Reload,
            Tip::Souls,
            Tip::Hurt,
            Tip::Collector,
            Tip::Bolt,
        ];
        let mut g = game();
        let keyboard: Vec<_> = TIPS.iter().map(|t| t.text(&g)).collect();
        g.device = Device::Controller;
        let pad: Vec<_> = TIPS.iter().map(|t| t.text(&g)).collect();
        assert_eq!(
            pad[0],
            "LEFT STICK to move. Hold LT to sprint. A dodges, and nothing can hurt you mid-dodge."
        );
        assert!(pad[1].starts_with("X reloads") && pad[1].contains("B strikes"));
        assert!(pad[5].contains("Press Y to cast"));
        for pad in &pad {
            for label in ["W A S D", "SHIFT", "SPACE", "Press Q", "R reloads", "E strikes"] {
                assert!(!pad.contains(label), "{pad}");
            }
        }
        // Souls, damage arcs and the Collector name no inputs.
        assert_eq!(keyboard[2..5], pad[2..5]);
        assert!(keyboard[0].starts_with("W A S D to move. Hold SHIFT to sprint. SPACE dodges"));
        g.device = Device::Keyboard;
        assert_eq!(TIPS.map(|t| t.text(&g)).to_vec(), keyboard);
    }
}
