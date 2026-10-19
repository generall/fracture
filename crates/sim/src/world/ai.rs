//! The information boundary between authoritative physics and bot decisions.
use super::*;
use crate::ai::{Map, Observation, Obstacle, SpecialState, Supply, Target};

/// Authoritative geometry shared only inside one observation batch. It is not
/// serializable and never crosses the worker's human-information boundary.
pub struct AiGeometry {
    tick: u64,
    figures: Vec<AiFigure>,
}
struct AiFigure {
    body: BodyView,
    outline: Vec<Vector>,
    min: Vector,
    max: Vector,
}

pub(crate) fn outline(body: &BodyView) -> Vec<Vector> {
    let local = match body.shape {
        Shape::Box { half } => vec![
            Vector::new(-half.x, -half.y),
            Vector::new(half.x, -half.y),
            half,
            Vector::new(-half.x, half.y),
        ],
        Shape::Triangle { vertices } => vertices.to_vec(),
        Shape::Ball { radius } => (0..12)
            .map(|i| Vector::from_angle(i as f32 * std::f32::consts::TAU / 12.0) * radius)
            .collect(),
    };
    local
        .into_iter()
        .map(|p| body.position + Vector::from_angle(body.angle).rotate(p))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ai::{Config, Controller},
        level::{Level, LevelItem, Team},
        multiplayer::{MultiplayerWorld, PlayerInput},
    };
    fn observed(world: &World) -> serde_json::Value {
        serde_json::to_value(world.ai_observation()).unwrap()
    }
    #[test]
    fn unseen_enemy_motion_chassis_health_reload_and_ammo_do_not_change_observation_or_action() {
        let mut w = World::from_level(&Level::default(), Team::Blue).unwrap();
        let before = observed(&w);
        let mut a = Controller::new(Config::default(), w.ai_map());
        let action = a.decide(&w.ai_observation());
        let enemy = w.tanks.iter().find(|t| t.id != w.launcher).unwrap().id;
        let body = w.pieces[enemy].body;
        w.physics.bodies[body].set_translation(Vector::new(20.0, 8.0), true);
        let t = w.tanks.iter_mut().find(|t| t.id == enemy).unwrap();
        t.health = 12.0;
        t.reload = 50.0;
        t.aim.angle = 1.8;
        w.apply_vehicle(enemy, crate::VehicleKind::Mortar);
        w.physics
            .bodies
            .propagate_modified_body_positions_to_colliders(&mut w.physics.colliders);
        assert_eq!(before, observed(&w));
        let mut b = Controller::new(Config::default(), w.ai_map());
        assert_eq!(
            serde_json::to_value(action).unwrap(),
            serde_json::to_value(b.decide(&w.ai_observation())).unwrap()
        );
    }
    #[test]
    fn enemy_chassis_is_available_only_for_a_normally_visible_vehicle() {
        let mut w = World::from_level(&Level::default(), Team::Blue).unwrap();
        let enemy = w.tanks.iter().find(|t| t.id != w.launcher).unwrap().id;
        for kind in crate::VehicleKind::ALL {
            w.apply_vehicle(enemy, kind);
            let h = w.pieces[enemy].body;
            w.physics.bodies[h].set_translation(Vector::new(-10.0, 0.0), true);
            w.physics
                .bodies
                .propagate_modified_body_positions_to_colliders(&mut w.physics.colliders);
            let o = w.ai_observation();
            assert!(o.targets.iter().any(|t| t.vehicle == Some(kind)));
        }
    }
    #[test]
    fn cover_cone_and_grass_hide_targets_for_both_teams() {
        let mut level = Level::default();
        level.items.push(LevelItem::Solid {
            outline: crate::level::rectangle(Vector::new(1.0, 8.0)),
            position: [0.0, 0.0],
            angle: 0.0,
            cutouts: vec![],
        });
        let mut game = MultiplayerWorld::new(&level, &[Team::Blue, Team::Red]).unwrap();
        for side in 0..2 {
            let world = game.for_player(side).unwrap();
            let o = world.ai_observation();
            assert!(o.targets.is_empty());
            for r in &o.regions {
                assert!(r.tank_visibility(world.player_tank_id()).is_none());
            }
        }
        let mut w = World::from_level(&Level::default(), Team::Blue).unwrap();
        let enemy = w.tanks.iter().find(|t| t.id != w.launcher).unwrap().id;
        let h = w.pieces[enemy].body;
        for (position, expected) in [([-10.0, 0.0], true), ([-22.0, 7.0], false)] {
            w.physics.bodies[h].set_translation(Vector::from(position), true);
            w.physics
                .bodies
                .propagate_modified_body_positions_to_colliders(&mut w.physics.colliders);
            assert_eq!(!w.ai_observation().targets.is_empty(), expected);
        }
        w.physics.bodies[h].set_translation(Vector::new(-10.0, 0.0), true);
        w.physics
            .bodies
            .propagate_modified_body_positions_to_colliders(&mut w.physics.colliders);
        w.terrain.push(crate::TerrainPatch {
            kind: crate::TerrainKind::TallGrass,
            min: Vector::new(-12.0, -2.0),
            max: Vector::new(-8.0, 2.0),
        });
        assert!(w.ai_observation().targets.is_empty());
    }
    #[test]
    fn hidden_destroyed_cover_does_not_refresh_bot_memory() {
        let mut level = Level::default();
        level.items.push(LevelItem::Object {
            asset: Box::new(crate::level::wall_asset(Vector::new(1.0, 2.0))),
            position: [18.0, 8.0],
            angle: 0.0,
            cutouts: vec![],
        });
        let mut w = World::from_level(&level, Team::Blue).unwrap();
        let before = observed(&w);
        let layout_before = w.ai_map().layout;
        let ids: Vec<_> = w
            .pieces
            .iter()
            .enumerate()
            .filter(|(_, p)| p.destructible())
            .map(|(i, _)| i)
            .collect();
        for i in ids {
            w.pieces[i].active = false;
            w.physics.bodies[w.pieces[i].body].set_enabled(false);
        }
        assert_eq!(before, observed(&w));
        assert_eq!(layout_before, w.ai_map().layout);
    }
    #[test]
    fn bot_collects_and_activates_each_special_through_human_controls() {
        for weapon in [
            crate::SpecialWeapon::Laser,
            crate::SpecialWeapon::Rocket,
            crate::SpecialWeapon::Plasma,
            crate::SpecialWeapon::Mine,
        ] {
            let mut level = Level::default();
            level.items.push(LevelItem::Pickup {
                weapon,
                position: [-18.0, 0.0],
                respawn_time: 60.0,
                charges: 1,
                cooldown: 0.0,
            });
            let mut w = World::from_level(&level, Team::Blue).unwrap();
            w.enemy = None;
            let enemy = w.tanks.iter().find(|t| t.id != w.launcher).unwrap().id;
            let handle = w.pieces[enemy].body;
            w.physics.bodies[handle].set_translation(Vector::new(-6.0, 0.0), true);
            w.physics
                .bodies
                .propagate_modified_body_positions_to_colliders(&mut w.physics.colliders);
            w.update_specials(&mut vec![]);
            assert_eq!(w.carried_special(), Some(weapon));
            let mut c = Controller::new(Config::default(), w.ai_map());
            let mut input = PlayerInput::default();
            for tick in 0..72 {
                if tick % 12 == 0 {
                    input = c.decide(&w.ai_observation());
                }
                w.step(&input.commands());
            }
            assert_eq!(w.special_charges(), 0, "{weapon:?}");
            if weapon == crate::SpecialWeapon::Mine {
                assert_eq!(w.mines().count(), 1);
            }
        }
    }
    #[test]
    fn hidden_pickup_state_is_absent_and_starting_layout_matches_teams() {
        let mut level = Level::default();
        level.items.push(LevelItem::Pickup {
            weapon: crate::SpecialWeapon::Laser,
            position: [18.0, 8.0],
            respawn_time: 60.0,
            charges: 1,
            cooldown: 0.0,
        });
        let mut w = World::from_level(&level, Team::Blue).unwrap();
        let before = observed(&w);
        assert!(w.ai_observation().supplies.is_empty());
        w.pickups[0].weapon = crate::SpecialWeapon::Rocket;
        w.pickups[0].remaining = 50.0;
        assert_eq!(before, observed(&w));
        let other = World::from_level(&level, Team::Red).unwrap();
        assert_eq!(w.ai_map().layout, other.ai_map().layout);
    }
    #[test]
    fn allied_figures_are_observed_even_when_their_centers_are_occluded() {
        let mut game = MultiplayerWorld::new(&Level::default(), &crate::ai::ARENA_TEAMS).unwrap();
        let world = game.for_player(0).unwrap();
        let o = world.ai_observation();
        assert_eq!(o.allied_vehicles.len(), 1);
        let ally = o.allied_vehicles[0].position;
        assert!(o.allies.contains(&ally));
        assert!(!o.visible(ally), "this regression needs a clipped center");
    }
    #[test]
    fn shared_geometry_preserves_each_players_complete_restricted_observation() {
        let mut game = MultiplayerWorld::new(
            &Level::from_json(include_str!("../../../../levels/dota-level.json")).unwrap(),
            &crate::ai::ARENA_TEAMS,
        )
        .unwrap();
        game.require_vehicle_choices();
        let inputs: Vec<_> = [
            crate::VehicleKind::Scout,
            crate::VehicleKind::Tank,
            crate::VehicleKind::Mortar,
            crate::VehicleKind::Scout,
        ]
        .into_iter()
        .map(|kind| PlayerInput {
            deploy: Some(kind),
            ..Default::default()
        })
        .collect();
        for _ in 0..3 {
            game.step(&inputs);
        }
        let geometry = game.world().ai_geometry();
        for player in 0..4 {
            let w = game.for_player(player).unwrap();
            assert_eq!(
                serde_json::to_value(w.ai_observation()).unwrap(),
                serde_json::to_value(w.ai_observation_from(&geometry)).unwrap()
            );
        }
    }
    #[test]
    fn public_defenses_do_not_refresh_from_hidden_capture_or_destruction() {
        let level = Level::from_json(include_str!("../../../../levels/dota-level.json")).unwrap();
        let mut world = World::from_level(&level, Team::Blue).unwrap();
        let before = serde_json::to_value(world.ai_map()).unwrap();
        let turret = world
            .tanks
            .iter_mut()
            .find(|t| t.team == Team::Red && t.mount.is_some())
            .unwrap();
        turret.health = 0.0;
        turret.team = Team::Blue;
        turret.mount.as_mut().unwrap().view.facing += 1.0;
        assert_eq!(before, serde_json::to_value(world.ai_map()).unwrap());
    }
    #[test]
    fn public_pickup_sites_never_refresh_hidden_live_contents() {
        let level = Level::from_json(include_str!("../../../../levels/dota-level.json")).unwrap();
        let mut world = World::from_level(&level, Team::Blue).unwrap();
        let before = serde_json::to_value(world.ai_map()).unwrap();
        assert!(!world.ai_map().supply_sites.is_empty());
        world.pickups[0].weapon = crate::SpecialWeapon::Rocket;
        world.pickups[0].remaining = 50.0;
        world.pickups[0].position = Vector::ZERO;
        assert_eq!(before, serde_json::to_value(world.ai_map()).unwrap());
    }
    #[test]
    fn ruin_information_respects_the_human_foliage_visibility_override() {
        let level = Level {
            items: vec![
                crate::level::LevelItem::Spawn {
                    team: Team::Blue,
                    position: [0.0, 0.0],
                    angle: 0.0,
                },
                crate::level::LevelItem::Spawn {
                    team: Team::Red,
                    position: [20.0, 0.0],
                    angle: 0.0,
                },
                crate::level::LevelItem::Turret {
                    team: Team::Red,
                    position: [2.5, 0.0],
                    angle: 0.0,
                    arc: 180.0,
                    health: 100.0,
                    capturable: false,
                },
                crate::level::LevelItem::Terrain {
                    terrain: crate::TerrainKind::TallGrass,
                    min: [-2.0, -3.0],
                    max: [6.0, 3.0],
                },
            ],
            ..Default::default()
        };
        let mut world = World::from_level(&level, Team::Blue).unwrap();
        let gun = world.tanks.iter_mut().find(|t| t.mount.is_some()).unwrap();
        gun.health = 0.0;
        let id = gun.id;
        assert_eq!(world.player_vision(0.0).tank_visibility(id), Some(false));
        assert!(world.ai_observation().visible_ruins.is_empty());
    }
    #[test]
    fn bounded_observations_match_full_scan_on_both_teams_and_vehicle_sight_profiles() {
        for team in [Team::Blue, Team::Red] {
            let mut world = World::from_level(
                &Level::from_json(include_str!("../../../../levels/dota-level.json")).unwrap(),
                team,
            )
            .unwrap();
            for vehicle in crate::VehicleKind::ALL {
                let id = world.launcher;
                world.apply_vehicle(id, vehicle);
                for angle in [0.0, 1.4, 3.1] {
                    world.player_aim.angle = angle;
                    let w = &world;
                    let o = w.ai_observation();
                    let vision = w.player_vision(angle);
                    let map = w.ai_map();
                    let terrain: Vec<_> = (0..map.width() * map.height())
                        .filter_map(|i| {
                            let p = map.point(i);
                            o.visible(p)
                                .then(|| (i, if w.terrain_at(p).blocks_tanks() { 3 } else { 1 }))
                        })
                        .collect();
                    assert_eq!(o.terrain, terrain);
                    let obstacles: Vec<_> = w
                        .bodies()
                        .filter(|b| {
                            b.opacity > 0.0 && b.material != Material::Launcher && !b.bridge_deck
                        })
                        .filter(|b| vision.intersects_polygon(&outline(b)))
                        .map(|b| outline(&b))
                        .collect();
                    assert_eq!(
                        o.obstacles
                            .iter()
                            .map(|b| b.outline.clone())
                            .collect::<Vec<_>>(),
                        obstacles
                    );
                }
            }
        }
    }
}
impl World {
    fn ai_grid(&self) -> Map {
        Map {
            size: self.arena_size,
            flags: self.flags().collect(),
            team: self.team(),
            layout: vec![],
            seed: 0,
            team_slot: 0,
            emplacements: vec![],
            supply_sites: vec![],
        }
    }
    pub fn ai_map(&self) -> Map {
        let mut map = self.ai_grid();
        map.layout = self.public_layout.clone();
        map.emplacements = self.public_emplacements.clone();
        map.supply_sites = self.public_supply_sites.clone();
        map
    }
    // Capture the authored starting layout once, as shown in the human map
    // preview. It must never be rebuilt from unseen live destruction.
    pub(super) fn build_public_layout(&self) -> Vec<u8> {
        let map = self.ai_grid();
        let mut cells: Vec<u8> = (0..map.width() * map.height())
            .map(|i| {
                if self.terrain_at(map.point(i)).blocks_tanks() {
                    3
                } else {
                    1
                }
            })
            .collect();
        for body in self
            .bodies()
            .filter(|b| b.material != Material::Launcher && !b.bridge_deck && b.opacity > 0.0)
        {
            let poly = outline(&body);
            let lo = poly
                .iter()
                .copied()
                .fold(Vector::splat(f32::INFINITY), Vector::min);
            let hi = poly
                .iter()
                .copied()
                .fold(Vector::splat(f32::NEG_INFINITY), Vector::max);
            let a = map.index(lo - Vector::splat(map.cell()));
            let b = map.index(hi + Vector::splat(map.cell()));
            let class = if matches!(
                body.material,
                Material::Rock | Material::Masonry | Material::Panel
            ) {
                2
            } else {
                4
            };
            for y in a / map.width()..=b / map.width() {
                for x in a % map.width()..=b % map.width() {
                    let i = y * map.width() + x;
                    if crate::ai::polygon_distance(map.point(i), &poly) < map.cell() * 0.6 {
                        cells[i] = cells[i].max(class);
                    }
                }
            }
        }
        cells
    }
    /// Exactly the current player's rendered detection rules, including cone,
    /// cover, foliage, radar and allied sight. Never pass a replica Frame to bots:
    /// trusted presentation frames also contain hidden authoritative metadata.
    pub fn ai_observation(&self) -> Observation {
        self.ai_observation_from(&self.ai_geometry())
    }
    /// Build once for players observed at the same physics state. Selecting a
    /// player's controls does not change this geometry; stepping physics does.
    pub fn ai_geometry(&self) -> AiGeometry {
        AiGeometry {
            tick: self.tick_count(),
            figures: self
                .bodies()
                .filter(|b| b.opacity > 0.0)
                .map(|body| {
                    let outline = outline(&body);
                    let min = outline
                        .iter()
                        .copied()
                        .fold(Vector::splat(f32::INFINITY), Vector::min);
                    let max = outline
                        .iter()
                        .copied()
                        .fold(Vector::splat(f32::NEG_INFINITY), Vector::max);
                    AiFigure {
                        body,
                        outline,
                        min,
                        max,
                    }
                })
                .collect(),
        }
    }
    pub fn ai_observation_from(&self, geometry: &AiGeometry) -> Observation {
        debug_assert_eq!(
            geometry.tick,
            self.tick_count(),
            "AI geometry reused after a physics step"
        );
        let vision = self.player_vision(self.player_aim().angle);
        let own = geometry
            .figures
            .iter()
            .find(|f| f.body.id == self.player_tank_id())
            .map(|f| &f.body);
        let regions: Vec<_> = vision
            .regions()
            .map(|v| Vision::from_boundary(v.origin, v.boundary.clone()))
            .collect();
        let visible = |point| regions.iter().any(|v| v.contains(point));
        let allied_visible = |tank: &TankView| {
            tank.team == self.team()
                && tank.id != self.launcher
                && tank.health > 0.0
                && geometry
                    .figures
                    .iter()
                    .find(|f| f.body.id == tank.id)
                    .is_some_and(|f| {
                        // The figure's front face clips the sight rays before its
                        // center. Use the human whole-figure visibility rule.
                        vision
                            .tank_visibility(tank.id)
                            .unwrap_or_else(|| vision.intersects_polygon(&f.outline))
                    })
        };
        let bounds: Vec<_> = regions
            .iter()
            .filter(|v| !v.boundary.is_empty())
            .map(|v| {
                (
                    v.boundary
                        .iter()
                        .copied()
                        .fold(Vector::splat(f32::INFINITY), Vector::min),
                    v.boundary
                        .iter()
                        .copied()
                        .fold(Vector::splat(f32::NEG_INFINITY), Vector::max),
                )
            })
            .collect();
        let mut obstacles = vec![];
        let mut targets = vec![];
        let mut visible_ruins = vec![];
        for figure in &geometry.figures {
            let b = &figure.body;
            if b.material == Material::Launcher {
                let tank = self.tanks().find(|t| t.id == b.id);
                if tank.is_some_and(|t| t.mount.is_some() && t.health <= 0.0)
                    && vision
                        .tank_visibility(b.id)
                        .unwrap_or_else(|| vision.intersects_polygon(&figure.outline))
                {
                    visible_ruins.push(b.position);
                }
                if let Some(t) = tank
                    && t.team != self.team()
                    && t.health > 0.0
                    && (vision.firing_tank_visible(b.id)
                        || vision
                            .tank_visibility(b.id)
                            .unwrap_or_else(|| vision.intersects_polygon(&figure.outline)))
                {
                    targets.push(Target {
                        position: b.position,
                        mounted: t.mount.is_some(),
                        vehicle: (t.mount.is_none()
                            && vision
                                .tank_visibility(b.id)
                                .unwrap_or_else(|| vision.intersects_polygon(&figure.outline)))
                        .then_some(t.vehicle),
                    });
                }
            } else if !b.bridge_deck {
                let poly = &figure.outline;
                let lo = figure.min;
                let hi = figure.max;
                if bounds
                    .iter()
                    .any(|(min, max)| lo.cmple(*max).all() && hi.cmpge(*min).all())
                    && vision.intersects_polygon(poly)
                {
                    obstacles.push(Obstacle {
                        outline: poly.clone(),
                        breakable: matches!(
                            b.material,
                            Material::Rock | Material::Masonry | Material::Panel
                        ),
                    });
                }
            }
        }
        let map = self.ai_grid();
        // Most of a large map is outside every sight region. Bound grid work
        // by the visible polygons, preserving the original sorted cell order.
        let mut candidates = std::collections::BTreeSet::new();
        for (lo, hi) in bounds {
            let a = map.index(lo);
            let b = map.index(hi);
            for y in a / map.width()..=b / map.width() {
                for x in a % map.width()..=b % map.width() {
                    candidates.insert(y * map.width() + x);
                }
            }
        }
        let mut terrain = vec![];
        for i in candidates {
            let p = map.point(i);
            if visible(p) {
                terrain.push((
                    i,
                    if self.terrain_at(p).blocks_tanks() {
                        3
                    } else {
                        1
                    },
                ));
            }
        }
        Observation {
            tick: self.tick_count(),
            position: self.launcher_position(),
            hull_angle: own.map_or(0.0, |b| b.angle),
            turret_angle: self.player_aim().angle,
            alive: self.tank_alive(),
            health: self
                .tanks()
                .find(|t| t.id == self.launcher)
                .map_or(0.0, |t| t.health),
            reload: self.reload_remaining(),
            aim_spread: self.player_aim().spread,
            vehicle: self.player_vehicle(),
            special: self.carried_special().map(|weapon| SpecialState {
                weapon,
                charges: self.special_charges(),
                cooldown: self.special_cooldown(),
                active: self.special_active(),
            }),
            supplies: self
                .pickups()
                .filter(|p| p.remaining <= 0.0 && visible(p.position))
                .map(|p| Supply {
                    position: p.position,
                    weapon: p.weapon,
                })
                .chain(
                    self.airdrop()
                        .filter(|p| visible(p.position))
                        .map(|p| Supply {
                            position: p.position,
                            weapon: p.weapon,
                        }),
                )
                .collect(),
            allies: self
                .tanks()
                .filter(allied_visible)
                .map(|t| self.piece_pose(t.id).translation)
                .collect(),
            allied_vehicles: self
                .tanks()
                .filter(|t| t.mount.is_none() && allied_visible(t))
                .map(|t| {
                    let position = self.piece_pose(t.id).translation;
                    crate::ai::AlliedVehicle {
                        position,
                        vehicle: t.vehicle,
                    }
                })
                .collect(),
            regions,
            obstacles,
            targets,
            visible_ruins,
            terrain,
        }
    }
}
