use super::*;
use std::collections::{BTreeMap, BTreeSet};

impl World {
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
        let mut adjacency = vec![Vec::new(); self.pieces.len()];
        for b in self.bonds.iter().filter(|b| b.active) {
            if self.bending_torque(b).is_some() {
                continue;
            }
            adjacency[b.a].push(b.b);
            adjacency[b.b].push(b.a);
        }
        let mut visited = vec![false; self.pieces.len()];
        let mut remove = BTreeSet::new();
        for piece in &self.pieces {
            if !piece.active
                && piece.destructible()
                && self.physics.bodies.get(piece.body).is_some()
            {
                remove.insert(piece.body.into_raw_parts());
            }
        }
        for start in 0..self.pieces.len() {
            if visited[start] || !self.pieces[start].active || !self.pieces[start].destructible() {
                continue;
            }
            let mut members = vec![start];
            visited[start] = true;
            let mut fixed = false;
            let mut cursor = 0;
            while cursor < members.len() {
                fixed |= self.pieces[members[cursor]].ground_anchor.is_some();
                for &other in &adjacency[members[cursor]] {
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
        self.update_visible_edges();
        self.rebuild_bending_joints();
    }

    pub(super) fn update_visible_edges(&mut self) {
        let mut covered = vec![[0.0_f32; 3]; self.pieces.len()];
        for b in self.bonds.iter().filter(|b| b.active) {
            if self.pieces[b.a].grain.is_none() || self.pieces[b.a].grain != self.pieces[b.b].grain
            {
                continue;
            }
            let face = self.piece_pose(b.a).transform_point(b.anchor_a);
            for id in [b.a, b.b] {
                let Shape::Triangle { vertices } = self.pieces[id].shape else {
                    continue;
                };
                let p = self.piece_pose(id).inverse_transform_point(face);
                for i in 0..3 {
                    let a = vertices[i];
                    let z = vertices[(i + 1) % 3];
                    let edge = z - a;
                    if edge.perp_dot(p - a).abs() < 1e-4 * edge.length()
                        && (p - a).dot(p - z) <= 1e-5
                    {
                        covered[id][i] += b.width;
                    }
                }
            }
        }
        for (id, p) in self.pieces.iter_mut().enumerate() {
            p.edges = 7;
            if let Shape::Triangle { vertices } = p.shape {
                for i in 0..3 {
                    if covered[id][i] >= vertices[i].distance(vertices[(i + 1) % 3]) - 1e-5 {
                        p.edges &= !(1 << i);
                    }
                }
            }
        }
    }
}
