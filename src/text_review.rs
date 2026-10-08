//! Deterministic, disposable native UI gallery for typography review.
//! This mode renders the actual game UI and never loads or writes a player run.
use crate::{
    game::{
        Card, DAMAGE_MARK_LIFE, DamageMark, Enemy, Floater, Game, HIT_MARKER_LIFE, HitKind,
        HitMarker, Mode, Records,
    },
    weapons::WeaponKind,
};
use glam::Vec3;

#[derive(Clone, Copy)]
enum Screen {
    Title(bool),
    TitleRecords,
    /// The title with records and a chronicle of five runs.
    TitleChronicle,
    Hud(u8),
    /// A Bell Gargoyle's first-sighting note, in the fourth descent.
    CreatureNote,
    /// The Collector, with the chalice bound, or with the first-visit tip.
    Shop(bool, bool),
    Pack,
    /// The Collector after this descent, previewing the next one.
    ShopNext(u32),
    /// The Collector offering an area weapon with damage over time.
    ShopOffer,
    Binding(u8),
    Armory(usize, usize),
    Bestiary(usize),
    Powers(usize),
    /// A level-up while the controller is the last input used.
    PowersController,
    Pause,
    /// The pause ledger: 0 an early run with one power, 1 every power at
    /// full rank with a bound weapon, 2 practice.
    PauseLedger(u8),
    Settings,
    /// The journal's Display page: 0 at High, 1 at Ultra.
    Display(u8),
    /// The Collector a tenth of a second after the arena gave way to it,
    /// mid-fade.
    Fade,
    /// The controller cursor after D-pad presses: 0 the Collector, 1 the
    /// journal's sliders.
    PadMenu(u8),
    Controls(u8),
    Confirmation,
    Ending(bool),
    /// A first-descent death: small numbers and a common killer.
    EndingEarly,
}

pub struct Review {
    directory: String,
    screens: Vec<(String, Screen)>,
    index: usize,
    frame: u32,
    finished: bool,
    captures: Vec<String>,
    quit_check: bool,
}

