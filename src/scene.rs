use crate::anatomy::{Anatomy, BodyEvent, Part, Pose};
use crate::game::{Game, Mode, WeaponKind};
use crate::world_layout;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3};
use rapier3d::prelude::*;
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
    pub material: f32,
    pub local: [f32; 3],
    pub wear_uv: [f32; 2],
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct SphereInstance {
    pub transform: [[f32; 4]; 4],
    pub center_material: [f32; 4],
    pub radius: [f32; 4],
    pub color: [f32; 4],
}
/// Body space to world for one corpse section drawn from resident GPU geometry.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct PieceInstance {
    pub transform: [[f32; 4]; 4],
}
/// Conservative world-space sphere/frustum test; gameplay remains fully simulated.
pub struct Visibility {
    planes: [glam::Vec4; 6],
}
impl Visibility {
    pub fn new(vp: Mat4) -> Self {
        let r = vp.transpose();
        Self {
            planes: [
                r.w_axis + r.x_axis,
                r.w_axis - r.x_axis,
                r.w_axis + r.y_axis,
                r.w_axis - r.y_axis,
                r.z_axis,
                r.w_axis - r.z_axis,
            ],
        }
    }
    pub fn contains(&self, center: Vec3, radius: f32) -> bool {
        self.planes
            .iter()
            .all(|p| p.dot(center.extend(1.)) >= -radius * p.truncate().length())
    }
}
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub transparent: Vec<Vertex>,
    pub transform: Mat4,
    pub spheres: Vec<SphereInstance>,
    pub enemies: Vec<crate::enemy_assets::Instance>,
    pub instance_spheres: bool,
    /// Visible corpse sections drawn from resident geometry (by piece id),
    /// with one transform each.
    pub pieces: Vec<u64>,
    pub piece_instances: Vec<PieceInstance>,
    /// Expand corpse sections on the CPU even when optimized (a diagnostic
    /// comparison; see `GRAVEWAKE_CPU_CORPSES`).
    pub cpu_pieces: bool,
}
impl Mesh {
    pub fn new() -> Self {
        Self {
            vertices: vec![],
            transparent: vec![],
            transform: Mat4::IDENTITY,
            spheres: vec![],
            enemies: vec![],
            instance_spheres: false,
            pieces: vec![],
            piece_instances: vec![],
            cpu_pieces: false,
        }
    }
    pub fn triangle(&mut self, a: Vec3, b: Vec3, c: Vec3, color: [f32; 3], material: f32) {
        let normal = (b - a).cross(c - a).normalize_or_zero();
        let transformed_normal = self
            .transform
            .transform_vector3(normal)
            .normalize_or_zero()
            .to_array();
        let n = normal.abs();
        for p in [a, b, c] {
            let uv = if n.y > n.x && n.y > n.z {
                [p.x, p.z, 0.]
            } else if n.x > n.z {
                [p.z, p.y, 0.]
            } else {
                [p.x, p.y, 0.]
            };
            self.vertices.push(Vertex {
                pos: self.transform.transform_point3(p).to_array(),
                normal: transformed_normal,
                color,
                material,
                local: uv,
                wear_uv: [uv[0] * 2.7, uv[1] * 2.7],
            });
        }
    }
    pub fn quad(&mut self, p: [Vec3; 4], color: [f32; 3], material: f32) {
        self.triangle(p[0], p[1], p[2], color, material);
        self.triangle(p[0], p[2], p[3], color, material);
    }
    pub fn cube(&mut self, center: Vec3, size: Vec3, color: [f32; 3], material: f32) {
        let s = size * 0.5;
        let v = |x: f32, y: f32, z: f32| center + Vec3::new(x * s.x, y * s.y, z * s.z);
        for q in [
            [
                v(-1., -1., 1.),
                v(1., -1., 1.),
                v(1., 1., 1.),
                v(-1., 1., 1.),
            ],
            [
                v(1., -1., -1.),
                v(-1., -1., -1.),
                v(-1., 1., -1.),
                v(1., 1., -1.),
            ],
            [
                v(1., -1., 1.),
                v(1., -1., -1.),
                v(1., 1., -1.),
                v(1., 1., 1.),
            ],
            [
                v(-1., -1., -1.),
                v(-1., -1., 1.),
                v(-1., 1., 1.),
                v(-1., 1., -1.),
            ],
            [
                v(-1., 1., 1.),
                v(1., 1., 1.),
                v(1., 1., -1.),
                v(-1., 1., -1.),
            ],
            [
                v(-1., -1., -1.),
                v(1., -1., -1.),
                v(1., -1., 1.),
                v(-1., -1., 1.),
            ],
        ] {
            self.quad(q, color, material);
        }
    }
    pub fn taper(
        &mut self,
        a: Vec3,
        b: Vec3,
        r1: f32,
        r2: f32,
        sides: u32,
        color: [f32; 3],
        material: f32,
    ) {
        let axis = (b - a).normalize_or_zero();
        let tangent = axis
            .cross(if axis.y.abs() < 0.9 { Vec3::Y } else { Vec3::X })
            .normalize_or_zero();
        let bitangent = axis.cross(tangent);
        for i in 0..sides {
            let t = i as f32 / sides as f32 * std::f32::consts::TAU;
            let u = (i + 1) as f32 / sides as f32 * std::f32::consts::TAU;
            let v = tangent * t.cos() + bitangent * t.sin();
            let w = tangent * u.cos() + bitangent * u.sin();
            self.quad(
                [a + v * r1, a + w * r1, b + w * r2, b + v * r2],
                color,
                material,
            );
            self.triangle(b, b + v * r2, b + w * r2, color, material);
            self.triangle(a, a + w * r1, a + v * r1, color, material);
        }
    }
    pub fn ellipsoid(&mut self, p: Vec3, r: Vec3, c: [f32; 3], mat: f32) {
        if self.instance_spheres {
            self.spheres.push(SphereInstance {
                transform: self.transform.to_cols_array_2d(),
                center_material: [p.x, p.y, p.z, mat],
                radius: r.extend(0.).to_array(),
                color: [c[0], c[1], c[2], 0.],
            });
            return;
        }
        static UNIT: std::sync::OnceLock<Vec<[Vec3; 4]>> = std::sync::OnceLock::new();
        let quads = UNIT.get_or_init(|| {
            let mut faces = Vec::with_capacity(84);
            for j in 0..7 {
                let a = std::f32::consts::PI * j as f32 / 7.;
                let b = std::f32::consts::PI * (j + 1) as f32 / 7.;
                for i in 0..12 {
                    let t = std::f32::consts::TAU * i as f32 / 12.;
                    let u = std::f32::consts::TAU * (i + 1) as f32 / 12.;
                    let v =
                        |a: f32, t: f32| Vec3::new(a.sin() * t.cos(), a.cos(), a.sin() * t.sin());
                    faces.push([v(a, t), v(b, t), v(b, u), v(a, u)]);
                }
            }
            faces
        });
        for face in quads {
            self.quad(face.map(|v| p + v * r), c, mat);
        }
    }
    pub fn tube(&mut self, a: Vec3, b: Vec3, r: f32, inner: f32, c: [f32; 3], mat: f32) {
        let axis = (b - a).normalize();
        let t = axis.cross(Vec3::Y).normalize();
        let u = axis.cross(t);
        for i in 0..12 {
            let angle = i as f32 * std::f32::consts::TAU / 12.;
            let next = (i + 1) as f32 * std::f32::consts::TAU / 12.;
            let v = t * angle.cos() + u * angle.sin();
            let w = t * next.cos() + u * next.sin();
            self.quad([a + v * r, a + w * r, b + w * r, b + v * r], c, mat);
            self.quad([b + v * r, b + w * r, b + w * inner, b + v * inner], c, mat);
            self.quad(
                [
                    b + v * inner,
                    b + w * inner,
                    b - axis * 0.10 + w * inner,
                    b - axis * 0.10 + v * inner,
                ],
                [0.008, 0.009, 0.01],
                0.,
            );
        }
    }
    pub fn shadow(&mut self, pos: Vec3, rx: f32, rz: f32) {
        for i in 0..20 {
            let a = i as f32 * std::f32::consts::TAU / 20.;
            let b = (i + 1) as f32 * std::f32::consts::TAU / 20.;
            for uv in [[0., 0.], [a.cos(), a.sin()], [b.cos(), b.sin()]] {
                self.transparent.push(Vertex {
                    pos: (pos + Vec3::new(uv[0] * rx, 0.025, uv[1] * rz)).to_array(),
                    normal: [0., 1., 0.],
                    color: [0.; 3],
                    material: -1.,
                    local: [uv[0], uv[1], 1.],
                    wear_uv: [0.; 2],
                });
            }
        }
    }
    pub fn glow(&mut self, pos: Vec3, eye: Vec3, r: f32, color: [f32; 3]) {
        let forward = (eye - pos).normalize_or_zero();
        let right = forward.cross(Vec3::Y).normalize_or_zero();
        let up = right.cross(forward);
        for uv in [
            [-1., -1.],
            [1., -1.],
            [1., 1.],
            [-1., -1.],
            [1., 1.],
            [-1., 1.],
        ] {
            self.transparent.push(Vertex {
                pos: (pos + right * uv[0] * r + up * uv[1] * r).to_array(),
                normal: forward.to_array(),
                color,
                material: 10.,
                local: [uv[0], uv[1], 1.],
                wear_uv: [0.; 2],
            });
        }
    }
    pub fn bone(&mut self, a: Vec3, b: Vec3, r: f32, c: [f32; 3]) {
        self.taper(a, b, r, r * 0.8, 5, c, 2.);
        self.cube(a, Vec3::splat(r * 1.6), c, 2.);
    }
}
fn noise(i: u32) -> f32 {
    let x = i.wrapping_mul(747796405).wrapping_add(2891336453);
    let x = ((x >> ((x >> 28) + 4)) ^ x).wrapping_mul(277803737);
    ((x >> 22) ^ x) as f32 / u32::MAX as f32
}
fn tint(c: [f32; 3], s: f32) -> [f32; 3] {
    [c[0] * s, c[1] * s, c[2] * s]
}
/// The closest six lights illuminate each view; all bowls retain visible flames.
pub use crate::world_layout::FIRES;
pub fn nearest_lights(eye: Vec3) -> [[f32; 4]; 6] {
    let mut lights = FIRES;
    lights.sort_by(|a, b| {
        Vec3::from_array(*a)
            .distance_squared(eye)
            .total_cmp(&Vec3::from_array(*b).distance_squared(eye))
    });
    std::array::from_fn(|i| [lights[i][0], lights[i][1], lights[i][2], 1.])
}

fn bank_height(x: f32, z: f32) -> f32 {
    let d = (x.abs().max(z.abs()) - 47.).max(0.);
    let smooth = |a: f32, b: f32| {
        let t = (d / a).clamp(0., 1.);
        t * t * (3. - 2. * t) * b
    };
    -0.045
        + smooth(1.8, 1.6)
        + smooth(8., 2.5 + (x * 0.23).sin() * (z * 0.19).cos() * 1.4)
        + smooth(20., 1.7)
}

