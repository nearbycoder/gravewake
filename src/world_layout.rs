//! One footprint map for the expanded grounds, movement, navigation and physics.
//! Architecture is deliberately porous: every court has several broad exits.
use glam::{Vec2, Vec3};
use std::{cmp::Reverse, collections::BinaryHeap, sync::OnceLock};

pub const HALF_EXTENT: f32 = 48.;
pub const PLAYER_RADIUS: f32 = 0.38;
pub const ENEMY_RADIUS: f32 = 0.38;

#[derive(Clone, Copy, Debug)]
pub struct Wall {
    pub x: f32,
    pub z: f32,
    pub half_x: f32,
    pub half_z: f32,
    pub height: f32,
}
const fn wall(x: f32, z: f32, half_x: f32, half_z: f32, height: f32) -> Wall {
    Wall {
        x,
        z,
        half_x,
        half_z,
        height,
    }
}
/// West chapel (-29,-8), east cloister (29,-7), north sanctuary (0,-31).
/// The south orchard (0,28) and central crossing remain open ground.
pub const WALLS: [Wall; 19] = [
    wall(-38., -14.5, 0.65, 5.5, 5.8),
    wall(-38., 1.5, 0.65, 2.5, 3.2),
    wall(-20., -14.5, 0.65, 5.5, 4.4),
    wall(-20., 1.5, 0.65, 2.5, 2.7),
    wall(-35., -20., 3., 0.65, 5.2),
    wall(-23., -20., 3., 0.65, 3.7),
    wall(-35., 4., 3., 0.65, 5.4),
    wall(-23., 4., 3., 0.65, 5.4),
    wall(25., -20., 4., 0.6, 3.2),
    wall(36., -20., 2., 0.6, 4.3),
    wall(38., -14., 0.6, 6., 4.8),
    wall(38., 4., 0.6, 3., 2.6),
    wall(25., 7., 4., 0.6, 2.4),
    wall(36., 7., 2., 0.6, 3.7),
    wall(-9., -34., 0.7, 6., 4.2),
    wall(9., -34., 0.7, 6., 4.8),
    wall(-6., -40., 3., 0.7, 3.5),
    wall(6., -40., 3., 0.7, 4.5),
    wall(0., -35., 3.4, 3.4, 20.1),
];
/// Solid standing monuments. Small gravestones and loose rubble are decorative.
pub const PILLARS: [(f32, f32, f32); 9] = [
    (-12., -9., 0.95),
    (12., -9., 0.95),
    (-13., 11., 0.9),
    (13., 11., 0.9),
    (-25., 25., 1.1),
    (25., 25., 1.1),
    (-5., 34., 1.17),
    (7., 29., 1.17),
    (0., -8., 2.8),
];

/// Visible solid heights, shared with Rapier. The final obstacle is the basin.
pub fn pillar_height(index: usize) -> f32 {
    match index {
        6 | 7 => 4.35,
        8 => 2.18,
        _ => 6.,
    }
}
#[derive(Clone, Copy)]
pub struct PillarBand {
    pub bottom: f32,
    pub top: f32,
    pub radius: f32,
}
/// Stacked solids preserve low cover: shots and limbs can pass above the basin
/// beside its narrow urn, instead of striking an invisible full-height drum.
pub fn pillar_bands(index: usize) -> impl Iterator<Item = PillarBand> {
    let band = |bottom, top, radius| PillarBand {
        bottom,
        top,
        radius,
    };
    let bands = match index {
        8 => [
            band(0., 0.31, 2.8),
            band(0.31, 0.87, 2.5),
            band(0.87, 1.45, 0.49),
            band(1.45, 2.18, 0.62),
        ],
        6 | 7 => [
            band(0., 0.37, 1.17),
            band(0.37, 1.12, 0.83),
            band(1.12, 4.35, 0.61),
            band(0., 0., 0.),
        ],
        _ => [
            band(0., pillar_height(index), PILLARS[index].2),
            band(0., 0., 0.),
            band(0., 0., 0.),
            band(0., 0., 0.),
        ],
    };
    bands.into_iter().filter(|band| band.top > band.bottom)
}

