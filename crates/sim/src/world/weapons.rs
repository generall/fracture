use super::*;
use rapier2d::parry::query::{ShapeCastOptions, cast_shapes};

pub(super) struct BlastPulse {
    pub(super) piece: usize,
    pub(super) local_point: Vector,
    direction: Vector,
    impulse_left: f32,
    energy_left: f32,
    time_left: f32,
    event: u64,
}

impl World {
    // Sweep against the live collider poses, including their linear velocities.
    // This avoids stale broad-phase queries between fixed physics microsteps.
    fn shell_hit(
        &self,
        position: Vector,
        velocity: Vector,
        dt: f32,
        radius: f32,
        shooter: usize,
    ) -> Option<(f32, usize, Vector, Vector)> {
        let mut nearest = None;
        let owner = self.pieces[shooter].body;
        for (_, collider) in self.physics.colliders.iter() {
            let Some(id) = self.weapon_target(collider) else {
                continue;
            };
            let Some(handle) = collider.parent() else {
                continue;
            };
            if handle == owner {
                continue;
            }
            let body = &self.physics.bodies[handle];
            let target_velocity = body.linvel();
            let end = position + (velocity - target_velocity) * dt;
            let bounds = collider.compute_aabb();
            let padding = Vector::splat(radius);
            if (position.min(end) - padding).cmpgt(bounds.maxs).any()
                || (position.max(end) + padding).cmplt(bounds.mins).any()
            {
                continue;
            }
            if let Ok(Some(hit)) = cast_shapes(
                &Pose::translation(position.x, position.y),
                velocity,
                &Ball::new(radius),
                collider.position(),
                target_velocity,
                collider.shape(),
                ShapeCastOptions::with_max_time_of_impact(dt),
            ) && nearest.is_none_or(|(time, _, _, _)| hit.time_of_impact < time)
            {
                let point = collider.position().transform_point(hit.witness2)
                    + target_velocity * hit.time_of_impact;
                let normal = collider.position().rotation * hit.normal2;
                nearest = Some((hit.time_of_impact, id, point, normal));
            }
        }
        nearest
    }

    pub(super) fn fire(&mut self, shot: Shot, events: &mut Vec<Event>) {
        self.fire_from(self.launcher, shot, events);
    }

    pub(super) fn fire_from(&mut self, shooter: usize, shot: Shot, events: &mut Vec<Event>) {
        if !self.tank_is_alive(shooter)
            || !shot.target.is_finite()
            || !shot.mass.is_finite()
            || !shot.speed.is_finite()
        {
            return;
        }
        let origin = self.piece_pose(shooter).translation;
        let offset = shot.target - origin;
        if offset.length_squared() < 0.01 {
            return;
        }
        let (barrel, direction) =
            self.shooting_directions(shooter, offset.normalize(), shot.ammunition);
        let reload = if shooter == self.launcher {
            self.reload
        } else {
            self.tanks
                .iter()
                .find(|t| t.id == shooter)
                .map_or(0.0, |t| t.reload)
        };
        let ready = reload
            <= if shot.ammunition.automatic() {
                1e-6
            } else {
                0.0
            };
        if shot.ammunition == Ammunition::Laser {
            if shooter != self.launcher {
                return;
            }
            if ready {
                self.record_aimed_shot(shooter, shot.ammunition);
                self.shots += 1;
                self.reload = shot.ammunition.interval();
                self.last_ammunition = shot.ammunition;
                events.push(Event::Fired {
                    position: origin,
                    velocity: Vector::ZERO,
                });
            }
            // Aim/beam refresh at fixed ticks; material cutting has its own cadence.
            if ready || self.last_ammunition == Ammunition::Laser {
                self.reveal_firing_tank(shooter);
                self.fire_laser(origin, barrel, direction, ready, events);
            }
            return;
        }
        if !ready {
            return;
        }
        // Check the entire barrel, preventing shots through a nearby obstacle.
        if self
            .shell_hit(
                origin,
                barrel * 1_650.0,
                0.001,
                shot.ammunition.radius(),
                shooter,
            )
            .is_some()
        {
            events.push(Event::MuzzleBlocked);
            return;
        }
        let (lo, hi) = shot.ammunition.mass_range();
        let mass = shot.mass.clamp(lo, hi);
        let position = origin + barrel * 1.65;
        let (lo, hi) = shot.ammunition.speed_range();
        let velocity = direction * shot.speed.clamp(lo, hi);
        self.record_aimed_shot(shooter, shot.ammunition);
        self.reveal_firing_tank(shooter);
        self.shells.push(Shell {
            ammunition: shot.ammunition,
            position,
            previous_position: position,
            velocity,
            mass,
            age: 0.0,
            owner: shooter,
        });
        self.physics.bodies[self.pieces[shooter].body].apply_impulse(-velocity * mass, true);
        self.shots += 1;
        if shooter == self.launcher {
            self.reload = shot.ammunition.interval();
            self.last_ammunition = shot.ammunition;
        } else if let Some(tank) = self.tanks.iter_mut().find(|t| t.id == shooter) {
            tank.reload = shot.ammunition.interval();
        }
        events.push(Event::Fired { position, velocity });
    }