pub fn world() -> Mesh {
    use crate::architecture_assets::{self as architecture, Asset as Landmark};
    use crate::environment_assets::{self as environment, Asset};
    let mut m = Mesh::new();
    m.cube(
        Vec3::new(0., -0.25, 0.),
        Vec3::new(156., 0.4, 156.),
        [0.12, 0.14, 0.11],
        6.,
    );

    // Broad, connected walking routes. Paving follows the courts rather than
    // covering the whole landscape with one repeated rectangular tile field.
    for x in -32i32..33 {
        for z in -32i32..33 {
            let seed = ((x + 33) * 81 + z + 33) as u32;
            let n = noise(seed);
            let px = x as f32 * 1.4 + (z % 2) as f32 * 0.65;
            let pz = z as f32 * 1.4;
            let court = px * px + pz * pz < 14f32.powi(2);
            let avenue = (pz + 6.).abs() < 4.3 && px.abs() < 43.;
            let spine = px.abs() < 3.5 && pz > -42. && pz < 43.;
            let chapel = (px + 29.).abs() < 8.4 && (pz + 8.).abs() < 11.3;
            let cloister = (px - 29.).abs() < 8.3 && (pz + 6.5).abs() < 12.8;
            let sanctuary = px.abs() < 8.2 && (pz + 34.).abs() < 5.5;
            if !(court || avenue || spine || chapel || cloister || sanctuary) || n > 0.94 {
                continue;
            }
            let center = Vec3::new(px, -0.035 + n * 0.025, pz);
            let col = tint([0.22, 0.25, 0.22], 0.75 + n * 0.34);
            let a = 0.07 + noise(seed + 1700) * 0.16;
            let corners = [
                (-0.67, -0.67 + a),
                (-0.67 + a, -0.67),
                (0.67 - a, -0.67),
                (0.67, -0.67 + a),
                (0.67, 0.67 - a),
                (0.67 - a, 0.67),
                (-0.67 + a, 0.67),
                (-0.67, 0.67 - a),
            ];
            for i in 0..8 {
                let (ax, az) = corners[i];
                let (bx, bz) = corners[(i + 1) % 8];
                let a = center + Vec3::new(ax, noise(seed + i as u32 + 200) * 0.018, az);
                let b = center + Vec3::new(bx, noise(seed + (i as u32 + 1) % 8 + 200) * 0.018, bz);
                m.triangle(center, b, a, col, 1.);
                m.quad(
                    [a, b, b - Vec3::Y * 0.09, a - Vec3::Y * 0.09],
                    tint(col, 0.65),
                    1.,
                );
            }
        }
    }

    // The same footprints drive player movement, pursuit and body physics.
    for (index, wall) in world_layout::WALLS.iter().enumerate() {
        if matches!(index, 6 | 7 | 18) {
            continue;
        }
        let along_x = wall.half_x > wall.half_z;
        let length = if along_x { wall.half_x } else { wall.half_z } * 2.;
        let depth = if along_x { wall.half_z } else { wall.half_x } * 2.;
        m.transform = Mat4::from_translation(Vec3::new(wall.x, 0., wall.z))
            * Mat4::from_rotation_y(if along_x {
                0.
            } else {
                std::f32::consts::FRAC_PI_2
            })
            * Mat4::from_scale(Vec3::new(length / 22., wall.height / 4.6, depth / 1.4));
        architecture::draw(&mut m, Landmark::CurtainWall);
    }
    m.transform = Mat4::from_translation(Vec3::new(-29., 0., 4.));
    architecture::draw(&mut m, Landmark::ChapelFacade);
    for z in [-20., 7.] {
        m.transform = Mat4::from_translation(Vec3::new(31.5, 0., z))
            * Mat4::from_scale(Vec3::new(5. / 5.1, 0.82, 1.2 / 1.86));
        architecture::draw(&mut m, Landmark::CloisterBay);
    }
    m.transform = Mat4::from_translation(Vec3::new(0., 0., -35.));
    architecture::draw(&mut m, Landmark::BellTower);
    m.transform = Mat4::from_translation(Vec3::new(0., 0., -8.));
    architecture::draw(&mut m, Landmark::Fountain);

    // Smooth earthen banks and scattered glacial stones lead into a taller
    // forest. Vertex normals follow the terrain, avoiding faceted striped ramps.
    m.transform = Mat4::IDENTITY;
    for x in -26i32..26 {
        for z in -26i32..26 {
            if x.abs().max(z.abs()) < 15 {
                continue;
            }
            let x = x as f32 * 3.;
            let z = z as f32 * 3.;
            let v = |x: f32, z: f32| Vec3::new(x, bank_height(x, z), z);
            let start = m.vertices.len();
            m.quad(
                [v(x, z), v(x, z + 3.), v(x + 3., z + 3.), v(x + 3., z)],
                [0.14, 0.16, 0.10],
                6.,
            );
            for vertex in &mut m.vertices[start..] {
                let [x, _, z] = vertex.pos;
                vertex.normal = Vec3::new(
                    bank_height(x - 0.2, z) - bank_height(x + 0.2, z),
                    0.4,
                    bank_height(x, z - 0.2) - bank_height(x, z + 0.2),
                )
                .normalize()
                .to_array();
            }
        }
    }
    for i in 0..88u32 {
        let angle = noise(i + 9700) * std::f32::consts::TAU;
        let d = Vec3::new(angle.sin(), 0., angle.cos());
        let p = d * (50. + noise(i + 9800) * 5.) / d.x.abs().max(d.z.abs());
        m.ellipsoid(
            Vec3::new(p.x, bank_height(p.x, p.z) - 0.55, p.z),
            Vec3::new(
                1. + noise(i + 9850) * 1.2,
                0.9 + noise(i + 9860),
                1. + noise(i + 9870),
            ),
            [0.14, 0.15, 0.13],
            1.,
        );
    }
    for i in 0..144u32 {
        let angle = noise(i * 9) * std::f32::consts::TAU;
        let direction = Vec3::new(angle.sin(), 0., angle.cos());
        let radius = (52. + noise(i * 9 + 1) * 19.) / direction.x.abs().max(direction.z.abs());
        tree(
            &mut m,
            direction * radius + Vec3::Y * bank_height(direction.x * radius, direction.z * radius),
            11. + noise(i * 9 + 2) * 11.,
            i,
        );
    }
    for (i, &(x, z, r)) in world_layout::PILLARS.iter().take(8).enumerate() {
        if i < 6 {
            tree(
                &mut m,
                Vec3::new(x, 0., z),
                12. + i as f32 * 0.6,
                200 + i as u32,
            );
            m.transform = Mat4::IDENTITY;
            m.ellipsoid(
                Vec3::new(x, 0.08, z),
                Vec3::new(r, 0.20, r),
                [0.12, 0.15, 0.10],
                6.,
            );
        } else {
            m.transform = Mat4::from_translation(Vec3::new(x, 0., z));
            architecture::draw(&mut m, Landmark::Obelisk);
        }
    }
    for p in FIRES {
        m.transform = Mat4::from_translation(Vec3::new(p[0], 0., p[2]))
            * Mat4::from_scale(Vec3::new(1., p[1] / 1.4, 1.));
        environment::draw(&mut m, Asset::Brazier);
    }
    // Grave rows frame the orchard's broad north/south lane. Broken stones,
    // small grass and fallen masonry enrich the edges without blocking paths.
    for i in 0..64u32 {
        let grave = world_layout::grave_placement(i);
        m.transform = Mat4::from_translation(grave.position)
            * Mat4::from_rotation_y(grave.yaw)
            * Mat4::from_rotation_z(grave.lean);
        environment::draw(
            &mut m,
            match i % 3 {
                0 => Asset::Grave0,
                1 => Asset::Grave1,
                _ => Asset::Grave2,
            },
        );
    }
    for i in 0..36u32 {
        let side = if i % 2 == 0 { -1. } else { 1. };
        let x = side * (40.5 + noise(i + 1800) * 5.5);
        let z = -40. + noise(i + 1900) * 80.;
        m.transform = Mat4::from_translation(Vec3::new(x, -0.18, z))
            * Mat4::from_rotation_y(noise(i + 1950) * 6.28)
            * Mat4::from_scale(Vec3::new(0.7, 0.55, 0.7));
        architecture::draw(&mut m, Landmark::Rubble);
    }
    for (x, z, yaw) in [(-56., -23., 0.3), (56., 20., -0.4), (-22., -58., 0.2)] {
        m.transform = Mat4::from_translation(Vec3::new(x, 3., z)) * Mat4::from_rotation_y(yaw);
        architecture::draw(&mut m, Landmark::ChapelWall);
    }
    for (x, z) in [(-51., -22.), (51., 9.), (-20., 52.), (22., 53.)] {
        m.transform = Mat4::from_translation(Vec3::new(x, 3., z));
        architecture::draw(&mut m, Landmark::RuinWall);
    }
    // Urns are attached to the masonry footprint, keeping the courtyard lanes clear.
    for (x, z) in [(-35., 4.), (-23., 4.), (25., 7.), (36., 7.)] {
        m.transform = Mat4::from_translation(Vec3::new(
            x,
            world_layout::WALLS
                .iter()
                .find(|w| w.x == x && w.z == z)
                .map_or(3., |w| w.height),
            z,
        )) * Mat4::from_scale(Vec3::splat(0.65));
        architecture::draw(&mut m, Landmark::Urn);
    }
    m.transform = Mat4::IDENTITY;
    // Sparse low tufts at paving margins. Geometry sways in the foliage shader.
    for i in 0..420u32 {
        let x = (noise(i * 7 + 8000) - 0.5) * 89.;
        let z = (noise(i * 7 + 8100) - 0.5) * 89.;
        if x.abs() < 5.
            || (z + 6.).abs() < 5.
            || !world_layout::is_walkable(Vec3::new(x, 0., z), 1.2)
        {
            continue;
        }
        if x * x + z * z < 220.
            || ((x + 29.).abs() < 9. && (z + 8.).abs() < 13.)
            || ((x - 29.).abs() < 10. && (z + 6.).abs() < 14.)
        {
            continue;
        }
        for j in 0..5 {
            let a = j as f32 * 1.256 + noise(i + 8300) * 2.;
            let base = Vec3::new(x + a.sin() * 0.13, -0.015, z + a.cos() * 0.13);
            let tip = base + Vec3::new(a.sin() * 0.15, 0.18 + noise(i + j) * 0.22, a.cos() * 0.15);
            let edge = Vec3::new(a.cos(), 0., -a.sin()) * 0.035;
            m.triangle(base - edge, base + edge, tip, [0.15, 0.23, 0.12], 5.);
            m.triangle(base + edge, base - edge, tip, [0.15, 0.23, 0.12], 5.);
        }
    }
    m.transform = Mat4::IDENTITY;
    m
}
fn tree(m: &mut Mesh, p: Vec3, h: f32, seed: u32) {
    use crate::environment_assets::{self as environment, Asset};
    let previous = m.transform;
    m.transform = Mat4::from_translation(p)
        * Mat4::from_rotation_y(noise(seed + 1300) * std::f32::consts::TAU)
        * Mat4::from_scale(Vec3::splat(h / 10.));
    environment::draw(
        m,
        if seed >= 200 {
            Asset::SpruceNear
        } else {
            match seed % 3 {
                0 => Asset::Spruce0,
                1 => Asset::Spruce1,
                _ => Asset::Spruce2,
            }
        },
    );
    m.transform = previous;
}

