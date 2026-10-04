//! Disposable native tour of the expanded arena. Tour legs use Game::update,
//! sprint input and the same obstacle navigation as the live game. Fixture resets
//! are limited to the separate wall-contact, crowd and last-threat checks.
use crate::{
    game::{Enemy, Game, Input, Mode},
    world_layout::{self, Navigation, PLAYER_RADIUS},
};
use glam::Vec3;

struct Stop {
    name: &'static str,
    destination: Vec3,
    look_at: Vec3,
}
const STOPS: [Stop; 6] = [
    Stop {
        name: "00-mourning-court",
        destination: Vec3::new(0., 1.65, 9.),
        look_at: Vec3::new(0., 2.1, -8.),
    },
    Stop {
        name: "01-ruined-chapel",
        destination: Vec3::new(-29., 1.65, -12.),
        look_at: Vec3::new(-29., 5.3, 4.),
    },
    Stop {
        name: "02-bell-sanctuary",
        destination: Vec3::new(0., 1.65, -17.),
        look_at: Vec3::new(0., 8.5, -35.),
    },
    Stop {
        name: "03-sunken-cloister",
        destination: Vec3::new(29., 1.65, -7.),
        look_at: Vec3::new(34., 3.6, -17.),
    },
    Stop {
        name: "04-ash-orchard",
        destination: Vec3::new(0., 1.65, 28.),
        look_at: Vec3::new(5., 3.2, 39.),
    },
    Stop {
        name: "05-outer-processional-path",
        destination: Vec3::new(-42., 1.65, 35.),
        look_at: Vec3::new(-29., 5., 4.),
    },
];
const CONTACT: usize = STOPS.len();
const CROWD: usize = CONTACT + 1;
const LAST_THREAT: usize = CROWD + 1;

