//! `--fidelity-review`: the same two staged arena frames at every Graphics
//! fidelity step, with a capture and a frame-time measurement for each, then
//! the journal's Display page driven through real pointer and D-pad input.
//! Each timed stage warms up for 60 frames and measures the next 300 frame
//! intervals with VSync off and no frame limit; capture frames are never
//! timed. Results and the load average go to `captures/fidelity/report.json`.
//! Game time is frozen, so every step draws the same frame. No player save
//! or preference is loaded or written. With `--fidelity-offscreen` it draws
//! to an offscreen target and waits for the GPU to finish every frame, so
//! the times are each step's whole CPU and GPU work, without presentation;
//! results then go to `captures/fidelity/offscreen/`.
use crate::{
    fidelity::Fidelity,
    game::{Enemy, Game, JournalPage, Mode},
    gamepad::Button,
    weapons::WeaponKind,
};
use glam::Vec3;
use serde::Serialize;

/// Whether this run draws offscreen and waits for the GPU each frame.
pub fn offscreen() -> bool {
    std::env::args().any(|a| a == "--fidelity-review")
        && std::env::args().any(|a| a == "--fidelity-offscreen")
}
fn dir() -> &'static str {
    if offscreen() {
        "captures/fidelity/offscreen"
    } else {
        "captures/fidelity"
    }
}
const WARM_UP: u32 = 60;
const MEASURED: u32 = 300;
/// The capture frame, after the measured ones; the stage ends two frames on.
const CAPTURE: u32 = WARM_UP + MEASURED + 2;
const SCENES: [&str; 2] = ["brazier", "court"];
const TIMED: usize = SCENES.len() * Fidelity::ALL.len();

#[derive(Serialize)]
struct Stage {
    scene: &'static str,
    fidelity: &'static str,
    scene_size: [u32; 2],
    window: [u32; 2],
    frames: usize,
    mean_ms: f64,
    p95_ms: f64,
    max_ms: f64,
    fps: f64,
    load_before: String,
    load_after: String,
    capture: String,
}

pub struct Review {
    stage: usize,
    frame: u32,
    intervals: Vec<f64>,
    load_before: String,
    window: [u32; 2],
    results: Vec<Stage>,
    finished: bool,
}

fn load() -> String {
    std::fs::read_to_string("/proc/loadavg")
        .map(|l| l.split_whitespace().take(3).collect::<Vec<_>>().join(" "))
        .unwrap_or_else(|_| "n/a".into())
}

/// The fidelity slider's stops on the Display page, in design units.
fn stop_x(index: usize) -> f32 {
    615. + 360. * index as f32 / (Fidelity::ALL.len() - 1) as f32
}

