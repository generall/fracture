//! Sight is independent of collision: low/open props transmit it, foliage doesn't.
use super::*;
mod shading;

pub const VISION_RANGE: f32 = 24.0;
pub const NEAR_VISION: f32 = 3.0;
pub const VISION_HALF_ANGLE: f32 = std::f32::consts::FRAC_PI_4;

#[derive(Clone, Default)]
pub struct Vision {
    pub origin: Vector,
    /// Ordered boundary of the visible region, suitable for a triangle fan.
    pub boundary: Vec<Vector>,
    // Preserve sorted ray angles instead of recovering them from rounded world
    // positions on every query. Translation can collapse neighboring rays.
    angles: Vec<f32>,
    additional: Vec<Vision>,
}
impl Vision {
    pub fn from_boundary(origin: Vector, mut boundary: Vec<Vector>) -> Self {
        let angle = |p: &Vector| (*p - origin).to_angle().rem_euclid(std::f32::consts::TAU);
        boundary.sort_by(|a, b| angle(a).total_cmp(&angle(b)));
        let angles = boundary.iter().map(angle).collect();
        Self {
            origin,
            boundary,
            angles,
            additional: vec![],
        }
    }
    pub fn contains(&self, point: Vector) -> bool {
        self.regions().any(|region| region.contains_local(point))
    }
    pub fn regions(&self) -> impl Iterator<Item = &Vision> {
        std::iter::once(self).chain(self.additional.iter())
    }
    fn contains_local(&self, point: Vector) -> bool {
        let delta = point - self.origin;
        if delta.length_squared() < 1e-8 {
            return !self.boundary.is_empty();
        }
        if self.boundary.is_empty() {
            return false;
        }
        let angle = delta.to_angle().rem_euclid(std::f32::consts::TAU);
        let next = self.angles.partition_point(|a| *a <= angle);
        let i = (next + self.boundary.len() - 1) % self.boundary.len();
        let a = self.boundary[i] - self.origin;
        let b = self.boundary[(i + 1) % self.boundary.len()] - self.origin;
        // Silhouette rays can become coincident after conversion to world
        // coordinates. An absolute cross-product epsilon turns these tiny
        // triangles into unbounded strips and can reveal distant rear objects.
        const EPS: f64 = 1e-6; // distance in metres, independent of edge length
        let cross = |a: Vector, b: Vector| a.x as f64 * b.y as f64 - a.y as f64 * b.x as f64;
        let edge = b - a;
        delta.length() as f64 <= a.length().max(b.length()) as f64 + EPS
            && delta.dot(a + b) >= 0.0
            && cross(a, b) > 0.0
            && cross(a, delta) >= -EPS * a.length() as f64
            && cross(delta, b) >= -EPS * b.length() as f64
            && cross(edge, delta - a) >= -EPS * edge.length() as f64
    }

