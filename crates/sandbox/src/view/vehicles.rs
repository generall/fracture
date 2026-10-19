//! Distinct silhouettes, reused by the deployment cards and battlefield.
use super::*;
use sim::VehicleKind;

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw(
    kind: VehicleKind,
    center: Vec2,
    angle: f32,
    aim: f32,
    scale: f32,
    color: Color,
    hull: Color,
    track_travel: [f32; 2],
) {
    let size = kind.size();
    let transform =
        |p: Vec2| center + Vec2::from_angle(angle).rotate(p * vec2(size.x, size.y)) * scale;
    let polygon = |points: &[Vec2], color| {
        masked_polygon(
            &points.iter().copied().map(transform).collect::<Vec<_>>(),
            color,
        )
    };
    let rect = |x: f32, y: f32, w: f32, h: f32, color| {
        polygon(
            &[
                vec2(x, y),
                vec2(x + w, y),
                vec2(x + w, y + h),
                vec2(x, y + h),
            ],
            color,
        )
    };
    match kind {
        VehicleKind::Scout => {
            for x in [-0.67, 0.55] {
                for y in [-0.8, 0.5] {
                    rect(x, y, 0.42, 0.30, color);
                }
            }
            polygon(
                &[
                    vec2(-0.9, -0.4),
                    vec2(0.6, -0.4),
                    vec2(0.96, -0.26),
                    vec2(0.96, 0.26),
                    vec2(0.6, 0.4),
                    vec2(-0.9, 0.4),
                ],
                hull,
            );
            rect(-0.2, -0.35, 0.47, 0.7, color);
            line(
                transform(vec2(0.3, -0.29)),
                transform(vec2(0.3, 0.29)),
                2.5,
                color,
            );
            for y in [-0.25, 0.25] {
                let p = transform(vec2(0.83, y));
                draw_circle(p.x, p.y, 2.0, color);
            }
            let direction = Vec2::from_angle(aim);
            line(center, center + direction * scale * 1.48, 2.0, color);
            draw_circle_lines(center.x, center.y, scale * 0.19, 1.5, color);
        }
        VehicleKind::Tank => {
            for y in [-0.65, 0.65] {
                polygon(&tank_parts::track_outline(y), color);
            }
            polygon(&tank_parts::hull_outline(), hull);
            let turret = |p: Vec2| center + Vec2::from_angle(aim).rotate(p) * scale;
            masked_polygon(&tank_parts::turret_outline().map(turret), color);
            line(turret(vec2(0.4, 0.0)), turret(vec2(1.58, 0.0)), 5.0, color);
        }
        VehicleKind::Mortar => {
            for (side, y) in [-0.8, 0.49].into_iter().enumerate() {
                rect(-1.08, y, 2.16, 0.31, color);
                let offset = (track_travel[side] / size.x).rem_euclid(0.18);
                for i in 0..13 {
                    let x = -1.08 + i as f32 * 0.18 + offset;
                    if x > 1.06 {
                        continue;
                    }
                    line(
                        transform(vec2(x, y)),
                        transform(vec2(x, y + 0.31)),
                        1.0,
                        color,
                    );
                }
            }
            polygon(
                &[
                    vec2(-0.95, -0.46),
                    vec2(0.78, -0.46),
                    vec2(0.96, -0.25),
                    vec2(0.96, 0.25),
                    vec2(0.78, 0.46),
                    vec2(-0.95, 0.46),
                ],
                hull,
            );
            rect(-0.65, -0.36, 0.75, 0.72, color);
            // Open fighting compartment and a thick fixed forward mortar tube.
            let well = transform(vec2(-0.23, 0.0));
            draw_circle_lines(well.x, well.y, scale * 0.26, 2.0, color);
            rect(-0.15, -0.15, 1.05, 0.30, color);
            line(
                transform(vec2(0.8, -0.19)),
                transform(vec2(0.8, 0.19)),
                3.0,
                hull,
            );
            for y in [-0.38, 0.38] {
                line(
                    transform(vec2(-0.8, y)),
                    transform(vec2(-0.4, y)),
                    2.0,
                    color,
                );
            }
        }
    }
}

pub(crate) fn turret(center: Vec2, facing: f32, aim: f32, scale: f32, color: Color, alive: bool) {
    let transform = |p: Vec2| center + Vec2::from_angle(facing).rotate(p) * scale;
    let base = [
        vec2(-1.05, -0.5),
        vec2(-0.75, -0.8),
        vec2(0.75, -0.8),
        vec2(1.05, -0.5),
        vec2(1.05, 0.5),
        vec2(0.75, 0.8),
        vec2(-0.75, 0.8),
        vec2(-1.05, 0.5),
    ];
    masked_polygon(
        &base.map(transform),
        tint(color, if alive { 0.6 } else { 0.3 }),
    );
    for x in [-0.75, 0.75] {
        for y in [-0.5, 0.5] {
            let p = transform(vec2(x, y));
            draw_circle(p.x, p.y, scale * 0.07, color);
        }
    }
    if alive {
        let turn = |p: Vec2| center + Vec2::from_angle(aim).rotate(p) * scale;
        draw_circle_lines(center.x, center.y, scale * 0.58, 2.0, color);
        masked_polygon(&tank_parts::turret_outline().map(turn), color);
        masked_polygon(
            &[
                vec2(0.4, -0.10),
                vec2(1.6, -0.10),
                vec2(1.6, 0.10),
                vec2(0.4, 0.10),
            ]
            .map(turn),
            color,
        );
    } else {
        for sign in [-1.0, 1.0] {
            line(
                transform(vec2(-0.45, sign * 0.4)),
                transform(vec2(0.45, -sign * 0.4)),
                2.0,
                color,
            );
        }
    }
}
