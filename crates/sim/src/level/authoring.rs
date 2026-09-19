//! Geometry operations shared by pointer tools, numeric fields and clipboard actions.
use super::*;
use i_overlay::{
    core::{fill_rule::FillRule, overlay_rule::OverlayRule},
    float::single::SingleFloatOverlay,
};

fn bounds(items: impl Iterator<Item = Vec2>) -> (Vec2, Vec2) {
    items.fold(
        (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY)),
        |(lo, hi), p| (lo.min(p), hi.max(p)),
    )
}

pub fn clamp_translation(items: &[LevelItem], selected: &[usize], size: Vec2, delta: Vec2) -> Vec2 {
    if selected.is_empty() {
        return Vec2::ZERO;
    }
    let (lo, hi) = bounds(selected.iter().flat_map(|&i| items[i].world_outline()));
    let half = size * 0.5 - Vec2::splat(0.201);
    delta.clamp((-half - lo).min(Vec2::ZERO), (half - hi).max(Vec2::ZERO))
}

impl LevelItem {
    pub fn is_circular(&self) -> bool {
        matches!(self, Self::Object { asset, .. } if asset.is_circular())
    }

    /// Preserve the requested shape up to the map's physical limits, then slide
    /// the complete footprint (including bridge booths) back inside the border.
    pub fn fit_inside(&mut self, size: Vec2) -> Result<(), String> {
        if matches!(self, Self::Bridge { .. }) {
            self.resize(self.dimensions())?;
        }
        let half = size * 0.5 - Vec2::splat(0.201);
        for _ in 0..5 {
            let (lo, hi) = bounds(self.world_outline().into_iter());
            let ratio = (half * 2.0 / (hi - lo)).min_element().min(1.0);
            if ratio >= 0.999999 {
                break;
            }
            self.resize(self.dimensions() * (ratio * 0.9999))?;
        }
        let (lo, hi) = bounds(self.world_outline().into_iter());
        if (hi - lo).cmpgt(half * 2.0 + Vec2::splat(0.001)).any() {
            return Err("The map is too small for this footprint.".into());
        }
        let delta = (-half - lo).max(Vec2::ZERO) + (half - hi).min(Vec2::ZERO);
        self.translate(delta);
        Ok(())
    }

    pub fn resize_inside(&mut self, dimensions: Vec2, size: Vec2) -> Result<(), String> {
        self.resize(dimensions)?;
        self.fit_inside(size)
    }

    pub fn can_join(&self, other: &Self) -> bool {
        // Joining raw outlines would restore erased material.
        if !self.cutouts().is_empty() || !other.cutouts().is_empty() {
            return false;
        }
        match (self, other) {
            (Self::Solid { .. }, Self::Solid { .. }) => true,
            (Self::Object { asset: a, .. }, Self::Object { asset: b, .. }) => {
                if a.is_circular()
                    || b.is_circular()
                    || a.components.plasticity.is_some()
                    || b.components.plasticity.is_some()
                {
                    return false;
                }
                let mut a = (**a).clone();
                let mut b = (**b).clone();
                for asset in [&mut a, &mut b] {
                    asset.outline.clear();
                    asset.layout = None;
                    asset.name.clear();
                    asset.pinned_edge = None;
                    asset.additional_pinned_edges.clear();
                }
                a == b
            }
            _ => false,
        }
    }
}

