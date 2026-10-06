//! Deterministic, disposable native UI gallery for typography review.
//! This mode renders the actual game UI and never loads or writes a player run.
use crate::{
    game::{Card, Enemy, Floater, Game, Mode},
    weapons::WeaponKind,
};
use glam::Vec3;

#[derive(Clone, Copy)]
enum Screen {
    Title(bool),
    Hud(u8),
    Shop(bool),
    Pack,
    Binding(u8),
    Armory(usize, usize),
    Bestiary(usize),
    Powers(usize),
    Pause,
    Settings,
    Confirmation,
    Ending(bool),
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
            ("hud-new-run".into(), Screen::Hud(0)),
            ("hud-all-powers-boss".into(), Screen::Hud(1)),
            ("hud-reloading".into(), Screen::Hud(2)),
            ("hud-melee-endless".into(), Screen::Hud(3)),
            ("collector".into(), Screen::Shop(false)),
            ("collector-chalice-bound".into(), Screen::Shop(true)),
            ("pack".into(), Screen::Pack),
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
        screens.extend([
            ("pause".into(), Screen::Pause),
            ("settings".into(), Screen::Settings),
            ("new-run-confirmation".into(), Screen::Confirmation),
            ("death".into(), Screen::Ending(false)),
            ("victory".into(), Screen::Ending(true)),
        ]);
        if let Some((name, screen)) = quit_screen {
            screens = vec![(name.into(), screen)];
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

    pub fn input(&self, input: &mut egui::RawInput) {
        let screen = self.screens[self.index].1;
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
            Screen::Hud(kind) => {
                game.mode = Mode::Arena;
                game.run.survival.remaining = 180;
                game.run.survival.level = 99;
                game.run.survival.xp = 798;
                game.show_fps = true;
                if kind == 0 {
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
                if kind == 2 {
                    game.reload = 1.;
                    game.run.hp = 8.;
                }
                if kind == 3 {
                    game.run.weapon.kind = WeaponKind::TwinDaggers;
                    game.run.survival.endless = true;
                    game.run.wave = 123;
                }
            }
            Screen::Shop(bound) => {
                game.mode = Mode::Shop;
                game.run.chalice = bound;
                game.run.offer = game.run.weapon.clone();
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
            Screen::Pause => game.mode = Mode::Paused,
            Screen::Settings => {
                game.mode = Mode::Paused;
                game.settings = true;
                game.show_fps = true;
                game.prefs.volume = 1.;
            }
            Screen::Confirmation => {
                game.mode = Mode::Title;
                game.has_save = true;
                game.confirm_new_run = true;
            }
            Screen::Ending(win) => game.mode = if win { Mode::Victory } else { Mode::Dead },
        }
    }
}