impl Review {
    pub fn new() -> Self {
        std::fs::create_dir_all(dir()).expect("create fidelity review directory");
        Self {
            stage: 0,
            frame: 0,
            intervals: vec![],
            load_before: String::new(),
            window: [0; 2],
            results: vec![],
            finished: false,
        }
    }
    pub fn finished(&self) -> bool {
        self.finished
    }
    /// Timed frames draw the 3D scene alone; the last stage shows the journal.
    pub fn show_ui(&self) -> bool {
        self.stage >= TIMED
    }
    fn fidelity(stage: usize) -> Fidelity {
        Fidelity::ALL[stage % Fidelity::ALL.len()]
    }
    fn setup(game: &mut Game, stage: usize) {
        *game = Game::new(false);
        game.new_run();
        game.mode = Mode::Arena;
        game.vsync = false;
        game.notice_time = 0.;
        game.prefs.field_tips = false;
        game.run.survival.remaining = 0;
        game.run.survival.orbs.clear();
        game.run.enemies.clear();
        game.shot_age = 10.;
        game.run.weapon.kind = WeaponKind::Double;
        game.run.weapon.rarity = 1;
        game.run.ammo = 2;
        game.prefs.fidelity = Self::fidelity(stage.min(TIMED - 1));
        let scene = SCENES[(stage / Fidelity::ALL.len()).min(SCENES.len() - 1)];
        let look = |game: &mut Game, from: Vec3, at: Vec3| {
            game.run.pos = from;
            let d = at - from;
            game.run.yaw = d.x.atan2(-d.z);
            game.run.pitch = (d.y / d.length()).asin();
        };
        if scene == "brazier" {
            // The brazier at (6, 1.4, -2), with creatures between it and the
            // camera and beside it, so its light throws their shadows forward.
            look(game, Vec3::new(7.2, 1.65, 5.2), Vec3::new(6., 0.7, -2.));
            for (kind, pos) in [
                (0, Vec3::new(5.0, 0., 0.1)),
                (1, Vec3::new(7.6, 0., 0.6)),
                (4, Vec3::new(4.2, 0., -3.2)),
                (6, Vec3::new(8.4, 0., -3.4)),
            ] {
                let mut enemy = Enemy::spawn(kind, pos, 1, kind as f32 * 0.37);
                enemy.attack = 10.;
                game.run.enemies.push(enemy);
            }
        } else {
            // The Mourning Court from above, with 24 creatures across it.
            look(game, Vec3::new(0., 2.9, 14.), Vec3::new(0., 0.8, -4.));
            for index in 0..24 {
                let mut enemy = Enemy::spawn(
                    index % 12,
                    Vec3::new(
                        (index % 8) as f32 * 1.9 - 6.65,
                        0.,
                        5. - (index / 8) as f32 * 2.6,
                    ),
                    4,
                    index as f32 * 0.43,
                );
                enemy.attack = 10.;
                game.run.enemies.push(enemy);
            }
        }
        if stage >= TIMED {
            game.mode = Mode::Paused;
            game.settings = true;
            game.journal_page = JournalPage::Display;
            game.prefs.fidelity = Fidelity::High;
        }
    }
    /// Called once per frame before drawing; returns a capture path.
    pub fn step(&mut self, game: &mut Game) -> Option<String> {
        self.frame += 1;
        if self.frame == 1 {
            Self::setup(game, self.stage);
            self.load_before = load();
        }
        game.elapsed = 12.;
        game.run.enemies.iter_mut().for_each(|e| e.attack = 10.);
        if self.stage < TIMED {
            if self.frame == CAPTURE {
                let scene = SCENES[self.stage / Fidelity::ALL.len()];
                let name = format!(
                    "{}/{}-{}.png",
                    dir(),
                    scene,
                    Self::fidelity(self.stage).name().to_lowercase()
                );
                return Some(name);
            }
            if self.frame >= CAPTURE + 2 {
                self.finish_stage(game);
            }
            return None;
        }
        // The Display page: pointer clicks on the Low and Ultra stops, then
        // two D-pad presses left from Ultra.
        match self.frame {
            30 => assert_eq!(game.prefs.fidelity, Fidelity::Low, "FIDELITY REVIEW: a click on Low"),
            50 => assert_eq!(game.prefs.fidelity, Fidelity::Ultra, "FIDELITY REVIEW: a click on Ultra"),
            70 => assert_eq!(game.prefs.fidelity, Fidelity::High, "FIDELITY REVIEW: D-pad left to High"),
            80 => assert_eq!(game.prefs.fidelity, Fidelity::Medium, "FIDELITY REVIEW: D-pad left to Medium"),
            90 => return Some(format!("{}/display-page.png", dir())),
            94 => {
                self.finished = true;
                self.save_report();
            }
            _ => {}
        }
        None
    }
    fn finish_stage(&mut self, game: &Game) {
        let fidelity = Self::fidelity(self.stage);
        let scene = SCENES[self.stage / Fidelity::ALL.len()];
        let mut sorted = self.intervals.clone();
        sorted.sort_by(f64::total_cmp);
        let frames = sorted.len();
        assert_eq!(frames as u32, MEASURED, "FIDELITY REVIEW: measured frames");
        let mean_ms = sorted.iter().sum::<f64>() / frames as f64;
        let (w, h) = fidelity.profile().scene_size(self.window[0], self.window[1]);
        let stage = Stage {
            scene,
            fidelity: fidelity.name(),
            scene_size: [w, h],
            window: self.window,
            frames,
            mean_ms,
            p95_ms: sorted[frames * 95 / 100],
            max_ms: sorted[frames - 1],
            fps: 1000. / mean_ms,
            load_before: self.load_before.clone(),
            load_after: load(),
            capture: format!("{scene}-{}.png", fidelity.name().to_lowercase()),
        };
        assert_eq!(game.prefs.fidelity, fidelity);
        println!(
            "FIDELITY REVIEW: {scene} {}: scene {w}×{h} in a {}×{} window, mean {:.2} ms \
             ({:.0} FPS), p95 {:.2} ms, max {:.2} ms over {frames} frames; load {} → {}",
            stage.fidelity,
            self.window[0],
            self.window[1],
            stage.mean_ms,
            stage.fps,
            stage.p95_ms,
            stage.max_ms,
            stage.load_before,
            stage.load_after,
        );
        self.results.push(stage);
        self.intervals.clear();
        self.stage += 1;
        self.frame = 0;
    }
    /// The interval since the previous frame started, and the window size.
    pub fn record(&mut self, frame_ms: f64, window: [u32; 2]) {
        self.window = window;
        if self.stage < TIMED && (WARM_UP + 1..=WARM_UP + MEASURED).contains(&self.frame) {
            self.intervals.push(frame_ms);
        }
    }
    /// Pointer clicks on the Display page, in design units, and whether
    /// pressed or released.
    pub fn click(&self) -> Option<((f32, f32), bool)> {
        if self.stage < TIMED {
            return None;
        }
        let y = crate::ui::DISPLAY_FIDELITY_Y + 1.;
        match self.frame {
            20 | 21 => Some(((stop_x(0), y), self.frame == 20)),
            40 | 41 => Some(((stop_x(3), y), self.frame == 40)),
            _ => None,
        }
    }
    /// Where the controller cursor starts (on the Ultra stop, in design
    /// units), and the D-pad presses this frame.
    pub fn pad(&self) -> Option<((f32, f32), Vec<Button>)> {
        if self.stage < TIMED || !(55..=85).contains(&self.frame) {
            return None;
        }
        let presses = match self.frame {
            62 | 72 => vec![Button::DPadLeft],
            _ => vec![],
        };
        Some(((stop_x(3), crate::ui::DISPLAY_FIDELITY_Y + 1.), presses))
    }
    fn save_report(&self) {
        std::fs::write(
            format!("{}/report.json", dir()),
            serde_json::to_vec_pretty(&self.results).unwrap(),
        )
        .expect("write fidelity report");
        println!(
            "FIDELITY REVIEW PASS: {} timed stages, the Display page's slider moved by pointer and D-pad",
            self.results.len()
        );
    }
}