impl Review {
    pub fn new(small: bool) -> Self {
        let quit_screen = if std::env::args().any(|a| a == "--quit-review-title") {
            Some(("title", Screen::Title(false)))
        } else if std::env::args().any(|a| a == "--quit-review-pause") {
            Some(("pause", Screen::Pause))
        } else {
            None
        };
        let mut directory = format!(
            "captures/legibility/{}",
            if small { "small" } else { "normal" }
        );
        if let Some((name, _)) = quit_screen {
            directory = format!("captures/quit/{name}");
        }
        std::fs::create_dir_all(&directory).expect("create native review directory");
        let mut screens = vec![
            ("title-new".into(), Screen::Title(false)),
            ("title-continue".into(), Screen::Title(true)),
            ("title-records".into(), Screen::TitleRecords),
            ("title-chronicle".into(), Screen::TitleChronicle),
            ("hud-new-run".into(), Screen::Hud(0)),
            ("hud-all-powers-boss".into(), Screen::Hud(1)),
            ("hud-reloading".into(), Screen::Hud(2)),
            ("hud-melee-endless".into(), Screen::Hud(3)),
            ("hud-kill-and-damage-arcs".into(), Screen::Hud(4)),
            ("hud-headshot-reduced-flashes".into(), Screen::Hud(5)),
            ("hud-field-of-view-90".into(), Screen::Hud(6)),
            ("hud-rebound-mouse-keys".into(), Screen::Hud(7)),
            ("hud-tip-move".into(), Screen::Hud(8)),
            ("hud-tip-bolt-with-notice".into(), Screen::Hud(9)),
            ("hud-controller-tip-move".into(), Screen::Hud(10)),
            ("hud-controller-opening".into(), Screen::Hud(11)),
            ("hud-offscreen-warnings".into(), Screen::Hud(12)),
            ("hud-size-80-busy".into(), Screen::Hud(13)),
            ("hud-size-100-busy".into(), Screen::Hud(14)),
            ("hud-size-110-busy".into(), Screen::Hud(15)),
            ("hud-controller-remapped".into(), Screen::Hud(16)),
            ("hud-size-120-busy".into(), Screen::Hud(17)),
            ("hud-size-130-busy".into(), Screen::Hud(18)),
            ("hud-reticle-100-ivory".into(), Screen::Hud(19)),
            ("hud-reticle-150-green".into(), Screen::Hud(20)),
            ("hud-reticle-200-yellow".into(), Screen::Hud(21)),
            ("hud-reticle-100-cyan".into(), Screen::Hud(22)),
            ("hud-reticle-150-magenta".into(), Screen::Hud(23)),
            ("hud-creature-note".into(), Screen::CreatureNote),
            ("collector".into(), Screen::Shop(false, false)),
            ("collector-chalice-bound".into(), Screen::Shop(true, false)),
            ("collector-tip".into(), Screen::Shop(false, true)),
            ("collector-next-descent-04".into(), Screen::ShopNext(3)),
            ("collector-next-descent-08".into(), Screen::ShopNext(7)),
            ("collector-next-descent-14".into(), Screen::ShopNext(13)),
            ("pack".into(), Screen::Pack),
            ("collector-venom-splash-offer".into(), Screen::ShopOffer),
            ("binding-available-tooltip".into(), Screen::Binding(0)),
            ("binding-locked-tooltip".into(), Screen::Binding(1)),
            ("binding-owned-tooltip".into(), Screen::Binding(2)),
            ("binding-ascension-available".into(), Screen::Binding(3)),
            ("binding-complete".into(), Screen::Binding(4)),
        ];
        for (index, kind) in WeaponKind::ALL.iter().enumerate() {
            screens.push((
                format!("armory-{index:02}-{:?}", kind).to_lowercase(),
                Screen::Armory(index, index % 4),
            ));
        }
        // The same long weapon title on all four card stocks.
        for rarity in 0..4 {
            screens.push((format!("card-rarity-{rarity}"), Screen::Armory(18, rarity)));
        }
        for kind in 0..12 {
            screens.push((format!("bestiary-{kind:02}"), Screen::Bestiary(kind)));
        }
        for group in 0..4 {
            screens.push((format!("powers-{group}"), Screen::Powers(group)));
        }
        screens.push(("powers-controller".into(), Screen::PowersController));
        screens.extend([
            ("pause".into(), Screen::Pause),
            ("pause-ledger-early".into(), Screen::PauseLedger(0)),
            ("pause-ledger-full".into(), Screen::PauseLedger(1)),
            ("pause-ledger-practice".into(), Screen::PauseLedger(2)),
            ("settings".into(), Screen::Settings),
            ("display".into(), Screen::Display(0)),
            ("display-ultra".into(), Screen::Display(1)),
            ("fade-into-collector".into(), Screen::Fade),
            ("pad-collector-dpad".into(), Screen::PadMenu(0)),
            ("pad-journal-slider".into(), Screen::PadMenu(1)),
            ("controls".into(), Screen::Controls(0)),
            ("controls-waiting-for-key".into(), Screen::Controls(1)),
            ("controls-azerty-swap".into(), Screen::Controls(2)),
            ("controller-buttons".into(), Screen::Controls(3)),
            ("controller-waiting-for-button".into(), Screen::Controls(4)),
            ("controller-remapped".into(), Screen::Controls(5)),
            ("new-run-confirmation".into(), Screen::Confirmation),
            ("death".into(), Screen::Ending(false)),
            ("victory".into(), Screen::Ending(true)),
            ("death-first-descent".into(), Screen::EndingEarly),
        ]);
        if let Some((name, screen)) = quit_screen {
            screens = vec![(name.into(), screen)];
        }
        // GRAVEWAKE_REVIEW_ONLY=hud-size,pack keeps fixtures whose names
        // contain any of the comma-separated parts.
        if let Ok(only) = std::env::var("GRAVEWAKE_REVIEW_ONLY") {
            screens.retain(|(name, _)| only.split(',').any(|part| name.contains(part)));
        }
        Self {
            directory,
            screens,
            index: 0,
            frame: 0,
            finished: false,
            captures: vec![],
            quit_check: quit_screen.is_some(),
        }
    }

