use super::*;
mod scout;

const TRACK_OFFSET: f32 = 0.65;

/// Hull and track cuboids (offset, half extents), shared with replica sight.
pub(super) fn tank_colliders() -> [(Vector, Vector); 3] {
    [
        (Vector::ZERO, Vector::new(0.95, 0.48)),
        (Vector::new(0.0, -TRACK_OFFSET), Vector::new(1.08, 0.17)),
        (Vector::new(0.0, TRACK_OFFSET), Vector::new(1.08, 0.17)),
    ]
}

impl World {
    pub fn vehicle_kind(&self, id: usize) -> VehicleKind {
        self.tanks()
            .find(|t| t.id == id)
            .map_or(VehicleKind::Tank, |t| t.vehicle)
    }
    pub fn player_vehicle(&self) -> VehicleKind {
        self.vehicle_kind(self.launcher)
    }
    pub fn main_reload_interval(&self) -> f32 {
        if self.player_vehicle() == VehicleKind::Tank {
            self.last_ammunition().interval()
        } else {
            self.player_vehicle().reload()
        }
    }
    /// Sandbox redeployment: register experiment vehicles before offering a choice.
    pub fn request_player_deployment(&mut self) {
        if self.replica.is_some() {
            return;
        }
        let id = self.launcher;
        if !self.tanks.iter().any(|t| t.id == id) {
            let pose = self.piece_pose(id);
            self.register_tank(id, self.team, pose.translation, pose.rotation.angle());
        }
        self.require_vehicle_choice(id);
    }
    pub fn player_choosing_vehicle(&self) -> bool {
        self.tanks.iter().any(|t| {
            t.id == self.launcher && t.requires_choice && t.health <= 0.0 && t.respawn.is_some()
        })
    }
    /// Enable human deployment choices, including the first spawn.
    pub fn require_vehicle_choice(&mut self, id: usize) {
        if let Some(tank) = self.tanks.iter_mut().find(|t| t.id == id) {
            tank.requires_choice = true;
            tank.choice = None;
            tank.health = 0.0;
            tank.respawn = Some(0.0);
            self.physics.bodies[self.pieces[id].body].set_enabled(false);
        }
    }
    pub(super) fn choose_vehicle(&mut self, id: usize, kind: VehicleKind) {
        if let Some(tank) = self.tanks.iter_mut().find(|t| t.id == id)
            && tank.requires_choice
            && tank.health <= 0.0
            && tank.respawn.is_some()
        {
            tank.choice = Some(kind);
        }
    }
    pub(super) fn apply_vehicle(&mut self, id: usize, kind: VehicleKind) {
        if self.vehicle_kind(id) == kind {
            return;
        }
        let handle = self.pieces[id].body;
        let colliders = self.physics.bodies[handle].colliders().to_vec();
        for (i, ((offset, half), handle)) in tank_colliders().into_iter().zip(colliders).enumerate()
        {
            let collider = &mut self.physics.colliders[handle];
            let half = half * kind.size();
            collider.set_shape(SharedShape::cuboid(half.x, half.y));
            collider.set_translation_wrt_parent(offset * kind.size());
            collider.set_mass(
                kind.mass()
                    * if i == 0 {
                        3800.0 / 4500.0
                    } else {
                        350.0 / 4500.0
                    },
            );
        }
        self.physics.bodies[handle]
            .recompute_mass_properties_from_colliders(&self.physics.colliders);
        self.pieces[id].shape = Shape::Box {
            half: Vector::new(1.08, 0.82) * kind.size(),
        };
        self.pieces[id].shape_revision = self.pieces[id].shape_revision.wrapping_add(1);
        if let Some(tank) = self.tanks.iter_mut().find(|t| t.id == id) {
            tank.vehicle = kind;
        }
        self.sight_cache = None;
        self.physics
            .bodies
            .propagate_modified_body_positions_to_colliders(&mut self.physics.colliders);
    }

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

