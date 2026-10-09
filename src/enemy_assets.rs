//! Original Blender-authored creatures, driven by the shared anatomical pose.
//! The same meshes render alive, in the bestiary, and as detached physics pieces.
use crate::{
    anatomy::{Anatomy, Part, Pose},
    dismemberment::{Section, SectionMesh},
    scene::{Mesh, Vertex},
};
use glam::{Mat4, Quat, Vec3};
use std::{ops::Range, sync::OnceLock};

pub const GROUPS: usize = 22;
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Instance {
    pub transforms: [[[f32; 4]; 4]; GROUPS],
    pub normals: [[[f32; 4]; 4]; GROUPS],
    pub params: [f32; 4],
}
struct AssetVertex {
    group: usize,
    vertex: Vertex,
}

fn decode(bytes: &[u8]) -> Vec<AssetVertex> {
    assert_eq!(&bytes[..4], b"GVM1");
    let count = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    assert_eq!(bytes.len(), 8 + count * 52);
    bytes[8..]
        .chunks_exact(52)
        .map(|b| {
            let f = |i: usize| f32::from_le_bytes(b[4 + i * 4..8 + i * 4].try_into().unwrap());
            let group = u32::from_le_bytes(b[..4].try_into().unwrap()) as usize;
            let normal = Vec3::new(f(3), f(4), f(5));
            let p = Vec3::new(f(0), f(1), f(2));
            let a = normal.abs();
            let uv = if a.y > a.x && a.y > a.z {
                [p.x, p.z]
            } else if a.x > a.z {
                [p.z, p.y]
            } else {
                [p.x, p.y]
            };
            AssetVertex {
                group,
                vertex: Vertex {
                    pos: p.to_array(),
                    normal: normal.to_array(),
                    color: [f(6), f(7), f(8)],
                    material: f(9),
                    local: [uv[0], uv[1], group as f32],
                    wear_uv: [f(10), f(11)],
                },
            }
        })
        .collect()
}
fn assets() -> &'static [Vec<AssetVertex>; 36] {
    static ASSETS: OnceLock<[Vec<AssetVertex>; 36]> = OnceLock::new();
    ASSETS.get_or_init(|| {
        [
            decode(asset_bytes!("../assets/enemies/ossuary-drudge.gvm")),
            decode(asset_bytes!("../assets/enemies/ribblade-skirmisher.gvm")),
            decode(asset_bytes!("../assets/enemies/cinder-skull.gvm")),
            decode(asset_bytes!("../assets/enemies/tithekeeper.gvm")),
            decode(asset_bytes!("../assets/enemies/gloamwing.gvm")),
            decode(asset_bytes!("../assets/enemies/grave-crawler.gvm")),
            decode(asset_bytes!("../assets/enemies/iron-penitent.gvm")),
            decode(asset_bytes!("../assets/enemies/plague-vessel.gvm")),
            decode(asset_bytes!("../assets/enemies/ash-cantor.gvm")),
            decode(asset_bytes!("../assets/enemies/bell-gargoyle.gvm")),
            decode(asset_bytes!("../assets/enemies/bone-shepherd.gvm")),
            decode(asset_bytes!("../assets/enemies/tithe-reaper.gvm")),
            decode(asset_bytes!("../assets/enemies/ossuary-drudge-lod1.gvm")),
            decode(asset_bytes!(
                "../assets/enemies/ribblade-skirmisher-lod1.gvm"
            )),
            decode(asset_bytes!("../assets/enemies/cinder-skull-lod1.gvm")),
            decode(asset_bytes!("../assets/enemies/tithekeeper-lod1.gvm")),
            decode(asset_bytes!("../assets/enemies/gloamwing-lod1.gvm")),
            decode(asset_bytes!("../assets/enemies/grave-crawler-lod1.gvm")),
            decode(asset_bytes!("../assets/enemies/iron-penitent-lod1.gvm")),
            decode(asset_bytes!("../assets/enemies/plague-vessel-lod1.gvm")),
            decode(asset_bytes!("../assets/enemies/ash-cantor-lod1.gvm")),
            decode(asset_bytes!("../assets/enemies/bell-gargoyle-lod1.gvm")),
            decode(asset_bytes!("../assets/enemies/bone-shepherd-lod1.gvm")),
            decode(asset_bytes!("../assets/enemies/tithe-reaper-lod1.gvm")),
            decode(asset_bytes!("../assets/enemies/ossuary-drudge-lod2.gvm")),
            decode(asset_bytes!(
                "../assets/enemies/ribblade-skirmisher-lod2.gvm"
            )),
            decode(asset_bytes!("../assets/enemies/cinder-skull-lod2.gvm")),
            decode(asset_bytes!("../assets/enemies/tithekeeper-lod2.gvm")),
            decode(asset_bytes!("../assets/enemies/gloamwing-lod2.gvm")),
            decode(asset_bytes!("../assets/enemies/grave-crawler-lod2.gvm")),
            decode(asset_bytes!("../assets/enemies/iron-penitent-lod2.gvm")),
            decode(asset_bytes!("../assets/enemies/plague-vessel-lod2.gvm")),
            decode(asset_bytes!("../assets/enemies/ash-cantor-lod2.gvm")),
            decode(asset_bytes!("../assets/enemies/bell-gargoyle-lod2.gvm")),
            decode(asset_bytes!("../assets/enemies/bone-shepherd-lod2.gvm")),
            decode(asset_bytes!("../assets/enemies/tithe-reaper-lod2.gvm")),
        ]
    })
}
pub fn gpu_geometry() -> (Vec<Vertex>, Vec<Range<u32>>) {
    let mut vertices = Vec::with_capacity(assets().iter().map(Vec::len).sum());
    let mut ranges = Vec::new();
    for asset in assets() {
        let start = vertices.len() as u32;
        vertices.extend(asset.iter().map(|v| v.vertex));
        ranges.push(start..vertices.len() as u32);
    }
    (vertices, ranges)
}
fn part_for_group(kind: usize, group: usize) -> Part {
    if crate::encounters::head_only(kind) {
        return Part::Head;
    }
    match group {
        1 => Part::Head,
        2..=4 => Part::LeftArm,
        5..=7 => Part::RightArm,
        8..=10 => Part::LeftLeg,
        11..=13 => Part::RightLeg,
        _ => Part::Torso,
    }
}
fn segment(a: Vec3, b: Vec3) -> Mat4 {
    let d = b - a;
    Mat4::from_scale_rotation_translation(
        Vec3::new(1., d.length().max(0.001), 1.),
        Quat::from_rotation_arc(Vec3::Y, d.normalize_or_zero()),
        a,
    )
}
fn hand(a: Vec3, b: Vec3) -> Mat4 {
    Mat4::from_rotation_translation(
        Quat::from_rotation_arc(Vec3::Y, (b - a).normalize_or_zero()),
        b,
    )
}
fn rig(pose: &Pose, kind: usize, phase: f32) -> [Mat4; GROUPS] {
    let mut t = [pose.root; GROUPS];
    t[1] = pose.head_root * Mat4::from_translation(Vec3::Y * pose.head_y);
    for i in 0..2 {
        let arm = pose.arms[i];
        let leg = pose.legs[i];
        let a = 2 + i * 3;
        let l = 8 + i * 3;
        t[a] = pose.root * segment(arm.root, arm.joint);
        t[a + 1] = pose.root * segment(arm.joint, arm.tip);
        t[a + 2] = pose.root * hand(arm.joint, arm.tip);
        t[l] = pose.root * segment(leg.root, leg.joint);
        t[l + 1] = pose.root * segment(leg.joint, leg.tip);
        t[l + 2] = pose.root * Mat4::from_translation(leg.tip);
        let side = if i == 0 { -1. } else { 1. };
        t[14 + i] = if kind == 4 { pose.head_root } else { pose.root }
            * Mat4::from_translation(Vec3::new(
                side * 0.12,
                if kind == 4 { pose.head_y - 0.06 } else { 1.48 },
                -0.08,
            ))
            * Mat4::from_rotation_z(side * (phase * 8.).sin() * 0.32);
        for j in 0..3 {
            let p = Vec3::new(side * 0.18, 0.75, -0.38 + j as f32 * 0.23);
            t[16 + i * 3 + j] = pose.root
                * Mat4::from_translation(p)
                * Mat4::from_rotation_z(side * (phase * 9. + j as f32 * 2. + side).sin() * 0.13)
                * Mat4::from_translation(-p);
        }
    }
    t
}