    /// Whole-figure visibility: any overlap with the original sight region is
    /// enough. Revealing a figure never extends sight to figures behind it.
    pub fn intersects_polygon(&self, polygon: &[Vector]) -> bool {
        self.regions()
            .any(|region| region.intersects_local(polygon))
    }
    fn intersects_local(&self, polygon: &[Vector]) -> bool {
        if self.boundary.is_empty() || polygon.len() < 3 {
            return false;
        }
        if polygon.iter().any(|&p| self.contains_local(p)) {
            return true;
        }
        let lo = polygon
            .iter()
            .fold(Vector::splat(f32::INFINITY), |a, &p| a.min(p));
        let hi = polygon
            .iter()
            .fold(Vector::splat(f32::NEG_INFINITY), |a, &p| a.max(p));
        let inside = |point: Vector| {
            // Tiny fracture cells must not classify a distant observer as
            // inside merely because all their cross products are near zero.
            if point.cmplt(lo).any() || point.cmpgt(hi).any() {
                return false;
            }
            let mut positive = false;
            let mut negative = false;
            for i in 0..polygon.len() {
                let cross =
                    (polygon[(i + 1) % polygon.len()] - polygon[i]).perp_dot(point - polygon[i]);
                positive |= cross > 1e-5;
                negative |= cross < -1e-5;
            }
            !(positive && negative)
        };
        if inside(self.origin) {
            return true;
        }
        let center = (lo + hi) * 0.5 - self.origin;
        let radius = (hi - lo).length() * 0.5;
        let tau = std::f32::consts::TAU;
        let half = if center.length() <= radius {
            std::f32::consts::PI
        } else {
            (radius / center.length()).asin() + 0.0001
        };
        let middle = center.to_angle().rem_euclid(tau);
        let search = |a: f32, b: f32| {
            let first = self.angles.partition_point(|angle| *angle < a);
            let last = self.angles.partition_point(|angle| *angle <= b);
            self.boundary[first..last]
                .iter()
                .any(|&p| p.cmpge(lo).all() && p.cmple(hi).all() && inside(p))
        };
        if half >= std::f32::consts::PI {
            return search(0.0, tau);
        }
        search((middle - half).max(0.0), (middle + half).min(tau))
            || (middle - half < 0.0 && search(middle - half + tau, tau))
            || (middle + half > tau && search(0.0, middle + half - tau))
    }
}
struct Occluder {
    shape: SharedShape,
    pose: Pose,
    bounds: Aabb,
    grass: bool,
    tank: bool,
}
// Adjacent collision triangles share internal edges. Only their exposed
// outline corners affect visibility; mortar subdivisions need no extra rays.
fn silhouette_vertices(obstacles: &[Occluder]) -> Vec<Vector> {
    use std::collections::{BTreeMap, BTreeSet};
    type Key = (i32, i32);
    let key = |p: Vector| {
        (
            (p.x * 10000.0).round() as i32,
            (p.y * 10000.0).round() as i32,
        )
    };
    let mut points: BTreeMap<Key, Vector> = BTreeMap::new();
    let mut edges = BTreeSet::new();
    for o in obstacles {
        let polygon = if let Some(t) = o.shape.as_triangle() {
            [t.a, t.b, t.c].map(|p| o.pose.transform_point(p)).to_vec()
        } else {
            vec![
                o.bounds.mins,
                Vector::new(o.bounds.maxs.x, o.bounds.mins.y),
                o.bounds.maxs,
                Vector::new(o.bounds.mins.x, o.bounds.maxs.y),
            ]
        };
        for i in 0..polygon.len() {
            let a = key(polygon[i]);
            let b = key(polygon[(i + 1) % polygon.len()]);
            points.insert(a, polygon[i]);
            if a == b {
                continue;
            }
            let edge = (a.min(b), a.max(b));
            if !edges.insert(edge) {
                edges.remove(&edge);
            }
        }
    }
    let mut neighbors: BTreeMap<Key, Vec<Key>> = BTreeMap::new();
    for (a, b) in edges {
        neighbors.entry(a).or_default().push(b);
        neighbors.entry(b).or_default().push(a);
    }
    neighbors
        .into_iter()
        .filter_map(|(id, adjacent)| {
            let p = points[&id];
            if adjacent.len() == 2 {
                let a = points[&adjacent[0]] - p;
                let b = points[&adjacent[1]] - p;
                if a.dot(b) < 0.0 && a.perp_dot(b).abs() <= 0.0001 * (a.length() + b.length()) {
                    return None;
                }
            }
            Some(p)
        })
        .collect()
}

impl World {
    fn sight_occluders(&self, origin: Vector, ignored: Option<usize>, range: f32) -> Vec<Occluder> {
        let nearby =
            |bounds: Aabb| origin.clamp(bounds.mins, bounds.maxs).distance(origin) <= range;
        let mut out = vec![];
        for (_, collider) in self.physics.colliders.iter() {
            let Some(id) = self.weapon_target(collider) else {
                continue;
            };
            if Some(id) == ignored {
                continue;
            }
            let piece = &self.pieces[id];
            let blocks = piece
                .object
                .and_then(|i| self.objects[i].components.blocks_vision)
                .unwrap_or(true);
            let bounds = collider.compute_aabb();
            if blocks && nearby(bounds) {
                out.push(Occluder {
                    shape: collider.shared_shape().clone(),
                    pose: *collider.position(),
                    bounds,
                    grass: false,
                    tank: piece.material == Material::Launcher,
                });
            }
        }
        // Use painted terrain precedence, so painting ground over grass clears it.
        for (i, patch) in self
            .terrain
            .iter()
            .enumerate()
            .filter(|(_, p)| p.kind == crate::TerrainKind::TallGrass)
        {
            let mut patches = vec![*patch];
            for other in self.terrain[i + 1..].iter().chain(
                self.terrain
                    .iter()
                    .filter(|p| p.kind == crate::TerrainKind::Water),
            ) {
                patches = patches
                    .into_iter()
                    .flat_map(|p| p.subtract(*other))
                    .collect();
            }
            for p in patches {
                let bounds = Aabb::new(p.min, p.max);
                if nearby(bounds) {
                    let half = (p.max - p.min) * 0.5;
                    out.push(Occluder {
                        shape: SharedShape::cuboid(half.x, half.y),
                        pose: Pose::new((p.min + p.max) * 0.5, 0.0),
                        bounds,
                        grass: true,
                        tank: false,
                    });
                }
            }
        }
        out
    }