    fn mortar_push_force(&self, id: usize, throttles: [f32; 2], dt: f32) -> f32 {
        let throttle = (throttles[0] + throttles[1]) * 0.5;
        if throttle.abs() < 0.01 {
            return 0.0;
        }
        let handle = self.pieces[id].body;
        let direction = self.piece_pose(id).rotation * Vector::X * throttle.signum();
        let mut impulse = 0.0;
        for pair in self.physics.contact_pairs() {
            if !pair.has_any_active_contact() {
                continue;
            }
            // Fracture can replace colliders after the prior physics step.
            let (Some(a), Some(b)) = (
                self.physics.colliders.get(pair.collider1),
                self.physics.colliders.get(pair.collider2),
            ) else {
                continue;
            };
            let (other, sign) = if a.parent() == Some(handle) {
                (b, -1.0)
            } else if b.parent() == Some(handle) {
                (a, 1.0)
            } else {
                continue;
            };
            let destructible = (other.user_data as usize)
                .checked_sub(1)
                .and_then(|i| self.pieces.get(i))
                .is_some_and(|p| p.active && p.destructible());
            if !destructible {
                continue;
            }
            for manifold in &pair.manifolds {
                let resistance = -(manifold.data.normal * sign).dot(direction);
                if resistance > 0.0 {
                    impulse += manifold
                        .points
                        .iter()
                        .map(|c| c.data.impulse.max(0.0))
                        .sum::<f32>()
                        * resistance;
                }
            }
        }
        // The prior solver impulse is the load the engine must overcome.
        // Compensation vanishes immediately when the obstacle stops resisting.
        (impulse / dt).min(220_000.0)
    }

    pub(super) fn drive_tank(&mut self, id: usize, throttles: [f32; 2], dt: f32) {
        if !self.tank_is_alive(id) {
            return;
        }
        let kind = self.vehicle_kind(id);
        let track_offset = TRACK_OFFSET * kind.size().y;
        let drive_scale = match kind {
            VehicleKind::Scout => 1.05,
            VehicleKind::Tank => 1.0,
            VehicleKind::Mortar => 2.2,
        };
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
        let surfaces = [-track_offset, track_offset].map(|y| {
            [-0.78, 0.78].map(|x| {
                self.terrain_at(pose.transform_point(Vector::new(x, y)))
                    .traction()
            })
        });
        if kind == VehicleKind::Scout {
            self.drive_scout(id, throttles, surfaces, edge, dt);
            return;
        }
        let pushing = if kind == VehicleKind::Mortar {
            self.mortar_push_force(id, throttles, dt)
        } else {
            0.0
        };
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
                * (kind.speed() / 6.5)
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
        for (i, y) in [-track_offset, track_offset].into_iter().enumerate() {
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
                // The regular tank engine gives about 1.6 m/s² acceleration. Brakes
                // have a higher force budget, rather than a reversible motor drag.
                let force_limit = if is_braking {
                    6_500.0 * grip * (1.0 + edge)
                } else {
                    1_800.0 * drive * drive_scale
                };
                let gain = if kind == VehicleKind::Mortar && !is_braking {
                    1.5
                } else {
                    20.0
                };
                let mut longitudinal =
                    ((targets[i] - speed) * patch_mass * gain).clamp(-force_limit, force_limit);
                if kind == VehicleKind::Mortar && !is_braking {
                    // Compensate actual wall resistance, rather than giving the
                    // unloaded hull the full breaching force at every start.
                    longitudinal = (longitudinal + targets[i].signum() * pushing * 0.25)
                        .clamp(-55_000.0 * drive, 55_000.0 * drive);
                }
                // Front/rear lateral contacts generate scrub torque, absent from
                // the old centreline-only model. A turn allows controlled slip.
                let lateral_gain = if steering && !braking { 3.0 } else { 24.0 };
                let lateral = (-velocity.dot(side) * patch_mass * lateral_gain * damping).clamp(
                    -patch_mass * 9.81 * 0.9 * grip,
                    patch_mass * 9.81 * 0.9 * grip,
                );
                let traction = if kind == VehicleKind::Mortar {
                    1.4
                } else {
                    0.95
                };
                let force = (forward * longitudinal + side * lateral)
                    .clamp_length_max(patch_mass * 9.81 * traction * grip);
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
