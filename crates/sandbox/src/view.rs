pub(crate) mod decorations;
pub(crate) mod scene;
pub(crate) mod tank_parts;
mod weapon_effects;
use crate::ui::{CYAN, INK, MUTED, PAPER, PINK, RULE, VIOLET};
use macroquad::prelude::*;
use scene::Scene;
use sim::{Material, Shape, World};
pub(crate) use weapon_effects::pickup_marker;

pub struct Viewport {
    pub rect: Rect,
    pub scale: f32,
    center: Vec2,
}
impl Viewport {
    pub fn new(rect: Rect, center: Vec2, zoom: f32) -> Self {
        Self {
            rect,
            center,
            scale: ((rect.w - 90.0) / 24.0).min((rect.h - 95.0) / 15.0) * zoom,
        }
    }
    pub fn screen(&self, p: Vec2) -> Vec2 {
        self.rect.center() + (p - self.center) * self.scale
    }
    pub fn world(&self, p: Vec2) -> Vec2 {
        (p - self.rect.center()) / self.scale + self.center
    }
}
pub struct Spark {
    pub position: Vec2,
    pub velocity: Vec2,
    pub life: f32,
}
pub struct Blast {
    pub position: Vec2,
    pub radius: f32,
    pub life: f32,
}
fn v(p: sim::Vec2) -> Vec2 {
    vec2(p.x, p.y)
}
fn tint(color: Color, alpha: f32) -> Color {
    Color {
        a: color.a * alpha,
        ..color
    }
}
pub(crate) fn station_marker(
    station: &sim::StationView,
    view: &Viewport,
    opacity: f32,
    show_radius: bool,
) {
    use sim::level::{StationKind, Team};
    let team_color = |team| match team {
        Some(Team::Blue) => CYAN,
        Some(Team::Red) => PINK,
        None => MUTED,
    };
    let color = tint(team_color(station.owner), opacity);
    let p = view.screen(v(station.position));
    let scale = view.scale;
    if show_radius {
        draw_circle_lines(
            p.x,
            p.y,
            station.settings.capture_radius * scale,
            1.0,
            tint(color, 0.45),
        );
        if station.kind == StationKind::Radar
            && station.owner.is_none()
            && station.capturing.is_none()
        {
            draw_circle_lines(
                p.x,
                p.y,
                station.settings.radar_radius * scale,
                1.0,
                tint(VIOLET, opacity * 0.25),
            );
        }
    }
    draw_rectangle(p.x - scale, p.y - scale, scale * 2.0, scale * 2.0, PAPER);
    draw_rectangle_lines(
        p.x - scale,
        p.y - scale,
        scale * 2.0,
        scale * 2.0,
        2.0,
        color,
    );
    match station.kind {
        StationKind::Repair => {
            line(
                p - vec2(0.5, 0.0) * scale,
                p + vec2(0.5, 0.0) * scale,
                (scale * 0.24).max(2.0),
                color,
            );
            line(
                p - vec2(0.0, 0.5) * scale,
                p + vec2(0.0, 0.5) * scale,
                (scale * 0.24).max(2.0),
                color,
            );
        }
        StationKind::Radar => {
            draw_circle_lines(p.x, p.y, scale * 0.58, 1.5, color);
            draw_circle_lines(p.x, p.y, scale * 0.3, 1.0, tint(color, 0.6));
            line(p, p + vec2(0.43, -0.4) * scale, 2.0, color);
            draw_circle(p.x, p.y, 2.5, color);
        }
    }
    let label = if station.contested {
        "CONTESTED"
    } else if station.capturing.is_some() && station.owner.is_some() {
        "NEUTRALIZING"
    } else if let Some(team) = station.capturing {
        if team == Team::Blue {
            "CAPTURING / BLUE"
        } else {
            "CAPTURING / RED"
        }
    } else {
        match station.owner {
            Some(Team::Blue) => "BLUE",
            Some(Team::Red) => "RED",
            None => "NEUTRAL",
        }
    };
    let width = measure_text(label, None, 12, 1.0).width;
    draw_text(label, p.x - width * 0.5, p.y + scale + 17.0, 12.0, color);
    if station.capturing.is_some() {
        let y = p.y - scale - 7.0;
        draw_rectangle(p.x - scale, y, 2.0 * scale, 4.0, tint(RULE, opacity));
        draw_rectangle(
            p.x - scale,
            y,
            2.0 * scale * station.progress,
            4.0,
            tint(team_color(station.capturing), opacity),
        );
    }
}
fn line(a: Vec2, b: Vec2, width: f32, color: Color) {
    draw_line(a.x, a.y, b.x, b.y, width, color);
}
fn glow(a: Vec2, b: Vec2, width: f32, color: Color) {
    line(a, b, width + 4.0, tint(color, 0.06));
    line(a, b, width + 1.5, tint(color, 0.13));
    line(a, b, width, color);
}
fn outline(points: &[Vec2], width: f32, color: Color, glowing: bool) {
    for (i, &a) in points.iter().enumerate() {
        let b = points[(i + 1) % points.len()];
        if glowing {
            glow(a, b, width, color);
        } else {
            line(a, b, width, color);
        }
    }
}
fn masked_polygon(points: &[Vec2], color: Color) {
    for i in 1..points.len() - 1 {
        draw_triangle(points[0], points[i], points[i + 1], PAPER);
    }
    outline(points, 1.7, color, true);
}
pub fn load_color(value: f32) -> Color {
    let t = value.clamp(0.0, 1.0);
    Color::new(
        CYAN.r + (PINK.r - CYAN.r) * t,
        CYAN.g + (PINK.g - CYAN.g) * t,
        CYAN.b + (PINK.b - CYAN.b) * t,
        1.0,
    )
}
fn interpolate_angle(previous: f32, current: f32, alpha: f32) -> f32 {
    previous
        + ((current - previous + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI)
            * alpha
}

fn draw_terrain(world: &World, viewport: &Viewport) {
    crate::terrain_view::draw(world.terrain(), viewport);
    if world.scene() == sim::Scene::Terrain {
        for (label, position, color) in [
            ("ROAD / FAST", vec2(-10.7, -1.8), MUTED),
            ("GROUND", vec2(-10.7, 5.8), MUTED),
        ] {
            let p = viewport.screen(position);
            draw_text(label, p.x, p.y, 17.0, color);
        }
    }
}

fn draw_bridges(scene: &Scene, viewport: &Viewport) {
    let rectangle = |min: sim::Vec2, max: sim::Vec2, fill, edge| {
        let a = viewport.screen(v(min));
        let b = viewport.screen(v(max));
        draw_rectangle(a.x, a.y, b.x - a.x, b.y - a.y, fill);
        draw_rectangle_lines(a.x, a.y, b.x - a.x, b.y - a.y, 1.5, edge);
    };
    for bridge in &scene.bridges {
        let water = tint(Color::from_rgba(91, 171, 227, 255), bridge.opacity);
        let bridge_color = tint(Color::from_rgba(218, 223, 191, 255), bridge.opacity);
        if bridge.intact {
            rectangle(bridge.deck.min, bridge.deck.max, PAPER, bridge_color);
            let vertical = (bridge.controls[1] - bridge.controls[0]).y.abs()
                > (bridge.controls[1] - bridge.controls[0]).x.abs();
            for side in [0.14, -0.14] {
                let (a, b) = if vertical {
                    let x = if side > 0.0 {
                        bridge.deck.min.x + side
                    } else {
                        bridge.deck.max.x + side
                    };
                    (vec2(x, bridge.deck.min.y), vec2(x, bridge.deck.max.y))
                } else {
                    let y = if side > 0.0 {
                        bridge.deck.min.y + side
                    } else {
                        bridge.deck.max.y + side
                    };
                    (vec2(bridge.deck.min.x, y), vec2(bridge.deck.max.x, y))
                };
                line(
                    viewport.screen(a),
                    viewport.screen(b),
                    1.4,
                    tint(bridge_color, 0.5),
                );
            }
        } else {
            let t = bridge.fall_progress;
            for body in &bridge.fragments {
                let Shape::Triangle { vertices } = body.shape else {
                    continue;
                };
                let center = v(body.position).lerp(
                    vec2(
                        (bridge.deck.min.x + bridge.deck.max.x) * 0.5,
                        body.position.y,
                    ),
                    t * 0.16,
                ) + vec2(0.0, t * t * 0.5);
                let rotation =
                    Vec2::from_angle(body.angle + ((body.id % 5) as f32 - 2.0) * t * 0.1);
                let points = vertices
                    .map(|p| viewport.screen(center + rotation.rotate(v(p)) * (1.0 - t * 0.25)));
                for i in 0..3 {
                    if body.edges & (1 << i) != 0 {
                        line(
                            points[i],
                            points[(i + 1) % 3],
                            1.4,
                            tint(bridge_color, (1.0 - t).powi(2)),
                        );
                    }
                }
            }
            if t < 1.0 {
                let center = viewport.screen(v((bridge.deck.min + bridge.deck.max) * 0.5));
                draw_ellipse_lines(
                    center.x,
                    center.y,
                    viewport.scale * (1.8 + t * 2.0),
                    viewport.scale * (0.6 + t),
                    0.0,
                    1.6,
                    tint(water, 1.0 - t),
                );
            }
        }
        for (i, control) in bridge.controls.iter().enumerate() {
            let p = viewport.screen(v(*control));
            let color = if bridge.intact {
                bridge_color
            } else {
                tint(PINK, bridge.opacity)
            };
            draw_circle_lines(p.x, p.y, viewport.scale * 0.3, 1.8, color);
            let label = if bridge.intact {
                if i == 0 { "B" } else { "A" }
            } else {
                "X"
            };
            draw_text(label, p.x - 5.0, p.y + 5.0, 17.0, color);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_world(
    world: &World,
    viewport: &Viewport,
    aim: Option<Vec2>,
    alpha: f32,
    overlay: bool,
    sparks: &[Spark],
    blasts: &[Blast],
    wreck_effects: &[crate::wreck_effects::WreckEffect],
    track_marks: &mut crate::track_marks::TrackMarks,
    observed: Option<(&Scene, &sim::Vision)>,
) {
    let live = observed.is_none().then(|| Scene::capture(world));
    let scene = observed
        .map(|(scene, _)| scene)
        .unwrap_or_else(|| live.as_ref().unwrap());
    let vision = observed.map(|(_, vision)| vision);
    draw_background(world, viewport);
    track_marks.draw(viewport, vision);
    if let Some(vision) = vision {
        draw_sight_shading(viewport, vision);
    }
    for (pickup, opacity) in &scene.pickups {
        pickup_marker(pickup, viewport, *opacity);
    }
    draw_bridges(scene, viewport);
    for (prop, opacity) in &scene.decorations {
        decorations::draw(prop, viewport, *opacity);
    }
    draw_objects(scene, viewport, alpha, overlay);
    for (station, opacity) in &scene.stations {
        station_marker(
            station,
            viewport,
            *opacity,
            station.capturing.is_some() || station.contested,
        );
    }
    if overlay && observed.is_none() {
        for bond in world.bonds() {
            let p = viewport.screen(v(bond.position));
            draw_circle(p.x, p.y, 2.0, load_color(bond.load));
        }
    }
    for effect in wreck_effects {
        effect.draw(viewport, alpha);
    }
    draw_tanks(world, scene, viewport, alpha);
    for (prop, age) in world.decoration_bursts() {
        if vision.is_none_or(|v| v.intersects_polygon(&prop.outline())) {
            decorations::burst(&prop, age, viewport);
        }
    }
    draw_effects(world, viewport, alpha, sparks, blasts, vision);
    draw_crosshair(world, viewport, aim, alpha);
}

// Ordinary screen-space geometry only; objects never pass through a texture/mask.
fn draw_sight_shading(viewport: &Viewport, vision: &sim::Vision) {
    let color = Color::new(0.0, 0.0, 0.0, 0.25);
    let rect = viewport.rect;
    if vision.regions().count() > 1 {
        let a = viewport.world(vec2(rect.x, rect.y));
        let b = viewport.world(vec2(rect.right(), rect.bottom()));
        for triangle in vision.shadow_triangles(
            sim::Vec2::new(a.x.min(b.x), a.y.min(b.y)),
            sim::Vec2::new(a.x.max(b.x), a.y.max(b.y)),
        ) {
            let [a, b, c] = triangle.map(|p| viewport.screen(v(p)));
            draw_triangle(a, b, c, color);
        }
        return;
    }
    if vision.boundary.is_empty() {
        draw_rectangle(rect.x, rect.y, rect.w, rect.h, color);
        return;
    }
    let origin = viewport.screen(v(vision.origin));
    let radius = [
        vec2(rect.x, rect.y),
        vec2(rect.right(), rect.y),
        vec2(rect.right(), rect.bottom()),
        vec2(rect.x, rect.bottom()),
    ]
    .into_iter()
    .map(|p| p.distance(origin))
    .fold(0.0_f32, f32::max)
        + 1.0;
    for i in 0..vision.boundary.len() {
        let a = viewport.screen(v(vision.boundary[i]));
        let b = viewport.screen(v(vision.boundary[(i + 1) % vision.boundary.len()]));
        let outer_a = origin + (a - origin).normalize_or_zero() * radius;
        let outer_b = origin + (b - origin).normalize_or_zero() * radius;
        draw_triangle(a, outer_a, outer_b, color);
        draw_triangle(a, outer_b, b, color);
    }
}

pub(crate) fn draw_background(world: &World, viewport: &Viewport) {
    let rect = viewport.rect;
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, PAPER);
    // Sparse scale marks; no movement grid or textured surfaces.
    for x in -6..=6 {
        for y in -4..=4 {
            let p = viewport.screen(vec2(x as f32 * 2.0, y as f32 * 2.0));
            line(p - vec2(2.0, 0.0), p + vec2(2.0, 0.0), 1.0, tint(RULE, 0.7));
            line(p - vec2(0.0, 2.0), p + vec2(0.0, 2.0), 1.0, tint(RULE, 0.7));
        }
    }
    draw_terrain(world, viewport);
}

fn draw_objects(scene: &Scene, viewport: &Viewport, alpha: f32, overlay: bool) {
    for &(team, p, opacity) in &scene.flags {
        flag_marker(
            viewport.screen(v(p)),
            viewport.scale,
            tint(
                if team == sim::level::Team::Blue {
                    CYAN
                } else {
                    PINK
                },
                opacity,
            ),
        );
    }
    for body in &scene.bodies {
        if body.material == Material::Launcher {
            continue;
        }
        let center = viewport.screen(v(body.previous_position).lerp(v(body.position), alpha));
        let rotation = Vec2::from_angle(interpolate_angle(body.previous_angle, body.angle, alpha));
        let transform = |p: Vec2| center + rotation.rotate(p) * viewport.scale;
        let color = match body.material {
            Material::Masonry | Material::Rock => VIOLET,
            Material::Panel => CYAN,
            Material::Boundary => RULE,
            Material::Anchor => INK,
            _ => PINK,
        };
        let color = body
            .color
            .map_or(color, |[r, g, b]| Color::from_rgba(r, g, b, 255));
        let color = if overlay
            && matches!(
                body.material,
                Material::Masonry | Material::Panel | Material::Rock
            ) {
            load_color(body.load)
        } else {
            color
        };
        let color = tint(color, body.opacity);
        if body.tank_fragment.is_some() {
            tank_parts::draw_fragment(body, viewport, alpha, color);
            continue;
        }
        match body.shape {
            Shape::Triangle { vertices } => {
                let points = vertices.map(|p| transform(v(p)));
                for i in 0..3 {
                    if overlay || body.edges & (1 << i) != 0 {
                        line(points[i], points[(i + 1) % 3], 1.15, color);
                    }
                }
            }
            Shape::Box { half } => {
                let points = [
                    vec2(-half.x, -half.y),
                    vec2(half.x, -half.y),
                    vec2(half.x, half.y),
                    vec2(-half.x, half.y),
                ]
                .map(transform);
                outline(&points, 1.5, color, body.material == Material::Anchor);
                if body.material == Material::Anchor {
                    for dx in [-0.75, 0.75] {
                        let bolt = transform(vec2(half.x * dx, 0.0));
                        line(bolt - vec2(3.0, 3.0), bolt + vec2(3.0, 3.0), 1.3, color);
                        line(bolt - vec2(3.0, -3.0), bolt + vec2(3.0, -3.0), 1.3, color);
                    }
                }
            }
            Shape::Ball { radius } => {
                draw_circle_lines(center.x, center.y, radius * viewport.scale, 1.5, color)
            }
        }
    }
}

fn draw_tanks(world: &World, scene: &Scene, viewport: &Viewport, alpha: f32) {
    for tank in scene
        .bodies
        .iter()
        .filter(|b| b.material == Material::Launcher)
    {
        if tank.opacity <= 0.0 {
            continue;
        }
        let status = scene.tanks.iter().copied().find(|t| t.id == tank.id);
        let player = tank.id == world.player_tank_id();
        let tank_color = tint(
            if status.map_or(world.team(), |t| t.team) == sim::level::Team::Red {
                PINK
            } else {
                CYAN
            },
            tank.opacity,
        );
        let hull_color = tint(INK, tank.opacity);
        let fall = if player {
            world.tank_fall_progress()
        } else {
            0.0
        };
        let scale = viewport.scale * (1.0 - fall * 0.45);
        let position = v(tank.previous_position).lerp(v(tank.position), alpha);
        let angle = interpolate_angle(tank.previous_angle, tank.angle, alpha);
        let center = viewport.screen(position) + vec2(0.0, fall * viewport.scale * 0.4);
        let rotation = Vec2::from_angle(angle);
        let transform = |p: Vec2| center + rotation.rotate(p) * scale;
        let (previous_turret, turret) = if player {
            let aim = world.player_aim();
            (aim.previous_angle, aim.angle)
        } else {
            let tank = status.unwrap();
            (tank.previous_turret_angle, tank.turret_angle)
        };
        let aim_direction = Vec2::from_angle(interpolate_angle(previous_turret, turret, alpha));
        // Tracks, road wheels and moving tread slats are all vector geometry.
        for (i, y) in [-0.65, 0.65].into_iter().enumerate() {
            let points = tank_parts::track_outline(y).map(transform);
            masked_polygon(&points, tank_color);
            let offset =
                status.map_or(world.track_travel(), |t| t.track_travel)[i].rem_euclid(0.18);
            for j in 0..11 {
                let x = -0.99 + j as f32 * 0.18 + offset;
                line(
                    transform(vec2(x, y - 0.13)),
                    transform(vec2(x, y + 0.13)),
                    1.2,
                    tint(tank_color, 0.8),
                );
            }
        }
        let hull = tank_parts::hull_outline().map(transform);
        masked_polygon(&hull, hull_color);
        for x in [-0.72, -0.60, -0.48] {
            line(
                transform(vec2(x, -0.28)),
                transform(vec2(x, 0.28)),
                1.2,
                tint(MUTED, tank.opacity),
            );
        }
        let turret_rotation = Vec2::from_angle(aim_direction.to_angle());
        let turret = |p: Vec2| center + turret_rotation.rotate(p) * scale;
        let recoil = if matches!(
            if player {
                world.last_ammunition()
            } else {
                sim::Ammunition::ArmorPiercing
            },
            sim::Ammunition::HighExplosive | sim::Ammunition::ArmorPiercing
        ) {
            let elapsed = sim::Ammunition::ArmorPiercing.interval()
                - status.map_or(world.reload_remaining(), |t| t.reload);
            (1.0 - elapsed / 0.15).clamp(0.0, 1.0) * 0.22
        } else {
            0.0
        };
        masked_polygon(
            &[
                vec2(0.1, -0.105),
                vec2(1.58 - recoil, -0.105),
                vec2(1.58 - recoil, 0.105),
                vec2(0.1, 0.105),
            ]
            .map(turret),
            tank_color,
        );
        masked_polygon(&tank_parts::turret_outline().map(turret), tank_color);
        draw_circle_lines(center.x, center.y, scale * 0.13, 1.3, hull_color);
        if let Some(status) = status {
            let width = (scale * 2.0).max(28.0);
            let at = center + vec2(-width * 0.5, -scale * 1.35 - 6.0);
            draw_rectangle(at.x, at.y, width, 4.0, tint(MUTED, tank.opacity));
            draw_rectangle(at.x, at.y, width * status.health / 100.0, 4.0, tank_color);
            if status.protected {
                draw_circle_lines(center.x, center.y, scale * 1.45, 1.0, tank_color);
            }
        }
    }
}

fn draw_effects(
    world: &World,
    viewport: &Viewport,
    alpha: f32,
    sparks: &[Spark],
    blasts: &[Blast],
    vision: Option<&sim::Vision>,
) {
    let visible = |p| vision.is_none_or(|vision| vision.contains(p));
    for shell in world.shells() {
        if !visible(shell.position) {
            continue;
        }
        let p = viewport.screen(v(shell.previous_position).lerp(v(shell.position), alpha));
        let direction = v(shell.velocity).normalize();
        let side = vec2(-direction.y, direction.x);
        let color = match shell.ammunition {
            sim::Ammunition::HighExplosive => PINK,
            sim::Ammunition::ArmorPiercing => CYAN,
            _ => Color::from_rgba(255, 220, 120, 255),
        };
        if shell.ammunition == sim::Ammunition::Rocket {
            weapon_effects::rocket(p, direction, viewport.scale);
            continue;
        }
        glow(p - direction * viewport.scale * 0.9, p, 2.3, color);
        outline(
            &[
                p + direction * 7.0,
                p - direction * 5.0 + side * 2.5,
                p - direction * 5.0 - side * 2.5,
            ],
            1.6,
            INK,
            true,
        );
    }
    for weapon in world.weapon_effects() {
        if weapon.charging > 0.0 && visible(weapon.muzzle) {
            weapon_effects::plasma_charge(
                viewport.screen(v(weapon.muzzle)),
                viewport.scale,
                weapon.charging,
            );
        }
        for (beam, plasma) in [(weapon.laser, false), (weapon.plasma, true)] {
            let Some(beam) = beam.filter(|beam| visible(beam.start)) else {
                continue;
            };
            let start = viewport.screen(v(beam.start));
            let mut endpoint = beam.end;
            if !visible(endpoint) {
                let mut start = beam.start;
                for _ in 0..14 {
                    let mid = (start + endpoint) * 0.5;
                    if visible(mid) {
                        start = mid;
                    } else {
                        endpoint = mid;
                    }
                }
                endpoint = start;
            }
            let end = viewport.screen(v(endpoint));
            if plasma {
                weapon_effects::plasma(start, end, viewport.scale, beam.remaining);
            } else {
                weapon_effects::laser(start, end, viewport.scale, weapon.laser_elapsed);
            }
        }
    }
    for blast in blasts {
        if !visible(sim::Vec2::new(blast.position.x, blast.position.y)) {
            continue;
        }
        let t = (1.0 - blast.life / 0.45).clamp(0.0, 1.0);
        let p = viewport.screen(blast.position);
        let r = blast.radius * viewport.scale * (1.0 - (1.0 - t).powi(3));
        let color = tint(PINK, (1.0 - t).powi(2));
        for i in 0..24 {
            let a = i as f32 * std::f32::consts::TAU / 24.0;
            let b = (i + 1) as f32 * std::f32::consts::TAU / 24.0;
            glow(
                p + Vec2::from_angle(a) * r,
                p + Vec2::from_angle(b) * r,
                1.7,
                color,
            );
        }
        if t < 0.25 {
            let radius = (1.0 - t / 0.25) * viewport.scale * 0.30;
            draw_poly(p.x, p.y, 8, radius, 22.5, tint(INK, 1.0 - t / 0.25));
        }
    }
    for spark in sparks {
        if !visible(sim::Vec2::new(spark.position.x, spark.position.y)) {
            continue;
        }
        let a = viewport.screen(spark.position);
        glow(
            a,
            a - spark.velocity * viewport.scale * 0.035,
            1.4,
            tint(
                if spark.life > 0.25 { INK } else { PINK },
                (spark.life / 0.18).min(1.0),
            ),
        );
    }
}

pub(crate) fn draw_crosshair(world: &World, viewport: &Viewport, aim: Option<Vec2>, alpha: f32) {
    let color = if world.reload_remaining() > 0.0 {
        PINK
    } else {
        CYAN
    };
    if world.player_respawn_remaining().is_none() && world.tank_fall_progress() == 0.0 {
        let state = world.player_aim();
        let angle = interpolate_angle(state.previous_angle, state.angle, alpha);
        if let Some(tank) = world.bodies().find(|b| b.id == world.player_tank_id()) {
            let center = v(tank.previous_position).lerp(v(tank.position), alpha);
            let muzzle = center + Vec2::from_angle(angle) * 1.65;
            let origin = viewport.screen(muzzle);
            let radius = 2.5 * viewport.scale;
            let half_width = 3.5;
            let band = |from: f32, to: f32, fill: Color| {
                if to <= from {
                    return;
                }
                let edge = |a: f32, r: f32| origin + Vec2::from_angle(angle + a) * r;
                for i in 0..32 {
                    let a = from + (to - from) * i as f32 / 32.0;
                    let b = from + (to - from) * (i + 1) as f32 / 32.0;
                    let inner_a = edge(a, radius - half_width);
                    let outer_a = edge(a, radius + half_width);
                    let inner_b = edge(b, radius - half_width);
                    let outer_b = edge(b, radius + half_width);
                    draw_triangle(inner_a, outer_a, outer_b, fill);
                    draw_triangle(inner_a, outer_b, inner_b, fill);
                }
            };
            let progress = (1.0 - world.reload_remaining() / world.last_ammunition().interval())
                .clamp(0.0, 1.0);
            let split = -state.spread + 2.0 * state.spread * progress;
            band(-state.spread, split, tint(CYAN, 0.45));
            band(split, state.spread, tint(PINK, 0.18));
        }
    }
    let Some(aim) = aim else { return };
    let p = viewport.screen(aim);
    for axis in [Vec2::X, Vec2::Y] {
        for sign in [-1.0, 1.0] {
            line(
                p + axis * 5.0 * sign,
                p + axis * 11.0 * sign,
                1.2,
                tint(color, 0.65),
            );
        }
    }
}

/// A marker has no horizontal body or weapon target.
pub fn flag_marker(p: Vec2, scale: f32, color: Color) {
    let h = (scale * 1.0).clamp(14.0, 55.0);
    draw_circle_lines(
        p.x,
        p.y,
        (scale * sim::level::FLAG_RADIUS).max(4.0),
        1.5,
        color,
    );
    draw_line(p.x, p.y, p.x, p.y - h, 2.0, color);
    let a = vec2(p.x, p.y - h);
    let b = vec2(p.x + h * 0.65, p.y - h * 0.75);
    let c = vec2(p.x, p.y - h * 0.5);
    draw_triangle_lines(a, b, c, 2.0, color);
}
