//! Derived, non-overlapping geometry shared by the editor and simulation.
//! Recipes stay editable: moving a cutter restores the material it used to cover.
use super::*;
use std::collections::BTreeMap;

const EPS: f32 = 1e-5;

impl Level {
    /// Permanently erase solids under a world-space rectangle. Each cut travels
    /// with its object; newly placed objects can fill the erased area again.
    pub fn erase_rectangle(&mut self, min: Vec2, max: Vec2) -> Result<usize, String> {
        if !min.is_finite() || !max.is_finite() || !max.cmpgt(min).all() {
            return Err("Eraser rectangle must have positive, finite dimensions.".into());
        }
        let corners = [min, Vec2::new(max.x, min.y), max, Vec2::new(min.x, max.y)];
        let mut changed = 0;
        let mut remaining = Vec::with_capacity(self.items.len());
        for item in &self.items {
            let mut item = item.clone();
            if matches!(item, LevelItem::Object { .. } | LevelItem::Solid { .. }) {
                let inverse = Vec2::from_angle(-item.angle());
                let cutout = corners.map(|p| inverse.rotate(p - item.position()).to_array());
                let mesh = item.erased_mesh()?;
                let after = carve(mesh.clone(), &cutout_triangles(&[cutout]))?;
                if after.area < mesh.area - EPS {
                    changed += 1;
                    if after.triangles.is_empty() {
                        continue;
                    }
                    if let LevelItem::Object { cutouts, .. } | LevelItem::Solid { cutouts, .. } =
                        &mut item
                    {
                        cutouts.push(cutout);
                    }
                }
            }
            remaining.push(item);
        }
        self.items = remaining;
        Ok(changed)
    }

    /// Unbreakable objects win over ordinary objects; later items win ties.
    /// Mesh vertices remain in each item's local coordinates. Non-solids are None.
    pub fn resolved_meshes(&self) -> Result<Vec<Option<ObjectMesh>>, String> {
        let footprints: Vec<_> = self
            .items
            .iter()
            .map(|item| match item {
                LevelItem::Object { .. } | LevelItem::Solid { .. } | LevelItem::Station { .. } => {
                    let mesh = solid_asset(&item.local_outline()).level_mesh()?;
                    carve(mesh, &cutout_triangles(item.cutouts())).map(Some)
                }
                _ => Ok(None),
            })
            .collect::<Result<_, String>>()?;
        let mut result = Vec::with_capacity(self.items.len());
        let mut pieces = 0;
        for (i, item) in self.items.iter().enumerate() {
            let mesh = match item {
                LevelItem::Object { .. } | LevelItem::Solid { .. } | LevelItem::Station { .. } => {
                    item.erased_mesh()?
                }
                _ => {
                    result.push(None);
                    continue;
                }
            };
            let priority = |index: usize| {
                (
                    matches!(
                        self.items[index],
                        LevelItem::Solid { .. } | LevelItem::Station { .. }
                    ),
                    index,
                )
            };
            let inverse = Vec2::from_angle(-item.angle());
            let mut cutters = Vec::new();
            for (j, footprint) in footprints.iter().enumerate() {
                if priority(j) <= priority(i) {
                    continue;
                }
                let Some(footprint) = footprint else {
                    continue;
                };
                let other = &self.items[j];
                let rotation = Vec2::from_angle(other.angle());
                for triangle in &footprint.triangles {
                    cutters.push(triangle.map(|v| {
                        inverse.rotate(
                            other.position() + rotation.rotate(footprint.vertices[v])
                                - item.position(),
                        )
                    }));
                }
            }
            let mesh = carve(mesh, &cutters)?;
            if matches!(item, LevelItem::Station { .. }) && mesh.area < 4.0 - EPS {
                return Err("Station buildings need a clear footprint: move the overlapping unbreakable object.".into());
            }
            pieces += mesh.triangles.len();
            if pieces > 8192 {
                return Err("Overlapping objects exceed the 8192-piece budget; simplify their outlines or grains.".into());
            }
            result.push(Some(mesh));
        }
        Ok(result)
    }
}

