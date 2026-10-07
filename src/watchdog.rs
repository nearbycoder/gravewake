//! Progress watchdog for scripted runs (smoke, reviews, benchmark).
//!
//! A rare start-up stall has been seen in smoke runs, and Yama blocks
//! attaching a debugger to a running process. If no progress is reported for
//! the time limit, the watchdog names the last milestone and aborts, so the
//! system's core-dump handler keeps every thread's stack. Normal play never
//! starts it.
use std::{
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

static STARTED: AtomicBool = AtomicBool::new(false);
/// Milliseconds since `EPOCH` at the last reported progress.
static LAST: AtomicU64 = AtomicU64::new(0);
static FRAMES: AtomicU64 = AtomicU64::new(0);
static MILESTONE: Mutex<&'static str> = Mutex::new("process start");
static EPOCH: Mutex<Option<Instant>> = Mutex::new(None);

fn now_ms() -> u64 {
    let epoch = *EPOCH.lock().unwrap().get_or_insert_with(Instant::now);
    epoch.elapsed().as_millis() as u64
}

/// Start watching. Without a call to `start`, `milestone` and `frame` only
/// record state, so they are cheap to leave in normal play.
pub fn start(limit: Duration) {
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    LAST.store(now_ms(), Ordering::SeqCst);
    std::thread::Builder::new()
        .name("watchdog".into())
        .spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(1));
                let quiet = now_ms().saturating_sub(LAST.load(Ordering::SeqCst));
                if let Some(report) = stalled(quiet, limit) {
                    eprintln!("{report}");
                    std::process::abort();
                }
            }
        })
        .expect("start watchdog thread");
}

/// The report for a stall, or `None` while progress is recent enough.
fn stalled(quiet_ms: u64, limit: Duration) -> Option<String> {
    (quiet_ms >= limit.as_millis() as u64).then(|| {
        format!(
            "WATCHDOG: no progress for {:.0} s after \"{}\" (frame {}); aborting so a core dump records every thread",
            quiet_ms as f64 / 1000.,
            MILESTONE.lock().unwrap(),
            FRAMES.load(Ordering::SeqCst),
        )
    })
}

/// Record a named start-up or shutdown step.
pub fn milestone(name: &'static str) {
    *MILESTONE.lock().unwrap() = name;
    LAST.store(now_ms(), Ordering::SeqCst);
}

/// Record the step a frame has reached without counting it as progress, so a
/// frame that keeps failing partway still trips the watchdog.
pub fn step(name: &'static str) {
    *MILESTONE.lock().unwrap() = name;
}

/// Record a completed frame.
pub fn frame() {
    FRAMES.fetch_add(1, Ordering::SeqCst);
    milestone("frame presented");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reports_only_after_the_limit_and_names_the_last_step() {
        let limit = Duration::from_secs(120);
        milestone("GPU device ready");
        assert!(stalled(119_999, limit).is_none());
        let report = stalled(120_000, limit).unwrap();
        assert!(report.contains("\"GPU device ready\""), "{report}");
        assert!(report.contains("120 s"), "{report}");
        // Steps inside a frame name themselves in the report but aren't progress.
        let last = LAST.load(Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(5));
        step("surface lost or outdated");
        assert_eq!(LAST.load(Ordering::SeqCst), last);
        assert!(stalled(120_000, limit).unwrap().contains("\"surface lost or outdated\""));
        frame();
        assert!(LAST.load(Ordering::SeqCst) > last);
    }
}
