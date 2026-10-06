use rodio::{OutputStream, OutputStreamHandle, Sink, Source};
use std::{cell::Cell, collections::HashMap};
pub const RATE: u32 = 48000;
pub const EVENTS: &[&str] = &[
    "swing",
    "heavy_swing",
    "blade_hit",
    "impact",
    "bow",
    "shotgun",
    "shot",
    "spell",
    "coin",
    "soul",
    "level_up",
    "bone",
    "hurt",
    "kill",
    "dash",
    "deny",
    "cloth",
    "breech_open",
    "shell_eject",
    "shell_insert",
    "breech_close",
    "mag_out",
    "mag_in",
    "rack_back",
    "rack_close",
    "pack_tear",
    "card_deal",
    "card_flip",
    "card_take",
    "warn_cast",
    "warn_slam",
    "warn_dive",
    "warn_summon",
    "warn_blink",
];
pub struct Audio {
    stream: Option<(OutputStream, OutputStreamHandle)>,
    ambience: Option<Sink>,
    music: crate::music::Music,
    bank: HashMap<&'static str, Vec<Vec<f32>>>,
    variation: Cell<usize>,
}
impl Audio {
    pub fn new() -> Self {
        let stream = OutputStream::try_default().ok();
        let ambience = stream.as_ref().and_then(|(_, h)| Sink::try_new(h).ok());
        if let Some(s) = &ambience {
            let rate = 22050;
            let mut data = Vec::with_capacity(rate * 8);
            let mut seed = 92837u32;
            let mut wind = 0.;
            for i in 0..rate * 8 {
                let t = i as f32 / rate as f32;
                let noise = random(&mut seed);
                wind = wind * 0.995 + noise * 0.005;
                let drone = (t * 55. * std::f32::consts::TAU).sin() * 0.022
                    + (t * 82.5 * std::f32::consts::TAU).sin() * 0.012;
                data.push(drone + wind * 0.3);
            }
            s.append(rodio::buffer::SamplesBuffer::new(1, rate as u32, data).repeat_infinite());
            s.set_volume(0.3);
        }
        let bank = EVENTS
            .iter()
            .map(|&e| (e, (0..4).map(|v| synthesize(e, v)).collect()))
            .collect();
        let music = crate::music::Music::new(stream.as_ref().map(|(_, h)| h));
        Self {
            stream,
            ambience,
            music,
            bank,
            variation: Cell::new(0),
        }
    }
    pub fn volume(&self, v: f32) {
        if let Some(s) = &self.ambience {
            s.set_volume(v * 0.3);
        }
    }
    /// Steer the score: layer gains (calm, pressure, boss) and overall level.
    pub fn music(&self, layers: [f32; 3], master: f32) {
        self.music.set(layers, master);
    }
    pub fn play(&self, event: &str, volume: f32) {
        let Some((_, handle)) = &self.stream else {
            return;
        };
        let Some(variants) = self.bank.get(event) else {
            return;
        };
        let v = self.variation.get();
        self.variation.set(v.wrapping_add(1));
        let source =
            rodio::buffer::SamplesBuffer::new(2, RATE, variants[v % variants.len()].clone())
                .amplify(volume);
        let _ = handle.play_raw(source);
    }
    /// Play a world sound with per-channel gains from `spatial`.
    pub fn play_at(&self, event: &str, volume: f32, (left, right): (f32, f32)) {
        let Some((_, handle)) = &self.stream else {
            return;
        };
        let Some(variants) = self.bank.get(event) else {
            return;
        };
        let v = self.variation.get();
        self.variation.set(v.wrapping_add(1));
        let mut samples = variants[v % variants.len()].clone();
        for frame in samples.chunks_exact_mut(2) {
            frame[0] *= left;
            frame[1] *= right;
        }
        let source = rodio::buffer::SamplesBuffer::new(2, RATE, samples).amplify(volume);
        let _ = handle.play_raw(source);
    }
}
/// Left/right gains for a sound at `source` heard from `listener` facing
/// `yaw`: equal-power panning by relative bearing, distance attenuation, and
/// slight softening behind the listener. A source straight ahead at close
/// range plays at the same level as an unpanned sound.
pub fn spatial(listener: glam::Vec3, yaw: f32, source: glam::Vec3) -> (f32, f32) {
    let flat = (source - listener) * glam::Vec3::new(1., 0., 1.);
    let distance = flat.length();
    let gain = (1.4 / (1. + distance / 5.)).clamp(0.12, 1.);
    if distance < 0.05 {
        return (gain, gain);
    }
    let relative = flat.x.atan2(-flat.z) - yaw;
    let (side, front) = relative.sin_cos();
    let behind = 1. - 0.25 * (-front).max(0.);
    let angle = (side + 1.) * std::f32::consts::FRAC_PI_4;
    let level = gain * behind * std::f32::consts::SQRT_2;
    ((angle.cos() * level).min(1.), (angle.sin() * level).min(1.))
}
fn random(seed: &mut u32) -> f32 {
    *seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
    (*seed as f32 / u32::MAX as f32) * 2. - 1.
}
fn decay(t: f32, onset: f32, speed: f32) -> f32 {
    if t < onset {
        0.
    } else {
        (-(t - onset) * speed).exp()
    }
}
/// Recorded CC0 firearm transients, with restrained procedural sweetening.
/// See assets/audio/SOURCES.md for source licenses and reproducible edits.
pub fn synthesize(event: &str, variant: u32) -> Vec<f32> {
    let bytes: Option<&[u8]> = match event {
        "shotgun" if variant % 2 == 0 => Some(include_bytes!("../assets/audio/shotgun-a.wav")),
        "shotgun" => Some(include_bytes!("../assets/audio/shotgun-b.wav")),
        "shot" => Some(include_bytes!("../assets/audio/pistol.wav")),
        "shell_insert" => Some(include_bytes!("../assets/audio/shell-in.wav")),
        "breech_close" | "rack_close" => Some(include_bytes!("../assets/audio/rack.wav")),
        _ => None,
    };
    let Some(bytes) = bytes else {
        return designed(event, variant);
    };
    let decoder = rodio::Decoder::new(std::io::Cursor::new(bytes)).expect("embedded PCM recording");
    assert_eq!(decoder.sample_rate(), RATE);
    assert_eq!(decoder.channels(), 2);
    let source: Vec<f32> = decoder.convert_samples().collect();
    let pitch = 0.985 + variant as f32 * 0.01;
    let frames = (source.len() as f32 / 2. / pitch) as usize;
    let sweetener = designed(event, variant);
    let mut output = Vec::with_capacity(frames * 2);
    for i in 0..frames {
        let p = i as f32 * pitch;
        let j = p as usize;
        for ch in 0..2 {
            let a = source.get(j * 2 + ch).copied().unwrap_or(0.);
            let b = source.get((j + 1) * 2 + ch).copied().unwrap_or(0.);
            let dry = a + (b - a) * p.fract();
            let layer = sweetener.get(i * 2 + ch).copied().unwrap_or(0.);
            output.push((dry * 0.94 + layer * 0.075).clamp(-0.96, 0.96));
        }
    }
    output
}
/// Designed foil, card and mechanism layers with stereo outdoor reflections.
fn designed(event: &str, variant: u32) -> Vec<f32> {
    let gun = event == "shotgun" || event == "shot";
    let duration = if gun {
        1.15
    } else if event == "pack_tear" {
        1.05
    } else {
        0.5
    };
    let n = (duration * RATE as f32) as usize;
    let mut dry = vec![0.; n];
    let mut seed = 7717u32.wrapping_add(variant * 17291);
    let mut low = 0.;
    let mut mid = 0.;
    let mut previous = 0.;
    let pitch = 0.97 + variant as f32 * 0.02;
    let tau = std::f32::consts::TAU;
    for (i, sample) in dry.iter_mut().enumerate() {
        let t = i as f32 / RATE as f32;
        let noise = random(&mut seed);
        low += (noise - low) * 0.028;
        mid += (noise - mid) * 0.24;
        let high = noise - previous;
        previous = noise;
        let x = if gun {
            let shotgun = event == "shotgun";
            let body_hz = if shotgun { 66. } else { 116. };
            let pressure = (tau * pitch * (body_hz * t + 1.8 * (1. - (-t * 38.).exp()))).sin();
            let crack = high * 0.58 * decay(t, 0., 195.)
                + noise * 0.8 * decay(t, 0.003, 85.)
                + mid * 0.48 * decay(t, 0.013, 58.);
            let body = pressure * if shotgun { 0.7 } else { 0.36 } * decay(t, 0., 23.)
                + low * if shotgun { 2.8 } else { 1.4 } * decay(t, 0., 13.);
            let tail = mid * 0.38 * decay(t, 0.025, 11.);
            let action = high * 0.1 * decay(t, 0.07, 110.);
            // Mild saturation glues the pressure layers without hard clipping.
            ((crack + body + tail + action) * 1.8).tanh() * 0.83
        } else if ["swing", "heavy_swing", "bow"].contains(&event) {
            let span = if event == "heavy_swing" { 0.4 } else { 0.23 };
            mid * crate::motion::window(0., span * 0.4, span * 0.55, span, t) * 0.48
        } else if event == "impact" || event == "blade_hit" {
            (low * 2. + mid * 0.5 + (tau * 73. * t).sin() * 0.25)
                * decay(t, 0., if event == "impact" { 13. } else { 34. })
        } else if event == "pack_tear" {
            let pull = crate::motion::window(0., 0.09, 0.16, 0.24, t) * 0.10;
            let rip = crate::motion::window(0.18, 0.23, 0.70, 0.79, t);
            let grains = (t * 137.).sin().abs() * 0.65 + 0.35;
            let crinkle = high * grains * rip * 0.23 + mid * pull;
            crinkle + noise * 0.17 * decay(t, 0.76, 60.)
        } else if ["card_flip", "card_deal", "card_take", "cloth", "dash"].contains(&event) {
            let speed = if event == "card_take" { 0.32 } else { 0.20 };
            let whoosh = crate::motion::window(0., speed * 0.35, speed * 0.5, speed, t);
            mid * whoosh * 0.15 + high * 0.08 * decay(t, speed * 0.87, 170.)
        } else if event == "spell" {
            ((tau * (125. * t + 950. * t * t)).sin() * 0.18 + mid * 0.25) * decay(t, 0., 8.)
        } else if event == "hurt" {
            (low * 1.7 + (tau * 47. * t).sin() * 0.24) * decay(t, 0., 16.)
        } else if event == "warn_cast" {
            // A rising, shimmering intake before a bolt is loosed.
            let rise = (tau * (240. * t + 520. * t * t)).sin() * 0.16
                + (tau * (480. * t + 1040. * t * t)).sin() * 0.06;
            (rise + high * 0.05) * crate::motion::window(0., 0.25, 0.38, 0.47, t)
        } else if event == "warn_slam" {
            // A low swelling growl before a slam or plague burst.
            ((tau * (46. + 18. * t) * t).sin() * 0.32 + low * 2.4 + mid * 0.12)
                * crate::motion::window(0., 0.3, 0.38, 0.48, t)
        } else if event == "warn_dive" {
            // A descending screech from a diving flyer.
            ((tau * (1500. * t - 1100. * t * t)).sin() * 0.12 + high * 0.05)
                * crate::motion::window(0., 0.05, 0.3, 0.45, t)
        } else if event == "warn_summon" {
            // Two hollow bell partials as a shepherd calls the dead.
            ((tau * 330. * t).sin() * 0.16 + (tau * 494. * t).sin() * 0.1) * decay(t, 0., 6.)
                + low * 0.4 * decay(t, 0., 10.)
        } else if event == "warn_blink" {
            // A reversed whoosh that snaps shut as the reaper vanishes.
            mid * 0.5 * crate::motion::window(0., 0.32, 0.36, 0.4, t)
                + high * 0.2 * decay(t, 0.36, 90.)
        } else if event == "deny" {
            (tau * 100. * t).sin() * decay(t, 0., 18.) * 0.14
        } else {
            // Multiple inharmonic modes and impacts make metal sound mechanical,
            // instead of the old single oscillator / generic noise beep.
            let (f, weight, offsets): (f32, f32, &[f32]) = match event {
                "breech_open" => (670., 0.30, &[0., 0.055]),
                "breech_close" => (310., 0.52, &[0., 0.009, 0.028]),
                "shell_eject" => (1900., 0.23, &[0., 0.032, 0.10]),
                "shell_insert" => (920., 0.28, &[0., 0.025]),
                "mag_out" => (770., 0.24, &[0., 0.05]),
                "mag_in" => (410., 0.34, &[0., 0.014]),
                "rack_back" => (850., 0.25, &[0., 0.021, 0.054]),
                "rack_close" => (480., 0.39, &[0., 0.012]),
                "soul" => (1300., 0.12, &[0., 0.035]),
                "level_up" => (660., 0.3, &[0., 0.08, 0.16, 0.24]),
                "coin" => (2400., 0.18, &[0., 0.045, 0.09]),
                "bone" => (680., 0.18, &[0., 0.019, 0.043, 0.11]),
                "kill" => (1650., 0.13, &[0., 0.03]),
                _ => (600., 0.1, &[0.]),
            };
            let mut y = 0.;
            for onset in offsets {
                let a = t - onset;
                if a >= 0. {
                    let ring = (tau * f * pitch * a).sin() * 0.3
                        + (tau * f * 1.47 * a).sin() * 0.18
                        + (tau * f * 2.13 * a).sin() * 0.11;
                    y += weight
                        * (ring * (-a * 65.).exp()
                            + high * (-a * 220.).exp()
                            + low * 1.5 * (-a * 38.).exp());
                }
            }
            y
        };
        let attack = (t * 18000.).min(1.);
        let end = ((duration - t) * 60.).min(1.);
        *sample = x * attack * end;
    }
    let mut stereo = Vec::with_capacity(n * 2);
    for i in 0..n {
        for channel in 0..2 {
            let mut x = dry[i];
            if gun {
                for (delay, gain) in [
                    (0.047, 0.14),
                    (0.089, 0.10),
                    (0.151, 0.07),
                    (0.233, 0.045),
                    (0.341, 0.025),
                ] {
                    let lag = ((delay + channel as f32 * 0.007) * RATE as f32) as usize;
                    if i >= lag {
                        x += dry[i - lag] * gain;
                    }
                }
            }
            stereo.push(x.clamp(-0.96, 0.96));
        }
    }
    stereo
}
pub fn write_wav(path: &str, samples: &[f32]) -> std::io::Result<()> {
    write_wav_at(path, samples, RATE)
}
/// Write interleaved stereo samples as 16-bit PCM at `rate`.
pub fn write_wav_at(path: &str, samples: &[f32], rate: u32) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::fs::File::create(path)?;
    let bytes = (samples.len() * 2) as u32;
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + bytes).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?;
    f.write_all(&2u16.to_le_bytes())?;
    f.write_all(&rate.to_le_bytes())?;
    f.write_all(&(rate * 4).to_le_bytes())?;
    f.write_all(&4u16.to_le_bytes())?;
    f.write_all(&16u16.to_le_bytes())?;
    f.write_all(b"data")?;
    f.write_all(&bytes.to_le_bytes())?;
    for s in samples {
        f.write_all(&((s.clamp(-1., 1.) * 32767.) as i16).to_le_bytes())?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sound_bank_is_finite_bounded_and_has_a_quiet_tail() {
        for e in EVENTS {
            let s = synthesize(e, 0);
            assert!(s.iter().all(|x| x.is_finite() && x.abs() <= 0.96), "{e}");
            assert!(s.iter().any(|x| x.abs() > 0.01), "{e}");
            assert!(s[s.len() - 200..].iter().all(|x| x.abs() < 0.02), "{e}");
        }
        assert_ne!(synthesize("shotgun", 0), synthesize("shotgun", 1));
        assert_ne!(synthesize("shotgun", 0), synthesize("shot", 0));
    }
    #[test]
    fn world_sounds_pan_by_bearing_and_fade_with_distance() {
        use glam::Vec3;
        let ear = Vec3::new(0., 1.65, 0.);
        let close = |x: f32, z: f32| spatial(ear, 0., Vec3::new(x, 1., z));
        // Facing -Z: ahead is centred at the unpanned level.
        let (l, r) = close(0., -2.);
        assert!((l - r).abs() < 1e-5 && (l - 1.).abs() < 1e-5, "{l} {r}");
        let (l, r) = close(2., 0.);
        assert!(r > 0.95 && l < 0.05, "right {l} {r}");
        let (l, r) = close(-2., 0.);
        assert!(l > 0.95 && r < 0.05, "left {l} {r}");
        let (l, r) = close(0., 2.);
        assert!(
            (l - r).abs() < 1e-5 && l < 0.8,
            "behind is centred and softer {l}"
        );
        // Turning toward a source centres it.
        let (l, r) = spatial(ear, std::f32::consts::FRAC_PI_2, Vec3::new(2., 1., 0.));
        assert!((l - r).abs() < 1e-5);
        let near = close(0., -3.).0;
        let mid = close(0., -15.).0;
        let far = close(0., -60.).0;
        assert!(near > mid && mid > far && far >= 0.12 * std::f32::consts::FRAC_1_SQRT_2 * 1.4);
        assert_eq!(spatial(ear, 0., ear), (1., 1.));
    }
}
