//! `--fullscreen-review`: switch fullscreen while the game runs, through
//! F11's code path and through the journal's switch clicked with egui pointer
//! events, each way, and check the window, the frames and the saved setting
//! after every switch. Captures go to `captures/fullscreen/`.
//!
//! It refuses to run outside `scripts/nested-kwin.sh`'s private compositor or
//! without a throwaway `XDG_DATA_HOME`, so it can neither take over the
//! desktop in use nor touch a player's settings.
use crate::App;
use crate::game::{Game, JournalPage, Mode};
use winit::dpi::PhysicalSize;

const DIR: &str = "captures/fullscreen";
/// Frames the compositor gets to resize the window after a switch.
const WAIT: u32 = 600;
/// Frames rendered at the new size before checking and capturing.
const SETTLE: u32 = 20;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Switch {
    F11,
    Journal,
}
/// Each switch: how, whether it leaves the window fullscreen, and its capture.
const SWITCHES: [(Switch, bool, &str); 4] = [
    (Switch::F11, true, "01-f11-fullscreen"),
    (Switch::Journal, false, "02-journal-windowed"),
    (Switch::Journal, true, "03-journal-fullscreen"),
    (Switch::F11, false, "04-f11-windowed"),
];
/// The journal's Fullscreen switch and its close button, in design units.
fn fullscreen_switch() -> (f32, f32) {
    (392. + 155., crate::ui::JOURNAL_TOGGLES_Y + 132. + 17.5)
}
const CLOSE_JOURNAL: (f32, f32) = (720., 769.);

/// Why the review may not run here, if it may not.
pub fn refusal() -> Option<String> {
    if !cfg!(target_os = "linux") {
        return Some("the fullscreen review runs only on Linux, in a private KWin".into());
    }
    if std::env::var("GRAVEWAKE_NESTED_KWIN").as_deref() != Ok("1") {
        return Some(
            "the fullscreen review switches the window to fullscreen, so it runs only inside \
             the private compositor: use scripts/fullscreen-review.sh"
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
            "the fullscreen review saves settings, so it needs XDG_DATA_HOME set to a \
             throwaway folder"
                .into(),
        ),
    }
}

pub struct Review {
    /// 0 warms up windowed; then one step per entry of `SWITCHES`.
    step: usize,
    frames: u32,
    /// Frame of this step on which the window reached the expected size.
    reached: Option<u32>,
    /// Frame on which this step's capture was taken.
    captured: Option<u32>,
    windowed: PhysicalSize<u32>,
    output: PhysicalSize<u32>,
    /// Frames rendered, counted by the main loop.
    pub rendered: u32,
    rendered_at_reach: u32,
    click: Option<((f32, f32), bool)>,
    pub finished: bool,
}
impl Review {
    pub fn new() -> Self {
        std::fs::create_dir_all(DIR).expect("create fullscreen review directory");
        Self {
            step: 0,
            frames: 0,
            reached: None,
            captured: None,
            windowed: PhysicalSize::new(0, 0),
            output: PhysicalSize::new(0, 0),
            rendered: 0,
            rendered_at_reach: 0,
            click: None,
            finished: false,
        }
    }
    /// A click this frame on a design-unit point: pressed, then released.
    pub fn pointer(&self) -> Option<((f32, f32), bool)> {
        self.click
    }
}

fn saved_fullscreen() -> Option<bool> {
    let path = Game::save_path().with_file_name("settings.json");
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice::<serde_json::Value>(&bytes)
        .ok()?
        .get("fullscreen")?
        .as_bool()
}

