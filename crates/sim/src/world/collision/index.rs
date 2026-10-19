//! Live query bounds, independent of Rapier's collision-pair broad phase.
//! Refresh moving shapes on every query. Fixed geometry is validated once per
//! simulation tick and again after structural changes; standalone queries always
//! validate every collider, including direct editor/test pose changes.
use super::*;
use rapier2d::parry::partitioning::Bvh;
use std::sync::Arc;

#[derive(Clone, Copy)]
pub(in crate::world) struct ColliderTarget {
    pub collider: ColliderHandle,
    pub piece: usize,
    pub body: RigidBodyHandle,
    pub bounds: Aabb,
    pub velocity: Vector,
}
struct Entry {
    target: ColliderTarget,
    pose: Pose,
    shape: SharedShape,
    seen: u64,
}
#[derive(Default)]
pub(in crate::world) struct ColliderIndex {
    tree: Bvh,
    entries: Vec<Option<Entry>>,
    revision: u64,
    maximum_velocity: Vector,
    static_velocity: Vector,
    moving: Vec<usize>,
    epoch: u64,
    records: usize,
}
impl ColliderIndex {
    fn refresh(
        &mut self,
        world: &World,
        handle: ColliderHandle,
        collider: &Collider,
        piece: usize,
    ) {
        let body = collider.parent().unwrap();
        let velocity = world.physics.bodies[body].linvel();
        self.maximum_velocity = self.maximum_velocity.max(velocity.abs());
        let index = handle.into_raw_parts().0 as usize;
        if index >= self.entries.len() {
            self.entries.resize_with(index + 1, || None);
        }
        let changed = self.entries[index].as_ref().is_none_or(|old| {
            old.target.collider != handle
                || old.pose != *collider.position()
                || !Arc::ptr_eq(&old.shape.0, &collider.shared_shape().0)
        });
        if changed {
            let bounds = collider.compute_aabb();
            self.tree.insert(bounds, index as u32);
            self.entries[index] = Some(Entry {
                target: ColliderTarget {
                    collider: handle,
                    piece,
                    body,
                    bounds,
                    velocity,
                },
                pose: *collider.position(),
                shape: collider.shared_shape().clone(),
                seen: self.revision,
            });
        } else if let Some(entry) = &mut self.entries[index] {
            entry.target.velocity = velocity;
            entry.target.piece = piece;
            entry.target.body = body;
            entry.seen = self.revision;
        }
    }

    fn sync(&mut self, world: &World) {
        if world.in_tick && self.epoch == world.query_epoch && self.records == world.pieces.len() {
            self.maximum_velocity = self.static_velocity;
            let moving = std::mem::take(&mut self.moving);
            for &index in &moving {
                let Some(entry) = &self.entries[index] else {
                    continue;
                };
                let handle = entry.target.collider;
                if let Some(collider) = world.physics.colliders.get(handle)
                    && let Some(piece) = world.weapon_target(collider)
                {
                    self.refresh(world, handle, collider, piece);
                } else {
                    self.tree.remove(index as u32);
                    self.entries[index] = None;
                }
            }
            self.moving = moving;
            return;
        }
        self.revision += 1;
        self.maximum_velocity = Vector::ZERO;
        self.static_velocity = Vector::ZERO;
        self.moving.clear();
        for (handle, collider) in world.physics.colliders.iter() {
            let Some(piece) = world.weapon_target(collider) else {
                continue;
            };
            self.refresh(world, handle, collider, piece);
            let body = &world.physics.bodies[collider.parent().unwrap()];
            if body.is_fixed() {
                self.static_velocity = self.static_velocity.max(body.linvel().abs());
            } else {
                self.moving.push(handle.into_raw_parts().0 as usize);
            }
        }
        for (index, entry) in self.entries.iter_mut().enumerate() {
            if entry.as_ref().is_some_and(|old| old.seen != self.revision) {
                self.tree.remove(index as u32);
                *entry = None;
            }
        }
        self.epoch = world.query_epoch;
        self.records = world.pieces.len();
    }

    pub fn sweep(&mut self, world: &World, bounds: Aabb, horizon: f32) -> Vec<ColliderTarget> {
        self.sync(world);
        // A moving target can enter the bullet's path during this interval.
        // Bounds only choose candidates; the original relative-velocity cast
        // still decides the exact hit, normal and time of impact.
        let reach = self.maximum_velocity * horizon + Vector::splat(0.0001);
        let bounds = Aabb::new(bounds.mins - reach, bounds.maxs + reach);
        let mut indices: Vec<_> = self
            .tree
            .leaves(|node| node.aabb().intersects(&bounds))
            .collect();
        // Preserve collider arena order for equal-time impact ties.
        indices.sort_unstable();
        indices
            .into_iter()
            .filter_map(|index| {
                self.entries[index as usize]
                    .as_ref()
                    .map(|entry| entry.target)
            })
            .collect()
    }

    pub fn nearby(&mut self, world: &World, nearby: &dyn Fn(Aabb) -> bool) -> Vec<ColliderTarget> {
        self.sync(world);
        self.entries
            .iter()
            .flatten()
            .filter(|entry| nearby(entry.target.bounds))
            .map(|entry| entry.target)
            .collect()
    }
}
