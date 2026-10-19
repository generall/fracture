use super::*;
use std::collections::{BTreeMap, BTreeSet};

impl World {
    /// Resolve a chunk from Rapier's existing collider ownership, retaining the
    /// ascending cell order used by the fracture solver and replay recordings.
    pub(super) fn body_members(&self, body: RigidBodyHandle) -> Vec<usize> {
        let mut members: Vec<_> = self.physics.bodies[body]
            .colliders()
            .iter()
            .filter_map(|&h| {
                let id = self.physics.colliders[h].user_data.checked_sub(1)? as usize;
                let p = self.pieces.get(id)?;
                (p.active && p.destructible() && p.body == body).then_some(id)
            })
            .collect();
        members.sort_unstable();
        members.dedup();
        members
    }

    pub(super) fn member_bonds(&self, members: &[usize]) -> Vec<usize> {
        let mut bonds: Vec<_> = members
            .iter()
            .filter_map(|&id| self.incident_bonds.get(id))
            .flatten()
            .copied()
            .filter(|&id| self.bonds[id].active)
            .collect();
        bonds.sort_unstable();
        bonds.dedup();
        bonds
    }

    pub(super) fn piece_pose(&self, id: usize) -> Pose {
        *self.physics.bodies[self.pieces[id].body].position() * self.pieces[id].local_pose
    }

    pub(super) fn chunk_members(&self) -> BTreeMap<(u32, u32), Vec<usize>> {
        let mut groups: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for (id, piece) in self.pieces.iter().enumerate() {
            if piece.active && piece.destructible() {
                groups
                    .entry(piece.body.into_raw_parts())
                    .or_default()
                    .push(id);
            }
        }
        groups
    }

    /// Build one rigid body per connected material component. Supports remain
    /// separate fixed colliders and fix any component still bonded to them.
    /// Internal cells have exact geometry and no pairwise collision constraints.
    pub(super) fn rebuild_chunks(&mut self) {
        let candidates: Vec<_> = (0..self.pieces.len()).collect();
        self.rebuild_chunks_for(&candidates);
    }

