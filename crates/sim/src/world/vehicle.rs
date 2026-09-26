use super::*;

const TRACK_OFFSET: f32 = 0.65;

#[cfg(test)]
mod elevation_tests {
    use super::*;
    use crate::{
        TerrainKind,
        level::{Level, LevelItem, Team},
    };

    #[test]
    fn elevation_edges_strongly_slow_both_climbs_and_descents() {
        let level = Level {
            items: vec![
                LevelItem::Spawn {
                    team: Team::Blue,
                    position: [0.0, 0.0],
                    angle: 0.0,
                },
                LevelItem::Terrain {
                    terrain: TerrainKind::Elevation,
                    min: [0.0, -5.0],
                    max: [10.0, 5.0],
                },
            ],
            ..Level::default()
        };
        for direction in [1.0, -1.0] {
            let mut w = World::from_level(&level, Team::Blue).unwrap();
            let handle = w.pieces[w.launcher].body;
            w.physics.bodies[handle].set_linvel(Vector::new(direction * 6.5, 0.0), true);
            w.drive_tracks([direction; 2], TICK_DT);
            let speed = w.physics.bodies[handle].linvel().x * direction;
            assert!(speed < 6.5, "edge crossing: {speed}");
            for _ in 0..120 {
                w.drive_tracks([direction; 2], TICK_DT);
            }
            let edge_speed = w.physics.bodies[handle].linvel().x.abs();
            assert!(
                (edge_speed - 6.5 * 0.25).abs() < 0.01,
                "edge speed: {edge_speed}"
            );
            assert!(w.physics.bodies[handle].angvel().abs() < 0.001);
            w.physics.bodies[handle].set_translation(Vector::new(5.0, 0.0), true);
            w.physics.bodies[handle].set_linvel(Vector::new(direction * 6.5, 0.0), true);
            w.drive_tracks([direction; 2], TICK_DT);
            assert!(
                (w.physics.bodies[handle].linvel().x.abs() - 6.5).abs() < 0.001,
                "plateau has normal speed"
            );
        }
    }
}

/// Hull and track cuboids (offset, half extents), shared with replica sight.
pub(super) fn tank_colliders() -> [(Vector, Vector); 3] {
    [
        (Vector::ZERO, Vector::new(0.95, 0.48)),
        (Vector::new(0.0, -TRACK_OFFSET), Vector::new(1.08, 0.17)),
        (Vector::new(0.0, TRACK_OFFSET), Vector::new(1.08, 0.17)),
    ]
}

impl World {
    pub(super) fn add_tank(&mut self) -> usize {
        let position = Vector::new(-7.0, 0.0);
        let [(_, hull), tracks @ ..] = tank_colliders();
        let (body, collider) = self.physics.insert(
            RigidBodyBuilder::dynamic()
                .translation(position)
                .ccd_enabled(true),
            ColliderBuilder::cuboid(hull.x, hull.y)
                .collision_groups(CollisionLayer::Tank.groups())
                .mass(3_800.0)
                .friction(0.8)
                .restitution(0.0),
        );
        for (offset, half) in tracks {
            self.physics.insert_collider(
                ColliderBuilder::cuboid(half.x, half.y)
                    .collision_groups(CollisionLayer::Tank.groups())
                    .translation(offset)
                    .mass(350.0)
                    .friction(0.85)
                    .restitution(0.0),
                Some(body),
            );
        }
        let id = self.pieces.len();
        for &c in self.physics.bodies[body].colliders() {
            self.physics.colliders[c].user_data = (id + 1) as u128;
        }
        self.pieces.push(Piece::new(
            id,
            body,
            collider,
            Shape::Box {
                half: Vector::new(1.08, 0.82),
            },
            Material::Launcher,
            position,
        ));
        id
    }

    pub fn track_travel(&self) -> [f32; 2] {
        self.track_travel
    }

    pub fn track_speeds(&self) -> [f32; 2] {
        if let Some(frame) = &self.replica {
            return frame.tracks;
        }
        let body = &self.physics.bodies[self.pieces[self.launcher].body];
        let forward = *body.rotation() * Vector::X;
        [-TRACK_OFFSET, TRACK_OFFSET].map(|y| {
            body.velocity_at_point(body.position().transform_point(Vector::new(0.0, y)))
                .dot(forward)
        })
    }

    #[cfg(test)]
    pub(super) fn drive_tracks(&mut self, throttles: [f32; 2], dt: f32) {
        self.drive_tank(self.launcher, throttles, dt);
    }

