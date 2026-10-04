//! Original Blender-authored, open Gothic architecture for the expanded grounds.
//! Positions are baked once into the static world mesh. ENV1 stores artist normals
//! and local stone UVs, so repeated/scaled modules retain stable material mapping.
use crate::scene::{Mesh, Vertex};
use glam::{Mat3, Vec3};
use std::sync::OnceLock;

#[derive(Clone, Copy)]
pub enum Asset {
    ChapelFacade,
    ChapelWall,
    CloisterBay,
    BellTower,
    Fountain,
    RuinWall,
    Rubble,
    Urn,
    Obelisk,
    CurtainWall,
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

fn assets() -> &'static [Vec<AssetVertex>; 10] {
    static ASSETS: OnceLock<[Vec<AssetVertex>; 10]> = OnceLock::new();
    ASSETS.get_or_init(|| {
        [
            decode(include_bytes!("../assets/architecture/chapel-facade.gwe")),
            decode(include_bytes!("../assets/architecture/chapel-wall.gwe")),
            decode(include_bytes!("../assets/architecture/cloister-bay.gwe")),
            decode(include_bytes!("../assets/architecture/bell-tower.gwe")),
            decode(include_bytes!(
                "../assets/architecture/memorial-fountain.gwe"
            )),
            decode(include_bytes!("../assets/architecture/ruin-wall.gwe")),
            decode(include_bytes!("../assets/architecture/rubble.gwe")),
            decode(include_bytes!("../assets/architecture/funerary-urn.gwe")),
            decode(include_bytes!(
                "../assets/architecture/cemetery-obelisk.gwe"
            )),
            decode(include_bytes!("../assets/architecture/curtain-wall.gwe")),
        ]
    })
}

pub fn draw(mesh: &mut Mesh, asset: Asset) {
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
    fn landmarks_have_finite_normals_and_bounded_geometry() {
        for (index, asset) in assets().iter().enumerate() {
            assert!(!asset.is_empty() && asset.len() % 3 == 0);
            assert!(
                asset.len() < 60000,
                "landmark {index} exceeds 20k triangles"
            );
            for v in asset {
                assert!(v.position.is_finite() && v.normal.is_finite());
                assert!((v.normal.length() - 1.).abs() < 0.002);
                assert!(v.uv.iter().all(|x| x.is_finite()));
                assert!((0. ..=9.).contains(&v.material));
            }
        }
    }

    #[test]
    fn chapel_and_cloister_ground_passages_remain_clear() {
        // This checks real exported geometry, guarding the walkable openings
        // against later Blender edits that accidentally fill the doorway.
        for (asset, clear_half_width) in [(Asset::ChapelFacade, 2.96), (Asset::CloisterBay, 2.5)] {
            for triangle in assets()[asset as usize].chunks_exact(3) {
                if triangle.iter().all(|v| v.position.y > 2.2) {
                    continue;
                }
                let low = triangle
                    .iter()
                    .map(|v| v.position.x)
                    .fold(f32::INFINITY, f32::min);
                let high = triangle
                    .iter()
                    .map(|v| v.position.x)
                    .fold(f32::NEG_INFINITY, f32::max);
                assert!(
                    high <= -clear_half_width || low >= clear_half_width,
                    "geometry intrudes into passage"
                );
            }
        }
    }

    #[test]
    fn resized_masonry_retains_unit_normals() {
        let mut mesh = Mesh::new();
        mesh.transform = glam::Mat4::from_scale(Vec3::new(0.3, 0.8, 1.));
        draw(&mut mesh, Asset::ChapelWall);
        assert!(
            mesh.vertices
                .iter()
                .all(|v| (Vec3::from_array(v.normal).length() - 1.).abs() < 0.002)
        );
    }
}