pub struct Review {
    directory: String,
    stage: usize,
    frame: u32,
    settled: u32,
    finished: bool,
    navigation: Navigation,
    previous: Option<Vec3>,
    distance_walked: f32,
    movement_frames: usize,
    captures: Vec<String>,
    arrivals: Vec<serde_json::Value>,
    samples: Vec<[f64; 5]>,
    max_vertices: usize,
    dimensions: [u32; 2],
    presentation: String,
}
impl Review {
    pub fn new() -> Self {
        let small = std::env::args().any(|arg| arg == "--review-small");
        let offscreen = std::env::args().any(|arg| arg == "--world-offscreen");
        let directory = match (small, offscreen) {
            (false, false) => "captures/world/normal",
            (true, false) => "captures/world/small",
            (false, true) => "captures/world/offscreen",
            (true, true) => "captures/world/offscreen-small",
        }
        .to_string();
        std::fs::create_dir_all(&directory).expect("create world-review directory");
        Self {
            directory,
            stage: 0,
            frame: 0,
            settled: 0,
            finished: false,
            navigation: Navigation::default(),
            previous: None,
            distance_walked: 0.,
            movement_frames: 0,
            captures: vec![],
            arrivals: vec![],
            samples: vec![],
            max_vertices: 0,
            dimensions: [0; 2],
            presentation: String::new(),
        }
    }
    pub fn finished(&self) -> bool {
        self.finished
    }
    pub fn step(&mut self, game: &mut Game) -> Option<String> {
        if self.frame == 0 {
            self.setup(game);
        }
        self.frame += 1;
        assert!(
            self.frame < 3600,
            "World tour stalled before stage {} at {:?}",
            self.stage,
            game.run.pos
        );
        game.run.hp = game.max_hp();
        game.notice_time = 0.;
        game.input = Input::default();
        if self.stage < STOPS.len() {
            let stop = &STOPS[self.stage];
            let distance = ((game.run.pos - stop.destination) * Vec3::new(1., 0., 1.)).length();
            if distance > 0.4 && self.settled == 0 {
                let direction =
                    self.navigation
                        .direction(game.run.pos, stop.destination, PLAYER_RADIUS);
                game.run.yaw = direction.x.atan2(-direction.z);
                game.run.pitch = 0.;
                game.input.forward = 1.;
                game.input.sprint = true;
                self.movement_frames += 1;
            } else {
                self.settled += 1;
                Self::look_at(game, stop.look_at);
                if self.settled == 12 {
                    self.arrivals.push(serde_json::json!({"name":stop.name,"position":game.run.pos.to_array(),
                        "distance_to_target_m":distance,"arrival_simulation_seconds":game.run.time}));
                    return self.capture(stop.name);
                }
                if self.settled >= 30 {
                    self.advance();
                }
            }
        } else if self.stage == CONTACT {
            game.run.yaw = -std::f32::consts::FRAC_PI_2;
            game.input.forward = 1.;
            game.input.sprint = true;
            if self.frame == 90 {
                assert!(
                    game.run.pos.x < -18.5 && game.run.pos.x > -19.,
                    "Player must contact but not cross the chapel wall: {:?}",
                    game.run.pos
                );
                return self.capture("06-chapel-wall-contact");
            }
            if self.frame >= 105 {
                self.advance();
            }
        } else if self.stage == CROWD {
            // Actual AI and physics continue for the entire fixture, off origin.
            if self.frame == 16 {
                return self.capture("07-cloister-48-enemy-fight");
            }
            if self.frame >= 300 {
                self.advance();
            }
        } else {
            debug_assert_eq!(self.stage, LAST_THREAT);
            if self.frame == 16 {
                return self.capture("08-last-threat-bearing");
            }
            if self.frame >= 32 {
                self.finished = true;
                self.save_report();
            }
        }
        None
    }
    fn capture(&mut self, name: &str) -> Option<String> {
        let file = format!("{name}.png");
        self.captures.push(file.clone());
        Some(format!("{}/{file}", self.directory))
    }
    fn advance(&mut self) {
        self.stage += 1;
        self.frame = 0;
        self.settled = 0;
    }
    fn look_at(game: &mut Game, target: Vec3) {
        let d = target - game.run.pos;
        game.run.yaw = d.x.atan2(-d.z);
        game.run.pitch = (d.y / d.length()).clamp(-1., 1.).asin();
    }
    fn setup(&mut self, game: &mut Game) {
        if self.stage == 0 {
            *game = Game::new(false);
            game.new_run();
            game.mode = Mode::Arena;
            game.vsync = false;
            game.run.pos = STOPS[0].destination;
            game.run.survival.remaining = 0;
            game.run.survival.orbs.clear();
            game.run.enemies.clear();
            game.shot_age = 10.;
            // Four live pursuers keep the arena running during the tour. Their
            // movement and obstacle avoidance are real; player health is protected.
            for i in 0..4 {
                let p = world_layout::spawn_point(
                    game.run.pos,
                    i as f32 * 1.5,
                    18.,
                    world_layout::ENEMY_RADIUS,
                );
                game.run.enemies.push(Enemy::spawn(i % 3, p, 1, i as f32));
            }
        }
        if self.stage < STOPS.len() {
            self.navigation.update(STOPS[self.stage].destination);
        } else {
            game.mode = Mode::Arena;
            game.run.enemies.clear();
            game.hazards.clear();
            game.run.survival.remaining = 0;
            game.run.survival.orbs.clear();
            game.run.time = game.run.time.max(12.);
            if self.stage == CONTACT {
                game.run.pos = Vec3::new(-16., 1.65, -14.);
                game.run
                    .enemies
                    .push(Enemy::spawn(0, Vec3::new(-29., 0., -8.), 1, 0.));
            } else if self.stage == CROWD {
                game.run.pos = Vec3::new(29., 1.65, 4.);
                game.run.pitch = -0.035;
                game.run.yaw = 0.;
                for i in 0..48 {
                    let p = world_layout::resolve_position(
                        Vec3::new(19. + (i % 8) as f32 * 2.3, 0., -1. - (i / 8) as f32 * 2.4),
                        world_layout::ENEMY_RADIUS,
                    );
                    game.run
                        .enemies
                        .push(Enemy::spawn(i % 12, p, 4, i as f32 * 0.43));
                }
            } else {
                game.run.pos = Vec3::new(26., 1.65, -5.);
                game.run.yaw = 0.;
                game.run.pitch = 0.;
                game.run
                    .enemies
                    .push(Enemy::spawn(0, Vec3::new(16., 0., 3.), 1, 0.));
            }
            self.previous = None;
        }
        game.notice_time = 0.;
    }
    pub fn observe(&mut self, game: &Game) {
        assert!(game.run.pos.is_finite());
        assert!(
            world_layout::is_walkable(game.run.pos, PLAYER_RADIUS),
            "Review player inside solid: {:?}",
            game.run.pos
        );
        if self.stage < STOPS.len() {
            if let Some(previous) = self.previous {
                let d = game.run.pos.distance(previous);
                assert!(d < 0.2, "Tour teleported instead of moving: {d}");
                self.distance_walked += d;
            }
        }
        self.previous = Some(game.run.pos);
    }
    pub fn record(
        &mut self,
        frame_ms: f64,
        simulation_ms: f64,
        renderer_ms: [f64; 3],
        vertices: usize,
        dimensions: [u32; 2],
        presentation: String,
    ) {
        if self.stage == CROWD && self.frame > 60 {
            self.samples.push([
                frame_ms,
                simulation_ms,
                renderer_ms[0],
                renderer_ms[1],
                renderer_ms[2],
            ]);
            self.max_vertices = self.max_vertices.max(vertices);
            self.dimensions = dimensions;
            self.presentation = presentation;
        }
    }
    fn save_report(&self) {
        assert_eq!(self.arrivals.len(), STOPS.len());
        assert!(
            self.distance_walked > 175.,
            "Expanded tour must actually cover the larger grounds"
        );
        assert!(self.movement_frames > 1000);
        assert_eq!(self.samples.len(), 239);
        let average = |column: usize| {
            self.samples.iter().map(|s| s[column]).sum::<f64>() / self.samples.len() as f64
        };
        let mut sorted: Vec<_> = self.samples.iter().map(|s| s[0]).collect();
        sorted.sort_by(f64::total_cmp);
        let report = serde_json::json!({"captures":self.captures,"player_save_loaded":false,"player_save_written":false,
            "preferences_loaded_or_written":false,"tour":{"method":"Actual Game::update sprint input and collision; obstacle navigation chooses heading; no teleport between six tour stops",
            "distance_walked_m":self.distance_walked,"movement_frames":self.movement_frames,"arrivals":self.arrivals},
            "collision_probe":"Player sprinted into chapel wall for90 simulation frames and remained outside",
            "last_threat":"One living enemy with no reinforcements; bearing shown",
            "stress":{"description":"48 enemies initialized in the eastern cloister; actual AI and physics advance; player health protected; no shooting",
            "measurement_type":if std::env::args().any(|arg|arg=="--world-offscreen") { "Offscreen diagnostic: actual simulation and native rendering with explicit per-frame GPU completion wait; not visible window presentation FPS" } else { "Visible native window: actual simulation, surface acquisition, render submission and presentation included" },
            "gpu_completion_wait":std::env::args().any(|arg|arg=="--world-offscreen"),"position":[29.,1.65,4.],"measured_frames":self.samples.len(),"warmup_frames_excluded":60,"capture_frames_measured":0,
            "width":self.dimensions[0],"height":self.dimensions[1],"presentation":self.presentation,"mean_frame_ms":average(0),"p95_frame_ms":sorted[sorted.len()*95/100],
            "mean_fps":1000./average(0),"mean_simulation_ms":average(1),"mean_mesh_cpu_ms":average(2),"mean_surface_acquire_ms":average(3),"mean_submit_cpu_ms":average(4),"max_dynamic_vertices":self.max_vertices}});
        std::fs::write(
            format!("{}/manifest.json", self.directory),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .expect("write world-review manifest");
        println!(
            "WORLD REVIEW PASS: {:.1} metres walked, {} native captures; {}",
            self.distance_walked,
            self.captures.len(),
            report["stress"]
        );
    }
}
