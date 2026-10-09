//! The audio output device.
//!
//! rodio 0.20's `OutputStream` prints every stream error with `eprintln!` and
//! never recovers, so a failing device could print one line millions of times
//! and leave the game silent until a restart. Here the game owns the cpal
//! stream. One mixer lives for the whole session; a thread of its own opens
//! the default device and feeds it from that mixer, reports errors sparingly,
//! and reopens the device after a failure (an error, or a stream that stops
//! asking for sound), when none could be opened, or when another device
//! becomes the default. Opening a device can block (a sound server that is
//! starting or stuck), so none of this runs on the main thread.

use rodio::cpal::{
    self,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use rodio::dynamic_mixer::{self, DynamicMixer, DynamicMixerController};
use rodio::{Sink, Source};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};
use web_time::Instant;

/// The mixer's format: everything the game plays is stereo at this rate.
const RATE: u32 = crate::audio::RATE;
/// After errors are first reported, at most one summary line this often.
pub const REPORT_EVERY: Duration = Duration::from_secs(10);
/// A second error this soon after the last means the stream is failing.
const FAILING_WITHIN: Duration = Duration::from_secs(1);
/// How often the default device is checked for a change.
const DEVICE_CHECK: Duration = Duration::from_secs(2);
/// A stream that hasn't asked for sound this long has stalled. A sound server
/// that goes away can leave the stream waiting forever without an error.
const STALLED: Duration = Duration::from_secs(2);

/// Wait before reopening after `failures` failed or lost streams in a row:
/// 1, 2, 4 and 8 seconds, then 10.
pub fn retry_delay(failures: u32) -> Duration {
    Duration::from_secs(1u64 << failures.saturating_sub(1).min(4)).min(Duration::from_secs(10))
}

/// Whether the system's default device is no longer the one playing. An
/// unknown current default (none, or no name) isn't a change.
pub fn default_moved(playing: &str, current: Option<&str>) -> bool {
    current.is_some_and(|name| name != playing)
}

/// Decides which errors are printed and when a stream counts as failed.
#[derive(Default)]
pub struct Reporter {
    last_print: Option<Instant>,
    last_error: Option<Instant>,
    /// Errors since the last line printed.
    unreported: u64,
}
impl Reporter {
    /// Note an error. Returns the line to print, if any, and whether the
    /// stream should be closed and reopened: a device that has gone away,
    /// or errors arriving faster than one a second.
    pub fn note(&mut self, now: Instant, error: &str, lost: bool) -> (Option<String>, bool) {
        let failing = lost
            || self
                .last_error
                .is_some_and(|last| now.duration_since(last) < FAILING_WITHIN);
        self.last_error = Some(now);
        let due = self
            .last_print
            .is_none_or(|last| now.duration_since(last) >= REPORT_EVERY);
        if !due {
            self.unreported += 1;
            return (None, failing);
        }
        let line = if self.unreported == 0 {
            format!("Audio output error: {error}")
        } else {
            format!(
                "Audio output error: {error} ({} more since the last report)",
                self.unreported
            )
        };
        self.last_print = Some(now);
        self.unreported = 0;
        (Some(line), failing)
    }
    /// A summary of errors not yet printed, for when a stream closes.
    pub fn flush(&mut self) -> Option<String> {
        (self.unreported > 0).then(|| {
            let line = format!(
                "Audio output: {} more errors before the stream closed",
                self.unreported
            );
            self.unreported = 0;
            line
        })
    }
}

#[derive(Default)]
struct Shared {
    /// A stream is open and playing the mixer.
    live: AtomicBool,
    /// The open stream is failing: close it and reopen after a delay.
    failing: AtomicBool,
    stop: AtomicBool,
    /// Every stream error since launch, printed or not.
    errors: AtomicU64,
    /// Streams opened since launch.
    opened: AtomicU64,
    /// Buffers the device has asked for, to notice a stalled stream.
    fills: AtomicU64,
    reporter: Mutex<Reporter>,
    /// The device and format playing, for diagnostics.
    playing: Mutex<Option<String>>,
}
impl Shared {
    fn error(&self, error: &str, lost: bool) {
        self.errors.fetch_add(1, Ordering::Relaxed);
        let Ok(mut reporter) = self.reporter.lock() else {
            return;
        };
        let (line, failing) = reporter.note(Instant::now(), error, lost);
        drop(reporter);
        if let Some(line) = line {
            eprintln!("{line}");
        }
        if failing {
            self.failing.store(true, Ordering::Relaxed);
        }
    }
}

