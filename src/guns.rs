//! Sculpted firearm assemblies. Separate receiver, moving action and barrel group
//! preserve reload articulation; all surfaces carry object-space material coordinates.
use crate::{
    scene::{Mesh, Vertex},
    weapons::{Model, WeaponKind},
};
use glam::{Mat4, Vec3};
use std::f32::consts::{PI, TAU};
const STEEL: [f32; 3] = [0.027, 0.042, 0.059];
const EDGE: [f32; 3] = [0.19, 0.22, 0.24];
const GOLD: [f32; 3] = [0.58, 0.32, 0.10];
const WOOD: [f32; 3] = [0.17, 0.044, 0.017];
const DARK: [f32; 3] = [0.035, 0.028, 0.022];

fn vertex(m: &mut Mesh, p: Vec3, n: Vec3, c: [f32; 3], mat: f32) {
    m.vertices.push(Vertex {
        pos: m.transform.transform_point3(p).to_array(),
        normal: m
            .transform
            .transform_vector3(n)
            .normalize_or_zero()
            .to_array(),
        color: c,
        material: mat,
        local: p.to_array(),
        wear_uv: {
            let a = n.abs();
            let uv = if a.y > a.x && a.y > a.z {
                [p.x, p.z]
            } else if a.x > a.z {
                [p.z, p.y]
            } else {
                [p.x, p.y]
            };
            [uv[0] * 2.7, uv[1] * 2.7]
        },
    });
}
fn patch(m: &mut Mesh, p: [Vec3; 4], n: [Vec3; 4], c: [f32; 3], mat: f32) {
    for i in [0, 1, 2, 0, 2, 3] {
        vertex(m, p[i], n[i], c, mat);
    }
}
pub fn ellipsoid(m: &mut Mesh, p: Vec3, r: Vec3, c: [f32; 3], mat: f32) {
    for j in 0..12 {
        for i in 0..24 {
            let point = |j: usize, i: usize| {
                let a = PI * j as f32 / 12.;
                let b = TAU * i as f32 / 24.;
                let unit = Vec3::new(a.sin() * b.cos(), a.cos(), a.sin() * b.sin());
                (p + unit * r, (unit / r).normalize())
            };
            let a = point(j, i);
            let b = point(j + 1, i);
            let c1 = point(j + 1, i + 1);
            let d = point(j, i + 1);
            patch(m, [a.0, b.0, c1.0, d.0], [a.1, b.1, c1.1, d.1], c, mat);
        }
    }
}
/// Rounded box with planar broad faces and continuous normals around its bevels.
pub fn bevel(m: &mut Mesh, p: Vec3, size: Vec3, radius: f32, c: [f32; 3], mat: f32) {
    let half = size * 0.5;
    let r = radius.min(half.min_element() * 0.95);
    let core = half - Vec3::splat(r);
    for axis in 0..3 {
        let u = (axis + 1) % 3;
        let v = (axis + 2) % 3;
        let coords = |a: usize| {
            [
                -half[a],
                -core[a] - r * 0.707,
                -core[a],
                core[a],
                core[a] + r * 0.707,
                half[a],
            ]
        };
        let us = coords(u);
        let vs = coords(v);
        for sign in [-1., 1.] {
            let point = |i: usize, j: usize| {
                let mut q = Vec3::ZERO;
                q[axis] = sign * half[axis];
                q[u] = us[i];
                q[v] = vs[j];
                let closest = q.clamp(-core, core);
                let n = (q - closest).normalize();
                (p + closest + n * r, n)
            };
            for i in 0..5 {
                for j in 0..5 {
                    let a = point(i, j);
                    let b = point(i + 1, j);
                    let d = point(i, j + 1);
                    let e = point(i + 1, j + 1);
                    patch(m, [a.0, b.0, e.0, d.0], [a.1, b.1, e.1, d.1], c, mat);
                }
            }
        }
    }
}
/// Surface of revolution including the lip and deep inner wall of a real bore.
fn lathe(m: &mut Mesh, a: Vec3, b: Vec3, profile: &[(f32, f32)], c: [f32; 3], mat: f32) {
    let axis = (b - a).normalize();
    let u = axis
        .cross(if axis.y.abs() < 0.9 { Vec3::Y } else { Vec3::X })
        .normalize();
    let v = axis.cross(u);
    for k in 0..profile.len() - 1 {
        let (z0, r0) = profile[k];
        let (z1, r1) = profile[k + 1];
        // Each profile span has its own axial normal; radial normals remain smooth.
        let slope = (r0 - r1) / (z1 - z0).abs().max(0.00001);
        let flat = (z1 - z0).abs() < 0.00001;
        for i in 0..32 {
            let t = i as f32 * TAU / 32.;
            let t1 = (i + 1) as f32 * TAU / 32.;
            let radial = u * t.cos() + v * t.sin();
            let next = u * t1.cos() + v * t1.sin();
            let inner = z1 < z0;
            let normal = |r: Vec3| {
                if flat {
                    axis * if r1 < r0 { 1. } else { -1. }
                } else {
                    (r * if inner { -1. } else { 1. } + axis * slope).normalize()
                }
            };
            patch(
                m,
                [
                    a + axis * z0 + radial * r0,
                    a + axis * z0 + next * r0,
                    a + axis * z1 + next * r1,
                    a + axis * z1 + radial * r1,
                ],
                [normal(radial), normal(next), normal(next), normal(radial)],
                c,
                mat,
            );
        }
    }
}
fn barrel(m: &mut Mesh, x: f32, y: f32, start: f32, end: f32, r: f32) {
    let len = start - end;
    lathe(
        m,
        Vec3::new(x, y, start),
        Vec3::new(x, y, end),
        &[
            (0., r * 1.08),
            (0.018, r * 1.12),
            (0.055, r),
            (len - 0.035, r * 0.88),
            (len - 0.008, r * 0.91),
            (len, r * 0.87),
            (len, r * 0.65),
        ],
        STEEL,
        11.,
    );
    lathe(
        m,
        Vec3::new(x, y, start),
        Vec3::new(x, y, end),
        &[
            (len, r * 0.65),
            (len - 0.012, r * 0.63),
            (len - 0.19, r * 0.63),
            (len - 0.19, 0.),
        ],
        [0.004, 0.006, 0.008],
        14.,
    );
    for z in [start - 0.05, end + 0.018] {
        collar(m, Vec3::new(x, y, z), r * 1.02, 0.018, GOLD);
    }
}
fn collar(m: &mut Mesh, p: Vec3, r: f32, len: f32, c: [f32; 3]) {
    lathe(
        m,
        p,
        p - Vec3::Z,
        &[
            (0., r * 0.94),
            (0.003, r),
            (len - 0.003, r),
            (len, r * 0.94),
            (len, r * 0.87),
        ],
        c,
        12.,
    );
}
/// Elliptical sections produce a shaped comb, neck and heel, instead of a block stock.
fn loft(m: &mut Mesh, rings: &[(Vec3, f32, f32)], up: Vec3, c: [f32; 3], mat: f32) {
    let axis = (rings[rings.len() - 1].0 - rings[0].0).normalize();
    let right = up.cross(axis).normalize();
    let vertical = axis.cross(right);
    for k in 0..rings.len() - 1 {
        for i in 0..24 {
            let a = i as f32 * TAU / 24.;
            let b = (i + 1) as f32 * TAU / 24.;
            let pt = |k: usize, t: f32| {
                let (p, w, h) = rings[k];
                p + right * t.cos() * w + vertical * t.sin() * h
            };
            let n = |k: usize, t: f32| {
                (right * t.cos() / rings[k].1.max(0.001)
                    + vertical * t.sin() / rings[k].2.max(0.001))
                .normalize()
            };
            patch(
                m,
                [pt(k, a), pt(k, b), pt(k + 1, b), pt(k + 1, a)],
                [n(k, a), n(k, b), n(k + 1, b), n(k + 1, a)],
                c,
                mat,
            );
        }
    }
}
fn wire(m: &mut Mesh, points: &[Vec3], r: f32, c: [f32; 3], mat: f32) {
    for p in points.windows(2) {
        let len = p[0].distance(p[1]);
        lathe(m, p[0], p[1], &[(0., r), (len, r)], c, mat);
    }
}
fn screw(m: &mut Mesh, p: Vec3, side: f32, r: f32) {
    lathe(
        m,
        p,
        p + Vec3::X * side,
        &[(0., r * 0.83), (0.002, r), (0.006, r * 0.85), (0.008, 0.)],
        EDGE,
        11.,
    );
    bevel(
        m,
        p + Vec3::X * side * 0.008,
        Vec3::new(0.002, r * 0.24, r * 1.45),
        0.001,
        DARK,
        14.,
    );
}
fn grip(m: &mut Mesh) {
    loft(
        m,
        &[
            (Vec3::new(0., -0.035, 0.14), 0.056, 0.060),
            (Vec3::new(0., -0.08, 0.16), 0.072, 0.075),
            (Vec3::new(0., -0.17, 0.195), 0.069, 0.075),
            (Vec3::new(0., -0.29, 0.23), 0.072, 0.078),
            (Vec3::new(0., -0.335, 0.225), 0.065, 0.061),
            (Vec3::new(0., -0.342, 0.225), 0.008, 0.008),
        ],
        Vec3::Z,
        WOOD,
        13.,
    );
    for side in [-1., 1.] {
        screw(m, Vec3::new(side * 0.071, -0.18, 0.205), side, 0.012);
        for i in 0..8 {
            let y = -0.105 - i as f32 * 0.022;
            wire(
                m,
                &[
                    Vec3::new(side * 0.064, y, 0.16),
                    Vec3::new(side * 0.070, y - 0.035, 0.205),
                    Vec3::new(side * 0.064, y - 0.024, 0.255),
                ],
                0.0018,
                DARK,
                14.,
            );
        }
    }
}
fn stock(m: &mut Mesh, short: bool) {
    let length = if short { 0.43 } else { 0.66 };
    loft(
        m,
        &[
            (Vec3::new(0., -0.015, 0.14), 0.065, 0.065),
            (Vec3::new(0., -0.040, 0.24), 0.060, 0.065),
            (Vec3::new(0., -0.085, 0.32), 0.070, 0.090),
            (Vec3::new(0., -0.095, length), 0.088, 0.158),
            (Vec3::new(0., -0.095, length + 0.025), 0.090, 0.16),
            (Vec3::new(0., -0.095, length + 0.04), 0.077, 0.148),
            (Vec3::new(0., -0.095, length + 0.042), 0.001, 0.001),
        ],
        Vec3::Y,
        WOOD,
        13.,
    );
    bevel(
        m,
        Vec3::new(0., -0.095, length + 0.032),
        Vec3::new(0.179, 0.309, 0.025),
        0.012,
        DARK,
        14.,
    );
    for side in [-1., 1.] {
        screw(m, Vec3::new(side * 0.073, -0.07, 0.35), side, 0.012);
        // Inlaid scroll at the stock neck, intentionally subtle against the grain.
        for i in 0..4 {
            let z = 0.265 + i as f32 * 0.015;
            wire(
                m,
                &[
                    Vec3::new(side * 0.062, -0.038, z),
                    Vec3::new(side * 0.071, -0.100, z + 0.025),
                ],
                0.0018,
                GOLD,
                12.,
            );
        }
    }
}
fn engraving(m: &mut Mesh, x: f32, y: f32, z: f32, side: f32) {
    for flip in [-1., 1.] {
        let points = (0..=28)
            .map(|i| {
                let t = i as f32 / 28.;
                let a = t * TAU * 1.35;
                let r = (1. - t) * 0.045;
                Vec3::new(x, y + a.sin() * r, z + flip * (0.045 + a.cos() * r))
            })
            .collect::<Vec<_>>();
        wire(m, &points, 0.0017, GOLD, 12.);
    }
    for zz in [z - 0.096, z + 0.096] {
        screw(m, Vec3::new(x, y, zz), side, 0.009);
    }
}
fn sights(m: &mut Mesh, end: f32, height: f32) {
    bevel(
        m,
        Vec3::new(0., height, end + 0.065),
        Vec3::new(0.023, 0.042, 0.032),
        0.006,
        EDGE,
        11.,
    );
    bevel(
        m,
        Vec3::new(0., height + 0.021, end + 0.065),
        Vec3::new(0.009, 0.008, 0.021),
        0.003,
        GOLD,
        12.,
    );
}

