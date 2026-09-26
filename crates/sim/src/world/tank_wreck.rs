//! Unconnected rigid pieces; their motion, contacts and retirement use ordinary debris physics.
use super::*;

impl World {
    pub(super) fn spawn_tank_wreck(
        &mut self,
        position: Vector,
        team: crate::level::Team,
        details: TankDestruction,
        mass: f32,
        omega: f32,
    ) {
        // Pieces fit inside the original hull/track colliders, with small gaps.
        // In particular, no overlapping turret layer can explode apart on creation.
        let mut parts = Vec::with_capacity(10);
        for x in [-0.475, 0.475] {
            for y in [-0.24, 0.24] {
                parts.push((Vector::new(x, y), Vector::new(0.46, 0.225)));
            }
        }
        for y in [-0.65, 0.65] {
            for x in [-0.72, 0.0, 0.72] {
                parts.push((Vector::new(x, y), Vector::new(0.35, 0.155)));
            }
        }
        // Always create new wrecks. Level cleanup targets 8192 active pieces
        // by retiring small offscreen debris instead of suppressing new ones.
        // These ten pieces also expire before their tank can respawn.
        let area: f32 = parts.iter().map(|(_, half)| 4.0 * half.x * half.y).sum();
        let rotation = Vector::from_angle(details.hull_angle);
        let weights: Vec<_> = parts
            .iter()
            .map(|(offset, _)| {
                1.0 / ((position + rotation.rotate(*offset)).distance_squared(details.impact.point)
                    + 0.25)
            })
            .collect();
        let total_weight: f32 = weights.iter().sum();
        let object = self.objects.len();
        self.objects.push(components::ObjectInstance {
            color: Some(if team == crate::level::Team::Blue {
                [76, 215, 235]
            } else {
                [242, 103, 173]
            }),
            components: crate::Components::default(),
            ignited: false,
            spent: false,
        });
        let travel = self
            .tanks
            .iter()
            .find(|t| t.id == details.id)
            .map_or([0.0; 2], |t| t.track_travel);
        for (part, ((offset, half), weight)) in parts.into_iter().zip(weights).enumerate() {
            let appearance = TankFragment {
                part: part as u8,
                local_offset: offset,
                tread_phase: travel[usize::from(offset.y > 0.0)].rem_euclid(0.18),
            };
            let offset = rotation.rotate(offset);
            let id = self.add_box(position + offset, half, Material::Panel, false);
            let piece = &mut self.pieces[id];
            piece.object = Some(object);
            piece.tank_fragment = Some(appearance);
            piece.fracture_parent = Some(details.id);
            piece.previous_angle = details.hull_angle;
            piece.debris_remaining = Some(2.5);
            piece.floor_friction = 0.45;
            piece.solid.density = mass / (area * DEPTH);
            self.physics.colliders[piece.collider].set_density(mass / area);
            let body = &mut self.physics.bodies[piece.body];
            body.recompute_mass_properties_from_colliders(&self.physics.colliders);
            body.set_rotation(Rotation::new(details.hull_angle), true);
            body.set_linvel(
                details.velocity + Vector::new(-offset.y, offset.x) * omega,
                true,
            );
            body.set_angvel(omega, true);
            body.apply_impulse_at_point(
                details.impact.impulse * (weight / total_weight),
                details.impact.point,
                true,
            );
        }
        self.physics
            .bodies
            .propagate_modified_body_positions_to_colliders(&mut self.physics.colliders);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::{Level, Team};

    #[test]
    fn large_levels_still_spawn_complete_tank_wrecks() {
        let mut world = World::from_level(&Level::default(), Team::Blue).unwrap();
        while world.pieces.len() < 8200 {
            let index = world.pieces.len();
            world.add_box(
                Vector::new((index % 100) as f32, 50.0 + (index / 100) as f32),
                Vector::splat(0.1),
                Material::Anchor,
                true,
            );
        }
        let tank = world.launcher;
        let mut events = Vec::new();
        world.destroy_tank(tank, &mut events);
        assert!(events.iter().any(|event| matches!(
            event,
            Event::TankDestroyed { details, .. } if details.id == tank
        )));
        let wreck: Vec<_> = world
            .bodies()
            .filter(|b| world.fracture_parent(b.id) == Some(tank))
            .collect();
        assert_eq!(wreck.len(), 10, "large maps need hull and track remains");
        assert!(
            wreck
                .iter()
                .all(|b| b.tank_fragment.is_some() && b.opacity == 1.0)
        );
    }

    #[test]
    fn wreck_pieces_are_separate_nonoverlapping_bodies_with_inherited_momentum() {
        let mut world = World::from_level(&Level::default(), Team::Blue).unwrap();
        let tank = world.launcher;
        let handle = world.pieces[tank].body;
        let mass = world.physics.bodies[handle].mass();
        world.tanks[0].track_travel = [1.23, 2.07];
        world.physics.bodies[handle].set_rotation(Rotation::new(0.7), true);
        world.physics.bodies[handle].set_linvel(Vector::new(3.0, 1.0), true);
        world.physics.bodies[handle].set_angvel(0.8, true);
        world.destroy_tank(tank, &mut vec![]);
        let pieces: Vec<_> = world
            .pieces
            .iter()
            .filter(|p| p.fracture_parent == Some(tank))
            .collect();
        assert_eq!(pieces.len(), 10);
        let appearances: Vec<_> = world.bodies().filter_map(|b| b.tank_fragment).collect();
        assert_eq!(appearances.len(), 10);
        for (index, part) in appearances.iter().enumerate() {
            assert_eq!(part.part as usize, index);
            assert_eq!(
                part.tread_phase,
                [1.23_f32, 2.07][usize::from(part.local_offset.y > 0.0)].rem_euclid(0.18)
            );
        }
        let wreck_mass: f32 = pieces
            .iter()
            .map(|p| world.physics.bodies[p.body].mass())
            .sum();
        assert!((wreck_mass - mass).abs() < 0.01);
        let momentum: Vector = pieces
            .iter()
            .map(|p| {
                let body = &world.physics.bodies[p.body];
                assert!((body.angvel() - 0.8).abs() < 0.001);
                body.linvel() * body.mass()
            })
            .sum();
        assert!(momentum.distance(Vector::new(3.0, 1.0) * mass) < 0.01);
        for (i, a) in pieces.iter().enumerate() {
            for b in &pieces[i + 1..] {
                assert_ne!(a.body, b.body);
                let (a, b) = (
                    &world.physics.colliders[a.collider],
                    &world.physics.colliders[b.collider],
                );
                assert!(
                    !rapier2d::parry::query::intersection_test(
                        a.position(),
                        a.shape(),
                        b.position(),
                        b.shape()
                    )
                    .unwrap()
                );
            }
        }
    }

    #[test]
    fn wreck_collides_with_walls_then_fades_and_retires_before_respawn() {
        let mut world = World::from_level(&Level::default(), Team::Blue).unwrap();
        let tank = world.launcher;
        let start = world.launcher_position();
        world.physics.bodies[world.pieces[tank].body].set_linvel(Vector::X * 4.0, true);
        world.destroy_tank(tank, &mut vec![]);
        let wall_x = start.x + 1.8;
        world.add_box(
            Vector::new(wall_x, start.y),
            Vector::new(0.15, 2.0),
            Material::Anchor,
            true,
        );
        for _ in 0..240 {
            world.step(&[]);
        }
        let fragments: Vec<_> = world
            .bodies()
            .filter(|b| world.fracture_parent(b.id) == Some(tank))
            .collect();
        assert_eq!(fragments.len(), 10);
        for fragment in fragments {
            assert!((fragment.opacity - 0.5).abs() < 0.02);
            let piece = &world.pieces[fragment.id];
            assert!(
                world.physics.colliders[piece.collider]
                    .compute_aabb()
                    .maxs
                    .x
                    <= wall_x - 0.15 + 0.04
            );
        }
        for _ in 0..65 {
            world.step(&[]);
        }
        for piece in world
            .pieces
            .iter()
            .filter(|p| p.fracture_parent == Some(tank))
        {
            assert!(!piece.active);
            assert!(world.physics.bodies.get(piece.body).is_none());
            assert!(world.physics.colliders.get(piece.collider).is_none());
        }
        assert!(!world.tank_is_alive(tank));
        for _ in 0..70 {
            world.step(&[]);
        }
        assert!(world.tank_is_alive(tank));
    }
}
