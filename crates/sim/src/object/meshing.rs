//! Authored grain regions, independent of the internal solver triangulation.
//! Clip in f64, dissolve construction diagonals, then weld all shared edges.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

const MAX_GRAINS: usize = 128;
type P = [f64; 2];
type Partition = (Vec<Vec2>, Vec<[usize; 3]>, Vec<usize>);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrainMode {
    Stones,
    Bricks,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrainLayout {
    pub mode: GrainMode,
    pub size: [f32; 2],
    pub seed: u32,
    /// Voronoi sites are authored data: editing a site changes its grain.
    pub sites: Vec<[f32; 2]>,
}

impl GrainLayout {
    pub fn generate(outline: &[[f32; 2]], mode: GrainMode, size: [f32; 2], seed: u32) -> Self {
        let mut layout = Self {
            mode,
            size,
            seed,
            sites: vec![],
        };
        if mode == GrainMode::Bricks || size.iter().any(|v| !v.is_finite() || *v < 0.15) {
            return layout;
        }
        let (min, max) = bounds(outline);
        let nx = ((max[0] - min[0]) / size[0] as f64).ceil().max(1.0) as usize;
        let ny = ((max[1] - min[1]) / size[1] as f64).ceil().max(1.0) as usize;
        let mut random = seed;
        for y in 0..ny.min(MAX_GRAINS + 1) {
            for x in 0..nx.min(MAX_GRAINS + 1) {
                let mut noise = || {
                    random = random.wrapping_mul(1664525).wrapping_add(1013904223);
                    (random >> 8) as f64 / 16777216.0 - 0.5
                };
                let px = min[0]
                    + (x as f64 + 0.5 + if y % 2 == 1 { 0.45 } else { 0.0 } + noise() * 0.18)
                        * (max[0] - min[0])
                        / nx as f64;
                let py = min[1] + (y as f64 + 0.5 + noise() * 0.18) * (max[1] - min[1]) / ny as f64;
                if contains(outline, [px as f32, py as f32]) {
                    layout.sites.push([px as f32, py as f32]);
                }
                if layout.sites.len() > MAX_GRAINS {
                    return layout;
                }
            }
        }
        layout
    }

    fn validate(&self, bounds: [f32; 2]) -> Result<(), String> {
        if self
            .size
            .iter()
            .any(|v| !v.is_finite() || !(0.15..=200.0).contains(v))
        {
            return Err("Grain dimensions must be between 0.15 and 200 m.".into());
        }
        if self.sites.len() > MAX_GRAINS {
            return Err("Use at most 128 grains; increase grain dimensions.".into());
        }
        if self.mode == GrainMode::Stones && self.sites.is_empty() {
            return Err("Add a grain point inside the outline, or regenerate the layout.".into());
        }
        for (i, p) in self.sites.iter().enumerate() {
            if p.iter().any(|v| !v.is_finite()) || p[0].abs() > bounds[0] || p[1].abs() > bounds[1]
            {
                return Err("Keep grain points inside the workshop bounds.".into());
            }
            if self.sites[..i]
                .iter()
                .any(|q| Vec2::from(*p).distance(Vec2::from(*q)) < 0.04)
            {
                return Err("Grain points need at least 4 cm separation.".into());
            }
        }
        Ok(())
    }
}

pub fn contains(outline: &[[f32; 2]], p: [f32; 2]) -> bool {
    let mut inside = false;
    for i in 0..outline.len() {
        let a = outline[i];
        let b = outline[(i + 1) % outline.len()];
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
    }
    inside
}
fn bounds(points: &[[f32; 2]]) -> (P, P) {
    let mut min = [f64::INFINITY; 2];
    let mut max = [f64::NEG_INFINITY; 2];
    for p in points {
        for d in 0..2 {
            min[d] = min[d].min(p[d] as f64);
            max[d] = max[d].max(p[d] as f64);
        }
    }
    (min, max)
}
fn cross(a: P, b: P, c: P) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}
fn clip(polygon: Vec<P>, normal: P, offset: f64) -> Vec<P> {
    let mut out = Vec::new();
    if polygon.is_empty() {
        return out;
    }
    for i in 0..polygon.len() {
        let a = polygon[i];
        let b = polygon[(i + 1) % polygon.len()];
        let da = a[0] * normal[0] + a[1] * normal[1] - offset;
        let db = b[0] * normal[0] + b[1] * normal[1] - offset;
        if da <= 1e-10 {
            out.push(a);
        }
        if (da < 0.0 && db > 0.0) || (db < 0.0 && da > 0.0) {
            let t = da / (da - db);
            out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
        }
    }
    out
}
fn vertex(p: P, points: &mut Vec<P>, ids: &mut BTreeMap<(i64, i64), usize>) -> usize {
    let key = ((p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64);
    *ids.entry(key).or_insert_with(|| {
        points.push(p);
        points.len() - 1
    })
}

fn collapse_short_edges(
    loops: &mut [(usize, Vec<usize>)],
    points: &[P],
    outline: &[Vec2],
    minimum: f64,
) {
    let on_edge = |p: P, a: P, b: P| {
        cross(a, b, p).abs() < 1e-7
            && (p[0] - a[0]) * (p[0] - b[0]) + (p[1] - a[1]) * (p[1] - b[1]) <= 1e-8
    };
    let boundary: Vec<Vec<usize>> = points
        .iter()
        .map(|&p| {
            (0..outline.len())
                .filter(|&i| {
                    on_edge(
                        p,
                        outline[i].to_array().map(f64::from),
                        outline[(i + 1) % outline.len()].to_array().map(f64::from),
                    )
                })
                .collect()
        })
        .collect();
    loop {
        let candidate = loops
            .iter()
            .flat_map(|(_, p)| (0..p.len()).map(move |i| (p[i], p[(i + 1) % p.len()])))
            .filter_map(|(a, b)| {
                if a == b {
                    return None;
                }
                let distance = (points[a][0] - points[b][0]).hypot(points[a][1] - points[b][1]);
                if distance >= minimum {
                    return None;
                }
                let (keep, remove) = if boundary[a].len() > boundary[b].len()
                    || (boundary[a].len() == boundary[b].len() && a < b)
                {
                    (a, b)
                } else {
                    (b, a)
                };
                if !boundary[remove].iter().all(|e| boundary[keep].contains(e)) {
                    return None;
                }
                Some((distance, keep, remove))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        let Some((_, keep, remove)) = candidate else {
            break;
        };
        for (_, polygon) in loops.iter_mut() {
            for id in polygon.iter_mut() {
                if *id == remove {
                    *id = keep;
                }
            }
            polygon.dedup();
            if polygon.len() > 1 && polygon.first() == polygon.last() {
                polygon.pop();
            }
        }
    }
}

pub(super) fn partition(
    outline: &[Vec2],
    coarse: &[[usize; 3]],
    layout: &GrainLayout,
    limits: [f32; 2],
) -> Result<Partition, String> {
    partition_impl(outline, coarse, layout, limits, false)
        .or_else(|_| partition_impl(outline, coarse, layout, limits, true))
}

fn partition_impl(
    outline: &[Vec2],
    coarse: &[[usize; 3]],
    layout: &GrainLayout,
    limits: [f32; 2],
    robust: bool,
) -> Result<Partition, String> {
    layout.validate(limits)?;
    let mut regions: Vec<Vec<(P, f64)>> = Vec::new();
    if layout.mode == GrainMode::Stones {
        for (i, a) in layout.sites.iter().enumerate() {
            let a = a.map(f64::from);
            regions.push(
                layout
                    .sites
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != i)
                    .map(|(_, b)| {
                        let b = b.map(f64::from);
                        (
                            [b[0] - a[0], b[1] - a[1]],
                            (b[0] * b[0] + b[1] * b[1] - a[0] * a[0] - a[1] * a[1]) * 0.5,
                        )
                    })
                    .collect(),
            );
        }
    } else {
        let points: Vec<_> = outline.iter().map(|v| v.to_array()).collect();
        let (min, max) = bounds(&points);
        let nx = ((max[0] - min[0]) / layout.size[0] as f64).ceil().max(1.0) as usize;
        let ny = ((max[1] - min[1]) / layout.size[1] as f64).ceil().max(1.0) as usize;
        if (nx + 1) * ny > MAX_GRAINS {
            return Err("Brick layout exceeds 128 grains; increase brick dimensions.".into());
        }
        let dx = (max[0] - min[0]) / nx as f64;
        let dy = (max[1] - min[1]) / ny as f64;
        for y in 0..ny {
            for x in 0..nx + usize::from(y % 2 == 1) {
                let left = min[0] + (x as f64 - if y % 2 == 1 { 0.5 } else { 0.0 }) * dx;
                let bottom = min[1] + y as f64 * dy;
                regions.push(vec![
                    ([-1.0, 0.0], -left),
                    ([1.0, 0.0], left + dx),
                    ([0.0, -1.0], -bottom),
                    ([0.0, 1.0], bottom + dy),
                ]);
            }
        }
    }
    let mut points = Vec::new();
    let mut ids = BTreeMap::new();
    let mut loops = Vec::new();
    for (grain, planes) in regions.iter().enumerate() {
        if robust {
            use i_overlay::{
                core::{fill_rule::FillRule, overlay_rule::OverlayRule},
                float::single::SingleFloatOverlay,
            };
            let subject: Vec<P> = outline.iter().map(|p| [p.x as f64, p.y as f64]).collect();
            let (x, y) = (limits[0] as f64 + 1.0, limits[1] as f64 + 1.0);
            let mut region = vec![[-x, -y], [x, -y], [x, y], [-x, y]];
            for &(normal, offset) in planes {
                region = clip(region, normal, offset);
            }
            for shape in subject.overlay(&region, OverlayRule::Intersect, FillRule::NonZero) {
                if shape.len() != 1 {
                    return Err("A grain contains a hole; use finer grains.".into());
                }
                let mut polygon: Vec<_> = shape[0]
                    .iter()
                    .map(|&p| vertex(p, &mut points, &mut ids))
                    .collect();
                polygon.dedup();
                if polygon.len() >= 3 {
                    loops.push((grain, polygon));
                }
            }
            continue;
        }
        let mut boundary = BTreeSet::new();
        for triangle in coarse {
            let mut polygon: Vec<_> = triangle
                .iter()
                .map(|&i| [outline[i].x as f64, outline[i].y as f64])
                .collect();
            for &(normal, offset) in planes {
                polygon = clip(polygon, normal, offset);
            }
            if polygon.len() < 3 {
                continue;
            }
            let mut polygon: Vec<_> = polygon
                .into_iter()
                .map(|p| vertex(p, &mut points, &mut ids))
                .collect();
            polygon.dedup();
            if polygon.first() == polygon.last() {
                polygon.pop();
            }
            if polygon.len() < 3 {
                continue;
            }
            let area: f64 = (1..polygon.len() - 1)
                .map(|i| {
                    cross(
                        points[polygon[0]],
                        points[polygon[i]],
                        points[polygon[i + 1]],
                    )
                })
                .sum();
            if area < 1e-9 {
                continue;
            }
            for i in 0..polygon.len() {
                let (a, b) = (polygon[i], polygon[(i + 1) % polygon.len()]);
                if !boundary.remove(&(b, a)) {
                    boundary.insert((a, b));
                }
            }
        }
        while let Some(&(first, next)) = boundary.first() {
            boundary.remove(&(first, next));
            let mut polygon = vec![first];
            let mut current = next;
            while current != first {
                polygon.push(current);
                let edge = boundary
                    .range((current, 0)..=(current, usize::MAX))
                    .next()
                    .copied()
                    .ok_or("Grain boundary could not be joined. Move the nearby grain point.")?;
                boundary.remove(&edge);
                current = edge.1;
                if polygon.len() > points.len() {
                    return Err("Invalid grain boundary.".into());
                }
            }
            // Remove traces of the outline triangulation before constructing
            // new, compact triangles. T junctions are inserted globally below.
            loop {
                if polygon.len() <= 3 {
                    break;
                }
                let removable = (0..polygon.len()).find(|&i| {
                    cross(
                        points[polygon[(i + polygon.len() - 1) % polygon.len()]],
                        points[polygon[i]],
                        points[polygon[(i + 1) % polygon.len()]],
                    )
                    .abs()
                        < 1e-8
                });
                if let Some(i) = removable {
                    polygon.remove(i);
                } else {
                    break;
                }
            }
            loops.push((grain, polygon));
        }
    }
    let mut used: BTreeSet<_> = loops.iter().flat_map(|(_, p)| p.iter().copied()).collect();
    if robust {
        // A union may split a straight outline edge at a support transition.
        // Keep those authored points in the conforming boundary even though
        // polygon clipping dissolved them as geometrically collinear.
        for p in outline {
            used.insert(vertex([p.x as f64, p.y as f64], &mut points, &mut ids));
        }
    }
    let mut conformed = Vec::new();
    for (grain, polygon) in loops {
        let mut conforming = Vec::new();
        for i in 0..polygon.len() {
            let a = points[polygon[i]];
            let b = points[polygon[(i + 1) % polygon.len()]];
            let length = (b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2);
            let mut splits: Vec<_> = used
                .iter()
                .filter_map(|&j| {
                    let p = points[j];
                    let t =
                        ((p[0] - a[0]) * (b[0] - a[0]) + (p[1] - a[1]) * (b[1] - a[1])) / length;
                    ((-1e-8..1.0 - 1e-8).contains(&t) && cross(a, b, p).abs() < 1e-7)
                        .then_some((t, j))
                })
                .collect();
            splits.sort_by(|a, b| a.0.total_cmp(&b.0));
            conforming.extend(splits.into_iter().map(|(_, j)| j));
        }
        conformed.push((grain, conforming));
    }
    // Collapse short shared faces globally, before triangulating. Voronoi
    // junctions clipped near the outline otherwise produce almost-hinges with
    // vanishing bending stiffness. Keeping boundary vertices on their original
    // edge preserves the authored outline and the exact covered area.
    collapse_short_edges(&mut conformed, &points, outline, 0.03);
    let mut triangles = Vec::new();
    let mut grains = Vec::new();
    for (grain, mut conforming) in conformed {
        if conforming.len() < 3 {
            continue;
        }
        let convex = (0..conforming.len()).all(|i| {
            cross(
                points[conforming[i]],
                points[conforming[(i + 1) % conforming.len()]],
                points[conforming[(i + 2) % conforming.len()]],
            ) >= -1e-8
        });
        if convex {
            let count = conforming.len() as f64;
            let center = [
                conforming.iter().map(|&i| points[i][0]).sum::<f64>() / count,
                conforming.iter().map(|&i| points[i][1]).sum::<f64>() / count,
            ];
            let center = vertex(center, &mut points, &mut ids);
            for i in 0..conforming.len() {
                triangles.push([
                    conforming[i],
                    conforming[(i + 1) % conforming.len()],
                    center,
                ]);
                grains.push(grain);
            }
        } else {
            while conforming.len() > 3 {
                let ear = (0..conforming.len())
                    .filter(|&i| {
                        let t = [
                            conforming[(i + conforming.len() - 1) % conforming.len()],
                            conforming[i],
                            conforming[(i + 1) % conforming.len()],
                        ];
                        let [a, b, c] = t.map(|i| points[i]);
                        cross(a, b, c) > 1e-9
                            && !conforming.iter().any(|&j| {
                                !t.contains(&j)
                                    && cross(a, b, points[j]) >= -1e-9
                                    && cross(b, c, points[j]) >= -1e-9
                                    && cross(c, a, points[j]) >= -1e-9
                            })
                    })
                    .max_by(|&i, &j| {
                        let quality = |i: usize| {
                            let [a, b, c] = [
                                conforming[(i + conforming.len() - 1) % conforming.len()],
                                conforming[i],
                                conforming[(i + 1) % conforming.len()],
                            ]
                            .map(|i| points[i]);
                            let d = |a: P, b: P| (b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2);
                            cross(a, b, c) / d(a, b).max(d(b, c)).max(d(c, a))
                        };
                        quality(i).total_cmp(&quality(j))
                    })
                    .ok_or("Grain shape is too thin. Move a grain point or increase grain size.")?;
                triangles.push([
                    conforming[(ear + conforming.len() - 1) % conforming.len()],
                    conforming[ear],
                    conforming[(ear + 1) % conforming.len()],
                ]);
                grains.push(grain);
                conforming.remove(ear);
            }
            triangles.push([conforming[0], conforming[1], conforming[2]]);
            grains.push(grain);
        }
    }
    // Refuse a topology-changing collapse if it introduced a gap, inverted
    // cell or nonmanifold seam. The user can move a point or change the seed.
    let mut edges = BTreeMap::<(usize, usize), usize>::new();
    let mut area = 0.0;
    for t in &triangles {
        let [a, b, c] = t.map(|i| points[i]);
        let signed = cross(a, b, c) * 0.5;
        if signed <= 1e-10 {
            return Err("Grain has a degenerate cell. Move a point or change the seed.".into());
        }
        area += signed;
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            *edges.entry((a.min(b), a.max(b))).or_default() += 1;
        }
    }
    let original: f64 = coarse
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| outline[i].to_array().map(f64::from));
            cross(a, b, c) * 0.5
        })
        .sum();
    if (area - original).abs() > original * 1e-6 {
        return Err("Grains do not cover the outline. Move a point or change the seed.".into());
    }
    for ((a, b), count) in edges {
        let on_outline = (0..outline.len()).any(|i| {
            let start = outline[i].to_array().map(f64::from);
            let end = outline[(i + 1) % outline.len()].to_array().map(f64::from);
            [a, b].into_iter().all(|j| {
                cross(start, end, points[j]).abs() < 1e-7
                    && (points[j][0] - start[0]) * (points[j][0] - end[0])
                        + (points[j][1] - start[1]) * (points[j][1] - end[1])
                        <= 1e-8
            })
        });
        if count > 2 || (count == 1 && !on_outline) {
            return Err("Grain seam could not be joined. Move a point or change the seed.".into());
        }
    }
    if triangles.is_empty() {
        return Err("The layout does not cover the object.".into());
    }
    Ok((
        points
            .into_iter()
            .map(|p| Vec2::new(p[0] as f32, p[1] as f32))
            .collect(),
        triangles,
        grains,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn layouts_cover_concave_outlines_and_share_every_internal_edge() {
        for seed in 0..16 {
            for preset in ObjectPreset::ALL {
                for mode in [GrainMode::Bricks, GrainMode::Stones] {
                    let mut asset = ObjectAsset::preset(preset);
                    asset.refinement = 0;
                    asset.regenerate_grains(mode, [0.7, 0.5], seed);
                    let mesh = asset
                        .mesh()
                        .unwrap_or_else(|e| panic!("{} {mode:?} seed {seed}: {e}", preset.label()));
                    let area: f32 = mesh
                        .triangles
                        .iter()
                        .map(|t| {
                            let [a, b, c] = t.map(|i| mesh.vertices[i]);
                            (b - a).perp_dot(c - a) * 0.5
                        })
                        .sum();
                    assert!(
                        (area - mesh.area).abs() < 1e-4,
                        "{} {mode:?}: {area} != {}",
                        preset.label(),
                        mesh.area
                    );
                    let mut edges = BTreeMap::new();
                    for t in &mesh.triangles {
                        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                            *edges.entry((a.min(b), a.max(b))).or_insert(0) += 1;
                        }
                    }
                    for ((a, b), count) in edges {
                        assert!(count <= 2);
                        if count == 1 {
                            let point = (mesh.vertices[a] + mesh.vertices[b]) * 0.5;
                            assert!(
                                (0..asset.outline.len()).any(|i| {
                                    let a = Vec2::from(asset.outline[i]);
                                    let b =
                                        Vec2::from(asset.outline[(i + 1) % asset.outline.len()]);
                                    (b - a).perp_dot(point - a).abs() < 1e-5
                                        && (point - a).dot(point - b) <= 1e-5
                                }),
                                "unjoined seam at {point:?}"
                            );
                        }
                    }
                    assert_eq!(
                        asset,
                        ObjectAsset::from_json(&asset.to_json().unwrap()).unwrap()
                    );
                }
            }
        }
    }

    #[test]
    fn wall_grains_are_compact_and_sites_are_editable() {
        let wall = ObjectAsset::workshop_preset(ObjectPreset::Wall);
        let mesh = wall.mesh().unwrap();
        for t in &mesh.triangles {
            let p = t.map(|i| mesh.vertices[i]);
            assert!(
                p[0].distance(p[1])
                    .max(p[1].distance(p[2]))
                    .max(p[2].distance(p[0]))
                    < 1.0
            );
        }
        let mut rock = ObjectAsset::workshop_preset(ObjectPreset::Rock);
        let before = rock.mesh().unwrap();
        rock.layout.as_mut().unwrap().sites[0][0] += 0.04;
        let after = rock.mesh().unwrap();
        assert_ne!(before.vertices, after.vertices);
        assert!((before.area - after.area).abs() < 1e-5);
    }
}
