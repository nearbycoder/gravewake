//! Bounded runtime cuts of the authored, already posed creature mesh.
//! Triangle clipping preserves the skin and material coordinates. Intersections
//! are joined into separate contours, capped, and handed to rigid-body physics.
use crate::{
    anatomy::{Anatomy, Part, Pose},
    scene::Vertex,
};
use glam::{Vec2, Vec3};
use std::collections::{HashMap, HashSet};

const EPSILON: f32 = 0.00001;
const JOIN: f32 = 0.0001;
pub const MAX_FRACTURE_VERTICES: usize = 120_000;
const BONE: [f32; 3] = [0.64, 0.58, 0.40];
const MARROW: [f32; 3] = [0.16, 0.055, 0.029];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Torso,
    Head,
    LeftUpperArm,
    LeftLowerArm,
    RightUpperArm,
    RightLowerArm,
    LeftUpperLeg,
    LeftLowerLeg,
    RightUpperLeg,
    RightLowerLeg,
}
impl Section {
    pub const ALL: [Self; 10] = [
        Self::Torso,
        Self::Head,
        Self::LeftUpperArm,
        Self::LeftLowerArm,
        Self::RightUpperArm,
        Self::RightLowerArm,
        Self::LeftUpperLeg,
        Self::LeftLowerLeg,
        Self::RightUpperLeg,
        Self::RightLowerLeg,
    ];
    pub fn part(self) -> Part {
        match self {
            Self::Torso => Part::Torso,
            Self::Head => Part::Head,
            Self::LeftUpperArm | Self::LeftLowerArm => Part::LeftArm,
            Self::RightUpperArm | Self::RightLowerArm => Part::RightArm,
            Self::LeftUpperLeg | Self::LeftLowerLeg => Part::LeftLeg,
            Self::RightUpperLeg | Self::RightLowerLeg => Part::RightLeg,
        }
    }
    pub fn segment(self) -> usize {
        usize::from(matches!(
            self,
            Self::LeftLowerArm | Self::RightLowerArm | Self::LeftLowerLeg | Self::RightLowerLeg
        ))
    }
}

pub struct SectionMesh {
    pub section: Section,
    pub part: Part,
    pub center: Vec3,
    pub proximal: Vec3,
    pub distal: Vec3,
    /// Exact posed vertices in world space. Physics converts them to body space.
    pub vertices: Vec<Vertex>,
}