pub fn skeleton(
    m: &mut Mesh,
    pos: Vec3,
    yaw: f32,
    phase: f32,
    kind: usize,
    hit: f32,
    walk: f32,
    attack: f32,
    anatomy: &Anatomy,
    only: Option<Part>,
) {
    if only.is_none() {
        m.shadow(
            pos,
            if kind == 3 { 0.7 } else { 0.42 },
            if kind == 3 { 0.52 } else { 0.29 },
        );
    }
    let pose = Pose::new(pos, yaw, phase, kind, hit, walk, attack, anatomy);
    crate::enemy_assets::draw(m, &pose, kind, phase, hit, anatomy, only);
    if only.is_none() && anatomy.severed != 0 {
        let normal = m.transform.inverse().transpose();
        for mut vertex in crate::dismemberment::stump_caps(&pose, anatomy, kind) {
            vertex.pos = m
                .transform
                .transform_point3(Vec3::from_array(vertex.pos))
                .to_array();
            vertex.normal = normal
                .transform_vector3(Vec3::from_array(vertex.normal))
                .normalize_or_zero()
                .to_array();
            m.vertices.push(vertex);
        }
    }
}

fn special_weapon(m: &mut Mesh, kind: WeaponKind, rarity: usize, age: f32) {
    if crate::weapon_assets::draw(m, kind, rarity, None, age) {
        return;
    }
    let weather_start = m.vertices.len();
    use crate::weapons::Model;
    let spec = kind.spec();
    let iron = [0.13, 0.15, 0.16];
    let gold = [0.65, 0.43, 0.17];
    let bone = [0.7, 0.65, 0.5];
    let wood = [0.23, 0.065, 0.035];
    let glow = kind.tint();
    let root = m.transform;
    match spec.model {
        Model::Staff => {
            m.taper(
                Vec3::new(0., -0.42, 0.08),
                Vec3::new(0., 0.50, -0.18),
                0.045,
                0.027,
                8,
                wood,
                4.,
            );
            for i in 0..4 {
                let y = -0.25 + i as f32 * 0.22;
                m.taper(
                    Vec3::new(0., y, 0.02 - y * 0.27),
                    Vec3::new(0., y + 0.035, 0.01 - y * 0.27),
                    0.063,
                    0.06,
                    8,
                    gold,
                    3.,
                );
            }
            for side in [-1., 1.] {
                m.bone(
                    Vec3::new(0., 0.44, -0.16),
                    Vec3::new(side * 0.17, 0.64, -0.2),
                    0.028,
                    gold,
                );
                m.bone(
                    Vec3::new(side * 0.17, 0.64, -0.2),
                    Vec3::new(side * 0.08, 0.82, -0.22),
                    0.019,
                    gold,
                );
            }
            m.ellipsoid(
                Vec3::new(0., 0.65, -0.2),
                if kind == WeaponKind::StormWand {
                    Vec3::new(0.055, 0.21, 0.055)
                } else {
                    Vec3::new(0.10, 0.13, 0.10)
                },
                glow,
                9.,
            );
        }
        Model::Lantern => {
            m.taper(
                Vec3::new(0., -0.25, 0.),
                Vec3::new(0., -0.20, 0.),
                0.19,
                0.22,
                8,
                gold,
                3.,
            );
            m.taper(
                Vec3::new(0., 0.22, 0.),
                Vec3::new(0., 0.34, 0.),
                0.22,
                0.07,
                8,
                gold,
                3.,
            );
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.;
                m.bone(
                    Vec3::new(a.cos() * 0.19, -0.2, a.sin() * 0.19),
                    Vec3::new(a.cos() * 0.19, 0.23, a.sin() * 0.19),
                    0.018,
                    iron,
                );
            }
            m.ellipsoid(
                Vec3::new(0., 0.015, 0.),
                Vec3::new(0.10, 0.16, 0.10),
                glow,
                9.,
            );
            for i in 0..10 {
                let a = i as f32 / 9. * std::f32::consts::PI;
                m.bone(
                    Vec3::new(a.cos() * 0.12, 0.35 + a.sin() * 0.12, 0.),
                    Vec3::new((a + 0.2).cos() * 0.12, 0.35 + (a + 0.2).sin() * 0.12, 0.),
                    0.015,
                    gold,
                );
            }
            if kind == WeaponKind::PlagueCenser {
                for i in 0..8 {
                    m.ellipsoid(
                        Vec3::new(0., 0.44 + i as f32 * 0.045, 0.),
                        Vec3::new(0.021, 0.029, 0.014),
                        iron,
                        3.,
                    );
                }
            }
        }
        Model::Bow | Model::Crossbow => {
            let cross = spec.model == Model::Crossbow;
            if cross {
                m.cube(
                    Vec3::new(0., -0.04, -0.16),
                    Vec3::new(0.08, 0.12, 0.8),
                    wood,
                    4.,
                );
                m.cube(
                    Vec3::new(0., -0.19, 0.13),
                    Vec3::new(0.07, 0.22, 0.09),
                    wood,
                    4.,
                );
            }
            let mut prev = if cross {
                Vec3::new(-0.48, 0.02, -0.38)
            } else {
                Vec3::new(0., -0.58, -0.28)
            };
            for i in 1..=16 {
                let t = i as f32 / 16.;
                let bend = (t * std::f32::consts::PI).sin() * 0.23;
                let p = if cross {
                    Vec3::new(-0.48 + t * 0.96, 0.02, -0.38 - bend)
                } else {
                    Vec3::new(0., -0.58 + t * 1.25, -0.28 - bend)
                };
                m.taper(
                    prev,
                    p,
                    0.025,
                    0.023,
                    7,
                    if kind == WeaponKind::Harpoon {
                        iron
                    } else {
                        bone
                    },
                    2.,
                );
                prev = p;
            }
            let ends = if cross {
                [Vec3::new(-0.48, 0.02, -0.38), Vec3::new(0.48, 0.02, -0.38)]
            } else {
                [Vec3::new(0., -0.58, -0.28), Vec3::new(0., 0.67, -0.28)]
            };
            let nock = Vec3::new(
                0.,
                0.02,
                -0.03 - crate::motion::window(0., 0.05, 0.15, 0.4, age) * 0.29,
            );
            for end in ends {
                m.taper(end, nock, 0.003, 0.003, 4, bone, 0.);
            }
            m.taper(nock, Vec3::new(0., 0.02, -0.87), 0.011, 0.008, 6, wood, 4.);
            m.taper(
                Vec3::new(0., 0.02, -0.84),
                Vec3::new(0., 0.02, -0.98),
                0.042,
                0.,
                4,
                gold,
                3.,
            );
        }
        Model::Cannon => {
            m.cube(
                Vec3::new(0., -0.17, 0.1),
                Vec3::new(0.15, 0.35, 0.17),
                wood,
                4.,
            );
            let r = if kind == WeaponKind::Mortar {
                0.19
            } else if kind == WeaponKind::HandCannon {
                0.10
            } else {
                0.14
            };
            m.tube(
                Vec3::new(0., 0.05, 0.13),
                Vec3::new(0., 0.05, -0.62),
                r,
                r * 0.74,
                iron,
                3.,
            );
            for z in [0.08, -0.12, -0.54] {
                m.tube(
                    Vec3::new(0., 0.05, z + 0.025),
                    Vec3::new(0., 0.05, z - 0.025),
                    r * 1.08,
                    r * 0.95,
                    gold,
                    3.,
                );
            }
            if kind == WeaponKind::GrenadeLauncher {
                m.ellipsoid(
                    Vec3::new(0., -0.10, -0.04),
                    Vec3::new(0.18, 0.19, 0.14),
                    iron,
                    3.,
                );
            }
        }
        Model::Blade
        | Model::Rapier
        | Model::Hammer
        | Model::Scythe
        | Model::Flail
        | Model::Daggers => {
            let copies = if spec.model == Model::Daggers { 2 } else { 1 };
            for side in 0..copies {
                if copies == 2 {
                    m.transform =
                        root * Mat4::from_translation(Vec3::new(
                            (side as f32 - 0.5) * 0.42,
                            0.,
                            -side as f32 * 0.07,
                        )) * Mat4::from_rotation_z((side as f32 - 0.5) * 0.3);
                }
                let length = match spec.model {
                    Model::Hammer => 0.55,
                    Model::Scythe => 0.8,
                    Model::Flail => 0.45,
                    _ => 0.02,
                };
                m.taper(
                    Vec3::new(0., -0.30, 0.),
                    Vec3::new(0., length, 0.),
                    0.032,
                    0.026,
                    8,
                    wood,
                    4.,
                );
                for i in 0..7 {
                    m.taper(
                        Vec3::new(0., -0.26 + i as f32 * 0.035, 0.),
                        Vec3::new(0., -0.248 + i as f32 * 0.035, 0.),
                        0.038,
                        0.038,
                        8,
                        gold,
                        3.,
                    );
                }
                match spec.model {
                    Model::Hammer => {
                        m.cube(
                            Vec3::new(0., 0.55, 0.),
                            Vec3::new(0.46, 0.22, 0.23),
                            iron,
                            3.,
                        );
                        for x in [-0.24, 0.24] {
                            m.cube(
                                Vec3::new(x, 0.55, 0.),
                                Vec3::new(0.04, 0.26, 0.27),
                                gold,
                                3.,
                            );
                        }
                    }
                    Model::Flail => {
                        let mut p = Vec3::new(0., 0.45, 0.);
                        for i in 0..10 {
                            let q = Vec3::new(
                                0.025 * (i as f32 * 0.7 + age * 6.).sin(),
                                0.48 + i as f32 * 0.045,
                                -i as f32 * 0.024,
                            );
                            m.bone(p, q, 0.009, gold);
                            p = q;
                        }
                        m.ellipsoid(p, Vec3::splat(0.13), iron, 3.);
                        for i in 0..8 {
                            let a = i as f32 * 0.785;
                            m.taper(
                                p,
                                p + Vec3::new(a.cos() * 0.23, a.sin() * 0.23, 0.),
                                0.034,
                                0.,
                                5,
                                gold,
                                3.,
                            );
                        }
                    }
                    Model::Scythe => {
                        m.quad(
                            [
                                Vec3::new(-0.05, 0.73, 0.),
                                Vec3::new(0.08, 0.90, 0.),
                                Vec3::new(-0.55, 0.81, -0.04),
                                Vec3::new(-0.8, 0.56, -0.08),
                            ],
                            [0.65, 0.71, 0.68],
                            3.,
                        );
                        m.bone(
                            Vec3::new(0., 0.79, 0.),
                            Vec3::new(-0.51, 0.81, -0.04),
                            0.022,
                            gold,
                        );
                    }
                    _ => {
                        let blade = if spec.model == Model::Rapier {
                            0.89
                        } else if copies == 2 {
                            0.43
                        } else {
                            0.58
                        };
                        let width = if spec.model == Model::Blade {
                            0.18
                        } else if copies == 2 {
                            0.055
                        } else {
                            0.023
                        };
                        m.cube(
                            Vec3::new(0., 0., 0.),
                            Vec3::new(width * 2. + 0.09, 0.035, 0.07),
                            gold,
                            3.,
                        );
                        m.taper(
                            Vec3::new(0., 0.025, 0.),
                            Vec3::new(0., blade, 0.),
                            width,
                            0.,
                            4,
                            [0.55, 0.63, 0.65],
                            3.,
                        );
                        if spec.model == Model::Blade {
                            m.cube(
                                Vec3::new(0., blade * 0.45, 0.),
                                Vec3::new(0.24, blade * 0.7, 0.027),
                                iron,
                                3.,
                            );
                        }
                    }
                }
                m.ellipsoid(
                    Vec3::new(0., -0.32, 0.),
                    Vec3::splat(0.044),
                    if rarity > 0 { glow } else { gold },
                    if rarity > 0 { 9. } else { 3. },
                );
            }
        }
        _ => {}
    }
    // All occult/melee/bow fittings share the same aged materials as the guns.
    for v in &mut m.vertices[weather_start..] {
        let c = v.color;
        if v.material == 3. || (v.material == 2. && (c == iron || c == gold)) {
            v.material = if c[0] > c[1] * 1.2 && c[2] < c[1] * 0.8 {
                12.
            } else {
                11.
            };
        } else if v.material == 4. {
            v.material = 13.;
        }
    }
    m.transform = root;
}
pub fn weapon(m: &mut Mesh, kind: WeaponKind, rarity: usize) {
    weapon_pose(m, kind, rarity, None, 10.);
}
fn weapon_pose(m: &mut Mesh, kind: WeaponKind, rarity: usize, reload: Option<f32>, shot_age: f32) {
    let design = kind;
    use crate::weapons::Model;
    if matches!(
        kind.spec().model,
        Model::Staff
            | Model::Lantern
            | Model::Bow
            | Model::Crossbow
            | Model::Blade
            | Model::Rapier
            | Model::Hammer
            | Model::Scythe
            | Model::Flail
            | Model::Daggers
    ) {
        special_weapon(m, kind, rarity, shot_age);
        return;
    }
    let kind = kind.base();
    let t = reload.unwrap_or(0.);
    let pose = crate::motion::reload_pose(kind, t);
    let root = m.transform;
    let hinge = Mat4::from_translation(Vec3::new(0., -0.02, 0.04))
        * Mat4::from_rotation_x(-pose.hinge * 0.92)
        * Mat4::from_translation(Vec3::new(0., 0.02, -0.04));
    crate::guns::firearm(m, design, rarity, reload, shot_age);
    if reload.is_some() && kind == WeaponKind::Double && design != WeaponKind::SlugGun {
        // Extractors throw the spent brass clear of the open chambers.
        if (0.27..0.54).contains(&t) {
            for side in [-1., 1.] {
                let age = (t - 0.27) * 2.35;
                let p = Vec3::new(
                    side * (if design == WeaponKind::Double {
                        0.060
                    } else {
                        0.075
                    } + age * 0.65),
                    0.1 + age * 2.8 - age * age * 1.5,
                    0.08 + age * 0.25,
                );
                m.transform = root
                    * Mat4::from_translation(p)
                    * Mat4::from_rotation_x(age * 12.)
                    * Mat4::from_rotation_z(side * age * 6.);
                shell(m);
            }
        }
        for (i, start) in [0.37, 0.54].iter().enumerate() {
            if t >= *start && t < 0.79 {
                let a = crate::motion::smooth(*start, *start + 0.11, t);
                let seated = if design == WeaponKind::Double {
                    // The brass rim seats flush in the Blender-authored open chambers.
                    Vec3::new((i as f32 - 0.5) * 0.12, 0.074, -0.235)
                } else {
                    Vec3::new((i as f32 - 0.5) * 0.15, 0.08, -0.10)
                };
                let p = seated + Vec3::new(-0.18, -0.34, 0.32) * (1. - a);
                m.transform = root * hinge * Mat4::from_translation(p);
                shell(m);
            }
        }
    }
    m.transform = root;
}
fn shell(m: &mut Mesh) {
    m.taper(
        Vec3::ZERO,
        Vec3::new(0., 0., 0.16),
        0.043,
        0.043,
        10,
        [0.49, 0.025, 0.012],
        7.,
    );
    m.taper(
        Vec3::new(0., 0., 0.13),
        Vec3::new(0., 0., 0.172),
        0.047,
        0.047,
        10,
        [0.83, 0.57, 0.16],
        3.,
    );
    m.ellipsoid(
        Vec3::new(0., 0., 0.176),
        Vec3::new(0.016, 0.016, 0.004),
        [0.27, 0.19, 0.10],
        3.,
    );
}
fn glove(m: &mut Mesh, p: Vec3, support: bool) {
    let leather = [0.035, 0.022, 0.016];
    crate::guns::ellipsoid(m, p, Vec3::new(0.09, 0.105, 0.08), leather, 14.);
    for i in 0..4 {
        let a = p + Vec3::new(-0.065 + i as f32 * 0.042, 0.02, -0.045);
        let b = a + Vec3::new(0., 0.045, -0.058);
        m.taper(a, b, 0.019, 0.016, 7, leather, 14.);
        m.taper(
            b,
            b + Vec3::new(0., -0.035, -0.03),
            0.016,
            0.012,
            7,
            leather,
            14.,
        );
    }
    m.taper(
        p + Vec3::new(-0.08, 0., 0.),
        p + Vec3::new(-0.11, 0.06, -0.04),
        0.029,
        0.023,
        7,
        leather,
        14.,
    );
    m.taper(
        p + Vec3::new(0., -0.03, 0.04),
        p + Vec3::new(if support { -0.13 } else { 0.09 }, -0.24, 0.32),
        0.07,
        0.11,
        8,
        [0.045, 0.010, 0.010],
        14.,
    );
}
pub fn chalice(m: &mut Mesh) {
    let gold = [0.60, 0.38, 0.12];
    let metal = [0.10, 0.09, 0.12];
    let z = -0.2;
    m.taper(
        Vec3::new(0., -0.35, z),
        Vec3::new(0., -0.30, z),
        0.20,
        0.16,
        8,
        metal,
        3.,
    );
    m.taper(
        Vec3::new(0., -0.305, z),
        Vec3::new(0., -0.28, z),
        0.17,
        0.11,
        8,
        gold,
        3.,
    );
    m.taper(
        Vec3::new(0., -0.28, z),
        Vec3::new(0., -0.02, z),
        0.037,
        0.045,
        8,
        gold,
        3.,
    );
    m.ellipsoid(
        Vec3::new(0., -0.12, z),
        Vec3::new(0.06, 0.045, 0.06),
        metal,
        3.,
    );
    m.taper(
        Vec3::new(0., -0.02, z),
        Vec3::new(0., 0.22, z),
        0.085,
        0.23,
        10,
        metal,
        3.,
    );
    // Raised rim, wine surface and warm glowing embers.
    m.taper(
        Vec3::new(0., 0.215, z),
        Vec3::new(0., 0.245, z),
        0.239,
        0.239,
        10,
        gold,
        3.,
    );
    m.taper(
        Vec3::new(0., 0.245, z),
        Vec3::new(0., 0.248, z),
        0.211,
        0.211,
        10,
        [0.20, 0.024, 0.026],
        0.,
    );
    for i in 0..5 {
        let a = i as f32 * 1.256;
        m.ellipsoid(
            Vec3::new(
                a.cos() * 0.115,
                0.28 + (i % 3) as f32 * 0.04,
                z + a.sin() * 0.115,
            ),
            Vec3::splat(0.025),
            [2., 0.35, 0.07],
            9.,
        );
    }
}
pub fn dynamic(game: &Game, physics: &Bones, mut m: &mut Mesh, vp: Mat4, optimized: bool) {
    let visibility = Visibility::new(vp);
    m.vertices.clear();
    m.transparent.clear();
    m.spheres.clear();
    m.enemies.clear();
    m.pieces.clear();
    m.piece_instances.clear();
    m.transform = Mat4::IDENTITY;
    let arena = matches!(
        game.mode,
        Mode::Arena | Mode::LevelUp | Mode::Paused | Mode::Dead | Mode::Victory
    );
    if arena {
        m.instance_spheres = optimized;
        for e in &game.run.enemies {
            if e.ai.warning > 0. {
                let color = match e.kind {
                    7 => [0.5, 2., 0.08],
                    11 => [1.3, 0.25, 2.5],
                    _ => [2.8, 0.5, 0.09],
                };
                let center = if matches!(e.kind, 3 | 7 | 11) {
                    e.ai.target
                } else {
                    e.pos
                };
                let r = match e.kind {
                    3 => 4.2,
                    7 => 3.2,
                    11 => 0.8,
                    _ => 0.65,
                };
                for j in 0..48 {
                    let a = j as f32 * std::f32::consts::TAU / 48.;
                    let b = (j + 1) as f32 * std::f32::consts::TAU / 48.;
                    m.taper(
                        center + Vec3::new(a.cos() * r, 0.08, a.sin() * r),
                        center + Vec3::new(b.cos() * r, 0.08, b.sin() * r),
                        0.025,
                        0.025,
                        3,
                        color,
                        9.,
                    );
                }
            }
            let scale = crate::encounters::species(e.kind).scale;
            if optimized && !visibility.contains(e.pos + Vec3::Y * 1.5 * scale, 2.4 * scale) {
                continue;
            }
            let surface_start = m.vertices.len();
            let sphere_start = m.spheres.len();
            let enemy_start = m.enemies.len();
            let delta = game.run.pos - e.pos;
            skeleton(
                &mut m,
                e.pos,
                delta.x.atan2(delta.z),
                e.phase,
                e.kind,
                e.hit,
                if Vec3::new(delta.x, 0., delta.z).length() > if e.kind == 2 { 7. } else { 1.25 } {
                    1.
                } else {
                    0.
                },
                if e.attack > 0.45 && e.kind != 2 {
                    1.1 - e.attack
                } else {
                    0.
                },
                &e.anatomy,
                None,
            );
            // Close combat keeps the sculpted mesh; distant crowds use the
            // authored silhouette-preserving Blender reductions.
            let distance_squared = game.run.pos.distance_squared(e.pos);
            let lod = if distance_squared > 196. {
                2
            } else if distance_squared > 64. {
                1
            } else {
                0
            };
            for enemy in &mut m.enemies[enemy_start..] {
                enemy.params[0] += 12. * lod as f32;
            }
            let status = if e.burn > 0. {
                20.
            } else if e.slow > 0. {
                21.
            } else if e.poison > 0. {
                22.
            } else {
                2.
            };
            if status > 2. {
                for enemy in &mut m.enemies[enemy_start..] {
                    enemy.params[1] = status;
                }
                for sphere in &mut m.spheres[sphere_start..] {
                    if sphere.center_material[3] == 2. {
                        sphere.center_material[3] = status;
                    }
                }
                for vertex in &mut m.vertices[surface_start..] {
                    if vertex.material == 2. {
                        vertex.material = status;
                    }
                }
            }
        }
        m.instance_spheres = false;
    } else if matches!(game.mode, Mode::Title | Mode::Bestiary) {
        let kind = if game.mode == Mode::Bestiary {
            game.bestiary_index.min(crate::encounters::ROSTER.len() - 1)
        } else {
            3
        };
        skeleton(
            &mut m,
            Vec3::new(0., 0., 0.),
            0.25,
            game.elapsed,
            kind,
            0.,
            0.,
            0.,
            &Anatomy::default(),
            None,
        );
    }
    if arena {
        for o in &game.run.survival.orbs {
            if optimized && !visibility.contains(o.pos, 0.6) {
                continue;
            }
            let radius = if o.value >= 40 {
                0.18
            } else if o.value >= 10 {
                0.13
            } else {
                0.095
            };
            let color = if o.value >= 40 {
                [2., 1.2, 0.15]
            } else {
                [0.12, 1.8, 1.25]
            };
            m.ellipsoid(o.pos, Vec3::new(radius, radius * 1.4, radius), color, 9.);
            m.glow(o.pos, game.run.pos, radius * 2.8, color);
            m.shadow(Vec3::new(o.pos.x, 0., o.pos.z), radius * 1.7, radius);
        }
        for h in &game.hazards {
            m.ellipsoid(h.pos, Vec3::splat(0.13), h.color, 9.);
            m.glow(h.pos, game.run.pos, 0.32, h.color);
        }
        let rank = game.run.survival.ranks[6];
        if rank > 0 {
            let blades = crate::survival::power::halo(rank).0;
            for j in 0..blades {
                let a = game.run.time * 2.7 + j as f32 * std::f32::consts::TAU / blades as f32;
                let p = game.run.pos + Vec3::new(a.cos() * 2.5, -0.45, a.sin() * 2.5);
                m.taper(
                    p - Vec3::Y * 0.3,
                    p + Vec3::Y * 0.3,
                    0.08,
                    0.002,
                    4,
                    [0.4, 1.8, 1.4],
                    9.,
                );
                m.glow(p, game.run.pos, 0.25, [0.2, 0.8, 0.6]);
            }
        }
    }
    m.instance_spheres = false;
    for p in FIRES {
        let p = Vec3::from_array(p);
        if optimized && !visibility.contains(p + Vec3::Y * 0.7, 1.6) {
            continue;
        }
        for j in 0..6 {
            let t = game.elapsed * 5. + j as f32 * 1.74;
            let pos = p + Vec3::new(
                t.sin() * 0.10,
                0.06 + (t * 0.7).cos().abs() * 0.2,
                t.cos() * 0.10,
            );
            m.taper(
                pos,
                pos + Vec3::new(t.sin() * 0.13, 0.45 + (t * 1.3).sin() * 0.17, 0.),
                0.13,
                0.,
                5,
                if j % 2 == 0 {
                    [4., 1.8, 0.15]
                } else {
                    [3., 0.45, 0.015]
                },
                9.,
            );
        }
        for j in 0..5 {
            let t = (game.elapsed * 0.7 + j as f32 * 0.31).fract();
            let pos = p + Vec3::new((game.elapsed + j as f32).sin() * t * 0.5, t * 1.7, 0.);
            m.cube(pos, Vec3::splat(0.024 * (1. - t)), [4., 1., 0.1], 9.);
        }
    }
    if arena {
        for p in &game.particles {
            if optimized && !visibility.contains(p.pos, p.size * 2.) {
                continue;
            }
            m.cube(p.pos, Vec3::splat(p.size), p.color, 9.);
        }
        let resident = optimized && !m.cpu_pieces;
        for piece in &physics.pieces {
            if let Some(b) = physics.bodies.get(piece.handle) {
                let t = b.translation();
                let q = b.rotation().quaternion();
                let rotation = Quat::from_xyzw(q.i, q.j, q.k, q.w);
                let position = Vec3::new(t.x, t.y, t.z);
                if optimized && !visibility.contains(position, piece.radius * 1.5) {
                    continue;
                }
                m.shadow(
                    position * Vec3::new(1., 0., 1.),
                    piece.radius * 1.3,
                    piece.radius * 0.8,
                );
                if resident {
                    m.pieces.push(piece.id);
                    m.piece_instances.push(PieceInstance {
                        transform: piece.model(rotation, position).to_cols_array_2d(),
                    });
                    continue;
                }
                let fade = piece.fade();
                for vertex in &piece.vertices {
                    let mut v = *vertex;
                    v.pos = (rotation * Vec3::from_array(v.pos) * fade + position).to_array();
                    v.normal = (rotation * Vec3::from_array(v.normal)).to_array();
                    m.vertices.push(v);
                }
            }
        }
        m.transform = Mat4::IDENTITY;
    }
    if game.mode == Mode::Arena && !game.run.weapon.kind.melee() {
        let moving = game.motion_speed;
        let bob = (game.run.time * 9.).sin() * 0.012 * moving;
        let strength = if game.run.weapon.kind.base() == WeaponKind::Double {
            1.
        } else {
            0.52
        };
        let kick = crate::motion::recoil(game.shot_age) * strength;
        let progress = if game.reload > 0. {
            Some(1. - game.reload / game.run.weapon.reload_time())
        } else {
            None
        };
        let p = crate::motion::reload_pose(game.run.weapon.kind, progress.unwrap_or(0.));
        let camera = Mat4::from_rotation_y(-game.run.yaw) * Mat4::from_rotation_x(game.run.pitch);
        m.transform = Mat4::from_translation(game.run.pos)
            * camera
            * Mat4::from_translation(Vec3::new(
                0.18 + bob - p.present * 0.13 + game.look_sway.x,
                -0.23
                    + kick * 0.028
                    + p.present * 0.10
                    + game.look_sway.y
                    + (game.run.time * 18.).cos() * 0.004 * moving,
                -0.58 + kick * 0.14 + p.present * 0.065,
            ))
            * Mat4::from_rotation_z(-kick * 0.065 - p.present * 0.22)
            * Mat4::from_rotation_y(p.present * 0.3 + game.look_sway.x * 2.)
            * Mat4::from_rotation_x(kick * 0.31 - p.present * 0.10)
            * Mat4::from_scale(Vec3::splat(0.65));
        let weapon_root = m.transform;
        weapon_pose(
            &mut m,
            game.run.weapon.kind,
            game.run.weapon.rarity,
            progress,
            game.shot_age,
        );
        glove(&mut m, Vec3::new(0.025, -0.19, 0.22), false);
        let mut hand = Vec3::new(-0.065, -0.15, -0.26);
        if let Some(t) = progress {
            if game.run.weapon.kind.base() == WeaponKind::Double {
                // Support hand leaves the fore-end, retrieves two shells, seats them, returns.
                let a = crate::motion::window(0.14, 0.28, 0.72, 0.88, t);
                hand = hand.lerp(Vec3::new(-0.22, -0.43, 0.22), a);
                for start in [0.37, 0.54] {
                    let load =
                        crate::motion::window(start, start + 0.10, start + 0.12, start + 0.16, t);
                    hand = hand.lerp(Vec3::new(-0.075, 0.02, 0.23), load);
                }
            } else {
                hand = hand.lerp(
                    Vec3::new(-0.06, -0.26 - p.magazine * 0.37, 0.1),
                    p.left_hand,
                );
                hand = hand.lerp(
                    Vec3::new(-0.07, 0.2, 0.12),
                    crate::motion::window(0.68, 0.73, 0.81, 0.89, t),
                );
            }
        }
        glove(&mut m, hand, true);
        m.transform = weapon_root;
        let muzzle_z = crate::guns::muzzle(game.run.weapon.kind);
        if game.shot_age < 0.055 && game.run.weapon.kind.spec().group == 4 {
            let p = if game.run.weapon.kind.spec().model == crate::weapons::Model::Staff {
                Vec3::new(0., 0.65, -0.2)
            } else {
                Vec3::ZERO
            };
            m.glow(
                weapon_root.transform_point3(p),
                game.run.pos,
                0.3,
                game.run.weapon.kind.tint(),
            );
        }
        if game.shot_age < 0.055
            && game.run.weapon.kind.spec().group != 4
            && !matches!(
                game.run.weapon.kind.spec().model,
                crate::weapons::Model::Bow | crate::weapons::Model::Crossbow
            )
        {
            let shots = if game.run.weapon.kind.base() == WeaponKind::Double
                && !matches!(
                    game.run.weapon.kind,
                    WeaponKind::SlugGun | WeaponKind::Blunderbuss
                ) {
                2
            } else {
                1
            };
            for barrel in 0..shots {
                let x = if shots == 2 {
                    (barrel as f32 - 0.5)
                        * if game.run.weapon.kind == WeaponKind::Double {
                            0.12
                        } else {
                            0.15
                        }
                } else {
                    0.
                };
                let muzzle = Vec3::new(x, 0.08, muzzle_z);
                m.ellipsoid(muzzle, Vec3::splat(0.035), [9., 6., 2.], 9.);
                m.taper(
                    muzzle,
                    muzzle - Vec3::Z * 0.33,
                    0.055,
                    0.,
                    7,
                    [6., 2.4, 0.15],
                    9.,
                );
                for i in 0..6 {
                    let a = i as f32 * std::f32::consts::TAU / 6. + game.run.time * 13.;
                    let r = 0.08 + noise(i + barrel * 7) * 0.055;
                    let tip = muzzle + Vec3::new(a.cos() * r, a.sin() * r, -0.10);
                    m.triangle(
                        muzzle - Vec3::X * 0.025,
                        tip,
                        muzzle + Vec3::X * 0.025,
                        [4., 1., 0.03],
                        9.,
                    );
                    m.triangle(
                        muzzle - Vec3::Y * 0.025,
                        tip,
                        muzzle + Vec3::Y * 0.025,
                        [4., 1., 0.03],
                        9.,
                    );
                }
                m.glow(
                    weapon_root.transform_point3(muzzle),
                    game.run.pos,
                    0.12,
                    [3., 0.9, 0.08],
                );
            }
        }
        if game.shot_age > 0.055
            && game.shot_age < 0.55
            && game.run.weapon.kind.spec().group < 4
            && !matches!(
                game.run.weapon.kind.spec().model,
                crate::weapons::Model::Bow | crate::weapons::Model::Crossbow
            )
        {
            let age = game.shot_age - 0.055;
            for i in 0..3 {
                let p = Vec3::new(
                    (age * 13. + i as f32).sin() * age * 0.065,
                    0.08 + age * 0.23,
                    muzzle_z - age * (0.15 + i as f32 * 0.1),
                );
                m.glow(
                    weapon_root.transform_point3(p),
                    game.run.pos,
                    0.025 + age * 0.1,
                    [0.23, 0.24, 0.23],
                );
                let len = m.vertices.len();
                for v in &mut m.vertices[len - 6..] {
                    v.local[2] = (1. - age / 0.5) * 0.7;
                }
            }
        }
    }
    if game.mode == Mode::Arena && game.run.weapon.kind.melee() {
        let kind = game.run.weapon.kind;
        let t = (game.shot_age / game.run.weapon.interval()).clamp(0., 1.);
        let wind = crate::motion::window(0., 0.15, 0.18, 0.30, t);
        let strike = crate::motion::window(0.16, 0.32, 0.42, 0.95, t);
        let side = if kind == WeaponKind::TwinDaggers && game.attack_serial % 2 == 1 {
            -1.
        } else {
            1.
        };
        let camera = Mat4::from_rotation_y(-game.run.yaw) * Mat4::from_rotation_x(game.run.pitch);
        m.transform = Mat4::from_translation(game.run.pos)
            * camera
            * Mat4::from_translation(Vec3::new(
                0.3 * side - strike * 0.2 * side,
                -0.36 + wind * 0.12,
                -0.68
                    - strike
                        * if kind == WeaponKind::Rapier {
                            0.5
                        } else {
                            0.12
                        },
            ))
            * Mat4::from_rotation_z((-0.22 + wind * 0.65 - strike * 1.2) * side)
            * Mat4::from_rotation_x(-0.3 + wind * 0.6 - strike * 0.45)
            * Mat4::from_scale(Vec3::splat(0.65));
        special_weapon(&mut m, kind, game.run.weapon.rarity, game.shot_age);
        glove(&mut m, Vec3::new(0., -0.19, 0.04), false);
    }
    m.transform = Mat4::IDENTITY;
    for p in &game.projectiles {
        m.ellipsoid(
            p.pos,
            Vec3::splat(if p.kind.spec().group == 3 {
                0.13
            } else {
                0.065
            }),
            p.kind.tint(),
            9.,
        );
        m.taper(
            p.pos,
            p.pos - p.vel.normalize_or_zero() * 0.32,
            0.04,
            0.,
            6,
            p.kind.tint(),
            9.,
        );
    }
    m.transform = Mat4::IDENTITY;
    let eye = if arena {
        game.run.pos
    } else if game.mode == Mode::Bestiary {
        Vec3::new(3.5, 2., 5.)
    } else {
        Vec3::new(4.5 + (game.elapsed * 0.04).sin() * 0.4, 2.5, 7.)
    };
    for p in FIRES {
        if optimized && !visibility.contains(Vec3::from_array(p), 1.2) {
            continue;
        }
        m.glow(
            Vec3::from_array(p) + Vec3::Y * 0.28,
            eye,
            0.84,
            [2.2, 0.56, 0.05],
        );
    }
}
pub const MAX_BODY_PIECES: usize = 144;