pub const FIRES: [[f32; 3]; 18] = [
    [-6., 1.4, 1.],
    [6., 1.4, -2.],
    [-9., 1.6, -18.],
    [9., 1.6, -18.],
    [-14., 1.6, 14.],
    [14., 1.6, 14.],
    [-33., 1.4, 6.],
    [-25., 1.4, 6.],
    [-34., 1.4, -16.],
    [-24., 1.4, -16.],
    [24., 1.4, -16.],
    [35., 1.4, -4.],
    [26., 1.4, 4.],
    [35., 1.4, 9.],
    [-6., 1.4, -29.],
    [6., 1.4, -29.],
    [-9., 1.4, 28.],
    [10., 1.4, 35.],
];
pub struct GravePlacement {
    pub position: Vec3,
    pub yaw: f32,
    pub lean: f32,
}
fn layout_noise(i: u32) -> f32 {
    let x = i.wrapping_mul(747796405).wrapping_add(2891336453);
    let x = ((x >> ((x >> 28) + 4)) ^ x).wrapping_mul(277803737);
    ((x >> 22) ^ x) as f32 / u32::MAX as f32
}
pub fn grave_placement(index: u32) -> GravePlacement {
    let side = if index % 2 == 0 { -1. } else { 1. };
    GravePlacement {
        position: Vec3::new(
            side * (12. + layout_noise(index + 700) * 25.),
            0.,
            18. + layout_noise(index + 800) * 23.,
        ),
        yaw: (layout_noise(index + 900) - 0.5) * 0.8,
        lean: (layout_noise(index + 930) - 0.5) * 0.10,
    }
}
pub fn is_spawnable(p: Vec3, radius: f32) -> bool {
    is_walkable(p, radius)
        && FIRES.iter().all(|fire| {
            Vec2::new(p.x - fire[0], p.z - fire[2]).length_squared() > (0.51 + radius).powi(2)
        })
        && (0..64).all(|i| {
            let grave = grave_placement(i).position;
            Vec2::new(p.x - grave.x, p.z - grave.z).length_squared() > (0.58 + radius).powi(2)
        })
}

#[derive(Clone, Copy)]
pub struct District {
    pub name: &'static str,
    pub center: Vec3,
}
pub const DISTRICTS: [District; 5] = [
    District {
        name: "MOURNING COURT",
        center: Vec3::new(0., 0., 0.),
    },
    District {
        name: "RUINED CHAPEL",
        center: Vec3::new(-29., 0., -8.),
    },
    District {
        name: "SUNKEN CLOISTER",
        center: Vec3::new(29., 0., -7.),
    },
    District {
        name: "BELL SANCTUARY",
        center: Vec3::new(0., 0., -30.),
    },
    District {
        name: "ASH ORCHARD",
        center: Vec3::new(0., 0., 28.),
    },
];
pub fn district_at(position: Vec3) -> usize {
    DISTRICTS
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            a.center
                .distance_squared(position)
                .total_cmp(&b.center.distance_squared(position))
        })
        .map(|(i, _)| i)
        .unwrap_or(0)
}

pub fn inside_bounds(p: Vec3, radius: f32) -> bool {
    p.x.abs() <= HALF_EXTENT - radius && p.z.abs() <= HALF_EXTENT - radius
}
pub fn clamp_bounds(mut p: Vec3, radius: f32) -> Vec3 {
    let edge = HALF_EXTENT - radius;
    p.x = p.x.clamp(-edge, edge);
    p.z = p.z.clamp(-edge, edge);
    p
}
pub fn is_walkable(p: Vec3, radius: f32) -> bool {
    inside_bounds(p, radius)
        && !WALLS
            .iter()
            .any(|w| (p.x - w.x).abs() < w.half_x + radius && (p.z - w.z).abs() < w.half_z + radius)
        && !PILLARS
            .iter()
            .any(|&(x, z, r)| Vec2::new(p.x - x, p.z - z).length_squared() < (r + radius).powi(2))
}
/// Resolve legacy saves and teleports as well as ordinary overlap. Choosing the
/// nearest face also handles the exact centre of a wall or circular monument.
pub fn resolve_position(mut p: Vec3, radius: f32) -> Vec3 {
    p = clamp_bounds(p, radius);
    for _ in 0..4 {
        for w in WALLS {
            let dx = p.x - w.x;
            let dz = p.z - w.z;
            let px = w.half_x + radius - dx.abs();
            let pz = w.half_z + radius - dz.abs();
            if px > 0. && pz > 0. {
                if px < pz {
                    p.x += if dx < 0. { -px - 0.002 } else { px + 0.002 };
                } else {
                    p.z += if dz < 0. { -pz - 0.002 } else { pz + 0.002 };
                }
            }
        }
        for (x, z, r) in PILLARS {
            let d = Vec2::new(p.x - x, p.z - z);
            if d.length_squared() < (r + radius).powi(2) {
                let normal = if d.length_squared() > 0.00001 {
                    d.normalize()
                } else {
                    Vec2::X
                };
                let q = Vec2::new(x, z) + normal * (r + radius + 0.001);
                p.x = q.x;
                p.z = q.y;
            }
        }
        p = clamp_bounds(p, radius);
    }
    p
}
/// Small swept steps prevent sprinting/dashing through thin ruined walls.
pub fn move_body(from: Vec3, delta: Vec3, radius: f32) -> Vec3 {
    let steps = (Vec2::new(delta.x, delta.z).length() / 0.25).ceil().max(1.) as usize;
    let step = delta / steps as f32;
    let mut p = resolve_position(from, radius);
    for _ in 0..steps {
        p = resolve_position(p + step, radius);
    }
    p
}