    pub fn step(&mut self, game: &mut Game, global_frame: u32) -> Option<String> {
        if self.finished {
            return None;
        }
        let screen = self.screens[self.index].1;
        let length = match screen {
            Screen::Pack => 184,
            Screen::Binding(0..=2) => 64,
            _ => 34,
        };
        self.frame += 1;
        if self.frame > length {
            assert!(
                !self.quit_check,
                "Quit button failed to close the native application"
            );
            self.index += 1;
            self.frame = 1;
            if self.index == self.screens.len() {
                self.finished = true;
                let manifest = serde_json::json!({
                    "screens": self.screens.len(),
                    "captures": self.captures,
                    "player_save_loaded": false,
                    "player_save_written": false,
                    "simulation": "frozen, disposable review fixtures",
                });
                std::fs::write(
                    format!("{}/manifest.json", self.directory),
                    serde_json::to_vec_pretty(&manifest).unwrap(),
                )
                .expect("write typography review manifest");
                println!(
                    "TEXT REVIEW PASS: {} native screenshots covering {} UI fixtures; player saves untouched; {}",
                    self.captures.len(),
                    self.screens.len(),
                    self.directory
                );
                return None;
            }
        }
        let screen = self.screens[self.index].1;
        if self.frame == 1 {
            Self::setup(game, screen);
        }
        if matches!(screen, Screen::Fade) && self.frame == 27 {
            game.mode = Mode::Shop;
        }
        // Advance UI/preview animation while holding combat and all game values fixed.
        game.elapsed = global_frame as f32 / 30.;
        game.fps = 144.;
        let suffix = match screen {
            Screen::Pack => match self.frame {
                20 => Some("sealed"),
                105 => Some("unrevealed"),
                150 => Some("revealed"),
                180 => Some("selected"),
                _ => None,
            },
            Screen::Binding(0..=2) if self.frame == 60 => Some(""),
            Screen::Binding(0..=2) => None,
            _ if self.frame == 30 => Some(""),
            _ => None,
        };
        suffix.map(|suffix| {
            let stem = &self.screens[self.index].0;
            let filename = if suffix.is_empty() {
                format!("{:02}-{stem}.png", self.index)
            } else {
                format!("{:02}-{stem}-{suffix}.png", self.index)
            };
            self.captures.push(filename.clone());
            format!("{}/{filename}", self.directory)
        })
    }

    pub fn finished(&self) -> bool {
        self.finished
    }

    /// Controller D-pad presses for this frame on the controller-cursor
    /// fixtures, or `None` on other fixtures (no controller cursor).
    pub fn pad_presses(&self) -> Option<Vec<crate::gamepad::Button>> {
        use crate::gamepad::Button::*;
        let Screen::PadMenu(kind) = self.screens.get(self.index)?.1 else {
            return None;
        };
        let presses: &[(u32, crate::gamepad::Button)] = match kind {
            // The first press lands near the centre; then over and down.
            0 => &[(8, DPadDown), (14, DPadRight), (20, DPadDown)],
            // The first press lands on field of view, the slider nearest
            // the centre; then two steps along it (70° to 73°).
            _ => &[(8, DPadDown), (18, DPadRight), (23, DPadRight)],
        };
        Some(
            presses
                .iter()
                .filter(|(frame, _)| *frame == self.frame)
                .map(|(_, button)| *button)
                .collect(),
        )
    }