    pub(super) fn advance_shells(&mut self, dt: f32, events: &mut Vec<Event>) {
        let shells = std::mem::take(&mut self.shells);
        for mut shell in shells {
            if let Some((_, piece, point, normal)) = self.shell_hit(
                shell.position,
                shell.velocity,
                dt,
                shell.ammunition.radius(),
                shell.owner,
            ) {
                self.sweep_decorations(shell.position, point, shell.ammunition.radius());
                let event = self.next_impact();
                self.hit_tank(
                    piece,
                    shell.velocity,
                    point,
                    0.5 * shell.mass * shell.velocity.length_squared(),
                    shell.ammunition,
                    events,
                );
                self.loads.push(loads::Load {
                    piece,
                    point,
                    impulse: shell.velocity * shell.mass,
                    energy: 0.5 * shell.mass * shell.velocity.length_squared(),
                    apply: true,
                    event,
                });
                // Start pressure rays just outside the surface, including grazing hits.
                let position = point + normal * 0.003;
                match shell.ammunition {
                    Ammunition::HighExplosive => {
                        self.detonate_with_event(position, shell.mass, event, events)
                    }
                    Ammunition::ArmorPiercing => self.blast_with_tank_scale(
                        position,
                        shell.mass * 100_000.0,
                        0.65 * (shell.mass / 8.0).cbrt(),
                        0.15,
                        event,
                        events,
                    ),
                    Ammunition::Rocket => {
                        self.rocket_shock(piece, point, event, events);
                        self.blast_with_tank_scale(
                            position,
                            20_000_000.0,
                            5.5,
                            0.035,
                            event,
                            events,
                        );
                    }
                    _ => events.push(Event::Impacted {
                        position: point,
                        normal,
                    }),
                }
                // The shell is consumed here; there is no bouncing collider.
            } else {
                self.sweep_decorations(
                    shell.position,
                    shell.position + shell.velocity * dt,
                    shell.ammunition.radius(),
                );
                shell.position += shell.velocity * dt;
                shell.age += dt;
                if shell.age
                    < if shell.ammunition == Ammunition::Rocket {
                        8.0
                    } else {
                        3.0
                    }
                {
                    self.shells.push(shell);
                }
            }
        }
    }

    fn rocket_shock(&mut self, hit: usize, impact: Vector, event: u64, events: &mut Vec<Event>) {
        // The warhead couples part of its energy into the struck connected
        // material as an internal shock; external blast rays still obey cover.
        let body = self.pieces[hit].body;
        let object = self.pieces[hit].object;
        let mut budget = 4_000_000.0;
        let mut failed = Vec::new();
        for (id, bond) in self.bonds.iter().enumerate() {
            let a = &self.pieces[bond.a];
            let b = &self.pieces[bond.b];
            if !bond.active
                || !a.active
                || !b.active
                || !a.destructible()
                || !b.destructible()
                || a.body != body
                || b.body != body
                || a.object != object
                || b.object != object
            {
                continue;
            }
            let point = self.piece_pose(bond.a).transform_point(bond.anchor_a);
            let distance = point.distance(impact);
            if distance >= 3.0 {
                continue;
            }
            let components = self.components_for(bond.a);
            if components.plasticity.is_some() || components.bending_torque.is_some() {
                continue;
            }
            // Approximate transient internal pressure (Pa), attenuated through
            // the connected target. Stronger/thicker seams still cost more work.
            let pressure = 8_000_000.0 * (1.0 - distance / 3.0).powi(2);
            let resistance = components.projectile_resistance;
            let limit = bond.strength.shear * bond.strength_scale * resistance;
            let cost = bond.toughness * bond.width * bond.depth * bond.strength_scale * resistance;
            if pressure > limit && cost <= budget {
                budget -= cost;
                self.fracture_work += cost as f64;
                failed.push((id, point, self.piece_pose(bond.a).rotation * bond.normal_a));
            }
        }
        let cells: Vec<_> = self
            .pieces
            .iter()
            .enumerate()
            .filter_map(|(id, p)| {
                if !p.active || !p.destructible() || p.body != body || p.object != object {
                    return None;
                }
                let point = self.piece_pose(id).translation;
                (point.distance(impact) < 3.0).then_some((
                    id,
                    point,
                    p.shape.area() * (1.0 - point.distance(impact) / 3.0),
                ))
            })
            .collect();
        for (id, position, normal) in failed {
            self.fracture_ignition(id);
            self.bonds[id].active = false;
            self.broken += 1;
            self.topology_changed = true;
            events.push(Event::Fractured { position, normal });
        }
        let total: f32 = cells.iter().map(|c| c.2).sum();
        for (piece, point, weight) in cells {
            let energy = budget * weight / total.max(1e-6);
            self.loads.push(loads::Load {
                piece,
                point,
                impulse: (point - impact).normalize_or_zero() * energy / 80.0,
                energy,
                apply: true,
                event,
            });
        }
        if self.topology_changed {
            self.structural_cache.clear();
            self.rebuild_chunks();
        }
    }

