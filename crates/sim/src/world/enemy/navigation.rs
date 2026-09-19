//! Coarse A* with tank clearance and an optional costly route through breakable cover.
use super::*;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};

impl World {
    pub(in super::super) fn tank_route(
        &self,
        id: usize,
        start: Vector,
        goal: Vector,
    ) -> Vec<Vector> {
        self.plan_tank_route(id, start, goal, false)
    }

    pub(super) fn plan_tank_route(
        &self,
        _id: usize,
        start: Vector,
        goal: Vector,
        breach: bool,
    ) -> Vec<Vector> {
        let cell = (self.arena_size.max_element() / 160.0).max(0.75);
        let width = (self.arena_size.x / cell).ceil() as usize;
        let height = (self.arena_size.y / cell).ceil() as usize;
        let origin = -self.arena_size * 0.5;
        let center = |i: usize| {
            origin + Vector::new((i % width) as f32 + 0.5, (i / width) as f32 + 0.5) * cell
        };
        let index = |p: Vector| {
            let v = ((p - origin) / cell).floor().max(Vector::ZERO);
            (v.y as usize).min(height - 1) * width + (v.x as usize).min(width - 1)
        };
        let mut costs = vec![10_u32; width * height];
        for (i, cost) in costs.iter_mut().enumerate() {
            let p = center(i);
            if p.abs()
                .cmpgt(self.arena_size * 0.5 - Vector::splat(1.3))
                .any()
            {
                *cost = u32::MAX;
            } else if self.terrain_at(p) == crate::TerrainKind::Sand {
                *cost = 17;
            }
        }
        let mut masses = BTreeMap::new();
        for piece in self
            .pieces
            .iter()
            .filter(|p| p.active && p.destructible() && !p.bridge_deck)
        {
            *masses.entry(piece.body.into_raw_parts()).or_insert(0.0) += piece.mass();
        }
        for (_, collider) in self.physics.colliders.iter() {
            if !collider.is_enabled()
                || collider
                    .parent()
                    .is_some_and(|h| !self.physics.bodies[h].is_enabled())
            {
                continue;
            }
            let piece = self.weapon_target(collider);
            let water = self
                .water_colliders
                .iter()
                .any(|h| std::ptr::eq(&self.physics.colliders[*h], collider));
            if !water && piece.is_none() {
                continue;
            }
            if piece.is_some_and(|i| self.pieces[i].material == Material::Launcher) {
                continue;
            }
            let massive = piece.is_some_and(|i| {
                masses
                    .get(&self.pieces[i].body.into_raw_parts())
                    .copied()
                    .unwrap_or(0.0)
                    >= 4500.0
            });
            let hard = water
                || piece.is_some_and(|i| !self.pieces[i].destructible())
                || (massive && !breach);
            let clearance = if hard { 1.25 } else { 0.85 };
            let bounds = collider.compute_aabb();
            let lo = index(bounds.mins - Vector::splat(clearance));
            let hi = index(bounds.maxs + Vector::splat(clearance));
            for y in lo / width..=hi / width {
                for x in lo % width..=hi % width {
                    let at = y * width + x;
                    if costs[at] == u32::MAX {
                        continue;
                    }
                    let point = center(at);
                    let projection =
                        collider
                            .shape()
                            .project_point(collider.position(), point, true);
                    if projection.point.distance(point) <= clearance {
                        costs[at] = if hard {
                            u32::MAX
                        } else {
                            costs[at].max(if massive { 100 } else { 35 })
                        };
                    }
                }
            }
        }
        let nearest_free = |point: Vector| {
            (0..costs.len())
                .filter(|&i| costs[i] != u32::MAX)
                .min_by(|&a, &b| {
                    center(a)
                        .distance_squared(point)
                        .total_cmp(&center(b).distance_squared(point))
                })
        };
        let Some(destination) = nearest_free(goal) else {
            return vec![];
        };
        let source = index(start);
        // A live tank may start beside a wall closer than conservative route clearance.
        costs[source] = 10;
        let mut distance = vec![u32::MAX; costs.len()];
        let mut previous = vec![usize::MAX; costs.len()];
        let mut queue = BinaryHeap::new();
        distance[source] = 0;
        let heuristic = |i: usize| (center(i).distance(center(destination)) / cell * 10.0) as u32;
        queue.push(Reverse((heuristic(source), 0, source)));
        while let Some(Reverse((_, travelled, at))) = queue.pop() {
            if travelled != distance[at] {
                continue;
            }
            if at == destination {
                break;
            }
            let (x, y) = ((at % width) as i32, (at / width) as i32);
            for dy in -1_i32..=1 {
                for dx in -1_i32..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 {
                        continue;
                    }
                    let next = ny as usize * width + nx as usize;
                    if costs[next] == u32::MAX {
                        continue;
                    }
                    if dx != 0
                        && dy != 0
                        && (costs[y as usize * width + nx as usize] == u32::MAX
                            || costs[ny as usize * width + x as usize] == u32::MAX)
                    {
                        continue;
                    }
                    let candidate =
                        travelled + costs[next] * if dx != 0 && dy != 0 { 14 } else { 10 } / 10;
                    if candidate < distance[next] {
                        distance[next] = candidate;
                        previous[next] = at;
                        queue.push(Reverse((candidate + heuristic(next), candidate, next)));
                    }
                }
            }
        }
        if distance[destination] == u32::MAX {
            return vec![];
        }
        let mut route = Vec::new();
        let mut at = destination;
        while at != source {
            route.push(center(at));
            at = previous[at];
        }
        route.reverse();
        // Keep the exact flag/target point when it lies in the reachable goal cell.
        if index(goal) == destination {
            route.push(goal);
        }
        route
    }
}
