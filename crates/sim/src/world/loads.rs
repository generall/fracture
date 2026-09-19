use super::*;
use crate::structure::{self, Interface, Node};
use std::collections::BTreeMap;

pub(super) struct Load {
    pub piece: usize,
    pub point: Vector,
    pub impulse: Vector,
    /// Remaining positive-work budget. Splitting never replenishes it.
    pub energy: f32,
    /// False for contact impulses already applied by Rapier.
    pub apply: bool,
    pub event: u64,
}

pub(super) struct Motion {
    center: Vector,
    velocity: Vector,
    omega: f32,
    energy: f64,
}

impl World {
    pub(super) fn next_impact(&mut self) -> u64 {
        self.impact_serial += 1;
        self.impact_serial
    }

    pub(super) fn contact_velocities(&self) -> BTreeMap<(u32, u32), Motion> {
        self.physics
            .bodies
            .iter()
            .map(|(h, b)| {
                (
                    h.into_raw_parts(),
                    Motion {
                        center: b.center_of_mass(),
                        velocity: b.linvel(),
                        omega: b.angvel(),
                        energy: b.kinetic_energy() as f64,
                    },
                )
            })
            .collect()
    }

    /// Return tank hull damage separately so both sides of a collision are
    /// evaluated before destruction disables either body.
    pub(super) fn collect_contact_loads(
        &mut self,
        previous: &BTreeMap<(u32, u32), Motion>,
    ) -> Vec<(usize, f32)> {
        let event = self.next_impact();
        let first = self.loads.len();
        let mut ramming: BTreeMap<usize, (f32, f32)> = BTreeMap::new();
        for pair in self.physics.contact_pairs() {
            let c1 = &self.physics.colliders[pair.collider1];
            let c2 = &self.physics.colliders[pair.collider2];
            let (Some(h1), Some(h2)) = (c1.parent(), c2.parent()) else {
                continue;
            };
            let b1 = &self.physics.bodies[h1];
            let b2 = &self.physics.bodies[h2];
            if (b1.is_fixed() || b1.is_sleeping()) && (b2.is_fixed() || b2.is_sleeping()) {
                continue;
            }
            let (Some(v1), Some(v2)) = (
                previous.get(&h1.into_raw_parts()),
                previous.get(&h2.into_raw_parts()),
            ) else {
                continue;
            };
            for manifold in &pair.manifolds {
                let normal = manifold.data.normal;
                let tangent = Vector::new(normal.y, -normal.x);
                for contact in &manifold.points {
                    if contact.data.impulse.abs() < 1e-5 {
                        continue;
                    }
                    let point = (c1.position().transform_point(contact.local_p1)
                        + c2.position().transform_point(contact.local_p2))
                        * 0.5;
                    let velocity = |m: &Motion| {
                        let r = point - m.center;
                        m.velocity + Vector::new(-r.y, r.x) * m.omega
                    };
                    let relative = velocity(v1) - velocity(v2);
                    // Rapier's normal points from collider 1 toward collider 2.
                    let impulse1 =
                        -normal * contact.data.impulse + tangent * contact.data.tangent_impulse[0];
                    // The collision solver already dissipated this work. Each
                    // side may spend half on crack surfaces; no energy is added.
                    let energy = (-0.25 * relative.dot(impulse1)).max(0.0);
                    for (collider, impulse) in [(c1, impulse1), (c2, -impulse1)] {
                        let Some(id) = (collider.user_data as usize).checked_sub(1) else {
                            continue;
                        };
                        if self
                            .pieces
                            .get(id)
                            .is_some_and(|p| p.material == Material::Launcher)
                            && !self.tanks.is_empty()
                        {
                            // Use normal collision work, not speed alone. Solver
                            // impulses distribute it across hull/track contacts;
                            // tangential sliding does not count as a ram.
                            let closing = relative.dot(normal);
                            if closing > 0.0 {
                                let work = 0.25 * contact.data.impulse.abs() * closing;
                                let incoming = if std::ptr::eq(collider, c1) {
                                    -normal
                                } else {
                                    normal
                                };
                                let forward = self.piece_pose(id).rotation * Vector::X;
                                let facing = -incoming.dot(forward);
                                let resistance = if facing > 0.5 {
                                    3000.0
                                } else if facing < -0.5 {
                                    1600.0
                                } else {
                                    2200.0
                                };
                                let entry = ramming.entry(id).or_default();
                                entry.0 += work;
                                entry.1 += work / resistance;
                            }
                        }
                        if self
                            .pieces
                            .get(id)
                            .is_some_and(|p| p.active && p.destructible())
                        {
                            self.loads.push(Load {
                                piece: id,
                                point,
                                impulse,
                                energy,
                                apply: false,
                                event,
                            });
                        }
                    }
                }
            }
        }
        // Contact-wise estimates can overlap in a multi-contact collision.
        // Cap their combined crack budget by the actual global kinetic loss
        // during this Rapier microstep (all driven/weapon loads preceded it).
        let before: f64 = previous.values().map(|m| m.energy).sum();
        let after: f64 = self
            .physics
            .bodies
            .iter()
            .map(|(_, b)| b.kinetic_energy() as f64)
            .sum();
        let available = (before - after).max(0.0);
        let requested: f64 = self.loads[first..].iter().map(|l| l.energy as f64).sum();
        if requested > available {
            let scale = (available / requested) as f32;
            for load in &mut self.loads[first..] {
                load.energy *= scale;
            }
        }
        // Bound the combined estimate by actual kinetic loss, including when
        // several track colliders or wall triangles share the same impact.
        let requested: f64 = ramming.values().map(|(work, _)| *work as f64).sum();
        let scale = if requested > available {
            (available / requested) as f32
        } else {
            1.0
        };
        ramming
            .into_iter()
            .filter(|(_, (work, _))| work * scale >= 1000.0)
            .map(|(id, (_, damage))| (id, damage * scale))
            .collect()
    }

