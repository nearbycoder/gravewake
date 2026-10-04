//! Repeatable native-renderer benchmark. No screenshots or disk I/O in timed frames.
use crate::{
    anatomy::BodyEvent,
    game::{Enemy, Game, Mode},
    scene::Bones,
};
use glam::Vec3;
use serde::Serialize;
#[derive(Default)]
pub struct Benchmark {
    pub samples: Vec<[f64; 5]>,
    pub reports: Vec<Report>,
    pub vertices: usize,
}
#[derive(Serialize)]
pub struct Report {
    pub scene: String,
    pub width: u32,
    pub height: u32,
    pub presentation: String,
    pub frames: usize,
    pub fps: f64,
    pub frame_ms: f64,
    pub p95_ms: f64,
    pub simulation_ms: f64,
    pub mesh_ms: f64,
    pub acquire_ms: f64,
    pub submit_ms: f64,
    pub max_vertices: usize,
}
impl Benchmark {
    pub fn setup(g: &mut Game, b: &mut Bones, scene: u32) {
        g.new_run();
        g.elapsed = 0.;
        g.run.survival.remaining = 0;
        g.run.survival.orbs.clear();
        g.mode = Mode::Arena;
        g.notice_time = 0.;
        g.run.pos = Vec3::new(0., 1.65, 14.);
        let count = if scene == 0 {
            12
        } else if scene == 1 {
            48
        } else {
            24
        };
        g.run.enemies = (0..count)
            .map(|i| {
                Enemy::spawn(
                    i % 12,
                    Vec3::new((i % 8) as f32 * 2. - 7., 0., 4. - (i / 8) as f32 * 2.6),
                    4,
                    i as f32,
                )
            })
            .collect();
        *b = Bones::new();
        if scene == 2 {
            for i in 0..24 {
                b.spawn_body(BodyEvent {
                    enemy: Enemy::spawn(
                        0,
                        Vec3::new((i % 8) as f32 * 1.5 - 5., 0.5, -(i / 8) as f32 * 2.),
                        4,
                        i as f32,
                    ),
                    target: g.run.pos,
                    part: None,
                    impulse: Vec3::new(1., 2., -1.),
                    impact: Vec3::new((i % 8) as f32 * 1.5 - 5., 1.5, -(i / 8) as f32 * 2.),
                    energy: 30.,
                    fracture: false,
                });
            }
        }
    }
    pub fn finish(&mut self, scene: u32, width: u32, height: u32, presentation: String) {
        let n = self.samples.len();
        let mut frames: Vec<_> = self.samples.iter().map(|v| v[0]).collect();
        frames.sort_by(f64::total_cmp);
        let avg = |i: usize| self.samples.iter().map(|v| v[i]).sum::<f64>() / n as f64;
        let report = Report {
            scene: format!(
                "{} / {}",
                if scene < 3 {
                    "CPU reference"
                } else {
                    "optimized"
                },
                [
                    "12 enemies",
                    "48 enemies",
                    "24 enemies + 14 articulated corpses (140 sections)",
                ][(scene % 3) as usize]
            ),
            width,
            height,
            presentation,
            frames: n,
            fps: 1000. / avg(0),
            frame_ms: avg(0),
            p95_ms: frames[(n as f64 * 0.95) as usize],
            simulation_ms: avg(1),
            mesh_ms: avg(2),
            acquire_ms: avg(3),
            submit_ms: avg(4),
            max_vertices: self.vertices,
        };
        println!("BENCH {}", serde_json::to_string(&report).unwrap());
        self.reports.push(report);
        self.samples.clear();
        self.vertices = 0;
    }
    pub fn save(&self) {
        let label = std::env::var("GRAVEWAKE_BENCH_LABEL").unwrap_or("latest".into());
        let label: String = label
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
            .collect();
        std::fs::create_dir_all("captures/performance").unwrap();
        std::fs::write(
            format!("captures/performance/{label}.json"),
            serde_json::to_vec_pretty(&self.reports).unwrap(),
        )
        .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{Mesh, Visibility};
    use glam::Mat4;
    #[test]
    fn visibility_rejects_behind_camera_but_keeps_intersecting_bounds() {
        let vp = Mat4::perspective_rh(70f32.to_radians(), 1.6, 0.04, 120.)
            * Mat4::look_at_rh(Vec3::ZERO, -Vec3::Z, Vec3::Y);
        let v = Visibility::new(vp);
        assert!(v.contains(Vec3::new(0., 0., -5.), 1.));
        assert!(!v.contains(Vec3::new(0., 0., 5.), 1.));
        assert!(!v.contains(Vec3::new(50., 0., -5.), 1.));
        assert!(v.contains(Vec3::new(0., 0., 0.2), 1.));
        assert!(!v.contains(Vec3::new(0., 0., -150.), 1.));
    }
    #[test]
    fn instances_preserve_full_detail_geometry_and_enemy_status() {
        let mut direct = Mesh::new();
        let matrix = Mat4::from_rotation_y(0.8) * Mat4::from_translation(Vec3::new(3., 1., -5.));
        let center = Vec3::new(0.2, 1.3, 0.1);
        let radius = Vec3::new(0.18, 0.21, 0.157);
        direct.transform = matrix;
        direct.ellipsoid(center, radius, [0.4, 0.5, 0.6], 21.);
        let mut instanced = Mesh::new();
        instanced.transform = matrix;
        instanced.instance_spheres = true;
        instanced.ellipsoid(center, radius, [0.4, 0.5, 0.6], 21.);
        assert!(instanced.vertices.is_empty());
        assert_eq!(instanced.spheres.len(), 1);
        assert_eq!(instanced.spheres[0].center_material[3], 21.);
        let mut unit = Mesh::new();
        unit.ellipsoid(Vec3::ZERO, Vec3::ONE, [1.; 3], 2.);
        assert_eq!(unit.vertices.len(), direct.vertices.len());
        for (source, expected) in unit.vertices.iter().zip(&direct.vertices) {
            let p = matrix.transform_point3(Vec3::from_array(source.pos) * radius + center);
            assert!(p.distance(Vec3::from_array(expected.pos)) < 0.00001);
        }
    }
    #[test]
    fn optimized_scene_keeps_collision_and_simulation_state_unchanged() {
        let mut g = Game::new(false);
        let mut bones = Bones::new();
        Benchmark::setup(&mut g, &mut bones, 1);
        g.run.enemies[0].burn = 2.;
        g.run.enemies[1].slow = 2.;
        g.run.enemies[2].poison = 2.;
        let before = serde_json::to_vec(&g.run).unwrap();
        let vp = Mat4::perspective_rh(70f32.to_radians(), 1.6, 0.04, 120.)
            * Mat4::look_at_rh(g.run.pos, g.run.pos - Vec3::Z, Vec3::Y);
        let mut reference = Mesh::new();
        let mut optimized = Mesh::new();
        crate::scene::dynamic(&g, &bones, &mut reference, vp, false);
        crate::scene::dynamic(&g, &bones, &mut optimized, vp, true);
        assert_eq!(optimized.enemies.len(), g.run.enemies.len());
        assert!(reference.enemies.is_empty());
        let (_, ranges) = crate::enemy_assets::gpu_geometry();
        assert!(
            optimized
                .enemies
                .iter()
                .all(|instance| (instance.params[0] as usize) < ranges.len())
        );
        assert!(
            optimized
                .enemies
                .iter()
                .any(|instance| (12. ..24.).contains(&instance.params[0]))
        );
        assert!(
            optimized
                .enemies
                .iter()
                .any(|instance| instance.params[0] >= 24.)
        );
        for status in [20., 21., 22.] {
            assert!(
                optimized
                    .enemies
                    .iter()
                    .any(|instance| instance.params[1] == status)
            );
        }
        assert!(optimized.vertices.len() < reference.vertices.len() / 2);
        assert_eq!(before, serde_json::to_vec(&g.run).unwrap());
        let capacity = optimized.vertices.capacity();
        let enemy_capacity = optimized.enemies.capacity();
        crate::scene::dynamic(&g, &bones, &mut optimized, vp, true);
        assert_eq!(optimized.vertices.capacity(), capacity);
        assert_eq!(optimized.enemies.len(), g.run.enemies.len());
        assert_eq!(optimized.enemies.capacity(), enemy_capacity);
    }
}