fn cutout_triangles(cutouts: &[[[f32; 2]; 4]]) -> Vec<[Vec2; 3]> {
    cutouts
        .iter()
        .flat_map(|p| {
            let p = p.map(Vec2::from);
            [[p[0], p[1], p[2]], [p[0], p[2], p[3]]]
        })
        .collect()
}

impl LevelItem {
    fn erased_mesh(&self) -> Result<ObjectMesh, String> {
        let mesh = match self {
            Self::Object { asset, .. } => asset.level_mesh()?,
            Self::Solid { outline, .. } => solid_asset(outline).level_mesh()?,
            Self::Station { .. } => solid_asset(&self.local_outline()).level_mesh()?,
            _ => return Err("Only solid objects have an eraser mesh.".into()),
        };
        carve(mesh, &cutout_triangles(self.cutouts()))
    }
}

impl ObjectMesh {
    /// Exterior segments, including the boundaries of holes.
    pub fn boundary_segments(&self) -> Vec<[Vec2; 2]> {
        let mut edges = BTreeMap::new();
        for t in &self.triangles {
            for i in 0..3 {
                let (a, b) = (t[i], t[(i + 1) % 3]);
                *edges.entry((a.min(b), a.max(b))).or_insert(0) += 1;
            }
        }
        edges
            .into_iter()
            .filter(|(_, count)| *count == 1)
            .map(|((a, b), _)| [self.vertices[a], self.vertices[b]])
            .collect()
    }

    pub fn contains_point(&self, point: Vec2) -> bool {
        self.triangles.iter().any(|t| {
            let p = t.map(|i| self.vertices[i]);
            let signs = [0, 1, 2].map(|i| (p[(i + 1) % 3] - p[i]).perp_dot(point - p[i]));
            signs.iter().all(|&d| d >= -EPS) || signs.iter().all(|&d| d <= EPS)
        })
    }
}

fn area(p: &[Vec2]) -> f32 {
    (1..p.len().saturating_sub(1))
        .map(|i| (p[i] - p[0]).perp_dot(p[i + 1] - p[0]).abs() * 0.5)
        .sum()
}

fn half_plane(polygon: &[Vec2], a: Vec2, normal: Vec2) -> Vec<Vec2> {
    let mut out = Vec::new();
    for i in 0..polygon.len() {
        let (p, q) = (polygon[i], polygon[(i + 1) % polygon.len()]);
        let (dp, dq) = ((p - a).dot(normal), (q - a).dot(normal));
        if dp >= 0.0 {
            out.push(p);
        }
        if (dp >= 0.0) != (dq >= 0.0) {
            out.push(p.lerp(q, dp / (dp - dq)));
        }
    }
    out.dedup_by(|a, b| a.distance_squared(*b) < EPS * EPS);
    if out.len() > 1 && out[0].distance_squared(*out.last().unwrap()) < EPS * EPS {
        out.pop();
    }
    out
}

fn subtract(polygon: Vec<Vec2>, cutter: &[Vec2; 3]) -> Vec<Vec<Vec2>> {
    let winding = (cutter[1] - cutter[0])
        .perp_dot(cutter[2] - cutter[0])
        .signum();
    let normals = [0, 1, 2].map(|i| (cutter[(i + 1) % 3] - cutter[i]).perp().normalize() * winding);
    // Test intersection first: touching or distant cutters must not subdivide cells.
    let mut inside = polygon.clone();
    for i in 0..3 {
        inside = half_plane(&inside, cutter[i], normals[i]);
    }
    if area(&inside) < EPS * EPS {
        return vec![polygon];
    }
    let mut remaining = polygon;
    let mut out = Vec::new();
    for i in 0..3 {
        let outside = half_plane(&remaining, cutter[i], -normals[i]);
        if area(&outside) > EPS * EPS {
            out.push(outside);
        }
        remaining = half_plane(&remaining, cutter[i], normals[i]);
    }
    out
}