    pub(super) fn fire_laser(
        &mut self,
        origin: Vector,
        barrel: Vector,
        _direction: Vector,
        cut: bool,
        events: &mut Vec<Event>,
    ) {
        const RANGE: f32 = 40.0;
        const RADIUS: f32 = 0.30;
        let owner = self.pieces[self.launcher].body;
        // Several bounded cutting operations per pulse carve a wider channel.
        // Recast after each cut so newly exposed material remains shielded.
        let mut damaged_tanks = Vec::new();
        for _ in 0..if cut { 8 } else { 1 } {
            let mut nearest: Option<(f32, usize)> = None;
            for (_, collider) in self.physics.colliders.iter() {
                let Some(id) = self.weapon_target(collider) else {
                    continue;
                };
                if collider.parent() == Some(owner) {
                    continue;
                }
                if let Ok(Some(hit)) = cast_shapes(
                    &Pose::translation(origin.x, origin.y),
                    barrel,
                    &Ball::new(RADIUS),
                    collider.position(),
                    Vector::ZERO,
                    collider.shape(),
                    ShapeCastOptions::with_max_time_of_impact(RANGE),
                ) && nearest.is_none_or(|(old, _)| hit.time_of_impact < old)
                {
                    nearest = Some((hit.time_of_impact, id));
                }
            }
            let distance = nearest.map_or(RANGE, |(d, _)| (d + RADIUS).min(RANGE));
            let end = origin + barrel * distance;
            self.sweep_decorations(origin + barrel * distance.min(1.65), end, RADIUS);
            self.laser_beam = Some(LaserBeam {
                start: origin + barrel * distance.min(1.65),
                end,
                remaining: 0.02,
            });
            if !cut {
                break;
            }
            let Some((_, id)) = nearest else {
                break;
            };
            if self.pieces[id].material == Material::Launcher {
                if !damaged_tanks.contains(&id) {
                    damaged_tanks.push(id);
                    self.damage_tank_from(
                        id,
                        12.0,
                        TankImpact {
                            cause: DestructionCause::Laser,
                            point: end,
                            direction: barrel,
                            impulse: Vector::ZERO,
                        },
                        events,
                    );
                }
                break;
            } else if !self.pieces[id].destructible() {
                break;
            } else if !self.laser_cut(id) {
                events.push(Event::LaserLimited);
                break;
            }
        }
    }

    #[cfg(test)]
    fn detonate(&mut self, position: Vector, mass: f32, events: &mut Vec<Event>) {
        let event = self.next_impact();
        self.detonate_with_event(position, mass, event, events);
    }