    /// A broken bond can only split the chunks that owned its endpoints. The
    /// caller supplies every member of those chunks before changing ownership.
    pub(super) fn rebuild_chunks_for(&mut self, candidates: &[usize]) {
        self.query_epoch += 1;
        self.ground_members_dirty = true;
        #[cfg(feature = "physics-profile")]
        let started = std::time::Instant::now();
        let mut visited = vec![false; self.pieces.len()];
        let mut remove = BTreeSet::new();
        for &id in candidates {
            let piece = &self.pieces[id];
            if !piece.active
                && piece.destructible()
                && self.physics.bodies.get(piece.body).is_some()
            {
                remove.insert(piece.body.into_raw_parts());
            }
        }
        for &start in candidates {
            if visited[start] || !self.pieces[start].active || !self.pieces[start].destructible() {
                continue;
            }
            let mut members = vec![start];
            visited[start] = true;
            let mut fixed = false;
            let mut cursor = 0;
            while cursor < members.len() {
                fixed |= self.pieces[members[cursor]].ground_anchor.is_some();
                let id = members[cursor];
                for &edge in self.incident_bonds.get(id).into_iter().flatten() {
                    let bond = &self.bonds[edge];
                    if !bond.active || self.bending_torque(bond).is_some() {
                        continue;
                    }
                    let other = if bond.a == id { bond.b } else { bond.a };
                    if !self.pieces[other].active {
                        continue;
                    }
                    if !self.pieces[other].destructible() {
                        fixed = true;
                        continue;
                    }
                    if !visited[other] {
                        visited[other] = true;
                        members.push(other);
                    }
                }
                cursor += 1;
            }
            members.sort_unstable();
            let old = self.pieces[start].body;
            let previous = &self.physics.bodies[old];
            if members.iter().all(|&i| self.pieces[i].body == old)
                && previous.colliders().len() == members.len()
                && previous.is_fixed() == fixed
            {
                continue;
            }

            // Use f64 for summed mass/moments. All new chunks inherit the old
            // rigid velocity field: no outward kicks or overlap corrections.
            let poses: Vec<_> = members.iter().map(|&id| self.piece_pose(id)).collect();
            let masses: Vec<_> = members
                .iter()
                .map(|&id| self.pieces[id].mass() as f64)
                .collect();
            let mass: f64 = masses.iter().sum();
            let center = Vector::new(
                (poses
                    .iter()
                    .zip(&masses)
                    .map(|(p, m)| p.translation.x as f64 * m)
                    .sum::<f64>()
                    / mass) as f32,
                (poses
                    .iter()
                    .zip(&masses)
                    .map(|(p, m)| p.translation.y as f64 * m)
                    .sum::<f64>()
                    / mass) as f32,
            );
            let mut momentum = [0.0; 2];
            let mut angular = 0.0;
            let mut inertia = 0.0;
            let mut sleeping = true;
            for ((&id, pose), &m) in members.iter().zip(&poses).zip(&masses) {
                let old = &self.physics.bodies[self.pieces[id].body];
                let v = old.velocity_at_point(pose.translation);
                let r = pose.translation - center;
                let own = self.pieces[id].shape.inertia(m as f32) as f64;
                momentum[0] += m * v.x as f64;
                momentum[1] += m * v.y as f64;
                angular += own * old.angvel() as f64 + m * r.perp_dot(v) as f64;
                inertia += own + m * r.length_squared() as f64;
                sleeping &= old.is_sleeping();
            }
            let builder = if fixed {
                RigidBodyBuilder::fixed()
            } else {
                RigidBodyBuilder::dynamic()
            };
            let body = self
                .physics
                .insert_body(builder.translation(center).ccd_enabled(!fixed));
            for (&id, &pose) in members.iter().zip(&poses) {
                let p = &mut self.pieces[id];
                remove.insert(p.body.into_raw_parts());
                let local = Pose::new(pose.translation - center, pose.rotation.angle());
                let collider = match p.shape {
                    Shape::Triangle { vertices } => {
                        ColliderBuilder::convex_hull(&vertices).expect("valid cell")
                    }
                    Shape::Box { half } => ColliderBuilder::cuboid(half.x, half.y),
                    Shape::Ball { radius } => ColliderBuilder::ball(radius),
                }
                .position(local)
                .collision_groups(p.collision_layer().groups())
                .density(p.solid.density * p.solid.depth)
                .friction(p.floor_friction)
                .restitution(p.restitution)
                .active_hooks(contacts::hooks_for(p.soft_bounce))
                .user_data((id + 1) as u128);
                let collider = if p.shape_revision > 0 {
                    collider.mass(p.mass())
                } else {
                    collider
                };
                p.collider = self.physics.insert_collider(collider, Some(body));
                p.body = body;
                p.local_pose = local;
            }
            let new = &mut self.physics.bodies[body];
            new.recompute_mass_properties_from_colliders(&self.physics.colliders);
            if !fixed {
                new.set_linvel(
                    Vector::new((momentum[0] / mass) as f32, (momentum[1] / mass) as f32),
                    true,
                );
                new.set_angvel((angular / inertia.max(1e-20)) as f32, true);
                if sleeping {
                    new.sleep();
                }
            }
        }
        // A former parent may still be needed while other components are being
        // copied, so only retire the old bodies after every reassignment.
        for (index, generation) in remove {
            let handle = RigidBodyHandle::from_raw_parts(index, generation);
            if !self.pieces.iter().any(|p| p.active && p.body == handle) {
                self.physics.remove_body(handle);
            }
        }
        self.structural_cache.retain(|&(i, g), _| {
            self.physics
                .bodies
                .get(RigidBodyHandle::from_raw_parts(i, g))
                .is_some()
        });
        self.physics
            .bodies
            .propagate_modified_body_positions_to_colliders(&mut self.physics.colliders);
        self.update_visible_edges_for(candidates);
        self.rebuild_bending_joints();
        #[cfg(feature = "physics-profile")]
        {
            self.profile.rebuild += started.elapsed().as_secs_f64() * 1000.0;
        }
    }