    pub(super) fn drive_tank(&mut self, id: usize, throttles: [f32; 2], dt: f32) {
        if !self.tank_is_alive(id) {
            return;
        }
        let pose = *self.physics.bodies[self.pieces[id].body].position();
        // A short transition band at exposed elevation edges. Sampling the
        // resolved terrain avoids slopes between touching/overlapping plateaus.
        let height = |x| {
            f32::from(
                self.terrain_at(pose.transform_point(Vector::new(x, 0.0)))
                    == crate::TerrainKind::Elevation,
            )
        };
        let edge = (height(1.25) - height(-1.25)).abs();
        let surfaces = [-TRACK_OFFSET, TRACK_OFFSET].map(|y| {
            [-0.78, 0.78].map(|x| {
                self.terrain_at(pose.transform_point(Vector::new(x, y)))
                    .traction()
            })
        });
        let body = &mut self.physics.bodies[self.pieces[id].body];
        if body.is_sleeping() && throttles == [0.0; 2] {
            return;
        }
        let forward = *body.rotation() * Vector::X;
        let side = Vector::new(-forward.y, forward.x);
        // Limit the requested track-speed difference, not angular velocity.
        // Collisions and contact forces still determine the actual rotation.
        let requested: [f32; 2] = std::array::from_fn(|i| {
            throttles[i]
                * self.tank_speed_limit
                * (surfaces[i][0][0] + surfaces[i][1][0])
                * 0.5
                * (1.0 - 0.75 * edge)
        });
        let difference = (requested[0] - requested[1]).abs();
        let steering_scale = if difference > 1.64 {
            1.64 / difference
        } else {
            1.0
        };
        // Scale both requests together; an uncommanded track stays braked.
        let targets = requested.map(|speed| speed * steering_scale);
        let patch_mass = body.mass() * 0.25;
        let braking = throttles == [0.0; 2];
        let steering = (throttles[0] - throttles[1]).abs() > 0.01;
        let mut impulses = Vec::with_capacity(4);
        for (i, y) in [-TRACK_OFFSET, TRACK_OFFSET].into_iter().enumerate() {
            let midpoint = body.position().transform_point(Vector::new(0.0, y));
            let travel = body.velocity_at_point(midpoint).dot(forward) * dt;
            if id == self.launcher {
                self.track_travel[i] += travel;
            }
            if let Some(tank) = self.tanks.iter_mut().find(|t| t.id == id) {
                tank.track_travel[i] += travel;
            }
            for (j, x) in [-0.78, 0.78].into_iter().enumerate() {
                let [_, drive, grip, damping] = surfaces[i][j];
                let point = body.position().transform_point(Vector::new(x, y));
                let velocity = body.velocity_at_point(point);
                let speed = velocity.dot(forward);
                let is_braking = targets[i].abs() < 0.01 || speed * (targets[i] - speed) < 0.0;
                // Fixed engine force gives about 1.6 m/s² acceleration. Brakes
                // have a higher force budget, rather than a reversible motor drag.
                let force_limit = if is_braking {
                    6_500.0 * grip * (1.0 + edge)
                } else {
                    1_800.0 * drive
                };
                let longitudinal =
                    ((targets[i] - speed) * patch_mass * 20.0).clamp(-force_limit, force_limit);
                // Front/rear lateral contacts generate scrub torque, absent from
                // the old centreline-only model. A turn allows controlled slip.
                let lateral_gain = if steering && !braking { 3.0 } else { 24.0 };
                let lateral = (-velocity.dot(side) * patch_mass * lateral_gain * damping).clamp(
                    -patch_mass * 9.81 * 0.9 * grip,
                    patch_mass * 9.81 * 0.9 * grip,
                );
                let force = (forward * longitudinal + side * lateral)
                    .clamp_length_max(patch_mass * 9.81 * 0.95 * grip);
                impulses.push((point, force * dt));
            }
        }
        // Evaluate all contact velocities before applying impulses so the
        // update is symmetric under swapping the left and right tracks.
        for (point, impulse) in impulses {
            body.apply_impulse_at_point(impulse, point, true);
        }
    }
}

impl World {
    /// Controlled initial conditions for a repeatable test range.
    pub fn configure_test_tank(
        &mut self,
        mass: f32,
        position: Vector,
        speed: f32,
    ) -> Result<(), String> {
        if !mass.is_finite()
            || !(500.0..=30_000.0).contains(&mass)
            || !position.is_finite()
            || position.x.abs() > 10.5
            || position.y.abs() > 6.0
            || !speed.is_finite()
            || !(0.0..=12.0).contains(&speed)
        {
            return Err("Tank test parameters are outside the supported range.".into());
        }
        let id = self.launcher;
        let h = self.pieces[id].body;
        let old_mass = self.physics.bodies[h].mass();
        for &c in self.physics.bodies[h].colliders() {
            let collider = &mut self.physics.colliders[c];
            collider.set_mass(collider.mass() * mass / old_mass);
        }
        let body = &mut self.physics.bodies[h];
        body.recompute_mass_properties_from_colliders(&self.physics.colliders);
        body.set_translation(position, true);
        body.set_linvel(Vector::new(speed, 0.0), true);
        self.pieces[id].previous_position = position;
        self.tank_speed_limit = speed.max(0.1);
        self.physics
            .bodies
            .propagate_modified_body_positions_to_colliders(&mut self.physics.colliders);
        Ok(())
    }

    /// Test-only initial heading; does not change driving or deformation rules.
    pub(crate) fn configure_test_tank_angle(
        &mut self,
        mass: f32,
        position: Vector,
        speed: f32,
        angle: f32,
    ) -> Result<(), String> {
        if !angle.is_finite() || angle.abs() > std::f32::consts::PI {
            return Err("Tank approach angle must be between -180 and 180 degrees.".into());
        }
        self.configure_test_tank(mass, position, speed)?;
        if angle != 0.0 {
            let handle = self.pieces[self.launcher].body;
            let body = &mut self.physics.bodies[handle];
            body.set_position(Pose::new(position, angle), true);
            body.set_linvel(Vector::new(angle.cos(), angle.sin()) * speed, true);
            self.pieces[self.launcher].previous_angle = angle;
            self.physics
                .bodies
                .propagate_modified_body_positions_to_colliders(&mut self.physics.colliders);
            for &own in self.physics.bodies[handle].colliders() {
                let a = &self.physics.colliders[own];
                for (_, b) in self
                    .physics
                    .colliders
                    .iter()
                    .filter(|(_, b)| b.parent() != Some(handle))
                {
                    if a.collision_groups().test(b.collision_groups())
                        && rapier2d::parry::query::intersection_test(
                            a.position(),
                            a.shape(),
                            b.position(),
                            b.shape(),
                        )
                        .unwrap_or(true)
                    {
                        return Err("Tank starts inside an object or backstop. Change angle/offset or disable the backstop.".into());
                    }
                }
            }
        }
        Ok(())
    }
}
