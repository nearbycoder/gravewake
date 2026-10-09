//! Frame pacing: an optional frame limit, and a lower rate while the window
//! is in the background. The schedule keeps a steady cadence without
//! spinning: the loop sleeps until the next deadline, and each deadline
//! follows the previous one, so a late wake-up is made up on the next frame.
//! A frame that runs longer than a whole period starts a fresh schedule
//! instead of drawing a burst of frames to catch up.
use std::time::Duration;
use web_time::Instant;

/// Frame limits the journal offers, in frames per second; 0 is Off.
pub const FRAME_LIMITS: [u32; 8] = [0, 30, 60, 90, 120, 144, 165, 240];
/// The most frames per second drawn while the window is visible but not
/// focused. Combat is paused then, so nothing needs a faster rate.
pub const BACKGROUND_LIMIT: u32 = 30;

/// The listed frame limit nearest `fps` (0 stays Off).
pub fn nearest_limit(fps: u32) -> u32 {
    FRAME_LIMITS
        .into_iter()
        .min_by_key(|&l| l.abs_diff(fps))
        .unwrap_or(0)
}
/// The limit in effect: the player's choice, capped in the background.
pub fn effective_limit(chosen: u32, focused: bool) -> u32 {
    match (chosen, focused) {
        (_, true) => chosen,
        (0, false) => BACKGROUND_LIMIT,
        (fps, false) => fps.min(BACKGROUND_LIMIT),
    }
}

#[derive(Default)]
pub struct Pacer {
    next: Option<Instant>,
}
impl Pacer {
    /// When the next frame may start: `None` to draw now, or the deadline
    /// to sleep until.
    pub fn wait_until(&self, now: Instant, limit: u32) -> Option<Instant> {
        match self.next {
            Some(next) if limit > 0 && now < next => Some(next),
            _ => None,
        }
    }
    /// A frame starts at `now`: schedule the one after it.
    pub fn started(&mut self, now: Instant, limit: u32) {
        if limit == 0 {
            self.next = None;
            return;
        }
        let period = Duration::from_secs_f64(1. / limit as f64);
        // Keep the cadence after a slightly late wake-up; start afresh after
        // a long frame, or when the limit changed to a shorter period.
        let base = match self.next {
            Some(previous) if now < previous + period && previous <= now + period => previous,
            _ => now,
        };
        self.next = Some(base + period);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Simulate a loop that sleeps until each deadline and wakes `late`
    /// after it, with frames taking `work`. Returns the frame start times.
    fn simulate(limit: u32, frames: usize, late: Duration, work: impl Fn(usize) -> Duration) -> Vec<Instant> {
        let mut pacer = Pacer::default();
        let mut now = Instant::now();
        let mut starts = vec![];
        for i in 0..frames {
            if let Some(deadline) = pacer.wait_until(now, limit) {
                now = deadline + late;
            }
            starts.push(now);
            pacer.started(now, limit);
            now += work(i);
        }
        starts
    }

    #[test]
    fn the_schedule_holds_the_limit_and_makes_up_late_wake_ups() {
        for limit in [30, 60, 144, 240] {
            // Waking 0.9 ms late every time still averages the limit.
            let starts = simulate(limit, 601, Duration::from_micros(900), |_| Duration::from_millis(2));
            let span = (starts[600] - starts[0]).as_secs_f64();
            let fps = 600. / span;
            assert!((fps - limit as f64).abs() / (limit as f64) < 0.01, "{limit}: {fps}");
            // No two frames start closer than a period minus the lateness.
            let period = 1. / limit as f64;
            for pair in starts.windows(2) {
                assert!((pair[1] - pair[0]).as_secs_f64() > period - 0.001);
            }
        }
    }

    #[test]
    fn a_long_frame_restarts_the_schedule_without_a_burst() {
        // Frame 10 takes 100 ms at a 60 FPS limit.
        let starts = simulate(60, 30, Duration::ZERO, |i| {
            Duration::from_millis(if i == 10 { 100 } else { 2 })
        });
        for pair in starts[11..].windows(2) {
            let gap = (pair[1] - pair[0]).as_secs_f64();
            assert!(gap > 1. / 60. - 1e-6, "burst of frames after a long one: {gap}");
        }
    }

    #[test]
    fn off_never_waits_and_the_background_caps_the_rate() {
        let starts = simulate(0, 50, Duration::ZERO, |_| Duration::from_millis(1));
        assert!((starts[49] - starts[0]).as_secs_f64() < 0.05 + 1e-6);
        assert_eq!(effective_limit(0, true), 0);
        assert_eq!(effective_limit(144, true), 144);
        assert_eq!(effective_limit(0, false), BACKGROUND_LIMIT);
        assert_eq!(effective_limit(144, false), BACKGROUND_LIMIT);
        assert_eq!(effective_limit(30, false), 30);
        // A limit switched off mid-schedule draws at once.
        let mut pacer = Pacer::default();
        let now = Instant::now();
        pacer.started(now, 30);
        assert!(pacer.wait_until(now, 30).is_some());
        assert!(pacer.wait_until(now, 0).is_none());
    }

    #[test]
    fn saved_limits_snap_to_the_listed_values() {
        assert_eq!(nearest_limit(0), 0);
        assert_eq!(nearest_limit(60), 60);
        assert_eq!(nearest_limit(70), 60);
        assert_eq!(nearest_limit(1000), 240);
        assert_eq!(nearest_limit(10), 0);
        assert_eq!(nearest_limit(20), 30);
    }
}
