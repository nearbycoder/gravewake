//! Adaptive score: three synchronized, synthesized layers over one 32-second
//! loop in D minor (Dm, B♭, Gm, A, two bars each at 60 BPM).
//!
//! - Calm: organ-and-choir pads, a low pedal and a distant bell. Title,
//!   Collector, Binding, Armory and endings.
//! - Pressure: a heartbeat drum and a bowed ostinato. Rises with the crowd
//!   and with low health.
//! - Boss: a tritone drone, war drums and a dissonant choir while the
//!   Tithekeeper lives.
//!
//! The layers are rendered once on a background thread at start-up, so the
//! window isn't delayed. One rodio source plays all three from the same
//! position and crossfades their gains, so they can never drift apart.
use crate::game::{Game, Mode};
use rodio::Source;
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicU32, Ordering},
};

pub const RATE: u32 = 24000;
pub const LOOP_SECONDS: f32 = 32.;
/// Rendered past the loop end, then folded back onto its start, so pads,
/// bells and echoes ring across the seam.
const TAIL_SECONDS: f32 = 8.;
const PEAK: f32 = 0.32;
/// Crossfade time constant between layer mixes, in seconds.
const CROSSFADE: f32 = 2.5;
pub const LAYERS: [&str; 3] = ["calm", "pressure", "boss"];

/// Interleaved stereo layers of equal length.
pub struct Stems {
    pub layers: [Vec<f32>; 3],
}

fn midi(note: f32) -> f32 {
    440. * 2f32.powf((note - 69.) / 12.)
}

/// Upper voices and the bass note of each two-bar chord.
const CHORDS: [([f32; 3], f32); 4] = [
    ([57., 62., 65.], 38.), // Dm: A3 D4 F4 over D2
    ([58., 62., 65.], 34.), // B♭: B♭3 D4 F4 over B♭1
    ([58., 62., 67.], 31.), // Gm: B♭3 D4 G4 over G1
    ([57., 61., 64.], 33.), // A: A3 C♯4 E4 over A1
];
const CHORD_SECONDS: f32 = 8.;