/// Extract the actual animated mesh into ten independently simulated sections.
/// Authored rig groups, including hands, feet, weapons, wings and crawler legs,
/// belong to exactly one section: there are no proxy skeletons or duplicate skin.
pub fn physical_sections(
    kind: usize,
    phase: f32,
    pose: &Pose,
    anatomy: &Anatomy,
) -> Vec<SectionMesh> {
    let kind = kind.min(11);
    let transforms = rig(pose, kind, phase);
    let normals = transforms.map(|matrix| matrix.inverse().transpose());
    let mut sections: Vec<_> = Section::ALL
        .into_iter()
        .filter(|section| {
            !anatomy.missing(section.part())
                && (!crate::encounters::head_only(kind) || *section == Section::Head)
        })
        .map(|section| {
            let segments = pose.segments(section.part());
            let (proximal, distal, _) = segments[section.segment()];
            SectionMesh {
                section,
                part: section.part(),
                center: (proximal + distal) * 0.5,
                proximal,
                distal,
                vertices: Vec::new(),
            }
        })
        .collect();
    for vertex in &assets()[kind] {
        let section = if crate::encounters::head_only(kind) {
            Section::Head
        } else {
            match vertex.group {
                1 => Section::Head,
                2 => Section::LeftUpperArm,
                3 | 4 => Section::LeftLowerArm,
                5 => Section::RightUpperArm,
                6 | 7 => Section::RightLowerArm,
                8 => Section::LeftUpperLeg,
                9 | 10 => Section::LeftLowerLeg,
                11 => Section::RightUpperLeg,
                12 | 13 => Section::RightLowerLeg,
                // Wing attachments on humanoids follow the chest. Flying skull
                // wings/crawler extras are already assigned to Head above.
                _ => Section::Torso,
            }
        };
        let Some(output) = sections.iter_mut().find(|piece| piece.section == section) else {
            continue;
        };
        let mut v = vertex.vertex;
        v.pos = transforms[vertex.group]
            .transform_point3(Vec3::from_array(v.pos))
            .to_array();
        v.normal = normals[vertex.group]
            .transform_vector3(Vec3::from_array(v.normal))
            .normalize_or_zero()
            .to_array();
        v.local[2] = 0.;
        output.vertices.push(v);
    }
    if let Some(torso) = sections
        .iter_mut()
        .find(|piece| piece.section == Section::Torso)
    {
        torso
            .vertices
            .extend(crate::dismemberment::stump_caps(pose, anatomy, kind));
    }
    sections.retain(|section| !section.vertices.is_empty());
    sections
}

