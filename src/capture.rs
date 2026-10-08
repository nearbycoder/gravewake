//! Capture-only options for recording the trailer and the README's
//! screenshots (docs/TRAILER.md). Only scripted runs read them; a player's
//! launch never does, and nothing here is saved.
//!
//! - `--capture-size 1920x1080`: the window size, in logical pixels, for
//!   every scripted review instead of its usual 960×600 or 1440×900.
//! - `--capture-fidelity ultra`: the Graphics Fidelity step to draw with
//!   (`low`, `medium`, `high` or `ultra`) instead of the default.
//! - `--capture-full-mix`: the motion, survival and anatomy reviews also mix
//!   the adaptive score, the ambience and the menu cues into their recorded
//!   audio, at the levels a player hears with the default volumes. Without
//!   it they record sound effects only, as before.
use crate::{
    fidelity::Fidelity,
    game::{Game, Preferences},
    music,
};
use std::sync::{Arc, OnceLock};

fn value(flag: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let at = args.iter().position(|a| a == flag)?;
    let value = args.get(at + 1).cloned();
    assert!(value.is_some(), "{flag} needs a value");
    value
}

/// `--capture-size WxH`, in logical pixels.
pub fn size() -> Option<(f32, f32)> {
    let text = value("--capture-size")?;
    let parsed = text
        .split_once('x')
        .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)));
    let (w, h): (f32, f32) =
        parsed.unwrap_or_else(|| panic!("--capture-size {text}: expected WIDTHxHEIGHT"));
    assert!(
        w >= crate::game::MIN_WINDOW[0] && h >= crate::game::MIN_WINDOW[1],
        "--capture-size {text}: smaller than the game's minimum window"
    );
    Some((w, h))
}

/// `--capture-fidelity STEP`.
pub fn fidelity() -> Option<Fidelity> {
    let text = value("--capture-fidelity")?;
    let step = Fidelity::ALL
        .into_iter()
        .find(|f| f.name().eq_ignore_ascii_case(&text));
    Some(step.unwrap_or_else(|| {
        panic!("--capture-fidelity {text}: expected low, medium, high or ultra")
    }))
}

pub fn full_mix() -> bool {
    std::env::args().any(|a| a == "--capture-full-mix")
}

/// Review audio holds sound effects at 0.8, where a player at the default
/// sound volume hears them at `volume`; the score, ambience and cues are
/// mixed with the same ratio.
fn review_gain() -> f32 {
    0.8 / Preferences::default().volume
}

/// The score and ambience a player hears, rendered one 1/60 s frame at a
/// time into a review's 48 kHz stereo buffer, following the game's state
/// exactly as the live mixer does.
pub struct Bed {
    score: music::Score,
    mix: Arc<music::Mix>,
    ambience: Vec<f32>,
    /// Stereo frames written so far, at `audio::RATE`.
    written: usize,
    last: [f32; 2],
}
impl Bed {
    pub fn new() -> Self {
        let stems = Arc::new(OnceLock::new());
        let _ = stems.set(music::compose());
        let mix = Arc::new(music::Mix::default());
        Self {
            score: music::Score::new(stems, mix.clone()),
            mix,
            ambience: crate::audio::ambience_loop(),
            written: 0,
            last: [0.; 2],
        }
    }
    /// Add one frame of score and ambience to `out` from stereo frame `at`.
    pub fn frame(&mut self, game: &Game, out: &mut [f32], at: usize) {
        let prefs = &game.prefs;
        let gain = review_gain();
        // The live mixer's master level for the score (see `App::frame`).
        self.mix.set(
            music::targets(game),
            prefs.music_volume * prefs.volume * 1.5 * gain,
        );
        let ambience = prefs.volume * crate::audio::AMBIENCE_LEVEL * gain;
        let frames = crate::audio::RATE as usize / 60;
        // The score is at half the output rate: each of its frames becomes
        // two, the first halfway from the previous one.
        let ratio = crate::audio::RATE / music::RATE;
        assert_eq!(ratio, 2);
        for i in 0..frames / 2 {
            let next = [self.score.next().unwrap(), self.score.next().unwrap()];
            for (k, value) in [
                [
                    (self.last[0] + next[0]) * 0.5,
                    (self.last[1] + next[1]) * 0.5,
                ],
                next,
            ]
            .into_iter()
            .enumerate()
            {
                let n = self.written + i * 2 + k;
                let t = n as f64 * crate::audio::AMBIENCE_RATE as f64 / crate::audio::RATE as f64;
                let a = t.floor() as usize;
                let f = (t - a as f64) as f32;
                let len = self.ambience.len();
                let wind = self.ambience[a % len] * (1. - f) + self.ambience[(a + 1) % len] * f;
                let offset = (at + i * 2 + k) * 2;
                for channel in 0..2 {
                    if let Some(s) = out.get_mut(offset + channel) {
                        *s += value[channel] + wind * ambience;
                    }
                }
            }
            self.last = next;
        }
        self.written += frames;
    }
}

/// A menu cue's level in review audio: the live game plays cues at 0.8 of
/// the sound volume.
pub fn cue_gain(game: &Game) -> f32 {
    game.prefs.volume * 0.8 * review_gain()
}
