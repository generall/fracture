//! Car steering: throttle drives the hull; steering sets a rolling turn radius.
use super::*;

impl World {
    pub(super) fn drive_scout(
        &mut self,
        id: usize,
        inputs: [f32; 2],
        surfaces: [[[f32; 4]; 2]; 2],
        edge: f32,
        dt: f32,
    ) {
        // Scout controls encode independent throttle/steering in the existing
        // two-channel drive command. Steering never reduces throttle.
        let throttle = (inputs[0] + inputs[1]).clamp(-1.0, 1.0);
        let steer = (inputs[0] - inputs[1]).clamp(-1.0, 1.0);
        let surface: [f32; 4] =
            std::array::from_fn(|i| surfaces.iter().flatten().map(|s| s[i]).sum::<f32>() * 0.25);
        let [speed_scale, drive, grip, damping] = surface;
        let limit = self.tank_speed_limit
            * (VehicleKind::Scout.speed() / 6.5)
            * speed_scale
            * (1.0 - 0.75 * edge);
        let body = &mut self.physics.bodies[self.pieces[id].body];
        if body.is_sleeping() && throttle == 0.0 {
            return;
        }
        let forward = *body.rotation() * Vector::X;
        let velocity = body.linvel();
        let speed = velocity.dot(forward);
        let mass = body.mass();
        let target = throttle * limit;
        let braking = throttle == 0.0 || speed * (target - speed) < 0.0;
        // Size engine/brake forces for the chassis mass. Acceleration stays
        // responsive, while the lighter car has less sustained pushing force.
        let force_limit = if braking {
            mass * (26_000.0 / 1800.0) * grip * (1.0 + edge)
        } else {
            mass * 4.2 * drive
        };
        let drive_impulse = ((target - speed) * mass * 20.0).clamp(-force_limit, force_limit) * dt;

        // A bicycle steering model: no rolling speed means no requested turn;
        // reversing naturally reverses the effect of the front wheels.
        let yaw = (speed / 1.56 * (steer * 0.5).tan()).clamp(-1.4, 1.4);
        // Effective inverse inertia can still be zero on the first substep
        // after respawn; the configured local mass properties remain valid.
        let inertia = body.mass_properties().local_mprops.principal_inertia();
        let turn_impulse = ((yaw - body.angvel()) * inertia * 12.0)
            .clamp(-inertia * 5.0 * grip, inertia * 5.0 * grip)
            * dt;

        // Tire grip redirects momentum without the inner-track brake or scrub
        // drag of a tank. Keep its length so steering alone doesn't shed speed.
        // Apply bounded impulses; collisions still resolve in the physics step.
        let rolling_speed = velocity.length();
        let direction = forward * if speed < 0.0 { -1.0 } else { 1.0 };
        let error = velocity.perp_dot(direction).atan2(velocity.dot(direction));
        let turn = (error * (1.0 - (-24.0 * damping * dt).exp())).clamp(
            -20.0 * grip * dt / rolling_speed.max(0.1),
            20.0 * grip * dt / rolling_speed.max(0.1),
        );
        let redirected = Vector::from_angle(turn).rotate(velocity);
        let travel = [-TRACK_OFFSET, TRACK_OFFSET].map(|y| {
            body.velocity_at_point(
                body.position()
                    .transform_point(Vector::new(0.0, y * VehicleKind::Scout.size().y)),
            )
            .dot(forward)
                * dt
        });
        body.apply_impulse(
            (redirected - velocity) * mass + forward * drive_impulse,
            true,
        );
        body.apply_torque_impulse(turn_impulse, true);
        for (i, distance) in travel.into_iter().enumerate() {
            if id == self.launcher {
                self.track_travel[i] += distance;
            }
            if let Some(tank) = self.tanks.iter_mut().find(|t| t.id == id) {
                tank.track_travel[i] += distance;
            }
        }
    }
}
