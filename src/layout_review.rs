//! `--layout-review`: key names from the compositor's keyboard layout, before
//! any key is pressed. Run by `scripts/layout-review.sh` inside the private
//! KWin, once with each layout it tests. It starts a run (the opening banner
//! names the movement keys), waits for the layout to arrive from `keymap`,
//! checks the movement label against `GRAVEWAKE_EXPECT_MOVEMENT`, and takes a
//! capture in `captures/layout/`. Nothing is saved.
use crate::App;

const DIR: &str = "captures/layout";
/// Give up if the layout hasn't arrived after this many frames.
const PATIENCE: u32 = 600;

pub fn refusal() -> Option<String> {
    if !cfg!(target_os = "linux") {
        return Some("the layout review runs only on Linux, in a private KWin".into());
    }
    if std::env::var("GRAVEWAKE_NESTED_KWIN").as_deref() != Ok("1") {
        return Some(
            "the layout review runs inside the private compositor, whose layout it sets: \
             use scripts/layout-review.sh"
                .into(),
        );
    }
    std::env::var("GRAVEWAKE_EXPECT_MOVEMENT")
        .is_err()
        .then(|| "set GRAVEWAKE_EXPECT_MOVEMENT to the movement label expected".into())
}

#[derive(Default)]
pub struct Review {
    frames: u32,
    /// The frame the layout arrived on.
    arrived: Option<u32>,
    captured: bool,
    pub finished: bool,
}
impl Review {
    pub fn new() -> Self {
        std::fs::create_dir_all(DIR).expect("create layout review directory");
        Self::default()
    }
}

impl App {
    pub(crate) fn layout_step(&mut self) -> Option<String> {
        let r = self.layout_review.as_mut()?;
        r.frames += 1;
        if r.captured {
            r.finished = true;
            return None;
        }
        if r.frames == 1 {
            self.game.new_run();
            self.game.prefs.volume = 0.;
            self.game.prefs.music_volume = 0.;
        }
        let game = &mut self.game;
        game.run.hp = game.max_hp();
        if game.layout.is_empty() {
            assert!(
                r.frames < PATIENCE,
                "LAYOUT REVIEW: no keyboard layout arrived in {PATIENCE} frames"
            );
            return None;
        }
        // Let the banner settle for half a second after the layout arrives.
        let arrived = *r.arrived.get_or_insert(r.frames);
        if r.frames < arrived + 30 {
            return None;
        }
        let expected = std::env::var("GRAVEWAKE_EXPECT_MOVEMENT").unwrap();
        let movement = game.prefs.bindings.movement_label();
        let ember = game.prefs.bindings.label(crate::controls::Action::Bolt);
        println!(
            "LAYOUT REVIEW: {} keys named by the layout; movement {movement}, Ember Bolt {ember}, \
             before any key press",
            game.layout.len()
        );
        assert_eq!(movement, expected, "LAYOUT REVIEW: movement label");
        println!("LAYOUT REVIEW PASS: the opening banner names {movement}");
        r.captured = true;
        let name = expected.replace(' ', "").to_lowercase();
        Some(format!("{DIR}/opening-{name}.png"))
    }
}