    pub fn input(&self, input: &mut egui::RawInput) {
        let screen = self.screens[self.index].1;
        if matches!(screen, Screen::PadMenu(_)) {
            // The controller cursor is the only pointer.
            return;
        }
        let click = match screen {
            Screen::Title(_) if self.quit_check && matches!(self.frame, 32 | 33) => {
                Some((275.5, 809.5, self.frame == 32))
            }
            Screen::Pause if self.quit_check && matches!(self.frame, 32 | 33) => {
                Some((720., 581.5, self.frame == 32))
            }
            Screen::Pack => match self.frame {
                24 | 25 => Some((720., 690., self.frame == 24)),
                111 | 112 => Some((720., 728., self.frame == 111)),
                158 | 159 => Some((380., 582., self.frame == 158)),
                _ => None,
            },
            _ => None,
        };
        let hover = match screen {
            Screen::Binding(0 | 2) => Some((587., 352.)),
            Screen::Binding(1) => Some((587., 425.)),
            _ => None,
        };
        let point = click
            .map(|(x, y, _)| (x, y))
            .or(hover)
            .unwrap_or((1435., 5.));
        let rect = input.screen_rect.expect("native review screen rect");
        let scale = (rect.width() / 1440.).min(rect.height() / 900.);
        let pos = egui::pos2(
            rect.min.x + (rect.width() - 1440. * scale) * 0.5 + point.0 * scale,
            rect.min.y + point.1 * scale,
        );
        input.events.push(egui::Event::PointerMoved(pos));
        if let Some((_, _, pressed)) = click {
            input.events.push(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
    }

    /// Fire on RB, melee on the right stick, dodge on B and Ember Bolt on
    /// LT, which moves sprint to Y.
    fn remap_controller(game: &mut Game) {
        use crate::gamepad::{Button, PadAction};
        for (action, slot, button) in [
            (PadAction::Fire, 0, Button::RightBumper),
            (PadAction::Melee, 0, Button::RightStick),
            (PadAction::Dodge, 0, Button::East),
            (PadAction::Bolt, 0, Button::LeftTrigger),
        ] {
            game.prefs.pad_bindings.assign(action, slot, button).unwrap();
        }
    }
    fn setup(game: &mut Game, screen: Screen) {
        *game = Game::new(false);
        game.has_save = false;
        game.run.gold = 98765;
        game.run.time = 3894.;
        game.run.kills = 12345;
        game.run.wave = 12;
        game.run.enemies = vec![Enemy::spawn(3, Vec3::new(0., 0., -6.), 12, 0.)];
        game.run.weapon = Card {
            kind: WeaponKind::GrenadeLauncher,
            rarity: 3,
            ..Card::starter()
        };
        game.run.ammo = game.run.weapon.capacity();
        game.run.inventory = WeaponKind::ALL
            .iter()
            .enumerate()
            .map(|(i, kind)| Card {
                kind: *kind,
                rarity: i % 4,
                ..Card::starter()
            })
            .collect();
        match screen {
            Screen::Title(saved) => {
                game.mode = Mode::Title;
                game.has_save = saved;
            }
            Screen::TitleRecords => {
                game.mode = Mode::Title;
                game.has_save = true;
                game.records = Records {
                    runs: 14,
                    deepest: 19,
                    most_souls: 1834,
                    victories: 2,
                    fastest_victory: Some(1694.),
                    ..Default::default()
                };
            }
            Screen::TitleChronicle => {
                use crate::game::{Attack, Card, Cause, Ending, PastRun, WeaponKind};
                game.mode = Mode::Title;
                game.has_save = true;
                let past = |number, descent, ending, souls, time, level, kind, rarity| PastRun {
                    number,
                    descent,
                    ending,
                    endless: descent > crate::survival::DESCENTS,
                    souls,
                    time,
                    level,
                    weapon: Card {
                        kind,
                        rarity,
                        ..Card::starter()
                    }
                    .name(),
                };
                let blow = |kind, attack| Ending::Slain(Some(Cause { kind, attack }));
                game.records = Records {
                    runs: 14,
                    deepest: 19,
                    most_souls: 1834,
                    victories: 2,
                    fastest_victory: Some(1694.),
                    history: vec![
                        past(14, 7, blow(8, Attack::Bolt), 412, 1102., 11, WeaponKind::Repeater, 1),
                        past(13, 3, Ending::Abandoned, 96, 388., 4, WeaponKind::Pistol, 0),
                        past(12, 19, blow(3, Attack::Slam), 1834, 3311., 31, WeaponKind::Double, 3),
                        past(11, 12, Ending::Victory, 1240, 1694., 24, WeaponKind::Double, 2),
                        past(10, 2, blow(5, Attack::Strike), 38, 241., 2, WeaponKind::Pistol, 0),
                    ],
                };
            }
            Screen::Hud(kind) => {
                game.mode = Mode::Arena;
                game.run.survival.remaining = 180;
                game.run.survival.level = 99;
                game.run.survival.xp = 798;
                game.show_fps = true;
                if kind == 0 || (4..=12).contains(&kind) || kind == 16 || kind >= 19 {
                    game.run.time = 0.;
                    game.run.wave = 1;
                    game.run.enemies = vec![Enemy::spawn(0, Vec3::new(0., 0., -6.), 1, 0.)];
                    game.run.gold = 0;
                    game.run.survival = Default::default();
                    game.run.weapon = Card::starter();
                    game.run.ammo = game.run.weapon.capacity();
                } else {
                    game.run.survival.ranks = [5; 10];
                    game.run.hp = 200.;
                    game.run.armor = 999.;
                    game.run.chalice = true;
                    game.run.mana = 120.;
                    game.dash_cd = 0.5;
                    game.notice = "HEADSHOT / THE SKULL BREAKS FREE".into();
                    game.notice_time = 2.;
                    game.floaters.push(Floater {
                        pos: Vec3::new(-1.5, 2.3, -4.),
                        text: "12345!".into(),
                        life: 1.,
                    });
                }
                if kind >= 19 {
                    // The reticle over pale bone: a drudge's ribcage fills
                    // the centre, past the opening banner.
                    use crate::game::ReticleColor;
                    game.run.time = 30.;
                    let player = game.run.pos;
                    game.run.yaw = 0.;
                    game.run.pitch = -0.2;
                    game.run.enemies = vec![Enemy::spawn(0, player + Vec3::new(0., -player.y, -1.9), 1, 0.)];
                    (game.prefs.reticle_size, game.prefs.reticle_color) = match kind {
                        19 => (1., ReticleColor::Ivory),
                        20 => (1.5, ReticleColor::Green),
                        21 => (2., ReticleColor::Yellow),
                        22 => (1., ReticleColor::Cyan),
                        _ => (1.5, ReticleColor::Magenta),
                    };
                }
                if kind == 2 {
                    game.reload = 1.;
                    game.run.hp = 8.;
                }
                if kind == 6 {
                    game.run.time = 30.;
                    game.prefs.fov = 90.;
                }
                if (10..=11).contains(&kind) {
                    // Controller prompts: the first tip with a melee weapon
                    // and a bound chalice, then the opening reminder.
                    game.device = crate::controls::Device::Controller;
                    game.run.chalice = true;
                    if kind == 10 {
                        game.run.weapon.kind = WeaponKind::TwinDaggers;
                    }
                }
                if kind == 11 {
                    game.run.time = 0.5;
                } else if (8..=10).contains(&kind) {
                    // The first tip of a first run, and a later tip with a
                    // notice above it.
                    use crate::tips::{ActiveTip, Tip};
                    let tip = if kind == 9 { Tip::Bolt } else { Tip::Move };
                    game.tip = Some(ActiveTip { tip, life: 5. });
                    game.run.time = if kind == 9 { 30. } else { 0.5 };
                    if kind == 9 {
                        game.run.chalice = true;
                        game.notice = "THE HOLLOW CHALICE ANSWERS".into();
                        game.notice_time = 2.;
                    }
                }
                if kind == 7 {
                    // The longest labels: mouse buttons, an arrow-key layout
                    // and a bound chalice, during the opening banner.
                    use crate::controls::{Action, Trigger};
                    use winit::{event::MouseButton, keyboard::KeyCode};
                    game.run.chalice = true;
                    for (action, trigger) in [
                        (Action::Dodge, Trigger::Mouse(MouseButton::Middle)),
                        (Action::Reload, Trigger::Mouse(MouseButton::Back)),
                        (Action::Bolt, Trigger::Mouse(MouseButton::Right)),
                        (Action::Forward, Trigger::Key(KeyCode::ArrowUp)),
                        (Action::Back, Trigger::Key(KeyCode::ArrowDown)),
                        (Action::Left, Trigger::Key(KeyCode::ArrowLeft)),
                        (Action::Right, Trigger::Key(KeyCode::ArrowRight)),
                    ] {
                        game.prefs.bindings.assign(action, trigger, None).unwrap();
                    }
                }
                if (4..=5).contains(&kind) {
                    // Past the opening banner, with combat feedback frozen.
                    game.run.time = 30.;
                    game.hurt = 0.3;
                    game.prefs.reduce_flashes = kind == 5;
                    game.hit_marker = Some(HitMarker {
                        kind: if kind == 4 {
                            HitKind::Kill
                        } else {
                            HitKind::Head
                        },
                        life: HIT_MARKER_LIFE,
                    });
                    game.damage_marks = [(1.1, 1.), (2.6, 0.6), (-1.9, 0.3)]
                        .into_iter()
                        .take(if kind == 4 { 3 } else { 1 })
                        .map(|(bearing, fade)| DamageMark {
                            bearing,
                            life: DAMAGE_MARK_LIFE * fade,
                        })
                        .collect();
                }
                if kind == 12 {
                    // Wind-ups out of view: a dive behind, a volley to the
                    // right and a blink behind-left at three stages, next to
                    // a damage arc and an in-view cast that needs no pointer.
                    game.run.time = 30.;
                    let player = game.run.pos;
                    game.run.yaw = 0.;
                    game.run.enemies = [(4, 2.5, 6., 0.15), (8, 9., -1., 0.5), (11, -7., 5., 0.8), (2, 1., -7., 0.4)]
                        .into_iter()
                        .map(|(kind, x, z, left)| {
                            let mut e = Enemy::spawn(kind, player + Vec3::new(x, -player.y, z), 4, 0.);
                            e.ai.warning = crate::encounters::warning_time(kind) * left;
                            // Blinks land beside the player; the rest aim at them.
                            let side = if kind == 11 { 2.8 } else { 0. };
                            e.ai.target = Vec3::new(player.x + side, 0., player.z);
                            e
                        })
                        .collect();
                    game.damage_marks = vec![DamageMark {
                        bearing: -1.4,
                        life: DAMAGE_MARK_LIFE * 0.8,
                    }];
                }
                if (13..=15).contains(&kind) || (17..=18).contains(&kind) {
                    // The busiest HUD at each size: every power, the boss bar
                    // under the last-threat bearing, a bound chalice, the longest
                    // arena tip under a notice, a damage arc and an unseen dive.
                    game.prefs.hud_scale = match kind {
                        13 => 0.8,
                        14 => 1.,
                        15 => 1.1,
                        17 => 1.2,
                        _ => 1.3,
                    };
                    game.run.time = 30.;
                    game.run.survival.remaining = 0;
                    let player = game.run.pos;
                    let mut diver = Enemy::spawn(4, player + Vec3::new(-2., -player.y, 5.), 12, 0.);
                    diver.ai.warning = crate::encounters::warning_time(4) * 0.3;
                    diver.ai.target = Vec3::new(player.x, 0., player.z);
                    game.run.enemies.push(diver);
                    game.tip = Some(crate::tips::ActiveTip {
                        tip: crate::tips::Tip::Souls,
                        life: 5.,
                    });
                    // The compact sizes show an arc straight ahead, toward the
                    // boss inside the descent panel.
                    game.damage_marks = vec![DamageMark {
                        bearing: if kind >= 17 { game.run.yaw } else { 1.3 },
                        life: DAMAGE_MARK_LIFE,
                    }];
                }
                if kind == 16 {
                    // Remapped controller prompts on the HUD and in the
                    // opening reminder, with a melee weapon and a chalice.
                    game.device = crate::controls::Device::Controller;
                    game.run.chalice = true;
                    game.run.time = 0.5;
                    game.run.weapon.kind = WeaponKind::TwinDaggers;
                    Self::remap_controller(game);
                }
                if kind == 3 {
                    game.run.weapon.kind = WeaponKind::TwinDaggers;
                    game.run.survival.endless = true;
                    game.run.wave = 123;
                }
            }
            Screen::Shop(bound, tip) => {
                game.mode = Mode::Shop;
                game.run.chalice = bound;
                if tip {
                    game.tip = Some(crate::tips::ActiveTip {
                        tip: crate::tips::Tip::Collector,
                        life: 20.,
                    });
                }
                game.run.offer = game.run.weapon.clone();
                game.run.draws = 12;
                game.run.pack_buys = 8;
            }
            Screen::ShopNext(wave) => {
                game.mode = Mode::Shop;
                game.run.wave = wave;
                game.run.survival.endless = wave > crate::survival::DESCENTS;
                game.run.offer = game.run.weapon.clone();
                game.run.draws = 12;
                game.run.pack_buys = 8;
            }
            Screen::ShopOffer => {
                game.mode = Mode::Shop;
                game.run.offer = Card {
                    kind: WeaponKind::PlagueCenser,
                    rarity: 3,
                    ..Card::starter()
                };
                game.run.draws = 12;
                game.run.pack_buys = 8;
            }
            Screen::Pack => {
                game.mode = Mode::Shop;
                game.open_pack();
                game.run.choices = [
                    WeaponKind::GrenadeLauncher,
                    WeaponKind::Dragonbreath,
                    WeaponKind::TwinDaggers,
                ]
                .into_iter()
                .enumerate()
                .map(|(i, kind)| Card {
                    kind,
                    rarity: i + 1,
                    ..Card::starter()
                })
                .collect();
            }
            Screen::Binding(kind) => {
                game.mode = Mode::Tree;
                game.run.weapon.paths = match kind {
                    2 => [2, 1, 3],
                    3 => [5, 2, 4],
                    4 => [5; 3],
                    _ => [0; 3],
                };
                game.run.weapon.major = kind == 4;
            }
            Screen::Armory(kind, rarity) => {
                game.mode = Mode::Collection;
                game.collection_kind = kind;
                game.collection_group = WeaponKind::ALL[kind].spec().group;
                game.collection_rarity = rarity;
            }
            Screen::Bestiary(kind) => {
                game.mode = Mode::Bestiary;
                game.bestiary_index = kind;
            }
            Screen::Powers(group) => {
                game.mode = Mode::LevelUp;
                game.run.survival.level = 49;
                game.run.survival.pending = 1;
                game.run.survival.ranks = [4; 10];
                game.run.survival.choices = (0..3).map(|i| (group * 3 + i) % 10).collect();
            }
            Screen::PowersController => {
                game.mode = Mode::LevelUp;
                game.device = crate::controls::Device::Controller;
                game.run.survival.level = 3;
                game.run.survival.pending = 1;
                game.run.survival.choices = vec![0, 6, 9];
            }
            Screen::Pause => game.mode = Mode::Paused,
            Screen::CreatureNote => {
                use crate::tips::{ActiveTip, Tip};
                game.mode = Mode::Arena;
                game.run.wave = 4;
                game.run.time = 412.;
                game.run.gold = 240;
                game.run.weapon = Card::starter();
                game.run.ammo = game.run.weapon.capacity();
                game.run.survival = Default::default();
                game.run.survival.remaining = 31;
                game.run.enemies = vec![
                    Enemy::spawn(9, Vec3::new(1.5, 0., -9.), 4, 0.),
                    Enemy::spawn(0, Vec3::new(-3., 0., -12.), 4, 0.3),
                ];
                game.tip = Some(ActiveTip {
                    tip: Tip::Creature(9),
                    life: 5.,
                });
            }
            Screen::PauseLedger(n) => {
                use crate::game::RunStats;
                game.mode = Mode::Paused;
                game.run.survival = Default::default();
                if n == 0 {
                    game.run.wave = 2;
                    game.run.kills = 41;
                    game.run.time = 263.;
                    game.run.gold = 135;
                    game.run.weapon = Card::starter();
                    game.run.survival.level = 2;
                    game.run.survival.xp = 9;
                    game.run.survival.ranks[6] = 1;
                    game.run.stats = RunStats {
                        headshots: 12,
                        damage_dealt: 2604.,
                        damage_taken: 57.,
                        ..Default::default()
                    };
                } else {
                    game.notice = "CONTROLLER DISCONNECTED / PAUSED".into();
                    game.notice_time = 2.;
                    game.run.armor = 30.;
                    game.run.hp = 164.;
                    game.run.weapon.paths = [5, 5, 3];
                    game.run.weapon.major = true;
                    game.run.survival.level = 52;
                    game.run.survival.xp = 311;
                    game.run.survival.ranks = [5; 10];
                    game.run.stats = RunStats {
                        headshots: 2876,
                        damage_dealt: 912345.,
                        damage_taken: 3530.,
                        ..Default::default()
                    };
                }
                if n == 2 {
                    let card = Card {
                        kind: WeaponKind::ALL[20],
                        rarity: 2,
                        ..Card::starter()
                    };
                    game.practice(card);
                    game.mode = Mode::Paused;
                    game.notice_time = 0.;
                }
            }
            Screen::PadMenu(0) => {
                game.mode = Mode::Shop;
                game.run.offer = game.run.weapon.clone();
                game.run.draws = 12;
                game.run.pack_buys = 8;
                game.device = crate::controls::Device::Controller;
            }
            Screen::PadMenu(_) => {
                game.mode = Mode::Paused;
                game.settings = true;
                game.device = crate::controls::Device::Controller;
            }
            Screen::Settings => {
                game.mode = Mode::Paused;
                game.settings = true;
                game.show_fps = true;
                game.prefs.volume = 1.;
                game.prefs.frame_limit = 144;
            }
            Screen::Fade => {
                game.mode = Mode::Arena;
                game.notice_time = 0.;
            }
            Screen::Display(kind) => {
                game.mode = Mode::Paused;
                game.settings = true;
                game.journal_page = crate::game::JournalPage::Display;
                game.show_fps = true;
                game.prefs.frame_limit = 144;
                if kind == 1 {
                    game.prefs.fidelity = crate::fidelity::Fidelity::Ultra;
                }
            }
            Screen::Controls(kind) => {
                use crate::controls::{Action, Trigger};
                use winit::{event::MouseButton, keyboard::KeyCode};
                game.mode = Mode::Paused;
                game.settings = true;
                game.journal_page = crate::game::JournalPage::Keyboard;
                game.rebinding = Some(Action::Dodge);
                if kind == 2 {
                    // An AZERTY keyboard, then melee moved to the dodge key's
                    // place and dodge moved to the right mouse button.
                    for (code, glyph) in [
                        (KeyCode::KeyW, 'z'),
                        (KeyCode::KeyA, 'q'),
                        (KeyCode::KeyQ, 'a'),
                    ] {
                        game.prefs.bindings.learn_glyph(code, glyph);
                    }
                    game.bind(Trigger::Mouse(MouseButton::Right), None);
                    game.rebinding = Some(Action::Melee);
                    game.bind(Trigger::Mouse(MouseButton::Right), None);
                } else if kind == 0 {
                    game.rebinding = None;
                }
                if kind >= 3 {
                    // The Controller page: defaults, waiting on Reload's
                    // second slot, then a remap with swaps and its note.
                    use crate::gamepad::{Button, PadAction};
                    game.journal_page = crate::game::JournalPage::Controller;
                    game.rebinding = None;
                    game.pad_rebinding = (kind == 4).then_some((PadAction::Reload, 1));
                    if kind == 5 {
                        Self::remap_controller(game);
                        game.pad_rebinding = Some((PadAction::Reload, 1));
                        game.bind_pad(Button::South);
                    }
                }
            }
            Screen::Confirmation => {
                game.mode = Mode::Title;
                game.has_save = true;
                game.confirm_new_run = true;
            }
            Screen::Ending(win) => {
                use crate::game::{Attack, Cause, RunStats};
                game.mode = if win { Mode::Victory } else { Mode::Dead };
                game.run_records = if win {
                    vec!["MOST SOULS", "FASTEST VICTORY"]
                } else {
                    vec!["DEEPEST DESCENT", "MOST SOULS"]
                };
                game.run.survival.level = 47;
                game.run.survival.ranks = [5, 5, 4, 5, 3, 5, 4, 5, 2, 4];
                let mut taken_from = vec![120.; 12];
                taken_from[3] = 2210.;
                game.run.stats = RunStats {
                    headshots: 2876,
                    damage_dealt: 912345.,
                    damage_taken: 3530.,
                    taken_from,
                    last_hit: Some(Cause {
                        kind: 3,
                        attack: Attack::Slam,
                    }),
                };
            }
            Screen::EndingEarly => {
                use crate::game::{Attack, Cause, RunStats};
                game.mode = Mode::Dead;
                game.run.wave = 1;
                game.run.kills = 23;
                game.run.time = 151.;
                game.run.weapon = Card::starter();
                game.run.survival = Default::default();
                game.run.survival.level = 3;
                game.run.survival.ranks[0] = 1;
                game.run.survival.ranks[6] = 1;
                game.run.stats = RunStats {
                    headshots: 7,
                    damage_dealt: 1480.,
                    damage_taken: 100.,
                    taken_from: vec![38., 62.],
                    last_hit: Some(Cause {
                        kind: 1,
                        attack: Attack::Strike,
                    }),
                };
            }
        }
    }
}
