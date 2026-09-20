//! Weapon markers and effects use the normal world renderer.
use super::*;

pub(crate) fn pickup_marker(pickup: &sim::PickupView, view: &Viewport, opacity: f32) {
    let p = view.screen(v(pickup.position));
    let ready = pickup.remaining <= 0.0;
    let r = (view.scale * 0.6).max(8.0);
    let color = tint(
        if pickup.weapon == sim::SpecialWeapon::Laser {
            CYAN
        } else {
            PINK
        },
        opacity,
    );
    draw_poly(
        p.x,
        p.y,
        6,
        r,
        30.0,
        tint(color, if ready { 0.15 } else { 0.04 }),
    );
    draw_poly_lines(
        p.x,
        p.y,
        6,
        r,
        30.0,
        1.5,
        tint(color, if ready { 0.8 } else { 0.3 }),
    );
    if ready {
        if pickup.weapon == sim::SpecialWeapon::Laser {
            let points = [
                vec2(0.15, -0.6),
                vec2(-0.3, 0.08),
                vec2(0.1, 0.08),
                vec2(-0.15, 0.6),
            ];
            for pair in points.windows(2) {
                glow(p + pair[0] * r, p + pair[1] * r, 2.0, color);
            }
        } else {
            let points = [
                vec2(0.0, -0.6),
                vec2(0.28, 0.2),
                vec2(0.0, 0.4),
                vec2(-0.28, 0.2),
            ];
            outline(&points.map(|q| p + q * r), 2.0, color, true);
            line(
                p + vec2(-0.1, 0.45) * r,
                p + vec2(-0.1, 0.65) * r,
                2.0,
                color,
            );
            line(p + vec2(0.1, 0.45) * r, p + vec2(0.1, 0.65) * r, 2.0, color);
        }
    } else {
        let progress = (1.0 - pickup.remaining / pickup.respawn_time).clamp(0.0, 1.0);
        let mut previous = p + Vec2::from_angle(-std::f32::consts::FRAC_PI_2) * r;
        for i in 1..=32 {
            let a =
                -std::f32::consts::FRAC_PI_2 + progress * std::f32::consts::TAU * i as f32 / 32.0;
            let next = p + Vec2::from_angle(a) * r;
            line(previous, next, 2.0, color);
            previous = next;
        }
    }
}

pub(super) fn laser(start: Vec2, end: Vec2, scale: f32, time: f32) {
    let delta = end - start;
    let axis = delta.normalize_or_zero();
    let side = vec2(-axis.y, axis.x);
    let width = (scale * 0.6).max(6.0);
    let pulse = 0.8 + 0.2 * (time * 41.0).sin();
    line(start, end, width * 2.3, tint(VIOLET, 0.08 * pulse));
    line(start, end, width * 1.4, tint(CYAN, 0.16 * pulse));
    line(start, end, width * 0.8, tint(CYAN, 0.65));
    line(start, end, (width * 0.28).max(2.0), INK);
    // Traveling filaments stay inside the damage footprint.
    for strand in [-1.0, 1.0] {
        let mut last = start;
        for i in 1..=48 {
            let t = i as f32 / 48.0;
            let wave = (t * 57.0 - time * 31.0 + strand).sin() * width * 0.22;
            let next = start + delta * t + side * wave * strand;
            line(last, next, 1.2, tint(INK, 0.65));
            last = next;
        }
    }
    for p in [start, end] {
        draw_circle(p.x, p.y, width * 0.9, tint(CYAN, 0.10));
        draw_circle(p.x, p.y, width * 0.3 * pulse, tint(INK, 0.9));
    }
    for i in 0..10 {
        let phase = (time * 3.0 + i as f32 * 0.173).fract();
        let a = i as f32 * 2.399 + time * 0.3;
        let direction = Vec2::from_angle(a);
        let p = end + direction * width * phase * 2.5;
        line(
            p,
            p - direction * width * 0.22,
            1.4,
            tint(CYAN, 1.0 - phase),
        );
    }
}

pub(super) fn rocket(p: Vec2, direction: Vec2, scale: f32) {
    let side = vec2(-direction.y, direction.x);
    let size = (scale * 0.45).max(7.0);
    for i in (1..=6).rev() {
        let tail = p - direction * size * (1.0 + i as f32 * 0.55);
        draw_circle(
            tail.x,
            tail.y,
            size * (0.12 + i as f32 * 0.04),
            tint(MUTED, 0.16 * (1.0 - i as f32 / 7.0)),
        );
    }
    glow(
        p - direction * size * 2.0,
        p - direction * size * 0.6,
        size * 0.25,
        PINK,
    );
    outline(
        &[
            p + direction * size,
            p - direction * size * 0.65 + side * size * 0.28,
            p - direction * size * 0.45,
            p - direction * size * 0.65 - side * size * 0.28,
        ],
        2.0,
        INK,
        true,
    );
}