    pub(super) fn process_loads(&mut self, dt: f32, events: &mut Vec<Event>) {
        if self.loads.is_empty() {
            return;
        }
        self.refine_loaded_cells();
        let mut deformed = std::collections::BTreeSet::new();
        // Re-equilibrate after each crack, with bounded rounds. This allows loads
        // to reach a distant neck, and redistributes them after local failure.
        for _ in 0..8 {
            let groups = self.chunk_members();
            let mut failed = Vec::new();
            let mut shape_changed = false;
            for members in groups.values() {
                let mut source: Vec<_> = self
                    .loads
                    .iter()
                    .enumerate()
                    .filter(|(_, l)| members.binary_search(&l.piece).is_ok())
                    .map(|(i, _)| i)
                    .collect();
                if source.is_empty() {
                    continue;
                }
                source.sort_unstable();
                let resistance = self.fracture_resistance(&source, dt);
                let handle = self.pieces[members[0]].body;
                let frame = *self.physics.bodies[handle].position();
                let mut ids = members.clone();
                let mut edge_ids = Vec::new();
                for (i, bond) in self.bonds.iter().enumerate() {
                    if !bond.active {
                        continue;
                    }
                    let a = members.binary_search(&bond.a).is_ok();
                    let b = members.binary_search(&bond.b).is_ok();
                    if a || b {
                        edge_ids.push(i);
                        if !a {
                            ids.push(bond.a);
                        }
                        if !b {
                            ids.push(bond.b);
                        }
                    }
                }
                if edge_ids.is_empty() {
                    continue;
                }
                ids.sort_unstable();
                ids.dedup();
                let mut nodes: Vec<_> = ids
                    .iter()
                    .map(|&id| {
                        let p = frame.inverse_transform_point(self.piece_pose(id).translation);
                        Node {
                            position: [p.x as f64, p.y as f64],
                            fixed: !self.pieces[id].destructible()
                                || self.pieces[id].ground_anchor.is_some(),
                            force: [0.0; 3],
                        }
                    })
                    .collect();
                for &i in &source {
                    let l = &self.loads[i];
                    let index = ids.binary_search(&l.piece).unwrap();
                    let f = frame.rotation.inverse() * l.impulse / dt;
                    let lever =
                        frame.rotation.inverse() * (l.point - self.piece_pose(l.piece).translation);
                    nodes[index].force[0] += f.x as f64;
                    nodes[index].force[1] += f.y as f64;
                    nodes[index].force[2] += lever.perp_dot(f) as f64;
                }
                if !nodes.iter().any(|n| n.fixed) {
                    // D'Alembert balance: only the portion of a load needed to
                    // accelerate different cells together creates internal stress.
                    let mass: f64 = ids.iter().map(|&i| self.pieces[i].mass() as f64).sum();
                    let center = [
                        (0..ids.len())
                            .map(|i| nodes[i].position[0] * self.pieces[ids[i]].mass() as f64)
                            .sum::<f64>()
                            / mass,
                        (0..ids.len())
                            .map(|i| nodes[i].position[1] * self.pieces[ids[i]].mass() as f64)
                            .sum::<f64>()
                            / mass,
                    ];
                    let force = [
                        nodes.iter().map(|n| n.force[0]).sum::<f64>(),
                        nodes.iter().map(|n| n.force[1]).sum::<f64>(),
                    ];
                    let mut inertia = 0.0;
                    let mut torque = 0.0;
                    for (i, node) in nodes.iter().enumerate() {
                        let m = self.pieces[ids[i]].mass();
                        let r = [node.position[0] - center[0], node.position[1] - center[1]];
                        inertia += self.pieces[ids[i]].shape.inertia(m) as f64
                            + m as f64 * (r[0] * r[0] + r[1] * r[1]);
                        torque += node.force[2] + r[0] * node.force[1] - r[1] * node.force[0];
                    }
                    let alpha = torque / inertia.max(1e-20);
                    for (i, node) in nodes.iter_mut().enumerate() {
                        let m = self.pieces[ids[i]].mass();
                        let r = [node.position[0] - center[0], node.position[1] - center[1]];
                        node.force[0] -= m as f64 * (force[0] / mass - alpha * r[1]);
                        node.force[1] -= m as f64 * (force[1] / mass + alpha * r[0]);
                        node.force[2] -= self.pieces[ids[i]].shape.inertia(m) as f64 * alpha;
                    }
                }
                let interfaces: Vec<_> = edge_ids
                    .iter()
                    .map(|&i| {
                        let b = &self.bonds[i];
                        let a = self.piece_pose(b.a);
                        let p = frame.inverse_transform_point(a.transform_point(b.anchor_a));
                        let n = frame.rotation.inverse() * a.rotation * b.normal_a;
                        Interface {
                            a: ids.binary_search(&b.a).unwrap(),
                            b: ids.binary_search(&b.b).unwrap(),
                            point: [p.x as f64, p.y as f64],
                            normal: [n.x as f64, n.y as f64],
                            width: b.width as f64,
                            depth: b.depth as f64,
                            young: b.young as f64,
                            poisson: 0.2,
                        }
                    })
                    .collect();
                let solution = self
                    .structural_cache
                    .entry(handle.into_raw_parts())
                    .or_insert_with(|| structure::System::new(&nodes, &interfaces))
                    .solve(&nodes);
                self.structural_residual = self.structural_residual.max(solution.residual);
                self.structural_iterations += solution.iterations;
                self.elastic_energy = self
                    .elastic_energy
                    .max(solution.reactions.iter().map(|r| r.energy).sum());
                if !solution.converged {
                    self.unconverged += 1;
                    continue;
                }
                if !deformed.contains(&handle.into_raw_parts())
                    && self.deform_plastically(
                        &ids,
                        &nodes,
                        &edge_ids,
                        &interfaces,
                        &solution,
                        &source,
                        dt,
                    )
                {
                    deformed.insert(handle.into_raw_parts());
                    shape_changed = true;
                    continue;
                }
                let mut candidates = Vec::new();
                let components = self.components_for(self.loads[source[0]].piece);
                let deformable = components.plasticity.is_some();
                let plastic_resistance = if source.iter().any(|&i| self.loads[i].apply) {
                    components.projectile_resistance
                } else {
                    1.0
                };
                for (&id, r) in edge_ids.iter().zip(&solution.reactions) {
                    let b = &mut self.bonds[id];
                    let ratio = if let Some(p) = components.plasticity {
                        let strain = (self.pieces[b.a].plastic_strain
                            + self.pieces[b.b].plastic_strain)
                            * 0.5;
                        let area = (b.width * b.depth) as f64;
                        let stress = ((r.normal / area).powi(2) + 3.0 * (r.shear / area).powi(2))
                            .sqrt()
                            + 6.0 * r.moment.abs() / (area * b.width as f64);
                        (stress
                            / (p.yield_stress * plastic_resistance * (1.0 + p.hardening * strain))
                                as f64) as f32
                    } else {
                        b.strength.utilization(
                            r.normal as f32,
                            r.shear as f32,
                            r.moment as f32,
                            b.width,
                            b.depth,
                        ) / (b.strength_scale * resistance)
                    };
                    b.load = b.load.max(ratio);
                    self.peak_load = self.peak_load.max(ratio);
                    self.pieces[b.a].load = self.pieces[b.a].load.max(ratio);
                    self.pieces[b.b].load = self.pieces[b.b].load.max(ratio);
                    if ratio > 1.0 && !deformable {
                        candidates.push((id, ratio));
                    }
                }
                candidates.sort_by(|(a, x), (b, y)| y.total_cmp(x).then(a.cmp(b)));
                // Re-equilibrate after each crack. A batch chosen from the old
                // stress field can shatter a region whose first crack would
                // already have relieved its load.
                for (id, _) in candidates {
                    let b = &self.bonds[id];
                    let cost = b.toughness * b.width * b.depth * b.strength_scale * resistance;
                    let available: f32 = source.iter().map(|&i| self.loads[i].energy).sum();
                    if available < cost {
                        continue;
                    }
                    let mut remaining = cost;
                    for &i in &source {
                        let taken = remaining.min(self.loads[i].energy);
                        self.loads[i].energy -= taken;
                        remaining -= taken;
                    }
                    self.fracture_work += cost as f64;
                    failed.push(id);
                    break;
                }
            }
            if failed.is_empty() {
                if shape_changed {
                    continue;
                } else {
                    break;
                }
            }
            for id in failed {
                self.fracture_ignition(id);
                let b = &self.bonds[id];
                let pose = self.piece_pose(b.a);
                events.push(Event::Fractured {
                    position: pose.transform_point(b.anchor_a),
                    normal: pose.rotation * b.normal_a,
                });
                self.bonds[id].active = false;
                self.broken += 1;
            }
            self.topology_changed = true;
            self.structural_cache.clear();
            self.rebuild_chunks();
        }
        for l in std::mem::take(&mut self.loads) {
            if !l.apply || !self.pieces[l.piece].active {
                continue;
            }
            let body = &mut self.physics.bodies[self.pieces[l.piece].body];
            let scale = impulse_scale(body, l.impulse, l.point, l.energy);
            body.apply_impulse_at_point(l.impulse * scale, l.point, true);
        }
    }
}

/// Bound positive kinetic work, including point torque. A smaller recipient
/// after fracture cannot turn the same nominal impulse into unlimited energy.
pub(super) fn impulse_scale(body: &RigidBody, impulse: Vector, point: Vector, energy: f32) -> f32 {
    let lever = point - body.center_of_mass();
    let inertia = body.mass_properties();
    let a = 0.5
        * (impulse.dot(impulse * inertia.effective_inv_mass)
            + lever.perp_dot(impulse).powi(2) * inertia.effective_world_inv_inertia);
    let b = impulse.dot(body.velocity_at_point(point));
    if a + b <= energy || a <= 0.0 {
        return 1.0;
    }
    let root = (b * b + 4.0 * a * energy.max(0.0)).sqrt();
    if b >= 0.0 {
        (2.0 * energy / (root + b).max(1e-20)).clamp(0.0, 1.0)
    } else {
        ((root - b) / (2.0 * a)).clamp(0.0, 1.0)
    }
}
