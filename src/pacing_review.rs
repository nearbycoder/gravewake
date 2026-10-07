//! `--pacing-review`: measure the frame limit on the title screen. Each
//! stage warms up for a second, then counts frames, frame intervals and the
//! process's CPU time for five seconds. The background stage sets the same
//! unfocused flag a lost focus sets, since nothing here can take focus from
//! the window. Results go to `captures/pacing/report.json`. The run fails if
//! a limited stage draws more than 2% above its limit, or more than 2% below
//! it when 95% of the unlimited title's frames took under 90% of the limit's
//! period.
use crate::pacing;
use serde::Serialize;
use std::time::{Duration, Instant};

const DIR: &str = "captures/pacing";
const WARM_UP: Duration = Duration::from_secs(1);
const MEASURE: Duration = Duration::from_secs(5);
/// Each stage: its name, the chosen limit and whether the window has focus.
const STAGES: [(&str, u32, bool); 5] = [
    ("off", 0, true),
    ("limit-30", 30, true),
    ("limit-60", 60, true),
    ("limit-144", 144, true),
    ("background", 0, false),
];

#[derive(Serialize)]
struct Stage {
    name: &'static str,
    limit: u32,
    frames: usize,
    fps: f64,
    mean_ms: f64,
    p95_ms: f64,
    max_ms: f64,
    /// CPU time the whole process used, as a share of one core.
    cpu_percent: Option<f64>,
}

pub struct Review {
    stage: usize,
    started: Instant,
    last: Option<Instant>,
    intervals: Vec<f64>,
    cpu_start: Option<f64>,
    results: Vec<Stage>,
    pub finished: bool,
}

/// User and system CPU time of this process in seconds (Linux only).
fn cpu_seconds() -> Option<f64> {
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    // Fields after the command name, which ends with the last ')'.
    let fields: Vec<&str> = stat.rsplit_once(')')?.1.split_whitespace().collect();
    let utime: f64 = fields.get(11)?.parse().ok()?;
    let stime: f64 = fields.get(12)?.parse().ok()?;
    // Linux reports these in USER_HZ, which is 100 on every common build.
    Some((utime + stime) / 100.)
}

impl Review {
    pub fn new() -> Self {
        std::fs::create_dir_all(DIR).expect("create pacing review directory");
        Self {
            stage: 0,
            started: Instant::now(),
            last: None,
            intervals: vec![],
            cpu_start: None,
            results: vec![],
            finished: false,
        }
    }
    /// The limit in effect for this stage, through the same function the
    /// game uses for focus.
    pub fn limit(&self) -> u32 {
        let (_, chosen, focused) = STAGES[self.stage.min(STAGES.len() - 1)];
        pacing::effective_limit(chosen, focused)
    }
    /// A frame started at `now`.
    pub fn frame(&mut self, now: Instant) {
        if self.finished {
            return;
        }
        let elapsed = now - self.started;
        if elapsed < WARM_UP {
            return;
        }
        if self.cpu_start.is_none() && self.last.is_none() {
            self.cpu_start = cpu_seconds();
        }
        if let Some(last) = self.last {
            self.intervals.push((now - last).as_secs_f64() * 1000.);
        }
        self.last = Some(now);
        if elapsed < WARM_UP + MEASURE {
            return;
        }
        self.finish_stage();
    }
    fn finish_stage(&mut self) {
        let (name, chosen, _) = STAGES[self.stage];
        let limit = self.limit();
        let mut sorted = self.intervals.clone();
        sorted.sort_by(f64::total_cmp);
        let total: f64 = sorted.iter().sum();
        let frames = sorted.len();
        let mean_ms = total / frames.max(1) as f64;
        let stage = Stage {
            name,
            limit,
            frames,
            fps: 1000. / mean_ms,
            mean_ms,
            p95_ms: sorted.get(frames * 95 / 100).copied().unwrap_or(0.),
            max_ms: sorted.last().copied().unwrap_or(0.),
            cpu_percent: self
                .cpu_start
                .zip(cpu_seconds())
                .map(|(a, b)| (b - a) / (total / 1000.) * 100.),
        };
        println!(
            "PACING REVIEW: {name} (chosen {chosen}, in effect {limit}): {:.1} FPS over {} frames, \
             mean {:.2} ms, p95 {:.2} ms, max {:.2} ms, CPU {}",
            stage.fps,
            stage.frames,
            stage.mean_ms,
            stage.p95_ms,
            stage.max_ms,
            stage
                .cpu_percent
                .map_or("n/a".into(), |c| format!("{c:.0}% of a core")),
        );
        if limit > 0 {
            // Never above the limit; and on it, when nearly every unlimited
            // frame was shorter than the limit's period. A limit can't make
            // slow frames faster, so on a busy machine whose frames often run
            // long it only caps.
            assert!(
                stage.fps < limit as f64 * 1.02,
                "PACING REVIEW: {name}: {:.2} FPS is more than 2% above {limit}",
                stage.fps
            );
            let unlimited = &self.results[0];
            let period_ms = 1000. / limit as f64;
            if unlimited.p95_ms < period_ms * 0.9 {
                assert!(
                    stage.fps > limit as f64 * 0.98,
                    "PACING REVIEW: {name}: {:.2} FPS is more than 2% below {limit}, though \
                     95% of unlimited frames took under {:.1} ms",
                    stage.fps,
                    unlimited.p95_ms
                );
            } else {
                println!(
                    "PACING REVIEW: {name}: 5% of unlimited frames took {:.1} ms or more, \
                     longer than 90% of {limit}'s {period_ms:.1} ms period, so {limit} is \
                     checked as a cap only",
                    unlimited.p95_ms
                );
            }
        }
        self.results.push(stage);
        self.stage += 1;
        self.started = Instant::now();
        self.last = None;
        self.cpu_start = None;
        self.intervals.clear();
        if self.stage == STAGES.len() {
            let unlimited = self.results[0].fps;
            assert!(
                unlimited > 45.,
                "PACING REVIEW: without a limit the title drew only {unlimited:.0} FPS, too \
                 close to 30 to show the limiter working"
            );
            std::fs::write(
                format!("{DIR}/report.json"),
                serde_json::to_vec_pretty(&self.results).unwrap(),
            )
            .unwrap();
            println!("PACING REVIEW PASS: no limited stage above its limit, and each reachable one within 2% of it");
            self.finished = true;
        }
    }
}
