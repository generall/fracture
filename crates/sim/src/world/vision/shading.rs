//! Ordinary triangle geometry for the area outside overlapping sight regions.
use super::*;
use i_overlay::{
    core::{fill_rule::FillRule, overlay_rule::OverlayRule},
    float::single::SingleFloatOverlay,
};

impl Vision {
    pub fn shadow_triangles(&self, min: Vector, max: Vector) -> Vec<[Vector; 3]> {
        let rect = vec![
            [min.x as f64, min.y as f64],
            [max.x as f64, min.y as f64],
            [max.x as f64, max.y as f64],
            [min.x as f64, max.y as f64],
        ];
        let sight: Vec<Vec<[f64; 2]>> = self
            .regions()
            .filter(|r| r.boundary.len() >= 3)
            .map(|r| {
                r.boundary
                    .iter()
                    .map(|p| [p.x as f64, p.y as f64])
                    .collect()
            })
            .collect();
        let shadow = rect.overlay(&sight, OverlayRule::Difference, FillRule::NonZero);
        // Sweep horizontal bands through the resulting contours. Between
        // vertices each pair of edges bounds a convex trapezoid, including
        // contours with holes. Overlapping sight is removed only once.
        let mut edges = Vec::new();
        for contour in shadow.iter().flatten() {
            for i in 0..contour.len() {
                let (mut a, mut b) = (contour[i], contour[(i + 1) % contour.len()]);
                if a[1] == b[1] {
                    continue;
                }
                if a[1] > b[1] {
                    std::mem::swap(&mut a, &mut b);
                }
                edges.push((a, b));
            }
        }
        edges.sort_by(|a, b| a.0[1].total_cmp(&b.0[1]));
        let mut levels: Vec<_> = edges.iter().flat_map(|(a, b)| [a[1], b[1]]).collect();
        levels.sort_by(f64::total_cmp);
        levels.dedup();
        let at = |i: usize, y: f64| {
            let (a, b) = edges[i];
            a[0] + (b[0] - a[0]) * ((y - a[1]) / (b[1] - a[1]))
        };
        let mut active: Vec<usize> = vec![];
        let mut next = 0;
        let mut triangles = vec![];
        for band in levels.windows(2) {
            let (lo, hi) = (band[0], band[1]);
            active.retain(|&i| edges[i].1[1] > lo);
            while next < edges.len() && edges[next].0[1] <= lo {
                if edges[next].1[1] > lo {
                    active.push(next);
                }
                next += 1;
            }
            let middle = (lo + hi) * 0.5;
            active.sort_by(|&a, &b| at(a, middle).total_cmp(&at(b, middle)));
            for pair in active.chunks_exact(2) {
                let p = |i, y| Vector::new(at(i, y) as f32, y as f32);
                let (a, b, c, d) = (
                    p(pair[0], lo),
                    p(pair[1], lo),
                    p(pair[1], hi),
                    p(pair[0], hi),
                );
                triangles.extend([[a, b, c], [a, c, d]]);
            }
        }
        triangles
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x: f32) -> Vision {
        Vision::from_boundary(
            Vector::new(x, 0.0),
            vec![
                Vector::new(x - 1.0, -1.0),
                Vector::new(x + 1.0, -1.0),
                Vector::new(x + 1.0, 1.0),
                Vector::new(x - 1.0, 1.0),
            ],
        )
    }

    #[test]
    fn shadow_geometry_excludes_union_of_overlapping_or_separate_sources() {
        for (x, visible_area) in [(1.0, 6.0), (3.0, 8.0)] {
            let mut vision = square(0.0);
            vision.additional.push(square(x));
            let triangles = vision.shadow_triangles(Vector::splat(-5.0), Vector::splat(5.0));
            let area: f32 = triangles
                .iter()
                .map(|[a, b, c]| (*b - *a).perp_dot(*c - *a).abs() * 0.5)
                .sum();
            assert!((area - (100.0 - visible_area)).abs() < 0.001);
            for triangle in triangles {
                if (triangle[1] - triangle[0])
                    .perp_dot(triangle[2] - triangle[0])
                    .abs()
                    > 1e-6
                {
                    assert!(!vision.contains(triangle.into_iter().sum::<Vector>() / 3.0));
                }
            }
        }
    }
}
