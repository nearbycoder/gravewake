mod anatomy;
mod anatomy_review;
mod architecture_assets;
mod audio;
mod controls;
mod dismemberment;
mod encounters;
mod fullscreen_review;
mod input_review;
mod enemy_assets;
mod environment_assets;
mod game;
mod gamepad;
mod guns;
mod model_review;
mod motion;
mod music;
mod pacing;
mod pacing_review;
mod perf;
mod renderer;
mod scene;
mod survival;
mod text_review;
mod tips;
mod ui;
mod watchdog;
mod weapon_assets;
mod weapons;
mod world_layout;
mod world_review;
use game::{Game, Mode};
use std::{collections::HashSet, sync::Arc, time::Instant};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{DeviceEvent, ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window, WindowId},
};
struct App {
    window: Option<Arc<Window>>,
    renderer: Option<renderer::Renderer>,
    egui_state: Option<egui_winit::State>,
    ctx: egui::Context,
    game: Game,
    bones: scene::Bones,
    audio: audio::Audio,
    /// Bound keys and mouse buttons currently held.
    held: HashSet<controls::Trigger>,
    pad: gamepad::Pad,
    pad_events: Vec<egui::Event>,
    gamepad_smoke: bool,
    mouse_fire: bool,
    /// Sprint and fire pressed since the last frame. A press and release
    /// that both land between two frames still count for one frame, so a
    /// quick tap toggles sprint or fires.
    sprint_pressed: bool,
    fire_pressed: bool,
    pointer_ignored: bool,
    /// A scripted run has logged a lost or outdated surface.
    surface_error_logged: bool,
    /// Last desktop pointer position, so prompts switch back to the keyboard
    /// only when the mouse really moves, not when a window opens under it.
    last_pointer: Option<winit::dpi::PhysicalPosition<f64>>,
    focused: bool,
    last: Instant,
    captured: bool,
    occluded: bool,
    smoke: bool,
    review: bool,
    armory_review: bool,
    gun_review: bool,
    shader_review: bool,
    anatomy_review: bool,
    anatomy_proof: Option<anatomy_review::Review>,
    survival_review: bool,
    text_review: Option<text_review::Review>,
    model_review: Option<model_review::Review>,
    world_review: Option<world_review::Review>,
    fullscreen_review: Option<fullscreen_review::Review>,
    pacing_review: Option<pacing_review::Review>,
    input_review: Option<input_review::Review>,
    /// The frame limit's schedule.
    pacer: pacing::Pacer,
    benchmark: bool,
    perf: perf::Benchmark,
    shader_frame_times: Vec<f32>,
    review_audio: Vec<f32>,
    frames: u32,
    stage: usize,
    stage_frames: u32,
}
impl App {
    fn quit_game(&mut self, event_loop: &ActiveEventLoop) {
        self.game.quit_requested = false;
        self.game.save_preferences();
        self.game.save_performance();
        if !self.game.save() {
            if self.game.mode == Mode::Arena {
                self.game.mode = Mode::Paused;
            }
            self.sync_cursor();
            return;
        }
        if let Some(window) = &self.window {
            let _ = window.set_cursor_grab(CursorGrabMode::None);
            window.set_cursor_visible(true);
        }
        println!("QUIT: save completed; closing game");
        event_loop.exit();
    }
    /// Match the window to `prefs.fullscreen`. Scripted runs never save
    /// preferences, so they keep their fixed window sizes.
    fn apply_fullscreen(&self) {
        if let Some(w) = &self.window {
            w.set_fullscreen(
                self.game
                    .prefs
                    .fullscreen
                    .then_some(winit::window::Fullscreen::Borderless(None)),
            );
        }
    }
    /// F11: switch the window in or out of fullscreen and save the choice.
    fn toggle_fullscreen(&mut self) {
        let Some(w) = &self.window else { return };
        self.game.prefs.fullscreen = w.fullscreen().is_none();
        self.apply_fullscreen();
        self.game.save_preferences();
    }
    /// Frames per second the loop may draw now; 0 for no limit. Scripted
    /// runs draw unlimited, apart from the pacing review's stages.
    fn frame_limit(&self) -> u32 {
        if let Some(review) = &self.pacing_review {
            return review.limit();
        }
        if self.smoke || self.review {
            return 0;
        }
        pacing::effective_limit(self.game.prefs.frame_limit, self.focused)
    }
    fn new(smoke: bool, review: bool) -> Self {
        let ctx = egui::Context::default();
        ui::configure(&ctx);
        Self {
            window: None,
            renderer: None,
            egui_state: None,
            ctx,
            game: Game::new(!smoke && !review),
            bones: scene::Bones::new(),
            audio: audio::Audio::new(),
            held: HashSet::new(),
            // The input review keeps a physical controller out of its checks.
            pad: gamepad::Pad::new(
                !smoke && !review && !std::env::args().any(|a| a == "--input-review"),
            ),
            pad_events: vec![],
            gamepad_smoke: smoke && std::env::args().any(|a| a == "--gamepad"),
            mouse_fire: false,
            sprint_pressed: false,
            fire_pressed: false,
            pointer_ignored: false,
            surface_error_logged: false,
            last_pointer: None,
            focused: true,
            last: Instant::now(),
            captured: false,
            occluded: false,
            smoke,
            review,
            armory_review: std::env::args().any(|a| a == "--armory-review"),
            gun_review: std::env::args().any(|a| a == "--gun-review"),
            shader_review: std::env::args().any(|a| a == "--shader-review"),
            anatomy_review: std::env::args().any(|a| a == "--anatomy-review"),
            anatomy_proof: std::env::args()
                .any(|a| a == "--anatomy-review")
                .then(anatomy_review::Review::new),
            survival_review: std::env::args().any(|a| a == "--survival-review"),
            text_review: std::env::args()
                .any(|a| a == "--text-review")
                .then(|| text_review::Review::new(std::env::args().any(|a| a == "--review-small"))),
            model_review: std::env::args().any(|a| a == "--model-review").then(|| {
                model_review::Review::new(std::env::args().any(|a| a == "--model-before"))
            }),
            world_review: std::env::args()
                .any(|a| a == "--world-review")
                .then(world_review::Review::new),
            fullscreen_review: std::env::args()
                .any(|a| a == "--fullscreen-review")
                .then(fullscreen_review::Review::new),
            input_review: std::env::args()
                .any(|a| a == "--input-review")
                .then(input_review::Review::new),
            pacing_review: std::env::args()
                .any(|a| a == "--pacing-review")
                .then(pacing_review::Review::new),
            pacer: pacing::Pacer::default(),
            benchmark: std::env::args().any(|a| a == "--benchmark"),
            perf: perf::Benchmark::default(),
            shader_frame_times: vec![],
            review_audio: if review
                || std::env::args().any(|a| a == "--anatomy-review" || a == "--survival-review")
            {
                vec![
                    0.;
                    audio::RATE as usize
                        * if std::env::args().any(|a| a == "--anatomy-review") {
                            50
                        } else {
                            40
                        }
                ]
            } else {
                vec![]
            },
            frames: 0,
            stage: if std::env::args().any(|a| a == "--armory-review") {
                7
            } else {
                0
            },
            stage_frames: 0,
        }
    }
    // Escape, controller B in menus, and Start share one back/pause action.
    fn escape(&mut self) {
        if self.game.rebinding.is_some() {
            self.game.rebinding = None;
        } else if self.game.pad_rebinding.is_some() {
            self.game.pad_rebinding = None;
        } else if self.game.confirm_new_run {
            self.game.confirm_new_run = false;
        } else if self.game.settings {
            self.game.save_preferences();
            self.game.settings = false;
        } else {
            self.game.back();
        }
        self.sync_cursor();
    }
    fn note_sprint_press(&mut self, trigger: controls::Trigger) {
        if self.game.prefs.bindings.action(trigger) == Some(controls::Action::Sprint) {
            self.sprint_pressed = true;
        }
    }
    /// Run the one-shot action bound to a key or mouse button, if any.
    fn press(&mut self, trigger: controls::Trigger) {
        use controls::Action;
        match self.game.prefs.bindings.action(trigger) {
            Some(Action::Dodge) => self.game.dodge(),
            Some(Action::Reload) => self.game.reload_weapon(),
            Some(Action::Melee) => self.game.melee(),
            Some(Action::Bolt) => self.game.fire(true),
            _ => {}
        }
    }
    fn sync_cursor(&mut self) {
        let Some(w) = &self.window else { return };
        let captured = self.game.mode == Mode::Arena && !self.smoke && !self.review;
        if captured != self.captured {
            self.captured = captured;
            w.set_cursor_visible(!captured);
            if captured {
                let _ = w
                    .set_cursor_grab(CursorGrabMode::Locked)
                    .or_else(|_| w.set_cursor_grab(CursorGrabMode::Confined));
            } else {
                let _ = w.set_cursor_grab(CursorGrabMode::None);
                self.held.clear();
                self.game.input = Default::default();
            }
        }
    }
    fn smoke_step(&mut self) -> Option<&'static str> {
        self.stage_frames += 1;
        if self.shader_review {
            const SHOTS: [&str; 8] = [
                "captures/shaders/00-original.png",
                "captures/shaders/01-hollowlight.png",
                "captures/shaders/02-firelight.png",
                "captures/shaders/03-muzzle.png",
                "captures/shaders/04-elemental.png",
                "captures/shaders/05-half-strength.png",
                "captures/shaders/06-settings.png",
                "captures/shaders/07-resized.png",
            ];
            if self.stage >= 8 {
                return None;
            }
            if self.stage_frames == 1 {
                self.game.new_run();
                self.bones = scene::Bones::new();
                self.game.run.weapon.kind = weapons::WeaponKind::Double;
                self.game.run.weapon.rarity = 1;
                self.game.run.ammo = 2;
                self.game.run.pitch = -0.04;
                self.game.shader_intensity = if self.stage == 0 {
                    0.
                } else if self.stage == 5 {
                    0.5
                } else {
                    1.
                };
                self.game.notice_time = 0.;
                if self.stage == 2 {
                    self.game.run.pos = glam::Vec3::new(2.5, 1.65, 2.);
                    self.game.run.yaw = 0.5;
                    self.game.run.pitch = -0.22;
                }
                if self.stage == 4 {
                    self.game.run.enemies.truncate(3);
                    for (i, e) in self.game.run.enemies.iter_mut().enumerate() {
                        e.pos = glam::Vec3::new((i as f32 - 1.) * 1.9, 0., -3.0);
                        e.hp = 500.;
                        e.max_hp = 500.;
                        if i == 0 {
                            e.burn = 5.;
                        } else if i == 1 {
                            e.slow = 5.;
                        } else {
                            e.poison = 5.;
                        }
                    }
                    self.game.run.pos = glam::Vec3::new(0., 1.65, 2.5);
                }
                if self.stage == 6 {
                    self.game.settings = true;
                }
                if self.stage == 7 {
                    self.game.settings = false;
                    let _ = self
                        .window
                        .as_ref()
                        .unwrap()
                        .request_inner_size(LogicalSize::new(1100., 700.));
                }
            }
            // Hold time and enemies still for exactly aligned baseline/effects images.
            self.game.elapsed = 12.;
            if self.stage < 6 {
                self.game.run.enemies.iter_mut().for_each(|e| {
                    e.attack = 10.;
                });
            }
            if self.stage == 3 {
                self.game.flash = 0.095;
            }
            if self.stage == 6 && self.stage_frames == 30 {
                assert!(
                    self.game.shader_intensity < 0.02,
                    "effects slider switched off through real UI"
                );
            }
            if self.stage == 6 && self.stage_frames == 60 {
                assert!(
                    self.game.shader_intensity > 0.98,
                    "effects slider restored through real UI"
                );
            }
            if self.stage == 6 && self.stage_frames == 80 {
                assert!(
                    self.game.vsync && self.game.show_fps,
                    "performance settings toggled through native UI"
                );
            }
            if self.stage == 6 && self.stage_frames == 94 {
                assert!(!self.game.vsync, "unlocked mode restored through native UI");
            }
            if self.stage_frames == 95 {
                return Some(SHOTS[self.stage]);
            }
            if self.stage_frames > 98 {
                if !self.shader_frame_times.is_empty() {
                    self.shader_frame_times.sort_by(f32::total_cmp);
                    let n = self.shader_frame_times.len();
                    let mean = self.shader_frame_times.iter().sum::<f32>() / n as f32;
                    println!(
                        "SHADER TIMING stage {}: mean {:.2} ms, p95 {:.2} ms ({} frames, native presentation included)",
                        self.stage,
                        mean,
                        self.shader_frame_times[n * 95 / 100],
                        n
                    );
                    self.shader_frame_times.clear();
                }
                self.stage += 1;
                self.stage_frames = 0;
            }
            return None;
        }
        if self.gun_review {
            const SHOTS: [&str; 44] = [
                "captures/guns/00-iron.png",
                "captures/guns/01-double.png",
                "captures/guns/02-repeater.png",
                "captures/guns/03-revolver.png",
                "captures/guns/04-duelist.png",
                "captures/guns/05-pepperbox.png",
                "captures/guns/06-hand-cannon.png",
                "captures/guns/07-needler.png",
                "captures/guns/08-blunderbuss.png",
                "captures/guns/09-coachgun.png",
                "captures/guns/10-slugbreaker.png",
                "captures/guns/11-dragonbreath.png",
                "captures/guns/12-frostbite.png",
                "captures/guns/13-longrifle.png",
                "captures/guns/14-carbine.png",
                "captures/guns/15-gatling.png",
                "captures/guns/16-harpoon.png",
                "captures/guns/17-ricochet.png",
                "captures/guns/18-grenadier.png",
                "captures/guns/19-mortar.png",
                "captures/guns/20-iron-fps.png",
                "captures/guns/21-double-fps.png",
                "captures/guns/22-revolver-fps.png",
                "captures/guns/23-repeater-fps.png",
                "captures/guns/24-rifle-fps.png",
                "captures/guns/25-gatling-fps.png",
                "captures/guns/26-blunderbuss-fps.png",
                "captures/guns/27-mortar-fps.png",
                "captures/guns/28-double-reload.png",
                "captures/guns/29-repeater-reload.png",
                "captures/guns/30-ember-staff.png",
                "captures/guns/31-storm-wand.png",
                "captures/guns/32-frost-scepter.png",
                "captures/guns/33-soul-lantern.png",
                "captures/guns/34-plague-censer.png",
                "captures/guns/35-crossbow.png",
                "captures/guns/36-longbow.png",
                "captures/guns/37-cleaver.png",
                "captures/guns/38-rapier.png",
                "captures/guns/39-warhammer.png",
                "captures/guns/40-scythe.png",
                "captures/guns/41-flail.png",
                "captures/guns/42-daggers.png",
                "captures/guns/43-cleaver-fps.png",
            ];
            if self.stage >= SHOTS.len() {
                return None;
            }
            if self.stage_frames == 1 {
                let index = if self.stage < 20 {
                    self.stage
                } else if self.stage == 43 {
                    27
                } else if self.stage >= 30 {
                    self.stage - 10
                } else {
                    [0, 1, 3, 2, 13, 15, 8, 19, 1, 2][self.stage - 20]
                };
                let kind = weapons::WeaponKind::ALL[index];
                if self.stage < 20 || (30..43).contains(&self.stage) {
                    self.game.mode = Mode::Collection;
                    self.game.collection_kind = index;
                    self.game.collection_group = kind.spec().group;
                    self.game.collection_rarity = 0;
                } else {
                    self.game.new_run();
                    self.game.run.weapon.kind = kind;
                    self.game.run.weapon.rarity = 1;
                    self.game.run.ammo = self.game.run.weapon.capacity();
                    self.game.notice_time = 0.;
                    self.game.run.hp = 100.;
                    self.game.run.yaw = 0.;
                    self.game.run.pitch = -0.04;
                }
            }
            if (28..30).contains(&self.stage) {
                self.game.reload = self.game.run.weapon.reload_time() * 0.52;
            }
            if self.stage_frames == 24 {
                return Some(SHOTS[self.stage]);
            }
            if self.stage_frames > 26 {
                self.stage += 1;
                self.stage_frames = 0;
            }
            return None;
        }
        if self.armory_review {
            match self.stage {
                7 => {
                    if self.stage_frames == 1 {
                        self.game.mode = Mode::Title;
                        self.game.settings = true;
                    }
                    if self.stage_frames == 20 {
                        return Some("captures/08-journal.png");
                    }
                    if self.stage_frames > 24 {
                        self.game.settings = false;
                        self.game.mode = Mode::Paused;
                        self.stage = 8;
                        self.stage_frames = 0;
                    }
                }
                8 => {
                    if self.stage_frames == 20 {
                        return Some("captures/09-pause.png");
                    }
                    if self.stage_frames > 24 {
                        self.game.mode = Mode::Title;
                        self.game.confirm_new_run = true;
                        self.stage = 9;
                        self.stage_frames = 0;
                    }
                }
                9 => {
                    if self.stage_frames == 20 {
                        return Some("captures/10-confirmation.png");
                    }
                    if self.stage_frames > 24 {
                        self.game.confirm_new_run = false;
                        self.game.mode = Mode::Collection;
                        self.game.return_mode = Mode::Title;
                        self.stage = 10;
                        self.stage_frames = 0;
                    }
                }
                10..=15 => {
                    if self.stage_frames == 1 {
                        self.game.collection_group = self.stage - 10;
                        self.game.collection_kind = [3, 11, 25, 18, 21, 30][self.stage - 10];
                    }
                    if self.stage_frames == 25 {
                        return Some(
                            [
                                "captures/11-sidearms.png",
                                "captures/12-scatterguns.png",
                                "captures/13-longarms.png",
                                "captures/14-ordnance.png",
                                "captures/15-occult.png",
                                "captures/16-melee.png",
                            ][self.stage - 10],
                        );
                    }
                    if self.stage_frames > 28 {
                        self.stage += 1;
                        self.stage_frames = 0;
                    }
                }
                16 => {
                    if self.stage_frames == 20 {
                        assert_eq!(self.game.mode, Mode::Arena);
                        assert!(self.game.practice_backup.is_some());
                        self.game.fire(false);
                    }
                    if self.stage_frames == 23 {
                        return Some("captures/17-melee-practice.png");
                    }
                    if self.stage_frames > 30 {
                        self.game.back();
                        assert_eq!(self.game.mode, Mode::Collection);
                        assert!(self.game.practice_backup.is_none());
                        println!(
                            "ARMORY REVIEW PASS: journal, pause, confirmation, six weapon categories, practice entry through UI, melee use and preserved run"
                        );
                        self.stage = 17;
                    }
                }
                _ => {}
            }
            return None;
        }
        match self.stage {
            0 if self.stage_frames == 20 => return Some("captures/01-title.png"),
            0 if self.stage_frames > 22 => {
                self.game.new_run();
                self.stage = 1;
                self.stage_frames = 0;
            }
            1 => {
                if let Some(e) = self.game.run.enemies.first() {
                    let dir = anatomy::Pose::for_enemy(e, self.game.run.pos)
                        .anchor(anatomy::Part::Head)
                        - self.game.run.pos;
                    self.game.run.yaw = dir.x.atan2(-dir.z);
                    self.game.run.pitch = (dir.y / dir.length()).asin();
                }
                self.game.input.fire = true;
                if self.stage_frames == 50 {
                    return Some("captures/02-arena.png");
                }
                if self.game.mode == Mode::Shop {
                    println!(
                        "SMOKE: cleared first wave through combat; kills={}, reward={}",
                        self.game.run.kills, self.game.run.gold
                    );
                    assert_eq!(self.game.run.kills, 8);
                    assert_eq!(self.game.run.gold, 90);
                    self.game.input.fire = false;
                    self.stage = 2;
                    self.stage_frames = 0;
                }
                if self.stage_frames > 6000 {
                    panic!(
                        "Combat smoke test timed out: hp={} kills={}",
                        self.game.run.hp, self.game.run.kills
                    );
                }
            }
            2 if self.stage_frames == 20 => return Some("captures/03-collector.png"),
            2 if self.stage_frames > 22 => {
                self.game.open_pack();
                assert_eq!(self.game.mode, Mode::Pack);
                assert_eq!(self.game.run.gold, 60);
                self.stage = 3;
                self.stage_frames = 0;
            }
            3 if self.stage_frames == 20 => return Some("captures/04a-sealed-pack.png"),
            3 if self.stage_frames == 240 => return Some("captures/04-pack.png"),
            3 if self.stage_frames > 310 => {
                assert_eq!(self.game.mode, Mode::Shop, "Pack was taken through UI");
                println!("SMOKE: pack tear, reveal, select, and equip UI passed");
                if self.gamepad_smoke {
                    assert!(
                        self.pad.cursor.pos.is_some(),
                        "controller cursor drove the pack"
                    );
                    println!("SMOKE: controller stick, D-pad and A button drove the pack UI");
                }
                self.game.mode = Mode::Tree;
                self.game.upgrade(0);
                assert_eq!(self.game.run.weapon.paths[0], 1);
                assert_eq!(self.game.run.gold, 45);
                self.stage = 4;
                self.stage_frames = 0;
            }
            4 if self.stage_frames == 25 => return Some("captures/05-upgrades.png"),
            4 if self.stage_frames > 27 => {
                self.game.mode = Mode::Shop;
                self.game.open_book(Mode::Collection);
                self.stage = 5;
                self.stage_frames = 0;
            }
            5 if self.stage_frames == 25 => return Some("captures/06-collection.png"),
            5 if self.stage_frames > 27 => {
                self.game.mode = Mode::Bestiary;
                self.game.bestiary_index = 3;
                self.stage = 6;
                self.stage_frames = 0;
            }
            6 if self.stage_frames == 25 => return Some("captures/07-bestiary.png"),
            6 if self.stage_frames > 27 => {
                let json = serde_json::to_vec(&self.game.run).unwrap();
                let saved: game::Run = serde_json::from_slice(&json).unwrap();
                assert_eq!(saved.weapon.paths, [1, 0, 0]);
                self.game.mode = Mode::Shop;
                self.game.next_wave();
                assert_eq!(self.game.run.wave, 2);
                assert_eq!(self.game.run.weapon.paths, [1, 0, 0]);
                println!(
                    "SMOKE PASS: combat -> reward -> pack -> equip -> upgrade -> next round; save roundtrip passed"
                );
                self.stage = 7;
            }
            _ => {}
        }
        None
    }
    fn review_step(&mut self) {
        use game::WeaponKind;
        match self.frames {
            1 => {
                self.game.mode = Mode::Shop;
                self.game.run.gold = 90;
                self.game.open_pack();
            }
            400 => {
                assert_eq!(
                    self.game.mode,
                    Mode::Shop,
                    "Motion review must complete actual pack UI"
                );
                self.game.new_run();
                self.game.run.weapon.kind = WeaponKind::Double;
                self.game.run.weapon.rarity = 2;
                self.game.run.ammo = 2;
                for (i, e) in self.game.run.enemies.iter_mut().enumerate() {
                    e.pos = glam::Vec3::new((i as f32 - 4.) * 3., 0., -13.);
                    e.hp = 10000.;
                }
            }
            430 | 470 | 690 | 750 | 930 | 938 | 946 => self.game.fire(false),
            500 | 780 | 1000 => self.game.reload_weapon(),
            745 => {
                assert_eq!(self.game.run.ammo, 1, "Double reload and subsequent shot");
                self.game.run.weapon.kind = WeaponKind::Pistol;
                self.game.run.ammo = 8;
            }
            925 => {
                assert_eq!(self.game.run.ammo, 8, "Pistol reload completed");
                self.game.run.weapon.kind = WeaponKind::Repeater;
                self.game.run.ammo = 24;
            }
            _ => {}
        }
    }
    fn survival_step(&mut self) {
        if self.frames <= 36 {
            self.game.mode = Mode::Bestiary;
            self.game.bestiary_index = ((self.frames - 1) / 3) as usize;
        }
        if self.frames == 37 {
            self.game.new_run();
            self.game.run.survival.remaining = 0;
            self.game.run.enemies = (0..48)
                .map(|i| {
                    game::Enemy::spawn(
                        i % 12,
                        glam::Vec3::new((i % 8) as f32 * 2. - 7., 0., 3. - (i / 8) as f32 * 2.5),
                        12,
                        i as f32,
                    )
                })
                .collect();
        }
        if self.frames == 40 {
            self.game.new_run();
            self.game.run.wave = 4;
            self.game.run.enemies = (0..12)
                .map(|k| {
                    game::Enemy::spawn(
                        k,
                        glam::Vec3::new((k % 6) as f32 * 2.6 - 6.5, 0., 2. - (k / 6) as f32 * 5.),
                        4,
                        k as f32 * 0.7,
                    )
                })
                .collect();
            self.game.run.survival.remaining = 0;
            self.game.run.weapon.kind = weapons::WeaponKind::Repeater;
            self.game.run.weapon.rarity = 2;
            self.game.run.ammo = 24;
            self.game.run.survival.ranks[5] = 1;
            self.game.run.survival.ranks[6] = 1;
            self.game.run.survival.ranks[7] = 1;
            self.game.notice_time = 0.;
        }
        if self.frames > 110 && self.game.mode == Mode::Arena {
            if let Some(e) = self
                .game
                .run
                .enemies
                .iter()
                .filter(|e| e.hp > 0.)
                .min_by(|a, b| {
                    a.pos
                        .distance_squared(self.game.run.pos)
                        .total_cmp(&b.pos.distance_squared(self.game.run.pos))
                })
            {
                let d = anatomy::Pose::for_enemy(e, self.game.run.pos).anchor(
                    if e.anatomy.missing(anatomy::Part::Head) {
                        anatomy::Part::Torso
                    } else {
                        anatomy::Part::Head
                    },
                ) - self.game.run.pos;
                self.game.run.yaw = d.x.atan2(-d.z);
                self.game.run.pitch = (d.y / d.length()).asin();
                self.game.input.forward = if d.length() > 4. { 0.5 } else { -0.2 };
                self.game.input.right = 0.4;
            }
            self.game.input.fire = true;
        }
        if self.game.mode == Mode::LevelUp {
            self.stage_frames += 1;
        } else {
            self.stage_frames = 0;
        }
    }
    fn frame(&mut self, event_loop: &ActiveEventLoop) {
        if event_loop.exiting() || (self.occluded && !self.smoke && !self.review) {
            return;
        }
        let started = Instant::now();
        self.pacer.started(started, self.frame_limit());
        if let Some(review) = &mut self.pacing_review {
            review.frame(started);
            if review.finished {
                event_loop.exit();
                return;
            }
        }
        let frame_interval = self.last.elapsed().as_secs_f64() * 1000.;
        // Smooth frame duration, not reciprocal FPS, to avoid overstating uneven pacing.
        let averaged_ms = if self.game.fps > 0. {
            1000. / self.game.fps as f64 * 0.94 + frame_interval * 0.06
        } else {
            frame_interval
        };
        self.game.fps = (1000. / averaged_ms.max(0.01)) as f32;
        self.frames += 1;
        if self.benchmark {
            if self.frames > 2520 {
                self.perf.save();
                event_loop.exit();
                return;
            }
            if (self.frames - 1) % 420 == 0 {
                perf::Benchmark::setup(
                    &mut self.game,
                    &mut self.bones,
                    ((self.frames - 1) / 420) % 3,
                );
            }
            self.game.run.hp = self.game.max_hp();
        }
        let dt = if self.benchmark {
            1. / 120.
        } else if self.smoke || self.review {
            1. / 60.
        } else {
            self.last.elapsed().as_secs_f32().min(0.05)
        };
        if self.shader_review && (20..90).contains(&self.stage_frames) {
            self.shader_frame_times
                .push(self.last.elapsed().as_secs_f32() * 1000.);
        }
        self.last = Instant::now();
        if self.survival_review {
            self.survival_step();
        }
        if let Some(review) = &mut self.anatomy_proof {
            review.step(&mut self.game, &mut self.bones, self.frames);
        }
        if self.review {
            self.review_step();
        }
        let review_path = if let Some(review) = &mut self.world_review {
            let path = review.step(&mut self.game);
            if review.finished() {
                event_loop.exit();
                return;
            }
            path
        } else if let Some(review) = &mut self.model_review {
            let path = review.step(&mut self.game);
            if review.finished() {
                event_loop.exit();
                return;
            }
            path
        } else if let Some(review) = &mut self.text_review {
            let path = review.step(&mut self.game, self.frames);
            if review.finished() {
                event_loop.exit();
                return;
            }
            path
        } else if self.input_review.is_some() {
            self.input_step()
        } else if self.fullscreen_review.is_some() {
            let path = self.fullscreen_step();
            if self.fullscreen_review.as_ref().is_some_and(|r| r.finished) {
                event_loop.exit();
                return;
            }
            path
        } else if self.benchmark && self.frames % 420 == 60 {
            Some(format!(
                "captures/performance/{}-{}.png",
                if self.frames <= 1260 {
                    "reference"
                } else {
                    "optimized"
                },
                ((self.frames - 1) / 420) % 3
            ))
        } else if self.survival_review && self.frames == 38 {
            Some("captures/survival/crowd-stress.png".into())
        } else if self.survival_review && self.frames <= 36 && self.frames % 3 == 0 {
            Some(format!(
                "captures/survival/species-{:02}.png",
                (self.frames - 1) / 3
            ))
        } else if self.survival_review && self.frames >= 40 && self.frames % 2 == 0 {
            Some(format!(
                "captures/survival/frame-{:04}.png",
                (self.frames - 40) / 2 + 1
            ))
        } else if self.anatomy_review && self.frames % 2 == 0 {
            Some(format!("captures/anatomy/frame-{:04}.png", self.frames / 2))
        } else if self.review && self.frames % 2 == 0 {
            Some(format!("captures/motion/frame-{:04}.png", self.frames / 2))
        } else {
            None
        };
        let capture = if self.benchmark
            || self.fullscreen_review.is_some()
            || self.pacing_review.is_some()
            || self.text_review.is_some()
            || self.model_review.is_some()
            || self.world_review.is_some()
        {
            review_path.as_deref()
        } else if self.survival_review || self.anatomy_review {
            review_path.as_deref()
        } else if self.smoke {
            self.smoke_step()
        } else {
            review_path.as_deref()
        };
        if self.survival_review && self.frames > 1120 {
            audio::write_wav(
                "captures/survival-review.wav",
                &self.review_audio[..audio::RATE as usize * 38],
            )
            .unwrap();
            assert!(self.game.run.kills >= 8, "survival review kills");
            assert!(
                self.game.run.survival.level > 1,
                "souls collected and leveled"
            );
            assert!(
                self.game
                    .run
                    .survival
                    .ranks
                    .iter()
                    .map(|r| *r as u32)
                    .sum::<u32>()
                    > 3,
                "power selected through UI"
            );
            println!(
                "SURVIVAL REVIEW PASS: 12 species rendered; {} kills, soul level {}, powers selected through native UI",
                self.game.run.kills, self.game.run.survival.level
            );
            event_loop.exit();
            return;
        }
        if self.anatomy_review && self.frames > anatomy_review::FRAMES {
            audio::write_wav(
                "captures/anatomy-review.wav",
                &self.review_audio[..audio::RATE as usize * 48],
            )
            .unwrap();
            self.anatomy_proof.as_ref().unwrap().finish();
            println!(
                "ANATOMY REVIEW PASS: six real-fire scenarios, articulated ragdolls, capped fractures and follow-up debris blast"
            );
            event_loop.exit();
            return;
        }
        if self.review && self.frames > 1140 {
            assert_eq!(self.game.run.ammo, 24, "Repeater reload completed");
            audio::write_wav(
                "captures/motion-review.wav",
                &self.review_audio[..audio::RATE as usize * 38],
            )
            .unwrap();
            println!(
                "MOTION REVIEW PASS: pack tear/deal/flip/equip and three weapon reloads captured at 30 fps with synchronized sound"
            );
            event_loop.exit();
            return;
        }
        if (self.smoke
            && !self.armory_review
            && !self.gun_review
            && !self.shader_review
            && !self.anatomy_review
            && !self.survival_review
            && !self.benchmark
            && self.text_review.is_none()
            && self.model_review.is_none()
            && self.world_review.is_none()
            && self.stage == 7)
            || (self.armory_review && self.stage == 17)
            || (self.gun_review && self.stage == 44)
            || (self.shader_review && self.stage == 8)
        {
            if self.shader_review {
                println!(
                    "SHADER REVIEW PASS: baseline, full and half strength, firelight, muzzle lighting, elemental surfaces, live slider, resized depth targets"
                );
            }
            if self.gun_review {
                println!(
                    "GUN REVIEW PASS: all 33 weapon previews, nine first-person silhouettes, two articulated reload poses"
                );
            }
            event_loop.exit();
            return;
        }
        let mut pad_frame = self.pad.poll();
        if !self.focused {
            pad_frame = gamepad::Frame::default();
        }
        for notice in self.pad.notices.drain(..) {
            self.game.notify(&notice);
        }
        if pad_frame.lost && !self.smoke && !self.review && self.game.controller_lost() {
            self.mouse_fire = false;
            self.sync_cursor();
        }
        if pad_frame.deliberate() {
            self.game.device = controls::Device::Controller;
        }
        if !self.smoke && !self.review {
            let pad = gamepad::arena(&pad_frame, &self.game.prefs.pad_bindings);
            let bindings = self.game.prefs.bindings;
            let down = |action| self.held.contains(&bindings.get(action).trigger);
            let axis = |positive, negative, stick: f32| {
                (down(positive) as u8 as f32 - down(negative) as u8 as f32 + stick).clamp(-1., 1.)
            };
            use controls::Action;
            self.game.input.forward = axis(Action::Forward, Action::Back, pad.forward);
            self.game.input.right = axis(Action::Right, Action::Left, pad.right);
            let sprint = down(Action::Sprint) || pad.sprint || self.sprint_pressed;
            self.game.sprint_input(sprint);
            let menus =
                self.game.mode != Mode::Arena || self.game.settings || self.game.confirm_new_run;
            if menus && self.game.pad_rebinding.is_some() {
                // The journal is waiting for a button: the next press binds
                // (Start cancels), and the cursor neither moves nor clicks.
                match pad_frame.first_pressed() {
                    Some(gamepad::Button::Start) => self.escape(),
                    Some(button) => self.game.bind_pad(button),
                    None => {}
                }
            } else if menus {
                let size = self.window.as_ref().unwrap().inner_size();
                let scale = self.window.as_ref().unwrap().scale_factor() as f32;
                let screen = egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(size.width as f32, size.height as f32) / scale,
                );
                // On the level-up screen the D-pad picks powers instead.
                let targets = if self.game.mode == Mode::LevelUp {
                    vec![]
                } else {
                    ui::pad_targets(&self.ctx)
                };
                let (events, back) = self.pad.cursor.step(&pad_frame, dt, screen, &targets);
                self.pad_events.extend(events);
                if back {
                    self.escape();
                }
                if let Some(power) = pad.power {
                    self.game.choose_power(power);
                }
            } else {
                self.pad.cursor.hide();
                self.game.input.fire = self.mouse_fire || pad.fire || self.fire_pressed;
                let prefs = self.game.prefs;
                let assist = if pad.look == glam::Vec2::ZERO {
                    1.
                } else {
                    self.game.aim_assist()
                };
                let (yaw, pitch) =
                    gamepad::look_turn(pad.look * assist, prefs.stick_speed, prefs.invert_y, dt);
                self.game.run.yaw += yaw;
                self.game.run.pitch = (self.game.run.pitch + pitch).clamp(-1.3, 1.3);
                if pad.dodge {
                    self.game.dodge();
                }
                if pad.reload {
                    self.game.reload_weapon();
                }
                if pad.melee {
                    self.game.melee();
                }
                if pad.spell {
                    self.game.fire(true);
                }
                if pad.pause {
                    self.escape();
                }
            }
        }
        self.sprint_pressed = false;
        self.fire_pressed = false;
        if (self.smoke
            && !self.survival_review
            && self.text_review.is_none()
            && self.model_review.is_none()
            && self.world_review.is_none())
            || self.review
        {
            self.game.run.survival.remaining = 0;
            if self.game.mode == Mode::LevelUp {
                self.game.choose_power(0);
            }
        }
        let simulation_start = Instant::now();
        if self.text_review.is_none() && self.model_review.is_none() {
            self.game.update(dt);
        }
        if let Some(review) = &mut self.world_review {
            review.observe(&self.game);
        }
        // Existing debris receives impacts before this frame's new bodies are spawned.
        for impact in self.game.physics_impacts.drain(..) {
            if let Some(review) = &mut self.anatomy_proof {
                review.impact(&impact, &self.bones);
            }
            match impact {
                anatomy::PhysicsImpact::Ray {
                    origin,
                    direction,
                    range,
                    energy,
                } => self.bones.impact_ray(origin, direction, range, energy),
                anatomy::PhysicsImpact::Blast {
                    center,
                    radius,
                    energy,
                } => self.bones.impact_blast(center, radius, energy),
            }
            if let Some(review) = &mut self.anatomy_proof {
                review.impacted(&self.bones);
            }
        }
        for event in self.game.spawned_bodies.drain(..) {
            if let Some(review) = &mut self.anatomy_proof {
                let count = self.bones.pieces.len();
                self.bones.spawn_body(event.clone());
                review.spawned(&event, &self.bones, count);
            } else {
                self.bones.spawn_body(event);
            }
        }
        // Let a fatal shot settle under death/victory overlays; pause still freezes physics.
        if matches!(self.game.mode, Mode::Arena | Mode::Dead | Mode::Victory) {
            self.bones.update(dt);
        }
        if let Some(review) = &mut self.anatomy_proof {
            review.observe(&self.bones);
        }
        let simulation_ms = simulation_start.elapsed().as_secs_f64() * 1000.;
        let scripted = self.smoke || self.review;
        self.audio.volume(if scripted { 0. } else { self.game.prefs.volume });
        let prefs = self.game.prefs;
        self.audio.music(
            music::targets(&self.game),
            if scripted {
                0.
            } else {
                // Music sits under effects; Sound volume scales both.
                prefs.music_volume * prefs.volume * 1.5
            },
        );
        let (listener, yaw) = (self.game.run.pos, self.game.run.yaw);
        for (event, source) in self.game.world_sounds.drain(..) {
            let gains = audio::spatial(listener, yaw, source);
            if self.review || self.anatomy_review || self.survival_review {
                let offset = (self.frames as usize - 1) * (audio::RATE as usize / 60) * 2;
                let sound = audio::synthesize(event, self.frames % 4);
                for (i, value) in sound.iter().enumerate() {
                    let gain = if i % 2 == 0 { gains.0 } else { gains.1 };
                    if let Some(s) = self.review_audio.get_mut(offset + i) {
                        *s += value * 0.8 * gain;
                    }
                }
            }
            if !self.smoke && !self.review {
                self.audio.play_at(event, self.game.prefs.volume, gains);
            }
        }
        for event in self.game.sound_events.drain(..) {
            if self.review || self.anatomy_review || self.survival_review {
                let offset = (self.frames as usize - 1) * (audio::RATE as usize / 60) * 2;
                let sound = audio::synthesize(event, self.frames % 4);
                for (i, value) in sound.iter().enumerate() {
                    if let Some(s) = self.review_audio.get_mut(offset + i) {
                        *s += value * 0.8;
                    }
                }
            }
            if !self.smoke && !self.review {
                self.audio.play(event, self.game.prefs.volume);
            }
        }
        self.sync_cursor();
        let window = self.window.as_ref().unwrap();
        let state = self.egui_state.as_mut().unwrap();
        let renderer = self.renderer.as_mut().unwrap();
        renderer.optimized = !self.benchmark || self.frames > 1260;
        renderer.camera(&self.game);
        let mut input = state.take_egui_input(window);
        if self.smoke || self.review {
            // Scripted UI input shares the simulation clock and must not race
            // desktop pointer/focus events or GPU capture time.
            input.events.clear();
            input.time = Some(self.frames as f64 / 60.);
            input.focused = true;
        }
        input.events.append(&mut self.pad_events);
        if let Some(review) = &self.text_review {
            input.time = Some(self.game.elapsed as f64);
            review.input(&mut input);
            // Controller-cursor fixtures press the D-pad through the real cursor.
            match review.pad_presses() {
                None => self.pad.cursor.hide(),
                Some(buttons) => {
                    let frame = gamepad::Frame {
                        pressed: buttons.clone(),
                        held: buttons,
                        ..Default::default()
                    };
                    let targets = ui::pad_targets(&self.ctx);
                    let r = input.screen_rect.unwrap();
                    let (events, _) = self.pad.cursor.step(&frame, 1. / 60., r, &targets);
                    input.events.extend(events);
                }
            }
        }
        if self.survival_review
            && self.game.mode == Mode::LevelUp
            && matches!(self.stage_frames, 45 | 46)
        {
            let r = input.screen_rect.unwrap();
            let scale = (r.width() / 1440.).min(r.height() / 900.);
            let pos = egui::pos2(
                r.min.x + (r.width() - 1440. * scale) * 0.5 + 354. * scale,
                r.min.y + 637. * scale,
            );
            input.events.push(egui::Event::PointerMoved(pos));
            input.events.push(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: self.stage_frames == 45,
                modifiers: egui::Modifiers::NONE,
            });
        }
        if self.armory_review && self.stage == 16 && matches!(self.stage_frames, 3 | 4) {
            let r = input.screen_rect.unwrap();
            let scale = (r.width() / 1440.).min(r.height() / 900.);
            let pos = egui::pos2(
                r.min.x + (r.width() - 1440. * scale) * 0.5 + 1125. * scale,
                r.min.y + 680. * scale,
            );
            input.events.push(egui::Event::PointerMoved(pos));
            input.events.push(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: self.stage_frames == 3,
                modifiers: egui::Modifiers::NONE,
            });
        }
        if let Some(((x, y), pressed)) = self.fullscreen_review.as_ref().and_then(|r| r.pointer()) {
            let r = input.screen_rect.unwrap();
            let s = (r.width() / 1440.).min(r.height() / 900.);
            let pos = egui::pos2(
                r.min.x + (r.width() - 1440. * s) * 0.5 + x * s,
                r.min.y + y * s,
            );
            input.events.push(egui::Event::PointerMoved(pos));
            input.events.push(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
        // Exercise egui's real hit testing and handlers in the native render loop.
        if (self.smoke
            && !self.gamepad_smoke
            && !self.gun_review
            && !self.shader_review
            && self.text_review.is_none()
            && self.stage == 3)
            || (self.review && self.frames < 400)
        {
            let tick = if self.review {
                match self.frames {
                    30 => 22,
                    31 => 23,
                    200 => 160,
                    201 => 161,
                    280 => 235,
                    281 => 236,
                    315 => 255,
                    316 => 256,
                    _ => 0,
                }
            } else {
                self.stage_frames
            };
            let point = match tick {
                22 | 23 => Some((720., 690.)),
                160 | 161 => Some((720., 728.)),
                235 | 236 => Some((720., 587.)),
                255 | 256 => Some((720., 728.)),
                _ => None,
            };
            if let Some((x, y)) = point {
                let r = input.screen_rect.unwrap();
                let s = (r.width() / 1440.).min(r.height() / 900.);
                let pos = egui::pos2(
                    r.min.x + (r.width() - 1440. * s) * 0.5 + x * s,
                    r.min.y + y * s,
                );
                input.events.push(egui::Event::PointerMoved(pos));
                input.events.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: matches!(tick, 22 | 160 | 235 | 255),
                    modifiers: egui::Modifiers::NONE,
                });
            }
        }
        // The same pack clicks with the controller: the stick steers the
        // cursor to TEAR OPEN, then the D-pad jumps it to REVEAL ALL, the
        // middle card's CHOOSE CARD and TAKE & EQUIP, and A presses each.
        if self.gamepad_smoke && self.stage == 3 {
            use gamepad::Button::{DPadDown, DPadUp};
            const CLICKS: [(u32, (f32, f32), Option<gamepad::Button>); 4] = [
                (22, (720., 690.), None),
                (160, (720., 728.), Some(DPadDown)),
                (235, (720., 587.), Some(DPadUp)),
                (255, (720., 728.), Some(DPadDown)),
            ];
            let r = input.screen_rect.unwrap();
            let s = (r.width() / 1440.).min(r.height() / 900.);
            let to_screen = |(x, y): (f32, f32)| {
                egui::pos2(
                    r.min.x + (r.width() - 1440. * s) * 0.5 + x * s,
                    r.min.y + y * s,
                )
            };
            let targets = ui::pad_targets(&self.ctx);
            let mut frame = gamepad::Frame::default();
            // Hold still while A is released, or egui sees a drag instead of a click.
            let releasing = CLICKS.iter().any(|(t, ..)| t + 1 == self.stage_frames);
            if let Some(&(tick, point, dpad)) = CLICKS
                .iter()
                .find(|(t, ..)| *t >= self.stage_frames)
                .filter(|_| !releasing)
            {
                match dpad {
                    None => {
                        let from = self.pad.cursor.pos.unwrap_or(r.center());
                        frame.left = gamepad::steer(from, to_screen(point), 1. / 60.);
                    }
                    Some(button) if tick - 6 == self.stage_frames => {
                        frame.pressed.push(button);
                        frame.held.push(button);
                    }
                    Some(_) => {}
                }
                if tick == self.stage_frames {
                    if dpad.is_some() {
                        // The D-pad must have landed on the control we mean to press.
                        let at = self.pad.cursor.pos.unwrap_or(r.center());
                        let expected = to_screen(point);
                        assert!(
                            targets
                                .iter()
                                .any(|t| t.rect.contains(expected) && t.rect.contains(at)),
                            "SMOKE: D-pad left the cursor at {at:?}, not on the control at {expected:?}"
                        );
                    }
                    frame.pressed.push(gamepad::Button::South);
                    frame.held.push(gamepad::Button::South);
                }
            }
            let (events, _) = self.pad.cursor.step(&frame, 1. / 60., r, &targets);
            input.events.extend(events);
        }
        if self.shader_review && self.stage == 6 && matches!(self.stage_frames, 20 | 21 | 50 | 51) {
            let r = input.screen_rect.unwrap();
            let scale = (r.width() / 1440.).min(r.height() / 900.);
            // Ends of the journal's Hollowlight slider (row 4).
            let x = if self.stage_frames < 40 { 615. } else { 975. };
            let pos = egui::pos2(
                r.min.x + (r.width() - 1440. * scale) * 0.5 + x * scale,
                r.min.y + (ui::JOURNAL_SLIDER_Y + 4. * 44. + 1.) * scale,
            );
            input.events.push(egui::Event::PointerMoved(pos));
            input.events.push(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: matches!(self.stage_frames, 20 | 50),
                modifiers: egui::Modifiers::NONE,
            });
        }
        if self.shader_review
            && self.stage == 6
            && matches!(self.stage_frames, 65 | 66 | 70 | 71 | 85 | 86)
        {
            let r = input.screen_rect.unwrap();
            let scale = (r.width() / 1440.).min(r.height() / 900.);
            let x = if matches!(self.stage_frames, 70 | 71) {
                890.
            } else {
                545.
            };
            let pos = egui::pos2(
                r.min.x + (r.width() - 1440. * scale) * 0.5 + x * scale,
                r.min.y + (ui::JOURNAL_TOGGLES_Y + 44. + 17.5) * scale,
            );
            input.events.push(egui::Event::PointerMoved(pos));
            input.events.push(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: matches!(self.stage_frames, 65 | 70 | 85),
                modifiers: egui::Modifiers::NONE,
            });
        }
        let out = self.ctx.run(input, |ctx| {
            if self
                .model_review
                .as_ref()
                .is_none_or(|review| review.show_ui())
            {
                ui::draw(ctx, &mut self.game, renderer.view_projection);
            }
            if let Some(pos) = self.pad.cursor.pos {
                ui::pad_cursor(ctx, pos);
            }
        });
        state.handle_platform_output(window, out.platform_output.clone());
        let rendered = match renderer.render(&self.game, &self.bones, &self.ctx, out, capture) {
            Ok(()) => true,
            Err(e @ (wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated)) => {
                let size = window.inner_size();
                // Scripted runs log the first one: a surface that never
                // recovers is a stall, and the watchdog will name it.
                if (self.smoke || self.review) && !self.surface_error_logged {
                    self.surface_error_logged = true;
                    println!(
                        "Surface {e} at frame {}; reconfiguring at {}x{}",
                        self.frames, size.width, size.height
                    );
                }
                watchdog::step("surface lost or outdated, reconfiguring");
                renderer.resize(size.width, size.height);
                false
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                event_loop.exit();
                false
            }
            Err(e) => {
                eprintln!("Frame skipped: {e}");
                false
            }
        };
        if let Some(review) = &mut self.model_review {
            review.record(
                frame_interval,
                renderer.timings,
                renderer.vertex_count,
                [renderer.config.width, renderer.config.height],
                format!("{:?}", renderer.config.present_mode),
            );
        }
        if let Some(review) = &mut self.world_review {
            review.record(
                frame_interval,
                simulation_ms,
                renderer.timings,
                renderer.vertex_count,
                [renderer.config.width, renderer.config.height],
                format!("{:?}", renderer.config.present_mode),
            );
        }
        if self.benchmark && (self.frames - 1) % 420 >= 120 {
            self.perf.samples.push([
                frame_interval,
                simulation_ms,
                renderer.timings[0],
                renderer.timings[1],
                renderer.timings[2],
            ]);
            self.perf.vertices = self.perf.vertices.max(renderer.vertex_count);
            if self.frames % 420 == 0 {
                self.perf.finish(
                    (self.frames - 1) / 420,
                    renderer.config.width,
                    renderer.config.height,
                    format!("{:?}", renderer.config.present_mode),
                );
            }
        }
        if rendered {
            watchdog::frame();
            if let Some(review) = &mut self.fullscreen_review {
                review.rendered += 1;
            }
        }
        // With a frame limit, `about_to_wait` asks for the next frame when
        // it's due.
        if self.frame_limit() == 0 {
            window.request_redraw();
        }
        if std::mem::take(&mut self.game.fullscreen_changed) {
            self.apply_fullscreen();
        }
        if self.game.quit_requested {
            self.quit_game(event_loop);
        }
    }
}
impl ApplicationHandler for App {
    fn exiting(&mut self, _: &ActiveEventLoop) {
        watchdog::milestone("exiting");
        // Native macOS menu/Dock Quit (including Cmd+Q) can bypass CloseRequested.
        self.game.save_preferences();
        self.game.save_performance();
        self.game.save();
        // Release the egui clipboard, GPU surface and window while the
        // display connection is still alive. Wayland's clipboard worker
        // otherwise destroys its proxies after the connection closes.
        self.egui_state = None;
        self.renderer = None;
        self.window = None;
    }
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("Gravewake — The Hollow Tithe")
                        .with_inner_size(
                            if self.review
                                || self.survival_review
                                || ((self.text_review.is_some() || self.world_review.is_some())
                                    && std::env::args().any(|a| a == "--review-small"))
                            {
                                LogicalSize::new(960., 600.)
                            } else {
                                LogicalSize::new(1440., 900.)
                            },
                        )
                        .with_min_inner_size(LogicalSize::new(960., 600.))
                        .with_fullscreen(
                            (self.game.save_enabled && self.game.prefs.fullscreen)
                                .then_some(winit::window::Fullscreen::Borderless(None)),
                        )
                        .with_app_identity(),
                )
                .expect("create window"),
        );
        watchdog::milestone("window created");
        self.egui_state = Some(egui_winit::State::new(
            self.ctx.clone(),
            egui::ViewportId::ROOT,
            window.as_ref(),
            Some(window.scale_factor() as f32),
            None,
            None,
        ));
        watchdog::milestone("egui state ready");
        self.renderer = Some(pollster::block_on(renderer::Renderer::new(window.clone())));
        self.renderer.as_mut().unwrap().register_previews(&self.ctx);
        watchdog::milestone("renderer ready");
        self.window = Some(window);
        self.last = Instant::now();
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if let Some(state) = &mut self.egui_state {
            let _ = state.on_window_event(self.window.as_ref().unwrap(), &event);
        }
        match event {
            WindowEvent::Occluded(occluded) if !self.smoke && !self.review => {
                self.occluded = occluded;
                event_loop.set_control_flow(if occluded {
                    winit::event_loop::ControlFlow::Wait
                } else {
                    winit::event_loop::ControlFlow::Poll
                });
                if !occluded {
                    self.last = Instant::now();
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
            }
            WindowEvent::CloseRequested => {
                self.quit_game(event_loop);
            }
            WindowEvent::Resized(size) => {
                if let Some(r) = &mut self.renderer {
                    r.resize(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => self.frame(event_loop),
            WindowEvent::Focused(true) => self.focused = true,
            // Scripted runs steer the controller cursor themselves; a desktop
            // pointer crossing the window must not reset it mid-click.
            WindowEvent::CursorMoved { .. } if self.smoke || self.review => {
                if !self.pointer_ignored {
                    self.pointer_ignored = true;
                    println!("SMOKE: ignoring desktop pointer motion over the window");
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.pad.cursor.hide();
                let previous = self.last_pointer.replace(position);
                if previous.is_some_and(|p| (p.x - position.x).hypot(p.y - position.y) > 4.) {
                    self.game.device = controls::Device::Keyboard;
                }
            }
            WindowEvent::Focused(false) => {
                self.focused = false;
                self.mouse_fire = false;
                if self.game.mode == Mode::Arena && !self.smoke && !self.review {
                    self.game.back();
                    self.sync_cursor();
                }
                self.held.clear();
                self.sprint_pressed = false;
                self.fire_pressed = false;
                self.game.input = Default::default();
            }
            WindowEvent::KeyboardInput { event, .. } if !self.smoke && !self.review => {
                let PhysicalKey::Code(code) = event.physical_key else {
                    return;
                };
                let pressed = event.state == ElementState::Pressed;
                let trigger = controls::Trigger::Key(code);
                if pressed {
                    self.game.device = controls::Device::Keyboard;
                }
                if self.game.rebinding.is_some() {
                    // The journal is waiting for a new key; nothing else sees it.
                    if pressed && !event.repeat {
                        if code == KeyCode::Escape {
                            self.escape();
                        } else {
                            self.game.bind(trigger, key_glyph(&event));
                        }
                    }
                    return;
                }
                if pressed {
                    if let Some(glyph) = key_glyph(&event) {
                        self.game.prefs.bindings.learn_glyph(code, glyph);
                    }
                    self.held.insert(trigger);
                    self.note_sprint_press(trigger);
                } else {
                    self.held.remove(&trigger);
                }
                if pressed && !event.repeat {
                    match code {
                        KeyCode::Escape => self.escape(),
                        KeyCode::F7 => {
                            self.game.vsync = !self.game.vsync;
                            self.game.save_performance();
                        }
                        KeyCode::F8 => {
                            self.game.show_fps = !self.game.show_fps;
                            self.game.save_performance();
                        }
                        KeyCode::F6 => {
                            self.game.shader_intensity = if self.game.shader_intensity > 0. {
                                0.
                            } else {
                                1.
                            };
                            self.game.save_preferences();
                        }
                        KeyCode::F11 => self.toggle_fullscreen(),
                        _ => {
                            // Power choices only apply while leveling and
                            // actions only in the arena, so a digit can be both.
                            if let Some(choice) = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3]
                                .iter()
                                .position(|&k| k == code)
                            {
                                self.game.choose_power(choice);
                            }
                            self.press(trigger);
                        }
                    }
                    self.sync_cursor();
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } if !self.smoke && !self.review => {
                if state == ElementState::Pressed {
                    self.game.device = controls::Device::Keyboard;
                }
                self.mouse_fire = self.game.mode == Mode::Arena && state == ElementState::Pressed;
                self.fire_pressed |= self.mouse_fire;
                self.game.input.fire = self.mouse_fire;
            }
            WindowEvent::MouseInput { state, button, .. } if !self.smoke && !self.review => {
                let trigger = controls::Trigger::Mouse(button);
                let pressed = state == ElementState::Pressed;
                if pressed {
                    self.game.device = controls::Device::Keyboard;
                }
                if self.game.rebinding.is_some() {
                    if pressed {
                        self.game.bind(trigger, None);
                    }
                } else if pressed {
                    self.held.insert(trigger);
                    self.note_sprint_press(trigger);
                    self.press(trigger);
                    self.sync_cursor();
                } else {
                    self.held.remove(&trigger);
                }
            }
            _ => {}
        }
    }
    fn device_event(&mut self, _: &ActiveEventLoop, _: winit::event::DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            if self.captured {
                if delta.0.hypot(delta.1) > 1. {
                    self.game.device = controls::Device::Keyboard;
                }
                self.game.look_sway += glam::Vec2::new(-delta.0 as f32, delta.1 as f32) * 0.00035;
                self.game.look_sway = self
                    .game
                    .look_sway
                    .clamp(glam::Vec2::splat(-0.045), glam::Vec2::splat(0.045));
                let prefs = self.game.prefs;
                let vertical = if prefs.invert_y { -1. } else { 1. };
                self.game.run.yaw += delta.0 as f32 * prefs.sensitivity;
                self.game.run.pitch = (self.game.run.pitch
                    - delta.1 as f32 * prefs.sensitivity * vertical)
                    .clamp(-1.3, 1.3);
            }
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if event_loop.exiting() || self.occluded {
            return;
        }
        // Sleep until the frame limit's next deadline, or draw now.
        match self.pacer.wait_until(Instant::now(), self.frame_limit()) {
            Some(deadline) => {
                event_loop.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(deadline))
            }
            None => {
                event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
        }
    }
}
/// The character a key produces without modifiers, for layout-aware labels.
fn key_glyph(event: &winit::event::KeyEvent) -> Option<char> {
    use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
    let winit::keyboard::Key::Character(text) = event.key_without_modifiers() else {
        return None;
    };
    let mut chars = text.chars();
    chars.next().filter(|_| chars.next().is_none())
}
trait AppIdentity {
    fn with_app_identity(self) -> Self;
}
impl AppIdentity for winit::window::WindowAttributes {
    // Wayland app_id and X11 WM_CLASS; desktops match it to gravewake.desktop.
    #[cfg(target_os = "linux")]
    fn with_app_identity(self) -> Self {
        use winit::platform::wayland::WindowAttributesExtWayland;
        self.with_name("gravewake", "gravewake")
    }
    #[cfg(not(target_os = "linux"))]
    fn with_app_identity(self) -> Self {
        self
    }
}
fn main() {
    let smoke = std::env::args().any(|a| {
        a == "--smoke"
            || a == "--armory-review"
            || a == "--gun-review"
            || a == "--shader-review"
            || a == "--anatomy-review"
            || a == "--survival-review"
            || a == "--benchmark"
            || a == "--text-review"
            || a == "--model-review"
            || a == "--world-review"
            || a == "--fullscreen-review"
            || a == "--pacing-review"
    });
    if std::env::args().any(|a| a == "--fullscreen-review") {
        if let Some(reason) = fullscreen_review::refusal() {
            eprintln!("Refusing to run: {reason}.");
            std::process::exit(2);
        }
    }
    let input_review = std::env::args().any(|a| a == "--input-review");
    if input_review {
        if let Some(reason) = input_review::refusal() {
            eprintln!("Refusing to run: {reason}.");
            std::process::exit(2);
        }
    }
    if std::env::args().any(|a| a == "--anatomy-review") {
        std::fs::create_dir_all("captures/anatomy").unwrap();
    }
    if std::env::args().any(|a| a == "--gun-review") {
        std::fs::create_dir_all("captures/guns").unwrap();
    }
    if std::env::args().any(|a| a == "--shader-review") {
        std::fs::create_dir_all("captures/shaders").unwrap();
    }
    if std::env::args().any(|a| a == "--survival-review") {
        std::fs::create_dir_all("captures/survival").unwrap();
    }
    let review = std::env::args().any(|a| a == "--motion-review");
    if review {
        std::fs::create_dir_all("captures/motion").unwrap();
    }
    if std::env::args().any(|a| a == "--export-audio") {
        std::fs::create_dir_all("captures/audio").unwrap();
        for event in audio::EVENTS {
            audio::write_wav(
                &format!("captures/audio/{event}.wav"),
                &audio::synthesize(event, 0),
            )
            .unwrap();
        }
        music::export("captures/audio").unwrap();
        return;
    }
    if smoke || review || input_review {
        // Scripted runs report progress; a long silence aborts with a core
        // dump. GRAVEWAKE_WATCHDOG_SECS overrides the limit.
        let limit = std::env::var("GRAVEWAKE_WATCHDOG_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(120);
        watchdog::start(std::time::Duration::from_secs(limit));
    }
    let event_loop = EventLoop::new().expect("event loop");
    watchdog::milestone("event loop created");
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    let mut app = App::new(smoke, review);
    // The fullscreen review checks that switches are saved, in the throwaway
    // data folder `fullscreen_review::refusal` insists on.
    if app.fullscreen_review.is_some() {
        app.game.save_enabled = true;
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        event_loop.run_app(&mut app)
    }));
    match result {
        Ok(result) => result.expect("run application"),
        Err(panic) => {
            // Unwinding has already closed the display connection, and dropping
            // the window-bound egui clipboard now would segfault on Wayland.
            // Leak the app so the original panic is the process's exit status.
            std::mem::forget(app);
            std::panic::resume_unwind(panic);
        }
    }
}