/// Union actual occupied material, then generate one shared grain graph.
/// Holes require a different asset schema; never fill them silently.
pub fn merge_items(items: &[LevelItem]) -> Result<LevelItem, String> {
    let first = items
        .first()
        .ok_or("Select connected rocks or walls to merge.")?;
    if items.len() < 2 || !items.iter().all(|i| first.can_join(i)) {
        return Err("Merge needs rocks or walls with matching material properties.".into());
    }
    let polygon = |item: &LevelItem| -> Vec<[f64; 2]> {
        let mut p: Vec<_> = item
            .world_outline()
            .iter()
            .map(|p| [p.x as f64, p.y as f64])
            .collect();
        let area: f64 = (0..p.len())
            .map(|i| p[i][0] * p[(i + 1) % p.len()][1] - p[i][1] * p[(i + 1) % p.len()][0])
            .sum();
        if area < 0.0 {
            p.reverse();
        }
        p
    };
    let mut shapes = vec![vec![polygon(first)]];
    for item in &items[1..] {
        shapes = shapes.overlay(&polygon(item), OverlayRule::Union, FillRule::NonZero);
    }
    if shapes.len() != 1 {
        return Err("Move the shapes together so their edges touch or overlap.".into());
    }
    if shapes[0].len() != 1 {
        return Err("This merge encloses a hole. Keep an opening or merge smaller groups.".into());
    }
    let points: Vec<_> = shapes
        .remove(0)
        .remove(0)
        .into_iter()
        .map(|p| Vec2::new(p[0] as f32, p[1] as f32))
        .collect();
    // Keep support transitions after the union dissolves collinear edges.
    let pins: Vec<_> = items
        .iter()
        .flat_map(|item| {
            let p = item.world_outline();
            if let LevelItem::Object { asset, .. } = item {
                asset
                    .pinned_edges()
                    .map(|i| (p[i], p[(i + 1) % p.len()]))
                    .collect::<Vec<_>>()
            } else {
                vec![]
            }
        })
        .collect();
    let points = split_support_transitions(&points, &pins);
    let (lo, hi) = bounds(points.iter().copied());
    let center = (lo + hi) * 0.5;
    let outline: Vec<_> = points.iter().map(|p| (*p - center).to_array()).collect();
    let result = match first {
        LevelItem::Solid { .. } => LevelItem::Solid {
            cutouts: vec![],
            outline,
            position: center.to_array(),
            angle: 0.0,
        },
        LevelItem::Object { asset, .. } => {
            let mut asset = asset.clone();
            asset.outline = outline;
            let pinned: Vec<_> = (0..points.len())
                .filter(|&i| {
                    pins.iter().any(|&(a, b)| {
                        let mid = (points[i] + points[(i + 1) % points.len()]) * 0.5;
                        let t = (mid - a).dot(b - a) / (b - a).length_squared();
                        (0.0..=1.0).contains(&t) && (a + (b - a) * t).distance(mid) < 0.001
                    })
                })
                .collect();
            asset.pinned_edge = pinned.first().copied();
            asset.additional_pinned_edges = pinned.into_iter().skip(1).collect();
            asset.fit_grain_budget();
            asset.level_mesh()?;
            LevelItem::Object {
                cutouts: vec![],
                asset,
                position: center.to_array(),
                angle: 0.0,
            }
        }
        _ => unreachable!(),
    };
    if let LevelItem::Solid { outline, .. } = &result {
        solid_asset(outline).level_mesh()?;
    }
    Ok(result)
}

