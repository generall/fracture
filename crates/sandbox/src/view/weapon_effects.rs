//! Weapon markers and effects use the normal world renderer.
use super::*;

pub(super) fn airdrop_marker(drop: &sim::AirdropView, view: &Viewport) {
    let target = view.screen(v(drop.position));
    let rect = view.rect;
    if !rect.contains(target) {
        return; // The shared edge indicator handles off-screen drops.
    }
    let p = target;
    let r = 13.0;
    let color = if drop.remaining <= 5.0 { PINK } else { CYAN };
    draw_circle(p.x, p.y, 22.0, tint(PAPER, 0.9));
    draw_rectangle_lines(p.x - r * 0.6, p.y - 2.0, r * 1.2, r, 2.0, color);
    line(p + vec2(0.0, -2.0), p + vec2(0.0, r - 2.0), 1.0, color);
    let canopy = [
        vec2(-r, -8.0),
        vec2(-r * 0.6, -16.0),
        vec2(r * 0.6, -16.0),
        vec2(r, -8.0),
    ];
    outline(&canopy.map(|q| p + q), 1.5, color, true);
    line(p + canopy[0], p + vec2(-r * 0.6, -2.0), 1.0, color);
    line(p + canopy[3], p + vec2(r * 0.6, -2.0), 1.0, color);
    let label = format!(
        "AIRDROP / {} / {:.0}s",
        drop.weapon.label(),
        drop.remaining.ceil()
    );
    let font_size = 15.0;
    let width = measure_text(&label, None, font_size as u16, 1.0).width;
    let x = (p.x - width * 0.5).clamp(rect.x + 8.0, (rect.right() - width - 8.0).max(rect.x + 8.0));
    draw_rectangle(x - 4.0, p.y + 23.0, width + 8.0, 21.0, tint(PAPER, 0.9));
    crate::ui::text(&label, x, p.y + 39.0, font_size, color);
}

pub(crate) fn pickup_marker(pickup: &sim::PickupView, view: &Viewport, opacity: f32) {
    let p = view.screen(v(pickup.position));
    let ready = pickup.remaining <= 0.0;
    let r = (view.scale * 0.6).max(8.0);
    let color = tint(
        match pickup.weapon {
            sim::SpecialWeapon::Laser => CYAN,
            sim::SpecialWeapon::Rocket => PINK,
            sim::SpecialWeapon::Plasma => VIOLET,
            sim::SpecialWeapon::Mine => YELLOW,
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
        if pickup.weapon == sim::SpecialWeapon::Mine {
            mine_symbol(p, r * 0.55, color);
        } else if pickup.weapon == sim::SpecialWeapon::Plasma {
            draw_circle_lines(p.x, p.y, r * 0.48, 2.0, color);
            draw_circle(p.x, p.y, r * 0.20, INK);
            for i in 0..4 {
                let axis = Vec2::from_angle(i as f32 * std::f32::consts::FRAC_PI_2);
                glow(p + axis * r * 0.55, p + axis * r * 0.8, 1.5, color);
            }
        } else if pickup.weapon == sim::SpecialWeapon::Laser {
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

fn mine_symbol(p: Vec2, radius: f32, color: Color) {
    draw_circle(p.x, p.y, radius, tint(PAPER, 0.95));
    draw_circle_lines(p.x, p.y, radius, 1.5, color);
    draw_circle(p.x, p.y, radius * 0.3, color);
    for i in 0..6 {
        let axis = Vec2::from_angle(i as f32 * std::f32::consts::TAU / 6.0);
        line(p + axis * radius * 0.8, p + axis * radius * 1.3, 1.5, color);
    }
}

pub(super) fn mine_marker(mine: sim::MineView, view: &Viewport, friendly: bool) {
    let p = view.screen(v(mine.position));
    let color = if friendly { CYAN } else { PINK };
    let radius = (view.scale * 0.28).max(5.0);
    mine_symbol(p, radius, color);
    if mine.arming_remaining > 0.0 {
        draw_circle_lines(p.x, p.y, radius * 1.6, 1.0, tint(color, 0.4));
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

/// A gathering muzzle orb, followed by a short fading discharge.
pub(super) fn plasma_charge(p: Vec2, scale: f32, remaining: f32) {
    let progress = (1.0 - remaining).clamp(0.0, 1.0);
    let radius = scale * (0.12 + progress * 0.35);
    draw_circle(p.x, p.y, radius * 2.0, tint(VIOLET, 0.10 + progress * 0.1));
    draw_circle_lines(p.x, p.y, radius, 2.0, VIOLET);
    draw_circle(p.x, p.y, radius * 0.5, tint(INK, 0.4 + progress * 0.6));
    for i in 0..8 {
        let axis = Vec2::from_angle(i as f32 * std::f32::consts::TAU / 8.0 + progress * 5.0);
        let distance = radius + scale * (1.0 - (progress * 3.0 + i as f32 / 8.0).fract());
        line(
            p + axis * distance,
            p + axis * (distance + scale * 0.15),
            1.5,
            tint(VIOLET, 0.8),
        );
    }
}

pub(super) fn plasma(start: Vec2, end: Vec2, scale: f32, remaining: f32) {
    let fade = (remaining / 0.18).clamp(0.0, 1.0);
    let width = (scale * 0.6).max(6.0);
    line(start, end, width * 2.5, tint(VIOLET, fade * 0.18));
    line(start, end, width, tint(VIOLET, fade));
    line(start, end, width * 0.35, tint(INK, fade));
    for p in [start, end] {
        draw_circle_lines(p.x, p.y, width * (2.0 - fade), 2.0, tint(VIOLET, fade));
        draw_circle(p.x, p.y, width * 0.6, tint(INK, fade));
    }
}
