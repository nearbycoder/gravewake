//! Persisted experience, drops, reinforcement director and player powers.
use crate::{
    anatomy::{Part, Pose},
    encounters,
    game::{Enemy, Game, Input, Mode},
    world_layout::{self, ENEMY_RADIUS},
};
use glam::Vec3;
use serde::{Deserialize, Serialize};
pub const DESCENTS: u32 = 12;
pub const POWERS: [(&str, &str); 10] = [
    ("BLOOD OATH", "All weapon damage +15% per rank."),
    ("QUICKENING", "Fire and melee recovery 10% faster per rank."),
    ("SOUL MAGNET", "Collect souls from 1.3m farther per rank."),
    (
        "UNDYING HEART",
        "Maximum vitality +20. Restore 30 vitality.",
    ),
    (
        "SOUL SIPPER",
        "Each collected orb restores 0.6 vitality per rank.",
    ),
    (
        "STORM FAMILIAR",
        "Lightning hunts nearby enemies automatically.",
    ),
    ("GRAVE HALO", "Orbiting blades cut enemies that come close."),
    (
        "WINTER PULSE",
        "Periodic frost nova damages and slows crowds.",
    ),
    (
        "THORN COVENANT",
        "Melee attackers suffer 12 damage per rank.",
    ),
    ("WRAITH STEP", "Movement speed +8% per rank."),
];
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Orb {
    pub pos: Vec3,
    pub value: u32,
    pub age: f32,
    #[serde(default)]
    pub attracted: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Survival {
    pub level: u32,
    pub xp: u32,
    pub ranks: [u8; 10],
    pub pending: u32,
    pub choices: Vec<usize>,
    pub orbs: Vec<Orb>,
    pub remaining: u32,
    pub spawn_timer: f32,
    pub wave_clock: f32,
    pub spawned: u32,
    pub endless: bool,
    pub storm_cd: f32,
    pub halo_cd: f32,
    pub frost_cd: f32,
    /// Species still to come in the current shuffled pass through the pool.
    pub bag: Vec<usize>,
}
impl Default for Survival {
    fn default() -> Self {
        Self {
            level: 1,
            xp: 0,
            ranks: [0; 10],
            pending: 0,
            choices: vec![],
            orbs: vec![],
            remaining: 0,
            spawn_timer: 3.,
            wave_clock: 0.,
            spawned: 0,
            endless: false,
            storm_cd: 1.,
            halo_cd: 0.,
            frost_cd: 3.,
            bag: vec![],
        }
    }
}
impl Survival {
    pub fn threshold(&self) -> u32 {
        12 + self.level * 8
    }
    pub fn max_hp(&self) -> f32 {
        100. + self.ranks[3] as f32 * 20.
    }
}
impl Game {
    pub fn damage_multiplier(&self) -> f32 {
        1. + self.run.survival.ranks[0] as f32 * 0.15
    }
    pub fn haste(&self) -> f32 {
        1. + self.run.survival.ranks[1] as f32 * 0.1
    }
    pub fn max_hp(&self) -> f32 {
        self.run.survival.max_hp()
    }
    pub fn seed_encounter(&mut self) {
        self.run.survival.remaining = (14 + self.run.wave * 5).min(180);
        self.run.survival.spawned = 0;
        self.run.survival.wave_clock = 0.;
        self.run.survival.spawn_timer = 4.;
        self.run.survival.orbs.clear();
        self.run.survival.bag.clear();
        for _ in 0..8 {
            self.spawn_reinforcement();
        }
    }
    pub fn spawn_reinforcement(&mut self) {
        if self.run.survival.remaining == 0 {
            return;
        }
        let n = self.run.survival.spawned;
        let wave = self.run.wave;
        let pool: &[usize] = match wave {
            1 => &[0, 1, 4, 5],
            2 => &[0, 1, 2, 4, 5, 7],
            3 => &[0, 1, 2, 4, 5, 6, 7, 8],
            _ => &[0, 1, 2, 4, 5, 6, 7, 8, 9, 10, 11],
        };
        // Each pass through the pool brings every species once, in a seeded
        // random order, so a wave's mix matches the old round-robin.
        let kind = if wave % 4 == 0 && n == 0 {
            3
        } else {
            if self.run.survival.bag.is_empty() {
                let mut bag = pool.to_vec();
                for i in (1..bag.len()).rev() {
                    let j = ((self.rand() * (i + 1) as f32) as usize).min(i);
                    bag.swap(i, j);
                }
                self.run.survival.bag = bag;
            }
            self.run.survival.bag.pop().unwrap()
        };
        let angle = self.rand() * std::f32::consts::TAU;
        let distance = 14. + self.rand() * 6.;
        let pos = world_layout::spawn_point(self.run.pos, angle, distance, ENEMY_RADIUS);
        self.run
            .enemies
            .push(Enemy::spawn(kind, pos, wave, self.rand_phase(n)));
        self.run.survival.remaining -= 1;
        self.run.survival.spawned += 1;
    }
    fn rand_phase(&self, n: u32) -> f32 {
        n as f32 * 2.17 + self.run.wave as f32
    }
    pub fn tick_director(&mut self, dt: f32) {
        self.run.survival.wave_clock += dt;
        self.run.survival.spawn_timer -= dt;
        // Keep the fight with the player when they cross the larger grounds.
        // Preserve the enemy and its damage; move only distant, unseen stragglers.
        let forward = Vec3::new(self.run.yaw.sin(), 0., -self.run.yaw.cos());
        let player = self.run.pos;
        for (index, enemy) in self.run.enemies.iter_mut().enumerate() {
            if enemy.hp <= 0. || enemy.ai.warning > 0. || enemy.ai.charging > 0. {
                continue;
            }
            let flat = (enemy.pos - player) * Vec3::new(1., 0., 1.);
            if flat.length_squared() > 38. * 38.
                && (flat.normalize_or_zero().dot(forward) < -0.15
                    || world_layout::obstruction(player, enemy.pos + Vec3::Y * 1.5).is_some())
            {
                let angle = -self.run.yaw + (index as f32 * 0.73).sin() * 0.8;
                let candidate = world_layout::spawn_point(player, angle, 19., ENEMY_RADIUS);
                let relative = (candidate - player) * Vec3::new(1., 0., 1.);
                // Never materialize a recycled enemy directly in the player's view.
                if relative.normalize_or_zero().dot(forward) < -0.15
                    || world_layout::obstruction(player, candidate + Vec3::Y * 1.5).is_some()
                {
                    enemy.pos = candidate;
                    enemy.ai.cooldown = enemy.ai.cooldown.max(1.5);
                    enemy.attack = enemy.attack.max(1.5);
                }
            }
        }
        if self.run.survival.remaining > 0
            && self.run.enemies.len() < 40
            && (self.run.survival.spawn_timer <= 0. || self.run.enemies.is_empty())
        {
            let batch = 3 + (self.run.wave / 3).min(5);
            for _ in 0..batch {
                self.spawn_reinforcement();
            }
            self.run.survival.spawn_timer = (5.5 - self.run.wave as f32 * 0.18).max(2.);
        }
    }
    pub fn add_xp(&mut self, amount: u32) {
        self.run.survival.xp += amount;
        while self.run.survival.xp >= self.run.survival.threshold() {
            self.run.survival.xp -= self.run.survival.threshold();
            self.run.survival.level += 1;
            self.run.survival.pending += 1;
        }
    }
    pub fn tick_orbs(&mut self, dt: f32) {
        let magnet = 2.2 + self.run.survival.ranks[2] as f32 * 1.3;
        let vacuum = self.run.enemies.is_empty() && self.run.survival.remaining == 0;
        let target = self.run.pos - Vec3::Y * 0.7;
        let mut xp = 0;
        let mut count = 0;
        for o in &mut self.run.survival.orbs {
            o.age += dt;
            let d = target - o.pos;
            let flat = d * Vec3::new(1., 0., 1.);
            if vacuum || flat.length() < magnet {
                o.attracted = true;
            }
            if o.attracted {
                o.pos += d.normalize_or_zero()
                    * (dt
                        * if vacuum {
                            (d.length() * 4.).max(22.)
                        } else {
                            13.
                        })
                    .min(d.length());
            } else {
                o.pos.y += (0.48 + (o.age * 3.).sin() * 0.12 - o.pos.y) * (dt * 6.).min(1.);
            }
            if o.pos.distance(target) < 0.55 {
                xp += o.value;
                o.value = 0;
                count += 1;
            }
        }
        self.run.survival.orbs.retain(|o| o.value > 0);
        if count > 0 {
            self.add_xp(xp);
            self.run.hp = (self.run.hp + count as f32 * self.run.survival.ranks[4] as f32 * 0.6)
                .min(self.max_hp());
            self.sound_events.push("soul");
        }
    }
    pub fn offer_power(&mut self) {
        if self.run.survival.pending == 0 {
            return;
        }
        if self.run.survival.choices.is_empty() {
            let mut pool: Vec<_> = (0..10)
                .filter(|i| self.run.survival.ranks[*i] < 5)
                .collect();
            if pool.is_empty() {
                self.run.hp = self.max_hp();
                self.run.armor += 10. * self.run.survival.pending as f32;
                self.run.survival.pending = 0;
                return;
            }
            for _ in 0..3.min(pool.len()) {
                let j = (self.rand() * pool.len() as f32) as usize;
                self.run
                    .survival
                    .choices
                    .push(pool.remove(j.min(pool.len() - 1)));
            }
        }
        self.mode = Mode::LevelUp;
        self.input = Input::default();
        self.sound_events.push("level_up");
        self.save();
    }
    pub fn choose_power(&mut self, index: usize) {
        if self.mode != Mode::LevelUp {
            return;
        }
        let Some(&p) = self.run.survival.choices.get(index) else {
            return;
        };
        self.run.survival.ranks[p] = (self.run.survival.ranks[p] + 1).min(5);
        if p == 3 {
            self.run.hp = (self.run.hp + 30.).min(self.max_hp());
        }
        self.run.survival.pending = self.run.survival.pending.saturating_sub(1);
        self.run.survival.choices.clear();
        self.mode = Mode::Arena;
        self.input = Input::default();
        self.notify(&format!(
            "{}  /  RANK {}",
            POWERS[p].0, self.run.survival.ranks[p]
        ));
        self.offer_power();
        self.save();
    }
    pub fn tick_powers(&mut self, dt: f32) {
        let ranks = self.run.survival.ranks;
        self.run.survival.storm_cd -= dt;
        self.run.survival.halo_cd -= dt;
        self.run.survival.frost_cd -= dt;
        if ranks[5] > 0 && self.run.survival.storm_cd <= 0. {
            self.run.survival.storm_cd = 2.8 - ranks[5] as f32 * 0.22;
            let mut targets: Vec<_> = self
                .run
                .enemies
                .iter()
                .enumerate()
                .filter(|(_, e)| e.hp > 0.)
                .map(|(i, e)| (i, e.pos.distance_squared(self.run.pos)))
                .filter(|(_, d)| *d < 196.)
                .collect();
            targets.sort_by(|a, b| a.1.total_cmp(&b.1));
            for &(i, _) in targets.iter().take(ranks[5] as usize) {
                let p = Pose::for_enemy(&self.run.enemies[i], self.run.pos).anchor(
                    if encounters::head_only(self.run.enemies[i].kind) {
                        Part::Head
                    } else {
                        Part::Torso
                    },
                );
                self.trail(p + Vec3::Y * 5., p, [0.4, 1.5, 3.]);
                self.damage_enemy(i, 22. + ranks[5] as f32 * 9., -Vec3::Y);
            }
        }
        if ranks[6] > 0 && self.run.survival.halo_cd <= 0. {
            self.run.survival.halo_cd = 0.22;
            for i in 0..self.run.enemies.len() {
                for blade in 0..ranks[6] + 1 {
                    let a = self.run.time * 2.7
                        + blade as f32 * std::f32::consts::TAU / (ranks[6] + 1) as f32;
                    let p = self.run.pos + Vec3::new(a.cos() * 2.5, -0.45, a.sin() * 2.5);
                    let e = &self.run.enemies[i];
                    if e.hp > 0. && e.pos.distance(p) < 1.65 {
                        self.damage_enemy(
                            i,
                            10. + ranks[6] as f32 * 4.,
                            (e.pos - self.run.pos).normalize_or_zero(),
                        );
                        break;
                    }
                }
            }
        }
        if ranks[7] > 0 && self.run.survival.frost_cd <= 0. {
            self.run.survival.frost_cd = 5.;
            let radius = 3.5 + ranks[7] as f32 * 0.7;
            for i in 0..self.run.enemies.len() {
                let e = &self.run.enemies[i];
                if e.hp > 0. && e.pos.distance(self.run.pos) < radius {
                    self.run.enemies[i].slow = 3.;
                    self.damage_enemy(i, 18. + ranks[7] as f32 * 8., Vec3::ZERO);
                }
            }
            for j in 0..64 {
                let a = j as f32 * std::f32::consts::TAU / 64.;
                self.particles.push(crate::game::Particle {
                    pos: self.run.pos - Vec3::Y,
                    vel: Vec3::new(a.cos() * radius, 0., a.sin() * radius) * 2.,
                    life: 0.55,
                    color: [0.3, 1.7, 3.],
                    size: 0.07,
                });
            }
            self.sound_events.push("spell");
        }
        self.collect_dead();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn arena() -> Game {
        let mut g = Game::new(false);
        g.new_run();
        g.run.enemies.clear();
        g.run.survival.remaining = 0;
        g
    }
    #[test]
    fn radial_specials_respect_cover_when_arming_and_when_detonating() {
        for kind in [3, 7] {
            for (z, clear) in [(-14., false), (-5., true)] {
                let mut g = arena();
                g.run.pos = Vec3::new(-18.8, 1.65, z);
                let origin = Vec3::new(-21.2, 0., z);
                let mut enemy = Enemy::spawn(kind, origin, 1, 0.);
                enemy.ai.cooldown = 0.;
                g.run.enemies.push(enemy);
                g.tick_enemies(0.01);
                assert_eq!(
                    g.run.enemies[0].ai.warning > 0.,
                    clear,
                    "arming kind {kind}, z {z}"
                );
                // A warning can predate taking cover: detonation must check again.
                g.run.enemies[0].ai.target = origin;
                g.run.enemies[0].ai.warning = 0.01;
                g.tick_enemies(0.02);
                let expected_damage = if !clear {
                    0.
                } else if kind == 3 {
                    24.
                } else {
                    18.
                };
                assert_eq!(
                    g.run.hp,
                    100. - expected_damage,
                    "detonation kind {kind}, z {z}"
                );
            }
        }
    }
    #[test]
    fn hazards_travel_beyond_old_bounds_and_stop_at_real_cover() {
        let mut g = arena();
        g.run.pos = Vec3::new(30., 1.65, 20.);
        g.hazards.push(encounters::Hazard {
            pos: Vec3::new(28., 1.5, 16.),
            vel: Vec3::Z * 3.,
            life: 5.,
            damage: 8.,
            color: [1., 0., 0.],
            from: 8,
        });
        g.tick_enemies(0.1);
        assert_eq!(g.hazards.len(), 1);
        g.hazards.clear();
        g.hazards.push(encounters::Hazard {
            pos: Vec3::new(-17., 1.5, -14.),
            vel: -Vec3::X * 30.,
            life: 5.,
            damage: 8.,
            color: [1., 0., 0.],
            from: 8,
        });
        g.tick_enemies(0.2);
        assert!(g.hazards.is_empty());
    }
    #[test]
    fn expanded_grounds_keep_reinforcements_near_each_district() {
        let mut g = arena();
        for district in world_layout::DISTRICTS {
            g.run.pos = district.center + Vec3::Y * 1.65;
            g.run.wave = 8;
            g.run.enemies.clear();
            g.seed_encounter();
            assert_eq!(g.run.enemies.len(), 8);
            for enemy in &g.run.enemies {
                assert!(world_layout::is_walkable(enemy.pos, ENEMY_RADIUS));
                let distance = ((enemy.pos - g.run.pos) * Vec3::new(1., 0., 1.)).length();
                assert!((9.9..=20.1).contains(&distance));
            }
        }
    }
    #[test]
    fn unseen_distant_stragglers_return_without_resetting_health_or_wave_progress() {
        let mut g = arena();
        g.run.pos = Vec3::new(0., 1.65, -20.);
        let mut enemy = Enemy::spawn(6, Vec3::new(0., 0., 35.), 1, 0.);
        enemy.hp = 17.;
        g.run.enemies.push(enemy);
        g.run.survival.spawned = 19;
        g.tick_director(0.016);
        assert!(g.run.enemies[0].pos.distance(g.run.pos) < 21.);
        assert_eq!(g.run.enemies[0].hp, 17.);
        assert_eq!(g.run.survival.spawned, 19);
        assert_eq!(g.run.survival.remaining, 0);
        assert!(g.run.enemies[0].pos.z > g.run.pos.z);
    }
    #[test]
    fn final_souls_cross_the_entire_ground_promptly() {
        let mut g = arena();
        g.run.pos = Vec3::new(-45., 1.65, -45.);
        g.run.survival.orbs.push(Orb {
            pos: Vec3::new(45., 0.5, 45.),
            value: 5,
            age: 0.,
            attracted: false,
        });
        for _ in 0..100 {
            g.tick_orbs(0.02);
        }
        assert!(g.run.survival.orbs.is_empty());
        assert_eq!(g.run.survival.xp, 5);
    }
    #[test]
    fn every_species_drops_once_and_orbs_wait_for_collection() {
        let mut g = arena();
        let total: u32 = encounters::ROSTER.iter().map(|s| s.xp).sum();
        for k in 0..12 {
            let mut e = Enemy::spawn(k, Vec3::ZERO, 1, 0.);
            e.hp = 0.;
            g.run.enemies.push(e);
        }
        g.collect_dead();
        g.collect_dead();
        assert_eq!(g.run.kills, 12);
        assert_eq!(g.run.survival.orbs.len(), 12);
        assert_eq!(g.run.survival.xp, 0);
        // Active enemies prevent the end-of-wave vacuum; drops never expire.
        g.run.enemies.push(Enemy::spawn(0, Vec3::ZERO, 1, 0.));
        for _ in 0..100 {
            g.tick_orbs(1.);
        }
        assert_eq!(g.run.survival.orbs.len(), 12);
        g.run.pos = Vec3::new(0., 1.65, 0.);
        for _ in 0..30 {
            g.tick_orbs(0.05);
        }
        assert!(g.run.survival.orbs.is_empty());
        let used: u32 = (1..g.run.survival.level).map(|l| 12 + l * 8).sum();
        assert_eq!(used + g.run.survival.xp, total);
    }
    #[test]
    fn final_drops_award_levels_before_shop_and_selection_resumes() {
        let mut g = arena();
        g.run.survival.orbs.push(Orb {
            pos: Vec3::new(14., 0.5, -14.),
            value: 100,
            age: 0.,
            attracted: false,
        });
        for _ in 0..200 {
            g.update(0.02);
            if g.mode == Mode::LevelUp {
                break;
            }
        }
        assert_eq!(g.mode, Mode::LevelUp);
        assert!(g.run.survival.orbs.is_empty());
        assert_eq!(g.run.survival.pending, 3);
        let time = g.run.time;
        g.update(1.);
        assert_eq!(g.run.time, time);
        let data = serde_json::to_vec(&g.run).unwrap();
        g.run = serde_json::from_slice(&data).unwrap();
        for _ in 0..3 {
            assert_eq!(g.mode, Mode::LevelUp);
            assert_eq!(g.run.survival.choices.len(), 3);
            g.choose_power(0);
        }
        g.update(0.02);
        assert_eq!(g.mode, Mode::Shop);
        assert_eq!(
            g.run.survival.ranks.iter().map(|x| *x as u32).sum::<u32>(),
            3
        );
    }
    #[test]
    fn legacy_save_defaults_and_power_cap_are_safe() {
        let g = arena();
        let mut json = serde_json::to_value(&g.run).unwrap();
        json.as_object_mut().unwrap().remove("survival");
        let restored: crate::game::Run = serde_json::from_value(json).unwrap();
        assert_eq!(restored.survival.level, 1);
        let mut g = arena();
        g.run.survival.ranks = [5; 10];
        g.run.survival.pending = 3;
        g.offer_power();
        assert_eq!(g.run.survival.pending, 0);
        assert_eq!(g.run.armor, 30.);
    }
    #[test]
    fn director_staggers_reinforcements_and_covers_roster() {
        let mut seen = [false; 12];
        let mut g = arena();
        for wave in 1..=12 {
            g.run.wave = wave;
            g.start_wave();
            let first = g.run.enemies.len();
            assert_eq!(first, 8);
            assert!(g.run.survival.remaining > 0);
            while g.run.survival.remaining > 0 {
                for e in &g.run.enemies {
                    seen[e.kind] = true;
                }
                g.run.enemies.clear();
                g.tick_director(0.1);
            }
            for e in &g.run.enemies {
                seen[e.kind] = true;
            }
        }
        assert!(seen.into_iter().all(|v| v));
    }
    #[test]
    fn automatic_powers_damage_and_freeze_without_input() {
        for power in [5, 6, 7] {
            let mut g = arena();
            g.run.pos = Vec3::new(0., 1.65, 0.);
            g.run.survival.ranks[power] = 2;
            g.run.survival.storm_cd = 0.;
            g.run.survival.frost_cd = 0.;
            let mut e = Enemy::spawn(3, Vec3::new(2.5, 0., 0.), 1, 0.);
            e.hp = 600.;
            g.run.enemies.push(e);
            g.tick_powers(0.01);
            assert!(g.run.enemies[0].hp < 600., "power {power}");
            if power == 7 {
                assert!(g.run.enemies[0].slow > 0.);
            }
        }
    }
    #[test]
    fn upgrades_change_weapon_damage_health_magnet_and_speed() {
        let shoot = |rank| {
            let mut g = arena();
            g.run.pos = Vec3::new(0., 1.2, 3.);
            let mut e = Enemy::spawn(0, Vec3::ZERO, 1, 0.);
            e.hp = 500.;
            e.max_hp = 500.;
            g.run.enemies.push(e);
            g.run.survival.ranks[0] = rank;
            g.run.survival.ranks[1] = rank;
            g.fire(false);
            (500. - g.run.enemies[0].hp, g.cooldown)
        };
        let base = shoot(0);
        let upgraded = shoot(3);
        assert!(upgraded.0 > base.0 * 1.4);
        assert!(upgraded.1 < base.1);
        let mut g = arena();
        g.run.hp = 60.;
        g.mode = Mode::LevelUp;
        g.run.survival.pending = 1;
        g.run.survival.choices = vec![3];
        g.choose_power(0);
        assert_eq!(g.max_hp(), 120.);
        assert_eq!(g.run.hp, 90.);
        g.run.survival.ranks[2] = 2;
        g.run.survival.ranks[4] = 2;
        g.run.enemies.push(Enemy::spawn(0, Vec3::ZERO, 1, 0.));
        g.run.survival.orbs.push(Orb {
            pos: g.run.pos + Vec3::X * 4.,
            value: 5,
            age: 0.,
            attracted: false,
        });
        for _ in 0..40 {
            g.tick_orbs(0.02);
        }
        assert!(g.run.survival.orbs.is_empty());
        assert!(g.run.hp > 90.);
        let movement = |rank| {
            let mut g = arena();
            g.run.survival.ranks[9] = rank;
            g.input.forward = 1.;
            let before = g.run.pos;
            g.update(0.1);
            g.run.pos.distance(before)
        };
        assert!(movement(3) > movement(0) * 1.2);
    }
    #[test]
    fn summoning_respects_the_live_enemy_budget() {
        let mut g = arena();
        g.run.enemies = (0..47)
            .map(|i| {
                Enemy::spawn(
                    if i == 0 { 10 } else { 0 },
                    Vec3::new(i as f32 % 8., 0., -5.),
                    1,
                    0.,
                )
            })
            .collect();
        g.run.enemies[0].ai.warning = 0.01;
        g.tick_enemies(0.02);
        assert!(g.run.enemies.len() <= 48);
    }
    #[test]
    fn ranged_warnings_launch_dodgeable_projectiles_and_summons_are_bounded() {
        let mut g = arena();
        g.run.pos = Vec3::new(0., 1.65, 7.);
        let mut skull = Enemy::spawn(2, Vec3::ZERO, 1, 0.);
        skull.ai.cooldown = 0.;
        g.run.enemies.push(skull);
        g.tick_enemies(0.01);
        assert!(g.run.enemies[0].ai.warning > 0.);
        assert!(g.hazards.is_empty());
        for _ in 0..60 {
            g.tick_enemies(0.016);
        }
        assert!(!g.hazards.is_empty());
        assert_eq!(g.run.hp, 100.);
        g.run.pos.x = 5.;
        for _ in 0..100 {
            g.tick_enemies(0.016);
        }
        assert_eq!(g.run.hp, 100.);
        g.run.enemies.clear();
        let mut summoner = Enemy::spawn(10, Vec3::ZERO, 1, 0.);
        summoner.ai.cooldown = 0.;
        g.run.enemies.push(summoner);
        for _ in 0..3000 {
            g.tick_enemies(0.016);
        }
        assert!(g.run.enemies.len() <= 7);
    }
}