    pub(super) fn update_visible_edges(&mut self) {
        let members: Vec<_> = (0..self.pieces.len()).collect();
        self.update_visible_edges_for(&members);
    }

    fn update_visible_edges_for(&mut self, members: &[usize]) {
        for &id in members {
            let mut covered = [0.0_f32; 3];
            if let Shape::Triangle { vertices } = self.pieces[id].shape {
                for &edge in self.incident_bonds.get(id).into_iter().flatten() {
                    let b = &self.bonds[edge];
                    if !b.active
                        || self.pieces[b.a].grain.is_none()
                        || self.pieces[b.a].grain != self.pieces[b.b].grain
                    {
                        continue;
                    }
                    let face = self.piece_pose(b.a).transform_point(b.anchor_a);
                    let p = self.piece_pose(id).inverse_transform_point(face);
                    for i in 0..3 {
                        let a = vertices[i];
                        let z = vertices[(i + 1) % 3];
                        let edge = z - a;
                        if edge.perp_dot(p - a).abs() < 1e-4 * edge.length()
                            && (p - a).dot(p - z) <= 1e-5
                        {
                            covered[i] += b.width;
                        }
                    }
                }
                self.pieces[id].edges = 7;
                for i in 0..3 {
                    if covered[i] >= vertices[i].distance(vertices[(i + 1) % 3]) - 1e-5 {
                        self.pieces[id].edges &= !(1 << i);
                    }
                }
            } else {
                self.pieces[id].edges = 7;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexed_chunks_and_bonds_match_scans_through_destruction() {
        let mut world = World::new(Scene::Masonry);
        for tick in 0..600 {
            world.step(&[Command::Fire(Shot::new(
                Ammunition::HighExplosive,
                Vector::new(3.0, (tick / 120) as f32 * 0.4 - 0.8),
            ))]);
            if tick % 30 != 0 {
                continue;
            }
            for (key, members) in world.chunk_members() {
                assert_eq!(
                    world.body_members(RigidBodyHandle::from_raw_parts(key.0, key.1)),
                    members
                );
                let scanned: Vec<_> = world
                    .bonds
                    .iter()
                    .enumerate()
                    .filter_map(|(id, b)| {
                        (b.active
                            && (members.binary_search(&b.a).is_ok()
                                || members.binary_search(&b.b).is_ok()))
                        .then_some(id)
                    })
                    .collect();
                assert_eq!(world.member_bonds(&members), scanned);
            }
        }
        assert!(world.refinements > 0);
        assert!(world.broken > 0);
        assert!(world.cleared > 0);
    }

    #[test]
    fn local_splits_match_full_rebuilds_including_supports_and_visible_edges() {
        for scene in [Scene::Masonry, Scene::Cantilever, Scene::BarrelRubble] {
            let mut local = World::new(scene);
            let mut full = World::new(scene);
            for _ in 0..12 {
                let Some(id) = local.bonds.iter().position(|b| b.active) else {
                    break;
                };
                let bond = &local.bonds[id];
                let mut affected: Vec<_> = [bond.a, bond.b]
                    .into_iter()
                    .flat_map(|id| local.body_members(local.pieces[id].body))
                    .collect();
                affected.extend([bond.a, bond.b]);
                affected.sort_unstable();
                affected.dedup();
                local.bonds[id].active = false;
                full.bonds[id].active = false;
                local.structural_cache.clear();
                full.structural_cache.clear();
                local.rebuild_chunks_for(&affected);
                full.rebuild_chunks();
                assert_eq!(
                    format!("{:?}", local.bodies().collect::<Vec<_>>()),
                    format!("{:?}", full.bodies().collect::<Vec<_>>())
                );
            }
        }
    }
}
