//! Capturing either ruined outpost restores the original bridge, not its debris.
use super::*;
use crate::{BRIDGE_REPAIR_RADIUS, BRIDGE_REPAIR_SECONDS, BridgeRepairView, level::Team};

const REPAIR_TICKS: u32 = (BRIDGE_REPAIR_SECONDS * 120.0) as u32;

pub(in crate::world) struct Blueprint {
    pieces: Vec<(usize, Piece, Pose)>,
    bonds: Vec<(usize, Bond)>,
}

impl Blueprint {
    pub(in crate::world) fn capture(world: &World, object: usize) -> Self {
        Self {
            pieces: world
                .pieces
                .iter()
                .enumerate()
                .filter(|(_, p)| p.object == Some(object))
                .map(|(id, p)| (id, p.clone(), world.piece_pose(id)))
                .collect(),
            bonds: world
                .bonds
                .iter()
                .enumerate()
                .filter(|(_, b)| {
                    world.pieces[b.a].object == Some(object)
                        || world.pieces[b.b].object == Some(object)
                })
                .map(|(id, b)| (id, b.clone()))
                .collect(),
        }
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct Capture {
    team: Option<Team>,
    ticks: u32,
    contested: bool,
}
impl Capture {
    pub(super) fn view(self) -> BridgeRepairView {
        BridgeRepairView {
            team: self.team,
            progress: self.ticks as f32 / REPAIR_TICKS as f32,
            contested: self.contested,
        }
    }
    fn advance(&mut self, blue: bool, red: bool) {
        self.contested = blue && red;
        if self.contested {
            return;
        }
        let team = if blue {
            Some(Team::Blue)
        } else if red {
            Some(Team::Red)
        } else {
            None
        };
        if team != self.team || team.is_none() {
            self.ticks = 0;
            self.team = team;
        }
        if team.is_some() {
            self.ticks = (self.ticks + 1).min(REPAIR_TICKS);
        }
    }
}

impl World {
    pub(in crate::world) fn update_bridge_repairs(&mut self) {
        if !self.bridges.iter().any(|b| b.fall.is_some()) {
            return;
        }
        let mut tanks: Vec<_> = self
            .tanks
            .iter()
            .filter(|t| t.mount.is_none() && self.tank_is_alive(t.id))
            .map(|t| (t.id, t.team, self.piece_pose(t.id).translation))
            .collect();
        // The terrain sandbox has one unregistered test tank.
        if self.tanks.is_empty() && self.tank_alive() {
            tanks.push((self.launcher, self.team, self.launcher_position()));
        }
        for index in 0..self.bridges.len() {
            if self.bridges[index].fall.is_none() {
                continue;
            }
            let mut finish = false;
            for outpost in 0..2 {
                let position = self.bridges[index].controls[outpost];
                let present = |team| {
                    tanks.iter().any(|(_, t, p)| {
                        *t == team
                            && p.distance_squared(position)
                                <= BRIDGE_REPAIR_RADIUS * BRIDGE_REPAIR_RADIUS
                    })
                };
                let capture = &mut self.bridges[index].repairs[outpost];
                let was_ready = capture.ticks == REPAIR_TICKS;
                capture.advance(present(Team::Blue), present(Team::Red));
                // If a crowded site cannot safely restore, retry four times a second.
                finish |= capture.ticks == REPAIR_TICKS
                    && !capture.contested
                    && capture.team.is_some()
                    && (!was_ready || self.tick.is_multiple_of(30));
            }
            if finish {
                self.restore_bridge(index);
            }
        }
    }

    fn clear_of_restored_booths(&self, bridge: usize, tank: usize, pose: Pose) -> bool {
        let body = &self.physics.bodies[self.pieces[tank].body];
        self.bridges[bridge]
            .blueprint
            .pieces
            .iter()
            .filter(|(_, p, _)| !p.bridge_deck)
            .all(|(_, p, at)| {
                let Shape::Triangle { vertices } = p.shape else {
                    return true;
                };
                let shape =
                    rapier2d::parry::shape::Triangle::new(vertices[0], vertices[1], vertices[2]);
                body.colliders().iter().all(|&handle| {
                    let collider = &self.physics.colliders[handle];
                    let tank_pose = pose * body.position().inverse() * *collider.position();
                    !rapier2d::parry::query::intersection_test(
                        &tank_pose,
                        collider.shape(),
                        at,
                        &shape,
                    )
                    .unwrap_or(true)
                })
            })
    }

    fn restore_bridge(&mut self, index: usize) -> bool {
        // Move only tanks occupying a returning booth, to the closest clear dry
        // position. Plan every move before changing the world; defer if crowded.
        let mut tanks: Vec<_> = self
            .tanks
            .iter()
            .filter(|t| t.mount.is_none() && self.tank_is_alive(t.id))
            .map(|t| t.id)
            .collect();
        if self.tanks.is_empty() && self.tank_alive() {
            tanks.push(self.launcher);
        }
        let mut moves: Vec<(usize, Pose)> = vec![];
        for id in tanks {
            let pose = self.piece_pose(id);
            if self.clear_of_restored_booths(index, id, pose) {
                continue;
            }
            let candidate = (0..256).find_map(|n| {
                let radius = (n / 16 + 1) as f32 * 0.25;
                let angle = n as f32 * std::f32::consts::TAU / 16.0;
                let position = pose.translation + Vector::from_angle(angle) * radius;
                let next = Pose::new(position, pose.rotation.angle());
                (moves
                    .iter()
                    .all(|(_, p)| p.translation.distance_squared(position) >= 3.0 * 3.0)
                    && self.clear_of_restored_booths(index, id, next)
                    && self.tank_pose_clear(id, next))
                .then_some(next)
            });
            let Some(candidate) = candidate else {
                return false;
            };
            moves.push((id, candidate));
        }
        for (id, pose) in moves {
            let p = &mut self.pieces[id];
            let body = &mut self.physics.bodies[p.body];
            body.set_position(pose, true);
            body.set_linvel(Vector::ZERO, true);
            body.set_angvel(0.0, true);
            p.previous_position = pose.translation;
            p.previous_angle = pose.rotation.angle();
        }

        // Reuse original cell and bond IDs. Repeated repairs don't append a new
        // bridge or accumulate replacement physics/material records.
        let object = self.bridges[index].object;
        for bond in &mut self.bonds {
            if self.pieces[bond.a].object == Some(object)
                || self.pieces[bond.b].object == Some(object)
            {
                bond.active = false;
            }
        }
        for (id, original, pose) in &self.bridges[index].blueprint.pieces {
            let mut piece = original.clone();
            let body = self
                .physics
                .insert_body(RigidBodyBuilder::dynamic().pose(*pose));
            let collider = match piece.shape {
                Shape::Triangle { vertices } => {
                    ColliderBuilder::convex_hull(&vertices).expect("original bridge cell")
                }
                Shape::Box { half } => ColliderBuilder::cuboid(half.x, half.y),
                Shape::Ball { radius } => ColliderBuilder::ball(radius),
            }
            .density(piece.solid.density * piece.solid.depth)
            .friction(piece.floor_friction)
            .restitution(piece.restitution)
            .collision_groups(piece.collision_layer().groups())
            .active_hooks(contacts::hooks_for(piece.soft_bounce))
            .user_data((*id + 1) as u128);
            piece.body = body;
            piece.collider = self.physics.insert_collider(collider, Some(body));
            piece.local_pose = Pose::IDENTITY;
            piece.shape_revision = self.pieces[*id].shape_revision.wrapping_add(1);
            self.pieces[*id] = piece;
        }
        for (id, original) in &self.bridges[index].blueprint.bonds {
            self.bonds[*id] = original.clone();
        }
        let bridge = &mut self.bridges[index];
        bridge.fall = None;
        bridge.fragments.clear();
        bridge.repairs = Default::default();
        bridge.checked = None;
        self.structural_cache.clear();
        self.sight_cache = None;
        self.rebuild_chunks();
        self.rebuild_terrain_barriers();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collapse(world: &mut World) {
        let object = world.bridges[0].object;
        for bond in &mut world.bonds {
            if world.pieces[bond.a].object == Some(object)
                || world.pieces[bond.b].object == Some(object)
            {
                bond.active = false;
            }
        }
        world.bridges[0].checked = None;
        world.update_bridges(0.0, &mut vec![]);
        assert!(!world.bridges().next().unwrap().intact);
    }

    fn move_tank(world: &mut World, position: Vector) {
        let body = world.pieces[world.launcher].body;
        world.physics.bodies[body].set_translation(position, true);
        world.physics.bodies[body].set_linvel(Vector::ZERO, true);
        world
            .physics
            .bodies
            .propagate_modified_body_positions_to_colliders(&mut world.physics.colliders);
    }

    fn approach(world: &mut World, outpost: usize) {
        let control = world.bridges[0].controls[outpost];
        let position = (0..64)
            .find_map(|n| {
                let position =
                    control + Vector::from_angle(n as f32 * std::f32::consts::TAU / 64.0) * 0.98;
                world
                    .tank_pose_clear(world.launcher, Pose::new(position, 0.0))
                    .then_some(position)
            })
            .expect("ruined outpost is accessible within one metre");
        move_tank(world, position);
    }

    #[test]
    fn either_outpost_repairs_at_fifteen_seconds_and_cells_are_reused() {
        let mut world = World::new(Scene::Terrain);
        let counts = (world.pieces.len(), world.bonds.len(), world.objects.len());
        let physics_counts = (world.physics.bodies.len(), world.physics.colliders.len());
        for outpost in [0, 1, 0] {
            move_tank(&mut world, Vector::new(-7.0, 0.0));
            collapse(&mut world);
            approach(&mut world, outpost);
            for _ in 0..REPAIR_TICKS - 1 {
                world.step(&[]);
            }
            assert!(!world.bridges().next().unwrap().intact);
            let before = world.bridges().next().unwrap().repairs[outpost];
            assert_eq!(before.team, Some(Team::Blue));
            assert!(before.progress > 0.99 && before.progress < 1.0);
            world.step(&[]);
            assert!(world.bridges().next().unwrap().intact);
            assert!(world.bridge_supported(&world.bridges[0]));
            assert_eq!(world.terrain_at(Vector::ZERO), TerrainKind::Bridge);
            assert_eq!(
                counts,
                (world.pieces.len(), world.bonds.len(), world.objects.len())
            );
            assert_eq!(
                physics_counts,
                (world.physics.bodies.len(), world.physics.colliders.len())
            );
            assert!(
                world.tank_pose_clear(world.launcher, world.piece_pose(world.launcher)),
                "repair safely clears the returning booth"
            );
            for _ in 0..30 {
                world.step(&[]);
            }
            assert!(
                world.bridges().next().unwrap().intact,
                "restored supports stay connected"
            );
            assert!(world.tank_alive());
        }
        world
            .configure_test_tank(4500.0, Vector::new(-7.0, 0.0), 4.6)
            .unwrap();
        for _ in 0..600 {
            world.step(&[Command::Tracks {
                left: 1.0,
                right: 1.0,
            }]);
        }
        assert!(
            world.launcher_position().x > 4.0,
            "restored bridge can be crossed: {:?}",
            world.launcher_position()
        );
        assert!(world.tank_alive());
    }

    #[test]
    fn shell_damaged_bridge_can_be_repaired_and_destroyed_again() {
        let mut world = World::new(Scene::Terrain);
        for _ in 0..2 {
            world
                .configure_test_tank(4500.0, Vector::new(-7.0, 2.1), 4.6)
                .unwrap();
            for tick in 0..960 {
                let commands = if tick % 60 == 0 {
                    vec![Command::Fire(Shot::new(
                        Ammunition::HighExplosive,
                        Vector::new(-3.0, 2.05),
                    ))]
                } else {
                    vec![]
                };
                world.step(&commands);
                if !world.bridges().next().unwrap().intact {
                    break;
                }
            }
            assert!(!world.bridges().next().unwrap().intact);
            approach(&mut world, 1);
            for _ in 0..REPAIR_TICKS {
                world.step(&[]);
            }
            assert!(world.bridges().next().unwrap().intact);
            assert!(world.bridge_supported(&world.bridges[0]));
            assert!(world.tank_alive());
        }
    }

    #[test]
    fn capture_resets_on_leaving_or_changing_teams_and_pauses_when_contested() {
        let mut capture = Capture::default();
        for _ in 0..600 {
            capture.advance(true, false);
        }
        let progress = capture.ticks;
        for _ in 0..600 {
            capture.advance(true, true);
        }
        assert_eq!(capture.ticks, progress);
        assert!(capture.view().contested);
        capture.advance(true, false);
        assert_eq!(capture.ticks, progress + 1);
        capture.advance(false, false);
        assert_eq!(capture.view(), BridgeRepairView::default());
        capture.advance(true, false);
        capture.advance(false, true);
        assert_eq!(capture.team, Some(Team::Red));
        assert_eq!(capture.ticks, 1);
    }

    #[test]
    fn repair_requires_a_living_tank_center_within_one_metre() {
        let mut world = World::new(Scene::Terrain);
        collapse(&mut world);
        let control = world.bridges[0].controls[0];
        move_tank(&mut world, control + Vector::X * 1.001);
        world.update_bridge_repairs();
        assert_eq!(world.bridges().next().unwrap().repairs[0].progress, 0.0);
        move_tank(&mut world, control + Vector::X);
        world.update_bridge_repairs();
        assert!(world.bridges().next().unwrap().repairs[0].progress > 0.0);
        world.tank_fall = Some(0.0);
        world.update_bridge_repairs();
        assert_eq!(
            world.bridges().next().unwrap().repairs[0],
            BridgeRepairView::default()
        );
    }

    #[test]
    fn restoration_works_for_vertical_and_flipped_bridges_and_replicas() {
        use crate::level::{Level, LevelItem};
        for vertical in [false, true] {
            for flipped in [false, true] {
                let mut level = Level::default();
                level.items.push(LevelItem::Bridge {
                    min: [-3.0, -3.0],
                    max: [3.0, 3.0],
                    vertical,
                    flipped,
                });
                let mut world = World::from_level(&level, Team::Blue).unwrap();
                world.enemy = None;
                collapse(&mut world);
                approach(&mut world, 1);
                for _ in 0..600 {
                    world.update_bridge_repairs();
                }
                let frame = replica::Frame::capture(&world);
                let client = World::from_replica(replica::Map::capture(&world), frame);
                assert_eq!(
                    client.bridges().next().unwrap().repairs,
                    world.bridges().next().unwrap().repairs
                );
                for _ in 600..REPAIR_TICKS {
                    world.update_bridge_repairs();
                }
                assert!(world.bridges().next().unwrap().intact);
                assert!(world.bridge_supported(&world.bridges[0]));
                let client = World::from_replica(
                    replica::Map::capture(&world),
                    replica::Frame::capture(&world),
                );
                assert!(client.bridges().next().unwrap().intact);
                assert_eq!(
                    client.bridges().next().unwrap().repairs,
                    [BridgeRepairView::default(); 2]
                );
            }
        }
    }
}
