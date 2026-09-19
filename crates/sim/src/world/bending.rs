//! Elasto-plastic creases: hold a rest angle below yield, retain permanent bends above it.
use super::*;
use std::collections::BTreeMap;

// Only a tiny elastic deflection is allowed before the rest angle yields.
const ELASTIC_ANGLE: f32 = 0.002;
fn wrap(angle: f32) -> f32 {
    angle.sin().atan2(angle.cos())
}

impl World {
    pub(super) fn bending_torque(&self, bond: &Bond) -> Option<f32> {
        let (a, b) = (&self.pieces[bond.a], &self.pieces[bond.b]);
        if a.object.is_some() && a.object == b.object && a.grain != b.grain {
            self.components_for(bond.a).bending_torque
        } else {
            None
        }
    }

    pub(super) fn rebuild_bending_joints(&mut self) {
        let mut previous = BTreeMap::new();
        for handle in self.bending_joints.drain(..) {
            if let Some(joint) = self.physics.impulse_joints.remove(handle, true) {
                previous.insert(
                    (
                        joint.body1().into_raw_parts(),
                        joint.body2().into_raw_parts(),
                    ),
                    joint.data,
                );
            }
        }
        let mut joins: BTreeMap<_, (Vector, f32, f32)> = BTreeMap::new();
        for bond in self.bonds.iter().filter(|b| b.active) {
            let Some(torque) = self.bending_torque(bond) else {
                continue;
            };
            let (a, b) = (self.pieces[bond.a].body, self.pieces[bond.b].body);
            if a == b {
                continue;
            }
            let key = if a.into_raw_parts() < b.into_raw_parts() {
                (a.into_raw_parts(), b.into_raw_parts())
            } else {
                (b.into_raw_parts(), a.into_raw_parts())
            };
            let point = self.piece_pose(bond.a).transform_point(bond.anchor_a);
            let entry = joins.entry(key).or_insert((Vector::ZERO, 0.0, torque));
            entry.0 += point * bond.width;
            entry.1 += bond.width;
        }
        for ((a, b), (point, width, torque)) in joins {
            let a = RigidBodyHandle::from_raw_parts(a.0, a.1);
            let b = RigidBodyHandle::from_raw_parts(b.0, b.1);
            let point = point / width;
            // Keep the rest angle and attachment points through unrelated topology
            // rebuilds, so numerical motion never becomes permanent deformation.
            if let Some(data) = previous.remove(&(a.into_raw_parts(), b.into_raw_parts())) {
                self.bending_joints
                    .push(self.physics.insert_impulse_joint(a, b, data));
                continue;
            }
            let rest = wrap(
                self.physics.bodies[b].rotation().angle()
                    - self.physics.bodies[a].rotation().angle(),
            );
            let stiffness = torque / ELASTIC_ANGLE;
            let joint = RevoluteJointBuilder::new()
                .local_anchor1(
                    self.physics.bodies[a]
                        .position()
                        .inverse_transform_point(point),
                )
                .local_anchor2(
                    self.physics.bodies[b]
                        .position()
                        .inverse_transform_point(point),
                )
                .contacts_enabled(false)
                .motor_model(MotorModel::ForceBased)
                .motor_position(rest, stiffness, 2.0 * stiffness.sqrt())
                .motor_max_force(torque);
            self.bending_joints
                .push(self.physics.insert_impulse_joint(a, b, joint));
        }
    }