    fn detonate_with_event(
        &mut self,
        position: Vector,
        mass: f32,
        event: u64,
        events: &mut Vec<Event>,
    ) {
        self.blast(
            position,
            mass * 50_000.0,
            1.7 * (mass / 6.0).cbrt(),
            event,
            events,
        );
    }
    pub(super) fn blast(
        &mut self,
        position: Vector,
        source_energy: f32,
        radius: f32,
        event: u64,
        events: &mut Vec<Event>,
    ) {
        self.blast_with_tank_scale(position, source_energy, radius, 1.0, event, events);
    }
    fn blast_with_tank_scale(
        &mut self,
        position: Vector,
        source_energy: f32,
        radius: f32,
        tank_scale: f32,
        event: u64,
        events: &mut Vec<Event>,
    ) {
        self.blast_pressure(position, source_energy, radius, tank_scale, event, events);
        self.blast_decorations(position, radius);
        self.explosions += 1;
        events.push(Event::Exploded { position, radius });
    }
    pub(super) fn blast_pressure(
        &mut self,
        position: Vector,
        source_energy: f32,
        radius: f32,
        tank_scale: f32,
        event: u64,
        events: &mut Vec<Event>,
    ) {
        const RAYS: usize = 192;
        // Each angular sector carries a fixed share of the blast. Only the
        // first surface intercepts it; energy escaping into air or striking a
        // fixed wall is NOT redistributed into the remaining bodies.
        let mut received = std::collections::BTreeMap::new();
        let candidates: Vec<_> = self
            .physics
            .colliders
            .iter()
            .filter_map(|(_, collider)| {
                let id = self.weapon_target(collider)?;
                let bounds = collider.compute_aabb();
                let reach = Vector::splat(radius);
                if (position - reach).cmpgt(bounds.maxs).any()
                    || (position + reach).cmplt(bounds.mins).any()
                {
                    return None;
                }
                Some((id, collider, bounds))
            })
            .collect();
        for i in 0..RAYS {
            let angle = (i as f32 + 0.5) * std::f32::consts::TAU / RAYS as f32;
            let direction = Vector::new(angle.cos(), angle.sin());
            let ray = Ray::new(position, direction);
            let mut nearest = None;
            for (id, collider, bounds) in &candidates {
                if bounds.cast_local_ray(&ray, radius, true).is_none() {
                    continue;
                }
                if let Some(time) =
                    collider
                        .shape()
                        .cast_ray(collider.position(), &ray, radius, true)
                    && nearest.is_none_or(|(old_time, _)| time < old_time)
                {
                    nearest = Some((time, *id));
                }
            }
            if let Some((distance, id)) = nearest {
                if !self.pieces[id].destructible() && self.pieces[id].material != Material::Launcher
                {
                    continue;
                }
                let energy = source_energy / RAYS as f32 * (1.0 - distance / radius).powi(2);
                let (total, weighted_direction) = received.entry(id).or_insert((0.0, Vector::ZERO));
                *total += energy;
                *weighted_direction += direction * energy;
            }
        }
        for (id, (energy, direction)) in received {
            if self.pieces[id].material == Material::Launcher {
                // Apply occluded, distance-attenuated blast energy once, not once per ray.
                let forward = self.piece_pose(id).rotation * Vector::X;
                let facing = -direction.normalize_or_zero().dot(forward);
                let resistance = if facing > 0.5 {
                    3000.0
                } else if facing < -0.5 {
                    1500.0
                } else {
                    2200.0
                };
                let impulse = direction / 80.0;
                let direction = direction.normalize_or_zero();
                let point = self.piece_pose(id).translation - direction * 0.8;
                self.damage_tank_from(
                    id,
                    energy * tank_scale / resistance,
                    TankImpact {
                        cause: DestructionCause::Blast,
                        point,
                        direction,
                        impulse,
                    },
                    events,
                );
            }
            if direction.length_squared() < 0.001 || energy <= 0.0 {
                continue;
            }
            // A fixed pressure impulse/work ratio makes the source independent
            // of how many mesh cells intercept it. This remains a tuned 2D blast.
            let magnitude = direction.length();
            let direction = direction / magnitude;
            let collider = &self.physics.colliders[self.pieces[id].collider];
            // Every ray's force passes through the blast origin. Place their
            // resultant on that same line of action, at the receiving surface,
            // preserving both net impulse and torque when cells are regrouped.
            let distance = collider
                .shape()
                .cast_ray(
                    collider.position(),
                    &Ray::new(position, direction),
                    radius,
                    true,
                )
                .unwrap_or(0.0);
            let point = position + direction * distance;
            self.blast_pulses.push(BlastPulse {
                piece: id,
                local_point: self.piece_pose(id).inverse_transform_point(point),
                direction,
                impulse_left: magnitude / 80.0,
                energy_left: energy,
                time_left: 0.035,
                event,
            });
        }
    }

    pub(super) fn advance_blasts(&mut self, dt: f32) {
        for mut pulse in std::mem::take(&mut self.blast_pulses) {
            if !self.pieces[pulse.piece].active {
                continue;
            }
            let fraction = (dt / pulse.time_left).min(1.0);
            let impulse = pulse.impulse_left * fraction;
            let energy = pulse.energy_left * fraction;
            self.loads.push(loads::Load {
                piece: pulse.piece,
                point: self
                    .piece_pose(pulse.piece)
                    .transform_point(pulse.local_point),
                impulse: pulse.direction * impulse,
                energy,
                apply: true,
                event: pulse.event,
            });
            pulse.energy_left -= energy;
            pulse.impulse_left -= impulse;
            pulse.time_left -= dt;
            if pulse.time_left > 1e-6 && pulse.energy_left > 1e-5 {
                self.blast_pulses.push(pulse);
            }
        }
    }
}

#[cfg(test)]
fn transfer_impact(body: &mut RigidBody, shell: &Shell, point: Vector) {
    let impulse = shell.velocity * shell.mass;
    let energy = 0.5 * shell.mass * shell.velocity.length_squared();
    let scale = loads::impulse_scale(body, impulse, point, energy);
    body.apply_impulse_at_point(impulse * scale, point, true);
}

#[cfg(test)]
mod tests;
