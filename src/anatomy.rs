//! Shared animated anatomy: rendering, bullet/projectile hit tests, and ragdoll anchors
//! all use this pose. Missing limbs are absent from both geometry and hit volumes.
use crate::game::Enemy;
use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Part {
    Torso,
    Head,
    LeftArm,
    RightArm,
    LeftLeg,
    RightLeg,
}
impl Part {
    pub const ALL: [Self; 6] = [
        Self::Torso,
        Self::Head,
        Self::LeftArm,
        Self::RightArm,
        Self::LeftLeg,
        Self::RightLeg,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Torso => "",
            Self::Head => "HEADSHOT",
            Self::LeftArm | Self::RightArm => "ARM SEVERED",
            Self::LeftLeg | Self::RightLeg => "LEG SEVERED",
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Anatomy {
    pub damage: [f32; 6],
    pub severed: u8,
    #[serde(default)]
    pub collapse: f32,
    #[serde(skip)]
    pub flinch: [f32; 6],
    #[serde(skip)]
    pub stagger: f32,
    #[serde(skip)]
    pub impulse: Vec3,
    #[serde(skip)]
    pub impact_point: Vec3,
    #[serde(skip)]
    pub impact_part: Option<Part>,
    #[serde(skip)]
    pub impact_energy: f32,
    #[serde(skip)]
    pub impact_time: f32,
    #[serde(skip)]
    pub impact_pose: Option<Box<Pose>>,
    #[serde(skip)]
    pub fracture: bool,
    /// Capture the animated pose before a fatal impact changes flinch/position.
    /// Missing-part flags still come from the final anatomy, avoiding duplicates.
    #[serde(skip)]
    pub death_pose: Option<Box<Pose>>,
}
impl Anatomy {
    pub fn missing(&self, part: Part) -> bool {
        self.severed & (1 << part as usize) != 0
    }
    pub fn legs(&self) -> u32 {
        [Part::LeftLeg, Part::RightLeg]
            .into_iter()
            .filter(|p| self.missing(*p))
            .count() as u32
    }
    pub fn arms(&self) -> u32 {
        [Part::LeftArm, Part::RightArm]
            .into_iter()
            .filter(|p| self.missing(*p))
            .count() as u32
    }
    pub fn speed(&self) -> f32 {
        match self.legs() {
            0 => 1.,
            1 => 0.48,
            _ => 0.2,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Limb {
    pub root: Vec3,
    pub joint: Vec3,
    pub tip: Vec3,
}
#[derive(Clone, Debug)]
pub struct Pose {
    pub root: Mat4,
    pub head_root: Mat4,
    pub arms: [Limb; 2],
    pub legs: [Limb; 2],
    pub head_y: f32,
    pub scale: f32,
    pub body_radius: f32,
}
impl Pose {
    pub fn for_enemy(e: &Enemy, target: Vec3) -> Self {
        let delta = target - e.pos;
        let walk = if Vec3::new(delta.x, 0., delta.z).length() > if e.kind == 2 { 7. } else { 1.25 }
        {
            1.
        } else {
            0.
        };
        Self::new(
            e.pos,
            delta.x.atan2(delta.z),
            e.phase,
            e.kind,
            e.hit,
            walk,
            if e.attack > 0.45 && e.kind != 2 {
                1.1 - e.attack
            } else {
                0.
            },
            &e.anatomy,
        )
    }
    pub fn new(
        pos: Vec3,
        yaw: f32,
        phase: f32,
        kind: usize,
        hit: f32,
        walk: f32,
        attack: f32,
        anatomy: &Anatomy,
    ) -> Self {
        let scale = crate::encounters::species(kind).scale;
        let lost_legs = anatomy.legs();
        let collapse = anatomy.collapse.clamp(0., 2.);
        let crawl = (collapse - 1.).clamp(0., 1.);
        let crouch = -0.23 * collapse.min(1.) - 0.19 * crawl;
        let lean = 0.95 * crawl;
        let roll = if anatomy.missing(Part::LeftLeg) {
            -0.14
        } else {
            0.14
        } * collapse.min(1.)
            * (1. - crawl);
        let root = Mat4::from_translation(pos)
            * Mat4::from_rotation_y(yaw)
            * Mat4::from_scale(Vec3::splat(scale))
            * Mat4::from_translation(Vec3::new(
                0.,
                crouch + (phase * 6.).sin().abs() * 0.018 * walk,
                0.,
            ))
            * Mat4::from_rotation_z(roll)
            * Mat4::from_rotation_x(lean - hit * 0.55);
        let empty = Limb {
            root: Vec3::ZERO,
            joint: Vec3::ZERO,
            tip: Vec3::ZERO,
        };
        let mut arms = [empty; 2];
        let mut legs = [empty; 2];
        for (i, side) in [-1., 1.].into_iter().enumerate() {
            let cycle = (phase * 0.95 + if side > 0. { 0.5 } else { 0. }).fract();
            let swing = cycle < 0.4;
            let stride = if swing {
                crate::motion::smooth(0., 0.4, cycle) * 0.72 - 0.36
            } else {
                0.36 - (cycle - 0.4) / 0.6 * 0.72
            };
            let lift = if swing {
                (cycle / 0.4 * std::f32::consts::PI).sin() * 0.15
            } else {
                0.
            };
            let s = stride * walk * anatomy.speed();
            let hip = Vec3::new(side * 0.15, 0.85, 0.);
            let foot_x = side * 0.18;
            let foot_y = (0.085 + lift * walk - crouch - roll.sin() * foot_x) / roll.cos();
            let foot = Vec3::new(foot_x, foot_y, s);
            let axis = (foot - hip).normalize();
            let length = (foot - hip).length().min(0.835);
            let bend = (0.42f32.powi(2) - (length * 0.5).powi(2)).max(0.).sqrt();
            let knee =
                (hip + foot) * 0.5 + Vec3::new(0., axis.z, -axis.y).normalize_or_zero() * bend;
            let kick = anatomy.flinch[4 + i] * 0.7;
            legs[i] = Limb {
                root: hip,
                joint: knee + Vec3::Z * kick,
                tip: foot + Vec3::Y * kick,
            };
            let shoulder = Vec3::new(side * 0.34, 1.4, 0.);
            let mut elbow = Vec3::new(side * 0.43, 1.03, -s * 0.6);
            let mut hand = Vec3::new(side * 0.42, 0.79, 0.12 - s * 0.9);
            // Attack with the surviving arm after the weapon arm has been shot off.
            let attacking = if anatomy.missing(Part::RightArm) {
                side < 0.
            } else {
                side > 0.
            };
            if attacking && attack > 0. {
                let swing = crate::motion::smooth(0., 0.18, attack);
                let recover = 1. - crate::motion::smooth(0.25, 0.65, attack);
                elbow = elbow.lerp(Vec3::new(side * 0.43, 1.36, 0.22), recover);
                hand = hand.lerp(
                    Vec3::new(
                        side * (0.36 - swing * 0.30),
                        1.72 - swing * 0.54,
                        0.05 + swing * 0.65,
                    ),
                    recover,
                );
            }
            if lost_legs == 2 {
                // Plant the hands on the floor while the pelvis drags behind.
                let reach = 0.28 + (phase * 4. + side * 1.6).sin() * 0.10;
                let planted_y = (0.48 + 0.95f32.sin() * reach) / 0.95f32.cos();
                elbow = elbow.lerp(Vec3::new(side * 0.43, 1.36, 0.12), crawl);
                hand = hand.lerp(Vec3::new(side * 0.45, planted_y, reach), crawl);
            }
            let recoil = anatomy.flinch[2 + i];
            elbow += Vec3::new(side * recoil * 0.35, recoil * 0.25, -recoil * 0.5);
            hand += Vec3::new(side * recoil * 0.5, recoil * 0.4, -recoil * 0.8);
            arms[i] = Limb {
                root: shoulder,
                joint: elbow,
                tip: hand,
            };
        }
        let head_y = if matches!(kind, 2 | 4) {
            2. + phase.sin() * 0.18
        } else if kind == 5 {
            0.85
        } else {
            1.72
        };
        let neck = Vec3::new(0., head_y - 0.18, 0.);
        let head_root = root
            * Mat4::from_translation(neck)
            * Mat4::from_rotation_x(-lean * 0.9)
            * Mat4::from_translation(-neck);
        Self {
            root,
            head_root,
            arms,
            legs,
            head_y,
            scale,
            body_radius: if matches!(kind, 6 | 7) { 0.32 } else { 0.235 },
        }
    }

    pub fn segments(&self, part: Part) -> Vec<(Vec3, Vec3, f32)> {
        let local = match part {
            Part::Head if self.head_y < 1. => vec![
                (
                    Vec3::new(0., self.head_y - 0.04, 0.),
                    Vec3::new(0., self.head_y + 0.07, 0.),
                    0.20,
                ),
                (Vec3::new(0., 0.76, -0.35), Vec3::new(0., 0.76, -0.08), 0.28),
            ],
            Part::Head => vec![(
                Vec3::new(0., self.head_y - 0.04, 0.),
                Vec3::new(0., self.head_y + 0.07, 0.),
                0.20,
            )],
            Part::Torso => vec![(
                Vec3::new(0., 0.89, 0.),
                Vec3::new(0., 1.37, 0.),
                self.body_radius,
            )],
            _ => {
                let l = match part {
                    Part::LeftArm => self.arms[0],
                    Part::RightArm => self.arms[1],
                    Part::LeftLeg => self.legs[0],
                    _ => self.legs[1],
                };
                vec![(l.root, l.joint, 0.095), (l.joint, l.tip, 0.08)]
            }
        };
        let transform = if part == Part::Head {
            self.head_root
        } else {
            self.root
        };
        local
            .into_iter()
            .map(|(a, b, r)| {
                (
                    transform.transform_point3(a),
                    transform.transform_point3(b),
                    r * self.scale,
                )
            })
            .collect()
    }
    pub fn anchor(&self, part: Part) -> Vec3 {
        self.root.transform_point3(match part {
            Part::Head => Vec3::new(0., self.head_y - 0.18, 0.),
            Part::Torso => Vec3::new(0., 1.2, 0.),
            Part::LeftArm => self.arms[0].root,
            Part::RightArm => self.arms[1].root,
            Part::LeftLeg => self.legs[0].root,
            Part::RightLeg => self.legs[1].root,
        })
    }
}
pub fn present(e: &Enemy, p: Part) -> bool {
    !e.anatomy.missing(p) && (!crate::encounters::head_only(e.kind) || p == Part::Head)
}
pub fn trace(
    e: &Enemy,
    target: Vec3,
    origin: Vec3,
    dir: Vec3,
    range: f32,
    frame_time: f32,
) -> Option<(Part, f32)> {
    // A volley sees the pose actually rendered, not intermediate per-pellet
    // knockback and flinch. The current sever mask still removes missing parts.
    let cached = (e.anatomy.impact_time == frame_time)
        .then_some(e.anatomy.impact_pose.as_deref())
        .flatten();
    let scale = crate::encounters::species(e.kind).scale;
    let base = cached.map_or(e.pos, |pose| pose.root.transform_point3(Vec3::ZERO));
    let center = base + Vec3::Y * (1.5 * scale);
    // Reject distant/off-axis enemies before constructing their animated hit capsules.
    let broad = ray_sphere(origin, dir, center, 3. * scale)?;
    if broad > range {
        return None;
    }
    let fresh;
    let pose = if let Some(pose) = cached {
        pose
    } else {
        fresh = Pose::for_enemy(e, target);
        &fresh
    };
    Part::ALL
        .into_iter()
        .filter(|p| present(e, *p))
        .flat_map(|part| {
            pose.segments(part)
                .into_iter()
                .filter_map(move |(a, b, r)| ray_capsule(origin, dir, a, b, r).map(|t| (part, t)))
        })
        .filter(|(_, t)| *t <= range)
        .min_by(|a, b| a.1.total_cmp(&b.1))
}
fn ray_sphere(o: Vec3, d: Vec3, c: Vec3, r: f32) -> Option<f32> {
    let oc = o - c;
    let b = oc.dot(d);
    let c = oc.length_squared() - r * r;
    if c <= 0. {
        return Some(0.);
    }
    let h = b * b - c;
    if h < 0. {
        return None;
    }
    let t = -b - h.sqrt();
    (t >= 0.).then_some(t)
}
fn ray_capsule(o: Vec3, d: Vec3, a: Vec3, b: Vec3, r: f32) -> Option<f32> {
    let ba = b - a;
    let oa = o - a;
    let baba = ba.length_squared();
    if baba < 1e-8 {
        return ray_sphere(o, d, a, r);
    }
    let near = a + ba * (oa.dot(ba) / baba).clamp(0., 1.);
    if (near - o).length_squared() <= r * r {
        return Some(0.);
    }
    let bard = ba.dot(d);
    let baoa = ba.dot(oa);
    let rdoa = d.dot(oa);
    let aa = baba - bard * bard;
    let bb = baba * rdoa - baoa * bard;
    let cc = baba * oa.length_squared() - baoa * baoa - r * r * baba;
    let mut result = [ray_sphere(o, d, a, r), ray_sphere(o, d, b, r)]
        .into_iter()
        .flatten()
        .min_by(f32::total_cmp);
    let h = bb * bb - aa * cc;
    if aa.abs() > 1e-7 && h >= 0. {
        let t = (-bb - h.sqrt()) / aa;
        let y = baoa + t * bard;
        if t >= 0. && y >= 0. && y <= baba && result.is_none_or(|old| t < old) {
            result = Some(t);
        }
    }
    result
}
#[derive(Clone)]
pub struct BodyEvent {
    pub enemy: Enemy,
    pub target: Vec3,
    pub part: Option<Part>,
    pub impulse: Vec3,
    pub impact: Vec3,
    pub energy: f32,
    pub fracture: bool,
}

/// Cosmetic physics impulses never award damage, kills or additional soul drops.
pub enum PhysicsImpact {
    Ray {
        origin: Vec3,
        direction: Vec3,
        range: f32,
        energy: f32,
    },
    Blast {
        center: Vec3,
        radius: f32,
        energy: f32,
    },
}
