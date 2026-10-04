//! Public objectives and currently visible targets, only beyond the viewport.
use super::*;

#[derive(Clone, Copy, Debug)]
enum Kind {
    Enemy,
    Loot(sim::SpecialWeapon),
    Flag,
    Airdrop(sim::AirdropView),
}

fn targets(world: &World, scene: &Scene) -> Vec<(Vec2, Kind)> {
    let mut targets = Vec::new();
    for tank in scene
        .tanks
        .iter()
        .filter(|t| t.team != world.team() && t.health > 0.0)
    {
        if let Some(body) = scene
            .bodies
            .iter()
            .find(|b| b.id == tank.id && b.opacity >= 0.99)
        {
            targets.push((v(body.position), Kind::Enemy));
        }
    }
    for (pickup, opacity) in &scene.pickups {
        if *opacity >= 0.99 && pickup.remaining <= 0.0 {
            targets.push((v(pickup.position), Kind::Loot(pickup.weapon)));
        }
    }
    for (team, position) in world.flags() {
        if team != world.team() {
            targets.push((v(position), Kind::Flag));
        }
    }
    if let Some(drop) = world.airdrop() {
        targets.push((v(drop.position), Kind::Airdrop(drop)));
    }
    targets
}

/// Intersect a center-to-target ray with an inset screen rectangle.
fn edge_position(rect: Rect, target: Vec2) -> Option<Vec2> {
    if rect.contains(target) {
        return None;
    }
    let delta = target - rect.center();
    let half = vec2(
        (rect.w * 0.5 - 26.0).max(1.0),
        (rect.h * 0.5 - 26.0).max(1.0),
    );
    let ratio = (delta.abs() / half).max_element();
    Some(rect.center() + delta / ratio)
}

pub(super) fn draw(world: &World, scene: &Scene, view: &Viewport) {
    let rect = view.rect;
    let mut occupied: Vec<Vec2> = vec![];
    for (position, kind) in targets(world, scene) {
        let target = view.screen(position);
        let Some(mut p) = edge_position(rect, target) else {
            continue;
        };
        // Spread neighbors along the same edge; point each arrow at its own target.
        let tangent =
            if (p.x - rect.x - 26.0).abs() < 0.1 || (p.x - rect.right() + 26.0).abs() < 0.1 {
                Vec2::Y
            } else {
                Vec2::X
            };
        for step in 0..24 {
            let offset = ((step + 1) / 2) as f32 * 30.0 * if step % 2 == 0 { 1.0 } else { -1.0 };
            let candidate = p + tangent * offset;
            if candidate.x < rect.x + 25.0
                || candidate.x > rect.right() - 25.0
                || candidate.y < rect.y + 25.0
                || candidate.y > rect.bottom() - 25.0
            {
                continue;
            }
            if occupied.iter().all(|q| q.distance(candidate) >= 28.0) {
                p = candidate;
                break;
            }
        }
        occupied.push(p);
        let color = match kind {
            Kind::Enemy => PINK,
            Kind::Flag => INK,
            Kind::Loot(_) | Kind::Airdrop(_) => CYAN,
        };
        let direction = (target - p).normalize_or_zero();
        let side = vec2(-direction.y, direction.x);
        let tip = p + direction * 15.0;
        draw_circle(p.x, p.y, 18.0, tint(PAPER, 0.92));
        draw_triangle(
            tip,
            tip - direction * 9.0 + side * 5.0,
            tip - direction * 9.0 - side * 5.0,
            color,
        );
        let icon = p - direction * 4.0;
        match kind {
            Kind::Enemy => {
                draw_rectangle_lines(icon.x - 5.0, icon.y - 4.0, 10.0, 8.0, 1.5, color);
                line(icon, icon + vec2(8.0, 0.0), 2.0, color);
            }
            Kind::Flag => {
                line(icon + vec2(-3.0, 6.0), icon + vec2(-3.0, -6.0), 1.5, color);
                draw_triangle(
                    icon + vec2(-3.0, -6.0),
                    icon + vec2(6.0, -3.0),
                    icon + vec2(-3.0, 0.0),
                    color,
                );
            }
            Kind::Loot(weapon) => {
                draw_poly_lines(icon.x, icon.y, 4, 8.0, 0.0, 1.0, color);
                let label = match weapon {
                    sim::SpecialWeapon::Laser => "L",
                    sim::SpecialWeapon::Rocket => "R",
                    sim::SpecialWeapon::Plasma => "P",
                };
                crate::ui::text(label, icon.x - 3.5, icon.y + 4.0, 12.0, color);
            }
            Kind::Airdrop(drop) => {
                draw_rectangle_lines(icon.x - 5.0, icon.y - 5.0, 10.0, 10.0, 1.5, color);
                let label = format!("AIRDROP / {:.0}s", drop.remaining.ceil());
                let width = measure_text(&label, None, 14, 1.0).width;
                let label_center = p - direction * (width * 0.5 + 25.0);
                let x =
                    (label_center.x - width * 0.5).clamp(rect.x + 8.0, rect.right() - width - 8.0);
                let y = label_center.y.clamp(rect.y + 20.0, rect.bottom() - 8.0);
                draw_rectangle(x - 3.0, y - 14.0, width + 6.0, 18.0, tint(PAPER, 0.9));
                crate::ui::text(&label, x, y, 14.0, color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrows_only_appear_beyond_actual_viewport_and_follow_target_direction() {
        let rect = Rect::new(20.0, 40.0, 1280.0, 800.0);
        for target in [rect.center(), vec2(21.0, 41.0), vec2(1299.0, 839.0)] {
            assert!(edge_position(rect, target).is_none());
        }
        for target in [
            vec2(-100.0, 400.0),
            vec2(1800.0, 400.0),
            vec2(660.0, -50.0),
            vec2(660.0, 1100.0),
            vec2(-100.0, -100.0),
        ] {
            let p = edge_position(rect, target).unwrap();
            assert!(rect.contains(p));
            let direction = (target - rect.center()).normalize();
            assert!((p - rect.center()).normalize().distance(direction) < 0.0001);
        }
    }

    #[test]
    fn hidden_enemies_and_remembered_or_unavailable_loot_have_no_arrows() {
        let world =
            World::from_level(&sim::level::Level::default(), sim::level::Team::Blue).unwrap();
        let mut scene = Scene::capture(&world);
        scene.pickups = vec![(
            sim::PickupView {
                position: sim::Vec2::new(10.0, 0.0),
                weapon: sim::SpecialWeapon::Rocket,
                respawn_time: 30.0,
                charges: 1,
                cooldown: 0.0,
                remaining: 0.0,
            },
            1.0,
        )];
        assert_eq!(targets(&world, &scene).len(), 3, "enemy, loot, enemy flag");
        for body in &mut scene.bodies {
            body.opacity = 0.32;
        }
        scene.pickups[0].1 = 0.32;
        let hidden = targets(&world, &scene);
        assert_eq!(hidden.len(), 1);
        assert!(matches!(hidden[0].1, Kind::Flag));
        scene.pickups[0].1 = 1.0;
        scene.pickups[0].0.remaining = 10.0;
        assert_eq!(targets(&world, &scene).len(), 1);
    }
}
