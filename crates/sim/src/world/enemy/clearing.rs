//! Find material blocking the tank's swept width, including the sides of a
//! partly blasted opening. Visibility is checked from the actual gun origin.
use super::*;
use rapier2d::parry::query::{ShapeCastOptions, cast_shapes};

impl World {
    pub(super) fn route_obstruction(
        &self,
        tank: usize,
        route: &[Vector],
    ) -> Option<(usize, Vector)> {
        let origin = self.piece_pose(tank).translation;
        let mut start = origin;
        let mut remaining = 6.0;
        for &waypoint in route {
            let delta = waypoint - start;
            let length = delta.length().min(remaining);
            if length < 0.01 {
                continue;
            }
            let direction = delta.normalize();
            let mut nearest: Option<(f32, usize, Vector)> = None;
            for (_, collider) in self.physics.colliders.iter() {
                let Some(piece) = self.weapon_target(collider) else {
                    continue;
                };
                if self.pieces[piece].material == Material::Launcher {
                    continue;
                }
                let bounds = collider.compute_aabb();
                let end = start + direction * length;
                if (start.min(end) - Vector::splat(0.95))
                    .cmpgt(bounds.maxs)
                    .any()
                    || (start.max(end) + Vector::splat(0.95))
                        .cmplt(bounds.mins)
                        .any()
                {
                    continue;
                }
                if let Ok(Some(hit)) = cast_shapes(
                    &Pose::new(start, 0.0),
                    direction,
                    &Ball::new(0.95),
                    collider.position(),
                    Vector::ZERO,
                    collider.shape(),
                    ShapeCastOptions::with_max_time_of_impact(length),
                ) && nearest.is_none_or(|(time, _, _)| hit.time_of_impact < time)
                {
                    nearest = Some((
                        hit.time_of_impact,
                        piece,
                        collider.position().transform_point(hit.witness2),
                    ));
                }
            }
            if let Some((_, piece, point)) = nearest {
                if !self.pieces[piece].destructible() {
                    return None;
                }
                // Aim slightly into the obstruction rather than skimming its edge.
                let point = point.lerp(self.piece_pose(piece).translation, 0.15);
                let delta = point - origin;
                let distance = delta.length();
                if distance < 0.01 {
                    return None;
                }
                let ray = Ray::new(origin, delta / distance);
                let mut visible = (distance, piece);
                for (_, collider) in self.physics.colliders.iter() {
                    let Some(hit) = self.weapon_target(collider) else {
                        continue;
                    };
                    if hit == tank {
                        continue;
                    }
                    if let Some(time) =
                        collider
                            .shape()
                            .cast_ray(collider.position(), &ray, distance, true)
                        && time < visible.0
                    {
                        visible = (time, hit);
                    }
                }
                let hit = visible.1;
                return self.pieces[hit]
                    .destructible()
                    .then_some((hit, origin + ray.dir * (visible.0 + 0.03)));
            }
            remaining -= length;
            if remaining <= 0.01 {
                break;
            }
            start = waypoint;
        }
        None
    }
}