    /// A visibility query for AI and notifications, separate from weapon clearance.
    pub fn sight_clear(&self, tank: usize, target: Vector) -> bool {
        let origin = self.piece_pose(tank).translation;
        let distance = origin.distance(target);
        if distance < 0.01 {
            return true;
        }
        let ray = Ray::new(origin, (target - origin) / distance);
        self.sight_occluders(origin, Some(tank), distance)
            .iter()
            .all(|o| {
                if o.tank && o.shape.contains_point(&o.pose, target) {
                    return true;
                }
                let hit = o.shape.cast_ray(&o.pose, &ray, distance, true);
                hit.is_none_or(|t| t >= distance - 0.05 || (o.grass && t == 0.0 && distance <= 1.2))
            })
    }

    pub fn vision(&self, tank: usize, angle: f32) -> Vision {
        let origin = self.piece_pose(tank).translation;
        if !self.tank_is_alive(tank) {
            return Vision {
                origin,
                ..Vision::default()
            };
        }
        self.vision_from(origin, Some(tank), Some(angle))
    }

    /// Combined player sight, including the allied flag's short-range lookout.
    pub fn player_vision(&self, angle: f32) -> Vision {
        let mut vision = self.vision(self.player_tank_id(), angle);
        for (team, position) in self.flags() {
            if team == self.team() {
                vision.additional.push(self.vision_from(
                    position,
                    Some(self.player_tank_id()),
                    None,
                ));
            }
        }
        vision
    }