impl App {
    /// One frame of the review; returns a capture to take this frame.
    pub(crate) fn fullscreen_step(&mut self) -> Option<String> {
        let mut r = self.fullscreen_review.take()?;
        let capture = self.fullscreen_frame(&mut r);
        self.fullscreen_review = Some(r);
        capture
    }
    fn fullscreen_frame(&mut self, r: &mut Review) -> Option<String> {
        r.frames += 1;
        r.click = None;
        // Keep the fight going behind the review without ending the run.
        self.game.run.hp = self.game.max_hp();
        let window = self.window.as_ref()?.clone();
        let surface = self
            .renderer
            .as_ref()
            .map(|renderer| (renderer.config.width, renderer.config.height))?;
        if r.step == 0 {
            if r.frames == 1 {
                self.game.new_run();
                self.game.prefs.volume = 0.;
                self.game.prefs.music_volume = 0.;
            }
            if r.frames == 40 {
                r.windowed = window.inner_size();
                r.output = window
                    .current_monitor()
                    .expect("the private compositor has an output")
                    .size();
                assert!(window.fullscreen().is_none(), "the review starts windowed");
                assert_ne!(r.windowed, r.output, "the window starts smaller than the output");
                println!(
                    "FULLSCREEN REVIEW: windowed {}x{} on a {}x{} output",
                    r.windowed.width, r.windowed.height, r.output.width, r.output.height
                );
                return Some(format!("{DIR}/00-windowed.png"));
            }
            if r.frames == 42 {
                r.step = 1;
                r.frames = 0;
            }
            return None;
        }
        let (how, fullscreen, name) = SWITCHES[r.step - 1];
        let expected = if fullscreen { r.output } else { r.windowed };
        // Start the switch.
        match (how, r.frames) {
            (Switch::F11, 1) => {
                self.game.mode = Mode::Arena;
                self.toggle_fullscreen();
            }
            (Switch::Journal, 1) => {
                self.game.mode = Mode::Paused;
                self.game.settings = true;
                self.game.journal_page = JournalPage::Preferences;
            }
            (Switch::Journal, 20) => r.click = Some((fullscreen_switch(), true)),
            (Switch::Journal, 21) => r.click = Some((fullscreen_switch(), false)),
            _ => {}
        }
        // The frame the switch happened on: F11's call, or the click's release.
        let switched = if how == Switch::F11 { 1 } else { 21 };
        if r.frames <= switched {
            return None;
        }
        let size = window.inner_size();
        if r.reached.is_none() {
            if size == expected
                && surface == (size.width, size.height)
                && window.fullscreen().is_some() == fullscreen
            {
                r.reached = Some(r.frames);
                r.rendered_at_reach = r.rendered;
            } else {
                assert!(
                    r.frames < WAIT,
                    "FULLSCREEN REVIEW: {name}: after {WAIT} frames the window is {}x{} \
                     (surface {}x{}, fullscreen {}), expected {}x{} (fullscreen {fullscreen})",
                    size.width,
                    size.height,
                    surface.0,
                    surface.1,
                    window.fullscreen().is_some(),
                    expected.width,
                    expected.height,
                );
                return None;
            }
        }
        let reached = r.reached.unwrap();
        if r.captured.is_none() {
            if r.frames < reached + SETTLE {
                return None;
            }
            assert_eq!(size, expected, "{name}: the window kept its size");
            assert_eq!(surface, (size.width, size.height), "{name}: the surface follows the window");
            assert_eq!(self.game.prefs.fullscreen, fullscreen, "{name}: the preference");
            let frames = r.rendered - r.rendered_at_reach;
            assert!(frames >= SETTLE - 1, "{name}: frames kept rendering ({frames})");
            println!(
                "FULLSCREEN REVIEW: {name}: {}x{} within {} frames of the switch, surface {}x{}, {frames} frames rendered since",
                size.width,
                size.height,
                reached - switched,
                surface.0,
                surface.1,
            );
            r.captured = Some(r.frames);
            return Some(format!("{DIR}/{name}.png"));
        }
        let captured = r.captured.unwrap();
        // The journal saves its settings when it closes; F11 saves at once.
        if how == Switch::Journal {
            if r.frames == captured + 2 {
                r.click = Some((CLOSE_JOURNAL, true));
            }
            if r.frames == captured + 3 {
                r.click = Some((CLOSE_JOURNAL, false));
            }
            if r.frames < captured + 6 {
                return None;
            }
            assert!(!self.game.settings, "{name}: the journal closed");
        } else if r.frames < captured + 2 {
            return None;
        }
        assert_eq!(
            saved_fullscreen(),
            Some(fullscreen),
            "{name}: settings.json records the switch"
        );
        println!("FULLSCREEN REVIEW: {name}: settings.json has \"fullscreen\": {fullscreen}");
        r.step += 1;
        r.frames = 0;
        r.reached = None;
        r.captured = None;
        if r.step > SWITCHES.len() {
            println!(
                "FULLSCREEN REVIEW PASS: F11 and the journal switch fullscreen both ways while \
                 the game runs; window, surface, frames and settings.json checked after each"
            );
            r.finished = true;
        }
        None
    }
}
