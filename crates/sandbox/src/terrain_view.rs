//! A cached rectangle arrangement: opaque fills and only exposed terrain edges.
//! Shared by the level editor and the live world, including impassable terrain precedence.
use crate::{ui::*, view::Viewport};
use macroquad::prelude::*;
use sim::{TerrainKind as Kind, TerrainPatch};
use std::cell::RefCell;

#[derive(Default)]
struct Surface {
    source: Vec<TerrainPatch>,
    fills: Vec<TerrainPatch>,
    edges: Vec<(Kind, sim::Vec2, sim::Vec2)>,
}
thread_local! { static CACHE: RefCell<Surface> = RefCell::new(Surface::default()); }

fn colors(kind: Kind) -> (Color, Color) {
    match kind {
        Kind::Road => (MUTED, Color::from_rgba(20, 29, 38, 255)),
        Kind::Sand => (
            Color::from_rgba(223, 185, 111, 255),
            Color::from_rgba(27, 25, 22, 255),
        ),
        Kind::TallGrass => (
            Color::from_rgba(105, 166, 94, 255),
            Color::from_rgba(24, 45, 30, 255),
        ),
        Kind::Elevation => (
            Color::from_rgba(190, 161, 120, 255),
            Color::from_rgba(43, 37, 30, 255),
        ),
        Kind::Cliff => (
            Color::from_rgba(158, 146, 132, 255),
            Color::from_rgba(31, 28, 27, 255),
        ),
        Kind::Water => (
            Color::from_rgba(91, 171, 227, 255),
            Color::from_rgba(13, 28, 44, 255),
        ),
        _ => (MUTED, PAPER),
    }
}
impl Surface {
    fn build(source: &[TerrainPatch]) -> Self {
        let mut result = Self {
            source: source.to_vec(),
            ..Self::default()
        };
        if source.is_empty() {
            return result;
        }
        let mut xs: Vec<_> = source.iter().flat_map(|p| [p.min.x, p.max.x]).collect();
        let mut ys: Vec<_> = source.iter().flat_map(|p| [p.min.y, p.max.y]).collect();
        xs.sort_by(f32::total_cmp);
        xs.dedup();
        ys.sort_by(f32::total_cmp);
        ys.dedup();
        let (nx, ny) = (xs.len() - 1, ys.len() - 1);
        let mut cells = vec![None; nx * ny];
        for p in source
            .iter()
            .filter(|p| !p.kind.blocks_tanks())
            .chain(source.iter().filter(|p| p.kind.blocks_tanks()))
        {
            let ix = |n: f32| xs.binary_search_by(|x| x.total_cmp(&n)).unwrap();
            let iy = |n: f32| ys.binary_search_by(|y| y.total_cmp(&n)).unwrap();
            for y in iy(p.min.y)..iy(p.max.y) {
                for x in ix(p.min.x)..ix(p.max.x) {
                    cells[y * nx + x] = Some(p.kind);
                }
            }
        }
        for y in 0..ny {
            let mut x = 0;
            while x < nx {
                let start = x;
                let kind = cells[y * nx + x];
                x += 1;
                while x < nx && cells[y * nx + x] == kind {
                    x += 1;
                }
                if let Some(kind) = kind {
                    result.fills.push(TerrainPatch {
                        kind,
                        min: sim::Vec2::new(xs[start], ys[y]),
                        max: sim::Vec2::new(xs[x], ys[y + 1]),
                    });
                }
            }
        }
        let cell = |x: isize, y: isize| {
            if x < 0 || y < 0 || x >= nx as isize || y >= ny as isize {
                None
            } else {
                cells[y as usize * nx + x as usize]
            }
        };
        for y in 0..ny {
            for x in 0..nx {
                let Some(kind) = cell(x as isize, y as isize) else {
                    continue;
                };
                if kind == Kind::Ground {
                    continue;
                }
                let a = sim::Vec2::new(xs[x], ys[y]);
                let b = sim::Vec2::new(xs[x + 1], ys[y + 1]);
                for (dx, dy, p, q) in [
                    (-1, 0, a, sim::Vec2::new(a.x, b.y)),
                    (1, 0, sim::Vec2::new(b.x, a.y), b),
                    (0, -1, a, sim::Vec2::new(b.x, a.y)),
                    (0, 1, sim::Vec2::new(a.x, b.y), b),
                ] {
                    if cell(x as isize + dx, y as isize + dy) != Some(kind) {
                        result.edges.push((kind, p, q));
                    }
                }
            }
        }
        result
    }
}