/// The session's audio output. Dropping it stops the output thread.
pub struct Output {
    mixer: Arc<DynamicMixerController<f32>>,
    shared: Arc<Shared>,
    /// In the browser: the mixer, and the Web Audio stream `resume` opens.
    #[cfg(target_arch = "wasm32")]
    web: std::cell::RefCell<(Arc<Mutex<DynamicMixer<f32>>>, Option<cpal::Stream>, bool)>,
}
impl Output {
    /// In the browser there are no threads, and a page may only start
    /// sound after the player clicks or presses a key, so the stream opens
    /// on the first input (`resume`).
    #[cfg(target_arch = "wasm32")]
    pub fn start() -> Self {
        let (mixer, source) = dynamic_mixer::mixer::<f32>(2, RATE);
        Self {
            mixer,
            shared: Arc::<Shared>::default(),
            web: std::cell::RefCell::new((Arc::new(Mutex::new(source)), None, false)),
        }
    }
    /// Open the Web Audio stream once, after the player's first input.
    #[cfg(target_arch = "wasm32")]
    pub fn resume(&self) {
        let mut web = self.web.borrow_mut();
        let (source, stream, tried) = &mut *web;
        if *tried {
            return;
        }
        *tried = true;
        match open(&cpal::default_host(), source, &self.shared) {
            Ok((opened, _, description)) => {
                println!("Audio output: {description}");
                self.shared.opened.fetch_add(1, Ordering::Relaxed);
                self.shared.live.store(true, Ordering::Relaxed);
                *stream = Some(opened);
            }
            Err(error) => {
                web_sys::console::warn_1(&format!("Audio output unavailable: {error}").into());
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn resume(&self) {}
    #[cfg(not(target_arch = "wasm32"))]
    pub fn start() -> Self {
        let (mixer, source) = dynamic_mixer::mixer::<f32>(2, RATE);
        let shared = Arc::<Shared>::default();
        let source = Arc::new(Mutex::new(source));
        let worker = shared.clone();
        let started = std::thread::Builder::new()
            .name("audio-output".into())
            .spawn(move || run(source, worker));
        if let Err(error) = started {
            eprintln!("Audio output disabled: {error}");
        }
        Self { mixer, shared }
    }
    /// Whether a device is open.
    pub fn live(&self) -> bool {
        self.shared.live.load(Ordering::Relaxed)
    }
    /// Play a one-off sound. Without a device it's dropped, so nothing piles
    /// up to play all at once when one appears.
    pub fn play<S>(&self, source: S)
    where
        S: Source<Item = f32> + Send + 'static,
    {
        if self.live() {
            self.mixer.add(source);
        }
    }
    /// A sink that plays for the whole session, through every device change.
    pub fn sink(&self) -> Sink {
        let (sink, output) = Sink::new_idle();
        self.mixer.add(output);
        sink
    }
    /// Stream errors since launch, and streams opened.
    pub fn counts(&self) -> (u64, u64) {
        (
            self.shared.errors.load(Ordering::Relaxed),
            self.shared.opened.load(Ordering::Relaxed),
        )
    }
    /// Report `count` copies of `error` through the path a stream's own
    /// errors take, for the audio review: a stream on a failing ALSA device
    /// can report the same error millions of times a minute.
    pub fn simulate_errors(&self, count: u64, error: &str) {
        for _ in 0..count {
            self.shared.error(error, false);
        }
    }
    /// The device and format playing, if any.
    pub fn playing(&self) -> Option<String> {
        self.shared.playing.lock().ok().and_then(|p| p.clone())
    }
}
impl Drop for Output {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
    }
}

/// Sleep for `duration`, or until the output is dropped. Returns false to
/// stop.
#[cfg(not(target_arch = "wasm32"))]
fn pause(shared: &Shared, duration: Duration) -> bool {
    let until = Instant::now() + duration;
    while Instant::now() < until {
        if shared.stop.load(Ordering::Relaxed) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    !shared.stop.load(Ordering::Relaxed)
}

#[cfg(not(target_arch = "wasm32"))]
fn run(source: Arc<Mutex<DynamicMixer<f32>>>, shared: Arc<Shared>) {
    let host = cpal::default_host();
    let mut failures = 0;
    let mut open_error: Option<String> = None;
    loop {
        if shared.stop.load(Ordering::Relaxed) {
            return;
        }
        shared.failing.store(false, Ordering::Relaxed);
        match open(&host, &source, &shared) {
            Ok((stream, device, description)) => {
                let first = shared.opened.fetch_add(1, Ordering::Relaxed) == 0;
                if first {
                    println!("Audio output: {description}");
                } else {
                    println!("Audio output reopened: {description}");
                }
                if let Ok(mut playing) = shared.playing.lock() {
                    *playing = Some(description);
                }
                open_error = None;
                shared.live.store(true, Ordering::Relaxed);
                let opened = Instant::now();
                let mut checked = Instant::now();
                let mut fills = shared.fills.load(Ordering::Relaxed);
                let mut fed = Instant::now();
                let failed = loop {
                    std::thread::sleep(Duration::from_millis(50));
                    if shared.stop.load(Ordering::Relaxed) {
                        return;
                    }
                    let now = shared.fills.load(Ordering::Relaxed);
                    if now != fills {
                        fills = now;
                        fed = Instant::now();
                    } else if fed.elapsed() >= STALLED {
                        shared.error("the device stopped asking for sound", true);
                    }
                    if shared.failing.load(Ordering::Relaxed) {
                        break true;
                    }
                    if checked.elapsed() >= DEVICE_CHECK {
                        checked = Instant::now();
                        let current = host.default_output_device().and_then(|d| d.name().ok());
                        if default_moved(&device, current.as_deref()) {
                            println!("Audio output: the default device changed");
                            break false;
                        }
                    }
                };
                shared.live.store(false, Ordering::Relaxed);
                drop(stream);
                if let Some(line) = shared.reporter.lock().ok().and_then(|mut r| r.flush()) {
                    eprintln!("{line}");
                }
                if let Ok(mut playing) = shared.playing.lock() {
                    *playing = None;
                }
                // A stream that played a while before failing starts the
                // retry schedule over.
                if opened.elapsed() > Duration::from_secs(30) {
                    failures = 0;
                }
                if failed {
                    failures += 1;
                    if !pause(&shared, retry_delay(failures)) {
                        return;
                    }
                }
            }
            Err(error) => {
                // Say why once, not on every retry.
                if open_error.as_deref() != Some(error.as_str()) {
                    eprintln!("Audio output unavailable: {error}; retrying");
                    open_error = Some(error);
                }
                failures += 1;
                if !pause(&shared, retry_delay(failures)) {
                    return;
                }
            }
        }
    }
}

/// Open the default device, preferring the mixer's own format.
fn open(
    host: &cpal::Host,
    source: &Arc<Mutex<DynamicMixer<f32>>>,
    shared: &Arc<Shared>,
) -> Result<(cpal::Stream, String, String), String> {
    let device = host.default_output_device().ok_or("no output device")?;
    let name = device.name().unwrap_or_else(|_| "unnamed device".into());
    let config = choose(&device)?;
    let format = config.sample_format();
    let config: cpal::StreamConfig = config.into();
    let feed = Feed::new(
        source.clone(),
        shared.clone(),
        config.channels as usize,
        config.sample_rate.0,
    );
    let description = format!(
        "{name}, {} Hz, {} channels, {format:?}",
        config.sample_rate.0, config.channels
    );
    let errors = shared.clone();
    let on_error = move |error: cpal::StreamError| {
        let lost = matches!(error, cpal::StreamError::DeviceNotAvailable);
        errors.error(&error.to_string(), lost);
    };
    use cpal::SampleFormat as F;
    let stream = match format {
        F::F32 => build::<f32>(&device, &config, feed, on_error),
        F::I16 => build::<i16>(&device, &config, feed, on_error),
        F::U16 => build::<u16>(&device, &config, feed, on_error),
        F::I32 => build::<i32>(&device, &config, feed, on_error),
        F::U32 => build::<u32>(&device, &config, feed, on_error),
        F::I8 => build::<i8>(&device, &config, feed, on_error),
        F::U8 => build::<u8>(&device, &config, feed, on_error),
        F::F64 => build::<f64>(&device, &config, feed, on_error),
        other => return Err(format!("{name}: unsupported sample format {other:?}")),
    }
    .map_err(|e| format!("{name}: {e}"))?;
    stream.play().map_err(|e| format!("{name}: {e}"))?;
    Ok((stream, name, description))
}

/// Stereo at the mixer's rate if the device offers it (as floats, then 16-bit),
/// otherwise the device's default.
fn choose(device: &cpal::Device) -> Result<cpal::SupportedStreamConfig, String> {
    let rank = |format: cpal::SampleFormat| match format {
        cpal::SampleFormat::F32 => 0,
        cpal::SampleFormat::I16 => 1,
        _ => 2,
    };
    if let Ok(configs) = device.supported_output_configs() {
        let best = configs
            .filter(|c| {
                c.channels() == 2 && c.min_sample_rate().0 <= RATE && RATE <= c.max_sample_rate().0
            })
            .min_by_key(|c| rank(c.sample_format()));
        if let Some(config) = best {
            return Ok(config.with_sample_rate(cpal::SampleRate(RATE)));
        }
    }
    device.default_output_config().map_err(|e| e.to_string())
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut feed: Feed,
    on_error: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    device.build_output_stream::<T, _, _>(
        config,
        move |data: &mut [T], _| feed.fill(data),
        on_error,
        None,
    )
}

/// Pulls stereo frames from the session's mixer and fits them to the device:
/// its channel count, and its rate by linear interpolation.
struct Feed {
    mixer: Arc<Mutex<DynamicMixer<f32>>>,
    shared: Arc<Shared>,
    channels: usize,
    /// Mixer frames per device frame.
    step: f64,
    phase: f64,
    previous: [f32; 2],
    next: [f32; 2],
}
impl Feed {
    fn new(
        mixer: Arc<Mutex<DynamicMixer<f32>>>,
        shared: Arc<Shared>,
        channels: usize,
        rate: u32,
    ) -> Self {
        Self {
            mixer,
            shared,
            channels: channels.max(1),
            step: RATE as f64 / rate.max(1) as f64,
            phase: 0.,
            previous: [0.; 2],
            next: [0.; 2],
        }
    }
    fn fill<T: cpal::Sample + cpal::FromSample<f32>>(&mut self, data: &mut [T]) {
        self.shared.fills.fetch_add(1, Ordering::Relaxed);
        let Ok(mut mixer) = self.mixer.lock() else {
            data.fill(T::EQUILIBRIUM);
            return;
        };
        let pull = |m: &mut DynamicMixer<f32>| [m.next().unwrap_or(0.), m.next().unwrap_or(0.)];
        for frame in data.chunks_mut(self.channels) {
            let [left, right] = if self.step == 1. {
                pull(&mut mixer)
            } else {
                while self.phase >= 1. {
                    self.phase -= 1.;
                    self.previous = self.next;
                    self.next = pull(&mut mixer);
                }
                let t = self.phase as f32;
                let mix = |c: usize| self.previous[c] + (self.next[c] - self.previous[c]) * t;
                self.phase += self.step;
                [mix(0), mix(1)]
            };
            match frame {
                [mono] => *mono = T::from_sample((left + right) * 0.5),
                [l, r, rest @ ..] => {
                    *l = T::from_sample(left);
                    *r = T::from_sample(right);
                    rest.fill(T::EQUILIBRIUM);
                }
                [] => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flood_of_errors_prints_a_line_then_a_summary_every_ten_seconds() {
        let start = Instant::now();
        let mut reporter = Reporter::default();
        let mut lines = vec![];
        let mut failing = 0;
        // A million errors over 25 seconds.
        for n in 0..1_000_000u64 {
            let now = start + Duration::from_micros(n * 25);
            let (line, fail) = reporter.note(now, "POLLERR", false);
            lines.extend(line);
            failing += fail as u32;
        }
        assert_eq!(lines.len(), 3, "{lines:?}");
        assert_eq!(lines[0], "Audio output error: POLLERR");
        assert!(
            lines[1].contains("more since the last report"),
            "{}",
            lines[1]
        );
        assert_eq!(
            failing, 999_999,
            "every error after the first is a failing stream"
        );
        assert!(
            reporter
                .flush()
                .unwrap()
                .contains("more errors before the stream closed")
        );
        assert_eq!(reporter.flush(), None);
    }

    #[test]
    fn an_isolated_error_is_reported_without_reopening_but_a_lost_device_reopens() {
        let start = Instant::now();
        let mut reporter = Reporter::default();
        assert_eq!(reporter.note(start, "one", false).1, false);
        let (line, failing) = reporter.note(start + Duration::from_secs(20), "two", false);
        assert!(line.is_some() && !failing, "errors 20 s apart are isolated");
        let (line, failing) = reporter.note(start + Duration::from_secs(21), "gone", true);
        assert!(line.is_none(), "inside the ten-second quiet period");
        assert!(failing);
    }

    #[test]
    fn retries_back_off_to_ten_seconds() {
        let seconds: Vec<u64> = (1..=7).map(|n| retry_delay(n).as_secs()).collect();
        assert_eq!(seconds, [1, 2, 4, 8, 10, 10, 10]);
        assert_eq!(retry_delay(0).as_secs(), 1);
        assert_eq!(retry_delay(u32::MAX).as_secs(), 10);
    }

    #[test]
    fn only_a_named_new_default_moves_the_output() {
        assert!(!default_moved("default", Some("default")));
        assert!(default_moved("MacBook Speakers", Some("AirPods")));
        assert!(!default_moved("MacBook Speakers", None));
    }

    fn feed_with(samples: Vec<f32>, channels: usize, rate: u32) -> Feed {
        let (controller, mixer) = dynamic_mixer::mixer::<f32>(2, RATE);
        controller.add(rodio::buffer::SamplesBuffer::new(2, RATE, samples));
        Feed::new(Arc::new(Mutex::new(mixer)), Arc::default(), channels, rate)
    }

    #[test]
    fn the_feed_passes_stereo_through_and_fits_other_layouts() {
        let ramp: Vec<f32> = (0..64).map(|i| i as f32 / 100.).collect();
        let mut out = vec![0f32; 16];
        feed_with(ramp.clone(), 2, RATE).fill(&mut out);
        assert_eq!(out, ramp[..16]);
        // Mono averages the pair; extra channels are silent.
        let mut mono = vec![0f32; 4];
        feed_with(ramp.clone(), 1, RATE).fill(&mut mono);
        assert!((mono[1] - 0.025).abs() < 1e-6, "{mono:?}");
        let mut six = vec![1f32; 12];
        feed_with(ramp.clone(), 6, RATE).fill(&mut six);
        assert_eq!(&six[..6], &[0., 0.01, 0., 0., 0., 0.]);
        assert_eq!(&six[6..8], &[0.02, 0.03]);
        // 16-bit output converts the floats.
        let mut ints = vec![0i16; 4];
        feed_with(vec![0.5; 8], 2, RATE).fill(&mut ints);
        assert!(ints.iter().all(|&s| (s - 16384).abs() <= 1), "{ints:?}");
        // An empty mixer plays silence.
        let mut quiet = vec![1f32; 8];
        feed_with(vec![], 2, RATE).fill(&mut quiet);
        assert!(quiet.iter().all(|&s| s == 0.));
    }

    #[test]
    fn the_feed_resamples_to_the_device_rate() {
        // A slow ramp on the left, constant on the right, played at 96 kHz:
        // twice the frames, with midpoints between the mixer's (after the first
        // few, which ease in from silence).
        let frames: Vec<f32> = (0..200).flat_map(|i| [i as f32 / 1000., 0.25]).collect();
        let mut out = vec![0f32; 200];
        feed_with(frames.clone(), 2, 96_000).fill(&mut out);
        let left: Vec<f32> = out.chunks(2).map(|f| f[0]).collect();
        let steps: Vec<f32> = left.windows(2).skip(4).map(|w| w[1] - w[0]).collect();
        assert!(steps.iter().all(|s| (s - 0.0005).abs() < 1e-5), "{steps:?}");
        assert!(out.chunks(2).skip(4).all(|f| (f[1] - 0.25).abs() < 1e-6));
        // And at 44.1 kHz, fewer frames covering the same ramp.
        let mut out = vec![0f32; 100];
        feed_with(frames, 2, 44_100).fill(&mut out);
        let last = out[out.len() - 2];
        assert!(
            (last - 49. * RATE as f32 / 44_100. / 1000.).abs() < 0.002,
            "{last}"
        );
    }
}