    pub(super) fn update_plastic_bends(&mut self) {
        for &handle in &self.bending_joints {
            let Some(joint) = self.physics.impulse_joints.get_mut(handle, false) else {
                continue;
            };
            let angle = wrap(
                self.physics.bodies[joint.body2()].rotation().angle()
                    - self.physics.bodies[joint.body1()].rotation().angle(),
            );
            let motor = joint.data.motor(JointAxis::AngX).unwrap();
            let error = wrap(angle - motor.target_pos);
            // A poorly converged multi-section chain can have angular error
            // without ever reaching the yield moment. Do not bake that solver
            // error into the material's rest shape.
            let yield_impulse = motor.max_force * self.physics.integration_parameters.dt;
            if error.abs() > ELASTIC_ANGLE && motor.impulse.abs() >= yield_impulse * 0.999 {
                // Return mapping for perfect plasticity: only the excess over
                // elastic yield becomes permanent. Below yield the rest stays put.
                let rest = wrap(angle - error.signum() * ELASTIC_ANGLE);
                let (stiffness, damping) = (motor.stiffness, motor.damping);
                joint
                    .data
                    .set_motor_position(JointAxis::AngX, rest, stiffness, damping);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rest_angles(world: &World) -> Vec<f32> {
        world
            .bending_joints
            .iter()
            .map(|&h| {
                world
                    .physics
                    .impulse_joints
                    .get(h)
                    .unwrap()
                    .data
                    .motor(JointAxis::AngX)
                    .unwrap()
                    .target_pos
            })
            .collect()
    }

    #[test]
    fn fence_holds_its_shape_until_yield_and_keeps_the_permanent_bend() {
        let experiment = crate::experiment::Experiment::fence_comparison();
        let mut asset = experiment
            .variants
            .iter()
            .find(|v| v.object.layout.as_ref().unwrap().sites.len() == 9)
            .unwrap()
            .object
            .clone();
        asset.components.bending_torque = Some(200.0);
        let mut world = World::from_object(&asset).unwrap();
        let initial = rest_angles(&world);
        assert_eq!(
            initial.len(),
            8,
            "creased spans follow the authored resolution"
        );
        let before: Vec<_> = world
            .pieces
            .iter()
            .enumerate()
            .filter(|(_, p)| p.object == Some(0))
            .map(|(i, _)| (i, world.piece_pose(i)))
            .collect();
        for _ in 0..360 {
            world.step(&[]);
        }
        assert_eq!(
            initial,
            rest_angles(&world),
            "no spontaneous plastic deformation"
        );
        for (id, pose) in &before {
            assert!(world.piece_pose(*id).translation.distance(pose.translation) < 1e-4);
        }
        // A small sub-yield torque must not accumulate permanent curvature.
        let handle = world.bending_joints[1];
        let body = world.physics.impulse_joints.get(handle).unwrap().body2();
        world.physics.bodies[body].add_torque(1.0, true);
        for _ in 0..240 {
            world.step(&[]);
        }
        assert_eq!(
            initial,
            rest_angles(&world),
            "sub-yield load must not creep"
        );
        world.physics.bodies[body].reset_torques(true);
        world
            .configure_test_tank(4500.0, Vector::new(1.42, 0.0), 3.0)
            .unwrap();
        for _ in 0..600 {
            world.step(&[Command::Tracks {
                left: 1.0,
                right: 1.0,
            }]);
            if world.launcher_position().x > 6.0 {
                break;
            }
        }
        assert!(
            rest_angles(&world)
                .iter()
                .zip(&initial)
                .any(|(a, b)| wrap(a - b).abs() > 0.3),
            "tank impact must leave a permanent fold"
        );
        // Let impact momentum settle, then verify the unloaded rest shape stays.
        for _ in 0..360 {
            world.step(&[]);
        }
        let settled = rest_angles(&world);
        for _ in 0..360 {
            world.step(&[]);
        }
        let final_angles = rest_angles(&world);
        println!("rest angles: initial {initial:?}, settled {settled:?}, final {final_angles:?}");
        for (a, b) in settled.iter().zip(&final_angles) {
            assert!(
                wrap(a - b).abs() < 0.005,
                "unloaded fence must hold its fold"
            );
        }
    }

    #[test]
    fn yielding_fence_folds_without_significantly_turning_the_tank() {
        let experiment = crate::experiment::Experiment::fence_comparison();
        // Keep the soft-material regression independent of comparison tuning.
        let mut asset = experiment
            .variants
            .iter()
            .find(|v| v.object.layout.as_ref().unwrap().sites.len() == 3)
            .unwrap()
            .object
            .clone();
        asset.components.bending_torque = Some(200.0);
        for speed in [0.8, 3.0, 6.5] {
            let mut world = World::from_object(&asset).unwrap();
            world
                .configure_test_tank(4500.0, Vector::new(1.42, 0.0), speed)
                .unwrap();
            let mut heading = 0.0_f32;
            let mut drift = 0.0_f32;
            for _ in 0..720 {
                world.step(&[Command::Tracks {
                    left: 1.0,
                    right: 1.0,
                }]);
                heading = heading.max(
                    world
                        .piece_pose(world.launcher)
                        .rotation
                        .angle()
                        .abs()
                        .to_degrees(),
                );
                drift = drift.max(world.launcher_position().y.abs());
                if world.launcher_position().x > 6.0 {
                    break; // Measure fence contact, not the later arena-wall impact.
                }
            }
            println!(
                "speed {speed}: x {}, heading {heading}, drift {drift}",
                world.launcher_position().x
            );
            assert!(
                world.launcher_position().x > 5.0,
                "tank must pass the fence"
            );
            assert!(heading < 3.0, "heading change: {heading}");
            assert!(drift < 0.5, "lateral drift: {drift}");
            assert_eq!(world.broken, 0);
            let pieces = world
                .pieces
                .iter()
                .filter(|p| p.active && p.object == Some(0))
                .count();
            world.topology_changed = true;
            for _ in 0..600 {
                world.step(&[]);
            }
            assert_eq!(
                world
                    .pieces
                    .iter()
                    .filter(|p| p.active && p.object == Some(0))
                    .count(),
                pieces,
                "connected fence sections must not expire as rubble"
            );
        }
    }
}