pub struct BodyPiece {
    pub handle: RigidBodyHandle,
    pub ttl: f32,
    pub part: Part,
    pub section: crate::dismemberment::Section,
    pub fractured: bool,
    /// Unique for the whole process, so resident GPU geometry can never be
    /// confused with a piece from an earlier `Bones`.
    pub id: u64,
    /// Body-space vertices; fixed once the piece exists.
    pub vertices: Vec<Vertex>,
    radius: f32,
    group: u64,
}
static NEXT_PIECE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
impl BodyPiece {
    /// Shrink away only at the end of the lifetime, after the body settles.
    fn fade(&self) -> f32 {
        (self.ttl / 1.2).clamp(0., 1.)
    }
    /// Body space to world: the rigid-body pose and the end-of-life shrink.
    pub fn model(&self, rotation: Quat, position: Vec3) -> Mat4 {
        Mat4::from_scale_rotation_translation(Vec3::splat(self.fade()), rotation, position)
    }
}
struct PlannedBody {
    section: crate::dismemberment::Section,
    vertices: Vec<Vertex>,
    center: Vec3,
    proximal: Vec3,
    distal: Vec3,
    capsules: Vec<(Vec3, Vec3, f32)>,
    hull: bool,
    fractured: bool,
}
struct RigBody {
    section: crate::dismemberment::Section,
    handle: RigidBodyHandle,
    center: Vec3,
    proximal: Vec3,
    distal: Vec3,
}
pub struct Bones {
    pub bodies: RigidBodySet,
    colliders: ColliderSet,
    pipeline: PhysicsPipeline,
    islands: IslandManager,
    broad: BroadPhaseMultiSap,
    narrow: NarrowPhase,
    joints: ImpulseJointSet,
    multi: MultibodyJointSet,
    ccd: CCDSolver,
    pub pieces: Vec<BodyPiece>,
    accum: f32,
    next_group: u64,
}
impl Bones {
    pub fn new() -> Self {
        let mut colliders = ColliderSet::new();
        colliders.insert(
            ColliderBuilder::cuboid(78., 0.1, 78.)
                .translation(Vector::new(0., -0.13, 0.))
                .friction(0.95)
                .build(),
        );
        for (index, (x, z, _)) in world_layout::PILLARS.into_iter().enumerate() {
            for band in world_layout::pillar_bands(index) {
                colliders.insert(
                    ColliderBuilder::cylinder((band.top - band.bottom) * 0.5, band.radius)
                        .translation(Vector::new(x, (band.top + band.bottom) * 0.5, z))
                        .build(),
                );
            }
        }
        for wall in world_layout::WALLS {
            colliders.insert(
                ColliderBuilder::cuboid(wall.half_x, wall.height * 0.5, wall.half_z)
                    .translation(Vector::new(wall.x, wall.height * 0.5, wall.z))
                    .build(),
            );
        }
        let edge = world_layout::HALF_EXTENT;
        for (x, z, hx, hz) in [
            (edge, 0., 0.3, edge),
            (-edge, 0., 0.3, edge),
            (0., edge, edge, 0.3),
            (0., -edge, edge, 0.3),
        ] {
            colliders.insert(
                ColliderBuilder::cuboid(hx, 3., hz)
                    .translation(Vector::new(x, 3., z))
                    .build(),
            );
        }
        Self {
            bodies: RigidBodySet::new(),
            colliders,
            pipeline: PhysicsPipeline::new(),
            islands: IslandManager::new(),
            broad: BroadPhaseMultiSap::new(),
            narrow: NarrowPhase::new(),
            joints: ImpulseJointSet::new(),
            multi: MultibodyJointSet::new(),
            ccd: CCDSolver::new(),
            pieces: vec![],
            accum: 0.,
            next_group: 0,
        }
    }
    pub fn joint_count(&self) -> usize {
        self.joints.len()
    }
    pub fn max_joint_anchor_error(&self) -> f32 {
        self.joints
            .iter()
            .map(|(_, joint)| {
                let a = self.bodies[joint.body1].position()
                    * Point::from(joint.data.local_frame1.translation.vector);
                let b = self.bodies[joint.body2].position()
                    * Point::from(joint.data.local_frame2.translation.vector);
                (a - b).norm()
            })
            .fold(0., f32::max)
    }

