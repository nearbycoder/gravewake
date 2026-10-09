//! Graphics fidelity: one setting with four steps, from Low for a weak GPU
//! to Ultra. High is the renderer as it was before the setting existed, and
//! the default. Each step is a `Profile`: how large the 3D scene is drawn,
//! how much work the Hollowlight composite does, how many braziers light a
//! surface, and how dense the fire and creature detail are.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fidelity {
    Low,
    Medium,
    #[default]
    High,
    Ultra,
}

/// What one fidelity step draws.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Profile {
    /// The 3D scene's width relative to the window's, then held between
    /// these widths in pixels. The interface always draws at full size.
    pub scale: f32,
    pub min_width: u32,
    pub max_width: u32,
    /// Braziers that light a surface (nearest first), at most `MAX_LIGHTS`.
    pub lights: u32,
    /// Depth-occlusion samples per pixel: 0 (off), 4, 8 or 16 (two rings).
    pub occlusion_taps: u32,
    /// Steps through the ground mist along each view ray.
    pub mist_steps: u32,
    /// Bloom: 1 is one 3×3 lobe, 3 is three 3×3 lobes, 5 is three 5×5 lobes.
    pub bloom: u32,
    /// Braziers (nearest first) whose light is tested for screen-space shadows.
    pub shadow_lights: u32,
    /// Blend high-contrast edges with their neighbours.
    pub edge_smoothing: bool,
    /// Anisotropic filtering of the material atlas (1 is off).
    pub anisotropy: u16,
    /// Flame tongues and rising sparks per brazier.
    pub tongues: u32,
    pub sparks: u32,
    /// Creatures switch to their reduced Blender meshes beyond these distances.
    pub detail_near: f32,
    pub detail_far: f32,
}

/// The most brazier lights the shaders read.
pub const MAX_LIGHTS: usize = 8;

impl Fidelity {
    pub const ALL: [Fidelity; 4] = [
        Fidelity::Low,
        Fidelity::Medium,
        Fidelity::High,
        Fidelity::Ultra,
    ];
    /// A new player's step: High on the desktop, Medium in the browser,
    /// where the same GPU usually has less to spare.
    pub const DEFAULT: Fidelity = if cfg!(target_arch = "wasm32") {
        Fidelity::Medium
    } else {
        Fidelity::High
    };
    /// The step a new player starts at: `DEFAULT`, or Low in the browser on
    /// a phone or tablet (`crate::lite`).
    pub fn for_new_player() -> Fidelity {
        if crate::lite() {
            Fidelity::Low
        } else {
            Self::DEFAULT
        }
    }
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|f| *f == self).unwrap()
    }
    pub fn name(self) -> &'static str {
        match self {
            Fidelity::Low => "LOW",
            Fidelity::Medium => "MEDIUM",
            Fidelity::High => "HIGH",
            Fidelity::Ultra => "ULTRA",
        }
    }
    /// One line for the journal: what this step changes.
    pub fn summary(self) -> &'static str {
        match self {
            Fidelity::Low => "60% scene resolution, four lights, no occlusion, lighter mist and bloom",
            Fidelity::Medium => "80% scene resolution, lighter occlusion and mist",
            Fidelity::High => "Full scene resolution up to 1440 wide, the standard Hollowlight composite",
            Fidelity::Ultra => {
                "Supersampled 2×, brazier shadows, eight lights, finer occlusion, bloom and texture filtering"
            }
        }
    }
    pub fn profile(self) -> Profile {
        match self {
            Fidelity::Low => Profile {
                scale: 0.6,
                min_width: 480,
                max_width: 960,
                lights: 4,
                occlusion_taps: 0,
                mist_steps: 4,
                bloom: 1,
                shadow_lights: 0,
                edge_smoothing: false,
                anisotropy: 1,
                tongues: 3,
                sparks: 2,
                detail_near: 5.,
                detail_far: 10.,
            },
            Fidelity::Medium => Profile {
                scale: 0.8,
                min_width: 560,
                max_width: 1200,
                lights: 6,
                occlusion_taps: 4,
                mist_steps: 6,
                bloom: 3,
                shadow_lights: 0,
                edge_smoothing: true,
                anisotropy: 1,
                tongues: 4,
                sparks: 3,
                detail_near: 6.5,
                detail_far: 12.,
            },
            Fidelity::High => Profile {
                scale: 1.,
                min_width: 640,
                max_width: 1440,
                lights: 6,
                occlusion_taps: 8,
                mist_steps: 8,
                bloom: 3,
                shadow_lights: 0,
                edge_smoothing: true,
                anisotropy: 1,
                tongues: 6,
                sparks: 5,
                detail_near: 8.,
                detail_far: 14.,
            },
            Fidelity::Ultra => Profile {
                scale: 2.,
                min_width: 640,
                max_width: 3840,
                lights: 8,
                occlusion_taps: 16,
                mist_steps: 12,
                bloom: 5,
                shadow_lights: 3,
                edge_smoothing: true,
                anisotropy: 16,
                tongues: 9,
                sparks: 10,
                detail_near: 14.,
                detail_far: 24.,
            },
        }
    }
}

