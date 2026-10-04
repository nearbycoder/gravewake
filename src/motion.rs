//! Shared, frame-rate-independent motion curves. Reload poses and sound cues use
//! the same normalized clock, including fire-rate upgrades.
use crate::game::WeaponKind;

pub fn smooth(a: f32, b: f32, t: f32) -> f32 {
    let x = ((t - a) / (b - a)).clamp(0., 1.);
    x * x * (3. - 2. * x)
}
pub fn out(t: f32) -> f32 {
    1. - (1. - t.clamp(0., 1.)).powi(3)
}
pub fn window(a: f32, b: f32, c: f32, d: f32, t: f32) -> f32 {
    smooth(a, b, t) * (1. - smooth(c, d, t))
}
#[derive(Default, Clone, Copy)]
pub struct ReloadPose {
    pub present: f32,
    pub hinge: f32,
    pub magazine: f32,
    pub rack: f32,
    pub left_hand: f32,
}
pub fn reload_pose(kind: WeaponKind, t: f32) -> ReloadPose {
    ReloadPose {
        present: window(0., 0.14, 0.86, 1., t),
        hinge: if kind.base() == WeaponKind::Double {
            window(0.10, 0.25, 0.77, 0.86, t)
        } else {
            0.
        },
        magazine: if kind.base() != WeaponKind::Double {
            window(0.18, 0.32, 0.52, 0.64, t)
        } else {
            0.
        },
        rack: if kind.base() != WeaponKind::Double {
            window(0.72, 0.77, 0.80, 0.84, t)
        } else {
            0.
        },
        left_hand: window(0.1, 0.25, 0.69, 0.83, t),
    }
}
pub fn reload_cues(kind: WeaponKind) -> &'static [(f32, &'static str)] {
    if kind.base() == WeaponKind::Double {
        &[
            (0.10, "breech_open"),
            (0.27, "shell_eject"),
            (0.48, "shell_insert"),
            (0.65, "shell_insert"),
            (0.82, "breech_close"),
        ]
    } else {
        &[
            (0.18, "mag_out"),
            (0.59, "mag_in"),
            (0.76, "rack_back"),
            (0.82, "rack_close"),
        ]
    }
}
/// Fast impulse, damped settling; recoil has its own clock, not muzzle flash.
pub fn recoil(age: f32) -> f32 {
    if age >= 0.6 {
        return 0.;
    }
    (1. - (-age * 150.).exp()) * (-age * 12.).exp() * (age * 21.).cos()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reload_opens_before_loading_and_locks_before_return() {
        let k = WeaponKind::Double;
        assert_eq!(reload_pose(k, 0.).hinge, 0.);
        assert_eq!(reload_pose(k, 0.48).hinge, 1.);
        assert_eq!(reload_pose(k, 0.65).hinge, 1.);
        assert_eq!(reload_pose(k, 0.9).hinge, 0.);
        assert_eq!(reload_pose(k, 1.).present, 0.);
        for kind in [k, WeaponKind::Pistol, WeaponKind::Repeater] {
            let cues = reload_cues(kind);
            assert!(cues.windows(2).all(|p| p[0].0 < p[1].0));
            assert!(cues.iter().all(|(t, _)| *t > 0. && *t < 1.));
        }
    }
}
