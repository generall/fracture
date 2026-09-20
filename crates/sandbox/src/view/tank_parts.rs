//! The same tank silhouettes and surface details, clipped onto moving wreck pieces.
use super::*;

pub(crate) fn hull_outline() -> [Vec2; 8] {
    [
        vec2(-0.95, -0.37),
        vec2(-0.80, -0.48),
        vec2(0.65, -0.48),
        vec2(0.95, -0.25),
        vec2(0.95, 0.25),
        vec2(0.65, 0.48),
        vec2(-0.80, 0.48),
        vec2(-0.95, 0.37),
    ]
}
pub(crate) fn track_outline(y: f32) -> [Vec2; 8] {
    [
        vec2(-1.08, y - 0.12),
        vec2(-0.94, y - 0.17),
        vec2(0.94, y - 0.17),
        vec2(1.08, y - 0.12),
        vec2(1.08, y + 0.12),
        vec2(0.94, y + 0.17),
        vec2(-0.94, y + 0.17),
        vec2(-1.08, y + 0.12),
    ]
}
pub(crate) fn turret_outline() -> [Vec2; 8] {
    [
        vec2(-0.42, -0.23),
        vec2(-0.20, -0.36),
        vec2(0.29, -0.32),
        vec2(0.46, -0.14),
        vec2(0.46, 0.14),
        vec2(0.29, 0.32),
        vec2(-0.20, 0.36),
        vec2(-0.42, 0.23),
    ]
}
fn clipped(mut polygon: Vec<Vec2>, half: Vec2) -> Vec<Vec2> {
    for normal in [Vec2::X, -Vec2::X, Vec2::Y, -Vec2::Y] {
        let extent = normal.abs().dot(half);
        let mut result = Vec::new();
        for i in 0..polygon.len() {
            let (a, b) = (polygon[i], polygon[(i + 1) % polygon.len()]);
            let (da, db) = (extent - a.dot(normal), extent - b.dot(normal));
            if da >= 0.0 {
                result.push(a);
            }
            if (da >= 0.0) != (db >= 0.0) {
                result.push(a.lerp(b, da / (da - db)));
            }
        }
        polygon = result;
    }
    polygon
}
pub(super) fn draw_fragment(body: &sim::BodyView, view: &Viewport, alpha: f32, color: Color) {
    let Some(part) = body.tank_fragment else {
        return;
    };
    let Shape::Box { half } = body.shape else {
        return;
    };
    let half = v(half);
    let offset = v(part.local_offset);
    let center = view.screen(v(body.previous_position).lerp(v(body.position), alpha));
    let rotation = Vec2::from_angle(interpolate_angle(body.previous_angle, body.angle, alpha));
    let transform = |p: Vec2| center + rotation.rotate(p) * view.scale;
    let track = part.part >= 4;
    let source = if track {
        track_outline(offset.y)
    } else {
        hull_outline()
    };
    let points: Vec<_> = clipped(source.iter().map(|&p| p - offset).collect(), half)
        .into_iter()
        .map(transform)
        .collect();
    for i in 1..points.len().saturating_sub(1) {
        draw_triangle(
            points[0],
            points[i],
            points[i + 1],
            tint(PAPER, body.opacity),
        );
    }
    let edge = if track {
        color
    } else {
        tint(INK, body.opacity)
    };
    outline(&points, 1.5, edge, false);
    // Surface markings remain in each piece's local frame, including the frozen tread phase.
    let vertical = |x: f32, y0: f32, y1: f32, color: Color| {
        let x = x - offset.x;
        let lo = (y0 - offset.y).max(-half.y);
        let hi = (y1 - offset.y).min(half.y);
        if x >= -half.x && x <= half.x && hi > lo {
            line(transform(vec2(x, lo)), transform(vec2(x, hi)), 1.2, color);
        }
    };
    if track {
        for i in 0..12 {
            vertical(
                -0.99 + i as f32 * 0.18 + part.tread_phase,
                offset.y - 0.13,
                offset.y + 0.13,
                tint(color, 0.85),
            );
        }
    } else {
        for x in [-0.72, -0.60, -0.48] {
            vertical(x, -0.28, 0.28, tint(MUTED, body.opacity));
        }
        // Short exposed metal edges give the separated plate a readable thickness.
        let a = transform(vec2(-half.x + 0.06, -half.y + 0.04));
        let b = transform(vec2(half.x - 0.08, -half.y + 0.04));
        line(a, b, 1.0, tint(color, 0.35));
    }
}

pub(crate) fn draw_turret(
    view: &Viewport,
    ground: Vec2,
    angle: f32,
    height: f32,
    opacity: f32,
    color: Color,
) {
    let ground = view.screen(ground);
    let lift = height * view.scale * 0.55;
    draw_ellipse(
        ground.x,
        ground.y,
        view.scale * 0.65,
        view.scale * 0.3,
        angle.to_degrees(),
        tint(BLACK, opacity * 0.4),
    );
    let center = ground - vec2(0.0, lift);
    let rotation = Vec2::from_angle(angle);
    let scale = view.scale * (1.0 + height * 0.06);
    let transform = |p: Vec2| center + rotation.rotate(p) * scale;
    for points in [
        vec![
            vec2(0.1, -0.105),
            vec2(1.58, -0.105),
            vec2(1.58, 0.105),
            vec2(0.1, 0.105),
        ],
        turret_outline().to_vec(),
    ] {
        let points: Vec<_> = points.into_iter().map(transform).collect();
        for i in 1..points.len() - 1 {
            draw_triangle(points[0], points[i], points[i + 1], tint(PAPER, opacity));
        }
        outline(&points, 1.7, tint(color, opacity), true);
    }
    draw_circle_lines(center.x, center.y, scale * 0.13, 1.3, tint(INK, opacity));
}