struct Buffer {
    left: Vec<f32>,
    right: Vec<f32>,
    seed: u32,
}
struct Tone<'a> {
    start: f32,
    duration: f32,
    attack: f32,
    release: f32,
    freq: f32,
    harmonics: &'a [f32],
    amp: f32,
    pan: f32,
    vibrato: f32,
    /// Slow swell: (depth, period in seconds).
    swell: (f32, f32),
}
impl Buffer {
    fn new() -> Self {
        let n = ((LOOP_SECONDS + TAIL_SECONDS) * RATE as f32) as usize;
        Self {
            left: vec![0.; n],
            right: vec![0.; n],
            seed: 0x5eed_1e55,
        }
    }
    fn noise(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.seed as f32 / u32::MAX as f32) * 2. - 1.
    }
    fn span(&self, start: f32, duration: f32) -> std::ops::Range<usize> {
        let a = (start * RATE as f32) as usize;
        let b = (((start + duration) * RATE as f32) as usize).min(self.left.len());
        a.min(b)..b
    }
    fn put(&mut self, i: usize, x: f32, pan: f32) {
        // Equal-power pan, -1 (left) to 1 (right).
        let angle = (pan.clamp(-1., 1.) + 1.) * std::f32::consts::FRAC_PI_4;
        self.left[i] += x * angle.cos();
        self.right[i] += x * angle.sin();
    }
    fn tone(&mut self, tone: Tone) {
        let tau = std::f32::consts::TAU;
        let mut phase = 0f32;
        let length = tone.duration + tone.release;
        for i in self.span(tone.start, length) {
            let t = i as f32 / RATE as f32 - tone.start;
            let rise = (t / tone.attack).min(1.);
            let fall = ((length - t) / tone.release).clamp(0., 1.);
            let envelope = rise * rise * (3. - 2. * rise) * fall * fall * (3. - 2. * fall);
            // Every voice shares one vibrato phase, so detuned twins keep a
            // steady, slow beat instead of warbling against each other.
            let vibrato = 1. + tone.vibrato * (tau * 5.1 * t).sin() * (t / 1.5).min(1.);
            phase = (phase + tau * tone.freq * vibrato / RATE as f32) % (tau * 64.);
            let mut x = 0.;
            for (k, h) in tone.harmonics.iter().enumerate() {
                x += h * ((k + 1) as f32 * phase).sin();
            }
            let (depth, period) = tone.swell;
            let swell = 1. - depth * 0.5 * (1. + (tau * t / period).cos());
            self.put(i, x * envelope * swell * tone.amp, tone.pan);
        }
    }
    /// A pitched drum: a falling sine body and a short noise skin.
    fn drum(&mut self, start: f32, from: f32, to: f32, amp: f32, decay: f32, skin: f32, pan: f32) {
        let tau = std::f32::consts::TAU;
        let mut phase = 0f32;
        let mut low = 0.;
        for i in self.span(start, 6. / decay) {
            let t = i as f32 / RATE as f32 - start;
            let freq = to + (from - to) * (-t * 30.).exp();
            phase += tau * freq / RATE as f32;
            let click = (t * 2000.).min(1.);
            let noise = self.noise();
            low += (noise - low) * 0.08;
            let x = phase.sin() * (-t * decay).exp() + low * skin * (-t * decay * 4.).exp();
            self.put(i, x * amp * click, pan);
        }
    }
    /// A struck bell with inharmonic partials and long, uneven decays.
    fn bell(&mut self, start: f32, freq: f32, amp: f32, pan: f32) {
        let tau = std::f32::consts::TAU;
        const PARTIALS: [(f32, f32, f32); 7] = [
            (0.5, 0.55, 0.35),
            (1.0, 1.0, 0.55),
            (1.19, 0.6, 0.8),
            (1.5, 0.35, 0.9),
            (2.0, 0.4, 1.1),
            (2.51, 0.22, 1.6),
            (3.01, 0.15, 2.2),
        ];
        for i in self.span(start, 9.) {
            let t = i as f32 / RATE as f32 - start;
            let strike = (t * 400.).min(1.);
            let x: f32 = PARTIALS
                .iter()
                .map(|(ratio, gain, decay)| {
                    gain * (tau * freq * ratio * t).sin() * (-t * decay).exp()
                })
                .sum();
            self.put(i, x * amp * strike, pan);
        }
    }
    /// Cross-fed stereo echoes, softened each pass, to place the score in
    /// the same open night as the arena.
    fn space(&mut self, wet: f32) {
        let delays = [(0.29 * RATE as f32) as usize, (0.37 * RATE as f32) as usize];
        let feedback = 0.38;
        let (mut dl, mut dr) = (vec![0f32; self.left.len()], vec![0f32; self.right.len()]);
        let (mut sl, mut sr) = (0f32, 0f32);
        for i in 0..self.left.len() {
            let el = if i >= delays[0] { dl[i - delays[0]] } else { 0. };
            let er = if i >= delays[1] { dr[i - delays[1]] } else { 0. };
            sl += (er - sl) * 0.35;
            sr += (el - sr) * 0.35;
            dl[i] = self.left[i] + sl * feedback;
            dr[i] = self.right[i] + sr * feedback;
            self.left[i] += sl * wet;
            self.right[i] += sr * wet;
        }
    }
    /// Fold the tail onto the start, interleave and scale to `PEAK`.
    fn finish(self) -> Vec<f32> {
        let n = (LOOP_SECONDS * RATE as f32) as usize;
        let mut out = Vec::with_capacity(n * 2);
        for i in 0..n {
            let fold = |v: &Vec<f32>| v[i] + v.get(n + i).copied().unwrap_or(0.);
            out.push(fold(&self.left));
            out.push(fold(&self.right));
        }
        let peak = out.iter().fold(0f32, |m, x| m.max(x.abs())).max(1e-6);
        out.iter_mut().for_each(|x| *x *= PEAK / peak);
        out
    }
}

fn calm() -> Vec<f32> {
    let mut b = Buffer::new();
    const PAD: [f32; 5] = [1., 0.45, 0.25, 0.12, 0.06];
    for (c, (upper, bass)) in CHORDS.iter().enumerate() {
        let start = c as f32 * CHORD_SECONDS;
        for (v, note) in upper.iter().enumerate() {
            let pan = (v as f32 - 1.) * 0.4;
            for (detune, amp, side) in [(0., 0.045, 1.), (0.04, 0.03, -1.)] {
                b.tone(Tone {
                    start,
                    duration: CHORD_SECONDS,
                    attack: 2.2,
                    release: 2.5,
                    freq: midi(note + detune),
                    harmonics: &PAD,
                    amp,
                    pan: pan * side,
                    vibrato: 0.0012,
                    swell: (0.15, 8.),
                });
            }
        }
        b.tone(Tone {
            start,
            duration: CHORD_SECONDS,
            attack: 1.5,
            release: 2.,
            freq: midi(*bass),
            harmonics: &[1., 0.3, 0.1],
            amp: 0.07,
            pan: 0.,
            vibrato: 0.,
            swell: (0., 1.),
        });
    }
    // The cracked bell, far off: the tonic twice, the dominant once.
    b.bell(0.05, midi(62.), 0.06, -0.3);
    b.bell(16.05, midi(62.), 0.05, -0.3);
    b.bell(24.05, midi(57.), 0.04, 0.35);
    b.space(0.45);
    b.finish()
}