fn position(vertex: &Vertex) -> Vec3 {
    Vec3::from_array(vertex.pos)
}
fn bounds(vertices: &[Vertex]) -> (Vec3, Vec3) {
    vertices.iter().fold(
        (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
        |(low, high), vertex| (low.min(position(vertex)), high.max(position(vertex))),
    )
}
fn push_triangle(vertices: &mut Vec<Vertex>, a: Vertex, b: Vertex, c: Vertex) {
    if (position(&b) - position(&a))
        .cross(position(&c) - position(&a))
        .length_squared()
        > 1e-14
    {
        vertices.extend_from_slice(&[a, b, c]);
    }
}
fn interpolate(a: Vertex, b: Vertex, t: f32) -> Vertex {
    let mix = |x: f32, y: f32| x + (y - x) * t;
    let n = Vec3::from_array(a.normal)
        .lerp(Vec3::from_array(b.normal), t)
        .normalize_or_zero();
    let n = if n.length_squared() > 0.5 {
        n
    } else {
        Vec3::from_array(a.normal)
    };
    Vertex {
        pos: position(&a).lerp(position(&b), t).to_array(),
        normal: n.to_array(),
        color: std::array::from_fn(|i| mix(a.color[i], b.color[i])),
        material: a.material,
        local: std::array::from_fn(|i| mix(a.local[i], b.local[i])),
        wear_uv: std::array::from_fn(|i| mix(a.wear_uv[i], b.wear_uv[i])),
    }
}
fn clip_triangle(
    triangle: &[Vertex],
    origin: Vec3,
    normal: Vec3,
    positive: bool,
    out: &mut Vec<Vertex>,
) {
    let mut polygon = Vec::with_capacity(4);
    for i in 0..3 {
        let a = triangle[i];
        let b = triangle[(i + 1) % 3];
        let da = (position(&a) - origin).dot(normal);
        let db = (position(&b) - origin).dot(normal);
        let inside_a = if positive { da >= 0. } else { da <= 0. };
        let inside_b = if positive { db >= 0. } else { db <= 0. };
        if inside_a {
            polygon.push(a);
        }
        if inside_a != inside_b {
            polygon.push(interpolate(a, b, (da / (da - db)).clamp(0., 1.)));
        }
    }
    for i in 1..polygon.len().saturating_sub(1) {
        push_triangle(out, polygon[0], polygon[i], polygon[i + 1]);
    }
}
fn basis(normal: Vec3) -> (Vec3, Vec3) {
    let u = normal
        .cross(if normal.y.abs() < 0.9 {
            Vec3::Y
        } else {
            Vec3::X
        })
        .normalize();
    (u, normal.cross(u))
}
fn projected(p: Vec3, u: Vec3, v: Vec3) -> Vec2 {
    Vec2::new(p.dot(u), p.dot(v))
}
fn cross(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

/// Ear clipping retains concave cut outlines instead of filling a bounding box
/// or drawing a fan across empty space between separate ribs.
fn triangulate(points: &[Vec2]) -> Vec<[usize; 3]> {
    let mut indices: Vec<_> = (0..points.len()).collect();
    let mut triangles = vec![];
    let mut remaining = points.len() * points.len();
    while indices.len() > 3 && remaining > 0 {
        remaining -= 1;
        let mut ear = None;
        for i in 0..indices.len() {
            let a = indices[(i + indices.len() - 1) % indices.len()];
            let b = indices[i];
            let c = indices[(i + 1) % indices.len()];
            if cross(points[b] - points[a], points[c] - points[b]) <= 1e-10 {
                continue;
            }
            let contains = indices.iter().copied().any(|p| {
                p != a
                    && p != b
                    && p != c
                    && cross(points[b] - points[a], points[p] - points[a]) > 1e-9
                    && cross(points[c] - points[b], points[p] - points[b]) > 1e-9
                    && cross(points[a] - points[c], points[p] - points[c]) > 1e-9
            });
            if !contains {
                ear = Some((i, [a, b, c]));
                break;
            }
        }
        if let Some((at, triangle)) = ear {
            triangles.push(triangle);
            indices.remove(at);
        } else {
            break;
        }
    }
    if indices.len() == 3 {
        triangles.push([indices[0], indices[1], indices[2]]);
    }
    triangles
}
fn cap_vertex(p: Vec3, normal: Vec3, u: Vec3, v: Vec3, color: [f32; 3], material: f32) -> Vertex {
    let uv = [p.dot(u), p.dot(v)];
    Vertex {
        pos: p.to_array(),
        normal: normal.to_array(),
        color,
        material,
        local: [uv[0], uv[1], 0.],
        wear_uv: [uv[0] * 2.7, uv[1] * 2.7],
    }
}
fn cap_triangle(
    out: &mut Vec<Vertex>,
    points: [Vec3; 3],
    normal: Vec3,
    color: [f32; 3],
    material: f32,
) {
    let (u, v) = basis(normal);
    let [a, mut b, mut c] = points;
    if (b - a).cross(c - a).dot(normal) < 0. {
        std::mem::swap(&mut b, &mut c);
    }
    push_triangle(
        out,
        cap_vertex(a, normal, u, v, color, material),
        cap_vertex(b, normal, u, v, color, material),
        cap_vertex(c, normal, u, v, color, material),
    );
}
fn append_cap(out: &mut Vec<Vertex>, contour: &[Vec3], normal: Vec3) {
    let (u, v) = basis(normal);
    let mut points = contour.to_vec();
    // Drop duplicate and collinear intersection vertices from triangulated faces.
    points.dedup_by(|a, b| a.distance_squared(*b) < JOIN * JOIN);
    if points.len() < 3 {
        return;
    }
    for _ in 0..2 {
        if points.len() < 4 {
            break;
        }
        let keep: Vec<_> = (0..points.len())
            .filter(|&i| {
                let a = points[(i + points.len() - 1) % points.len()];
                let b = points[i];
                let c = points[(i + 1) % points.len()];
                (b - a).cross(c - b).length_squared() > 1e-13
            })
            .map(|i| points[i])
            .collect();
        if keep.len() >= 3 {
            points = keep;
        }
    }
    let mut plane: Vec<_> = points.iter().map(|&p| projected(p, u, v)).collect();
    let area: f32 = (0..plane.len())
        .map(|i| cross(plane[i], plane[(i + 1) % plane.len()]))
        .sum();
    if area.abs() < 1e-9 {
        return;
    }
    if area < 0. {
        points.reverse();
        plane.reverse();
    }
    let convex = (0..plane.len()).all(|i| {
        cross(
            plane[(i + 1) % plane.len()] - plane[i],
            plane[(i + 2) % plane.len()] - plane[(i + 1) % plane.len()],
        ) >= -1e-9
    });
    if convex {
        let center = points.iter().copied().sum::<Vec3>() / points.len() as f32;
        for i in 0..points.len() {
            let a = points[i];
            let b = points[(i + 1) % points.len()];
            let ia = center.lerp(a, 0.67);
            let ib = center.lerp(b, 0.67);
            cap_triangle(out, [a, b, ib], normal, BONE, 2.);
            cap_triangle(out, [a, ib, ia], normal, BONE, 2.);
            cap_triangle(out, [center, ia, ib], normal, MARROW, 0.);
        }
    } else {
        for [a, b, c] in triangulate(&plane) {
            cap_triangle(out, [points[a], points[b], points[c]], normal, BONE, 2.);
        }
    }
}

fn contours(segments: &[(Vec3, Vec3)]) -> Vec<Vec<Vec3>> {
    let mut points: Vec<Vec3> = vec![];
    let mut buckets: HashMap<[i32; 3], Vec<usize>> = HashMap::new();
    let mut edges = vec![];
    let mut unique = HashSet::new();
    let mut node = |p: Vec3| {
        let key = p.to_array().map(|x| (x / JOIN).floor() as i32);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(ids) = buckets.get(&[key[0] + dx, key[1] + dy, key[2] + dz]) {
                        if let Some(&id) = ids
                            .iter()
                            .find(|&&id| points[id].distance_squared(p) < JOIN * JOIN)
                        {
                            return id;
                        }
                    }
                }
            }
        }
        let id = points.len();
        points.push(p);
        buckets.entry(key).or_default().push(id);
        id
    };
    for &(a, b) in segments {
        let a = node(a);
        let b = node(b);
        if a != b && unique.insert((a.min(b), a.max(b))) {
            edges.push((a, b));
        }
    }
    let mut adjacency = vec![vec![]; points.len()];
    for (i, &(a, b)) in edges.iter().enumerate() {
        adjacency[a].push(i);
        adjacency[b].push(i);
    }
    let mut used = vec![false; edges.len()];
    let mut result = vec![];
    for start_edge in 0..edges.len() {
        if used[start_edge] {
            continue;
        }
        let (start, mut current) = edges[start_edge];
        let mut previous = start;
        used[start_edge] = true;
        let mut path = vec![points[start], points[current]];
        while current != start && path.len() <= edges.len() + 1 {
            let incoming = (points[current] - points[previous]).normalize_or_zero();
            let next = adjacency[current]
                .iter()
                .copied()
                .filter(|&edge| !used[edge])
                .max_by(|&a, &b| {
                    let other = |edge: usize| {
                        let (x, y) = edges[edge];
                        if x == current { y } else { x }
                    };
                    incoming
                        .dot((points[other(a)] - points[current]).normalize_or_zero())
                        .total_cmp(
                            &incoming.dot((points[other(b)] - points[current]).normalize_or_zero()),
                        )
                });
            let Some(edge) = next else {
                break;
            };
            used[edge] = true;
            let (a, b) = edges[edge];
            previous = current;
            current = if a == current { b } else { a };
            path.push(points[current]);
        }
        if current == start {
            path.pop();
        }
        if path.len() >= 3 {
            result.push(path);
        }
    }
    result
}

