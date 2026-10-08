//! `--audio-review`: the audio output through real ALSA and a real sound
//! server that fails, without speakers.
//!
//! Run by `scripts/audio-review.sh` with `HOME` set to a throwaway folder
//! under `captures/` and `PIPEWIRE_RUNTIME_DIR` inside it, so nothing can
//! reach the machine's own sound server. The review starts a private PipeWire
//! and WirePlumber there (no D-Bus, no hardware, one null sink that keeps
//! real time) and points ALSA's default device at it, so the game plays
//! through cpal, ALSA and PipeWire's ALSA plugin as it would on a desktop:
//!
//! 1. *plays*: `pw-record` captures the null sink while the game plays;
//!    then a million copies of round 9's `POLLERR` error go through the
//!    stream's error handler (simulated: nothing here makes ALSA report it),
//!    and the stream must close and reopen;
//! 2. *fails*: the review stops the server, so the stream fails the way it
//!    does when a sound server goes away;
//! 3. *missing*: with no server, every reopen fails; the game keeps running;
//! 4. *returns*: the review starts the server again; the game's own retry
//!    finds it, and the capture shows the ambience and music sinks playing
//!    again without being restarted, then new sounds.
//!
//! `scripts/audio-review.sh` counts the lines the errors printed.

use crate::audio::Audio;
use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Mutex,
    time::{Duration, Instant},
};

/// Why the review won't run here, if it won't.
pub fn refusal() -> Option<String> {
    if !cfg!(target_os = "linux") {
        return Some("the audio review uses ALSA, so it runs only on Linux".into());
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let runtime = std::env::var_os("PIPEWIRE_RUNTIME_DIR").map(PathBuf::from);
    match (home, runtime) {
        (Some(home), Some(runtime))
            if home.is_absolute()
                && home.components().any(|c| c.as_os_str() == "captures")
                && runtime.starts_with(&home) =>
        {
            None
        }
        _ => Some(
            "the audio review writes ~/.asoundrc and runs its own sound server, so it needs \
             HOME set to a throwaway folder under captures/ and PIPEWIRE_RUNTIME_DIR inside \
             it: use scripts/audio-review.sh"
                .into(),
        ),
    }
}

/// The stock PipeWire configuration plus one null sink, and nothing that
/// looks for X11 or JACK.
const PIPEWIRE: &str = r#"context.properties = {
    module.x11.bell = false
    module.jackdbus-detect = false
}
context.objects = [
    { factory = adapter
      args = {
        factory.name = support.null-audio-sink
        node.name = review-sink
        node.description = "Audio review sink"
        media.class = Audio/Sink
        audio.position = [ FL FR ]
        audio.rate = 48000
      }
    }
]
"#;
/// WirePlumber's own profile without hardware, D-Bus or saved state: it only
/// links streams to the null sink.
const WIREPLUMBER: &str = r#"wireplumber.profiles = {
  review = {
    inherits = [ main, mixin.systemwide-session, mixin.stateless ]
    hardware.audio = disabled
    hardware.bluetooth = disabled
    hardware.video-capture = disabled
    support.dbus = disabled
  }
}
"#;
const ASOUNDRC: &str = "pcm.!default {\n    type pipewire\n}\n";