fn carve(mesh: ObjectMesh, cutters: &[[Vec2; 3]]) -> Result<ObjectMesh, String> {
    let bounds = |points: &[Vec2]| {
        points.iter().fold(
            (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY)),
            |(lo, hi), &p| (lo.min(p), hi.max(p)),
        )
    };
    let (lo, hi) = bounds(&mesh.vertices);
    let cutters: Vec<_> = cutters
        .iter()
        .filter(|c| {
            let (a, b) = bounds(*c);
            lo.cmplt(b - Vec2::splat(EPS)).all() && a.cmplt(hi - Vec2::splat(EPS)).all()
        })
        .collect();
    if cutters.is_empty() {
        return Ok(mesh);
    }
    let mut faces = Vec::new();
    let mut changed = false;
    for (cell, triangle) in mesh.triangles.iter().enumerate() {
        let original = triangle.map(|v| mesh.vertices[v]).to_vec();
        let mut polygons = vec![original.clone()];
        for cutter in &cutters {
            polygons = polygons
                .into_iter()
                .flat_map(|p| subtract(p, cutter))
                .collect();
            if polygons.len() > 8192 {
                return Err("Too many overlapping edges; simplify the map.".into());
            }
        }
        changed |= polygons.len() != 1 || polygons[0] != original;
        faces.extend(polygons.into_iter().map(|p| (p, mesh.grains[cell])));
    }
    if !changed {
        return Ok(mesh);
    }
    // Weld clipping intersections and insert them on every adjacent edge. This
    // keeps the fracture mesh conforming instead of creating unbonded T junctions.
    let mut vertices: Vec<Vec2> = Vec::new();
    let mut buckets: BTreeMap<(i32, i32), Vec<usize>> = BTreeMap::new();
    let mut indexed = Vec::new();
    for (face, grain) in faces {
        let mut ids = Vec::new();
        for p in face {
            let key = ((p.x / EPS).floor() as i32, (p.y / EPS).floor() as i32);
            let mut found = None;
            for dx in -1..=1 {
                for dy in -1..=1 {
                    if let Some(bucket) = buckets.get(&(key.0 + dx, key.1 + dy)) {
                        found = found.or_else(|| {
                            bucket
                                .iter()
                                .copied()
                                .find(|&i| vertices[i].distance_squared(p) < EPS * EPS)
                        });
                    }
                }
            }
            let id = found.unwrap_or_else(|| {
                let id = vertices.len();
                vertices.push(p);
                buckets.entry(key).or_default().push(id);
                id
            });
            ids.push(id);
        }
        ids.dedup();
        if ids.len() > 1 && ids.first() == ids.last() {
            ids.pop();
        }
        if ids.len() >= 3 {
            indexed.push((ids, grain));
        }
    }
    let boundary_count = vertices.len();
    let mut result = ObjectMesh {
        vertices,
        triangles: vec![],
        grains: vec![],
        area: 0.0,
    };
    for (face, grain) in indexed {
        let mut conformed = Vec::new();
        for i in 0..face.len() {
            let (a, b) = (face[i], face[(i + 1) % face.len()]);
            let start = result.vertices[a];
            let edge = result.vertices[b] - start;
            let length = edge.length();
            let mut along = vec![(0.0, a)];
            for (id, &p) in result.vertices[..boundary_count].iter().enumerate() {
                if id == a || id == b {
                    continue;
                }
                let t = (p - start).dot(edge) / length;
                if t > EPS && t < length - EPS && edge.perp_dot(p - start).abs() < EPS * length {
                    along.push((t, id));
                }
            }
            along.sort_by(|a, b| a.0.total_cmp(&b.0));
            conformed.extend(along.into_iter().map(|(_, id)| id));
        }
        let triangles = if conformed.len() == 3 {
            vec![[conformed[0], conformed[1], conformed[2]]]
        } else {
            let center = conformed.iter().map(|&i| result.vertices[i]).sum::<Vec2>()
                / conformed.len() as f32;
            let id = result.vertices.len();
            result.vertices.push(center);
            (0..conformed.len())
                .map(|i| [id, conformed[i], conformed[(i + 1) % conformed.len()]])
                .collect()
        };
        for t in triangles {
            let weight = area(&t.map(|i| result.vertices[i]));
            if weight > EPS * EPS {
                result.triangles.push(t);
                result.grains.push(grain);
                result.area += weight;
            }
        }
        if result.triangles.len() > 8192 {
            return Err(
                "Carved object exceeds the piece budget; simplify its outline or grains.".into(),
            );
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eraser_preserves_holes_through_save_transforms_and_overlap() {
        for solid in [false, true] {
            let mut item = wall();
            if solid {
                item = LevelItem::Solid {
                    outline: rectangle(Vec2::new(1.0, 3.0)),
                    position: [0.0, 0.0],
                    angle: 0.0,
                    cutouts: vec![],
                };
            }
            let mut level = Level {
                items: vec![item],
                ..Level::default()
            };
            assert_eq!(
                level
                    .erase_rectangle(Vec2::splat(-0.5), Vec2::splat(0.5))
                    .unwrap(),
                1
            );
            let mesh = level.resolved_meshes().unwrap().remove(0).unwrap();
            assert!((mesh.area - 11.0).abs() < 0.001);
            assert!(!mesh.contains_point(Vec2::ZERO));
            assert!(
                (mesh
                    .boundary_segments()
                    .iter()
                    .map(|[a, b]| a.distance(*b))
                    .sum::<f32>()
                    - 20.0)
                    .abs()
                    < 0.001
            );
            let before = level.clone();
            assert_eq!(
                level
                    .erase_rectangle(Vec2::splat(-0.5), Vec2::splat(0.5))
                    .unwrap(),
                0
            );
            assert_eq!(before, level);
            level = Level::from_json(&level.to_json().unwrap()).unwrap();
            level.items[0].translate(Vec2::new(5.0, 2.0));
            if let LevelItem::Object { angle, .. } | LevelItem::Solid { angle, .. } =
                &mut level.items[0]
            {
                *angle = 0.6;
            }
            level.items[0].resize(Vec2::new(4.0, 12.0)).unwrap();
            let mesh = level.resolved_meshes().unwrap().remove(0).unwrap();
            assert!((mesh.area - 44.0).abs() < 0.001);
            assert!(!mesh.contains_point(Vec2::ZERO));
            assert!(!level.items[0].can_join(&level.items[0]));
        }
        // Erase both overlapping layers, then allow fresh material in the hole.
        let mut level = Level {
            items: vec![wall(), block(0.0)],
            ..Level::default()
        };
        assert_eq!(
            level
                .erase_rectangle(Vec2::splat(-0.2), Vec2::splat(0.2))
                .unwrap(),
            2
        );
        for mesh in level.resolved_meshes().unwrap().into_iter().flatten() {
            assert!(!mesh.contains_point(Vec2::ZERO));
        }
        level.items.push(wall());
        assert!(
            level.resolved_meshes().unwrap()[2]
                .as_ref()
                .unwrap()
                .contains_point(Vec2::ZERO)
        );
        assert_eq!(
            level
                .erase_rectangle(Vec2::splat(-4.0), Vec2::splat(4.0))
                .unwrap(),
            3
        );
        assert!(level.items.is_empty());
    }

    #[test]
    fn erased_opening_is_clear_in_the_simulation() {
        let mut level = Level {
            items: vec![
                LevelItem::Solid {
                    outline: rectangle(Vec2::splat(4.0)),
                    position: [0.0, 0.0],
                    angle: 0.37,
                    cutouts: vec![],
                },
                LevelItem::Spawn {
                    team: Team::Blue,
                    position: [0.0, 0.0],
                    angle: 0.0,
                },
            ],
            ..Level::default()
        };
        assert!(crate::World::from_level(&level, Team::Blue).is_err());
        level
            .erase_rectangle(Vec2::splat(-2.0), Vec2::splat(2.0))
            .unwrap();
        let mut world = crate::World::from_level(&level, Team::Blue).unwrap();
        for _ in 0..120 {
            world.step(&[]);
        }
        assert!(world.launcher_position().length() < 0.01);
    }

    fn wall() -> LevelItem {
        LevelItem::Object {
            cutouts: vec![],
            asset: Box::new(wall_asset(Vec2::new(2.0, 6.0))),
            position: [0.0, 0.0],
            angle: 0.0,
        }
    }
    fn block(angle: f32) -> LevelItem {
        LevelItem::Solid {
            cutouts: vec![],
            outline: rectangle(Vec2::splat(0.3)),
            position: [0.0, 0.0],
            angle,
        }
    }

    #[test]
    fn embedded_cutout_has_no_hidden_material_or_unbonded_internal_edges() {
        for angle in [0.0, 0.37] {
            for reverse in [false, true] {
                let mut items = vec![wall(), block(angle)];
                if reverse {
                    items.reverse();
                }
                let level = Level {
                    items,
                    ..Level::default()
                };
                let meshes = level.resolved_meshes().unwrap();
                let wall = meshes[usize::from(reverse)].as_ref().unwrap();
                assert!((wall.area - 11.64).abs() < 1e-4);
                assert!(!wall.contains_point(Vec2::ZERO));
                assert!(wall.contains_point(Vec2::new(0.0, 2.0)));
                // The only free edges are the outer rectangle and the cutout.
                // A T junction would leave extra, unbonded internal edges here.
                let perimeter: f32 = wall
                    .boundary_segments()
                    .iter()
                    .map(|[a, b]| a.distance(*b))
                    .sum();
                assert!((perimeter - 18.4).abs() < 0.001, "perimeter {perimeter}");
                let rotation = Vec2::from_angle(-angle);
                for t in &wall.triangles {
                    let points = t.map(|i| rotation.rotate(wall.vertices[i]));
                    let mut overlap = points.to_vec();
                    for (a, n) in [
                        (Vec2::splat(-0.3), Vec2::X),
                        (Vec2::splat(0.3), -Vec2::X),
                        (Vec2::splat(-0.3), Vec2::Y),
                        (Vec2::splat(0.3), -Vec2::Y),
                    ] {
                        overlap = half_plane(&overlap, a, n);
                    }
                    assert!(area(&overlap) < 1e-7);
                }
            }
        }
    }

    #[test]
    fn latest_peer_wins_and_moving_it_restores_the_covered_material() {
        for items in [vec![wall(), wall()], vec![block(0.0), block(0.0)]] {
            let mut level = Level {
                items,
                ..Level::default()
            };
            let before = level.clone();
            let meshes = level.resolved_meshes().unwrap();
            assert!(meshes[0].as_ref().unwrap().triangles.is_empty());
            assert!(meshes[1].as_ref().unwrap().area > 0.0);
            assert_eq!(level, before, "carving preserves the editable recipe");
            level.items[1].translate(Vec2::new(5.0, 0.0));
            let meshes = level.resolved_meshes().unwrap();
            assert!(
                (meshes[0].as_ref().unwrap().area - meshes[1].as_ref().unwrap().area).abs() < 1e-5
            );
            let restored = Level::from_json(&level.to_json().unwrap())
                .unwrap()
                .resolved_meshes()
                .unwrap();
            assert_eq!(
                meshes[0].as_ref().unwrap().triangles,
                restored[0].as_ref().unwrap().triangles
            );
        }
    }
}
