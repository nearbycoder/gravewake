//! Original Blender-authored cemetery architecture and wind-bent spruce boughs.
//! Triangle streams are decoded once and baked into the static world buffer.
use crate::scene::{Mesh, Vertex};
use glam::{Mat3, Vec3};
use std::sync::OnceLock;

#[derive(Clone, Copy)]
pub enum Asset {
    Spruce0,
    Spruce1,
    Spruce2,
    SpruceNear,
    Grave0,
    Grave1,
    Grave2,
    Brazier,
}
struct AssetVertex {
    position: Vec3,
    normal: Vec3,
    color: [f32; 3],
    material: f32,
    uv: [f32; 2],
}
fn decode(bytes: &[u8]) -> Vec<AssetVertex> {
    assert_eq!(&bytes[..4], b"ENV1");
    let count = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    assert_eq!(bytes.len(), 8 + count * 48);
    bytes[8..]
        .chunks_exact(48)
        .map(|b| {
            let f = |i: usize| f32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap());
            AssetVertex {
                position: Vec3::new(f(0), f(1), f(2)),
                normal: Vec3::new(f(3), f(4), f(5)),
                color: [f(6), f(7), f(8)],
                material: f(9),
                uv: [f(10), f(11)],
            }
        })
        .collect()
}
fn assets() -> &'static [Vec<AssetVertex>; 8] {
    static ASSETS: OnceLock<[Vec<AssetVertex>; 8]> = OnceLock::new();
    ASSETS.get_or_init(|| {
        [
            decode(include_bytes!("../assets/environment/spruce-0.gwe")),
            decode(include_bytes!("../assets/environment/spruce-1.gwe")),
            decode(include_bytes!("../assets/environment/spruce-2.gwe")),
            decode(include_bytes!("../assets/environment/spruce-near.gwe")),
            decode(include_bytes!("../assets/environment/grave-0.gwe")),
            decode(include_bytes!("../assets/environment/grave-1.gwe")),
            decode(include_bytes!("../assets/environment/grave-2.gwe")),
            decode(include_bytes!("../assets/environment/brazier.gwe")),
        ]
    })
}
pub fn draw(mesh: &mut Mesh, asset: Asset) {
    // Inverse-transpose keeps the resized torch bowls and leaning headstones lit correctly.
    let normal_matrix = Mat3::from_mat4(mesh.transform).inverse().transpose();
    for v in &assets()[asset as usize] {
        mesh.vertices.push(Vertex {
            pos: mesh.transform.transform_point3(v.position).to_array(),
            normal: (normal_matrix * v.normal).normalize_or_zero().to_array(),
            color: v.color,
            material: v.material,
            local: [v.uv[0], v.uv[1], 0.],
            wear_uv: [v.uv[0] * 2.7, v.uv[1] * 2.7],
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn environment_assets_are_finite_and_fit_static_geometry_budget() {
        for (index, asset) in assets().iter().enumerate() {
            assert!(!asset.is_empty() && asset.len() % 3 == 0);
            assert!(
                asset.len() <= 30000,
                "asset {index} exceeds geometry budget"
            );
            for v in asset {
                assert!(v.position.is_finite() && v.normal.is_finite());
                assert!((v.normal.length() - 1.).abs() < 0.002);
                assert!(v.uv.iter().all(|x| x.is_finite()));
                assert!((0. ..=9.).contains(&v.material));
            }
        }
        // Distant silhouettes must be lighter than the close collision trees.
        for i in 0..3 {
            assert!(assets()[i].len() * 2 < assets()[3].len());
        }
    }
    #[test]
    fn nonuniform_placements_keep_unit_normals() {
        let mut mesh = Mesh::new();
        mesh.transform = glam::Mat4::from_scale(Vec3::new(1., 1.6 / 1.4, 1.));
        draw(&mut mesh, Asset::Brazier);
        assert!(
            mesh.vertices
                .iter()
                .all(|v| (Vec3::from_array(v.normal).length() - 1.).abs() < 0.002)
        );
        assert!(mesh.vertices.iter().any(|v| v.pos[1] > 1.6));
    }
}