pub fn muzzle(kind: WeaponKind) -> f32 {
    match kind {
        WeaponKind::Longrifle => -1.18,
        WeaponKind::Duelist => -0.73,
        WeaponKind::Blunderbuss => -0.87,
        WeaponKind::Gatling => -0.96,
        WeaponKind::Mortar => -0.82,
        WeaponKind::HandCannon => -0.55,
        _ => {
            if kind.spec().group == 0 {
                -0.52
            } else {
                -0.79
            }
        }
    }
}
pub fn firearm(m: &mut Mesh, kind: WeaponKind, rarity: usize, reload: Option<f32>, age: f32) {
    if crate::weapon_assets::draw(m, kind, rarity, reload, age) {
        return;
    }
    let root = m.transform;
    let pose = crate::motion::reload_pose(kind, reload.unwrap_or(0.));
    let model = kind.spec().model;
    let shotgun = model == Model::Shotgun;
    let revolver = model == Model::Revolver;
    let cannon = model == Model::Cannon;
    let long = kind.spec().group == 2
        || shotgun
        || kind == WeaponKind::Mortar
        || kind == WeaponKind::GrenadeLauncher;
    let end = muzzle(kind);
    let hinge = Mat4::from_translation(Vec3::new(0., -0.02, 0.04))
        * Mat4::from_rotation_x(-pose.hinge * 0.92)
        * Mat4::from_translation(Vec3::new(0., 0.02, -0.04));
    if long {
        stock(m, kind == WeaponKind::Coachgun || cannon);
    } else {
        grip(m);
    }
    // Forged receiver tapers at the breech; side locks carry inset scroll engraving.
    if !revolver {
        bevel(
            m,
            Vec3::new(0., 0., 0.045),
            Vec3::new(if shotgun { 0.235 } else { 0.19 }, 0.165, 0.29),
            0.034,
            STEEL,
            11.,
        );
        for side in [-1., 1.] {
            let x = side * if shotgun { 0.118 } else { 0.096 };
            bevel(
                m,
                Vec3::new(x, -0.01, 0.045),
                Vec3::new(0.01, 0.105, 0.245),
                0.004,
                GOLD,
                12.,
            );
            bevel(
                m,
                Vec3::new(x + side * 0.006, -0.01, 0.045),
                Vec3::new(0.008, 0.088, 0.228),
                0.003,
                STEEL,
                11.,
            );
            engraving(m, x + side * 0.011, -0.008, 0.045, side);
        }
    } else {
        // Open frame exposes the cylinder; no solid receiver obscuring it.
        bevel(
            m,
            Vec3::new(0., 0.155, 0.015),
            Vec3::new(0.085, 0.039, 0.32),
            0.013,
            STEEL,
            11.,
        );
        bevel(
            m,
            Vec3::new(0., -0.085, 0.025),
            Vec3::new(0.1, 0.04, 0.30),
            0.013,
            STEEL,
            11.,
        );
        bevel(
            m,
            Vec3::new(0., 0.028, 0.16),
            Vec3::new(0.16, 0.24, 0.075),
            0.023,
            STEEL,
            11.,
        );
        m.transform = root
            * Mat4::from_translation(Vec3::new(-pose.magazine * 0.18, 0.026, 0.))
            * Mat4::from_rotation_z(crate::motion::window(0., 0.025, 0.10, 0.19, age) * PI / 3.);
        lathe(
            m,
            Vec3::new(0., 0., 0.12),
            -Vec3::Z,
            &[
                (0., 0.10),
                (0.012, 0.124),
                (0.032, 0.126),
                (0.21, 0.126),
                (0.225, 0.11),
            ],
            STEEL,
            11.,
        );
        for i in 0..6 {
            let a = i as f32 * TAU / 6.;
            let x = a.cos() * 0.090;
            let y = a.sin() * 0.090;
            lathe(
                m,
                Vec3::new(x, y, -0.108),
                Vec3::new(x, y, -1.),
                &[(0., 0.029), (0.004, 0.032), (0.008, 0.024), (0.008, 0.)],
                DARK,
                14.,
            );
            wire(
                m,
                &[
                    Vec3::new(x * 1.36, y * 1.36, -0.06),
                    Vec3::new(x * 1.36, y * 1.36, 0.067),
                ],
                0.007,
                EDGE,
                11.,
            );
        }
        m.transform = root;
    }
    // Curved guard and hooked trigger remain attached to the fixed action.
    let guard = (0..=24)
        .map(|i| {
            let a = i as f32 * PI / 24.;
            Vec3::new(0., -0.06 - a.sin() * 0.122, 0.015 + a.cos() * 0.105)
        })
        .collect::<Vec<_>>();
    wire(m, &guard, 0.009, GOLD, 12.);
    wire(
        m,
        &[
            Vec3::new(0., -0.05, 0.055),
            Vec3::new(0., -0.10, 0.022),
            Vec3::new(0., -0.13, 0.038),
        ],
        0.010,
        EDGE,
        11.,
    );
    wire(
        m,
        &[
            Vec3::new(0., 0.07, 0.17),
            Vec3::new(0., 0.16, 0.19),
            Vec3::new(0., 0.18, 0.22),
        ],
        0.019,
        STEEL,
        11.,
    );
    m.transform = root * hinge;
    let twin = shotgun && kind != WeaponKind::SlugGun && kind != WeaponKind::Blunderbuss;
    if model == Model::Gatling {
        m.transform = root
            * Mat4::from_translation(Vec3::new(0., 0.08, 0.))
            * Mat4::from_rotation_z(age * 9.);
        for i in 0..6 {
            let a = i as f32 * TAU / 6.;
            barrel(m, a.cos() * 0.10, a.sin() * 0.10, -0.10, end, 0.040);
        }
        for z in [-0.18, -0.65, -0.91] {
            collar(m, Vec3::new(0., 0., z), 0.158, 0.045, GOLD);
        }
        m.transform = root;
    } else if kind == WeaponKind::Pepperbox {
        for i in 0..4 {
            let a = i as f32 * TAU / 4.;
            barrel(
                m,
                a.cos() * 0.070,
                0.08 + a.sin() * 0.070,
                -0.065,
                end,
                0.051,
            );
        }
    } else if kind == WeaponKind::Blunderbuss {
        lathe(
            m,
            Vec3::new(0., 0.08, 0.04),
            Vec3::new(0., 0.08, -1.),
            &[
                (0., 0.095),
                (0.10, 0.102),
                (0.48, 0.080),
                (0.63, 0.10),
                (0.81, 0.175),
                (0.90, 0.23),
                (0.91, 0.23),
                (0.91, 0.199),
                (0.89, 0.197),
                (0.8, 0.145),
                (0.64, 0.073),
                (0.5, 0.063),
                (0.5, 0.),
            ],
            GOLD,
            12.,
        );
        collar(m, Vec3::new(0., 0.08, -0.82), 0.225, 0.025, EDGE);
    } else {
        let r = if cannon {
            if kind == WeaponKind::Mortar {
                0.16
            } else if kind == WeaponKind::HandCannon {
                0.09
            } else {
                0.12
            }
        } else if shotgun {
            0.077
        } else {
            0.048
        };
        for i in 0..if twin { 2 } else { 1 } {
            let x = if twin { (i as f32 - 0.5) * 0.15 } else { 0. };
            barrel(m, x, 0.08, if revolver { -0.13 } else { 0.04 }, end, r);
        }
    }
    if long && model != Model::Gatling {
        loft(
            m,
            &[
                (Vec3::new(0., -0.045, -0.10), 0.075, 0.045),
                (Vec3::new(0., -0.045, -0.16), 0.110, 0.062),
                (Vec3::new(0., -0.045, -0.39), 0.094, 0.061),
                (Vec3::new(0., -0.037, -0.49), 0.07, 0.039),
                (Vec3::new(0., -0.034, -0.51), 0.002, 0.002),
            ],
            Vec3::Y,
            WOOD,
            13.,
        );
        for i in 0..10 {
            let z = -0.16 - i as f32 * 0.028;
            for side in [-1., 1.] {
                wire(
                    m,
                    &[
                        Vec3::new(side * 0.095, -0.019, z),
                        Vec3::new(side * 0.097, -0.075, z + 0.020),
                    ],
                    0.0025,
                    DARK,
                    14.,
                );
            }
        }
    }
    // A raised ventilated rib makes the double barrel readable from first person.
    if twin {
        bevel(
            m,
            Vec3::new(0., 0.158, (end + 0.04) * 0.5),
            Vec3::new(0.029, 0.011, 0.04 - end),
            0.005,
            EDGE,
            11.,
        );
        for i in 0..5 {
            bevel(
                m,
                Vec3::new(0., 0.145, -0.08 - i as f32 * 0.12),
                Vec3::new(0.022, 0.029, 0.020),
                0.004,
                STEEL,
                11.,
            );
        }
    }
    sights(m, end, if twin { 0.177 } else { 0.14 });
    m.transform = root;
    for side in [-1., 1.] {
        bevel(
            m,
            Vec3::new(side * 0.040, 0.14, 0.12),
            Vec3::new(0.017, 0.035, 0.026),
            0.005,
            STEEL,
            11.,
        );
    }
    if !shotgun && !revolver && !cannon && model != Model::Gatling {
        let slide = pose.rack * 0.13 + crate::motion::window(0., 0.025, 0.05, 0.11, age) * 0.10;
        bevel(
            m,
            Vec3::new(0., 0.115, 0.005 + slide),
            Vec3::new(0.155, 0.090, 0.30),
            0.020,
            STEEL,
            11.,
        );
        // Recessed ejection port, bolt surface and rear serrations.
        bevel(
            m,
            Vec3::new(0.078, 0.118, -0.03 + slide),
            Vec3::new(0.004, 0.050, 0.09),
            0.001,
            DARK,
            14.,
        );
        bevel(
            m,
            Vec3::new(0.079, 0.113, -0.04 + slide),
            Vec3::new(0.003, 0.030, 0.061),
            0.001,
            EDGE,
            11.,
        );
        for i in 0..7 {
            for side in [-1., 1.] {
                wire(
                    m,
                    &[
                        Vec3::new(side * 0.076, 0.09, 0.062 + slide + i as f32 * 0.011),
                        Vec3::new(side * 0.071, 0.144, 0.05 + slide + i as f32 * 0.011),
                    ],
                    0.002,
                    DARK,
                    14.,
                );
            }
        }
        let z = if long { -0.065 } else { 0.195 };
        bevel(
            m,
            Vec3::new(0., -0.205 - pose.magazine * 0.43, z),
            Vec3::new(0.105, if long { 0.29 } else { 0.18 }, 0.12),
            0.015,
            STEEL,
            11.,
        );
        bevel(
            m,
            Vec3::new(0., -0.35 - pose.magazine * 0.43, z),
            Vec3::new(0.121, 0.022, 0.137),
            0.009,
            EDGE,
            11.,
        );
    }
    if kind == WeaponKind::Longrifle {
        for z in [-0.20, 0.10] {
            bevel(
                m,
                Vec3::new(0., 0.19, z),
                Vec3::new(0.050, 0.07, 0.05),
                0.010,
                STEEL,
                11.,
            );
        }
        lathe(
            m,
            Vec3::new(0., 0.25, 0.18),
            Vec3::new(0., 0.25, -1.),
            &[
                (0., 0.074),
                (0.02, 0.076),
                (0.10, 0.055),
                (0.40, 0.045),
                (0.57, 0.083),
                (0.62, 0.083),
                (0.62, 0.064),
                (0.615, 0.),
            ],
            STEEL,
            11.,
        );
        lathe(
            m,
            Vec3::new(0., 0.25, 0.181),
            Vec3::new(0., 0.25, 1.),
            &[(0., 0.061), (0.003, 0.060), (0.007, 0.)],
            [0.003, 0.014, 0.018],
            11.,
        );
        collar(m, Vec3::new(0., 0.25, -0.405), 0.085, 0.021, GOLD);
    }
    if matches!(model, Model::Gatling) || kind == WeaponKind::GrenadeLauncher {
        lathe(
            m,
            Vec3::new(-0.20, -0.16, 0.),
            Vec3::new(1., -0.16, 0.),
            &[
                (0., 0.11),
                (0.018, 0.17),
                (0.04, 0.18),
                (0.35, 0.18),
                (0.38, 0.16),
                (0.4, 0.),
            ],
            STEEL,
            11.,
        );
        for i in 0..12 {
            let a = i as f32 * TAU / 12.;
            wire(
                m,
                &[
                    Vec3::new(-0.17, -0.16 + a.sin() * 0.18, a.cos() * 0.18),
                    Vec3::new(0.15, -0.16 + a.sin() * 0.18, a.cos() * 0.18),
                ],
                0.005,
                GOLD,
                12.,
            );
        }
    }
    if kind == WeaponKind::Harpoon {
        wire(
            m,
            &[Vec3::new(0., 0.10, -0.20), Vec3::new(0., 0.10, end - 0.10)],
            0.012,
            EDGE,
            11.,
        );
        for side in [-1., 1.] {
            wire(
                m,
                &[
                    Vec3::new(0., 0.10, end - 0.10),
                    Vec3::new(side * 0.052, 0.10, end + 0.04),
                ],
                0.010,
                EDGE,
                11.,
            );
        }
    }
    if matches!(
        kind,
        WeaponKind::Dragonbreath
            | WeaponKind::Frostbite
            | WeaponKind::Needler
            | WeaponKind::Ricochet
    ) {
        // Small caged reservoirs, rather than unshaded glowing blobs.
        for side in [-1., 1.] {
            let p = Vec3::new(side * 0.125, 0.075, -0.12);
            lathe(
                m,
                p,
                p - Vec3::Z,
                &[(0., 0.029), (0.018, 0.035), (0.16, 0.035), (0.18, 0.029)],
                kind.tint().map(|x| x * 0.34),
                9.,
            );
            for z in [0., -0.08, -0.16] {
                collar(m, p + Vec3::Z * z, 0.04, 0.019, GOLD);
            }
            for i in 0..4 {
                let a = i as f32 * TAU / 4.;
                let r = Vec3::new(a.cos() * 0.037, a.sin() * 0.037, 0.);
                wire(m, &[p + r, p + r - Vec3::Z * 0.17], 0.0035, STEEL, 11.);
            }
        }
    }
    // Rarity appears as a small set jewel; metal keeps its own lighting response.
    let accent = match rarity {
        0 => GOLD,
        1 => [0.07, 0.55, 0.75],
        2 => [0.9, 0.20, 0.03],
        _ => [0.60, 0.06, 0.9],
    };
    for side in [-1., 1.] {
        bevel(
            m,
            Vec3::new(side * 0.12, 0.014, 0.065),
            Vec3::new(0.016, 0.027, 0.039),
            0.007,
            accent,
            if rarity > 0 { 9. } else { 12. },
        );
    }
    m.transform = root;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn firearm_surfaces_are_finite_and_bounded_in_all_reload_poses() {
        for kind in WeaponKind::ALL.into_iter().filter(|k| k.spec().group < 4) {
            for t in [0., 0.27, 0.48, 0.65, 0.82, 1.] {
                let mut m = Mesh::new();
                firearm(&mut m, kind, 3, Some(t), 0.04);
                assert!(m.vertices.len() < 150_000, "{kind:?}: mesh budget");
                for v in &m.vertices {
                    assert!(
                        v.pos.iter().chain(v.normal.iter()).all(|v| v.is_finite()),
                        "{kind:?}"
                    );
                    assert!(Vec3::from_array(v.pos).length() < 3.);
                    assert!((Vec3::from_array(v.normal).length() - 1.).abs() < 0.001);
                }
            }
        }
    }
}
