//! `--input-review`: real key and mouse presses, sent through KWin's
//! fake-input protocol inside `scripts/input-review.sh`'s private compositor,
//! so they reach the game through Wayland, winit and `main.rs`'s handlers as
//! a keyboard's and a mouse's would. The game runs as a normal session with a
//! throwaway data folder (saves on, scripted input off); after each press
//! the review waits for the game to react, and fails if it doesn't. Captures
//! and the log go to `captures/input/`.
//!
//! Like the fullscreen review it refuses to run outside the private
//! compositor or without a throwaway `XDG_DATA_HOME`.
use crate::App;
use crate::controls::{Action, Trigger};
use crate::game::{Ending, Game, JournalPage, Mode, Preferences, Records};
use winit::dpi::PhysicalSize;
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

const DIR: &str = "captures/input";
/// Frames a step may wait for the game to react before the review fails.
const TIMEOUT: u32 = 600;
/// Frames toggle sprint must stay on after its key is released.
const LATCH_FRAMES: u32 = 30;

/// Linux input event codes (linux/input-event-codes.h).
mod code {
    pub const ESC: u32 = 1;
    pub const W: u32 = 17;
    pub const E: u32 = 18;
    pub const T: u32 = 20;
    pub const F: u32 = 33;
    pub const LEFT_SHIFT: u32 = 42;
    pub const F11: u32 = 87;
    pub const BUTTON_LEFT: u32 = 0x110;
}

/// The middle of Fire's button on the journal's Keyboard page, in design
/// units: the first row of the second column (`ui::journal_controls`).
fn fire_button() -> (f64, f64) {
    (372. + 372. + 128. + 98., 304. + 18.)
}

/// Why the review may not run here, if it may not.
pub fn refusal() -> Option<String> {
    if !cfg!(target_os = "linux") {
        return Some("the input review runs only on Linux, in a private KWin".into());
    }
    if std::env::var("GRAVEWAKE_NESTED_KWIN").as_deref() != Ok("1")
        || std::env::var("GRAVEWAKE_FAKE_INPUT").as_deref() != Ok("1")
    {
        return Some(
            "the input review sends key presses through the compositor, so it runs only \
             inside the private compositor: use scripts/input-review.sh"
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
            "the input review plays a normal session that saves, so it needs XDG_DATA_HOME \
             set to a throwaway folder"
                .into(),
        ),
    }
}

#[cfg(target_os = "linux")]
mod fake {
    use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, protocol::wl_registry};
    use wayland_protocols_plasma::fake_input::client::org_kde_kwin_fake_input::OrgKdeKwinFakeInput;

    struct State {
        input: Option<OrgKdeKwinFakeInput>,
    }
    impl Dispatch<wl_registry::WlRegistry, ()> for State {
        fn event(
            state: &mut Self,
            registry: &wl_registry::WlRegistry,
            event: wl_registry::Event,
            _: &(),
            _: &Connection,
            queue: &QueueHandle<Self>,
        ) {
            if let wl_registry::Event::Global {
                name,
                interface,
                version,
            } = event
            {
                if interface == "org_kde_kwin_fake_input" && version >= 4 {
                    state.input = Some(registry.bind(name, version.min(5), queue, ()));
                }
            }
        }
    }
    impl Dispatch<OrgKdeKwinFakeInput, ()> for State {
        fn event(
            _: &mut Self,
            _: &OrgKdeKwinFakeInput,
            _: <OrgKdeKwinFakeInput as wayland_client::Proxy>::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
    }
    /// A second Wayland connection to the compositor, used only to inject
    /// input as if from a keyboard and mouse.
    pub struct FakeInput {
        connection: Connection,
        queue: EventQueue<State>,
        state: State,
    }
    impl FakeInput {
        pub fn connect() -> Self {
            let connection =
                Connection::connect_to_env().expect("connect to the private compositor");
            let mut queue = connection.new_event_queue();
            let handle = queue.handle();
            connection.display().get_registry(&handle, ());
            let mut state = State { input: None };
            queue.roundtrip(&mut state).expect("list the compositor's globals");
            let input = state.input.as_ref().expect(
                "the compositor offers no org_kde_kwin_fake_input (scripts/input-review.sh opens \
                 it with KWIN_WAYLAND_NO_PERMISSION_CHECKS inside its private KWin)",
            );
            input.authenticate("Gravewake input review".into(), "scripted key presses".into());
            let mut fake = Self {
                connection,
                queue,
                state,
            };
            fake.flush();
            fake
        }
        fn input(&self) -> &OrgKdeKwinFakeInput {
            self.state.input.as_ref().unwrap()
        }
        fn flush(&mut self) {
            self.queue
                .roundtrip(&mut self.state)
                .expect("the compositor took the fake input");
            let _ = self.connection.flush();
        }
        pub fn key(&mut self, code: u32, pressed: bool) {
            self.input().keyboard_key(code, pressed as u32);
            self.flush();
        }
        pub fn button(&mut self, code: u32, pressed: bool) {
            self.input().button(code, pressed as u32);
            self.flush();
        }
        pub fn motion(&mut self, dx: f64, dy: f64) {
            self.input().pointer_motion(dx, dy);
            self.flush();
        }
        pub fn move_to(&mut self, x: f64, y: f64) {
            self.input().pointer_motion_absolute(x, y);
            self.flush();
        }
    }
}
#[cfg(not(target_os = "linux"))]
mod fake {
    /// `refusal` stops the review before this is reached.
    pub struct FakeInput;
    impl FakeInput {
        pub fn connect() -> Self {
            unreachable!("the input review runs only on Linux")
        }
        pub fn key(&mut self, _: u32, _: bool) {}
        pub fn button(&mut self, _: u32, _: bool) {}
        pub fn motion(&mut self, _: f64, _: f64) {}
        pub fn move_to(&mut self, _: f64, _: f64) {}
    }
}