fn pressure() -> Vec<f32> {
    let mut b = Buffer::new();
    // Heartbeat: lub-dub on every beat.
    for beat in 0..LOOP_SECONDS as usize {
        let t = beat as f32;
        b.drum(t, 78., 44., 0.32, 14., 0.25, 0.);
        b.drum(t + 0.24, 70., 42., 0.2, 16., 0.2, 0.);
    }
    // Bowed eighth-note ostinato on root, fifth and octave.
    const BOW: [f32; 6] = [1., 0.5, 0.33, 0.25, 0.2, 0.16];
    for (c, (_, bass)) in CHORDS.iter().enumerate() {
        let root = bass + 12.;
        for n in 0..16 {
            let step = [0., 7., 12., 7.][n % 4];
            let accent = if n % 4 == 0 { 1.25 } else { 1. };
            b.tone(Tone {
                start: c as f32 * CHORD_SECONDS + n as f32 * 0.5,
                duration: 0.42,
                attack: 0.03,
                release: 0.12,
                freq: midi(root + step),
                harmonics: &BOW,
                amp: 0.05 * accent,
                pan: if n % 2 == 0 { -0.25 } else { 0.25 },
                vibrato: 0.003,
                swell: (0., 1.),
            });
        }
    }
    b.space(0.3);
    b.finish()
}

fn boss() -> Vec<f32> {
    let mut b = Buffer::new();
    const SAW: [f32; 4] = [1., 0.5, 0.33, 0.25];
    for (c, (upper, bass)) in CHORDS.iter().enumerate() {
        let start = c as f32 * CHORD_SECONDS;
        // Root and tritone, detuned against each other.
        for (step, pan) in [(0., -0.3), (6., 0.3), (0.06, 0.3), (6.07, -0.3)] {
            b.tone(Tone {
                start,
                duration: CHORD_SECONDS,
                attack: 1.,
                release: 1.5,
                freq: midi(bass + 12. + step),
                harmonics: &SAW,
                amp: 0.04,
                pan,
                vibrato: 0.,
                swell: (0.25, 8.),
            });
        }
        // A semitone cluster above the chord's top voice.
        for (step, pan) in [(12., -0.6), (13., 0.6)] {
            b.tone(Tone {
                start,
                duration: CHORD_SECONDS,
                attack: 3.,
                release: 2.,
                freq: midi(upper[2] + step - 12.),
                harmonics: &[1., 0.2],
                amp: 0.022,
                pan,
                vibrato: 0.004,
                swell: (0.3, 4.),
            });
        }
    }
    // War drums: one figure per bar.
    for bar in 0..(LOOP_SECONDS / 4.) as usize {
        for (beat, amp) in [(0., 0.3), (1.5, 0.22), (2., 0.28), (3., 0.3), (3.5, 0.22)] {
            let pan = if beat % 1. == 0. { -0.15 } else { 0.2 };
            b.drum(bar as f32 * 4. + beat, 120., 68., amp, 9., 0.35, pan);
        }
    }
    b.space(0.3);
    b.finish()
}

pub fn compose() -> Stems {
    Stems {
        layers: [calm(), pressure(), boss()],
    }
}

/// Target layer gains for the game's current state.
pub fn targets(g: &Game) -> [f32; 3] {
    match g.mode {
        Mode::Arena | Mode::Paused | Mode::LevelUp => {
            let living = g.run.enemies.iter().filter(|e| e.hp > 0.).count();
            let boss = g.run.enemies.iter().any(|e| e.kind == 3 && e.hp > 0.);
            let crowd = ((living as f32 - 2.) / 14.).clamp(0., 1.);
            let wounded = ((1. - g.run.hp / g.max_hp() - 0.5) / 0.4).clamp(0., 1.);
            let pressure = crowd.max(wounded).max(if boss { 0.6 } else { 0. });
            let calm = if boss { 0.25 } else { 0.6 - 0.3 * pressure };
            [calm, pressure, boss as u8 as f32]
        }
        Mode::Dead => [0.35, 0., 0.],
        _ => [1., 0., 0.],
    }
}

