//! Recorded-object rendering and comparison viewport geometry.
use super::*;

pub(super) fn card_view(card: Rect, compact: bool) -> Rect {
    Rect::new(
        card.x + 1.0,
        card.y + 37.0,
        card.w - 2.0,
        card.h - if compact { 105.0 } else { 112.0 },
    )
}
pub(super) fn view_scale(rect: Rect, zoom: f32) -> f32 {
    (rect.w / 18.0).min(rect.h / 8.5) * zoom
}
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_state(
    geometry: &[Geometry],
    frame: &Frame,
    rect: Rect,
    center: Vec2,
    zoom: f32,
    stress: bool,
    pixel_scale: f32,
) {
    unsafe {
        get_internal_gl().quad_gl.scissor(Some((
            (rect.x * pixel_scale) as i32,
            (rect.y * pixel_scale) as i32,
            (rect.w * pixel_scale) as i32,
            (rect.h * pixel_scale) as i32,
        )));
    }
    let scale = view_scale(rect, zoom);
    let screen = |p: Vec2| rect.center() + (p - center) * scale;
    for x in -12..=12 {
        for y in -7..=7 {
            let p = screen(vec2(x as f32, y as f32));
            draw_circle(p.x, p.y, 0.8, RULE);
        }
    }
    let shapes: BTreeMap<_, _> = geometry.iter().map(|g| (g.id, g)).collect();
    for p in &frame.poses {
        let Some(g) = shapes.get(&p.0) else {
            continue;
        };
        let rotation = Vec2::from_angle(p.2);
        let world = |q: Vec2| Vec2::from(p.1) + rotation.rotate(q);
        let mut color = match g.material {
            0 => INK,
            1 => VIOLET,
            2 => CYAN,
            3 | 5 => MUTED,
            4 => CYAN,
            _ => INK,
        };
        color = g
            .color
            .map_or(color, |[r, g, b]| Color::from_rgba(r, g, b, 255));
        if stress && g.material < 3 {
            let t = p.5.clamp(0.0, 1.0);
            color = Color::new(
                CYAN.r + (PINK.r - CYAN.r) * t,
                CYAN.g + (PINK.g - CYAN.g) * t,
                CYAN.b + (PINK.b - CYAN.b) * t,
                1.0,
            );
        }
        color.a = p.4;
        for edge in 0..g.count as usize {
            if !stress && p.3 & (1 << edge) == 0 {
                continue;
            }
            let a = screen(world(Vec2::from(g.vertices[edge])));
            let b = screen(world(Vec2::from(g.vertices[(edge + 1) % g.count as usize])));
            draw_line(
                a.x,
                a.y,
                b.x,
                b.y,
                if g.material == 4 { 1.8 } else { 1.25 },
                color,
            );
        }
        if g.material == 4 {
            for y in [-0.55, 0.55] {
                let a = screen(world(vec2(-0.9, y)));
                let b = screen(world(vec2(0.9, y)));
                draw_line(a.x, a.y, b.x, b.y, 2.5, CYAN);
            }
            let a = screen(world(vec2(0.0, 0.0)));
            let b = screen(world(vec2(1.6, 0.0)));
            draw_circle_lines(a.x, a.y, 0.32 * scale, 1.5, CYAN);
            draw_line(a.x, a.y, b.x, b.y, 2.0, CYAN);
        }
    }
    for p in &frame.shells {
        let p = screen(Vec2::from(*p));
        draw_line(p.x - 5.0, p.y, p.x + 3.0, p.y, 2.0, INK);
    }
    for (p, r) in &frame.flashes {
        let p = screen(Vec2::from(*p));
        draw_circle_lines(p.x, p.y, r * scale, 1.2, PINK);
        draw_circle_lines(p.x, p.y, r * scale * 0.7, 1.0, INK);
    }
    if let Some((a, b)) = frame.beam {
        let a = screen(Vec2::from(a));
        let b = screen(Vec2::from(b));
        draw_line(a.x, a.y, b.x, b.y, 1.5, CYAN);
        draw_circle_lines(b.x, b.y, 3.0, 1.0, INK);
    }
    unsafe {
        get_internal_gl().quad_gl.scissor(None);
    }
}
