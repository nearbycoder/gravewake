//! Matched native-renderer model portraits and a disposable crowd stress fixture.
//! No player saves/preferences are loaded or written in this mode.
use crate::{
    encounters,
    game::{Enemy, Game, Mode},
    weapons::WeaponKind,
};
use glam::Vec3;

pub struct Review {
    directory: String,
    stage: usize,
    frame: u32,
    finished: bool,
    captures: Vec<String>,
    samples: Vec<[f64; 4]>,
    max_vertices: usize,
    dimensions: [u32; 2],
    presentation: String,
}

impl Review {
    const STRESS: usize = 16;

    pub fn new(before: bool) -> Self {
        let directory = format!(
            "captures/models/{}",
            if std::env::args().any(|arg| arg == "--model-offscreen") {
                "after-offscreen"
            } else if before {
                "before"
            } else if std::env::args().any(|arg| arg == "--model-cpu") {
                "after-cpu"
            } else {
                "after"
            }
        );
        std::fs::create_dir_all(&directory).expect("create native model review directory");
        Self {
            directory,
            stage: if std::env::args().any(|arg| arg == "--model-offscreen") {
                Self::STRESS
            } else {
                0
            },
            frame: 0,
            finished: false,
            captures: vec![],
            samples: vec![],
            max_vertices: 0,
            dimensions: [0; 2],
            presentation: String::new(),
        }
    }

    pub fn finished(&self) -> bool {
        self.finished
    }

    pub fn show_ui(&self) -> bool {
        (13..=16).contains(&self.stage)
    }

    pub fn step(&mut self, game: &mut Game) -> Option<String> {
        self.frame += 1;
        let length = if self.stage == Self::STRESS { 300 } else { 20 };
        if self.frame > length {
            self.stage += 1;
            self.frame = 1;
            if self.stage > Self::STRESS {
                self.finished = true;
                self.save_report();
                return None;
            }
        }
        if self.frame == 1 {
            Self::setup(game, self.stage);
        }
        // Fixed photographic pose. Crowd stress advances only the rendered rig;
        // this measures native model rendering, not AI simulation throughput.
        game.elapsed = 12.;
        if self.stage == Self::STRESS {
            game.elapsed += self.frame as f32 / 120.;
            for (index, enemy) in game.run.enemies.iter_mut().enumerate() {
                enemy.phase = index as f32 * 0.43 + self.frame as f32 / 120.;
            }
        }
        if self.frame != 16 {
            return None;
        }
        let name = match self.stage {
            0..=11 => format!("{:02}-enemy-{:02}.png", self.stage, self.stage),
            12 => "12-environment.png".into(),
            13 => "13-pistol-first-person.png".into(),
            14 => "14-double-first-person.png".into(),
            15 => "15-double-reload.png".into(),
            _ => "16-crowd-48.png".into(),
        };
        self.captures.push(name.clone());
        Some(format!("{}/{name}", self.directory))
    }

    pub fn record(
        &mut self,
        frame_ms: f64,
        renderer_ms: [f64; 3],
        vertices: usize,
        dimensions: [u32; 2],
        presentation: String,
    ) {
        // Screenshot is frame 16; samples start at 61, excluding GPU readback,
        // disk I/O, pipeline warm-up and the frame immediately following capture.
        if self.stage == Self::STRESS && self.frame > 60 {
            self.samples
                .push([frame_ms, renderer_ms[0], renderer_ms[1], renderer_ms[2]]);
            self.max_vertices = self.max_vertices.max(vertices);
            self.dimensions = dimensions;
            self.presentation = presentation;
        }
    }

