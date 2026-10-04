//! Disposable native review: all wounds are produced by Game::fire and real projectiles.
use crate::{
    anatomy::{BodyEvent, Part, PhysicsImpact, Pose},
    dismemberment::Section,
    game::{Enemy, Game},
    scene::{Bones, MAX_BODY_PIECES},
    weapons::WeaponKind,
};
use glam::Vec3;
use serde::Serialize;

pub const FRAMES: u32 = 1440;
const SCENE_FRAMES: u32 = 240;
const LABELS: [&str; 6] = [
    "HEADSHOT / THE SKULL BREAKS FREE",
    "ARM SHOT / SEVERED ARM KEEPS ITS ELBOW",
    "LEG SHOTS / LIMP, THEN CRAWL",
    "TORSO SHOT / KNEES AND ELBOWS COLLAPSE",
    "HEAVY SHOT / THE ACTUAL MESH FRACTURES",
    "EXPLOSIVE / DEBRIS REACTS TO THE NEXT BLAST",
];
#[derive(Default, Serialize)]
struct Evidence {
    scene: usize,
    label: &'static str,
    fired: u32,
    corpse_events: u32,
    detached_events: u32,
    max_bodies: usize,
    max_joints: usize,
    fractured_bodies: usize,
    max_handoff_error_m: f32,
    max_joint_anchor_error_m: f32,
    max_elbow_flex_radians: f32,
    max_knee_flex_radians: f32,
    peak_mean_speed: f32,
    final_mean_speed: f32,
    blasts_on_existing_debris: u32,
    max_blast_velocity_change: f32,
    passed: bool,
}
#[derive(Default)]
pub struct Review {
    evidence: Vec<Evidence>,
    frame: u32,
    scene: usize,
    before_blast: Option<Vec<Vec3>>,
}
impl Review {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn step(&mut self, game: &mut Game, bones: &mut Bones, frame: u32) {
        if frame > FRAMES {
            return;
        }
        self.scene = ((frame - 1) / SCENE_FRAMES) as usize;
        self.frame = (frame - 1) % SCENE_FRAMES;
        if self.frame == 0 {
            game.new_run();
            *bones = Bones::new();
            game.run.pos = Vec3::new(0., 1.65, 3.);
            game.run.time = 12.;
            let mut target = Enemy::spawn(0, Vec3::new(0., 0., -2.), 1, 0.);
            target.hp = match self.scene {
                0 => 65.,
                3 => 25.,
                4 => 90.,
                5 => 100.,
                _ => 100.,
            };
            target.max_hp = target.hp;
            target.attack = 10.;
            game.run.enemies = vec![target, Enemy::spawn(0, Vec3::new(24., 0., 24.), 1, 1.)];
            game.run.weapon.kind = match self.scene {
                4 => WeaponKind::Double,
                5 => WeaponKind::GrenadeLauncher,
                _ => WeaponKind::Pistol,
            };
            game.run.weapon.rarity = if self.scene >= 4 { 2 } else { 1 };
            game.run.ammo = game.run.weapon.capacity();
            game.run.survival.remaining = 0;
            self.evidence.push(Evidence {
                scene: self.scene,
                label: LABELS[self.scene],
                ..Default::default()
            });
        }
        game.run.hp = 100.;
        game.notify(LABELS[self.scene]);
        game.notice_time = 4.;
        let part = match self.scene {
            0 => Part::Head,
            1 if self.frame < 100 => Part::RightArm,
            2 if self.frame < 95 => Part::LeftLeg,
            2 if self.frame < 155 => Part::RightLeg,
            _ => Part::Torso,
        };
        let fire = self.frame == 45
            || (self.scene == 1 && [110, 136, 162].contains(&self.frame))
            || (self.scene == 2 && [105, 165, 192].contains(&self.frame))
            || (self.scene == 5 && self.frame == 145);
        let live_target = game
            .run
            .enemies
            .iter()
            .find(|e| e.pos.x.abs() < 10. && e.pos.z < 10.);
        let aim = if let Some(enemy) = live_target {
            let pose = Pose::for_enemy(enemy, game.run.pos);
            let (a, b, _) = pose.segments(part)[0];
            let mut target = (a + b) * 0.5;
            if self.scene == 5 {
                // Compensate the launcher's real upward lob at this short distance.
                let flight = target.distance(game.run.pos) / game.run.weapon.kind.spec().projectile;
                target.y -= 4. * flight - 4.9 * flight * flight;
            }
            target
        } else if !bones.pieces.is_empty() {
            let center = bones
                .pieces
                .iter()
                .map(|p| {
                    let t = bones.bodies[p.handle].translation();
                    Vec3::new(t.x, t.y, t.z)
                })
                .sum::<Vec3>()
                / bones.pieces.len() as f32;
            if self.scene == 5 && fire {
                let flight = center.distance(game.run.pos) / game.run.weapon.kind.spec().projectile;
                center - Vec3::Y * (4. * flight - 4.9 * flight * flight)
            } else {
                center + Vec3::Y * 0.18
            }
        } else {
            Vec3::new(0., 1., -2.)
        };
        let direction = (aim - game.run.pos).normalize();
        game.run.yaw = direction.x.atan2(-direction.z);
        game.run.pitch = direction.y.asin();
        if fire {
            let before = game.attack_serial;
            game.fire(false);
            assert!(
                game.attack_serial > before,
                "review shot must pass real cooldown/ammo checks"
            );
            self.evidence[self.scene].fired += 1;
            if self.scene == 0 && self.frame == 45 {
                assert!(
                    game.spawned_bodies
                        .iter()
                        .any(|e| e.part == Some(Part::Head)),
                    "aimed headshot detaches skull"
                );
            }
            if self.scene == 1 && self.frame == 45 {
                assert!(
                    game.run.enemies[0].anatomy.missing(Part::RightArm),
                    "aimed arm shot must sever arm"
                );
            }
            if self.scene == 2 && [45, 105].contains(&self.frame) {
                assert_eq!(
                    game.run.enemies[0].anatomy.legs(),
                    if self.frame == 45 { 1 } else { 2 },
                    "actual shots cause limp, then crawl"
                );
            }
        }
    }
    pub fn impact(&mut self, impact: &PhysicsImpact, bones: &Bones) {
        if matches!(impact, PhysicsImpact::Blast { .. }) && !bones.pieces.is_empty() {
            self.evidence[self.scene].blasts_on_existing_debris += 1;
            self.before_blast = Some(
                bones
                    .pieces
                    .iter()
                    .map(|p| {
                        let v = bones.bodies[p.handle].linvel();
                        Vec3::new(v.x, v.y, v.z)
                    })
                    .collect(),
            );
        }
    }
    pub fn impacted(&mut self, bones: &Bones) {
        if let Some(before) = self.before_blast.take() {
            let change = bones
                .pieces
                .iter()
                .zip(before)
                .map(|(p, before)| {
                    let v = bones.bodies[p.handle].linvel();
                    Vec3::new(v.x, v.y, v.z).distance(before)
                })
                .fold(0., f32::max);
            self.evidence[self.scene].max_blast_velocity_change = self.evidence[self.scene]
                .max_blast_velocity_change
                .max(change);
        }
    }
    /// Called immediately after spawning, before the first physics tick.
    pub fn spawned(&mut self, event: &BodyEvent, bones: &Bones, old_count: usize) {
        let evidence = &mut self.evidence[self.scene];
        if event.part.is_none() {
            evidence.corpse_events += 1;
        } else {
            evidence.detached_events += 1;
        }
        let pose = event
            .enemy
            .anatomy
            .death_pose
            .as_deref()
            .cloned()
            .unwrap_or_else(|| Pose::for_enemy(&event.enemy, event.target));
        let sections = crate::enemy_assets::physical_sections(
            event.enemy.kind,
            event.enemy.phase,
            &pose,
            &event.enemy.anatomy,
        );
        for piece in &bones.pieces[old_count..] {
            if event.part.is_none() {
                assert!(
                    !event.enemy.anatomy.missing(piece.part),
                    "corpse regenerated a severed part"
                );
            }
            if piece.fractured {
                continue;
            }
            let center = sections
                .iter()
                .find(|s| s.section == piece.section)
                .expect("spawned section belongs to posed source mesh")
                .center;
            let actual = bones.bodies[piece.handle].translation();
            let error = center.distance(Vec3::new(actual.x, actual.y, actual.z));
            evidence.max_handoff_error_m = evidence.max_handoff_error_m.max(error);
            assert!(
                error < 0.0001,
                "animated-to-physics handoff must preserve pose"
            );
        }
    }
    pub fn observe(&mut self, bones: &Bones) {
        let evidence = &mut self.evidence[self.scene];
        assert!(
            bones.pieces.len() <= MAX_BODY_PIECES,
            "bounded physics body count"
        );
        let mut total_speed = 0.;
        for piece in &bones.pieces {
            let body = &bones.bodies[piece.handle];
            assert!(
                body.translation().iter().all(|v| v.is_finite())
                    && body.linvel().iter().all(|v| v.is_finite()),
                "finite physical motion"
            );
            assert!(body.translation().y > -2., "no falling through ground");
            total_speed += body.linvel().norm();
        }
        let mean_speed = total_speed / bones.pieces.len().max(1) as f32;
        evidence.peak_mean_speed = evidence.peak_mean_speed.max(mean_speed);
        evidence.final_mean_speed = mean_speed;
        evidence.max_bodies = evidence.max_bodies.max(bones.pieces.len());
        evidence.max_joints = evidence.max_joints.max(bones.joint_count());
        evidence.fractured_bodies = evidence
            .fractured_bodies
            .max(bones.pieces.iter().filter(|p| p.fractured).count());
        evidence.max_joint_anchor_error_m = evidence
            .max_joint_anchor_error_m
            .max(bones.max_joint_anchor_error());
        for (upper, lower, knee) in [
            (Section::LeftUpperArm, Section::LeftLowerArm, false),
            (Section::RightUpperArm, Section::RightLowerArm, false),
            (Section::LeftUpperLeg, Section::LeftLowerLeg, true),
            (Section::RightUpperLeg, Section::RightLowerLeg, true),
        ] {
            if let (Some(a), Some(b)) = (
                bones.pieces.iter().find(|p| p.section == upper),
                bones.pieces.iter().find(|p| p.section == lower),
            ) {
                let angle = (bones.bodies[a.handle].rotation().inverse()
                    * bones.bodies[b.handle].rotation())
                .angle();
                let max = if knee {
                    &mut evidence.max_knee_flex_radians
                } else {
                    &mut evidence.max_elbow_flex_radians
                };
                *max = max.max(angle);
            }
        }
        if self.frame != SCENE_FRAMES - 1 {
            return;
        }
        assert!(
            evidence.corpse_events > 0,
            "each scenario must show an actual fatal shot"
        );
        assert!(
            evidence.max_joint_anchor_error_m < 0.2,
            "physical joints must remain connected"
        );
        match self.scene {
            0 => assert_eq!(
                bones.pieces.iter().filter(|p| p.part == Part::Head).count(),
                1,
                "skull must not regenerate"
            ),
            1 => assert_eq!(
                bones
                    .pieces
                    .iter()
                    .filter(|p| p.part == Part::RightArm)
                    .count(),
                2,
                "severed arm stays detached after death"
            ),
            2 => {
                for part in [Part::LeftLeg, Part::RightLeg] {
                    assert_eq!(
                        bones.pieces.iter().filter(|p| p.part == part).count(),
                        2,
                        "legs stay detached after death"
                    );
                }
            }
            3 => {
                assert_eq!(evidence.max_bodies, 10);
                assert_eq!(evidence.max_joints, 9);
                assert_eq!(evidence.fractured_bodies, 0);
                assert!(
                    evidence.max_elbow_flex_radians > 0.04 && evidence.max_knee_flex_radians > 0.04,
                    "independent knees and elbows flex"
                );
                assert!(
                    evidence.final_mean_speed < evidence.peak_mean_speed * 0.7,
                    "corpse settles after impact"
                );
            }
            4 => assert!(
                evidence.fractured_bodies >= 2,
                "shotgun creates genuine clipped chunks"
            ),
            5 => {
                assert!(
                    evidence.fractured_bodies >= 2,
                    "explosion fractures geometry"
                );
                assert!(
                    evidence.blasts_on_existing_debris > 0,
                    "next real explosion affects existing debris"
                );
                assert!(
                    evidence.max_blast_velocity_change > 0.1,
                    "follow-up blast must physically move old debris"
                );
            }
            _ => unreachable!(),
        }
        evidence.passed = true;
        println!(
            "ANATOMY scene {} PASS: {} bodies, {} joints, {} fractured, joint gap {:.4}m, handoff {:.6}m, final speed {:.3}m/s",
            self.scene,
            evidence.max_bodies,
            evidence.max_joints,
            evidence.fractured_bodies,
            evidence.max_joint_anchor_error_m,
            evidence.max_handoff_error_m,
            evidence.final_mean_speed
        );
    }
    pub fn finish(&self) {
        assert_eq!(self.evidence.len(), 6);
        assert!(self.evidence.iter().all(|e| e.passed));
        std::fs::write("captures/anatomy/manifest.json",serde_json::to_string_pretty(&serde_json::json!({
            "simulation_hz":60,"physics_hz":120,"capture_fps":30,"duration_seconds":24,"saves":"disabled",
            "damage_path":"Game::fire / actual ray traces and projectiles","body_budget":MAX_BODY_PIECES,"scenes":self.evidence,
        })).unwrap()).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn review_scenarios_use_actual_combat_and_articulated_physics() {
        let mut game = Game::new(false);
        let mut bones = Bones::new();
        let mut review = Review::new();
        for frame in 1..=FRAMES {
            review.step(&mut game, &mut bones, frame);
            game.update(1. / 60.);
            for impact in game.physics_impacts.drain(..) {
                review.impact(&impact, &bones);
                match impact {
                    PhysicsImpact::Ray {
                        origin,
                        direction,
                        range,
                        energy,
                    } => bones.impact_ray(origin, direction, range, energy),
                    PhysicsImpact::Blast {
                        center,
                        radius,
                        energy,
                    } => bones.impact_blast(center, radius, energy),
                }
                review.impacted(&bones);
            }
            for event in game.spawned_bodies.drain(..) {
                let count = bones.pieces.len();
                bones.spawn_body(event.clone());
                review.spawned(&event, &bones, count);
            }
            bones.update(1. / 60.);
            review.observe(&bones);
        }
    }
}