/// Gains shared between the game and the playing source.
#[derive(Default)]
pub struct Mix {
    layers: [AtomicU32; 3],
    master: AtomicU32,
}
impl Mix {
    pub fn set(&self, layers: [f32; 3], master: f32) {
        for (slot, gain) in self.layers.iter().zip(layers) {
            slot.store(gain.clamp(0., 1.).to_bits(), Ordering::Relaxed);
        }
        self.master
            .store(master.clamp(0., 2.).to_bits(), Ordering::Relaxed);
    }
    fn get(&self) -> ([f32; 3], f32) {
        (
            self.layers
                .each_ref()
                .map(|g| f32::from_bits(g.load(Ordering::Relaxed))),
            f32::from_bits(self.master.load(Ordering::Relaxed)),
        )
    }
}

/// All three layers, read from one position and mixed with smoothed gains.
/// Silent until the background render finishes.
pub struct Score {
    stems: Arc<OnceLock<Stems>>,
    mix: Arc<Mix>,
    position: usize,
    gains: [f32; 3],
    master: f32,
}
impl Score {
    pub fn new(stems: Arc<OnceLock<Stems>>, mix: Arc<Mix>) -> Self {
        Self {
            stems,
            mix,
            position: 0,
            gains: [0.; 3],
            master: 0.,
        }
    }
}
impl Iterator for Score {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let Some(stems) = self.stems.get() else {
            return Some(0.);
        };
        if self.position % 2 == 0 {
            let (targets, master) = self.mix.get();
            let fade = 1. - (-1. / (CROSSFADE * RATE as f32)).exp();
            let quick = 1. - (-1. / (0.1 * RATE as f32)).exp();
            for (gain, target) in self.gains.iter_mut().zip(targets) {
                *gain += (target - *gain) * fade;
            }
            self.master += (master - self.master) * quick;
        }
        let x: f32 = stems
            .layers
            .iter()
            .zip(self.gains)
            .map(|(layer, gain)| layer[self.position] * gain)
            .sum();
        self.position = (self.position + 1) % stems.layers[0].len();
        Some(x * self.master)
    }
}
impl Source for Score {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        RATE
    }
    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

/// The playing score, on a sink that lasts the whole session.
pub struct Music {
    mix: Arc<Mix>,
    _sink: rodio::Sink,
}
impl Music {
    pub fn new(sink: rodio::Sink) -> Self {
        let mix = Arc::new(Mix::default());
        let stems = Arc::new(OnceLock::new());
        let render = stems.clone();
        let render_score = move || {
            let start = web_time::Instant::now();
            let _ = render.set(compose());
            println!("Music rendered in {} ms", start.elapsed().as_millis());
        };
        // The browser has no threads; the score is composed while the page
        // is still loading.
        #[cfg(target_arch = "wasm32")]
        render_score();
        #[cfg(not(target_arch = "wasm32"))]
        let _ = std::thread::Builder::new().name("music".into()).spawn(render_score);
        sink.append(Score::new(stems, mix.clone()));
        Self { mix, _sink: sink }
    }
    pub fn set(&self, layers: [f32; 3], master: f32) {
        self.mix.set(layers, master);
    }
}