pub fn draw(patches: &[TerrainPatch], view: &Viewport) {
    CACHE.with_borrow_mut(|cache| {
        if cache.source != patches {
            *cache = Surface::build(patches);
        }
        for p in &cache.fills {
            let a = view.screen(vec2(p.min.x, p.min.y));
            let b = view.screen(vec2(p.max.x, p.max.y));
            draw_rectangle(a.x, a.y, b.x - a.x, b.y - a.y, colors(p.kind).1);
            if p.kind == Kind::Cliff {
                // World-aligned stone fissures, clipped to the composed surface.
                let spacing = 1.6_f32.max(14.0 / view.scale);
                let mut x = (p.min.x / spacing).floor() * spacing;
                while x < p.max.x {
                    let mut y = (p.min.y / spacing).floor() * spacing;
                    while y < p.max.y {
                        let points = [
                            vec2(x + spacing * 0.15, y + spacing * 0.3),
                            vec2(x + spacing * 0.4, y + spacing * 0.45),
                            vec2(x + spacing * 0.55, y + spacing * 0.8),
                            vec2(x + spacing * 0.85, y + spacing * 0.65),
                        ];
                        for pair in points.windows(2) {
                            let lo = vec2(p.min.x, p.min.y);
                            let hi = vec2(p.max.x, p.max.y);
                            let from = view.screen(pair[0].clamp(lo, hi));
                            let to = view.screen(pair[1].clamp(lo, hi));
                            draw_line(
                                from.x,
                                from.y,
                                to.x,
                                to.y,
                                1.0,
                                Color::from_rgba(72, 63, 55, 255),
                            );
                        }
                        y += spacing;
                    }
                    x += spacing;
                }
            }
            if matches!(p.kind, Kind::TallGrass | Kind::Elevation) {
                let spacing = if p.kind == Kind::Elevation {
                    2.0_f32.max(16.0 / view.scale)
                } else {
                    0.75_f32.max(8.0 / view.scale)
                };
                let mut x = (p.min.x / spacing).ceil() * spacing;
                while x < p.max.x {
                    let mut y = (p.min.y / spacing).ceil() * spacing;
                    while y < p.max.y {
                        let at = view.screen(vec2(x, y));
                        let h = (view.scale * 0.15).clamp(1.0, 5.0);
                        draw_line(at.x - h, at.y + h, at.x, at.y - h, 1.0, colors(p.kind).0);
                        if p.kind == Kind::Elevation {
                            draw_line(at.x, at.y - h, at.x + h, at.y + h, 1.0, colors(p.kind).0);
                        } else {
                            draw_line(at.x, at.y + h, at.x + h, at.y - h, 1.0, colors(p.kind).0);
                        }
                        y += spacing;
                    }
                    x += spacing;
                }
            }
        }
        // Draw blocked boundaries last so adjacent traversable terrain cannot
        // cover their stronger outline with its subdued border.
        for &(kind, a, b) in cache
            .edges
            .iter()
            .filter(|(kind, _, _)| !kind.blocks_tanks())
            .chain(
                cache
                    .edges
                    .iter()
                    .filter(|(kind, _, _)| kind.blocks_tanks()),
            )
        {
            let a = view.screen(vec2(a.x, a.y));
            let b = view.screen(vec2(b.x, b.y));
            if kind == Kind::Cliff {
                draw_line(a.x, a.y, b.x, b.y, 5.0, Color::from_rgba(65, 55, 46, 255));
            }
            let (accent, fill) = colors(kind);
            let (width, border) = if kind.blocks_tanks() {
                (1.5, accent)
            } else {
                // Keep terrain identity, but reserve high contrast for barriers.
                // An opaque blend also avoids brightening shared, repeated edges.
                (
                    1.0,
                    Color::new(
                        fill.r + (accent.r - fill.r) * 0.3,
                        fill.g + (accent.g - fill.g) * 0.3,
                        fill.b + (accent.b - fill.b) * 0.3,
                        1.0,
                    ),
                )
            };
            draw_line(a.x, a.y, b.x, b.y, width, border);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn patch(kind: Kind, a: [f32; 2], b: [f32; 2]) -> TerrainPatch {
        TerrainPatch {
            kind,
            min: a.into(),
            max: b.into(),
        }
    }
    #[test]
    fn overlap_has_no_internal_edges_or_double_fill() {
        let surface = Surface::build(&[
            patch(Kind::Sand, [0., 0.], [3., 2.]),
            patch(Kind::Sand, [2., 0.], [5., 2.]),
        ]);
        let area: f32 = surface
            .fills
            .iter()
            .map(|p| (p.max.x - p.min.x) * (p.max.y - p.min.y))
            .sum();
        assert_eq!(area, 10.);
        assert!(
            surface
                .edges
                .iter()
                .all(|(_, a, b)| a.y == b.y || a.x == 0. || a.x == 5.)
        );
    }
    #[test]
    fn gaps_win_and_dry_layers_respect_placement_order() {
        let surface = Surface::build(&[
            patch(Kind::Water, [1., 0.], [2., 3.]),
            patch(Kind::Cliff, [1.75, 0.], [3., 3.]),
            patch(Kind::Sand, [0., 0.], [3., 3.]),
            patch(Kind::Road, [0., 1.], [3., 2.]),
        ]);
        for (p, kind) in [
            ([1.5, 1.5], Kind::Water),
            ([1.9, 1.5], Kind::Cliff),
            ([2.5, 1.5], Kind::Cliff),
            ([0.5, 1.5], Kind::Road),
            ([0.5, 0.5], Kind::Sand),
        ] {
            assert_eq!(
                surface
                    .fills
                    .iter()
                    .find(|r| r.contains(p.into()))
                    .unwrap()
                    .kind,
                kind
            );
        }
    }
}