/// Returns the distance to the first actual masonry/tree hit on a 3D segment.
/// The footprint is inflated for movement, but shots use the visible geometry.
pub fn obstruction(from: Vec3, to: Vec3) -> Option<f32> {
    let delta = to - from;
    let length = delta.length();
    let mut first: f32 = 2.;
    for w in WALLS {
        let low = Vec3::new(w.x - w.half_x, 0., w.z - w.half_z);
        let high = Vec3::new(w.x + w.half_x, w.height, w.z + w.half_z);
        let mut enter: f32 = 0.;
        let mut leave: f32 = 1.;
        for axis in 0..3 {
            if delta[axis].abs() < 0.00001 {
                if from[axis] < low[axis] || from[axis] > high[axis] {
                    leave = -1.;
                }
            } else {
                let a = (low[axis] - from[axis]) / delta[axis];
                let b = (high[axis] - from[axis]) / delta[axis];
                enter = enter.max(a.min(b));
                leave = leave.min(a.max(b));
            }
        }
        if leave >= enter {
            first = first.min(enter);
        }
    }
    for (index, (x, z, _)) in PILLARS.into_iter().enumerate() {
        let flat = Vec2::new(from.x - x, from.z - z);
        let d = Vec2::new(delta.x, delta.z);
        let a = d.length_squared();
        let b = 2. * flat.dot(d);
        for band in pillar_bands(index) {
            let c = flat.length_squared() - band.radius * band.radius;
            let (mut enter, mut leave) = if a < 0.00001 {
                if c > 0. {
                    continue;
                }
                (0f32, 1f32)
            } else {
                let disc = b * b - 4. * a * c;
                if disc < 0. {
                    continue;
                }
                (
                    ((-b - disc.sqrt()) / (2. * a)).max(0.),
                    ((-b + disc.sqrt()) / (2. * a)).min(1.),
                )
            };
            if delta.y.abs() < 0.00001 {
                if from.y < band.bottom || from.y > band.top {
                    continue;
                }
            } else {
                let low = (band.bottom - from.y) / delta.y;
                let high = (band.top - from.y) / delta.y;
                enter = enter.max(low.min(high));
                leave = leave.min(low.max(high));
            }
            if leave >= enter {
                first = first.min(enter);
            }
        }
    }
    (first <= 1.).then_some(first * length)
}