    fn setup(game: &mut Game, stage: usize) {
        *game = Game::new(false);
        game.new_run();
        game.mode = Mode::Paused;
        game.vsync = false;
        game.notice_time = 0.;
        game.run.survival.remaining = 0;
        game.run.survival.orbs.clear();
        game.run.enemies.clear();
        game.shot_age = 10.;
        if stage < 12 {
            let scale = encounters::species(stage).scale;
            let (height, distance) = match stage {
                2 => (2.05, 0.86),
                3 => (1.17, 2.25),
                4 => (2.03, 1.85),
                5 => (0.47, 1.28),
                _ => (1.03, 2.04),
            };
            game.run.pos = Vec3::new(0., height * scale, distance * scale);
            game.run.yaw = 0.;
            game.run.pitch = 0.;
            let mut enemy = Enemy::spawn(stage, Vec3::ZERO, 1, 0.17);
            enemy.attack = 10.;
            game.run.enemies.push(enemy);
        } else if stage == 12 {
            game.run.pos = Vec3::new(8., 3.2, 12.);
            let direction = Vec3::new(-2., 1.3, -8.) - game.run.pos;
            game.run.yaw = direction.x.atan2(-direction.z);
            game.run.pitch = (direction.y / direction.length()).asin();
        } else {
            game.mode = Mode::Arena;
            game.run.pos = Vec3::new(0., 1.65, 9.);
            game.run.yaw = 0.;
            game.run.pitch = -0.04;
            game.run.weapon.kind = if stage == 13 {
                WeaponKind::Pistol
            } else {
                WeaponKind::Double
            };
            game.run.weapon.rarity = 0;
            game.run.ammo = game.run.weapon.capacity();
            if stage == 15 {
                game.reload = game.run.weapon.reload_time() * 0.52;
            }
            let count = if stage == Self::STRESS { 48 } else { 3 };
            for index in 0..count {
                let mut enemy = Enemy::spawn(
                    if stage == Self::STRESS {
                        index % 12
                    } else {
                        index
                    },
                    Vec3::new(
                        (index % 8) as f32 * 1.9 - 6.65,
                        0.,
                        3. - (index / 8) as f32 * 2.5,
                    ),
                    4,
                    index as f32 * 0.43,
                );
                enemy.attack = 10.;
                game.run.enemies.push(enemy);
            }
            if stage == Self::STRESS {
                game.run.pos = Vec3::new(0., 2.9, 14.);
                game.run.pitch = -0.15;
            }
        }
    }

    fn save_report(&self) {
        let offscreen = std::env::args().any(|arg| arg == "--model-offscreen");
        let count = self.samples.len();
        assert_eq!(count, 240, "model stress frame count");
        let average = |column: usize| {
            self.samples
                .iter()
                .map(|sample| sample[column])
                .sum::<f64>()
                / count as f64
        };
        let mut frame_times: Vec<_> = self.samples.iter().map(|sample| sample[0]).collect();
        frame_times.sort_by(f64::total_cmp);
        let report = serde_json::json!({
            "captures": self.captures,
            "player_save_loaded": false,
            "player_save_written": false,
            "preferences_loaded_or_written": false,
            "stress": {
                "description": if offscreen { "48 animated enemy models, native renderer; frozen AI; offscreen target with explicit per-frame GPU completion wait" } else { "48 animated enemy models, native renderer; frozen AI; presentation included" },
                "submit_measurement": if offscreen { "CPU submission plus explicit GPU completion wait" } else { "CPU submission and presentation call" },
                "measured_frames": count,
                "warmup_frames_excluded": 60,
                "capture_frames_measured": 0,
                "width": self.dimensions[0], "height": self.dimensions[1],
                "presentation": self.presentation,
                "mean_frame_ms": average(0),
                "p95_frame_ms": frame_times[count * 95 / 100],
                "mean_fps": 1000. / average(0),
                "mean_mesh_cpu_ms": average(1),
                "mean_surface_acquire_ms": average(2),
                "mean_submit_cpu_ms": average(3),
                "max_dynamic_vertices": self.max_vertices,
            }
        });
        std::fs::write(
            format!("{}/manifest.json", self.directory),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .expect("write native model review manifest");
        println!(
            "MODEL REVIEW PASS: {} captures; {}",
            self.captures.len(),
            report["stress"]
        );
    }
}
