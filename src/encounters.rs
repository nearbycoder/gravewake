//! Arena roster and enemy decision making. Attack warnings precede real hazards.
use crate::{
    anatomy::Anatomy,
    game::{Attack, Cause, Enemy, Game},
    world_layout::{self, ENEMY_RADIUS},
};
use glam::Vec3;
use serde::{Deserialize, Serialize};

pub struct Species {
    pub name: &'static str,
    pub role: &'static str,
    pub lore: [&'static str; 3],
    pub hp: f32,
    pub speed: f32,
    pub xp: u32,
    pub scale: f32,
}
pub const ROSTER: [Species; 12] = [
    Species {
        name: "OSSUARY DRUDGE",
        role: "GROUND / HORDE",
        lore: [
            "A debt the grave refused.",
            "Slow, relentless, never alone.",
            "Thin the crowd before it closes.",
        ],
        hp: 52.,
        speed: 1.3,
        xp: 5,
        scale: 1.,
    },
    Species {
        name: "RIBBLADE SKIRMISHER",
        role: "GROUND / FLANKER",
        lore: [
            "Bone, blade, and restless hunger.",
            "Circles before closing to strike.",
            "Shoot the sword arm to disarm.",
        ],
        hp: 75.,
        speed: 2.25,
        xp: 7,
        scale: 1.,
    },
    Species {
        name: "CINDER SKULL",
        role: "FLYING / EMBER SPITTER",
        lore: [
            "A soul with nowhere to burn.",
            "Fires slow, dodgeable embers.",
            "Watch the glow before it spits.",
        ],
        hp: 45.,
        speed: 1.5,
        xp: 7,
        scale: 1.,
    },
    Species {
        name: "THE TITHEKEEPER",
        role: "BOSS / SHOCKWAVE",
        lore: [
            "The forest remembers its king.",
            "Returns every fourth descent.",
            "Leave the ring before it erupts.",
        ],
        hp: 600.,
        speed: 0.9,
        xp: 60,
        scale: 1.7,
    },
    Species {
        name: "GLOAMWING",
        role: "FLYING / DIVE HUNTER",
        lore: [
            "Leather wings around a stolen skull.",
            "Dives toward your last position.",
            "Sidestep when its wings flare.",
        ],
        hp: 32.,
        speed: 2.6,
        xp: 5,
        scale: 1.15,
    },
    Species {
        name: "GRAVE CRAWLER",
        role: "GROUND / LOW SWARM",
        lore: [
            "Six legs, one terrible appetite.",
            "Scuttles below your sights.",
            "Look down; do not let them gather.",
        ],
        hp: 28.,
        speed: 2.9,
        xp: 4,
        scale: 0.62,
    },
    Species {
        name: "IRON PENITENT",
        role: "GROUND / ARMORED",
        lore: [
            "Rusted plates hide a broken oath.",
            "Torso armor blunts bullet impacts.",
            "Expose its skull or shoot its limbs.",
        ],
        hp: 150.,
        speed: 0.95,
        xp: 12,
        scale: 1.22,
    },
    Species {
        name: "PLAGUE VESSEL",
        role: "GROUND / VOLATILE",
        lore: [
            "Something is growing beneath it.",
            "Swells before a close-range blast.",
            "Escape the green warning circle.",
        ],
        hp: 105.,
        speed: 1.5,
        xp: 10,
        scale: 1.18,
    },
    Species {
        name: "ASH CANTOR",
        role: "LEVITATING / VOLLEY",
        lore: [
            "A hymn sung through burned teeth.",
            "Keeps distance; casts three bolts.",
            "Thread the gaps in its fan of fire.",
        ],
        hp: 85.,
        speed: 1.6,
        xp: 12,
        scale: 1.05,
    },
    Species {
        name: "BELL GARGOYLE",
        role: "FLYING / HEAVY DIVER",
        lore: [
            "Stone wings crack the silence.",
            "Marks a dive, then commits to it.",
            "Its great wings betray the attack.",
        ],
        hp: 180.,
        speed: 1.65,
        xp: 16,
        scale: 1.25,
    },
    Species {
        name: "BONE SHEPHERD",
        role: "GROUND / SUMMONER",
        lore: [
            "Carries a lantern full of names.",
            "Raises drudges while left alive.",
            "Break the shepherd before the flock.",
        ],
        hp: 110.,
        speed: 1.05,
        xp: 15,
        scale: 1.1,
    },
    Species {
        name: "TITHE REAPER",
        role: "GROUND / BLINK ASSASSIN",
        lore: [
            "A silhouette between heartbeats.",
            "Marks a landing before it blinks.",
            "Keep clear of its violet sigil.",
        ],
        hp: 120.,
        speed: 2.2,
        xp: 16,
        scale: 1.12,
    },
];
pub fn species(k: usize) -> &'static Species {
    &ROSTER[k.min(ROSTER.len() - 1)]
}
/// The wind-up sound that accompanies each special attack's visible warning.
pub fn warning_cue(kind: usize) -> &'static str {
    match kind {
        3 | 7 => "warn_slam",
        4 | 9 => "warn_dive",
        10 => "warn_summon",
        11 => "warn_blink",
        _ => "warn_cast",
    }
}
/// How long each special attack's warning lasts before it lands.
pub fn warning_time(kind: usize) -> f32 {
    if kind == 7 { 1.1 } else { 0.85 }
}
/// Whether a special attack is aimed at the player. Summons aren't; slams and
/// bursts only threaten a player near the marked circle.
pub fn aimed_at(kind: usize, target: Vec3, player: Vec3) -> bool {
    match kind {
        2 | 4 | 8 | 9 | 11 => true,
        3 | 7 => {
            let radius = if kind == 3 { 4.2 } else { 3.2 };
            ((player - target) * Vec3::new(1., 0., 1.)).length() < radius + 1.5
        }
        _ => false,
    }
}
pub fn flying(k: usize) -> bool {
    matches!(k, 2 | 4 | 8 | 9)
}
pub fn head_only(k: usize) -> bool {
    matches!(k, 2 | 4 | 5)
}
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
pub struct Brain {
    pub warning: f32,
    pub target: Vec3,
    pub charging: f32,
    pub cooldown: f32,
    pub summons: u8,
}
pub struct Hazard {
    pub pos: Vec3,
    pub vel: Vec3,
    pub life: f32,
    pub damage: f32,
    pub color: [f32; 3],
    /// The species that cast it, for the death recap.
    pub from: usize,
}
impl Enemy {
    pub fn spawn(kind: usize, pos: Vec3, wave: u32, phase: f32) -> Self {
        let hp = species(kind).hp * (1. + wave.saturating_sub(1) as f32 * 0.085);
        Self {
            kind,
            pos,
            hp,
            max_hp: hp,
            attack: 1.8,
            phase,
            hit: 0.,
            burn: 0.,
            poison: 0.,
            slow: 0.,
            anatomy: Anatomy::default(),
            ai: Brain {
                cooldown: 2. + phase.rem_euclid(3.),
                ..Brain::default()
            },
        }
    }
}
impl Game {
    pub fn tick_enemies(&mut self, dt: f32) {
        let player = self.run.pos;
        self.navigation.update(player);
        let cleanup = self.run.survival.remaining == 0 && self.run.enemies.len() <= 3;
        let positions: Vec<_> = self.run.enemies.iter().map(|e| e.pos).collect();
        // Each hit this update: where it came from (for the HUD's damage
        // arcs), what dealt it (for the death recap) and how hard.
        let mut hits: Vec<(Vec3, Cause, f32)> = vec![];
        let mut sounds = vec![];
        let mut summons = vec![];
        for (index, e) in self.run.enemies.iter_mut().enumerate() {
            if e.hp <= 0. {
                continue;
            }
            e.phase += dt * e.anatomy.speed();
            e.anatomy.collapse +=
                (e.anatomy.legs() as f32 - e.anatomy.collapse) * (1. - (-12. * dt).exp());
            e.anatomy.stagger = (e.anatomy.stagger - dt).max(0.);
            for f in &mut e.anatomy.flinch {
                *f = (*f - dt).max(0.);
            }
            if e.burn > 0. {
                e.hp -= 18. * dt;
                e.burn = (e.burn - dt).max(0.);
            }
            if e.poison > 0. {
                e.hp -= 12. * dt;
                e.poison = (e.poison - dt).max(0.);
            }
            e.slow = (e.slow - dt).max(0.);
            e.hit = (e.hit - dt).max(0.);
            e.attack -= dt;
            e.ai.cooldown -= dt;
            if e.hp <= 0. {
                continue;
            }
            let flat = (player - e.pos) * Vec3::new(1., 0., 1.);
            let distance = flat.length();
            let dir = flat.normalize_or_zero();
            let ranged = matches!(e.kind, 2 | 8 | 10);
            let clear_sight = world_layout::obstruction(
                e.pos + Vec3::Y * if flying(e.kind) { 1.8 } else { 1.5 },
                player,
            )
            .is_none();
            let stop = if ranged && clear_sight { 7. } else { 1.25 };
            let pursuit = self.navigation.direction(e.pos, player, ENEMY_RADIUS);
            let previous = e.pos;
            // All specials have a visible warning and commit to a recorded position.
            if e.ai.warning > 0. {
                e.ai.warning -= dt;
                if e.ai.warning <= 0. && e.anatomy.stagger <= 0. {
                    match e.kind {
                        2 | 8 => {
                            let origin = e.pos + Vec3::Y * if e.kind == 2 { 2. } else { 1.65 };
                            let aim = (e.ai.target - origin).normalize_or_zero();
                            for j in 0..if e.kind == 8 { 3 } else { 1 } {
                                let angle = if e.kind == 8 {
                                    (j as f32 - 1.) * 0.19
                                } else {
                                    0.
                                };
                                self.hazards.push(Hazard {
                                    pos: origin,
                                    vel: glam::Quat::from_rotation_y(angle) * aim * 7.,
                                    life: 5.,
                                    damage: 8.,
                                    color: if e.kind == 8 {
                                        [1.8, 0.3, 1.1]
                                    } else {
                                        [3., 0.6, 0.08]
                                    },
                                    from: e.kind,
                                });
                            }
                        }
                        3 | 7 => {
                            sounds.push(("impact", e.ai.target));
                            let radius = if e.kind == 3 { 4.2 } else { 3.2 };
                            let blast_origin =
                                e.ai.target + Vec3::Y * if e.kind == 3 { 0.3 } else { 1.1 };
                            let player_body = player - Vec3::Y * 0.75;
                            if ((player - e.ai.target) * Vec3::new(1., 0., 1.)).length() < radius
                                && world_layout::obstruction(blast_origin, player_body).is_none()
                            {
                                let (attack, amount) = if e.kind == 3 {
                                    (Attack::Slam, 24.)
                                } else {
                                    (Attack::Burst, 18.)
                                };
                                hits.push((e.pos, Cause { kind: e.kind, attack }, amount));
                            }
                            for j in 0..40 {
                                let a = j as f32 * std::f32::consts::TAU / 40.;
                                self.particles.push(crate::game::Particle {
                                    pos: e.ai.target
                                        + Vec3::new(a.cos() * radius, 0.2, a.sin() * radius),
                                    vel: Vec3::Y * 2.,
                                    life: 0.7,
                                    color: if e.kind == 7 {
                                        [0.6, 2., 0.1]
                                    } else {
                                        [3., 0.5, 0.1]
                                    },
                                    size: 0.08,
                                });
                            }
                            if e.kind == 7 {
                                e.hp = 0.;
                            }
                        }
                        4 | 9 => e.ai.charging = 0.65,
                        10 => {
                            if positions.len() + summons.len() + 2 <= 48 && e.ai.summons < 3 {
                                for side in [-1., 1.] {
                                    summons.push(e.pos + Vec3::new(side * 1.3, 0., 0.6));
                                }
                                e.ai.summons += 1;
                            }
                        }
                        11 => {
                            e.pos = e.ai.target;
                            e.attack = 0.65;
                        }
                        _ => {}
                    }
                    e.ai.cooldown = if e.kind == 10 { 8. } else { 4. };
                }
            } else if e.ai.cooldown <= 0.
                && e.anatomy.stagger <= 0.
                && match e.kind {
                    2 | 8 => distance < 20. && clear_sight,
                    10 => distance < 22.,
                    3 | 7 => distance < 5. && clear_sight,
                    4 | 9 => distance < 13.,
                    11 => distance > 4. && distance < 22.,
                    _ => false,
                }
            {
                e.ai.warning = warning_time(e.kind);
                sounds.push((warning_cue(e.kind), e.pos + Vec3::Y));
                e.ai.target = match e.kind {
                    3 | 7 => Vec3::new(e.pos.x, 0., e.pos.z),
                    11 => world_layout::resolve_position(
                        Vec3::new(player.x + dir.z * 2.8, 0., player.z - dir.x * 2.8),
                        ENEMY_RADIUS,
                    ),
                    _ => player,
                };
            }
            if e.hp <= 0. {
                continue;
            }
            if e.ai.charging > 0. {
                e.ai.charging -= dt;
                let to = (e.ai.target - e.pos) * Vec3::new(1., 0., 1.);
                e.pos += to.normalize_or_zero() * (dt * 11.).min(to.length());
            } else if e.ai.warning <= 0. && e.anatomy.stagger <= 0. {
                let mut travel = if distance > stop {
                    pursuit
                } else if ranged && distance < 5. && clear_sight {
                    -dir
                } else {
                    Vec3::ZERO
                };
                if matches!(e.kind, 1 | 4 | 8 | 11) && distance > 3. && clear_sight {
                    travel += Vec3::new(dir.z, 0., -dir.x) * (e.phase * 0.8).sin() * 0.8;
                }
                e.pos += travel
                    * dt
                    * species(e.kind).speed
                    * e.anatomy.speed()
                    * if e.slow > 0. { 0.35 } else { 1. }
                    * if cleanup && distance > 12. { 1.5 } else { 1. };
            }
            let altitude = match e.kind {
                4 => 0.35 + 0.35 * (e.phase * 2.).sin(),
                8 => 0.35 + 0.15 * e.phase.sin(),
                9 => 0.8 + 0.35 * e.phase.sin(),
                _ => 0.,
            };
            e.pos.y += (altitude - e.pos.y) * (dt * 5.).min(1.);
            for (j, p) in positions.iter().enumerate() {
                if index == j {
                    continue;
                }
                let d = (e.pos - *p) * Vec3::new(1., 0., 1.);
                let length = d.length();
                if length > 0.001 && length < 0.65 {
                    e.pos += d / length * (0.65 - length) * dt * 3.;
                }
            }
            // Even low-flying enemies respect masonry; their poses supply altitude.
            // Blink is a teleport, while dives and ordinary movement are swept.
            e.pos = if e.kind == 11 && previous.distance(e.pos) > 5. {
                world_layout::resolve_position(e.pos, ENEMY_RADIUS)
            } else {
                world_layout::move_body(previous, e.pos - previous, ENEMY_RADIUS)
            };
            if !ranged
                && distance < 1.75
                && e.attack <= 0.
                && e.ai.warning <= 0.
                && e.anatomy.stagger <= 0.
                && clear_sight
            {
                e.attack = 1.3 * (1. + e.anatomy.arms() as f32 * 0.6);
                sounds.push(("swing", e.pos + Vec3::Y));
                let amount = (if e.kind == 3 {
                    22.
                } else if e.kind == 5 {
                    5.
                } else {
                    9.
                }) * match e.anatomy.arms() {
                    0 => 1.,
                    1 => 0.55,
                    _ => 0.2,
                };
                let cause = Cause {
                    kind: e.kind,
                    attack: Attack::Strike,
                };
                hits.push((e.pos, cause, amount));
                e.hp -= self.run.survival.ranks[8] as f32 * 12.;
            }
        }
        self.world_sounds.extend(sounds);
        for pos in summons {
            self.run.enemies.push(Enemy::spawn(
                0,
                world_layout::resolve_position(pos, ENEMY_RADIUS),
                self.run.wave,
                self.run.time,
            ));
        }
        for h in &mut self.hazards {
            let a = h.pos;
            h.pos += h.vel * dt;
            h.life -= dt;
            let ab = h.pos - a;
            let nearest =
                a + ab * ((player - a).dot(ab) / ab.length_squared().max(0.0001)).clamp(0., 1.);
            let blocked = world_layout::obstruction(a, h.pos).is_some();
            if nearest.distance(player) < 0.5 && !blocked {
                // A projectile came from opposite its direction of travel.
                let source = if h.vel.length_squared() > 0.01 {
                    player - h.vel
                } else {
                    h.pos
                };
                let cause = Cause {
                    kind: h.from,
                    attack: Attack::Bolt,
                };
                hits.push((source, cause, h.damage));
                h.life = 0.;
            }
            if blocked || h.pos.y < 0.1 || !world_layout::inside_bounds(h.pos, 0.) {
                h.life = 0.;
            }
        }
        self.hazards.retain(|h| h.life > 0.);
        let damage: f32 = hits.iter().map(|hit| hit.2).sum();
        if damage > 0. && self.dash <= 0. {
            let absorbed = self.run.armor.min(damage * 0.65);
            self.run.armor -= absorbed;
            let lost = (damage - absorbed).min(self.run.hp.max(0.));
            self.run.hp -= damage - absorbed;
            self.run.stats.record_wounds(&hits, lost);
            self.hurt = 0.35;
            self.sound_events.push("hurt");
            for (source, ..) in hits {
                self.mark_damage_from(source);
            }
        }
    }
}
