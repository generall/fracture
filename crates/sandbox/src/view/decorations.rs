//! The same cosmetic prop renderer is used in the editor, live play and fog memories.
use super::*;
use sim::{DecorationKind, DecorationView};

pub(crate) fn draw(prop: &DecorationView, view: &Viewport, opacity: f32) {
    let rotation = Vec2::from_angle(prop.angle);
    let point = |p: Vec2| view.screen(v(prop.position) + rotation.rotate(p * v(prop.size)));
    let stroke = |a: Vec2, b: Vec2, color: Color, width: f32| {
        let (a, b) = (point(a), point(b));
        draw_line(a.x, a.y, b.x, b.y, width, tint(color, opacity));
    };
    let wood = Color::from_rgba(179, 138, 89, 255);
    match prop.kind {
        DecorationKind::Shrub => {
            for i in 0..7 {
                let angle = i as f32 * 2.399963;
                let offset = Vec2::from_angle(angle) * if i == 0 { 0. } else { 0.27 };
                let p = point(offset);
                let r = prop.size.min_element() * view.scale * 0.23;
                draw_circle(
                    p.x,
                    p.y,
                    r,
                    tint(Color::from_rgba(70, 105, 62, 180), opacity),
                );
                draw_circle_lines(
                    p.x,
                    p.y,
                    r,
                    1.,
                    tint(Color::from_rgba(116, 153, 89, 255), opacity),
                );
            }
        }
        DecorationKind::Crate => {
            let points = [
                vec2(-0.5, -0.5),
                vec2(0.5, -0.5),
                vec2(0.5, 0.5),
                vec2(-0.5, 0.5),
            ];
            let p = points.map(point);
            draw_triangle(
                p[0],
                p[1],
                p[2],
                tint(Color::from_rgba(99, 73, 44, 150), opacity),
            );
            draw_triangle(
                p[0],
                p[2],
                p[3],
                tint(Color::from_rgba(99, 73, 44, 150), opacity),
            );
            for i in 0..4 {
                stroke(points[i], points[(i + 1) % 4], wood, 1.5);
            }
            stroke(points[0], points[2], wood, 1.);
            stroke(points[1], points[3], wood, 1.);
            stroke(vec2(-0.33, -0.5), vec2(-0.33, 0.5), wood, 1.);
            stroke(vec2(0.33, -0.5), vec2(0.33, 0.5), wood, 1.);
        }
        DecorationKind::Timber => {
            for i in 0..4 {
                let y = -0.36 + i as f32 * 0.24;
                stroke(
                    vec2(-0.43, y),
                    vec2(0.43, y),
                    Color::from_rgba(92, 68, 43, 255),
                    (prop.size.y * view.scale * 0.2).max(1.),
                );
                stroke(vec2(-0.43, y - 0.07), vec2(0.43, y - 0.07), wood, 1.);
                for x in [-0.43, 0.43] {
                    let p = point(vec2(x, y));
                    draw_circle_lines(
                        p.x,
                        p.y,
                        (prop.size.y * view.scale * 0.09).max(1.),
                        1.,
                        tint(wood, opacity),
                    );
                }
            }
        }
    }
}

pub(crate) fn burst(prop: &DecorationView, age: f32, view: &Viewport) {
    let life = (1. - age / 0.8).clamp(0., 1.);
    let color = if prop.kind == DecorationKind::Shrub {
        Color::from_rgba(124, 158, 86, 255)
    } else {
        Color::from_rgba(188, 151, 104, 255)
    };
    let center = view.screen(v(prop.position));
    draw_circle(
        center.x,
        center.y,
        (0.2 + age * 1.1) * view.scale,
        tint(Color::from_rgba(163, 150, 121, 60), life),
    );
    for i in 0..12 {
        let angle = i as f32 * 2.399963 + prop.id as f32;
        let direction = Vec2::from_angle(angle);
        let distance = age * (1.2 + (i % 4) as f32 * 0.4);
        let p = view.screen(v(prop.position) + direction * distance);
        let r = view.scale * 0.1 * life;
        let c = tint(color, life);
        if prop.kind == DecorationKind::Shrub {
            draw_circle(p.x, p.y, r, c);
        } else {
            draw_line(
                p.x,
                p.y,
                p.x + direction.y * r * 2.,
                p.y - direction.x * r * 2.,
                1.5 * life,
                c,
            );
        }
    }
}