fn split_once(
    vertices: &[Vertex],
    origin: Vec3,
    normal: Vec3,
) -> Option<(Vec<Vertex>, Vec<Vertex>)> {
    let mut positive = Vec::with_capacity(vertices.len() / 2);
    let mut negative = Vec::with_capacity(vertices.len() / 2);
    let mut cuts = vec![];
    for triangle in vertices.chunks_exact(3) {
        let distances: [f32; 3] =
            std::array::from_fn(|i| (position(&triangle[i]) - origin).dot(normal));
        let low = distances.into_iter().fold(f32::INFINITY, f32::min);
        let high = distances.into_iter().fold(f32::NEG_INFINITY, f32::max);
        if low >= -EPSILON {
            positive.extend_from_slice(triangle);
            continue;
        }
        if high <= EPSILON {
            negative.extend_from_slice(triangle);
            continue;
        }
        clip_triangle(triangle, origin, normal, true, &mut positive);
        clip_triangle(triangle, origin, normal, false, &mut negative);
        let mut crossings: Vec<Vec3> = vec![];
        for i in 0..3 {
            let j = (i + 1) % 3;
            let a = distances[i];
            let b = distances[j];
            let point = if a.abs() <= EPSILON {
                Some(position(&triangle[i]))
            } else if (a < 0.) != (b < 0.) {
                Some(position(&triangle[i]).lerp(position(&triangle[j]), a / (a - b)))
            } else {
                None
            };
            if let Some(p) = point {
                if crossings
                    .iter()
                    .all(|q| q.distance_squared(p) > JOIN * JOIN)
                {
                    crossings.push(p);
                }
            }
        }
        if crossings.len() == 2 {
            cuts.push((crossings[0], crossings[1]));
        }
    }
    if positive.len() < 12 || negative.len() < 12 {
        return None;
    }
    for contour in contours(&cuts) {
        append_cap(&mut positive, &contour, -normal);
        append_cap(&mut negative, &contour, normal);
    }
    if positive.len() + negative.len() > MAX_FRACTURE_VERTICES {
        return None;
    }
    Some((positive, negative))
}