const CELL: f32 = 1.5;
const SIDE: usize = 65;
const NODES: usize = SIDE * SIDE;
const NAV_RADIUS: f32 = 0.72;
fn point(index: usize) -> Vec3 {
    Vec3::new(
        index.rem_euclid(SIDE) as f32 * CELL - HALF_EXTENT,
        0.,
        (index / SIDE) as f32 * CELL - HALF_EXTENT,
    )
}
fn cell(p: Vec3) -> usize {
    let x = ((p.x + HALF_EXTENT) / CELL)
        .round()
        .clamp(0., (SIDE - 1) as f32) as usize;
    let z = ((p.z + HALF_EXTENT) / CELL)
        .round()
        .clamp(0., (SIDE - 1) as f32) as usize;
    z * SIDE + x
}
fn clear_path(a: Vec3, b: Vec3, radius: f32) -> bool {
    let delta = (b - a) * Vec3::new(1., 0., 1.);
    let steps = (delta.length() / 0.35).ceil().max(1.) as usize;
    (0..=steps).all(|i| is_walkable(a + delta * (i as f32 / steps as f32), radius))
}
struct Topology {
    edges: Vec<Vec<(usize, u32)>>,
    walkable: Vec<bool>,
}
fn topology() -> &'static Topology {
    static TOPOLOGY: OnceLock<Topology> = OnceLock::new();
    TOPOLOGY.get_or_init(|| {
        let walkable: Vec<_> = (0..NODES)
            .map(|i| is_walkable(point(i), NAV_RADIUS))
            .collect();
        let mut edges = vec![vec![]; NODES];
        for i in 0..NODES {
            if !walkable[i] {
                continue;
            }
            let x = (i % SIDE) as i32;
            let z = (i / SIDE) as i32;
            for dz in -1..=1 {
                for dx in -1..=1 {
                    if dx == 0 && dz == 0 {
                        continue;
                    }
                    let nx = x + dx;
                    let nz = z + dz;
                    if nx < 0 || nz < 0 || nx >= SIDE as i32 || nz >= SIDE as i32 {
                        continue;
                    }
                    let j = nz as usize * SIDE + nx as usize;
                    if walkable[j] && clear_path(point(i), point(j), NAV_RADIUS) {
                        edges[i].push((j, if dx == 0 || dz == 0 { 10 } else { 14 }));
                    }
                }
            }
        }
        Topology { edges, walkable }
    })
}
/// One shared pursuit field per game, refreshed only when the player changes
/// grid cells. Dozens of enemies can route through the same open door cheaply.
pub struct Navigation {
    target_cell: Option<usize>,
    costs: Vec<u32>,
}
impl Default for Navigation {
    fn default() -> Self {
        Self {
            target_cell: None,
            costs: vec![u32::MAX; NODES],
        }
    }
}
impl Navigation {
    pub fn update(&mut self, target: Vec3) {
        let topo = topology();
        let mut at = cell(target);
        if !topo.walkable[at] {
            at = (0..NODES)
                .filter(|&i| topo.walkable[i])
                .min_by(|&a, &b| {
                    point(a)
                        .distance_squared(target)
                        .total_cmp(&point(b).distance_squared(target))
                })
                .unwrap();
        }
        if self.target_cell == Some(at) {
            return;
        }
        self.target_cell = Some(at);
        self.costs.fill(u32::MAX);
        self.costs[at] = 0;
        let mut frontier = BinaryHeap::from([Reverse((0, at))]);
        while let Some(Reverse((cost, i))) = frontier.pop() {
            if cost != self.costs[i] {
                continue;
            }
            for &(j, step) in &topo.edges[i] {
                let candidate = cost + step;
                if candidate < self.costs[j] {
                    self.costs[j] = candidate;
                    frontier.push(Reverse((candidate, j)));
                }
            }
        }
    }
    pub fn direction(&self, from: Vec3, target: Vec3, radius: f32) -> Vec3 {
        if clear_path(from, target, radius) {
            return ((target - from) * Vec3::new(1., 0., 1.)).normalize_or_zero();
        }
        let origin = cell(from);
        let x = (origin % SIDE) as i32;
        let z = (origin / SIDE) as i32;
        let mut best = None;
        let mut score = f32::INFINITY;
        // Look ahead over neighbouring cells, smoothing the grid path while
        // retaining wall clearance; never cut diagonally through a buttress.
        for dz in -2..=2 {
            for dx in -2..=2 {
                let nx = x + dx;
                let nz = z + dz;
                if nx < 0 || nz < 0 || nx >= SIDE as i32 || nz >= SIDE as i32 {
                    continue;
                }
                let i = nz as usize * SIDE + nx as usize;
                if self.costs[i] == u32::MAX {
                    continue;
                }
                let p = point(i);
                let distance = Vec2::new(p.x - from.x, p.z - from.z).length();
                let candidate = self.costs[i] as f32 + distance * 6.;
                if distance > 0.2 && candidate < score && clear_path(from, p, radius) {
                    score = candidate;
                    best = Some(p);
                }
            }
        }
        best.map(|p| ((p - from) * Vec3::new(1., 0., 1.)).normalize_or_zero())
            .unwrap_or(Vec3::ZERO)
    }
}

