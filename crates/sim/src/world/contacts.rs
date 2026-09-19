use super::*;

pub(super) fn hooks_for(response: Option<crate::SoftBounce>) -> ActiveHooks {
    if response.is_some() {
        ActiveHooks::MODIFY_SOLVER_CONTACTS
    } else {
        ActiveHooks::empty()
    }
}

/// Restitution depends on incoming contact speed, including rotational velocity.
/// The solver still supplies the collision impulse; no separation impulse is added.
pub(super) struct ReboundHooks<'a>(pub &'a [Piece]);

impl PhysicsHooks for ReboundHooks<'_> {
    fn modify_solver_contacts(&self, context: &mut ContactModificationContext) {
        let velocity = |handle: Option<RigidBodyHandle>, point: Vector| {
            handle.map_or(Vector::ZERO, |h| context.bodies[h].velocity_at_point(point))
        };
        let closing = context
            .solver_contacts
            .iter()
            .map(|c| {
                (velocity(context.rigid_body1, c.anchor1)
                    - velocity(context.rigid_body2, c.anchor2))
                .dot(*context.normal)
            })
            .fold(0.0_f32, f32::max);
        let coefficient = |handle: ColliderHandle| {
            let c = &context.colliders[handle];
            let soft = c
                .user_data
                .checked_sub(1)
                .and_then(|id| usize::try_from(id).ok())
                .and_then(|id| self.0.get(id))
                .and_then(|p| p.soft_bounce);
            soft.map_or(c.restitution(), |b| b.coefficient(c.restitution(), closing))
        };
        // All authored and fixture colliders use Rapier's average combine rule.
        *context.restitution =
            (coefficient(context.collider1) + coefficient(context.collider2)) * 0.5;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restitution_uses_incoming_speed_without_adding_collision_energy() {
        for direction in [-1.0, 1.0] {
            for speed in [8.0, 24.0, 40.0] {
                let mut asset = crate::LibraryPreset::Barrel.asset();
                // A flat impact fixture isolates restitution from rim rotation.
                asset.name = "Contact test plate".into();
                asset.circular = false;
                asset.friction = 0.0;
                asset.outline = vec![[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]];
                asset.layout = None;
                asset.components.plasticity.as_mut().unwrap().yield_stress = 1e9;
                let mut world = World::from_object(&asset).unwrap();
                world.add_box(
                    Vector::new(3.0 + direction * 0.85, 0.0),
                    Vector::new(0.2, 2.0),
                    Material::Anchor,
                    true,
                );
                let id = world
                    .pieces
                    .iter()
                    .position(|p| p.object.is_some())
                    .unwrap();
                let handle = world.pieces[id].body;
                world.physics.bodies[handle].set_linvel(Vector::X * direction * speed, true);
                let before = world.physics.bodies[handle].kinetic_energy();
                for _ in 0..60 {
                    world.step(&[]);
                }
                let body = &world.physics.bodies[handle];
                let expected = (asset
                    .soft_bounce
                    .unwrap()
                    .coefficient(asset.restitution, speed)
                    + 0.08)
                    * 0.5;
                let rebound = body.linvel().x / (-direction * speed);
                // The finite contact solver can dissipate additional motion.
                assert!(
                    rebound >= expected * 0.7 && rebound <= expected + 0.01,
                    "speed {speed}, direction {direction}: {:?}, expected {expected}",
                    body.linvel()
                );
                assert!(body.kinetic_energy() <= before + 0.001);
            }
        }
    }
}