/// Slice the posed mesh into at most four substantial physical fragments.
/// The impact plane is moved just inside the hit surface, retaining its direction;
/// a tangent bullet hit must not silently leave the entire skull intact.
pub fn fracture(
    vertices: &[Vertex],
    impact: Vec3,
    direction: Vec3,
    energy: f32,
) -> Vec<Vec<Vertex>> {
    if vertices.len() < 24
        || vertices.len() > MAX_FRACTURE_VERTICES
        || !impact.is_finite()
        || !direction.is_finite()
    {
        return vec![vertices.to_vec()];
    }
    let normal = if direction.length_squared() > 1e-8 {
        direction.normalize()
    } else {
        Vec3::Y
    };
    let (low, high) = bounds(vertices);
    let center = (low + high) * 0.5;
    let mut projections: Vec<_> = vertices.iter().map(|v| position(v).dot(normal)).collect();
    projections.sort_by(f32::total_cmp);
    let a = projections[projections.len() * 28 / 100];
    let b = projections[projections.len() * 72 / 100];
    let distance = (impact.dot(normal) * 0.35 + center.dot(normal) * 0.65).clamp(a, b);
    let origin = center + normal * (distance - center.dot(normal));
    let Some((a, b)) = split_once(vertices, origin, normal) else {
        return vec![vertices.to_vec()];
    };
    let mut fragments = vec![a, b];
    let desired = if energy >= 220. {
        4
    } else if energy >= 120. {
        3
    } else {
        2
    };
    let (u, v) = basis(normal);
    for i in 0..desired - 2 {
        let Some((index, _)) = fragments
            .iter()
            .enumerate()
            .max_by_key(|(_, mesh)| mesh.len())
        else {
            break;
        };
        let (low, high) = bounds(&fragments[index]);
        let center = (low + high) * 0.5;
        let cut = (normal * 0.23 + if i == 0 { u } else { v }).normalize();
        let Some((a, b)) = split_once(&fragments[index], center, cut) else {
            break;
        };
        let total = fragments.iter().map(Vec::len).sum::<usize>() - fragments[index].len()
            + a.len()
            + b.len();
        if total > MAX_FRACTURE_VERTICES {
            break;
        }
        fragments[index] = a;
        fragments.push(b);
    }
    fragments
}