    fn vision_from(&self, origin: Vector, ignored: Option<usize>, cone: Option<f32>) -> Vision {
        let range = if cone.is_some() {
            VISION_RANGE
        } else {
            NEAR_VISION
        };
        let mut obstacles = self.sight_occluders(origin, ignored, range);
        // Resolve near cover first, then skip every farther shape already
        // behind that cover rather than casting through the whole map.
        let nearest = |o: &Occluder| origin.clamp(o.bounds.mins, o.bounds.maxs).distance(origin);
        obstacles.sort_by(|a, b| nearest(a).total_cmp(&nearest(b)));
        let tau = std::f32::consts::TAU;
        let mut angles: Vec<f32> = (0..360).map(|i| i as f32 * tau / 360.0).collect();
        // Exact silhouette rays prevent leaks across corners and narrow walls.
        let mut add_angle = |a: f32| {
            for offset in [-0.00002, 0.0, 0.00002] {
                angles.push((a + offset).rem_euclid(tau));
            }
        };
        if let Some(angle) = cone {
            add_angle(angle - VISION_HALF_ANGLE);
            add_angle(angle + VISION_HALF_ANGLE);
        }
        let forward = Vector::from_angle(cone.unwrap_or(0.0));
        for p in silhouette_vertices(&obstacles) {
            let delta = p - origin;
            // Distant corners outside the cone cannot affect the nearby circle.
            // Keep a small angular margin for the cone's boundary rays.
            if delta.length() <= NEAR_VISION + 0.025
                || (cone.is_some()
                    && delta.normalize_or_zero().dot(forward) >= (VISION_HALF_ANGLE + 0.0001).cos())
            {
                add_angle(delta.to_angle());
            }
        }
        angles.sort_by(f32::total_cmp);
        angles.dedup_by(|a, b| (*a - *b).abs() < 1e-7);
        let directions: Vec<_> = angles.iter().map(|&a| Vector::from_angle(a)).collect();
        let mut lengths: Vec<f32> = directions
            .iter()
            .map(|d| {
                if cone.is_some() && d.dot(forward) >= VISION_HALF_ANGLE.cos() {
                    VISION_RANGE
                } else {
                    NEAR_VISION
                }
            })
            .collect();
        for o in obstacles {
            let minimum_distance = nearest(&o);
            let center = (o.bounds.mins + o.bounds.maxs) * 0.5 - origin;
            let radius = (o.bounds.maxs - o.bounds.mins).length() * 0.5;
            let half_angle = if center.length() <= radius {
                std::f32::consts::PI
            } else {
                (radius / center.length()).asin() + 0.0001
            };
            let middle = center.to_angle().rem_euclid(tau);
            let mut cast = |lo: f32, hi: f32| {
                let first = angles.partition_point(|a| *a < lo);
                let last = angles.partition_point(|a| *a <= hi);
                for i in first..last {
                    if lengths[i] < minimum_distance {
                        continue;
                    }
                    if let Some(t) = o.shape.cast_ray(
                        &o.pose,
                        &Ray::new(origin, directions[i]),
                        lengths[i],
                        true,
                    ) {
                        // Reveal the front surface itself, but no material behind it.
                        let t = if o.grass && t == 0.0 { 1.2 } else { t + 0.025 };
                        lengths[i] = lengths[i].min(t);
                    }
                }
            };
            if half_angle >= std::f32::consts::PI {
                cast(0.0, tau);
            } else {
                let lo = middle - half_angle;
                let hi = middle + half_angle;
                cast(lo.max(0.0), hi.min(tau));
                if lo < 0.0 {
                    cast(lo + tau, tau);
                }
                if hi > tau {
                    cast(0.0, hi - tau);
                }
            }
        }
        Vision {
            origin,
            angles,
            additional: vec![],
            boundary: directions
                .into_iter()
                .zip(lengths)
                .map(|(d, t)| origin + d * t)
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::{Level, LevelItem, Team, rectangle};
    fn map(item: Option<LevelItem>) -> Level {
        let mut items = vec![LevelItem::Spawn {
            team: Team::Blue,
            position: [0.0, 0.0],
            angle: 0.0,
        }];
        items.extend(item);
        Level {
            items,
            ..Level::default()
        }
    }
    #[test]
    fn allied_flag_reveals_nearby_objects_through_respawn_but_respects_cover() {
        let mut level = map(None);
        level.items.extend([
            LevelItem::Spawn {
                team: Team::Red,
                position: [0.0, 5.0],
                angle: 0.0,
            },
            LevelItem::Flag {
                team: Team::Blue,
                position: [-12.0, 0.0],
            },
            LevelItem::Flag {
                team: Team::Red,
                position: [12.0, 0.0],
            },
            LevelItem::Solid {
                outline: rectangle(Vector::new(0.1, 2.0)),
                position: [-10.8, 0.0],
                angle: 0.0,
                cutouts: vec![],
            },
        ]);
        for team in [Team::Blue, Team::Red] {
            let mut world = World::from_level(&level, team).unwrap();
            let x = if team == Team::Blue { -12.0 } else { 12.0 };
            let vision = world.player_vision(std::f32::consts::FRAC_PI_2);
            assert!(vision.contains(Vector::new(x, 2.0)));
            assert!(!vision.contains(Vector::new(x, 3.5)));
            assert!(
                !vision.contains(Vector::new(-x, 2.0)),
                "enemy flag grants no sight"
            );
            let target = [
                Vector::new(x - 0.1, 1.9),
                Vector::new(x + 0.1, 1.9),
                Vector::new(x, 2.1),
            ];
            assert!(vision.intersects_polygon(&target));
            world.damage_tank(world.player_tank_id(), 1000.0, &mut vec![]);
            let vision = world.player_vision(0.0);
            assert!(vision.contains(Vector::new(x, 2.0)));
            assert!(vision.intersects_polygon(&target));
            if team == Team::Blue {
                assert!(
                    !vision.contains(Vector::new(-10.0, 0.0)),
                    "wall blocks flag sight"
                );
            }
        }
    }

    #[test]
    fn moving_observer_does_not_reveal_objects_behind_the_near_circle() {
        let obstacle = LevelItem::Solid {
            outline: rectangle(Vector::new(0.5, 2.0)),
            position: [-8.0, -2.0],
            angle: 0.0,
            cutouts: vec![],
        };
        let polygon = obstacle.world_outline();
        let mut world = World::from_level(&map(Some(obstacle)), Team::Blue).unwrap();
        for step in 0..2000 {
            let position = Vector::new(2.0 + step as f32 * 0.0031, (step as f32 * 0.017).sin());
            let body = world.pieces[world.launcher].body;
            world.physics.bodies[body].set_translation(position, true);
            let vision = world.vision(world.launcher, 0.0);
            assert!(
                !vision.intersects_polygon(&polygon),
                "rear object revealed at {position:?}"
            );
        }
    }

    #[test]
    fn cone_turns_with_the_turret_and_near_circle_is_always_available() {
        let world = World::from_level(&map(None), Team::Blue).unwrap();
        let vision = world.vision(world.launcher, 0.0);
        for p in [
            Vector::new(12.0, 0.0),
            Vector::new(-2.0, 0.0),
            Vector::new(0.0, 2.0),
        ] {
            assert!(vision.contains(p), "{p:?}");
        }
        for p in [
            Vector::new(-4.0, 0.0),
            Vector::new(0.0, 8.0),
            Vector::new(25.0, 0.0),
        ] {
            assert!(!vision.contains(p), "{p:?}");
        }
        let turned = world.vision(world.launcher, std::f32::consts::FRAC_PI_2);
        assert!(turned.contains(Vector::new(0.0, 12.0)));
        assert!(!turned.contains(Vector::new(12.0, 0.0)));
    }
    #[test]
    fn solid_cover_blocks_sight_but_barrels_and_fences_transmit_it() {
        for preset in crate::LibraryPreset::ALL {
            let item = LevelItem::Object {
                asset: Box::new(preset.asset()),
                position: [5.0, 0.0],
                angle: 0.0,
                cutouts: vec![],
            };
            let world = World::from_level(&map(Some(item)), Team::Blue).unwrap();
            let expected = matches!(
                preset,
                crate::LibraryPreset::Barrel
                    | crate::LibraryPreset::ExplosiveBarrel
                    | crate::LibraryPreset::MetalFence
                    | crate::LibraryPreset::Fence
            );
            assert_eq!(
                world
                    .vision(world.launcher, 0.0)
                    .contains(Vector::new(9.0, 0.0)),
                expected,
                "{preset:?}"
            );
            assert_eq!(
                world.sight_clear(world.launcher, Vector::new(9.0, 0.0)),
                expected,
                "{preset:?}"
            );
        }
        let mut level = map(Some(LevelItem::Solid {
            outline: rectangle(Vector::new(0.2, 4.0)),
            position: [2.0, 0.0],
            angle: 0.0,
            cutouts: vec![],
        }));
        let world = World::from_level(&level, Team::Blue).unwrap();
        assert!(
            !world
                .vision(world.launcher, 0.0)
                .contains(Vector::new(2.5, 0.0)),
            "near circle respects cover"
        );
        level
            .erase_rectangle(Vector::new(1.0, -1.0), Vector::new(3.0, 1.0))
            .unwrap();
        let world = World::from_level(&level, Team::Blue).unwrap();
        assert!(
            world
                .vision(world.launcher, 0.0)
                .contains(Vector::new(9.0, 0.0)),
            "erased openings transmit sight"
        );
    }
    #[test]
    fn grass_blocks_sight_without_blocking_shells_or_tanks() {
        let grass = LevelItem::Terrain {
            terrain: crate::TerrainKind::TallGrass,
            min: [4.0, -3.0],
            max: [7.0, 3.0],
        };
        let mut level = map(Some(grass));
        let mut world = World::from_level(&level, Team::Blue).unwrap();
        assert!(
            !world
                .vision(world.launcher, 0.0)
                .contains(Vector::new(9.0, 0.0))
        );
        assert!(!world.sight_clear(world.launcher, Vector::new(9.0, 0.0)));
        assert!(world.tank_pose_clear(world.launcher, Pose::new(Vector::new(5.0, 0.0), 0.0)));
        world.step(&[Command::Fire(Shot::new(
            Ammunition::ArmorPiercing,
            Vector::new(10.0, 0.0),
        ))]);
        for _ in 0..5 {
            world.step(&[]);
        }
        assert!(world.shells().any(|s| s.position.x > 7.0));
        level.items.push(LevelItem::Terrain {
            terrain: crate::TerrainKind::Ground,
            min: [3.0, -4.0],
            max: [8.0, 4.0],
        });
        let world = World::from_level(&level, Team::Blue).unwrap();
        assert!(
            world
                .vision(world.launcher, 0.0)
                .contains(Vector::new(9.0, 0.0))
        );
    }
}