pub fn draw(
    m: &mut Mesh,
    pose: &Pose,
    kind: usize,
    phase: f32,
    hit: f32,
    anatomy: &Anatomy,
    only: Option<Part>,
) {
    let kind = kind.min(11);
    let rig = rig(pose, kind, phase);
    let mut transforms = [Mat4::ZERO; GROUPS];
    let mut normals = [Mat4::IDENTITY; GROUPS];
    for i in 0..GROUPS {
        let part = part_for_group(kind, i);
        if only.map_or(!anatomy.missing(part), |selected| selected == part) {
            transforms[i] = m.transform * rig[i];
            normals[i] = transforms[i].inverse().transpose();
        }
    }
    if m.instance_spheres && only.is_none() {
        m.enemies.push(Instance {
            transforms: transforms.map(|t| t.to_cols_array_2d()),
            normals: normals.map(|t| t.to_cols_array_2d()),
            params: [kind as f32, 2., if hit > 0. { 1. } else { 0. }, 0.],
        });
        return;
    }
    for v in &assets()[kind] {
        let t = transforms[v.group];
        if t == Mat4::ZERO {
            continue;
        }
        let mut out = v.vertex;
        out.pos = t.transform_point3(Vec3::from_array(out.pos)).to_array();
        out.normal = normals[v.group]
            .transform_vector3(Vec3::from_array(out.normal))
            .normalize_or_zero()
            .to_array();
        if hit > 0. && out.material == 2. {
            out.color = [2.3, 1.8, 1.];
        }
        // local.z is a GPU rig index, never part of material noise coordinates.
        out.local[2] = 0.;
        m.vertices.push(out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rigid_sections_partition_every_posed_vertex_once() {
        for kind in 0..12 {
            let anatomy = Anatomy::default();
            let pose = Pose::new(
                Vec3::new(3., 0.4, -2.),
                0.71,
                0.39,
                kind,
                0.,
                1.,
                0.16,
                &anatomy,
            );
            let pieces = physical_sections(kind, 0.39, &pose, &anatomy);
            assert_eq!(
                pieces.len(),
                if crate::encounters::head_only(kind) {
                    1
                } else {
                    10
                }
            );
            let mut rendered = Mesh::new();
            draw(&mut rendered, &pose, kind, 0.39, 0., &anatomy, None);
            let mut actual: Vec<[u32; 15]> = pieces
                .iter()
                .flat_map(|s| s.vertices.iter().copied())
                .map(bytemuck::cast)
                .collect();
            let mut expected: Vec<[u32; 15]> =
                rendered.vertices.into_iter().map(bytemuck::cast).collect();
            actual.sort_unstable();
            expected.sort_unstable();
            assert_eq!(
                actual, expected,
                "posed mesh duplicated or lost for species {kind}"
            );
            for section in pieces {
                assert!(section.proximal.is_finite() && section.distal.is_finite());
                assert_eq!(section.part, section.section.part());
            }
        }
    }
    #[test]
    fn missing_limb_is_not_reintroduced_in_a_corpse_and_keeps_its_stump() {
        let mut anatomy = Anatomy::default();
        anatomy.severed = 1 << Part::LeftArm as usize;
        let pose = Pose::new(Vec3::ZERO, 0., 0., 0, 0., 0., 0., &anatomy);
        let sections = physical_sections(0, 0., &pose, &anatomy);
        assert_eq!(sections.len(), 8);
        assert!(sections.iter().all(|piece| piece.part != Part::LeftArm));
        let caps = crate::dismemberment::stump_caps(&pose, &anatomy, 0);
        let torso = sections.iter().find(|s| s.part == Part::Torso).unwrap();
        assert!(!caps.is_empty());
        let cap_bytes: Vec<[u32; 15]> = caps.into_iter().map(bytemuck::cast).collect();
        let tail: Vec<[u32; 15]> = torso.vertices[torso.vertices.len() - cap_bytes.len()..]
            .iter()
            .copied()
            .map(bytemuck::cast)
            .collect();
        assert_eq!(tail, cap_bytes);
    }
    #[test]
    fn every_authored_skull_can_fracture_without_invalid_geometry() {
        for kind in 0..12 {
            let anatomy = Anatomy::default();
            let pose = Pose::new(
                Vec3::new(2., 0.2, -3.),
                0.63,
                0.4,
                kind,
                0.,
                1.,
                0.,
                &anatomy,
            );
            let sections = physical_sections(kind, 0.4, &pose, &anatomy);
            let head = sections
                .iter()
                .find(|section| section.part == Part::Head)
                .unwrap();
            let chunks = crate::dismemberment::fracture(
                &head.vertices,
                head.center + Vec3::Z * 0.3,
                Vec3::new(0.3, 0.2, -1.),
                240.,
            );
            assert!(
                (2..=4).contains(&chunks.len()),
                "species {kind} remained whole"
            );
            assert!(
                chunks.iter().map(Vec::len).sum::<usize>()
                    <= crate::dismemberment::MAX_FRACTURE_VERTICES
            );
            for chunk in chunks {
                assert!(!chunk.is_empty() && chunk.len() % 3 == 0);
                assert!(chunk.iter().all(|v| Vec3::from_array(v.pos).is_finite()
                    && (Vec3::from_array(v.normal).length() - 1.).abs() < 0.002));
            }
        }
    }
    #[test]
    fn all_creatures_have_valid_geometry_and_rig_groups() {
        for (index, asset) in assets().iter().enumerate() {
            let kind = index % 12;
            assert!(
                asset.len() > 600 && asset.len() < 60000 && asset.len() % 3 == 0,
                "{kind}"
            );
            for v in asset {
                assert!(v.group < GROUPS);
                assert!(Vec3::from_array(v.vertex.pos).is_finite());
                assert!((Vec3::from_array(v.vertex.normal).length() - 1.).abs() < 0.001);
            }
            for part in Part::ALL {
                if crate::encounters::head_only(kind) && part != Part::Head {
                    continue;
                }
                assert!(
                    asset.iter().any(|v| part_for_group(kind, v.group) == part),
                    "kind {kind}, {part:?}"
                );
            }
        }
    }
    #[test]
    fn authored_parts_follow_shared_pose_and_sever_cleanly() {
        for kind in 0..12 {
            for phase in [0., 0.31, 0.77] {
                let mut anatomy = Anatomy::default();
                let pose = Pose::new(
                    Vec3::new(2., 0., -1.),
                    0.7,
                    phase,
                    kind,
                    0.,
                    1.,
                    0.21,
                    &anatomy,
                );
                let mut full = Mesh::new();
                draw(&mut full, &pose, kind, phase, 0., &anatomy, None);
                let mut head = Mesh::new();
                draw(
                    &mut head,
                    &pose,
                    kind,
                    phase,
                    0.,
                    &anatomy,
                    Some(Part::Head),
                );
                anatomy.severed = 1 << Part::Head as usize;
                let mut missing = Mesh::new();
                draw(&mut missing, &pose, kind, phase, 0., &anatomy, None);
                assert_eq!(
                    full.vertices.len(),
                    head.vertices.len() + missing.vertices.len()
                );
                assert!(
                    full.vertices
                        .iter()
                        .all(|v| Vec3::from_array(v.pos).is_finite()
                            && (Vec3::from_array(v.normal).length() - 1.).abs() < 0.001)
                );
                let mut gpu = Mesh::new();
                gpu.instance_spheres = true;
                draw(&mut gpu, &pose, kind, phase, 0., &anatomy, None);
                assert!(gpu.vertices.is_empty());
                assert_eq!(gpu.enemies.len(), 1);
                let instance = &gpu.enemies[0];
                let expected: Vec<_> = assets()[kind]
                    .iter()
                    .filter(|v| part_for_group(kind, v.group) != Part::Head)
                    .collect();
                for (v, cpu) in expected.iter().zip(&missing.vertices) {
                    let t = Mat4::from_cols_array_2d(&instance.transforms[v.group]);
                    assert!(
                        (t.transform_point3(Vec3::from_array(v.vertex.pos))
                            - Vec3::from_array(cpu.pos))
                        .length()
                            < 0.0001
                    );
                }
                assert_eq!(instance.transforms[1], Mat4::ZERO.to_cols_array_2d());
            }
        }
    }
}
