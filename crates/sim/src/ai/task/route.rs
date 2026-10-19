//! Geometry and complete-route planning. No combat or teammate control here.
use super::*;

pub(super) enum Plan {
    Complete(Vec<Vec2>),
    Unreachable { nearest: Vec2 },
}

pub(super) struct Navigation {
    pub map: Map,
    pub cells: Vec<u8>,
    pub revision: u64,
    costs: Vec<u32>,
    built: Option<(u64, VehicleKind)>,
    retired: Vec<bool>,
    local_bins: Vec<Vec<usize>>,
    seen_cells: Vec<bool>,
}
impl Navigation {
    pub fn new(map: Map) -> Self {
        let n = map.width() * map.height();
        Self {
            cells: if map.layout.len() == n {
                map.layout.clone()
            } else {
                vec![0; n]
            },
            retired: vec![false; map.emplacements.len()],
            map,
            revision: 0,
            costs: vec![],
            built: None,
            local_bins: vec![vec![]; n],
            seen_cells: vec![false; n],
        }
    }
    pub fn observe(&mut self, o: &Observation) {
        let mut changed = false;
        for &(i, value) in &o.terrain {
            if let Some(old) = self.cells.get_mut(i) {
                changed |= *old != value;
                *old = value;
            }
        }
        for b in &o.obstacles {
            let lo = b
                .outline
                .iter()
                .copied()
                .fold(Vec2::splat(f32::INFINITY), Vec2::min);
            let hi = b
                .outline
                .iter()
                .copied()
                .fold(Vec2::splat(f32::NEG_INFINITY), Vec2::max);
            let a = self.map.index(lo - Vec2::splat(self.map.cell()));
            let z = self.map.index(hi + Vec2::splat(self.map.cell()));
            for y in a / self.map.width()..=z / self.map.width() {
                for x in a % self.map.width()..=z % self.map.width() {
                    let i = y * self.map.width() + x;
                    if polygon_distance(self.map.point(i), &b.outline) < self.map.cell() * 0.6 {
                        let value = if b.breakable { 2 } else { 4 };
                        changed |= self.cells[i] != value;
                        self.cells[i] = value;
                    }
                }
            }
        }
        for (i, gun) in self.map.emplacements.iter().enumerate() {
            if !self.retired[i]
                && (o
                    .visible_ruins
                    .iter()
                    .any(|p| p.distance(gun.position) < 1.0)
                    || o.allies.iter().any(|p| p.distance(gun.position) < 0.5))
            {
                self.retired[i] = true;
                changed = true;
            }
        }
        if changed {
            self.revision += 1;
        }
        for bin in &mut self.local_bins {
            bin.clear();
        }
        for (i, b) in o.obstacles.iter().enumerate() {
            let lo = b
                .outline
                .iter()
                .copied()
                .fold(Vec2::splat(f32::INFINITY), Vec2::min)
                - Vec2::splat(4.0);
            let hi = b
                .outline
                .iter()
                .copied()
                .fold(Vec2::splat(f32::NEG_INFINITY), Vec2::max)
                + Vec2::splat(4.0);
            let a = self.map.index(lo);
            let z = self.map.index(hi);
            let w = self.map.width();
            for y in a / w..=z / w {
                for x in a % w..=z % w {
                    self.local_bins[y * w + x].push(i);
                }
            }
        }
        for (i, seen) in self.seen_cells.iter_mut().enumerate() {
            *seen = self.cells[i] < 2 || o.visible(self.map.point(i));
        }
    }
    pub fn clearance(&self, p: Vec2, kind: VehicleKind) -> f32 {
        let radius = 1.36 * kind.size().max_element();
        let mut clear = (self.map.size * 0.5 - p.abs()).min_element() - radius;
        let at = self.map.index(p);
        let w = self.map.width();
        for dy in -4isize..=4 {
            for dx in -4isize..=4 {
                let x = (at % w) as isize + dx;
                let y = (at / w) as isize + dy;
                if x < 0 || y < 0 || x >= w as isize || y >= self.map.height() as isize {
                    continue;
                }
                let i = y as usize * w + x as usize;
                if self.cells[i] >= 2 {
                    clear =
                        clear.min(p.distance(self.map.point(i)) - self.map.cell() * 0.71 - radius);
                }
            }
        }
        clear
    }
    pub fn corridor(&self, a: Vec2, b: Vec2, kind: VehicleKind) -> bool {
        let steps = (a.distance(b) / self.map.cell() * 3.0).ceil() as usize;
        let initial = self.clearance(a, kind).min(0.0);
        (0..=steps).all(|n| {
            self.clearance(a.lerp(b, n as f32 / steps.max(1) as f32), kind) >= initial - 0.03
        })
    }
    pub fn local_clearance(&self, o: &Observation, p: Vec2) -> f32 {
        // Keep water/cliff clearance conservative. For visible solid geometry,
        // use its actual boundary instead of adding grid quantisation twice.
        let radius = o.vehicle.size().length();
        let mut clear = (self.map.size * 0.5 - p.abs()).min_element() - radius;
        let at = self.map.index(p);
        let w = self.map.width();
        for dy in -4isize..=4 {
            for dx in -4isize..=4 {
                let x = (at % w) as isize + dx;
                let y = (at / w) as isize + dy;
                if x < 0 || y < 0 || x >= w as isize || y >= self.map.height() as isize {
                    continue;
                }
                let i = y as usize * w + x as usize;
                if self.cells[i] == 3 || (self.cells[i] >= 2 && !self.seen_cells[i]) {
                    clear =
                        clear.min(p.distance(self.map.point(i)) - self.map.cell() * 0.71 - radius);
                }
            }
        }
        for &i in &self.local_bins[at] {
            let b = &o.obstacles[i];
            clear = clear.min(polygon_distance(p, &b.outline) - radius);
        }
        clear
    }
    pub fn nearby<'a>(&'a self, o: &'a Observation, p: Vec2) -> impl Iterator<Item = &'a Obstacle> {
        self.local_bins[self.map.index(p)]
            .iter()
            .map(|&i| &o.obstacles[i])
    }
    fn aligned_corridor(&self, o: &Observation, end: Vec2) -> bool {
        let delta = end - o.position;
        let error = angle_error(delta.to_angle(), o.hull_angle).abs();
        if o.regions.is_empty() || (error > 0.04 && (std::f32::consts::PI - error).abs() > 0.04) {
            return false;
        }
        let axis = delta.normalize_or_zero();
        let side = axis.perp() * o.vehicle.size().y * 1.05;
        let a = o.position - axis * o.vehicle.size().x * 1.05;
        let z = end + axis * o.vehicle.size().x * 1.05;
        let swept = [a + side, z + side, z - side, a - side];
        if swept
            .iter()
            .any(|p| p.abs().cmpge(self.map.size * 0.5).any())
        {
            return false;
        }
        let steps = (a.distance(z) * 4.0).ceil() as usize;
        if !(0..=steps).all(|n| {
            [-1.0, 0.0, 1.0].into_iter().all(|offset| {
                let p = a.lerp(z, n as f32 / steps.max(1) as f32) + side * offset;
                let v = self.cells[self.map.index(p)];
                v != 3 && (v < 2 || o.visible(p))
            })
        }) {
            return false;
        }
        o.obstacles.iter().all(|b| !overlap(&swept, &b.outline))
    }
    pub fn initial_corridor(&self, o: &Observation, end: Vec2) -> bool {
        self.corridor(o.position, end, o.vehicle) || self.aligned_corridor(o, end)
    }
    pub fn public_safe(&self, point: Vec2) -> bool {
        self.public_risk(point) == 0
    }
    pub fn public_risk(&self, point: Vec2) -> usize {
        self.map
            .emplacements
            .iter()
            .enumerate()
            .filter(|(i, gun)| {
                let d = point - gun.position;
                gun.team != self.map.team
                    && !self.retired[*i]
                    && d.length() < gun.range + 2.0
                    && angle_error(d.to_angle(), gun.facing).abs()
                        < gun.arc.to_radians() * 0.5 + 0.08
            })
            .count()
    }
    pub fn route_corridor(&self, a: Vec2, b: Vec2, kind: VehicleKind) -> bool {
        if !self.corridor(a, b, kind) {
            return false;
        }
        let risk = self.public_risk(a).max(self.public_risk(b));
        let steps = (a.distance(b) / self.map.cell() * 3.0).ceil() as usize;
        (0..=steps).all(|n| self.public_risk(a.lerp(b, n as f32 / steps.max(1) as f32)) <= risk)
    }
    fn build(&mut self, kind: VehicleKind) {
        if self.built == Some((self.revision, kind)) {
            return;
        }
        let w = self.map.width();
        let h = self.map.height();
        let margin = 1.36 * kind.size().max_element() + self.map.cell() * 0.71;
        let reach = (margin / self.map.cell()).ceil() as isize;
        self.costs = vec![10; self.cells.len()];
        for (i, &v) in self.cells.iter().enumerate() {
            if self
                .map
                .point(i)
                .abs()
                .cmpgt(self.map.size * 0.5 - Vec2::splat(1.4))
                .any()
            {
                self.costs[i] = u32::MAX;
            }
            if v < 2 {
                continue;
            }
            for dy in -reach..=reach {
                for dx in -reach..=reach {
                    let x = (i % w) as isize + dx;
                    let y = (i / w) as isize + dy;
                    if x >= 0
                        && y >= 0
                        && x < w as isize
                        && y < h as isize
                        && ((dx * dx + dy * dy) as f32).sqrt() * self.map.cell() < margin
                    {
                        self.costs[y as usize * w + x as usize] = u32::MAX;
                    }
                }
            }
        }
        for (i, gun) in self.map.emplacements.iter().enumerate() {
            if gun.team == self.map.team || self.retired[i] {
                continue;
            }
            for (at, cost) in self.costs.iter_mut().enumerate() {
                let d = self.map.point(at) - gun.position;
                if *cost != u32::MAX
                    && d.length() < gun.range + 2.0
                    && angle_error(d.to_angle(), gun.facing).abs()
                        < gun.arc.to_radians() * 0.5 + 0.08
                {
                    *cost += 2000;
                }
            }
        }
        self.built = Some((self.revision, kind));
    }
    pub fn plan(&mut self, start: Vec2, goal: Vec2, kind: VehicleKind) -> Plan {
        self.plan_impl(start, goal, kind, None)
    }
    pub fn plan_observed(&mut self, o: &Observation, goal: Vec2) -> Plan {
        self.plan_impl(o.position, goal, o.vehicle, Some(o))
    }
    fn plan_impl(
        &mut self,
        start: Vec2,
        goal: Vec2,
        kind: VehicleKind,
        observation: Option<&Observation>,
    ) -> Plan {
        self.build(kind);
        let w = self.map.width();
        let h = self.map.height();
        let source = self.map.index(start);
        let destination = self.map.index(goal);
        let heuristic =
            |i: usize| (self.map.point(i).distance(goal) / self.map.cell() * 10.0) as u32;
        let mut distance = vec![u32::MAX; self.cells.len()];
        let mut previous = vec![usize::MAX; self.cells.len()];
        let mut departure = vec![None; self.cells.len()];
        let mut queue = BinaryHeap::new();
        distance[source] = 0;
        queue.push(Reverse((heuristic(source), 0, source)));
        // The actual pose can be clear while its rounded grid centre is
        // blocked. Connect it to nearby graph nodes through continuously
        // checked corridors, rather than requiring a step through that centre
        // or relaxing the entire obstacle margin around spawn.
        for dy in -3isize..=3 {
            for dx in -3isize..=3 {
                let x = (source % w) as isize + dx;
                let y = (source / w) as isize + dy;
                if x < 0 || y < 0 || x >= w as isize || y >= h as isize {
                    continue;
                }
                let i = y as usize * w + x as usize;
                let p = self.map.point(i);
                if i != source
                    && self.costs[i] != u32::MAX
                    && p.distance(start) <= 3.0
                    && self.corridor(start, p, kind)
                {
                    let d =
                        (p.distance(start) / self.map.cell() * 10.0).ceil() as u32 + self.costs[i];
                    distance[i] = d;
                    previous[i] = source;
                    queue.push(Reverse((d + heuristic(i), d, i)));
                }
            }
        }
        // A narrow garage admits an aligned hull but not its turning circle.
        // Add a straight departure edge verified against visible polygons;
        // retain that edge explicitly in the returned route.
        if let Some(o) = observation {
            for sign in [1.0, -1.0] {
                for metres in 2..=8 {
                    let p = start + Vec2::from_angle(o.hull_angle) * sign * metres as f32;
                    if self.clearance(p, kind) < 0.0 || !self.aligned_corridor(o, p) {
                        continue;
                    }
                    let i = self.map.index(p);
                    let node = self.map.point(i);
                    if i == source || self.costs[i] == u32::MAX || !self.corridor(p, node, kind) {
                        continue;
                    }
                    let d =
                        (start.distance(p) / self.map.cell() * 10.0).ceil() as u32 + self.costs[i];
                    if d < distance[i] {
                        distance[i] = d;
                        previous[i] = source;
                        departure[i] = Some(p);
                        queue.push(Reverse((d + heuristic(i), d, i)));
                    }
                }
            }
        }
        let mut closest = source;
        while let Some(Reverse((_, travelled, at))) = queue.pop() {
            if distance[at] != travelled {
                continue;
            }
            if heuristic(at) < heuristic(closest) {
                closest = at;
            }
            if at == destination {
                let mut path = vec![goal];
                let mut p = at;
                while p != source {
                    path.push(self.map.point(p));
                    if let Some(point) = departure[p] {
                        path.push(point);
                    }
                    p = previous[p];
                }
                path.reverse();
                return Plan::Complete(path);
            }
            let x = (at % w) as isize;
            let y = (at / w) as isize;
            for dy in -1isize..=1 {
                for dx in -1isize..=1 {
                    let nx = x + dx;
                    let ny = y + dy;
                    if (dx == 0 && dy == 0)
                        || nx < 0
                        || ny < 0
                        || nx >= w as isize
                        || ny >= h as isize
                    {
                        continue;
                    }
                    let next = ny as usize * w + nx as usize;
                    let escape = self.cells[next] < 2
                        && self.map.point(next).distance(start) < 3.0
                        && self.clearance(self.map.point(next), kind)
                            >= self.clearance(start, kind) - 0.03;
                    if self.costs[next] == u32::MAX && !escape {
                        continue;
                    }
                    if dx != 0
                        && dy != 0
                        && !escape
                        && (self.costs[y as usize * w + nx as usize] == u32::MAX
                            || self.costs[ny as usize * w + x as usize] == u32::MAX)
                    {
                        continue;
                    }
                    let d = travelled
                        + if escape && self.costs[next] == u32::MAX {
                            1000
                        } else {
                            self.costs[next] * if dx != 0 && dy != 0 { 14 } else { 10 } / 10
                        };
                    if d < distance[next] {
                        distance[next] = d;
                        previous[next] = at;
                        departure[next] = None;
                        queue.push(Reverse((d + heuristic(next), d, next)));
                    }
                }
            }
        }
        Plan::Unreachable {
            nearest: self.map.point(closest),
        }
    }
}

fn overlap(a: &[Vec2], b: &[Vec2]) -> bool {
    if a.len() < 3 || b.len() < 3 {
        return false;
    }
    [a, b].into_iter().all(|poly| {
        poly.iter()
            .zip(poly.iter().cycle().skip(1))
            .take(poly.len())
            .all(|(p, z)| {
                let axis = (*z - *p).perp();
                if axis.length_squared() < 1e-10 {
                    return true;
                }
                let range = |vertices: &[Vec2]| {
                    vertices
                        .iter()
                        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), v| {
                            let d = v.dot(axis);
                            (lo.min(d), hi.max(d))
                        })
                };
                let (alo, ahi) = range(a);
                let (blo, bhi) = range(b);
                ahi > blo && bhi > alo
            })
    })
}