    pub fn spawn_body(&mut self, event: BodyEvent) {
        use crate::dismemberment::{self, Section};
        let enemy = &event.enemy;
        let pose = enemy
            .anatomy
            .death_pose
            .as_deref()
            .cloned()
            .unwrap_or_else(|| Pose::for_enemy(enemy, event.target));
        let sections =
            crate::enemy_assets::physical_sections(enemy.kind, enemy.phase, &pose, &enemy.anatomy);
        let fracture_section = if event.fracture {
            sections
                .iter()
                .filter(|s| {
                    matches!(s.section, Section::Torso | Section::Head)
                        && event.part.is_none_or(|part| s.part == part)
                })
                .min_by(|a, b| {
                    a.center
                        .distance_squared(event.impact)
                        .total_cmp(&b.center.distance_squared(event.impact))
                })
                .map(|s| s.section)
        } else {
            None
        };
        let mut planned = Vec::new();
        for mut section in sections {
            if event.part.is_some_and(|part| section.part != part) {
                continue;
            }
            if event.part == Some(section.part) && section.section.segment() == 0 {
                section
                    .vertices
                    .extend(dismemberment::sever_cap(&pose, section.part, true));
            }
            if fracture_section == Some(section.section) {
                let normal = if event.impulse.length_squared() > 0.001 {
                    event.impulse.normalize()
                } else {
                    Vec3::X
                };
                let fragments =
                    dismemberment::fracture(&section.vertices, event.impact, normal, event.energy);
                if fragments.len() > 1 {
                    for vertices in fragments {
                        let center = vertices
                            .iter()
                            .map(|v| Vec3::from_array(v.pos))
                            .sum::<Vec3>()
                            / vertices.len() as f32;
                        planned.push(PlannedBody {
                            section: section.section,
                            vertices,
                            center,
                            proximal: center,
                            distal: center,
                            capsules: vec![],
                            hull: true,
                            fractured: true,
                        });
                    }
                    continue;
                }
            }
            let all_segments = pose.segments(section.part);
            let mut capsules = if matches!(section.part, Part::Head | Part::Torso) {
                all_segments
            } else {
                vec![all_segments[section.section.segment()]]
            };
            // Keep the skirmisher's held blade and the warden's antlers off the floor.
            if section.section == Section::RightLowerArm && enemy.kind == 1 {
                let hand = pose.arms[1].tip;
                capsules.push((
                    pose.root.transform_point3(hand + Vec3::Y * 0.05),
                    pose.root.transform_point3(hand - Vec3::Y * 0.65),
                    0.035 * pose.scale,
                ));
            }
            if section.section == Section::Head && enemy.kind == 3 {
                for side in [-1., 1.] {
                    capsules.push((
                        pose.head_root.transform_point3(Vec3::new(
                            side * 0.14,
                            pose.head_y + 0.13,
                            -0.02,
                        )),
                        pose.head_root.transform_point3(Vec3::new(
                            side * 0.46,
                            pose.head_y + 0.59,
                            -0.1,
                        )),
                        0.035 * pose.scale,
                    ));
                }
            }
            // A single flying head can include broad wings or crawler limbs.
            // Its real posed hull prevents those appendages falling through terrain.
            let hull = crate::encounters::head_only(enemy.kind)
                || (section.section == Section::Torso && enemy.kind == 9);
            planned.push(PlannedBody {
                section: section.section,
                vertices: section.vertices,
                center: section.center,
                proximal: section.proximal,
                distal: section.distal,
                capsules,
                hull,
                fractured: false,
            });
        }
        if planned.is_empty() {
            return;
        }
        // Evict whole old ragdolls, avoiding partially vanished bodies and dangling joints.
        while self.pieces.len() + planned.len() > MAX_BODY_PIECES {
            let group = self.pieces[0].group;
            for index in (0..self.pieces.len()).rev() {
                if self.pieces[index].group == group {
                    self.remove_piece(index);
                }
            }
        }
        let group = self.next_group;
        self.next_group = self.next_group.wrapping_add(1);
        let first_piece = self.pieces.len();
        let mut rig = Vec::new();
        let inherited = event.impulse * if event.part.is_some() { 0.42 } else { 0.26 };
        for mut plan in planned {
            for vertex in &mut plan.vertices {
                vertex.pos = (Vec3::from_array(vertex.pos) - plan.center).to_array();
            }
            let radius = plan
                .vertices
                .iter()
                .map(|v| Vec3::from_array(v.pos).length())
                .fold(0.02f32, f32::max);
            let body = RigidBodyBuilder::dynamic()
                .translation(Vector::new(plan.center.x, plan.center.y, plan.center.z))
                .linvel(Vector::new(inherited.x, inherited.y, inherited.z))
                .linear_damping(0.28)
                .angular_damping(if plan.fractured { 1.25 } else { 1.7 })
                .additional_solver_iterations(2)
                .ccd_enabled(true)
                .build();
            let handle = self.bodies.insert(body);
            let density = match plan.section.part() {
                Part::Torso => 170.,
                Part::Head => 160.,
                Part::LeftLeg | Part::RightLeg => 230.,
                _ => 190.,
            };
            if plan.hull {
                let mut points: Vec<_> = plan
                    .vertices
                    .iter()
                    .step_by((plan.vertices.len() / 80).max(1))
                    .map(|v| Point::new(v.pos[0], v.pos[1], v.pos[2]))
                    .collect();
                // Include every extent even when decimating the hull's source points.
                for axis in 0..3 {
                    for sign in [-1., 1.] {
                        if let Some(v) = plan
                            .vertices
                            .iter()
                            .max_by(|a, b| (a.pos[axis] * sign).total_cmp(&(b.pos[axis] * sign)))
                        {
                            points.push(Point::new(v.pos[0], v.pos[1], v.pos[2]));
                        }
                    }
                }
                let shape = ColliderBuilder::convex_hull(&points)
                    .unwrap_or_else(|| ColliderBuilder::ball(radius.max(0.025)));
                self.colliders.insert_with_parent(
                    shape
                        .density(density * 0.7)
                        .restitution(0.08)
                        .friction(0.95)
                        .build(),
                    handle,
                    &mut self.bodies,
                );
            } else {
                for (a, b, r) in plan.capsules {
                    let a = a - plan.center;
                    let b = b - plan.center;
                    self.colliders.insert_with_parent(
                        ColliderBuilder::new(SharedShape::capsule(
                            Point::new(a.x, a.y, a.z),
                            Point::new(b.x, b.y, b.z),
                            r * 0.85,
                        ))
                        .density(density)
                        .restitution(0.08)
                        .friction(0.95)
                        .build(),
                        handle,
                        &mut self.bodies,
                    );
                }
            }
            self.pieces.push(BodyPiece {
                handle,
                ttl: 18.,
                part: plan.section.part(),
                section: plan.section,
                fractured: plan.fractured,
                id: NEXT_PIECE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                vertices: plan.vertices,
                radius,
                group,
            });
            if !plan.fractured {
                rig.push(RigBody {
                    section: plan.section,
                    handle,
                    center: plan.center,
                    proximal: plan.proximal,
                    distal: plan.distal,
                });
            }
        }
        self.connect_rig(&rig, &pose, event.part.is_none());
        // Transfer the impact at its real contact point. Nearby pieces inherit
        // momentum through joints rather than sharing an arbitrary spin vector.
        // A severed hit already drove its detached piece. Applying that same
        // contact at the now-empty neck/shoulder would somersault the remainder.
        let detached_impact = event.part.is_none()
            && enemy
                .anatomy
                .impact_part
                .is_some_and(|part| enemy.anatomy.missing(part));
        if let Some(index) = (first_piece..self.pieces.len())
            .filter(|_| !detached_impact)
            .min_by(|&a, &b| {
                let distance = |i: usize| {
                    let p = self.bodies[self.pieces[i].handle].translation();
                    Vec3::new(p.x, p.y, p.z).distance_squared(event.impact)
                };
                distance(a).total_cmp(&distance(b))
            })
        {
            let body = &mut self.bodies[self.pieces[index].handle];
            let power = (0.7 + event.energy.max(0.) / 160.).clamp(0.7, 2.2);
            let impulse = event.impulse * body.mass() * power;
            body.apply_impulse_at_point(
                Vector::new(impulse.x, impulse.y, impulse.z),
                Point::new(event.impact.x, event.impact.y, event.impact.z),
                true,
            );
        }
        for piece in &self.pieces[first_piece..] {
            if piece.fractured {
                let body = &mut self.bodies[piece.handle];
                let p = body.translation();
                let outward = (Vec3::new(p.x, p.y, p.z) - event.impact).normalize_or_zero();
                let speed = (event.energy * 0.012).clamp(0.8, 4.);
                let impulse = outward * body.mass() * speed;
                body.apply_impulse(Vector::new(impulse.x, impulse.y, impulse.z), true);
            }
        }
    }
    /// Query only the bounded physical remains; intact enemies use game hit tests.
    /// Static cover limits the ray before it can wake a corpse behind masonry.
    pub fn impact_ray(&mut self, origin: Vec3, direction: Vec3, range: f32, energy: f32) {
        let direction = direction.normalize_or_zero();
        if direction.length_squared() < 0.1 || range <= 0. || energy <= 0. {
            return;
        }
        let range = world_layout::obstruction(origin, origin + direction * range).unwrap_or(range);
        let ray = Ray::new(
            Point::new(origin.x, origin.y, origin.z),
            Vector::new(direction.x, direction.y, direction.z),
        );
        let hit = self
            .pieces
            .iter()
            .filter_map(|piece| {
                let body = self.bodies.get(piece.handle)?;
                let distance = body
                    .colliders()
                    .iter()
                    .filter_map(|handle| {
                        let collider = self.colliders.get(*handle)?;
                        collider
                            .shape()
                            .cast_ray(collider.position(), &ray, range, true)
                    })
                    .min_by(f32::total_cmp)?;
                Some((piece.handle, distance))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((handle, distance)) = hit {
            let point = origin + direction * distance;
            let body = &mut self.bodies[handle];
            let impulse = direction * body.mass() * (energy * 0.035).clamp(0.35, 10.);
            body.apply_impulse_at_point(
                Vector::new(impulse.x, impulse.y, impulse.z),
                Point::new(point.x, point.y, point.z),
                true,
            );
        }
    }
    pub fn impact_blast(&mut self, center: Vec3, radius: f32, energy: f32) {
        if radius <= 0. || energy <= 0. {
            return;
        }
        for piece in &self.pieces {
            let body = &mut self.bodies[piece.handle];
            let position = body.translation();
            let position = Vec3::new(position.x, position.y, position.z);
            let offset = position - center;
            let distance = offset.length();
            if distance >= radius || world_layout::obstruction(center, position).is_some() {
                continue;
            }
            let direction = if distance > 0.01 {
                offset / distance
            } else {
                Vec3::Y
            };
            let impulse = (direction + Vec3::Y * 0.18)
                * body.mass()
                * (energy * 0.06).clamp(0.5, 13.)
                * (1. - distance / radius);
            // Pressure acts on this piece's facing surface, not at the remote
            // explosion centre. A remote contact gives tiny limbs an enormous
            // artificial torque arm from the small upward component.
            let ray = Ray::new(
                Point::new(center.x, center.y, center.z),
                Vector::new(direction.x, direction.y, direction.z),
            );
            let contact = body
                .colliders()
                .iter()
                .filter_map(|handle| {
                    let collider = self.colliders.get(*handle)?;
                    collider
                        .shape()
                        .cast_ray(collider.position(), &ray, distance, true)
                })
                .min_by(f32::total_cmp)
                .map_or(position, |along| center + direction * along);
            body.apply_impulse_at_point(
                Vector::new(impulse.x, impulse.y, impulse.z),
                Point::new(contact.x, contact.y, contact.z),
                true,
            );
        }
    }
    fn connect_rig(&mut self, rig: &[RigBody], pose: &Pose, whole_body: bool) {
        use crate::dismemberment::Section;
        let local = |point: Vec3, center: Vec3| {
            let p = point - center;
            Point::new(p.x, p.y, p.z)
        };
        if whole_body {
            if let Some(torso) = rig.iter().find(|body| body.section == Section::Torso) {
                for body in rig
                    .iter()
                    .filter(|body| body.section != Section::Torso && body.section.segment() == 0)
                {
                    let anchor = if body.section == Section::Head {
                        pose.anchor(Part::Head)
                    } else {
                        body.proximal
                    };
                    let (bend, twist, roll) = match body.section.part() {
                        Part::Head => (0.65, 0.75, 0.5),
                        Part::LeftArm | Part::RightArm => (1.5, 0.8, 1.3),
                        _ => (1.2, 0.5, 0.65),
                    };
                    let q = pose.root.to_scale_rotation_translation().1;
                    let orientation = Rotation::from_quaternion(rapier3d::na::Quaternion::new(
                        q.w, q.x, q.y, q.z,
                    ));
                    let frame = |p: Point<Real>| {
                        Isometry::from_parts(Translation::new(p.x, p.y, p.z), orientation)
                    };
                    let joint = SphericalJointBuilder::new()
                        .local_frame1(frame(local(anchor, torso.center)))
                        .local_frame2(frame(local(anchor, body.center)))
                        .limits(JointAxis::AngX, [-bend, bend])
                        .limits(JointAxis::AngY, [-twist, twist])
                        .limits(JointAxis::AngZ, [-roll, roll])
                        .contacts_enabled(false);
                    self.joints.insert(torso.handle, body.handle, joint, true);
                }
            }
        }
        for part in [Part::LeftArm, Part::RightArm, Part::LeftLeg, Part::RightLeg] {
            let upper = rig
                .iter()
                .find(|body| body.section.part() == part && body.section.segment() == 0);
            let lower = rig
                .iter()
                .find(|body| body.section.part() == part && body.section.segment() == 1);
            if let (Some(upper), Some(lower)) = (upper, lower) {
                let a = (upper.distal - upper.proximal).normalize_or_zero();
                let b = (lower.distal - lower.proximal).normalize_or_zero();
                let initial = a.dot(b).clamp(-1., 1.).acos();
                let axis = if a.cross(b).length_squared() > 0.0001 {
                    a.cross(b).normalize()
                } else {
                    pose.root.transform_vector3(Vec3::X).normalize()
                };
                let max_flexion = if matches!(part, Part::LeftLeg | Part::RightLeg) {
                    2.25
                } else {
                    2.6
                };
                let joint = RevoluteJointBuilder::new(rapier3d::na::Unit::new_normalize(
                    Vector::new(axis.x, axis.y, axis.z),
                ))
                .local_anchor1(local(upper.distal, upper.center))
                .local_anchor2(local(upper.distal, lower.center))
                .limits([-initial - 0.06, (max_flexion - initial).max(0.06)])
                .contacts_enabled(false);
                self.joints.insert(upper.handle, lower.handle, joint, true);
            }
        }
    }
    fn remove_piece(&mut self, index: usize) {
        let piece = self.pieces.remove(index);
        self.bodies.remove(
            piece.handle,
            &mut self.islands,
            &mut self.colliders,
            &mut self.joints,
            &mut self.multi,
            true,
        );
    }
    pub fn update(&mut self, dt: f32) {
        // Fixed steps retain joint stability; bounded catch-up avoids a physics spiral.
        self.accum = (self.accum + dt.max(0.).min(0.1)).min(0.1);
        let parameters = IntegrationParameters {
            // Half-frame steps prevent strong blast impulses stretching small
            // elbow/knee constraints before the solver can correct them.
            dt: 1. / 120.,
            num_solver_iterations: std::num::NonZeroUsize::new(6).unwrap(),
            ..Default::default()
        };
        while self.accum >= parameters.dt {
            self.pipeline.step(
                &Vector::new(0., -9.81, 0.),
                &parameters,
                &mut self.islands,
                &mut self.broad,
                &mut self.narrow,
                &mut self.bodies,
                &mut self.colliders,
                &mut self.joints,
                &mut self.multi,
                &mut self.ccd,
                None,
                &(),
                &(),
            );
            self.accum -= parameters.dt;
        }
        for piece in &mut self.pieces {
            piece.ttl -= dt.max(0.);
        }
        for i in (0..self.pieces.len()).rev() {
            if self.pieces[i].ttl <= 0. {
                self.remove_piece(i);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dismemberment::Section;
    fn corpse() -> BodyEvent {
        let mut g = Game::new(false);
        g.new_run();
        let mut e = g.run.enemies[1].clone();
        e.pos = Vec3::ZERO;
        e.kind = 0;
        e.phase = 0.;
        e.attack = 10.;
        BodyEvent {
            enemy: e,
            target: Vec3::new(0., 1.65, 4.),
            part: None,
            impulse: Vec3::new(0., 0., -4.),
            impact: Vec3::new(0.15, 1.25, 0.),
            energy: 45.,
            fracture: false,
        }
    }
    #[test]
    fn articulated_corpses_exclude_severed_parts_collide_and_expire() {
        let mut bones = Bones::new();
        let mut event = corpse();
        event.enemy.anatomy.severed = 1 << Part::Head as usize;
        bones.spawn_body(event);
        assert_eq!(bones.pieces.len(), 9);
        assert_eq!(bones.joints.len(), 8);
        assert!(
            bones
                .pieces
                .iter()
                .all(|p| p.part != Part::Head && p.vertices.len() > 30)
        );
        for _ in 0..600 {
            bones.update(1. / 60.);
        }
        for p in &bones.pieces {
            let b = &bones.bodies[p.handle];
            assert!(b.translation().iter().all(|v| v.is_finite()));
            assert!(
                b.translation().y > -0.3 && b.translation().y < 1.,
                "ragdoll must settle on floor: {:?}",
                b.translation()
            );
            assert!(b.linvel().norm() < 0.2, "corpse should stop sliding");
        }
        assert!(
            bones
                .pieces
                .iter()
                .any(|p| bones.bodies[p.handle].is_sleeping()),
            "settled bodies should sleep"
        );
        for _ in 0..510 {
            bones.update(1. / 60.);
        }
        assert!(bones.pieces.is_empty());
        assert_eq!(bones.bodies.len(), 0);
        assert_eq!(bones.joints.len(), 0);
    }
    #[test]
    fn detached_skull_retains_geometry_and_physics_budget_is_bounded() {
        let mut bones = Bones::new();
        let mut event = corpse();
        event.part = Some(Part::Head);
        bones.spawn_body(event);
        assert_eq!(bones.pieces.len(), 1);
        assert_eq!(bones.joints.len(), 0);
        assert_eq!(bones.pieces[0].part, Part::Head);
        assert!(bones.pieces[0].vertices.len() > 300);
        for _ in 0..30 {
            bones.spawn_body(corpse());
        }
        assert!(bones.pieces.len() <= MAX_BODY_PIECES);
        assert_eq!(bones.bodies.len(), bones.pieces.len());
        assert!(bones.joints.len() <= bones.pieces.len());
    }
    #[test]
    fn severed_arm_retains_an_elbow_and_impact_produces_rotation() {
        let mut bones = Bones::new();
        let mut event = corpse();
        event.part = Some(Part::RightArm);
        event.impact = Vec3::new(0.52, 1.06, 0.12);
        event.impulse = Vec3::new(5., 0., -4.);
        bones.spawn_body(event);
        assert_eq!(bones.pieces.len(), 2);
        assert_eq!(bones.joints.len(), 1);
        assert!(
            bones
                .pieces
                .iter()
                .any(|p| p.section == Section::RightUpperArm)
        );
        assert!(
            bones
                .pieces
                .iter()
                .any(|p| p.section == Section::RightLowerArm)
        );
        assert!(
            bones
                .pieces
                .iter()
                .any(|p| bones.bodies[p.handle].angvel().norm() > 0.1)
        );
        for _ in 0..30 {
            bones.update(1. / 60.);
        }
        let (_, joint) = bones.joints.iter().next().unwrap();
        let a = &bones.bodies[joint.body1];
        let b = &bones.bodies[joint.body2];
        let gap = bones.max_joint_anchor_error();
        assert!(gap < 0.03, "detached elbow stretched {gap}");
        let relative = a.rotation().inverse() * b.rotation();
        assert!(relative.angle() > 0.04, "elbow should bend independently");
    }
    #[test]
    fn whole_rig_has_hinged_knees_and_elbows_with_finite_limits() {
        let mut bones = Bones::new();
        bones.spawn_body(corpse());
        assert_eq!(bones.pieces.len(), 10);
        assert_eq!(bones.joints.len(), 9);
        let hinges = bones
            .joints
            .iter()
            .filter(|(_, j)| j.data.as_revolute().is_some())
            .count();
        assert_eq!(hinges, 4);
        for (_, joint) in bones.joints.iter() {
            assert!(joint.data.limits(JointAxis::AngX).is_some());
        }
        for _ in 0..120 {
            bones.update(1. / 60.);
        }
        for (_, joint) in bones.joints.iter() {
            if let Some(hinge) = joint.data.as_revolute() {
                let angle = hinge.angle(
                    bones.bodies[joint.body1].rotation(),
                    bones.bodies[joint.body2].rotation(),
                );
                let limits = hinge.limits().unwrap();
                assert!(
                    angle >= limits.min - 0.08 && angle <= limits.max + 0.08,
                    "hinge escaped limit: {angle}"
                );
            }
        }
    }
    #[test]
    fn later_shots_and_blasts_react_to_remains_and_respect_cover() {
        let mut bones = Bones::new();
        let mut event = corpse();
        event.part = Some(Part::Head);
        event.impulse = Vec3::ZERO;
        event.enemy.pos = Vec3::new(-21.2, 0., -14.);
        event.target = Vec3::new(-21.2, 1.65, -10.);
        bones.spawn_body(event);
        bones.update(1. / 60.);
        let handle = bones.pieces[0].handle;
        let p = bones.bodies[handle].translation();
        let position = Vec3::new(p.x, p.y, p.z);
        let before = *bones.bodies[handle].linvel();
        let blocked_origin = Vec3::new(-18.8, position.y, -14.);
        bones.impact_ray(blocked_origin, -Vec3::X, 6., 100.);
        bones.impact_blast(blocked_origin, 6., 100.);
        assert_eq!(
            *bones.bodies[handle].linvel(),
            before,
            "masonry must shield existing corpses"
        );
        bones.impact_ray(position + Vec3::Z * 3., -Vec3::Z, 6., 100.);
        assert!(
            bones.bodies[handle].linvel().z < before.z - 1.,
            "shot must transfer momentum to remains"
        );
        let before = *bones.bodies[handle].linvel();
        bones.impact_blast(position - Vec3::X * 2., 5., 100.);
        assert!(
            bones.bodies[handle].linvel().x > before.x + 1.,
            "nearby blast must move remains"
        );
        assert_eq!(
            bones.pieces.len(),
            1,
            "secondary impacts cannot duplicate bodies"
        );
    }
    #[test]
    fn all_creature_rigs_remain_finite_and_fall_onto_the_world() {
        for kind in 0..12 {
            let mut bones = Bones::new();
            let mut event = corpse();
            event.enemy.kind = kind;
            event.enemy.phase = 0.42;
            bones.spawn_body(event);
            let expected = if crate::encounters::head_only(kind) {
                1
            } else {
                10
            };
            assert_eq!(bones.pieces.len(), expected, "kind {kind}");
            for _ in 0..240 {
                bones.update(1. / 60.);
            }
            assert!(
                bones.max_joint_anchor_error() < 0.08,
                "kind {kind} joints stretched"
            );
            for piece in &bones.pieces {
                let body = &bones.bodies[piece.handle];
                assert!(
                    body.translation().iter().all(|v| v.is_finite()),
                    "kind {kind}"
                );
                assert!(
                    body.translation().y > -0.5,
                    "kind {kind} tunneled through floor"
                );
                assert!(body.linvel().norm() < 20., "kind {kind} unstable velocity");
            }
        }
    }
    #[test]
    fn fatal_decapitation_impulse_is_not_applied_again_at_the_empty_neck() {
        let mut bones = Bones::new();
        let mut event = corpse();
        event.enemy.anatomy.severed = 1 << Part::Head as usize;
        event.enemy.anatomy.impact_part = Some(Part::Head);
        event.impact = Vec3::new(0.12, 1.79, 0.15);
        event.impulse = Vec3::new(0., 0., -8.);
        event.energy = 60.;
        bones.spawn_body(event.clone());
        for piece in &bones.pieces {
            let body = &bones.bodies[piece.handle];
            assert!(
                body.angvel().norm() < 0.00001,
                "empty neck must not torque another part"
            );
            assert!((body.linvel().z + 8. * 0.26).abs() < 0.00001);
        }
        let mut detached = Bones::new();
        event.enemy.anatomy.severed = 0;
        event.part = Some(Part::Head);
        detached.spawn_body(event);
        assert!(
            detached.bodies[detached.pieces[0].handle].angvel().norm() > 0.1,
            "the actual detached skull still receives its off-centre impact"
        );
    }
    #[test]
    fn blast_impulse_uses_body_surface_and_keeps_articulated_joints_together() {
        let mut bones = Bones::new();
        let mut event = corpse();
        event.impulse = Vec3::ZERO;
        bones.spawn_body(event);
        for _ in 0..180 {
            bones.update(1. / 60.);
        }
        bones.impact_blast(Vec3::new(2., 0.3, 0.), 5., 240.);
        let mut max_gap = 0f32;
        for _ in 0..120 {
            bones.update(1. / 60.);
            max_gap = max_gap.max(bones.max_joint_anchor_error());
        }
        assert!(max_gap < 0.06, "blast stretched joint anchors {max_gap}");
        assert!(
            bones
                .pieces
                .iter()
                .all(|piece| bones.bodies[piece.handle].angvel().norm() < 8.)
        );
    }
    #[test]
    fn high_energy_fractures_replace_one_section_with_real_physics_meshes() {
        let mut bones = Bones::new();
        let mut event = corpse();
        event.fracture = true;
        event.energy = 240.;
        event.impact = Vec3::new(0., 1.13, 0.);
        bones.spawn_body(event);
        let fractured: Vec<_> = bones.pieces.iter().filter(|p| p.fractured).collect();
        assert!((2..=4).contains(&fractured.len()));
        assert!(fractured.iter().all(|p| p.part == Part::Torso));
        assert!(
            !bones
                .pieces
                .iter()
                .any(|p| p.part == Part::Torso && !p.fractured)
        );
        assert_eq!(
            bones.joints.len(),
            4,
            "remaining limbs keep their elbows/knees"
        );
        for _ in 0..180 {
            bones.update(1. / 60.);
        }
        assert!(bones.pieces.iter().all(|p| {
            bones.bodies[p.handle]
                .translation()
                .iter()
                .all(|v| v.is_finite())
        }));
    }
}
