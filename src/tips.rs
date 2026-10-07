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
    /// The first sighting of a species (an `encounters::ROSTER` index).
    Creature(u8),
}
/// Creature notes appear when a species first comes this close, in view.
pub const SIGHTING_RANGE: f32 = 30.;
/// ... within this many radians of straight ahead, well inside the view at
/// any field of view and window shape.
pub const SIGHTING_HALF_ANGLE: f32 = 0.7;
impl Tip {
    fn bit(self) -> u32 {
        match self {
            Tip::Move => 1,
            Tip::Reload => 1 << 1,
            Tip::Souls => 1 << 2,
            Tip::Hurt => 1 << 3,
            Tip::Collector => 1 << 4,
            Tip::Bolt => 1 << 5,
            // Bits 8 to 19, one per species.
            Tip::Creature(kind) => 1 << (8 + kind as u32),
        }
    }
    /// The Ossuary Drudge, the plain shambler every run opens with, has no
    /// note; every other species does.
    pub fn creature(kind: usize) -> Option<Tip> {
        (kind > 0 && kind < crate::encounters::ROSTER.len()).then_some(Tip::Creature(kind as u8))
    }
    /// The panel's heading.
    pub fn heading(self) -> String {
        match self {
            Tip::Creature(kind) => format!(
                "NEW CREATURE  /  {}",
                crate::encounters::species(kind as usize).name
            ),
            _ => "FIELD NOTE".into(),
        }
    }
    /// The Collector's tip shows in the shop; the rest show in the arena.
    pub fn in_shop(self) -> bool {
        self == Tip::Collector
    }
    fn duration(self) -> f32 {
        match self {
            Tip::Collector => 40.,
            Tip::Creature(_) => 7.,
            _ => 10.,
        }
    }
    pub fn text(self, g: &Game) -> String {
        let key = |action| g.prompt(action);
        match self {
            Tip::Move => format!(
                "{} to move. {} {} to sprint. {} dodges, and nothing can hurt you mid-dodge.",
                g.movement_prompt(),
                if g.prefs.toggle_sprint { "Press" } else { "Hold" },
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
            // The bestiary's own lines: what it does, then how to answer it.
            Tip::Creature(kind) => {
                let lore = crate::encounters::species(kind as usize).lore;
                format!("{} {}", lore[1], lore[2])
            }
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
        ]
        .into_iter()
        .chain(self.sightings().into_iter().map(|tip| (tip, arena)))
        {
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
    /// Notes for species in view within `SIGHTING_RANGE`, with nothing solid
    /// between them and the player, nearest first.
    fn sightings(&self) -> Vec<Tip> {
        let mut seen: Vec<(f32, Tip)> = self
            .run
            .enemies
            .iter()
            .filter(|e| e.hp > 0.)
            .filter_map(|e| {
                let tip = Tip::creature(e.kind)?;
                let flat = (e.pos - self.run.pos) * glam::Vec3::new(1., 0., 1.);
                let distance = flat.length();
                let relative = flat.x.atan2(-flat.z) - self.run.yaw;
                let in_view = relative.sin().atan2(relative.cos()).abs() < SIGHTING_HALF_ANGLE;
                (distance < SIGHTING_RANGE && in_view && self.in_sight(e))
                    .then_some((distance, tip))
            })
            .collect();
        seen.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut tips: Vec<Tip> = vec![];
        for (_, tip) in seen {
            if !tips.contains(&tip) {
                tips.push(tip);
            }
        }
        tips
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
    /// A game partway into a first run, with the opening tips already seen,
    /// facing straight ahead (-z) with one creature of `kind` at `offset`.
    fn sighting(kind: usize, offset: glam::Vec3) -> Game {
        use crate::game::Enemy;
        let mut g = game();
        g.prefs.tips_seen = Tip::Move.bit() | Tip::Souls.bit() | Tip::Reload.bit();
        g.run.time = 20.;
        g.run.yaw = 0.;
        g.run.enemies = vec![Enemy::spawn(kind, g.run.pos + offset, 4, 0.)];
        g
    }
    #[test]
    fn a_new_creature_gets_one_note_when_it_comes_into_view() {
        use glam::Vec3;
        // A Bell Gargoyle 12 m ahead: its note, with the bestiary's lines.
        let mut g = sighting(9, Vec3::new(0., 0., -12.));
        assert_eq!(run_until_tip(&mut g), Some(Tip::Creature(9)));
        let tip = g.tip.unwrap().tip;
        assert_eq!(tip.heading(), "NEW CREATURE  /  BELL GARGOYLE");
        assert_eq!(
            tip.text(&g),
            "Marks a dive, then commits to it. Its great wings betray the attack."
        );
        assert!(g.prefs.tips_seen & tip.bit() != 0);
        // It lasts 7 seconds and doesn't come back.
        g.update_tips(7.1);
        assert_eq!(g.tip, None);
        assert_eq!(run_until_tip(&mut g), None);
        // Behind the player, or beyond 30 m, it isn't seen yet.
        for offset in [Vec3::new(0., 0., 12.), Vec3::new(20., 0., 0.), Vec3::new(0., 0., -31.)] {
            let mut g = sighting(11, offset);
            assert_eq!(run_until_tip(&mut g), None, "{offset}");
            assert_eq!(g.prefs.tips_seen & Tip::Creature(11).bit(), 0);
            // Turning to face it shows the note.
            if offset.length() < SIGHTING_RANGE {
                g.run.yaw = offset.x.atan2(-offset.z);
                assert_eq!(run_until_tip(&mut g), Some(Tip::Creature(11)), "{offset}");
            }
        }
        // The Ossuary Drudge has none; every other species has its own bit,
        // clear of the six field tips.
        assert_eq!(Tip::creature(0), None);
        let mut g = sighting(0, Vec3::new(0., 0., -8.));
        assert_eq!(run_until_tip(&mut g), None);
        let bits: Vec<u32> = (1..12).map(|k| Tip::creature(k).unwrap().bit()).collect();
        for (i, bit) in bits.iter().enumerate() {
            assert!(*bit >= 1 << 8 && bits[i + 1..].iter().all(|b| b != bit));
        }
        // Several new species at once queue, nearest first.
        let mut g = sighting(5, Vec3::new(1., 0., -9.));
        g.run.enemies.push(crate::game::Enemy::spawn(4, g.run.pos + Vec3::new(0., 0., -5.), 4, 0.));
        g.update_tips(0.);
        assert_eq!(g.tip.map(|t| t.tip), Some(Tip::Creature(4)));
        assert_eq!(g.tip_queue, vec![Tip::Creature(5)]);
    }
    #[test]
    fn walls_hide_a_creature_until_it_steps_into_the_open() {
        use glam::Vec3;
        // Inside the chapel's east wall, facing west through it.
        let mut g = sighting(7, Vec3::ZERO);
        g.run.pos = Vec3::new(-17., 1.65, -8.);
        g.run.yaw = -std::f32::consts::FRAC_PI_2;
        // In the cone and 7 m away, but the wall is between.
        g.run.enemies[0].pos = Vec3::new(-23., 0., -12.);
        assert!(!g.in_sight(&g.run.enemies[0]));
        assert_eq!(run_until_tip(&mut g), None);
        assert_eq!(g.prefs.tips_seen & Tip::Creature(7).bit(), 0);
        // Seen through the doorway the moment it steps across.
        g.run.enemies[0].pos = Vec3::new(-23., 0., -5.);
        assert!(g.in_sight(&g.run.enemies[0]));
        g.update_tips(1. / 60.);
        assert_eq!(g.tip.map(|t| t.tip), Some(Tip::Creature(7)));
        // A monument hides a creature too; out in the open court it doesn't.
        let mut g = sighting(5, Vec3::ZERO);
        g.run.pos = Vec3::new(0., 1.65, 2.);
        g.run.enemies[0].pos = Vec3::new(0., 0., -16.);
        assert!(!g.in_sight(&g.run.enemies[0]), "the central monument");
        assert_eq!(run_until_tip(&mut g), None);
        g.run.enemies[0].pos = Vec3::new(6., 0., -16.);
        assert!(g.in_sight(&g.run.enemies[0]));
        assert_eq!(run_until_tip(&mut g), Some(Tip::Creature(5)));
    }
    #[test]
    fn creature_notes_follow_the_field_tips_switch_and_skip_scripted_runs() {
        use glam::Vec3;
        let mut g = sighting(7, Vec3::new(0., 0., -10.));
        g.set_field_tips(false);
        assert_eq!(run_until_tip(&mut g), None);
        g.set_field_tips(true);
        assert_eq!(g.prefs.tips_seen, 0, "switching back on clears creature notes too");
        g.prefs.tips_seen = Tip::Move.bit();
        assert_eq!(run_until_tip(&mut g), Some(Tip::Creature(7)));
        // Seen creatures persist with the other tips.
        let bytes = serde_json::to_vec(&g.prefs).unwrap();
        let loaded = Preferences::from_json(&bytes).unwrap();
        assert!(loaded.tips_seen & Tip::Creature(7).bit() != 0);
        // Smoke, review and practice runs never show or record them.
        let mut scripted = Game::new(false);
        scripted.new_run();
        scripted.run.time = 20.;
        scripted.run.enemies = vec![crate::game::Enemy::spawn(7, scripted.run.pos + Vec3::new(0., 0., -10.), 4, 0.)];
        assert_eq!(run_until_tip(&mut scripted), None);
        assert_eq!(scripted.prefs.tips_seen, 0);
        let mut practice = sighting(7, Vec3::new(0., 0., -10.));
        practice.practice_backup = Some(practice.run.clone());
        assert_eq!(run_until_tip(&mut practice), None);
        assert_eq!(practice.prefs.tips_seen & Tip::Creature(7).bit(), 0);
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
