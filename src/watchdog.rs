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
    time::Duration,
};
use web_time::Instant;

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
                    report_and_abort(report);
                }
            }
        })
        .expect("start watchdog thread");
}

/// Save the report, try to print it, and abort whatever happens to stderr.
/// If stdout and stderr are a pipe that has stopped draining, the main thread
/// can be stuck writing to it, and so would an `eprintln!` here: the round 5
/// hang (no report, killed by an outer timeout) looked like that.
fn report_and_abort(report: String) -> ! {
    // A file write can't block on a reader; scripted runs work in the repo.
    let path = format!("captures/watchdog-{}.txt", std::process::id());
    let saved = std::fs::create_dir_all("captures")
        .and_then(|()| std::fs::write(&path, format!("{report}\n")))
        .is_ok();
    let _ = std::thread::Builder::new()
        .name("watchdog report".into())
        .spawn(move || {
            eprintln!("{report}");
            if saved {
                eprintln!("WATCHDOG: report saved to {path}");
            }
        });
    std::thread::sleep(REPORT_GRACE);
    std::process::abort();
}
/// How long the report may take to print before the abort goes ahead.
const REPORT_GRACE: Duration = Duration::from_secs(2);

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