/// Write each layer and a 64-second adaptive mix (calm, a rising crowd, then
/// the boss) for listening and spectrograms.
pub fn export(directory: &str) -> std::io::Result<()> {
    let stems = compose();
    for (name, layer) in LAYERS.iter().zip(&stems.layers) {
        crate::audio::write_wav_at(&format!("{directory}/music-{name}.wav"), layer, RATE)?;
    }
    let cell = Arc::new(OnceLock::new());
    let _ = cell.set(stems);
    let mix = Arc::new(Mix::default());
    let mut score = Score::new(cell, mix.clone());
    let mut samples = vec![];
    for second in 0..64 {
        let s = second as f32;
        let layers = if s < 16. {
            [1., 0., 0.]
        } else if s < 40. {
            let pressure = ((s - 16.) / 16.).min(1.);
            [0.6 - 0.3 * pressure, pressure, 0.]
        } else {
            [0.25, 0.6, 1.]
        };
        mix.set(layers, 1.);
        samples.extend(score.by_ref().take(RATE as usize * 2));
    }
    crate::audio::write_wav_at(&format!("{directory}/music-adaptive-mix.wav"), &samples, RATE)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn stems() -> &'static Stems {
        static STEMS: OnceLock<Stems> = OnceLock::new();
        STEMS.get_or_init(compose)
    }
    #[test]
    fn layers_are_finite_bounded_audible_and_the_same_length() {
        let n = (LOOP_SECONDS * RATE as f32) as usize * 2;
        for (name, layer) in LAYERS.iter().zip(&stems().layers) {
            assert_eq!(layer.len(), n, "{name}");
            assert!(layer.iter().all(|x| x.is_finite() && x.abs() <= PEAK + 1e-4), "{name}");
            let rms = (layer.iter().map(|x| x * x).sum::<f32>() / n as f32).sqrt();
            assert!(rms > 0.02, "{name} rms {rms}");
            // Both channels carry sound, and they differ (stereo, not mono).
            assert!(layer.chunks(2).any(|f| (f[0] - f[1]).abs() > 0.01), "{name}");
        }
    }
    #[test]
    fn the_loop_seam_is_as_smooth_as_the_music_itself() {
        for (name, layer) in LAYERS.iter().zip(&stems().layers) {
            for channel in 0..2 {
                let samples: Vec<f32> = layer.iter().skip(channel).step_by(2).copied().collect();
                let steps: Vec<f32> = samples.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
                let mut sorted = steps.clone();
                sorted.sort_by(f32::total_cmp);
                let typical = sorted[sorted.len() * 99 / 100];
                let seam = (samples[0] - samples[samples.len() - 1]).abs();
                assert!(seam <= typical, "{name} channel {channel}: seam {seam} vs {typical}");
            }
        }
    }
    #[test]
    fn the_mix_follows_the_fight() {
        use crate::game::Enemy;
        use glam::Vec3;
        let mut g = Game::new(false);
        assert_eq!(targets(&g), [1., 0., 0.], "title");
        g.new_run();
        g.run.enemies.clear();
        let quiet = targets(&g);
        assert_eq!(quiet[1], 0.);
        assert!(quiet[0] > 0.5);
        g.run.enemies = (0..20)
            .map(|i| Enemy::spawn(0, Vec3::new(i as f32, 0., -8.), 1, 0.))
            .collect();
        let crowded = targets(&g);
        assert_eq!(crowded[1], 1.);
        assert!(crowded[0] < quiet[0]);
        g.run.enemies.truncate(1);
        g.run.hp = g.max_hp() * 0.15;
        assert!(targets(&g)[1] > 0.8, "low health raises the pressure");
        g.run.hp = g.max_hp();
        g.run.enemies = vec![Enemy::spawn(3, Vec3::new(0., 0., -8.), 4, 0.)];
        let boss = targets(&g);
        assert_eq!(boss[2], 1.);
        assert!(boss[1] >= 0.6);
        g.mode = Mode::Shop;
        assert_eq!(targets(&g), [1., 0., 0.], "the Collector is calm");
    }
    #[test]
    fn layers_crossfade_smoothly_and_stay_in_step() {
        let cell = Arc::new(OnceLock::new());
        let _ = cell.set(Stems {
            layers: [vec![0.25; 64], vec![0.5; 64], vec![1.; 64]],
        });
        let mix = Arc::new(Mix::default());
        let mut score = Score::new(cell, mix.clone());
        mix.set([1., 0., 0.], 1.);
        let settled: Vec<f32> = score.by_ref().take(RATE as usize * 30).collect();
        assert!((settled.last().unwrap() - 0.25).abs() < 0.01);
        mix.set([0., 1., 0.], 1.);
        let fade: Vec<f32> = score.by_ref().take(RATE as usize * 2 * 15).collect();
        // No jumps: each frame moves a tiny fraction of the way.
        assert!(fade.windows(2).all(|w| (w[1] - w[0]).abs() < 1e-3));
        let after_one_second = fade[RATE as usize * 2];
        assert!(after_one_second > 0.3 && after_one_second < 0.45, "{after_one_second}");
        assert!((fade.last().unwrap() - 0.5).abs() < 0.01);
    }
}
