//! Blender-authored assemblies, embedded in the native executable.
//! UVs and mechanical groups survive reload animation; Blender is authoring-only.
use crate::{
    scene::{Mesh, Vertex},
    weapons::WeaponKind,
};
use glam::{Mat4, Vec3};
use std::sync::OnceLock;
struct AssetVertex {
    group: usize,
    position: Vec3,
    normal: Vec3,
    material: f32,
    uv: [f32; 2],
}
fn decode(bytes: &[u8]) -> Vec<AssetVertex> {
    assert_eq!(&bytes[..4], b"DVM1");
    let count = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    assert_eq!(bytes.len(), 8 + count * 40);
    bytes[8..]
        .chunks_exact(40)
        .map(|b| {
            let f = |i: usize| f32::from_le_bytes(b[4 + i * 4..8 + i * 4].try_into().unwrap());
            AssetVertex {
                group: u32::from_le_bytes(b[..4].try_into().unwrap()) as usize,
                position: Vec3::new(f(0), f(1), f(2)),
                normal: Vec3::new(f(3), f(4), f(5)),
                material: f(6),
                uv: [f(7), f(8)],
            }
        })
        .collect()
}
fn assets() -> &'static [Vec<AssetVertex>; 7] {
    static ASSETS: OnceLock<[Vec<AssetVertex>; 7]> = OnceLock::new();
    ASSETS.get_or_init(|| {
        [
            decode(asset_bytes!("../assets/weapons/iron-pistol.dvm")),
            decode(asset_bytes!("../assets/weapons/double-barrel.dvm")),
            decode(asset_bytes!("../assets/weapons/cleaver.dvm")),
            decode(asset_bytes!("../assets/weapons/grave-revolver.dvm")),
            decode(asset_bytes!("../assets/weapons/slug-pump.dvm")),
            decode(asset_bytes!("../assets/weapons/repeater.dvm")),
            decode(asset_bytes!("../assets/weapons/longrifle.dvm")),
        ]
    })
}
pub fn draw(m: &mut Mesh, kind: WeaponKind, rarity: usize, reload: Option<f32>, age: f32) -> bool {
    let index = match kind {
        WeaponKind::Pistol => 0,
        WeaponKind::Double => 1,
        WeaponKind::Cleaver => 2,
        WeaponKind::GraveRevolver => 3,
        WeaponKind::SlugGun => 4,
        WeaponKind::Repeater => 5,
        WeaponKind::Longrifle => 6,
        _ => return false,
    };
    // Pump actions keep their fixed barrel; the pump and bolt cycle independently.
    let mechanism = if kind == WeaponKind::SlugGun {
        WeaponKind::Pistol
    } else {
        kind
    };
    let pose = crate::motion::reload_pose(mechanism, reload.unwrap_or(0.));
    let hinge = Mat4::from_translation(Vec3::new(0., -0.02, 0.04))
        * Mat4::from_rotation_x(-pose.hinge * 0.92)
        * Mat4::from_translation(Vec3::new(0., 0.02, -0.04));
    let rack = pose.rack * 0.13 + crate::motion::window(0., 0.025, 0.05, 0.11, age) * 0.1;
    let pump = (pose.rack + crate::motion::window(0.06, 0.13, 0.24, 0.34, age)) * 0.14;
    let cylinder = Mat4::from_translation(Vec3::new(-pose.magazine * 0.19, 0.027, 0.))
        * Mat4::from_rotation_z(
            crate::motion::window(0., 0.03, 0.11, 0.20, age) * std::f32::consts::PI / 3.,
        )
        * Mat4::from_translation(Vec3::new(0., -0.027, 0.));
    let lever = Mat4::from_translation(Vec3::new(0., -0.075, 0.12))
        * Mat4::from_rotation_x(
            (pose.magazine * 0.40 + crate::motion::window(0.05, 0.13, 0.25, 0.36, age) * 0.65)
                .min(0.8),
        )
        * Mat4::from_translation(Vec3::new(0., 0.075, -0.12));
    let transforms = [
        m.transform,
        m.transform * hinge,
        m.transform * Mat4::from_translation(Vec3::Z * rack),
        m.transform * Mat4::from_translation(-Vec3::Y * pose.magazine * 0.43),
        m.transform
            * Mat4::from_translation(Vec3::new(0., 0.10, 0.14))
            * Mat4::from_rotation_x(-crate::motion::window(0., 0.02, 0.07, 0.16, age) * 0.3)
            * Mat4::from_translation(Vec3::new(0., -0.10, -0.14)),
        m.transform * cylinder,
        m.transform * Mat4::from_translation(Vec3::Z * pump),
        m.transform * lever,
    ];
    for v in &assets()[index] {
        let transform = transforms[v.group];
        m.vertices.push(Vertex {
            pos: transform.transform_point3(v.position).to_array(),
            normal: transform
                .transform_vector3(v.normal)
                .normalize_or_zero()
                .to_array(),
            color: [0.25; 3],
            material: v.material,
            local: v.position.to_array(),
            wear_uv: v.uv,
        });
    }
    // Rarity remains a small inset jewel, not a fresh coat of polished metal.
    if rarity > 0 {
        let tint = match rarity {
            1 => [0.07, 0.55, 0.75],
            2 => [0.9, 0.2, 0.03],
            _ => [0.6, 0.06, 0.9],
        };
        if index != 2 {
            for side in [-1., 1.] {
                crate::guns::bevel(
                    m,
                    Vec3::new(
                        side * if index == 0 {
                            0.072
                        } else if index == 3 {
                            0.056
                        } else {
                            0.091
                        },
                        if index == 3 { -0.172 } else { 0.014 },
                        if index == 3 { 0.172 } else { 0.026 },
                    ),
                    Vec3::new(0.006, 0.023, 0.032),
                    0.004,
                    tint,
                    9.,
                );
            }
        } else {
            m.ellipsoid(Vec3::new(0., -0.31, 0.), Vec3::splat(0.028), tint, 9.);
        }
    }
    true
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_weapon_uses_weathered_surfaces() {
        for kind in WeaponKind::ALL {
            let mut mesh = Mesh::new();
            crate::scene::weapon(&mut mesh, kind, 0);
            let weathered: Vec<_> = mesh
                .vertices
                .iter()
                .filter(|v| (11. ..=14.).contains(&v.material))
                .collect();
            assert!(weathered.len() > 100, "{kind:?} has no weathered surface");
            assert!(
                weathered
                    .iter()
                    .all(|v| v.wear_uv.iter().all(|x| x.is_finite())),
                "{kind:?} UVs"
            );
            let (lo, hi) = weathered
                .iter()
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), v| {
                    (lo.min(v.wear_uv[0]), hi.max(v.wear_uv[0]))
                });
            assert!(hi - lo > 0.01, "{kind:?} texture collapsed to one texel");
        }
    }
    #[test]
    fn blender_meshes_have_valid_uvs_normals_and_articulation() {
        for (i, asset) in assets().iter().enumerate() {
            assert!(asset.len() > 1000 && asset.len() < 30000 && asset.len() % 3 == 0);
            for v in asset {
                assert!(v.position.is_finite() && v.normal.is_finite());
                assert!((v.normal.length() - 1.).abs() < 0.001);
                assert!(v.uv.iter().all(|x| x.is_finite()));
                assert!(v.group < 8);
                assert!((11. ..=14.).contains(&v.material));
            }
            if i < 2 {
                assert!(asset.iter().any(|v| v.group == if i == 0 { 3 } else { 1 }));
            }
        }
        for kind in [
            WeaponKind::Pistol,
            WeaponKind::Double,
            WeaponKind::GraveRevolver,
            WeaponKind::SlugGun,
            WeaponKind::Repeater,
            WeaponKind::Longrifle,
        ] {
            let mut rest = Mesh::new();
            let mut reload = Mesh::new();
            draw(&mut rest, kind, 0, None, 10.);
            // Some mechanisms move during loading, others during the final bolt rack.
            let phase = if matches!(kind, WeaponKind::SlugGun | WeaponKind::Longrifle) {
                0.78
            } else {
                0.5
            };
            draw(&mut reload, kind, 0, Some(phase), 10.);
            assert_eq!(rest.vertices.len(), reload.vertices.len());
            assert!(
                rest.vertices
                    .iter()
                    .zip(&reload.vertices)
                    .any(|(a, b)| a.pos != b.pos),
                "{kind:?} did not articulate"
            );
            assert!(
                rest.vertices
                    .iter()
                    .zip(&reload.vertices)
                    .all(|(a, b)| a.wear_uv == b.wear_uv),
                "{kind:?} texture slipped during reload"
            );
        }
    }
}
