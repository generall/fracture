//! Ground resistance and retirement of small disconnected chunks.
use super::*;

impl World {
    pub(super) fn ground_resistance(&mut self, dt: f32) {
        let mut visited = std::collections::BTreeSet::new();
        for piece in &self.pieces {
            if !piece.active || !visited.insert(piece.body.into_raw_parts()) {
                continue;
            }
            let Some(body) = self.physics.bodies.get_mut(piece.body) else {
                continue;
            };
            if piece.material == Material::Launcher || !body.is_dynamic() || body.is_sleeping() {
                continue;
            }
            // Approximate Coulomb friction against the unmodeled horizontal floor.
            // Clamp so friction never reverses velocity or adds kinetic energy.
            let speed = body.linvel().length();
            let next_speed = (speed - piece.floor_friction * 9.81 * dt).max(0.0);
            if speed > 0.0 {
                body.set_linvel(body.linvel() * (next_speed / speed), false);
            }
            let omega = body.angvel();
            body.set_angvel(
                omega.signum()
                    * (omega.abs() - FLOOR_ANGULAR_DECELERATION * piece.floor_friction / 0.32 * dt)
                        .max(0.0),
                false,
            );
        }
    }

    pub(super) fn cleanup_debris(&mut self) {
        // One lifetime for the entire physical chunk. Touching another chunk
        // does not reattach it. Intact objects are not marked until a fracture.
        if self.topology_changed {
            self.topology_changed = false;
            let linked: std::collections::BTreeSet<_> = self
                .bonds
                .iter()
                .filter(|b| b.active && self.bending_torque(b).is_some())
                .flat_map(|b| [b.a, b.b])
                .collect();
            for members in self.chunk_members().into_values() {
                if members.iter().any(|id| linked.contains(id)) {
                    continue;
                }
                let body = self.pieces[members[0]].body;
                if self.physics.bodies[body].is_fixed()
                    && !members
                        .iter()
                        .any(|&i| self.pieces[i].ground_anchor.is_some())
                {
                    continue;
                }
                let area: f32 = members.iter().map(|&i| self.pieces[i].shape.area()).sum();
                if area < DEBRIS_AREA {
                    let lifetime = 3.0 + (members[0] % 11) as f32 * 0.09;
                    for id in members {
                        self.pieces[id].debris_remaining.get_or_insert(lifetime);
                    }
                }
            }
        }
        let mut remove = std::collections::BTreeSet::new();
        for piece in &mut self.pieces {
            if !piece.active {
                continue;
            }
            if let Some(remaining) = &mut piece.debris_remaining {
                *remaining -= TICK_DT;
                if *remaining <= 0.0 {
                    remove.insert(piece.body.into_raw_parts());
                }
            }
        }
        for (index, generation) in remove {
            let handle = RigidBodyHandle::from_raw_parts(index, generation);
            for piece in &mut self.pieces {
                if piece.active && piece.body == handle {
                    piece.active = false;
                    self.cleared += 1;
                }
            }
            self.physics.remove_body(handle);
            self.structural_cache.remove(&(index, generation));
        }
        for bond in &mut self.bonds {
            if !self.pieces[bond.a].active || !self.pieces[bond.b].active {
                bond.active = false;
            }
        }
    }
}