fn split_support_transitions(points: &[Vec2], pins: &[(Vec2, Vec2)]) -> Vec<Vec2> {
    let mut result = Vec::new();
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        let edge = b - a;
        let length = edge.length();
        result.push(a);
        let mut intervals: Vec<(f32, f32)> = pins
            .iter()
            .filter_map(|&(p, q)| {
                if (p - a).perp_dot(edge).abs() / length > 0.001
                    || (q - a).perp_dot(edge).abs() / length > 0.001
                {
                    return None;
                }
                let p = (p - a).dot(edge) / edge.length_squared();
                let q = (q - a).dot(edge) / edge.length_squared();
                let (lo, hi) = (p.min(q).max(0.0), p.max(q).min(1.0));
                (hi > lo).then_some((lo, hi))
            })
            .collect();
        intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut joined: Vec<(f32, f32)> = Vec::new();
        for (lo, hi) in intervals {
            if let Some(last) = joined.last_mut()
                && lo <= last.1 + 0.001 / length
            {
                last.1 = last.1.max(hi);
            } else {
                joined.push((lo, hi));
            }
        }
        for t in joined.into_iter().flat_map(|(lo, hi)| [lo, hi]) {
            let p = a + edge * t;
            if p.distance(*result.last().unwrap()) >= 0.0501 && p.distance(b) >= 0.0501 {
                result.push(p);
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn object(preset: LibraryPreset, position: [f32; 2]) -> LevelItem {
        LevelItem::Object {
            cutouts: vec![],
            asset: Box::new(preset.asset()),
            position,
            angle: 0.0,
        }
    }
    #[test]
    fn barrels_keep_a_circle_and_reject_edited_noncircles() {
        for preset in [LibraryPreset::Barrel, LibraryPreset::ExplosiveBarrel] {
            let mut item = object(preset, [0., 0.]);
            item.resize(Vec2::new(8., 1.3)).unwrap();
            assert!((item.dimensions() - Vec2::splat(8.)).length() < 0.001);
            item.reshape(rectangle(Vec2::new(1., 2.)));
            assert!(item.is_circular());
            let LevelItem::Object { asset, .. } = &mut item else {
                unreachable!()
            };
            assert!(asset.level_mesh().is_ok());
            asset.name = "Renamed barrel".into();
            asset.outline[3][1] *= 0.5;
            assert!(asset.level_mesh().is_err());
        }
    }
    #[test]
    fn large_rocks_and_walls_coarsen_automatically() {
        for preset in [
            LibraryPreset::Rock,
            LibraryPreset::Wall,
            LibraryPreset::Fence,
        ] {
            let mut item = object(preset, [0., 0.]);
            item.resize(Vec2::new(180., 60.)).unwrap();
            let LevelItem::Object { asset, .. } = item else {
                unreachable!()
            };
            let mesh = asset.level_mesh().unwrap();
            assert!(
                mesh.triangles.len() < 1200,
                "{} cells",
                mesh.triangles.len()
            );
            assert!(mesh.area > 5000.);
            assert!(asset.layout.unwrap().size[0] > 4.);
        }
    }
    #[test]
    fn transforms_clamp_whole_groups_and_rotated_footprints() {
        let items = vec![
            object(LibraryPreset::Barrel, [-5., 0.]),
            object(LibraryPreset::Barrel, [5., 0.]),
        ];
        let delta = clamp_translation(&items, &[0, 1], Vec2::new(24., 16.), Vec2::new(100., 100.));
        assert!(delta.x > 6. && delta.x < 6.2);
        assert!(delta.y > 6. && delta.y < 7.2);
        let mut wall = object(LibraryPreset::Wall, [10., 0.]);
        if let LevelItem::Object { angle, .. } = &mut wall {
            *angle = 0.7;
        }
        wall.resize_inside(Vec2::new(300., 2.), Vec2::new(48., 30.))
            .unwrap();
        assert!(wall.dimensions().x > 35.);
        for p in wall.world_outline() {
            assert!(p.abs().cmple(Vec2::new(23.8, 14.8)).all());
        }
    }
    #[test]
    fn union_is_one_material_region_with_no_overlap_mass() {
        let mut a = object(LibraryPreset::Wall, [-1., 0.]);
        a.resize(Vec2::new(4., 2.)).unwrap();
        let mut b = object(LibraryPreset::Wall, [1., 0.]);
        b.resize(Vec2::new(4., 2.)).unwrap();
        let merged = merge_items(&[a, b]).unwrap();
        let LevelItem::Object { asset, .. } = merged else {
            unreachable!()
        };
        let mesh = asset.level_mesh().unwrap();
        assert!((mesh.area - 12.).abs() < 0.001);
        assert_eq!(asset.outline.len(), 4);
    }
    #[test]
    fn unions_preserve_concavity_and_reject_disconnected_shapes_and_holes() {
        let solid = |x, y, w, h| LevelItem::Solid {
            cutouts: vec![],
            outline: rectangle(Vec2::new(w, h) * 0.5),
            position: [x, y],
            angle: 0.,
        };
        let a = solid(0., 0., 4., 2.);
        let b = solid(1., 1., 2., 4.);
        let merged = merge_items(&[a.clone(), b]).unwrap();
        assert_eq!(merged.local_outline().len(), 6);
        assert!(merge_items(&[a, solid(10., 0., 2., 2.)]).is_err());
        assert!(
            merge_items(&[
                solid(0., -2., 6., 2.),
                solid(0., 2., 6., 2.),
                solid(-2., 0., 2., 6.),
                solid(2., 0., 2., 6.)
            ])
            .unwrap_err()
            .contains("hole")
        );
    }
    #[test]
    fn merge_preserves_a_support_on_only_part_of_a_straight_edge() {
        let mut a = object(LibraryPreset::Wall, [-3., 0.]);
        a.resize(Vec2::new(4., 2.)).unwrap();
        let mut b = object(LibraryPreset::Wall, [2., 0.]);
        b.resize(Vec2::new(8., 2.)).unwrap();
        if let LevelItem::Object { asset, .. } = &mut b {
            asset.pinned_edge = None;
        }
        let merged = merge_items(&[a, b]).unwrap();
        let LevelItem::Object { asset, .. } = merged else {
            unreachable!()
        };
        let length: f32 = asset
            .pinned_edges()
            .map(|i| {
                Vec2::from(asset.outline[i])
                    .distance(Vec2::from(asset.outline[(i + 1) % asset.outline.len()]))
            })
            .sum();
        assert!((length - 4.).abs() < 0.001);
    }
    #[test]
    fn bridge_grid_scales_and_retains_both_booths() {
        for vertical in [false, true] {
            for size in [[7.2, 3.6], [50., 10.], [180., 80.]] {
                let size = Vec2::from(size);
                let size = if vertical {
                    Vec2::new(size.y, size.x)
                } else {
                    size
                };
                let mesh = bridge_mesh(size, vertical).unwrap();
                let area: f32 = mesh
                    .triangles
                    .iter()
                    .map(|t| {
                        let [a, b, c] = t.map(|i| mesh.vertices[i]);
                        (b - a).perp_dot(c - a) * 0.5
                    })
                    .sum();
                assert!((area - mesh.area).abs() < 0.02, "{area} != {}", mesh.area);
                assert!(mesh.triangles.len() < 300);
                let booth_cells = mesh
                    .triangles
                    .iter()
                    .filter(|t| {
                        t.iter().all(|&i| {
                            if vertical {
                                mesh.vertices[i].x <= -size.x * 0.5
                            } else {
                                mesh.vertices[i].y >= size.y * 0.5
                            }
                        })
                    })
                    .count();
                assert_eq!(booth_cells, 16);
            }
        }
        let mut bridge = LevelItem::Bridge {
            min: [-95., -90.],
            max: [95., 90.],
            vertical: true,
        };
        bridge.fit_inside(Vec2::new(48., 30.)).unwrap();
        for p in bridge.world_outline() {
            assert!(p.abs().cmple(Vec2::new(23.8, 14.8)).all());
        }
    }
}