pub struct Review {
    fake: Option<fake::FakeInput>,
    step: usize,
    /// Frames since this step began.
    frames: u32,
    /// Frame of this step on which its condition first held.
    held_since: Option<u32>,
    yaw: f32,
    /// Rounds left after the quick click.
    ammo: u32,
    windowed: PhysicalSize<u32>,
    output: PhysicalSize<u32>,
    pub finished: bool,
}
impl Review {
    pub fn new() -> Self {
        std::fs::create_dir_all(DIR).expect("create input review directory");
        Self {
            fake: None,
            step: 0,
            frames: 0,
            held_since: None,
            yaw: 0.,
            ammo: 0,
            windowed: PhysicalSize::new(0, 0),
            output: PhysicalSize::new(0, 0),
            finished: false,
        }
    }
    fn fake(&mut self) -> &mut fake::FakeInput {
        self.fake.get_or_insert_with(fake::FakeInput::connect)
    }
    fn tap(&mut self, code: u32) {
        self.fake().key(code, true);
        self.fake().key(code, false);
    }
}

fn saved_settings() -> serde_json::Value {
    let path = Game::save_path().with_file_name("settings.json");
    std::fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}
fn saved_records() -> Records {
    let path = Game::save_path().with_file_name("records.json");
    std::fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

/// What a step decided this frame.
enum Next {
    /// Keep waiting; fail after `TIMEOUT` frames with this description.
    Wait(&'static str),
    Done,
}

impl App {
    /// One frame of the review; returns a capture to take this frame.
    pub(crate) fn input_step(&mut self) -> Option<String> {
        let mut r = self.input_review.take()?;
        let (next, capture) = self.input_frame(&mut r);
        match next {
            Next::Done => {
                r.step += 1;
                r.frames = 0;
                r.held_since = None;
            }
            Next::Wait(what) => {
                r.frames += 1;
                assert!(
                    r.frames < TIMEOUT,
                    "INPUT REVIEW: step {}: after {TIMEOUT} frames, still waiting for {what}",
                    r.step
                );
            }
        }
        self.input_review = Some(r);
        capture
    }
    fn input_frame(&mut self, r: &mut Review) -> (Next, Option<String>) {
        use Next::{Done, Wait};
        // Keep the fight from ending the run.
        self.game.run.hp = self.game.max_hp();
        let window = self.window.clone().expect("window");
        let bindings = self.game.prefs.bindings;
        match r.step {
            // The window has keyboard focus, and the pointer rests over it.
            0 => {
                if !self.focused || r.frames < 30 {
                    return (Wait("keyboard focus on the game window"), None);
                }
                self.game.prefs.volume = 0.;
                self.game.prefs.music_volume = 0.;
                self.game.prefs.field_tips = false;
                let output = window.current_monitor().expect("an output").size();
                r.fake().move_to(output.width as f64 / 2., output.height as f64 / 2.);
                println!("INPUT REVIEW: the window has keyboard focus");
                (Done, None)
            }
            // A saved run that hasn't ended, then a new run over it, as the
            // title's confirmation starts one: the chronicle on disk records
            // the saved run as abandoned.
            1 => {
                self.game.new_run();
                self.game.run.wave = 3;
                self.game.mode = Mode::Shop;
                assert!(self.game.save(), "save a run to replace");
                self.game.mode = Mode::Title;
                let before = saved_records().history.len();
                self.game.new_run();
                let records = saved_records();
                assert_eq!(records.history.len(), before + 1, "the chronicle grew by one");
                let entry = &records.history[0];
                assert_eq!(
                    (entry.ending, entry.descent, entry.number),
                    (Ending::Abandoned, 3, 1),
                    "records.json names the replaced run abandoned"
                );
                println!(
                    "INPUT REVIEW: a new run over a saved descent-3 run entered it in records.json as abandoned"
                );
                self.sync_cursor();
                (Done, None)
            }
            // Escape pauses, and again resumes.
            2 => {
                if !(self.captured && self.game.mode == Mode::Arena && r.frames > 20) {
                    return (Wait("the arena with the pointer captured"), None);
                }
                r.tap(code::ESC);
                (Done, None)
            }
            3 => {
                if self.game.mode != Mode::Paused {
                    return (Wait("Escape to pause"), None);
                }
                println!("INPUT REVIEW: Escape paused the fight");
                r.tap(code::ESC);
                (Done, Some(format!("{DIR}/01-paused-by-escape.png")))
            }
            4 => {
                if self.game.mode != Mode::Arena {
                    return (Wait("Escape to resume"), None);
                }
                println!("INPUT REVIEW: Escape resumed it");
                // Hold sprint: W and Shift down.
                self.game.prefs.toggle_sprint = false;
                r.fake().key(code::W, true);
                r.fake().key(code::LEFT_SHIFT, true);
                (Done, None)
            }
            5 => {
                if !(self.game.input.forward > 0. && self.game.input.sprint) {
                    return (Wait("W and a held Shift to move and sprint"), None);
                }
                r.fake().key(code::LEFT_SHIFT, false);
                (Done, Some(format!("{DIR}/02-sprint-held.png")))
            }
            6 => {
                if self.game.input.sprint {
                    return (Wait("sprint to end when Shift is released (hold)"), None);
                }
                assert!(self.game.input.forward > 0., "W is still held");
                println!("INPUT REVIEW: hold sprint lasted only while Shift was down");
                // Toggle sprint: a press and release starts it.
                self.game.prefs.toggle_sprint = true;
                r.tap(code::LEFT_SHIFT);
                (Done, None)
            }
            7 => {
                if !self.game.input.sprint {
                    r.held_since = None;
                    return (Wait("a Shift press to start toggle sprint"), None);
                }
                let since = *r.held_since.get_or_insert(r.frames);
                if r.frames < since + LATCH_FRAMES {
                    return (Wait("toggle sprint to stay on after Shift's release"), None);
                }
                println!(
                    "INPUT REVIEW: toggle sprint started on a Shift press and stayed on for {LATCH_FRAMES} frames after the release"
                );
                r.tap(code::LEFT_SHIFT);
                (Done, None)
            }
            8 => {
                if self.game.input.sprint {
                    return (Wait("the next Shift press to stop toggle sprint"), None);
                }
                println!("INPUT REVIEW: the next Shift press stopped it");
                r.fake().key(code::W, false);
                (Done, None)
            }
            // Rebind Reload on the Keyboard page with real key presses.
            9 => {
                if self.game.input.forward != 0. {
                    return (Wait("W's release to stop moving"), None);
                }
                r.tap(code::ESC);
                (Done, None)
            }
            10 => {
                if self.game.mode != Mode::Paused {
                    return (Wait("Escape to pause before the journal"), None);
                }
                assert_eq!(bindings.get(Action::Reload).trigger, Trigger::Key(KeyCode::KeyR));
                assert_eq!(bindings.get(Action::Melee).trigger, Trigger::Key(KeyCode::KeyE));
                // The journal's Keyboard page, waiting for Reload's new key, as
                // a click on Reload's button leaves it.
                self.game.settings = true;
                self.game.journal_page = JournalPage::Keyboard;
                self.game.rebinding = Some(Action::Reload);
                r.tap(code::T);
                (Done, None)
            }
            11 => {
                if bindings.get(Action::Reload).trigger != Trigger::Key(KeyCode::KeyT) {
                    return (Wait("a T press to bind Reload"), None);
                }
                assert!(self.game.rebinding.is_none(), "binding stops waiting");
                println!("INPUT REVIEW: a T press bound Reload to T");
                // E belongs to Melee: binding it swaps the two.
                self.game.rebinding = Some(Action::Reload);
                r.tap(code::E);
                (Done, None)
            }
            12 => {
                if bindings.get(Action::Reload).trigger != Trigger::Key(KeyCode::KeyE) {
                    return (Wait("an E press to bind Reload"), None);
                }
                assert_eq!(
                    bindings.get(Action::Melee).trigger,
                    Trigger::Key(KeyCode::KeyT),
                    "Melee took Reload's old key"
                );
                assert!(!self.game.controls_note.is_empty(), "the page notes the swap");
                if r.held_since.is_none() {
                    println!(
                        "INPUT REVIEW: an E press bound Reload to E and moved Melee to T ({})",
                        self.game.controls_note
                    );
                }
                let since = *r.held_since.get_or_insert(r.frames);
                // The journal fades in over its first frames.
                if r.frames < since + 45 {
                    return (Wait("the page to draw the swap"), None);
                }
                (Done, Some(format!("{DIR}/03-keyboard-page-swap.png")))
            }
            13 => {
                // Close the journal as its button does, and resume with Escape.
                self.game.save_preferences();
                self.game.settings = false;
                r.tap(code::ESC);
                (Done, None)
            }
            14 => {
                if !(self.game.mode == Mode::Arena && self.captured) {
                    return (Wait("Escape to resume after the journal"), None);
                }
                let saved = std::fs::read(Game::save_path().with_file_name("settings.json"))
                    .ok()
                    .and_then(|b| Preferences::from_json(&b))
                    .expect("settings.json saved when the journal closed");
                assert_eq!(saved.bindings, bindings, "settings.json holds the new bindings");
                self.game.run.ammo = 1;
                self.game.reload = 0.;
                r.tap(code::E);
                (Done, None)
            }
            15 => {
                if self.game.reload <= 0. {
                    return (Wait("an E press to reload with the rebound key"), None);
                }
                println!("INPUT REVIEW: E, Reload's new key, started a reload");
                (Done, None)
            }
            // A quick left click fires once; a held one keeps firing;
            // mouse motion turns the view.
            16 => {
                if self.game.reload > 0. || self.game.cooldown > 0. {
                    return (Wait("the reload to finish"), None);
                }
                assert_eq!(self.game.run.ammo, self.game.run.weapon.capacity());
                r.fake().button(code::BUTTON_LEFT, true);
                r.fake().button(code::BUTTON_LEFT, false);
                (Done, None)
            }
            17 => {
                if self.game.run.ammo >= self.game.run.weapon.capacity() {
                    return (Wait("a quick left click to fire"), None);
                }
                println!("INPUT REVIEW: a left click released before the next frame still fired");
                r.ammo = self.game.run.ammo;
                r.fake().button(code::BUTTON_LEFT, true);
                (Done, None)
            }
            18 if r.held_since.is_none() => {
                if self.game.run.ammo + 2 > r.ammo {
                    return (Wait("a held left click to keep firing"), None);
                }
                r.fake().button(code::BUTTON_LEFT, false);
                println!(
                    "INPUT REVIEW: a held left click kept firing ({} of {} rounds left)",
                    self.game.run.ammo,
                    self.game.run.weapon.capacity()
                );
                r.yaw = self.game.run.yaw;
                r.fake().motion(200., 0.);
                r.held_since = Some(r.frames);
                (Wait("mouse motion to turn the view"), None)
            }
            18 => {
                let turned = self.game.run.yaw - r.yaw;
                if turned.abs() < 0.25 {
                    return (Wait("mouse motion to turn the view"), None);
                }
                assert!(!self.game.input.fire, "the release stopped firing");
                println!("INPUT REVIEW: 200 counts of mouse motion turned the view {turned:.2} rad");
                // F11 switches to fullscreen.
                r.windowed = window.inner_size();
                r.output = window.current_monitor().expect("an output").size();
                assert!(window.fullscreen().is_none(), "the review starts windowed");
                r.tap(code::F11);
                (Done, None)
            }
            19 => {
                if !(window.fullscreen().is_some() && window.inner_size() == r.output) {
                    return (Wait("F11 to make the window fullscreen"), None);
                }
                if saved_settings()["fullscreen"] != serde_json::Value::Bool(true) {
                    return (Wait("settings.json to record fullscreen"), None);
                }
                let since = *r.held_since.get_or_insert(r.frames);
                if r.frames < since + 20 {
                    return (Wait("frames at the fullscreen size"), None);
                }
                println!(
                    "INPUT REVIEW: F11 went fullscreen ({}x{}) and settings.json recorded it",
                    r.output.width, r.output.height
                );
                r.tap(code::F11);
                (Done, Some(format!("{DIR}/04-f11-fullscreen.png")))
            }
            20 => {
                if !(window.fullscreen().is_none() && window.inner_size() == r.windowed) {
                    return (Wait("F11 to return to the window"), None);
                }
                if saved_settings()["fullscreen"] != serde_json::Value::Bool(false) {
                    return (Wait("settings.json to record the window"), None);
                }
                println!(
                    "INPUT REVIEW: F11 returned to the {}x{} window and settings.json recorded it",
                    r.windowed.width, r.windowed.height
                );
                // Pause, for the Keyboard page.
                r.tap(code::ESC);
                (Done, None)
            }
            // Fire moves to F: a left click no longer fires, F does.
            21 => {
                if self.game.mode != Mode::Paused {
                    return (Wait("Escape to pause"), None);
                }
                assert_eq!(bindings.get(Action::Fire).trigger, Trigger::Mouse(MouseButton::Left));
                self.game.settings = true;
                self.game.journal_page = JournalPage::Keyboard;
                self.game.rebinding = Some(Action::Fire);
                r.tap(code::F);
                (Done, None)
            }
            22 => {
                if bindings.get(Action::Fire).trigger != Trigger::Key(KeyCode::KeyF) {
                    return (Wait("an F press to bind Fire"), None);
                }
                assert_eq!(
                    bindings.action(Trigger::Mouse(MouseButton::Left)),
                    None,
                    "the left button is free"
                );
                println!("INPUT REVIEW: an F press bound Fire to F ({})", self.game.controls_note);
                self.game.save_preferences();
                self.game.settings = false;
                r.tap(code::ESC);
                (Done, None)
            }
            23 if r.held_since.is_none() => {
                if !(self.game.mode == Mode::Arena && self.captured) {
                    return (Wait("Escape to resume"), None);
                }
                if self.game.reload > 0. || self.game.cooldown > 0. {
                    return (Wait("the weapon to be ready"), None);
                }
                self.game.run.ammo = self.game.run.weapon.capacity();
                r.ammo = self.game.run.ammo;
                r.fake().button(code::BUTTON_LEFT, true);
                r.fake().button(code::BUTTON_LEFT, false);
                r.held_since = Some(r.frames);
                (Wait("frames after a left click"), None)
            }
            23 => {
                if r.frames < r.held_since.unwrap() + 30 {
                    return (Wait("frames after a left click"), None);
                }
                assert_eq!(self.game.run.ammo, r.ammo, "a left click no longer fires");
                println!("INPUT REVIEW: with Fire on F, a left click didn't fire");
                r.tap(code::F);
                (Done, None)
            }
            24 => {
                if self.game.run.ammo >= r.ammo {
                    return (Wait("an F press to fire"), None);
                }
                println!(
                    "INPUT REVIEW: an F press fired ({} of {} rounds left)",
                    self.game.run.ammo,
                    self.game.run.weapon.capacity()
                );
                r.tap(code::ESC);
                (Done, None)
            }
            // Back to the left button by clicking Fire's button twice on the
            // Keyboard page, with the real pointer.
            25 if r.held_since.is_none() => {
                if self.game.mode != Mode::Paused {
                    return (Wait("Escape to pause"), None);
                }
                self.game.settings = true;
                self.game.journal_page = JournalPage::Keyboard;
                self.game.controls_note.clear();
                let output = window.current_monitor().expect("an output").size();
                r.output = output;
                r.fake().move_to(output.width as f64 / 2., output.height as f64 / 2.);
                r.held_since = Some(r.frames);
                (Wait("the pointer over the window"), None)
            }
            25 => {
                if r.frames < r.held_since.unwrap() + 20 {
                    return (Wait("the pointer over the window"), None);
                }
                // The output's centre, as the window sees it, places the window.
                let seen = self.last_pointer.expect("the pointer crossed the window");
                let origin = (
                    r.output.width as f64 / 2. - seen.x,
                    r.output.height as f64 / 2. - seen.y,
                );
                let size = window.inner_size();
                let scale = (size.width as f64 / 1440.).min(size.height as f64 / 900.);
                let (x, y) = fire_button();
                r.fake().move_to(
                    origin.0 + (size.width as f64 - 1440. * scale) / 2. + x * scale,
                    origin.1 + y * scale,
                );
                (Done, None)
            }
            26 if r.held_since.is_none() => {
                if r.frames < 10 {
                    return (Wait("the pointer on Fire's button"), None);
                }
                r.fake().button(code::BUTTON_LEFT, true);
                r.fake().button(code::BUTTON_LEFT, false);
                r.held_since = Some(r.frames);
                (Wait("a click on Fire's button"), None)
            }
            26 => {
                if self.game.rebinding != Some(Action::Fire) {
                    return (Wait("a click on Fire's button to wait for a key"), None);
                }
                println!("INPUT REVIEW: a click on Fire's button waits for its new input");
                r.fake().button(code::BUTTON_LEFT, true);
                r.fake().button(code::BUTTON_LEFT, false);
                (Done, None)
            }
            27 => {
                if bindings.get(Action::Fire).trigger != Trigger::Mouse(MouseButton::Left) {
                    return (Wait("a second click to bind Fire to the left button"), None);
                }
                assert!(self.game.rebinding.is_none(), "binding stops waiting");
                assert_eq!(bindings.action(Trigger::Key(KeyCode::KeyF)), None, "F is free");
                let since = *r.held_since.get_or_insert(r.frames);
                if r.frames < since + 20 {
                    return (Wait("the page to draw the new binding"), None);
                }
                println!(
                    "INPUT REVIEW: a second click on Fire's button bound it to the left button ({})",
                    self.game.controls_note
                );
                self.game.save_preferences();
                self.game.settings = false;
                r.tap(code::ESC);
                (Done, Some(format!("{DIR}/05-keyboard-fire-left-button.png")))
            }
            28 if r.held_since.is_none() => {
                if !(self.game.mode == Mode::Arena && self.captured) {
                    return (Wait("Escape to resume"), None);
                }
                if self.game.reload > 0. || self.game.cooldown > 0. {
                    return (Wait("the weapon to be ready"), None);
                }
                let saved = std::fs::read(Game::save_path().with_file_name("settings.json"))
                    .ok()
                    .and_then(|b| Preferences::from_json(&b))
                    .expect("settings.json saved when the journal closed");
                assert_eq!(saved.bindings, bindings, "settings.json holds Fire on the left button");
                self.game.run.ammo = self.game.run.weapon.capacity();
                r.ammo = self.game.run.ammo;
                r.fake().button(code::BUTTON_LEFT, true);
                r.fake().button(code::BUTTON_LEFT, false);
                r.held_since = Some(r.frames);
                (Wait("a left click to fire again"), None)
            }
            28 => {
                if self.game.run.ammo >= r.ammo {
                    return (Wait("a left click to fire again"), None);
                }
                println!("INPUT REVIEW: back on the left button, a left click fired");
                println!(
                    "INPUT REVIEW PASS: Escape, hold and toggle sprint, rebinding with real key \
                     presses, the rebound key, mouse fire and look, F11 both ways, and Fire \
                     moved to a key and back to the left button"
                );
                r.finished = true;
                self.game.quit_requested = true;
                (Done, None)
            }
            _ => (Wait("nothing"), None),
        }
    }
}
