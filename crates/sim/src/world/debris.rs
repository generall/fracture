//! Ground resistance and retirement of small disconnected chunks.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

// A soft live-cell budget, not a cap on stable IDs or authored map size.
const ACTIVE_PIECE_BUDGET: usize = 8192;
const SIGHT_MARGIN: f32 = 8.0;

#[cfg(test)]
#[path = "debris_tests.rs"]
mod tests;

impl World {
    pub(super) fn ground_resistance(&mut self, dt: f32) {
        if self.ground_members_dirty || self.ground_records != self.pieces.len() {
            // Keep the first active cell of each body, in original cell order.
            // Rebuild lazily after a topology batch, not for every crack round.
            let mut visited = BTreeSet::new();
            self.ground_members = self
                .pieces
                .iter()
                .enumerate()
                .filter_map(|(id, p)| {
                    (p.active && visited.insert(p.body.into_raw_parts())).then_some(id)
                })
                .collect();
            self.ground_records = self.pieces.len();
            self.ground_members_dirty = false;
        }
        for &id in &self.ground_members {
            let piece = &self.pieces[id];
            if !piece.active {
                continue;
            }
            let Some(body) = self.physics.bodies.get_mut(piece.body) else {
                continue;
            };
            // Preserve Rapier's original first-touch order, but only apply
            // friction to moving chunks. Each body is visited once.
            if !body.is_dynamic() || body.is_sleeping() {
                continue;
            }
            if piece.material == Material::Launcher {
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
        // Budget cleanup is level-only so material experiment replays retain
        // their existing retirement rules. Run at 4 Hz, not every physics tick.
        if self.scene == Scene::Level && self.tick.is_multiple_of(30) {
            self.retire_excess_debris(ACTIVE_PIECE_BUDGET, &mut remove);
        }
        // Retire all selected cells in one pass, even when budget cleanup
        // evicts many chunks together.
        for piece in &mut self.pieces {
            if piece.active && remove.contains(&piece.body.into_raw_parts()) {
                piece.active = false;
                self.cleared += 1;
            }
        }
        for (index, generation) in remove {
            let handle = RigidBodyHandle::from_raw_parts(index, generation);
            self.physics.remove_body(handle);
            self.structural_cache.remove(&(index, generation));
        }
        for bond in &mut self.bonds {
            if !self.pieces[bond.a].active || !self.pieces[bond.b].active {
                bond.active = false;
            }
        }
    }

    fn retire_excess_debris(&self, budget: usize, remove: &mut BTreeSet<(u32, u32)>) {
        let active = self
            .pieces
            .iter()
            .filter(|p| p.active && !remove.contains(&p.body.into_raw_parts()))
            .count();
        let mut excess = active.saturating_sub(budget);
        if excess == 0 {
            return;
        }

        // Conservatively protect full sight-radius circles, regardless of
        // turret direction or occlusion. Include every tank (also during its
        // respawn delay), both flag lookouts and owned radars for LAN fairness.
        let observers: Vec<_> = std::iter::once(self.launcher)
            .chain(
                self.tanks
                    .iter()
                    .map(|t| t.id)
                    .filter(|&id| id != self.launcher),
            )
            .map(|id| {
                let position = self.piece_pose(id).translation;
                (position, self.vision_range_at(position))
            })
            .chain(self.flags().map(|(_, position)| (position, NEAR_VISION)))
            .chain(self.stations().filter_map(|station| {
                (station.kind == crate::level::StationKind::Radar && station.owner.is_some())
                    .then_some((station.position, station.settings.radar_radius))
            }))
            .collect();

        let mut chunks: BTreeMap<_, Vec<usize>> = BTreeMap::new();
        for (id, piece) in self.pieces.iter().enumerate() {
            if piece.active && !remove.contains(&piece.body.into_raw_parts()) {
                chunks
                    .entry(piece.body.into_raw_parts())
                    .or_default()
                    .push(id);
            }
        }
        let mut candidates = Vec::new();
        let linked: BTreeSet<_> = self
            .bonds
            .iter()
            .filter(|bond| bond.active && self.pieces[bond.a].body != self.pieces[bond.b].body)
            .flat_map(|bond| {
                [
                    self.pieces[bond.a].body.into_raw_parts(),
                    self.pieces[bond.b].body.into_raw_parts(),
                ]
            })
            .collect();
        for (handle, members) in chunks {
            // Only already-disposable debris qualifies. Never carve individual
            // cells out of a chunk, delete fixed scenery, or remove a tank.
            if linked.contains(&handle)
                || members.iter().any(|&id| {
                    let p = &self.pieces[id];
                    p.debris_remaining.is_none()
                        || p.material == Material::Launcher
                        || p.ground_anchor.is_some()
                        || p.bridge_deck
                })
            {
                continue;
            }
            let Some(body) = self.physics.bodies.get(self.pieces[members[0]].body) else {
                continue;
            };
            if !body.is_dynamic() {
                continue;
            }
            let mut distance = f32::INFINITY;
            let mut area = 0.0;
            for &id in &members {
                let piece = &self.pieces[id];
                let bounds = self.physics.colliders[piece.collider].compute_aabb();
                for &(origin, radius) in &observers {
                    distance = distance.min(
                        origin.distance(origin.clamp(bounds.mins, bounds.maxs))
                            - radius
                            - SIGHT_MARGIN,
                    );
                }
                area += piece.shape.area();
            }
            if distance > 0.0 {
                candidates.push((area, distance, handle, members.len()));
            }
        }
        // Stable tie-breaking keeps host simulation and replays deterministic.
        candidates.sort_by(|a, b| {
            a.0.total_cmp(&b.0)
                .then_with(|| b.1.total_cmp(&a.1))
                .then_with(|| a.2.cmp(&b.2))
        });
        for (_, _, handle, cells) in candidates {
            if excess == 0 {
                break;
            }
            remove.insert(handle);
            excess = excess.saturating_sub(cells);
        }
    }
}