impl Profile {
    /// The 3D scene's size in pixels for a window (surface) of this size,
    /// keeping its shape.
    pub fn scene_size(&self, width: u32, height: u32) -> (u32, u32) {
        let width = width.max(1);
        let w = ((width as f32 * self.scale).round() as u32).clamp(self.min_width, self.max_width);
        let h = (w as f32 * height.max(1) as f32 / width as f32) as u32;
        (w, h.max(1))
    }
    /// How much larger this step's scene is than High's for the same
    /// window, so composite radii given in High's pixels cover the same part
    /// of the screen at every step.
    pub fn radius_scale(&self, width: u32, height: u32) -> f32 {
        let high = Fidelity::High.profile().scene_size(width, height).0;
        self.scene_size(width, height).0 as f32 / high as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_is_the_renderer_as_it_was() {
        let high = Fidelity::High.profile();
        // Before the setting: width clamped to 640..1440, six lights, eight
        // occlusion taps and mist steps, three 3×3 bloom lobes, no shadows.
        for (w, h) in [(1440, 900), (960, 600), (2560, 1440), (3840, 2160), (500, 400)] {
            let width = w.clamp(640, 1440);
            let height = (width as f32 * h as f32 / w as f32) as u32;
            assert_eq!(high.scene_size(w, h), (width, height), "{w}×{h}");
            assert_eq!(high.radius_scale(w, h), 1.);
        }
        assert_eq!(
            (high.lights, high.occlusion_taps, high.mist_steps, high.bloom, high.shadow_lights),
            (6, 8, 8, 3, 0)
        );
        assert!(high.edge_smoothing);
        assert_eq!(high.anisotropy, 1);
        assert_eq!((high.tongues, high.sparks), (6, 5));
        assert_eq!((high.detail_near, high.detail_far), (8., 14.));
        assert_eq!(Fidelity::default(), Fidelity::High);
    }

    #[test]
    fn each_step_does_more_than_the_one_below() {
        for pair in Fidelity::ALL.windows(2) {
            let (a, b) = (pair[0].profile(), pair[1].profile());
            for (w, h) in [(960, 600), (1440, 900), (1920, 1080), (3840, 2160)] {
                assert!(a.scene_size(w, h).0 <= b.scene_size(w, h).0, "{pair:?} at {w}×{h}");
            }
            assert!(a.lights <= b.lights && a.occlusion_taps <= b.occlusion_taps);
            assert!(a.mist_steps < b.mist_steps || a.bloom < b.bloom);
            assert!(a.tongues < b.tongues && a.sparks < b.sparks);
            assert!(a.detail_far < b.detail_far);
        }
        let ultra = Fidelity::Ultra.profile();
        assert!(ultra.lights as usize <= MAX_LIGHTS);
        assert!(Fidelity::ALL.iter().all(|f| f.profile().lights <= 8));
    }

    #[test]
    fn scene_sizes_at_common_windows() {
        let size = |f: Fidelity, w, h| f.profile().scene_size(w, h);
        assert_eq!(size(Fidelity::Low, 1440, 900), (864, 540));
        assert_eq!(size(Fidelity::Medium, 1440, 900), (1152, 720));
        assert_eq!(size(Fidelity::Ultra, 1440, 900), (2880, 1800));
        // The smallest window keeps a usable scene; 4K stays within limits.
        assert_eq!(size(Fidelity::Low, 960, 600), (576, 360));
        assert_eq!(size(Fidelity::Ultra, 3840, 2160), (3840, 2160));
        assert_eq!(size(Fidelity::Low, 3840, 2160), (960, 540));
        // Composite radii follow the scene, so effects keep their size on screen.
        let low = Fidelity::Low.profile().radius_scale(1440, 900);
        assert!((low - 0.6).abs() < 1e-6);
        assert_eq!(Fidelity::Ultra.profile().radius_scale(1440, 900), 2.);
    }
}