/// A visible bone rim and marrow core at a severed attachment. Both the living
/// stump and the detached limb use the same pose anchor and opposite cap normals.
pub fn sever_cap(pose: &Pose, part: Part, detached: bool) -> Vec<Vertex> {
    let (center, axis, radius) = match part {
        Part::Head => (
            pose.head_root
                .transform_point3(Vec3::new(0., pose.head_y - 0.18, 0.)),
            pose.head_root.transform_vector3(Vec3::Y).normalize(),
            0.095 * pose.scale,
        ),
        Part::LeftArm | Part::RightArm | Part::LeftLeg | Part::RightLeg => {
            let (a, b, r) = pose.segments(part)[0];
            (a, (b - a).normalize_or_zero(), r * 0.91)
        }
        Part::Torso => return vec![],
    };
    let normal = if detached { -axis } else { axis };
    let (u, v) = basis(normal);
    let center = center + normal * 0.004 * pose.scale;
    let outline: Vec<_> = (0..12)
        .map(|i| {
            let angle = i as f32 * std::f32::consts::TAU / 12.;
            center + (u * angle.cos() + v * angle.sin()) * radius
        })
        .collect();
    let mut vertices = vec![];
    append_cap(&mut vertices, &outline, normal);
    vertices
}
pub fn stump_caps(pose: &Pose, anatomy: &Anatomy, kind: usize) -> Vec<Vertex> {
    if crate::encounters::head_only(kind) {
        return vec![];
    }
    Part::ALL
        .into_iter()
        .filter(|&part| part != Part::Torso && anatomy.missing(part))
        .flat_map(|part| sever_cap(pose, part, false))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn cube() -> Vec<Vertex> {
        let mut mesh = crate::scene::Mesh::new();
        mesh.cube(Vec3::ZERO, Vec3::splat(2.), [0.7, 0.65, 0.4], 2.);
        mesh.vertices
    }
    fn volume(vertices: &[Vertex]) -> f32 {
        vertices
            .chunks_exact(3)
            .map(|t| position(&t[0]).dot(position(&t[1]).cross(position(&t[2]))) / 6.)
            .sum::<f32>()
            .abs()
    }
    fn open_surface_flux(vertices: &[Vertex]) -> f32 {
        vertices
            .chunks_exact(3)
            .map(|triangle| {
                (position(&triangle[1]) - position(&triangle[0]))
                    .cross(position(&triangle[2]) - position(&triangle[0]))
                    * 0.5
            })
            .sum::<Vec3>()
            .length()
    }
    #[test]
    fn planar_cuts_close_the_surface_and_preserve_volume() {
        let cube = cube();
        let original = volume(&cube);
        for normal in [Vec3::X, Vec3::new(0.7, 0.5, -0.2).normalize()] {
            let fragments = fracture(&cube, normal * 1.1, normal, 80.);
            assert_eq!(fragments.len(), 2);
            assert!((fragments.iter().map(|m| volume(m)).sum::<f32>() - original).abs() < 0.0001);
            for fragment in &fragments {
                assert!(fragment.iter().any(|v| v.color == MARROW));
                assert!(
                    open_surface_flux(fragment) < 0.0001,
                    "fracture has an uncapped boundary"
                );
                assert!(fragment.iter().all(|v| position(v).is_finite()
                    && (Vec3::from_array(v.normal).length() - 1.).abs() < 0.001));
            }
            if normal == Vec3::X {
                assert!((volume(&fragments[0]) - (1. - 0.385) * 4.).abs() < 0.0001);
                assert!((volume(&fragments[1]) - (1. + 0.385) * 4.).abs() < 0.0001);
            }
        }
    }
    #[test]
    fn high_energy_fracture_is_bounded_and_keeps_all_material_attributes() {
        let cube = cube();
        let fragments = fracture(&cube, Vec3::new(1., 0.3, 0.2), Vec3::X, 300.);
        assert_eq!(fragments.len(), 4);
        assert!(fragments.iter().map(Vec::len).sum::<usize>() < MAX_FRACTURE_VERTICES);
        assert!((fragments.iter().map(|m| volume(m)).sum::<f32>() - volume(&cube)).abs() < 0.0001);
        for v in fragments.iter().flatten() {
            assert!(
                v.local
                    .iter()
                    .chain(v.wear_uv.iter())
                    .all(|x| x.is_finite())
            );
            assert!((Vec3::from_array(v.normal).length() - 1.).abs() < 0.001);
        }
        assert!(
            fragments
                .iter()
                .all(|fragment| open_surface_flux(fragment) < 0.0001)
        );
    }
    #[test]
    fn concave_cross_sections_are_capped_without_filling_the_notch() {
        let outline = [
            Vec2::new(-1., -1.),
            Vec2::new(1., -1.),
            Vec2::new(1., 0.),
            Vec2::ZERO,
            Vec2::new(0., 1.),
            Vec2::new(-1., 1.),
        ];
        let mut mesh = crate::scene::Mesh::new();
        for z in [-1., 1.] {
            for [a, b, c] in [[0, 1, 3], [1, 2, 3], [0, 3, 5], [3, 4, 5]] {
                let p = |i: usize| outline[i].extend(z);
                if z > 0. {
                    mesh.triangle(p(a), p(b), p(c), BONE, 2.);
                } else {
                    mesh.triangle(p(a), p(c), p(b), BONE, 2.);
                }
            }
        }
        for i in 0..outline.len() {
            let a = outline[i];
            let b = outline[(i + 1) % outline.len()];
            mesh.quad(
                [a.extend(-1.), b.extend(-1.), b.extend(1.), a.extend(1.)],
                BONE,
                2.,
            );
        }
        let fragments = fracture(&mesh.vertices, Vec3::Z, Vec3::Z, 80.);
        assert_eq!(fragments.len(), 2);
        assert!(
            fragments
                .iter()
                .all(|fragment| open_surface_flux(fragment) < 0.0001)
        );
        assert!(
            (fragments
                .iter()
                .map(|fragment| volume(fragment))
                .sum::<f32>()
                - 6.)
                .abs()
                < 0.0001
        );
        for triangle in fragments.iter().flat_map(|mesh| mesh.chunks_exact(3)) {
            let centroid = triangle.iter().map(position).sum::<Vec3>() / 3.;
            assert!(
                centroid.x <= 0.00001 || centroid.y <= 0.00001,
                "cap fills the L-shaped notch"
            );
        }
    }
    #[test]
    fn disconnected_bones_get_separate_caps_without_bridging_the_gap() {
        let mut mesh = crate::scene::Mesh::new();
        mesh.cube(Vec3::new(-2., 0., 0.), Vec3::ONE, [0.7; 3], 2.);
        mesh.cube(Vec3::new(2., 0., 0.), Vec3::ONE, [0.7; 3], 2.);
        let fragments = fracture(&mesh.vertices, Vec3::ZERO, Vec3::Y, 80.);
        assert_eq!(fragments.len(), 2);
        for triangle in fragments.iter().flat_map(|m| m.chunks_exact(3)) {
            let low = triangle
                .iter()
                .map(|v| v.pos[0])
                .fold(f32::INFINITY, f32::min);
            let high = triangle
                .iter()
                .map(|v| v.pos[0])
                .fold(f32::NEG_INFINITY, f32::max);
            assert!(high - low < 1.01, "cap bridges two separate bones");
        }
    }
    #[test]
    fn attached_and_detached_caps_share_the_cut_with_opposite_normals() {
        let pose = Pose::new(Vec3::ZERO, 0.3, 0.2, 0, 0., 1., 0., &Anatomy::default());
        for part in Part::ALL.into_iter().filter(|p| *p != Part::Torso) {
            let attached = sever_cap(&pose, part, false);
            let detached = sever_cap(&pose, part, true);
            assert!(!attached.is_empty() && attached.len() == detached.len());
            assert!(
                Vec3::from_array(attached[0].normal).dot(Vec3::from_array(detached[0].normal))
                    < -0.999
            );
        }
    }
}
