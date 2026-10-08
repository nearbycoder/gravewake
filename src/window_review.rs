//! `--window-review`: a normal launch's window size, checked inside
//! `scripts/window-review.sh`'s private KWin. The game loads and saves its
//! preferences as a player's launch would (in a throwaway data folder), then
//! checks the window opened at the saved size, or 1440×900, fitted to 90% of
//! the output (`game::fit_window`). Optional stages, from the environment:
//!
//! - `GRAVEWAKE_WINDOW_EXPECT=WxH`: the size the window must open at.
//! - `GRAVEWAKE_WINDOW_PROBE=WxH`: ask for that size without fitting and
//!   report what the compositor gives, to show whether it would have kept
//!   an oversized window.
//! - `GRAVEWAKE_WINDOW_RESIZE=WxH`: wait for the compositor to resize the
//!   window, as a player dragging its edge would (the script asks KWin to,
//!   through KWin scripting, once the review prints that it's waiting), and
//!   check the preference follows, before quitting through the normal save.
//!
//! It refuses to run outside the private compositor or without a throwaway
//! `XDG_DATA_HOME`.
use crate::App;
use crate::game::{SCREEN_SHARE, fit_window};
use winit::dpi::LogicalSize;

/// Frames the compositor gets to settle or apply a resize.
const WAIT: u32 = 600;
/// Frames the size must hold before it counts.
const STEADY: u32 = 10;

/// Why the review may not run here, if it may not.
pub fn refusal() -> Option<String> {
    if !cfg!(target_os = "linux") {
        return Some("the window review runs only on Linux, in a private KWin".into());
    }
    if std::env::var("GRAVEWAKE_NESTED_KWIN").as_deref() != Ok("1") {
        return Some(
            "the window review opens and resizes a game window, so it runs only inside the \
             private compositor: use scripts/window-review.sh"
                .into(),
        );
    }
    let data = std::env::var_os("XDG_DATA_HOME").map(std::path::PathBuf::from);
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    match (data, home) {
        (Some(data), Some(home)) if data.is_absolute() && data != home.join(".local/share") => {
            None
        }
        _ => Some(
            "the window review saves settings, so it needs XDG_DATA_HOME set to a throwaway \
             folder"
                .into(),
        ),
    }
}

fn size_from_env(name: &str) -> Option<[f32; 2]> {
    let value = std::env::var(name).ok()?;
    let (w, h) = value.split_once('x')?;
    Some([w.parse().ok()?, h.parse().ok()?])
}