/// Reinforcements arrive near the current fight, even in the farthest court.
/// Try a complete ring before reducing range; do not clamp points into walls.
pub fn spawn_point(player: Vec3, angle: f32, distance: f32, radius: f32) -> Vec3 {
    for ring in [distance, distance * 0.85, 10.] {
        for n in 0..32 {
            let a = angle + n as f32 * 2.399_963_1;
            let p = Vec3::new(player.x + a.sin() * ring, 0., player.z + a.cos() * ring);
            if is_spawnable(p, radius) {
                return p;
            }
        }
    }
    resolve_position(player - Vec3::new(0., player.y, 10.), radius)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_landmarks_and_outer_lanes_are_connected() {
        let mut nav = Navigation::default();
        nav.update(Vec3::ZERO);
        for destination in [
            Vec3::new(-29., 0., -8.),
            Vec3::new(29., 0., -7.),
            Vec3::new(0., 0., -30.),
            Vec3::new(0., 0., 28.),
            Vec3::new(-43., 0., -43.),
            Vec3::new(43., 0., 43.),
        ] {
            assert!(is_walkable(destination, PLAYER_RADIUS));
            assert!(
                nav.costs[cell(destination)] < u32::MAX,
                "Disconnected court {destination:?}"
            );
        }
        let topo = topology();
        assert!(
            (0..NODES)
                .filter(|&i| topo.walkable[i])
                .all(|i| nav.costs[i] < u32::MAX),
            "An isolated walkable pocket would trap an enemy"
        );
    }
    #[test]
    fn dashing_cannot_cross_walls_or_world_boundary() {
        let p = move_body(
            Vec3::new(-16., 1.65, -14.),
            Vec3::new(-15., 0., 0.),
            PLAYER_RADIUS,
        );
        assert!(p.x > -20. && is_walkable(p, PLAYER_RADIUS));
        let p = move_body(
            Vec3::new(45., 1.65, 30.),
            Vec3::new(50., 0., 50.),
            PLAYER_RADIUS,
        );
        assert!(inside_bounds(p, PLAYER_RADIUS));
        assert_eq!(p.y, 1.65);
        for w in WALLS {
            assert!(is_walkable(
                resolve_position(Vec3::new(w.x, 0., w.z), PLAYER_RADIUS),
                PLAYER_RADIUS
            ));
        }
    }
    #[test]
    fn reinforcement_ring_remains_near_player_and_outside_solids() {
        for player in [
            Vec3::ZERO,
            Vec3::new(-29., 0., -8.),
            Vec3::new(29., 0., -7.),
            Vec3::new(0., 0., -30.),
            Vec3::new(-46., 0., 46.),
            Vec3::new(46., 0., -46.),
        ] {
            for i in 0..100 {
                let p = spawn_point(player, i as f32 * 0.17, 15. + (i % 5) as f32, ENEMY_RADIUS);
                assert!(is_spawnable(p, ENEMY_RADIUS));
                assert!(
                    (9.9..=19.1).contains(&p.distance(player)),
                    "Spawn at {p:?} from {player:?}"
                );
            }
        }
    }
    #[test]
    fn navigation_routes_through_chapel_doors_without_sticking() {
        let mut nav = Navigation::default();
        let target = Vec3::new(-29., 0., -8.);
        nav.update(target);
        for start in [
            Vec3::ZERO,
            Vec3::new(-43., 0., -16.),
            Vec3::new(-29., 0., -27.),
        ] {
            let mut p = start;
            for _ in 0..1800 {
                p = move_body(
                    p,
                    nav.direction(p, target, ENEMY_RADIUS) * 0.06,
                    ENEMY_RADIUS,
                );
                if p.distance(target) < 1. {
                    break;
                }
            }
            assert!(
                p.distance(target) < 1.,
                "Enemy stuck at {p:?} from {start:?}"
            );
        }
    }
    #[test]
    fn rays_pointing_away_from_monuments_are_not_false_occluded() {
        assert!(obstruction(Vec3::new(0., 1., 0.), Vec3::new(0., 1., 12.)).is_none());
        assert!(obstruction(Vec3::new(0., 3., -4.), Vec3::new(0., 3., -12.)).is_none());
        assert!(obstruction(Vec3::new(0., 1., -4.), Vec3::new(0., 1., -12.)).is_some());
    }
    #[test]
    fn fountain_is_low_cover_with_a_narrow_solid_urn() {
        let side = Vec3::new(1.3, 1.4, -4.);
        assert!(obstruction(side, side - Vec3::Z * 8.).is_none());
        assert!(obstruction(Vec3::new(0., 1.4, -4.), Vec3::new(0., 1.4, -12.)).is_some());
        assert!(obstruction(Vec3::new(1.3, 0.6, -4.), Vec3::new(1.3, 0.6, -12.)).is_some());
        // A lob may enter from above and strike the rim after entering its XZ circle.
        assert!(obstruction(Vec3::new(1.3, 3., -6.), Vec3::new(1.3, 0.4, -9.)).is_some());
        assert!(obstruction(Vec3::new(-5., 5., 31.), Vec3::new(-5., 5., 37.)).is_none());
    }
    #[test]
    fn shots_respect_wall_height_and_open_doors() {
        assert!(obstruction(Vec3::new(-16., 1.5, -14.), Vec3::new(-29., 1.5, -14.)).is_some());
        assert!(obstruction(Vec3::new(-16., 1.5, -5.), Vec3::new(-29., 1.5, -5.)).is_none());
        assert!(obstruction(Vec3::new(-16., 8., -14.), Vec3::new(-29., 8., -14.)).is_none());
    }
}
