//! Map instantiation and nonphysical team objectives.
use super::*;
use crate::level::{
    FLAG_RADIUS, Level, LevelItem, Team, bridge_asset, bridge_mesh, rotate_asset_mesh, solid_asset,
};
mod supports;

pub(super) struct Flag {
    pub team: Team,
    pub position: Vector,
}

impl World {
    pub fn team(&self) -> Team {
        self.team
    }
    pub fn winner(&self) -> Option<Team> {
        self.winner
    }
    pub fn flags(&self) -> impl Iterator<Item = (Team, Vector)> + '_ {
        self.flags.iter().map(|f| (f.team, f.position))
    }
    pub fn from_level(level: &Level, team: Team) -> Result<Self, String> {
        level.validate()?;
        let (position, angle) = level
            .items
            .iter()
            .find_map(|i| match i {
                LevelItem::Spawn {
                    team: t,
                    position,
                    angle,
                } if *t == team => Some((Vector::from(*position), *angle)),
                _ => None,
            })
            .ok_or_else(|| format!("Place a {} spawn before testing this team.", team.label()))?;
        let mut w = Self::new(Scene::Level);
        w.team = team;
        w.arena_size = Vector::from(level.size);
        w.terrain = level.terrain_mask.patches();
        if !level
            .items
            .iter()
            .any(|i| matches!(i, LevelItem::Terrain { .. }))
        {
            w.terrain_mask = Some(level.terrain_mask.clone());
        }
        // Keep the normal world constructor and replace its four arena edges.
        let half = Vector::from(level.size) * 0.5;
        for (id, (center, size)) in [
            (Vector::new(0.0, -half.y), Vector::new(half.x, 0.15)),
            (Vector::new(0.0, half.y), Vector::new(half.x, 0.15)),
            (Vector::new(-half.x, 0.0), Vector::new(0.15, half.y)),
            (Vector::new(half.x, 0.0), Vector::new(0.15, half.y)),
        ]
        .into_iter()
        .enumerate()
        {
            let p = &mut w.pieces[id];
            w.physics.bodies[p.body].set_translation(center, true);
            w.physics.colliders[p.collider].set_shape(SharedShape::cuboid(size.x, size.y));
            p.shape = Shape::Box { half: size };
            p.previous_position = center;
        }
        let mut wall_edges = Vec::new();
        let mut solid_edges = Vec::new();
        let meshes = level.resolved_meshes()?;
        for (index, item) in level.items.iter().enumerate() {
            match item {
                LevelItem::Object {
                    asset,
                    position,
                    angle,
                    ..
                } => {
                    if let Some(kind) = asset.components.decoration {
                        w.add_decoration(kind, Vector::from(*position), *angle, item.dimensions());
                        continue;
                    }
                    let mut mesh = meshes[index].clone().unwrap();
                    if mesh.triangles.is_empty() {
                        continue;
                    }
                    let mut asset = (**asset).clone();
                    let wall = asset.surface == crate::Surface::Masonry
                        && asset.components.plasticity.is_none();
                    if wall {
                        // Level walls attach to authored solids, including old
                        // drafts that still contain automatic footing settings.
                        asset.pinned_edge = None;
                        asset.additional_pinned_edges.clear();
                        asset.support_spacing = None;
                    }
                    rotate_asset_mesh(&mut asset, &mut mesh, *angle);
                    if wall {
                        wall_edges.extend(supports::boundary_edges(
                            &mesh,
                            w.pieces.len(),
                            Vector::from(*position),
                        ));
                    }
                    w.add_object(&asset, &mesh, Vector::from(*position));
                }
                LevelItem::Solid { position, .. } | LevelItem::Station { position, .. } => {
                    let mut asset = solid_asset(&item.local_outline());
                    let mut mesh = meshes[index].clone().unwrap();
                    if mesh.triangles.is_empty() {
                        continue;
                    }
                    rotate_asset_mesh(&mut asset, &mut mesh, item.angle());
                    solid_edges.extend(supports::boundary_edges(
                        &mesh,
                        w.pieces.len(),
                        Vector::from(*position),
                    ));
                    let object = w.objects.len();
                    if let LevelItem::Station {
                        station, settings, ..
                    } = item
                    {
                        w.stations.push(stations::Station::new(
                            *station,
                            Vector::from(*position),
                            *settings,
                            object,
                        ));
                    }
                    w.objects.push(components::ObjectInstance {
                        color: None,
                        components: crate::Components::default(),
                        ignited: false,
                        spent: false,
                    });
                    for triangle in mesh.triangles {
                        let id = w.add_triangle(
                            triangle.map(|i| mesh.vertices[i] + Vector::from(*position)),
                            Material::Anchor,
                        );
                        w.pieces[id].object = Some(object);
                        w.physics.bodies[w.pieces[id].body]
                            .set_body_type(RigidBodyType::Fixed, false);
                    }
                }
                LevelItem::Terrain { terrain, min, max } => w.terrain.push(crate::TerrainPatch {
                    kind: *terrain,
                    min: Vector::from(*min),
                    max: Vector::from(*max),
                }),
                LevelItem::Bridge {
                    min,
                    max,
                    vertical,
                    flipped,
                } => {
                    w.add_level_bridge(Vector::from(*min), Vector::from(*max), *vertical, *flipped)?
                }
                LevelItem::Flag { team, position } => w.flags.push(Flag {
                    team: *team,
                    position: Vector::from(*position),
                }),
                LevelItem::Pickup {
                    weapon,
                    position,
                    respawn_time,
                    charges,
                    cooldown,
                } => w.pickups.push(specials::Pickup {
                    weapon: *weapon,
                    position: Vector::from(*position),
                    respawn_time: *respawn_time,
                    charges: *charges,
                    cooldown: *cooldown,
                    remaining: 0.0,
                }),
                LevelItem::Spawn { .. } => {}
            }
        }
        supports::connect(&mut w, &wall_edges, &solid_edges);
        w.rebuild_chunks();
        w.rebuild_terrain_barriers();
        let p = &mut w.pieces[w.launcher];
        let body = &mut w.physics.bodies[p.body];
        body.set_position(Pose::new(position, angle), true);
        p.previous_position = position;
        p.previous_angle = angle;
        w.physics
            .bodies
            .propagate_modified_body_positions_to_colliders(&mut w.physics.colliders);
        for bridge in w.bridges() {
            let vertical = (bridge.controls[1] - bridge.controls[0]).y.abs()
                > (bridge.controls[1] - bridge.controls[0]).x.abs();
            let half = if vertical {
                Vector::new(0.54, 0.39)
            } else {
                Vector::new(0.39, 0.54)
            };
            for center in bridge.controls {
                for x in [-half.x, half.x] {
                    for y in [-half.y, half.y] {
                        if w.terrain_at(center + Vector::new(x, y)).blocks_tanks() {
                            return Err("Bridge booths need dry banks. Extend or move the span so both booths clear the water or cliff.".into());
                        }
                    }
                }
            }
        }
        w.validate_spawn()?;
        w.register_tank(w.launcher, team, position, angle);
        if let Some((enemy_position, enemy_angle)) =
            level.items.iter().find_map(|item| match item {
                LevelItem::Spawn {
                    team: t,
                    position,
                    angle,
                } if *t != team => Some((Vector::from(*position), *angle)),
                _ => None,
            })
        {
            let id = w.add_tank();
            w.register_tank(id, team.other(), enemy_position, enemy_angle);
            w.physics
                .bodies
                .propagate_modified_body_positions_to_colliders(&mut w.physics.colliders);
            if !w.tank_pose_clear(id, Pose::new(enemy_position, enemy_angle)) {
                return Err(format!(
                    "{} enemy spawn overlaps an object, tank, water or cliff. Move its whole footprint into a clear area.",
                    team.other().label()
                ));
            }
            w.enemy = Some(enemy::Enemy::new(id, level.enemy_activity, enemy_position));
        }
        Ok(w)
    }

    fn validate_spawn(&self) -> Result<(), String> {
        let tank = &self.physics.bodies[self.pieces[self.launcher].body];
        for &handle in tank.colliders() {
            let a = &self.physics.colliders[handle];
            for (h, b) in self.physics.colliders.iter() {
                if tank.colliders().contains(&h)
                    || b.collision_groups() == CollisionLayer::Deck.groups()
                {
                    continue;
                }
                if rapier2d::parry::query::intersection_test(
                    a.position(),
                    a.shape(),
                    b.position(),
                    b.shape(),
                )
                .unwrap_or(true)
                {
                    return Err(format!(
                        "{} spawn overlaps an object, a bank, water or cliff. Move the whole tank footprint into a clear area.",
                        self.team.label()
                    ));
                }
            }
        }
        Ok(())
    }

    fn add_level_bridge(
        &mut self,
        min: Vector,
        max: Vector,
        vertical: bool,
        flipped: bool,
    ) -> Result<(), String> {
        let center = (min + max) * 0.5;
        let asset = bridge_asset(max - min, vertical, flipped)?;
        let mesh = bridge_mesh(max - min, vertical, flipped)?;
        let first = self.pieces.len();
        let object = self.objects.len();
        self.add_object(&asset, &mesh, center);
        let anchors: Vec<_> = (first..self.pieces.len())
            .filter(|&id| self.pieces[id].material == Material::Anchor)
            .collect();
        for id in first..self.pieces.len() {
            let p = &mut self.pieces[id];
            if p.object != Some(object) {
                continue;
            }
            if let Shape::Triangle { vertices } = p.shape {
                p.bridge_deck = vertices.iter().any(|v| {
                    let point = *v + p.previous_position;
                    let point = if flipped { center * 2.0 - point } else { point };
                    if vertical {
                        point.x > min.x + 1e-4
                    } else {
                        point.y < max.y - 1e-4
                    }
                });
                self.physics.colliders[p.collider]
                    .set_collision_groups(p.collision_layer().groups());
            }
        }
        let controls = if vertical {
            [
                Vector::new(min.x - 0.55, max.y - 0.4),
                Vector::new(min.x - 0.55, min.y + 0.4),
            ]
        } else {
            [
                Vector::new(max.x - 0.4, max.y + 0.55),
                Vector::new(min.x + 0.4, max.y + 0.55),
            ]
        };
        let controls = controls.map(|p| if flipped { center * 2.0 - p } else { p });
        self.bridges.push(terrain::Bridge::new(
            object,
            [anchors[0], anchors[1]],
            crate::TerrainPatch {
                kind: crate::TerrainKind::Bridge,
                min,
                max,
            },
            controls,
            terrain::repair::Blueprint::capture(self, object),
        ));
        Ok(())
    }

    pub(super) fn capture_flags(&mut self, events: &mut Vec<Event>) {
        if self.flags.is_empty() || self.winner.is_some() {
            return;
        }
        for tank in self.tanks.iter().filter(|t| self.tank_is_alive(t.id)) {
            let body = &self.physics.bodies[self.pieces[tank.id].body];
            for flag in self.flags.iter().filter(|f| f.team != tank.team) {
                let pose = Pose::translation(flag.position.x, flag.position.y);
                let shape = rapier2d::parry::shape::Ball::new(FLAG_RADIUS);
                if body.colliders().iter().any(|&h| {
                    let c = &self.physics.colliders[h];
                    rapier2d::parry::query::intersection_test(
                        c.position(),
                        c.shape(),
                        &pose,
                        &shape,
                    )
                    .unwrap_or(false)
                }) {
                    self.winner = Some(tank.team);
                    events.push(Event::FlagCaptured {
                        position: flag.position,
                        team: tank.team,
                    });
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod demo_tests;