/// What the launch knew when it chose the window's size.
#[derive(Clone, Copy, Debug)]
pub struct Launch {
    pub saved: Option<[f32; 2]>,
    pub screen: Option<[f32; 2]>,
    pub size: [f32; 2],
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Stage {
    Opened,
    Probe,
    Resize,
    Done,
}

pub struct Review {
    pub launch: Option<Launch>,
    stage: Stage,
    frames: u32,
    /// The size last seen, and since which frame.
    steady: ([f32; 2], u32),
    expect: Option<[f32; 2]>,
    probe: Option<[f32; 2]>,
    resize: Option<[f32; 2]>,
    pub finished: bool,
}
impl Review {
    pub fn new() -> Self {
        Self {
            launch: None,
            stage: Stage::Opened,
            frames: 0,
            steady: ([0., 0.], 0),
            expect: size_from_env("GRAVEWAKE_WINDOW_EXPECT"),
            probe: size_from_env("GRAVEWAKE_WINDOW_PROBE"),
            resize: size_from_env("GRAVEWAKE_WINDOW_RESIZE"),
            finished: false,
        }
    }
}

fn near(a: [f32; 2], b: [f32; 2]) -> bool {
    (a[0] - b[0]).abs() <= 1. && (a[1] - b[1]).abs() <= 1.
}
fn show(s: [f32; 2]) -> String {
    format!("{}x{}", s[0].round(), s[1].round())
}

impl App {
    /// One frame of the review; true once it's time to quit.
    pub(crate) fn window_step(&mut self) -> bool {
        let Some(mut r) = self.window_review.take() else {
            return false;
        };
        self.window_frame(&mut r);
        let finished = r.finished;
        self.window_review = Some(r);
        finished
    }
    fn window_frame(&mut self, r: &mut Review) {
        r.frames += 1;
        self.game.prefs.volume = 0.;
        let Some(window) = self.window.clone() else {
            return;
        };
        let scale = window.scale_factor();
        let inner = window.inner_size().to_logical::<f32>(scale);
        let size = [inner.width, inner.height];
        if !near(size, r.steady.0) {
            r.steady = (size, r.frames);
        }
        let steady = r.frames - r.steady.1 >= STEADY;
        let screen = window.current_monitor().map(|m| {
            let s = m.size().to_logical::<f32>(m.scale_factor());
            [s.width, s.height]
        });
        assert!(
            r.frames < WAIT,
            "WINDOW REVIEW: stage {:?} didn't settle in {WAIT} frames: the window is {} on a {} output",
            r.stage,
            show(size),
            screen.map_or("unknown".into(), show),
        );
        let launch = r.launch.expect("the launch recorded its size");
        match r.stage {
            Stage::Opened => {
                // Wait for the output and for the shrink the launch may make
                // once it knows which output it's on.
                let Some(screen) = screen else { return };
                if r.frames < 60 || self.fit_frames > 0 || !steady {
                    return;
                }
                let fitted = fit_window(launch.saved, Some(screen));
                println!(
                    "WINDOW REVIEW: launched with {} saved, guessed a {} screen and asked for {}; \
                     opened at {} on a {} output (90% of it is {})",
                    launch.saved.map_or("no size".into(), show),
                    launch.screen.map_or("unknown".into(), show),
                    show(launch.size),
                    show(size),
                    show(screen),
                    show(screen.map(|v| v * SCREEN_SHARE)),
                );
                assert!(
                    near(size, fitted),
                    "WINDOW REVIEW: the window is {}, expected {} (the saved size or 1440x900, \
                     fitted to the output)",
                    show(size),
                    show(fitted),
                );
                if let Some(expect) = r.expect {
                    assert!(
                        near(size, expect),
                        "WINDOW REVIEW: the window is {}, the script expected {}",
                        show(size),
                        show(expect)
                    );
                }
                assert!(
                    size[0] <= screen[0] && size[1] <= screen[1],
                    "WINDOW REVIEW: the window is bigger than the output"
                );
                r.stage = Stage::Probe;
                r.frames = 0;
            }
            Stage::Probe => {
                let Some(probe) = r.probe else {
                    r.stage = Stage::Resize;
                    r.frames = 0;
                    return;
                };
                if r.frames == 1 {
                    let _ = window.request_inner_size(LogicalSize::new(probe[0], probe[1]));
                    return;
                }
                if r.frames < 30 || !steady {
                    return;
                }
                let screen = screen.unwrap_or([0., 0.]);
                println!(
                    "WINDOW REVIEW: asked for {} without fitting; the compositor gave {} on a {} \
                     output ({})",
                    show(probe),
                    show(size),
                    show(screen),
                    if size[0] > screen[0] || size[1] > screen[1] {
                        "bigger than the output: it doesn't shrink a window that asks for too much"
                    } else {
                        "it fits"
                    }
                );
                r.stage = Stage::Resize;
                r.frames = 0;
            }
            Stage::Resize => {
                let Some(resize) = r.resize else {
                    r.stage = Stage::Done;
                    r.frames = 0;
                    return;
                };
                if r.frames == 1 {
                    // A size the game asks for itself applies at once with no
                    // resize event; a player's resize comes from the
                    // compositor, so the script has KWin make it.
                    println!(
                        "WINDOW REVIEW: waiting for the compositor to resize the window to {}",
                        show(resize)
                    );
                    use std::io::Write;
                    let _ = std::io::stdout().flush();
                    return;
                }
                if !near(size, resize) || !steady {
                    return;
                }
                let saved = self.game.prefs.window_size;
                assert!(
                    saved.is_some_and(|s| near(s, resize)),
                    "WINDOW REVIEW: resized to {} but the preference holds {:?}",
                    show(size),
                    saved
                );
                println!(
                    "WINDOW REVIEW: resized to {}; the preference holds {}",
                    show(size),
                    show(saved.unwrap())
                );
                r.stage = Stage::Done;
                r.frames = 0;
            }
            Stage::Done => {
                println!("WINDOW REVIEW PASS: quitting through the normal save");
                r.finished = true;
            }
        }
    }
}
