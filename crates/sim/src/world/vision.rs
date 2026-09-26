//! Sight is independent of collision: low/open props transmit it, foliage doesn't.
use super::*;
mod shading;

pub const VISION_RANGE: f32 = 24.0;
pub const GRASS_VISION_RANGE: f32 = 8.0;
pub const ELEVATED_VISION_RANGE: f32 = 36.0;
const GRASS_REVEAL_SECONDS: f32 = 5.0;
pub const NEAR_VISION: f32 = 3.0;
pub const VISION_HALF_ANGLE: f32 = std::f32::consts::FRAC_PI_4;
// World-space footprint, measured per connected chunk, never per mesh triangle.
const MIN_SIGHT_BLOCKER_AREA: f32 = 0.5;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SightGroup {
    Chunk(u32, u32),
    FixedObject(usize),
}

impl Piece {
    fn sight_group(&self) -> SightGroup {
        // Authored indestructible objects use a separate fixed body per triangle.
        // Their triangles still belong to one prop for size classification.
        if self.material == Material::Anchor
            && let Some(object) = self.object
        {
            SightGroup::FixedObject(object)
        } else {
            let (index, generation) = self.body.into_raw_parts();
            SightGroup::Chunk(index, generation)
        }
    }
}

#[derive(Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Vision {
    pub origin: Vector,
    /// Ordered boundary of the visible region, suitable for a triangle fan.
    pub boundary: Vec<Vector>,
    // Preserve sorted ray angles instead of recovering them from rounded world
    // positions on every query. Translation can collapse neighboring rays.
    angles: Vec<f32>,
    additional: Vec<Vision>,
    // Target visibility overrides foliage geometry without revealing nearby scenery.
    tank_visibility: Vec<(usize, bool)>,
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
            tank_visibility: vec![],
        }
    }
    pub fn tank_visibility(&self, id: usize) -> Option<bool> {
        self.tank_visibility
            .iter()
            .find(|(tank, _)| *tank == id)
            .map(|(_, visible)| *visible)
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
#[derive(Clone)]
pub(super) struct Occluder {
    source: Option<usize>,
    shape: SharedShape,
    pose: Pose,
    bounds: Aabb,
    grass: bool,
    tank: bool,
}
// Adjacent collision triangles share internal edges. Only their exposed
// outline corners affect visibility; mortar subdivisions need no extra rays.
// Also reports which obstacles own an exposed edge: a ray from outside the
// union always meets such a triangle first, so fully interior ones never
// shorten a ray.
/// World-space vertices of polygonal occluders (triangles, and the convex
/// cells that chunk rebuilding creates); None for other shapes.
fn outline(o: &Occluder) -> Option<Vec<Vector>> {
    if let Some(t) = o.shape.as_triangle() {
        Some([t.a, t.b, t.c].map(|p| o.pose.transform_point(p)).to_vec())
    } else {
        o.shape.as_convex_polygon().map(|c| {
            c.points()
                .iter()
                .map(|&p| o.pose.transform_point(p))
                .collect()
        })
    }
}
fn silhouette(obstacles: &[Occluder]) -> (Vec<Vector>, Vec<bool>) {
    type Key = (i32, i32);
    let key = |p: Vector| {
        (
            (p.x * 10000.0).round() as i32,
            (p.y * 10000.0).round() as i32,
        )
    };
    let mut points: Vec<(Key, Vector)> = Vec::with_capacity(obstacles.len() * 3);
    let mut edges: Vec<((Key, Key), usize)> = Vec::with_capacity(obstacles.len() * 3);
    for (index, o) in obstacles.iter().enumerate() {
        let polygon = outline(o).unwrap_or_else(|| {
            vec![
                o.bounds.mins,
                Vector::new(o.bounds.maxs.x, o.bounds.mins.y),
                o.bounds.maxs,
                Vector::new(o.bounds.mins.x, o.bounds.maxs.y),
            ]
        });
        for i in 0..polygon.len() {
            let a = key(polygon[i]);
            let b = key(polygon[(i + 1) % polygon.len()]);
            points.push((a, polygon[i]));
            if a != b {
                edges.push(((a.min(b), a.max(b)), index));
            }
        }
    }
    // Stable sorts keep the last-inserted point per key, like a map insert.
    points.sort_by_key(|p| p.0);
    let point = |k: Key| {
        let end = points.partition_point(|p| p.0 <= k);
        points[end - 1].1
    };
    edges.sort_unstable();
    let mut exposed = vec![false; obstacles.len()];
    let mut outline: Vec<(Key, Key)> = vec![];
    for run in edges.chunk_by(|a, b| a.0 == b.0) {
        // Edges shared an even number of times cancel.
        if run.len() % 2 == 1 {
            outline.push((run[0].0.0, run[0].0.1));
            for &(_, index) in run {
                exposed[index] = true;
            }
        }
    }
    let mut ends: Vec<(Key, Key)> = outline
        .iter()
        .flat_map(|&(a, b)| [(a, b), (b, a)])
        .collect();
    ends.sort_by_key(|e| e.0);
    let vertices = ends
        .chunk_by(|a, b| a.0 == b.0)
        .filter_map(|adjacent| {
            let p = point(adjacent[0].0);
            if adjacent.len() == 2 {
                let a = point(adjacent[0].1) - p;
                let b = point(adjacent[1].1) - p;
                if a.dot(b) < 0.0 && a.perp_dot(b).abs() <= 0.0001 * (a.length() + b.length()) {
                    return None;
                }
            }
            Some(p)
        })
        .collect();
    (vertices, exposed)
}

impl World {
    pub fn player_sight_range(&self) -> f32 {
        if let Some(frame) = &self.replica {
            return frame.sight_range;
        }
        self.vision_range_at(self.launcher_position())
    }

    pub(super) fn vision_range_at(&self, position: Vector) -> f32 {
        match self.terrain_at(position) {
            crate::TerrainKind::TallGrass => GRASS_VISION_RANGE,
            crate::TerrainKind::Elevation => ELEVATED_VISION_RANGE,
            _ => VISION_RANGE,
        }
    }
    pub(super) fn prepare_snapshot_vision(&mut self) {
        self.sight_cache = None;
        self.sight_cache = Some(self.sight_occluders(Vector::ZERO, None, f32::MAX));
    }
    fn sight_occluders(&self, origin: Vector, ignored: Option<usize>, range: f32) -> Vec<Occluder> {
        if let Some(cache) = &self.sight_cache {
            return cache
                .iter()
                .filter(|o| {
                    (ignored.is_none() || o.source != ignored)
                        && origin.clamp(o.bounds.mins, o.bounds.maxs).distance(origin) <= range
                })
                .cloned()
                .collect();
        }
        let nearby =
            |bounds: Aabb| origin.clamp(bounds.mins, bounds.maxs).distance(origin) <= range;
        let mut out = vec![];
        if let Some(frame) = &self.replica {
            // Replicas rebuild the host's classified blockers from body geometry.
            for body in frame
                .bodies
                .iter()
                .filter(|b| b.blocks_sight && Some(b.id) != ignored)
            {
                let pose = Pose::new(body.position, body.angle);
                let shapes = match body.shape {
                    _ if body.material == Material::Launcher => super::vehicle::tank_colliders()
                        .map(|(offset, half)| {
                            (
                                SharedShape::cuboid(half.x, half.y),
                                pose * Pose::from_translation(offset),
                            )
                        })
                        .to_vec(),
                    Shape::Triangle {
                        vertices: [a, b, c],
                    } => vec![(SharedShape::triangle(a, b, c), pose)],
                    Shape::Box { half } => vec![(SharedShape::cuboid(half.x, half.y), pose)],
                    Shape::Ball { radius } => vec![(SharedShape::ball(radius), pose)],
                };
                for (shape, pose) in shapes {
                    let bounds = shape.compute_aabb(&pose);
                    if nearby(bounds) {
                        out.push(Occluder {
                            source: Some(body.id),
                            shape,
                            pose,
                            bounds,
                            grass: false,
                            tank: body.material == Material::Launcher,
                        });
                    }
                }
            }
        } else {
            self.collider_occluders(ignored, &nearby, &mut out);
        }
        self.grass_occluders(&nearby, &mut out);
        out
    }

    fn collider_occluders(
        &self,
        ignored: Option<usize>,
        nearby: &dyn Fn(Aabb) -> bool,
        out: &mut Vec<Occluder>,
    ) {
        let mut areas = std::collections::BTreeMap::<SightGroup, f32>::new();
        for piece in self.pieces.iter().filter(|p| p.active && !p.bridge_deck) {
            *areas.entry(piece.sight_group()).or_default() += piece.shape.area();
        }
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
            // Reject small props/debris before bounds, silhouette extraction or
            // ray tests. Splitting a chunk immediately changes its eligibility.
            if !blocks || areas[&piece.sight_group()] < MIN_SIGHT_BLOCKER_AREA {
                continue;
            }
            let bounds = collider.compute_aabb();
            if nearby(bounds) {
                out.push(Occluder {
                    source: Some(id),
                    shape: collider.shared_shape().clone(),
                    pose: *collider.position(),
                    bounds,
                    grass: false,
                    tank: piece.material == Material::Launcher,
                });
            }
        }
    }

    fn grass_occluders(&self, nearby: &dyn Fn(Aabb) -> bool, out: &mut Vec<Occluder>) {
        // Use painted terrain precedence, so painting ground over grass clears it.
        for (i, patch) in self
            .terrain
            .iter()
            .enumerate()
            .filter(|(_, p)| p.kind == crate::TerrainKind::TallGrass)
        {
            let mut patches = vec![*patch];
            for other in self.terrain[i + 1..]
                .iter()
                .chain(self.terrain.iter().filter(|p| p.kind.blocks_tanks()))
            {
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
                        source: None,
                        shape: SharedShape::cuboid(half.x, half.y),
                        pose: Pose::new((p.min + p.max) * 0.5, 0.0),
                        bounds,
                        grass: true,
                        tank: false,
                    });
                }
            }
        }
    }

    /// Pieces currently classified as sight blockers, for network snapshots.
    pub(super) fn sight_blockers(&self) -> std::collections::BTreeSet<usize> {
        self.sight_occluders(Vector::ZERO, None, f32::MAX)
            .into_iter()
            .filter_map(|o| o.source)
            .collect()
    }

    /// A visibility query for AI and notifications, separate from weapon clearance.
    pub fn sight_clear(&self, tank: usize, target: Vector) -> bool {
        let origin = self.piece_pose(tank).translation;
        let in_grass = self.terrain_at(origin) == crate::TerrainKind::TallGrass;
        (!in_grass || origin.distance(target) <= GRASS_VISION_RANGE)
            && self.sight_path_clear(origin, Some(tank), target, in_grass)
    }

    fn sight_path_clear(
        &self,
        origin: Vector,
        ignored: Option<usize>,
        target: Vector,
        ignore_grass: bool,
    ) -> bool {
        let distance = origin.distance(target);
        if distance < 0.01 {
            return true;
        }
        let ray = Ray::new(origin, (target - origin) / distance);
        self.sight_occluders(origin, ignored, distance)
            .iter()
            .all(|o| {
                if (ignore_grass && o.grass) || (o.tank && o.shape.contains_point(&o.pose, target))
                {
                    return true;
                }
                o.shape
                    .cast_ray(&o.pose, &ray, distance, true)
                    .is_none_or(|t| t >= distance - 0.05)
            })
    }

    pub(super) fn reveal_firing_tank(&mut self, id: usize) {
        if let Some(tank) = self.tanks.iter_mut().find(|t| t.id == id) {
            tank.reveal_until = self.tick + (GRASS_REVEAL_SECONDS / TICK_DT).round() as u64;
        }
    }
    fn firing_reveals(&self, id: usize) -> bool {
        self.tanks
            .iter()
            .any(|t| t.id == id && self.tick < t.reveal_until)
    }
    fn concealed_from(&self, observer: usize, target: usize) -> bool {
        let origin = self.piece_pose(observer).translation;
        let position = self.piece_pose(target).translation;
        self.terrain_at(position) == crate::TerrainKind::TallGrass
            && !self.firing_reveals(target)
            && (!self.tank_is_alive(observer)
                || self.terrain_at(origin) != crate::TerrainKind::TallGrass
                || origin.distance(position) > GRASS_VISION_RANGE)
    }
    /// AI detection respects foliage concealment; weapon clearance remains separate.
    pub(super) fn detects_tank(&self, observer: usize, target: usize) -> bool {
        if !self.tank_is_alive(observer)
            || !self.tank_is_alive(target)
            || self.concealed_from(observer, target)
        {
            return false;
        }
        let origin = self.piece_pose(observer).translation;
        let position = self.piece_pose(target).translation;
        let team = self
            .tanks
            .iter()
            .find(|t| t.id == observer)
            .map_or(self.team, |t| t.team);
        if self.radar_covers(team, position) {
            return true;
        }
        let in_grass = self.terrain_at(origin) == crate::TerrainKind::TallGrass;
        (!in_grass || origin.distance(position) <= GRASS_VISION_RANGE)
            && self.sight_path_clear(
                origin,
                Some(observer),
                position,
                in_grass || self.firing_reveals(target),
            )
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
        if let Some(frame) = &self.replica {
            return frame.vision.clone();
        }
        let mut vision = self.sight_regions(angle);
        vision.tank_visibility = self.tank_visibility(angle);
        vision
    }

    /// Host-only part of player sight: foliage, fire reveals and radar decide
    /// which tanks are detected. Trusted clients compute the geometry locally.
    pub fn host_vision(&self, angle: f32) -> Vision {
        Vision {
            origin: self.launcher_position(),
            tank_visibility: self.tank_visibility(angle),
            ..Vision::default()
        }
    }

    /// Geometric sight regions: turret cone, allied flag lookout and radar.
    pub(super) fn sight_regions(&self, angle: f32) -> Vision {
        let mut vision = if self.replica.is_none() {
            self.vision(self.player_tank_id(), angle)
        } else if self.tank_alive() {
            self.vision_from(self.launcher_position(), Some(self.launcher), Some(angle))
        } else {
            Vision {
                origin: self.launcher_position(),
                ..Vision::default()
            }
        };
        for (team, position) in self.flags() {
            if team == self.team() {
                vision.additional.push(self.vision_from(
                    position,
                    Some(self.player_tank_id()),
                    None,
                ));
            }
        }
        for station in self
            .stations()
            .filter(|s| s.owner == Some(self.team()) && s.kind == crate::level::StationKind::Radar)
        {
            vision.additional.push(Vision::from_boundary(
                station.position,
                (0..96)
                    .map(|i| {
                        station.position
                            + Vector::from_angle(i as f32 * std::f32::consts::TAU / 96.0)
                                * station.settings.radar_radius
                    })
                    .collect(),
            ));
        }
        vision
    }

    fn tank_visibility(&self, angle: f32) -> Vec<(usize, bool)> {
        let mut visibility = vec![];
        for tank in &self.tanks {
            let target = self.piece_pose(tank.id).translation;
            if tank.id == self.launcher || self.terrain_at(target) != crate::TerrainKind::TallGrass
            {
                continue;
            }
            let origin = self.launcher_position();
            let in_grass = self.terrain_at(origin) == crate::TerrainKind::TallGrass;
            let delta = target - origin;
            let range = self.vision_range_at(origin);
            let in_cone = delta.length() <= NEAR_VISION
                || (delta.length() <= range
                    && delta.normalize_or_zero().dot(Vector::from_angle(angle))
                        >= VISION_HALF_ANGLE.cos());
            let player_sees = self.tank_is_alive(self.launcher)
                && in_cone
                && self.sight_path_clear(
                    origin,
                    Some(self.launcher),
                    target,
                    in_grass || self.firing_reveals(tank.id),
                );
            let flag_sees = self.firing_reveals(tank.id)
                && self.flags().any(|(team, p)| {
                    team == self.team
                        && p.distance(target) <= NEAR_VISION
                        && self.sight_path_clear(p, None, target, true)
                });
            let visible = self.tank_is_alive(tank.id)
                && !self.concealed_from(self.launcher, tank.id)
                && (player_sees || flag_sees || self.radar_covers(self.team, target));
            visibility.push((tank.id, visible));
        }
        visibility
    }

    /// Replica sight from replicated blockers; the host supplies tank detection.
    pub(super) fn apply_replica_vision(&mut self) {
        let Some(frame) = &self.replica else {
            return;
        };
        let angle = frame.aim.angle;
        self.sight_cache = None;
        self.sight_cache = Some(self.sight_occluders(Vector::ZERO, None, f32::MAX));
        let mut vision = self.sight_regions(angle);
        self.sight_cache = None;
        let frame = self.replica.as_mut().unwrap();
        vision.tank_visibility = std::mem::take(&mut frame.vision.tank_visibility);
        frame.vision = vision;
    }

    fn vision_from(&self, origin: Vector, ignored: Option<usize>, cone: Option<f32>) -> Vision {
        let in_grass = self.terrain_at(origin) == crate::TerrainKind::TallGrass;
        let range = if cone.is_some() {
            self.vision_range_at(origin)
        } else {
            NEAR_VISION
        };
        let mut obstacles = self.sight_occluders(origin, ignored, range);
        if in_grass && cone.is_some() {
            obstacles.retain(|o| !o.grass);
        }
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
        let (corners, exposed) = silhouette(&obstacles);
        let mut exposed = exposed.into_iter();
        obstacles.retain(|o| {
            exposed.next().unwrap()
                || outline(o).is_none()
                || origin.cmpge(o.bounds.mins).all() && origin.cmple(o.bounds.maxs).all()
        });
        for p in corners {
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
                    range
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
            // With the origin outside its bounding circle, a polygon lies within
            // the wedge of its vertex angles; rays outside that wedge cannot hit.
            let (below, above) = match outline(&o) {
                Some(points) if half_angle < std::f32::consts::PI => {
                    points.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &v| {
                        let d = v - origin;
                        let relative = (d.to_angle() - middle + std::f32::consts::PI)
                            .rem_euclid(tau)
                            - std::f32::consts::PI;
                        (lo.min(relative - 0.0001), hi.max(relative + 0.0001))
                    })
                }
                _ => (-half_angle, half_angle),
            };
            let (below, above) = (below.max(-half_angle), above.min(half_angle));
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
                let lo = middle + below;
                let hi = middle + above;
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
            tank_visibility: vec![],
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
    #[test]
    fn elevation_extends_sight_without_blocking_shots() {
        let level = Level {
            size: [100.0, 40.0],
            items: vec![
                LevelItem::Spawn {
                    team: Team::Blue,
                    position: [-20.0, 0.0],
                    angle: 0.0,
                },
                LevelItem::Terrain {
                    terrain: crate::TerrainKind::Elevation,
                    min: [-25.0, -5.0],
                    max: [-5.0, 5.0],
                },
            ],
            ..Level::default()
        };
        assert_eq!(Level::from_json(&level.to_json().unwrap()).unwrap(), level);
        let mut world = World::from_level(&level, Team::Blue).unwrap();
        let vision = world.player_vision(0.0);
        assert!(vision.contains(Vector::new(10.0, 0.0)));
        assert!(!vision.contains(Vector::new(17.0, 0.0)));
        world.step(&[Command::Fire(Shot::new(
            Ammunition::ArmorPiercing,
            Vector::new(30.0, 0.0),
        ))]);
        for _ in 0..4 {
            world.step(&[]);
        }
        assert!(
            world.shells().any(|s| s.position.x > -5.0),
            "shell crosses the elevation edge"
        );
        world.terrain.clear();
        assert!(!world.player_vision(0.0).contains(Vector::new(10.0, 0.0)));
    }
    fn grass_tanks(observer_in_grass: bool) -> World {
        let mut level = map(Some(LevelItem::Terrain {
            terrain: crate::TerrainKind::TallGrass,
            min: [if observer_in_grass { -2.0 } else { 3.0 }, -3.0],
            max: [20.0, 3.0],
        }));
        level.items.push(LevelItem::Spawn {
            team: Team::Red,
            position: [6.0, 0.0],
            angle: 0.0,
        });
        World::from_level(&level, Team::Blue).unwrap()
    }
    #[test]
    fn grass_keeps_short_vision_and_tanks_inside_can_detect_each_other() {
        let world = grass_tanks(true);
        let enemy = world.tanks[1].id;
        let vision = world.player_vision(0.0);
        assert!(vision.contains(Vector::new(4.0, 0.0)));
        assert!(vision.contains(Vector::new(-2.0, 0.0)));
        assert!(!vision.contains(Vector::new(9.0, 2.0)));
        assert_eq!(vision.tank_visibility(enemy), Some(true));
        assert_eq!(
            world
                .player_vision(std::f32::consts::PI)
                .tank_visibility(enemy),
            Some(false)
        );
        assert!(world.detects_tank(world.launcher, enemy));
        assert!(world.detects_tank(enemy, world.launcher));
        let mut level = map(Some(LevelItem::Terrain {
            terrain: crate::TerrainKind::TallGrass,
            min: [-2.0, -3.0],
            max: [20.0, 3.0],
        }));
        level.items.push(LevelItem::Solid {
            outline: rectangle(Vector::new(0.2, 3.0)),
            position: [3.0, 0.0],
            angle: 0.0,
            cutouts: vec![],
        });
        let blocked = World::from_level(&level, Team::Blue).unwrap();
        assert!(!blocked.player_vision(0.0).contains(Vector::new(5.0, 0.0)));
    }
    #[test]
    fn grass_reveal_requires_actual_fire_expires_and_refreshes() {
        let mut world = grass_tanks(false);
        let enemy = world.tanks[1].id;
        let visible = |w: &World| w.player_vision(0.0).tank_visibility(enemy).unwrap();
        assert!(!visible(&world));
        assert!(!world.detects_tank(world.launcher, enemy));
        // The tank inside can see the nearby tank outside, without being seen back.
        assert!(world.detects_tank(enemy, world.launcher));
        let shot = Shot::new(Ammunition::ArmorPiercing, Vector::new(15.0, 0.0));
        world.tanks[1].reload = 1.0;
        world.fire_from(enemy, shot, &mut vec![]);
        assert!(
            !visible(&world),
            "pressing fire while reloading does not reveal"
        );
        world.tanks[1].reload = 0.0;
        world.fire_from(enemy, shot, &mut vec![]);
        assert!(visible(&world));
        assert!(world.detects_tank(world.launcher, enemy));
        assert!(
            !world.player_vision(0.0).contains(Vector::new(10.0, 2.0)),
            "a shot reveals the tank, not the whole grass patch"
        );
        world.tick = 599;
        assert!(visible(&world));
        world.tick = 600;
        assert!(!visible(&world));
        world.tanks[1].reload = 0.0;
        world.fire_from(enemy, shot, &mut vec![]);
        world.tick = 1199;
        assert!(visible(&world));
        world.tick = 1200;
        assert!(!visible(&world));
    }
    #[test]
    fn grass_shots_do_not_reveal_through_walls_and_death_clears_reveal() {
        let mut world = grass_tanks(false);
        let enemy = world.tanks[1].id;
        world.reveal_firing_tank(enemy);
        let level = Level {
            items: vec![
                LevelItem::Spawn {
                    team: Team::Blue,
                    position: [0.0, 0.0],
                    angle: 0.0,
                },
                LevelItem::Spawn {
                    team: Team::Red,
                    position: [6.0, 0.0],
                    angle: 0.0,
                },
                LevelItem::Terrain {
                    terrain: crate::TerrainKind::TallGrass,
                    min: [3.0, -3.0],
                    max: [20.0, 3.0],
                },
                LevelItem::Solid {
                    outline: rectangle(Vector::new(0.2, 3.0)),
                    position: [3.0, 0.0],
                    angle: 0.0,
                    cutouts: vec![],
                },
            ],
            ..Level::default()
        };
        let mut blocked = World::from_level(&level, Team::Blue).unwrap();
        let blocked_enemy = blocked.tanks[1].id;
        blocked.reveal_firing_tank(blocked_enemy);
        assert_eq!(
            blocked.player_vision(0.0).tank_visibility(blocked_enemy),
            Some(false)
        );
        assert!(!blocked.detects_tank(blocked.launcher, blocked_enemy));
        world.destroy_tank(enemy, &mut vec![]);
        assert!(!world.firing_reveals(enemy));
    }
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

    fn sized_wall(half: Vector, refinement: u8) -> LevelItem {
        let mut asset = crate::LibraryPreset::Wall.asset();
        asset.outline = rectangle(half);
        asset.layout = None;
        asset.refinement = refinement;
        asset.components.blocks_vision = Some(true);
        LevelItem::Object {
            asset: Box::new(asset),
            position: [5.0, 0.0],
            angle: 0.0,
            cutouts: vec![],
        }
    }

    #[test]
    fn small_props_are_not_occluders_but_keep_physical_collision() {
        for item in [
            sized_wall(Vector::splat(0.3), 2),
            LevelItem::Solid {
                outline: rectangle(Vector::splat(0.3)),
                position: [5.0, 0.0],
                angle: 0.7,
                cutouts: vec![],
            },
        ] {
            let mut level = map(Some(item));
            let world = World::from_level(&level, Team::Blue).unwrap();
            let baseline = World::from_level(&map(None), Team::Blue).unwrap();
            assert_eq!(
                world
                    .sight_occluders(Vector::ZERO, Some(world.launcher), VISION_RANGE)
                    .len(),
                baseline
                    .sight_occluders(Vector::ZERO, Some(baseline.launcher), VISION_RANGE)
                    .len(),
                "small props must not contribute any occlusion geometry"
            );
            assert!(world.player_vision(0.0).contains(Vector::new(9.0, 0.0)));
            assert!(world.sight_clear(world.launcher, Vector::new(9.0, 0.0)));
            assert!(!world.tank_pose_clear(world.launcher, Pose::new(Vector::new(5.0, 0.0), 0.0)));

            level.items.push(LevelItem::Spawn {
                team: Team::Red,
                position: [9.0, 0.0],
                angle: 0.0,
            });
            let world = World::from_level(&level, Team::Blue).unwrap();
            let enemy = world.tanks[1].id;
            assert!(world.detects_tank(world.launcher, enemy));
            assert!(world.detects_tank(enemy, world.launcher));
        }
    }

    #[test]
    fn intact_finely_meshed_cover_blocks_but_small_detached_chunks_do_not() {
        let mut world =
            World::from_level(&map(Some(sized_wall(Vector::new(0.3, 2.0), 3))), Team::Blue)
                .unwrap();
        assert!(
            world
                .pieces
                .iter()
                .filter(|p| p.destructible())
                .all(|p| p.shape.area() < MIN_SIGHT_BLOCKER_AREA)
        );
        let target = Vector::new(9.0, 0.0);
        assert!(!world.player_vision(0.0).contains(target));
        assert!(!world.sight_clear(world.launcher, target));
        // Detach the cells without moving or retiring them. Their original
        // object identity must not keep the new small chunks opaque.
        for bond in &mut world.bonds {
            bond.active = false;
        }
        world.rebuild_chunks();
        let baseline = World::from_level(&map(None), Team::Blue).unwrap();
        assert_eq!(
            world
                .sight_occluders(Vector::ZERO, Some(world.launcher), VISION_RANGE)
                .len(),
            baseline
                .sight_occluders(Vector::ZERO, Some(baseline.launcher), VISION_RANGE)
                .len(),
            "detached small chunks must not contribute any occlusion geometry"
        );
        assert!(world.player_vision(0.0).contains(target));
        assert!(world.sight_clear(world.launcher, target));
    }

    #[test]
    fn fixed_cover_size_uses_the_whole_object_and_cutoff_is_in_world_units() {
        // Each triangle has only 0.4 m², but together they form a 0.8 m² wall.
        let world = World::from_level(
            &map(Some(LevelItem::Solid {
                outline: rectangle(Vector::new(0.1, 2.0)),
                position: [5.0, 0.0],
                angle: 0.0,
                cutouts: vec![],
            })),
            Team::Blue,
        )
        .unwrap();
        assert!(!world.player_vision(0.0).contains(Vector::new(9.0, 0.0)));
        assert!(!world.sight_clear(world.launcher, Vector::new(9.0, 0.0)));
        for (half, blocks) in [
            (Vector::new(0.25, 0.49), false),
            (Vector::new(0.25, 0.5), true),
        ] {
            let world = World::from_level(&map(Some(sized_wall(half, 0))), Team::Blue).unwrap();
            assert_eq!(
                !world.sight_clear(world.launcher, Vector::new(9.0, 0.0)),
                blocks
            );
        }
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
                    | crate::LibraryPreset::Shrub
                    | crate::LibraryPreset::Crate
                    | crate::LibraryPreset::Timber
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