/// The private sound server's processes and the recorder, stopped on every
/// way out.
static CHILDREN: Mutex<Vec<(&'static str, Child)>> = Mutex::new(Vec::new());

fn stop(names: &[&str]) {
    let mut children = CHILDREN.lock().unwrap();
    for (name, child) in children.iter_mut() {
        if names.contains(name) {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
    children.retain(|(name, _)| !names.contains(name));
}

fn spawn(home: &Path, name: &'static str, program: &str, args: &[&str], log: &str) {
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(home.join(log))
        .expect("child log");
    let child = Command::new(program)
        .args(args)
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_STATE_HOME", home.join("state"))
        .stdin(Stdio::null())
        .stdout(log.try_clone().expect("child log"))
        .stderr(log)
        .spawn()
        .unwrap_or_else(|e| fail(format!("couldn't start {program}: {e}")));
    CHILDREN.lock().unwrap().push((name, child));
}

fn start_servers(home: &Path) {
    let socket =
        PathBuf::from(std::env::var_os("PIPEWIRE_RUNTIME_DIR").unwrap()).join("pipewire-0");
    let _ = std::fs::remove_file(&socket);
    spawn(home, "pipewire", "pipewire", &[], "pipewire.log");
    if !wait(5., || socket.exists()) {
        fail("the private PipeWire didn't open its socket".into());
    }
    spawn(
        home,
        "wireplumber",
        "wireplumber",
        &["--profile", "review"],
        "wireplumber.log",
    );
}

/// Capture the null sink to `file` until `stop(&["recorder"])`.
fn record(home: &Path, file: &str) {
    let path = home.join(file);
    spawn(
        home,
        "recorder",
        "pw-record",
        &[
            "-P",
            "{ stream.capture.sink = true }",
            "--target",
            "review-sink",
            "--format",
            "f32",
            "--rate",
            "48000",
            "--channels",
            "2",
            path.to_str().unwrap(),
        ],
        "recorder.log",
    );
}

/// The private graph's links, as `pw-link` lists them.
fn links() -> String {
    Command::new("timeout")
        .args(["-s", "KILL", "5", "pw-link", "--links"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

fn wait(seconds: f32, mut until: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs_f32(seconds);
    while Instant::now() < end {
        if until() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    until()
}

/// Seconds in a recording, and the peak and RMS of its stereo float samples
/// between `from` and `to` seconds into it. The header's sizes may be
/// unfinished, so the samples run from the data chunk to the end of the file.
fn levels(path: &Path, from: f32, to: f32) -> (f32, f32, f32) {
    let bytes = std::fs::read(path).unwrap_or_default();
    let data = bytes
        .windows(4)
        .position(|w| w == b"data")
        .map_or(bytes.len(), |at| at + 8)
        .min(bytes.len());
    let samples: Vec<f32> = bytes[data..]
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect();
    let seconds = samples.len() as f32 / 96_000.;
    let at = |s: f32| ((s * 96_000.) as usize / 2 * 2).min(samples.len());
    let window = &samples[at(from)..at(to)];
    let peak = window.iter().fold(0f32, |m, s| m.max(s.abs()));
    let rms = (window.iter().map(|s| s * s).sum::<f32>() / window.len().max(1) as f32).sqrt();
    (seconds, peak, rms)
}

fn fail(message: String) -> ! {
    stop(&["recorder", "wireplumber", "pipewire"]);
    eprintln!("AUDIO REVIEW FAIL: {message}");
    std::process::exit(1);
}

/// Play a gunshot every quarter second for `seconds`.
fn shots(audio: &Audio, seconds: f32) {
    let end = Instant::now() + Duration::from_secs_f32(seconds);
    while Instant::now() < end {
        audio.play("shot", 0.8);
        std::thread::sleep(Duration::from_millis(250));
    }
}

pub fn run() {
    let report = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Ok(mut children) = CHILDREN.try_lock() {
            for (_, child) in children.iter_mut() {
                let _ = child.kill();
            }
        }
        report(info);
    }));
    let home = PathBuf::from(std::env::var_os("HOME").expect("HOME"));
    let put = |path: &str, text: &str| {
        let path = home.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).expect("create config folder");
        std::fs::write(path, text).expect("write config");
    };
    put("config/pipewire/pipewire.conf.d/review.conf", PIPEWIRE);
    put(
        "config/wireplumber/wireplumber.conf.d/review.conf",
        WIREPLUMBER,
    );
    put(".asoundrc", ASOUNDRC);
    std::fs::create_dir_all(std::env::var_os("PIPEWIRE_RUNTIME_DIR").unwrap())
        .expect("create the private PipeWire folder");
    start_servers(&home);

    // 1. Plays.
    let audio = Audio::new();
    let output = audio.output();
    if !wait(15., || output.live()) {
        fail("no stream opened on the private PipeWire within 15 s".into());
    }
    println!(
        "AUDIO REVIEW: plays: on {}",
        output.playing().unwrap_or_default()
    );
    audio.volume(1.);
    audio.music([1., 0., 0.], 1.);
    record(&home, "plays.wav");
    std::thread::sleep(Duration::from_millis(500));
    shots(&audio, 3.);
    let linked = links();
    stop(&["recorder"]);
    let to_sink = linked
        .lines()
        .filter(|l| l.contains("review-sink:playback"))
        .count();
    let (seconds, peak, rms) = levels(&home.join("plays.wav"), 0.5, 3.5);
    println!(
        "AUDIO REVIEW: plays: {to_sink} links into review-sink; {seconds:.2} s recorded, \
         peak {peak:.3}, RMS {rms:.4} over 0.5-3.5 s"
    );
    if to_sink == 0 {
        fail(format!(
            "the game's stream isn't linked to the private sink:\n{linked}"
        ));
    }
    if seconds < 3. || peak < 0.1 {
        fail(format!(
            "expected at least 3 s with shots in plays.wav, got {seconds:.2} s, peak {peak:.3}"
        ));
    }

    // A simulated flood through the stream's own error handler.
    let (errors_before, opened_before) = output.counts();
    let flood = Instant::now();
    output.simulate_errors(
        1_000_000,
        "A backend-specific error has occurred: `alsa::poll()` returned POLLERR",
    );
    let took = flood.elapsed().as_secs_f32();
    if !wait(10., || output.live() && output.counts().1 > opened_before) {
        fail("the stream didn't reopen within 10 s of the error flood".into());
    }
    println!(
        "AUDIO REVIEW: flood: {} simulated errors in {took:.2} s; reopened {:.1} s after it began",
        output.counts().0 - errors_before,
        flood.elapsed().as_secs_f32()
    );
    // Back to one error per line for what follows.
    std::thread::sleep(crate::output::REPORT_EVERY);

    // 2. Fails: stop the sound server under the open stream.
    let (errors_before, opened_before) = output.counts();
    stop(&["wireplumber", "pipewire"]);
    let stopped = Instant::now();
    if !wait(10., || !output.live()) {
        fail("the stream stayed open for 10 s after the sound server stopped".into());
    }
    let closed = stopped.elapsed().as_secs_f32();

    // 3. Missing: retries at 1, 2 and 4 seconds all fail; keep playing.
    shots(&audio, 6.);
    let (errors, opened) = output.counts();
    println!(
        "AUDIO REVIEW: fails: stream closed {closed:.2} s after the server stopped, \
         {} stream errors reported",
        errors - errors_before
    );
    if output.live() || opened != opened_before {
        fail("a stream opened with no sound server".into());
    }
    println!("AUDIO REVIEW: missing: no stream for 6 s, sounds dropped, still running");

    // 4. Returns: the server starts again.
    start_servers(&home);
    let back = Instant::now();
    if !wait(20., || output.live()) {
        fail("no stream reopened within 20 s of the server returning".into());
    }
    println!(
        "AUDIO REVIEW: returns: reopened {:.1} s after the server started, on {}",
        back.elapsed().as_secs_f32(),
        output.playing().unwrap_or_default()
    );
    // Ambience and music alone, then shots.
    record(&home, "returns.wav");
    std::thread::sleep(Duration::from_secs(3));
    shots(&audio, 2.);
    std::thread::sleep(Duration::from_millis(300));
    stop(&["recorder"]);
    let path = home.join("returns.wav");
    let (seconds, peak, ambient) = levels(&path, 0.5, 2.5);
    let (_, shot_peak, _) = levels(&path, 3.1, 5.);
    println!(
        "AUDIO REVIEW: returns: {seconds:.2} s recorded; before any new sound peak {peak:.3}, \
         RMS {ambient:.4}; with shots peak {shot_peak:.3}"
    );
    if ambient < 0.002 {
        fail(format!(
            "ambience and music didn't resume (RMS {ambient:.4})"
        ));
    }
    if shot_peak < 0.1 {
        fail(format!(
            "new sounds didn't play after the reopen (peak {shot_peak:.3})"
        ));
    }
    let (errors, opened) = output.counts();
    println!("AUDIO REVIEW: {errors} stream errors in all, {opened} streams opened");
    stop(&["wireplumber", "pipewire"]);
    println!("AUDIO REVIEW PASS");
}
