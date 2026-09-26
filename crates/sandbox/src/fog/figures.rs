//! Whole physical figures and their last observed footprints.
use crate::view::scene::{Bridge, Scene};
use sim::{BodyView, Material, Shape, Vec2, Vision, World};
use std::collections::{BTreeMap, BTreeSet};

const VISIBILITY_COOLDOWN: f64 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Id {
    Object(usize, u32),
    Piece(usize),
    Flag(u8),
    Bridge(u32, u32),
    Pickup(u32, u32),
    Decoration(usize),
}
#[derive(Clone, Default)]
pub(super) struct Figure {
    pub polygons: Vec<Vec<Vec2>>,
    pub snapshot: Scene,
    // Stable material-cell IDs, including ancestors of refined cells.
    pub lineage: BTreeSet<usize>,
}
impl Figure {
    fn visible(&self, vision: &Vision) -> bool {
        self.polygons.iter().any(|p| vision.intersects_polygon(p))
    }
}
#[derive(Default)]
pub(super) struct Figures {
    remembered: BTreeMap<Id, Figure>,
    last_seen: BTreeMap<Id, f64>,
    initialized: bool,
}
pub(super) struct Frame {
    pub visible: BTreeSet<Id>,
    pub scene: Scene,
}
impl Figures {
    pub(super) fn observe_tank_wreck(&mut self, world: &World, tank: usize, now: f64) {
        self.remembered
            .retain(|_, figure| !figure.snapshot.tanks.iter().any(|t| t.id == tank));
        self.last_seen
            .retain(|id, _| self.remembered.contains_key(id));
        for (id, figure) in current(world) {
            if figure.lineage.contains(&tank) {
                self.remembered.insert(id, figure);
                self.last_seen.insert(id, now);
            }
        }
    }
    pub fn update(&mut self, world: &World, vision: &Vision, now: f64) -> Frame {
        let current = current(world);
        if !self.initialized {
            // The enemy objective's starting location is known, without
            // revealing the surrounding area or tracking unseen changes.
            let enemy_flag = Id::Flag(world.team().other() as u8);
            if let Some(flag) = current.get(&enemy_flag) {
                self.remembered.insert(enemy_flag, flag.clone());
            }
            self.initialized = true;
        }
        self.observe(current, vision, now)
    }
    fn observe(&mut self, current: BTreeMap<Id, Figure>, vision: &Vision, now: f64) -> Frame {
        let mut seen_cells = BTreeMap::new();
        for (id, figure) in &self.remembered {
            if let Some(&last) = self.last_seen.get(id)
                && now - last < VISIBILITY_COOLDOWN
            {
                for body in &figure.snapshot.bodies {
                    seen_cells.insert(body.id, (*id, last));
                }
            }
        }
        let mut replaced = BTreeSet::new();
        for (id, figure) in &current {
            for cell in &figure.lineage {
                if let Some(&(parent, last)) = seen_cells.get(cell) {
                    self.last_seen
                        .entry(*id)
                        .and_modify(|seen| *seen = seen.max(last))
                        .or_insert(last);
                    if parent != *id {
                        replaced.insert(parent);
                    }
                }
            }
        }
        // A visible split replaces the complete parent snapshot. Hidden splits
        // after the cooldown keep their old memory until observed again.
        for id in replaced {
            self.remembered.remove(&id);
        }
        let mut frame = Frame {
            visible: BTreeSet::new(),
            scene: Scene::default(),
        };
        for (id, figure) in current {
            let tank_visibility = figure
                .snapshot
                .tanks
                .first()
                .and_then(|tank| vision.tank_visibility(tank.id));
            if tank_visibility == Some(false) {
                // Entering cover and the five-second reveal deadline must not
                // inherit the ordinary scenery visibility cooldown.
                self.last_seen.remove(&id);
                continue;
            }
            if tank_visibility.unwrap_or_else(|| figure.visible(vision)) {
                self.last_seen.insert(id, now);
            }
            // Keep live geometry during brief sight interruptions. Only actual
            // sightings reset the timer, so the grace period cannot renew itself.
            if self
                .last_seen
                .get(&id)
                .is_some_and(|last| now - last < VISIBILITY_COOLDOWN)
            {
                frame.visible.insert(id);
                self.remembered.insert(id, figure);
            }
        }
        self.remembered.retain(|id, figure| {
            if !frame.visible.contains(id) && figure.visible(vision) {
                // We can see the old location: discard its stale snapshot.
                false
            } else {
                true
            }
        });
        self.last_seen
            .retain(|id, _| self.remembered.contains_key(id));
        // Memories go underneath current observations.
        for remembered in [true, false] {
            for (id, figure) in &self.remembered {
                if !frame.visible.contains(id) == remembered {
                    frame.scene.append(&figure.snapshot, remembered);
                }
            }
        }
        frame
    }
}
fn rectangle(min: Vec2, max: Vec2) -> Vec<Vec2> {
    vec![min, Vec2::new(max.x, min.y), max, Vec2::new(min.x, max.y)]
}
fn polygon(body: BodyView) -> Vec<Vec2> {
    let local = if body.material == Material::Launcher {
        rectangle(Vec2::new(-1.08, -0.82), Vec2::new(1.08, 0.82))
    } else {
        match body.shape {
            Shape::Triangle { vertices } => vertices.to_vec(),
            Shape::Box { half } => rectangle(-half, half),
            Shape::Ball { radius } => (0..20)
                .map(|i| Vec2::from_angle(i as f32 * std::f32::consts::TAU / 20.0) * radius)
                .collect(),
        }
    };
    let rotation = Vec2::from_angle(body.angle);
    local
        .into_iter()
        .map(|p| body.position + rotation.rotate(p))
        .collect()
}
fn current(world: &World) -> BTreeMap<Id, Figure> {
    let mut result: BTreeMap<Id, Figure> = BTreeMap::new();
    for body in world
        .bodies()
        .filter(|b| b.id != world.player_tank_id() && b.opacity > 0.0)
    {
        let id = body.object.map_or(Id::Piece(body.id), |object| {
            Id::Object(object, if body.fixed { u32::MAX } else { body.chunk })
        });
        let figure = result.entry(id).or_default();
        let mut ancestor = Some(body.id);
        while let Some(cell) = ancestor {
            figure.lineage.insert(cell);
            ancestor = world.fracture_parent(cell);
        }
        let polygon = polygon(body);
        figure.polygons.push(polygon);
        figure.snapshot.bodies.push(body);
        if body.material == Material::Launcher {
            figure
                .snapshot
                .tanks
                .extend(world.tanks().filter(|t| t.id == body.id));
        }
    }
    for prop in world.decorations() {
        result.insert(
            Id::Decoration(prop.id),
            Figure {
                polygons: vec![prop.outline()],
                snapshot: Scene {
                    decorations: vec![(prop, 1.)],
                    ..Scene::default()
                },
                ..Figure::default()
            },
        );
    }
    for station in world.stations() {
        if let Some(figure) = result.get_mut(&Id::Object(station.object, u32::MAX)) {
            figure.snapshot.stations.push((station, 1.0));
        }
    }
    for pickup in world.pickups() {
        result.insert(
            Id::Pickup(pickup.position.x.to_bits(), pickup.position.y.to_bits()),
            Figure {
                polygons: vec![rectangle(
                    pickup.position - Vec2::splat(0.65),
                    pickup.position + Vec2::splat(0.65),
                )],
                snapshot: Scene {
                    pickups: vec![(pickup, 1.0)],
                    ..Scene::default()
                },
                ..Figure::default()
            },
        );
    }
    for (team, position) in world.flags() {
        result.insert(
            Id::Flag(team as u8),
            Figure {
                polygons: vec![rectangle(
                    position - Vec2::splat(0.35),
                    position + Vec2::splat(0.35),
                )],
                snapshot: Scene {
                    flags: vec![(team, position, 1.0)],
                    ..Scene::default()
                },
                ..Figure::default()
            },
        );
    }
    for bridge in world.bridges() {
        let mut polygons = vec![rectangle(bridge.deck.min, bridge.deck.max)];
        for p in bridge.controls {
            polygons.push(rectangle(p - Vec2::splat(0.6), p + Vec2::splat(0.6)));
        }
        result.insert(
            Id::Bridge(bridge.deck.min.x.to_bits(), bridge.deck.min.y.to_bits()),
            Figure {
                polygons,
                snapshot: Scene {
                    bridges: vec![Bridge::from(bridge)],
                    ..Scene::default()
                },
                ..Figure::default()
            },
        );
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grass_conceals_whole_tanks_at_the_edge_without_visibility_cooldown() {
        use sim::level::{Level, LevelItem, Team};
        let mut level = Level {
            items: vec![
                LevelItem::Spawn {
                    team: Team::Blue,
                    position: [0.0, 0.0],
                    angle: 0.0,
                },
                LevelItem::Spawn {
                    team: Team::Red,
                    position: [4.0, 0.0],
                    angle: 0.0,
                },
            ],
            ..Level::default()
        };
        let open = World::from_level(&level, Team::Blue).unwrap();
        let enemy = open.tanks().find(|t| t.team == Team::Red).unwrap().id;
        let mut memory = Figures::default();
        let seen = memory.update(&open, &open.player_vision(0.0), 0.0);
        assert!(seen.scene.tanks.iter().any(|t| t.id == enemy));
        level.items.push(LevelItem::Terrain {
            terrain: sim::TerrainKind::TallGrass,
            min: [4.0, -3.0],
            max: [10.0, 3.0],
        });
        let covered = World::from_level(&level, Team::Blue).unwrap();
        let vision = covered.player_vision(0.0);
        assert!(
            vision.contains(Vec2::new(3.0, 0.0)),
            "tank nose sticks out into geometric sight"
        );
        let hidden = memory.update(&covered, &vision, 0.1);
        assert!(!hidden.visible.contains(&Id::Piece(enemy)));
        assert!(
            !hidden
                .scene
                .bodies
                .iter()
                .any(|b| b.id == enemy && b.opacity > 0.32)
        );
        let unseen = Figures::default().update(&covered, &vision, 0.0);
        assert!(!unseen.scene.tanks.iter().any(|t| t.id == enemy));
    }
    #[test]
    fn stations_share_the_building_figure_and_remember_last_seen_ownership() {
        use sim::level::{Level, LevelItem, StationKind, StationSettings, Team};
        let mut level = Level::default();
        level.items.push(LevelItem::Station {
            station: StationKind::Repair,
            position: [0.0, 0.0],
            settings: StationSettings::default(),
        });
        let world = World::from_level(&level, Team::Blue).unwrap();
        let station = world.stations().next().unwrap();
        let id = Id::Object(station.object, u32::MAX);
        let mut memory = Figures::default();
        assert!(
            memory
                .update(&world, &Vision::default(), 0.0)
                .scene
                .stations
                .is_empty()
        );
        let seen = memory.update(&world, &vision(), 0.1);
        assert_eq!(seen.scene.stations.len(), 1);
        assert_eq!(seen.scene.stations[0].0.owner, None);
        assert_eq!(memory.remembered[&id].snapshot.bodies.len(), 2);
        let mut changed = current(&world);
        changed.get_mut(&id).unwrap().snapshot.stations[0].0.owner = Some(Team::Red);
        let hidden = memory.observe(changed, &Vision::default(), 1.2);
        assert_eq!(hidden.scene.stations[0].0.owner, None);
        assert_eq!(hidden.scene.stations[0].1, 0.32);
    }
    fn vision() -> Vision {
        Vision::from_boundary(
            Vec2::ZERO,
            vec![
                Vec2::new(5., 0.),
                Vec2::new(0., 5.),
                Vec2::new(-5., 0.),
                Vec2::new(0., -5.),
            ],
        )
    }

    fn figure(x: f32) -> Figure {
        let polygon = rectangle(Vec2::new(x, -1.), Vec2::new(x + 3., 1.));
        Figure {
            polygons: vec![polygon],
            snapshot: Scene {
                flags: vec![(sim::level::Team::Red, Vec2::new(x, 0.0), 1.0)],
                ..Scene::default()
            },
            ..Figure::default()
        }
    }
    #[test]
    fn partial_sighting_reveals_whole_figure_without_revealing_the_next_one() {
        let id = Id::Piece(7);
        let hidden = Id::Piece(8);
        let mut memory = Figures::default();
        let frame = memory.observe(
            BTreeMap::from([(id, figure(4.)), (hidden, figure(8.))]),
            &vision(),
            0.0,
        );
        assert_eq!(frame.scene.flags[0].1.x, 4.);
        assert_eq!(frame.visible, BTreeSet::from([id]));
        assert!(memory.remembered[&id].polygons[0].iter().any(|p| p.x == 7.));
    }
    #[test]
    fn brief_sight_interruptions_keep_live_figures_and_reacquisition_resets_cooldown() {
        let id = Id::Piece(7);
        let hidden = Id::Piece(8);
        let mut memory = Figures::default();
        memory.observe(BTreeMap::from([(id, figure(2.))]), &vision(), 0.0);
        let frame = memory.observe(
            BTreeMap::from([(id, figure(12.)), (hidden, figure(15.))]),
            &vision(),
            0.5,
        );
        assert_eq!(frame.visible, BTreeSet::from([id]));
        assert_eq!(frame.scene.flags[0].1.x, 12., "keep moving during grace");
        assert_eq!(frame.scene.flags[0].2, 1.0);

        memory.observe(BTreeMap::from([(id, figure(2.))]), &vision(), 0.75);
        let frame = memory.observe(BTreeMap::from([(id, figure(13.))]), &Vision::default(), 1.5);
        assert!(frame.visible.contains(&id), "new sighting resets the timer");
        let frame = memory.observe(
            BTreeMap::from([(id, figure(14.))]),
            &Vision::default(),
            1.75,
        );
        assert!(frame.visible.is_empty(), "expire after one second unseen");
        assert_eq!(frame.scene.flags[0].1.x, 13., "freeze last displayed pose");
        assert_eq!(frame.scene.flags[0].2, 0.32);
    }

    #[test]
    fn split_pieces_inherit_remaining_visibility_without_revealing_old_siblings() {
        let world =
            World::from_level(&sim::level::Level::default(), sim::level::Team::Blue).unwrap();
        let template = world.bodies().next().unwrap();
        let fragment = |cell: usize, ancestors: &[usize], x: f32| {
            let mut f = figure(x);
            let mut body = template;
            body.id = cell;
            f.snapshot.bodies.push(body);
            f.lineage = ancestors.iter().copied().chain([cell]).collect();
            f
        };
        let parent = Id::Object(0, 1);
        let a = Id::Object(0, 2);
        let b = Id::Object(0, 3);
        let sibling = Id::Object(0, 4);
        let mut memory = Figures::default();
        memory.observe(
            BTreeMap::from([(parent, fragment(10, &[], 2.))]),
            &vision(),
            0.0,
        );
        // One cell keeps its ID; another is newly refined from that cell.
        // An older sibling with a shared ancestor must remain hidden.
        let split = BTreeMap::from([
            (a, fragment(10, &[1], 12.)),
            (b, fragment(11, &[10, 1], 15.)),
            (sibling, fragment(12, &[1], 18.)),
        ]);
        let frame = memory.observe(split.clone(), &Vision::default(), 0.4);
        assert_eq!(frame.visible, BTreeSet::from([a, b]));
        assert!(!memory.remembered.contains_key(&parent));
        assert_eq!(
            frame.scene.bodies.len(),
            2,
            "replace parent without a ghost"
        );
        assert_eq!(
            memory.last_seen[&b], 0.0,
            "inherit without restarting timer"
        );
        let frame = memory.observe(split.clone(), &Vision::default(), 1.0);
        assert!(frame.visible.is_empty());
        assert_eq!(frame.scene.bodies.len(), 2, "keep fragment memories");

        let mut hidden_memory = Figures::default();
        hidden_memory.observe(
            BTreeMap::from([(parent, fragment(10, &[], 2.))]),
            &vision(),
            0.0,
        );
        let frame = hidden_memory.observe(split, &Vision::default(), 1.1);
        assert!(frame.visible.is_empty());
        assert!(hidden_memory.remembered.contains_key(&parent));
        assert!(!hidden_memory.remembered.contains_key(&a));
    }

    #[test]
    fn memory_freezes_tank_geometry_and_status_for_the_shared_renderer() {
        use sim::level::{Level, LevelItem, Team};
        let world = World::from_level(
            &Level {
                items: vec![
                    LevelItem::Spawn {
                        team: Team::Blue,
                        position: [0., 0.],
                        angle: 0.,
                    },
                    LevelItem::Spawn {
                        team: Team::Red,
                        position: [4., 0.],
                        angle: 1.,
                    },
                ],
                ..Level::default()
            },
            Team::Blue,
        )
        .unwrap();
        let mut figures = current(&world);
        let id = *figures
            .iter()
            .find(|(_, f)| !f.snapshot.tanks.is_empty())
            .unwrap()
            .0;
        let initial = figures[&id].snapshot.clone();
        let mut memory = Figures::default();
        memory.observe(figures.clone(), &vision(), 0.0);
        let changed = figures.get_mut(&id).unwrap();
        changed.snapshot.bodies[0].position.x = 12.;
        changed.snapshot.bodies[0].edges = 0;
        changed.snapshot.tanks[0].health = 10.;
        changed.snapshot.tanks[0].turret_angle = 2.;
        let frame = memory.observe(figures, &Vision::default(), 1.0);
        let body = frame
            .scene
            .bodies
            .iter()
            .find(|b| b.id == initial.bodies[0].id)
            .unwrap();
        assert_eq!(body.position, initial.bodies[0].position);
        assert_eq!(body.previous_position, body.position);
        assert_eq!(body.edges, initial.bodies[0].edges);
        assert_eq!(body.opacity, initial.bodies[0].opacity * 0.32);
        assert_eq!(frame.scene.tanks[0].health, initial.tanks[0].health);
        assert_eq!(
            frame.scene.tanks[0].turret_angle,
            initial.tanks[0].turret_angle
        );
    }

    #[test]
    fn unseen_movement_keeps_the_snapshot_until_old_location_is_seen_again() {
        let id = Id::Piece(7);
        let mut memory = Figures::default();
        memory.observe(BTreeMap::from([(id, figure(2.))]), &vision(), 0.0);
        let frame = memory.observe(BTreeMap::from([(id, figure(12.))]), &Vision::default(), 1.0);
        assert!(frame.visible.is_empty());
        assert_eq!(frame.scene.flags[0].1.x, 2.);
        assert_eq!(frame.scene.flags[0].2, 0.32);
        assert_eq!(memory.remembered[&id].polygons[0][0].x, 2.);
        let frame = memory.observe(BTreeMap::from([(id, figure(12.))]), &vision(), 2.0);
        assert!(!memory.remembered.contains_key(&id));
        assert!(frame.scene.flags.is_empty(), "clear the old complete image");
        assert!(frame.visible.is_empty(), "don't disclose its new position");
    }
}

#[cfg(test)]
mod decoration_tests {
    use super::*;
    #[test]
    fn decoration_memory_does_not_reveal_unseen_destruction() {
        use sim::{
            Ammunition, Command, LibraryPreset, Shot,
            level::{Level, LevelItem, Team},
        };
        let level = Level {
            items: vec![
                LevelItem::Spawn {
                    team: Team::Blue,
                    position: [0., 0.],
                    angle: 0.,
                },
                LevelItem::Object {
                    asset: Box::new(LibraryPreset::Crate.asset()),
                    position: [8., 0.],
                    angle: 0.,
                    cutouts: vec![],
                },
            ],
            ..Level::default()
        };
        let mut w = World::from_level(&level, Team::Blue).unwrap();
        let mut fog = Figures::default();
        assert_eq!(
            fog.update(&w, &w.player_vision(0.), 0.)
                .scene
                .decorations
                .len(),
            1
        );
        let away = w.player_vision(std::f32::consts::PI);
        fog.update(&w, &away, 2.);
        w.step(&[Command::Fire(Shot::new(
            Ammunition::ArmorPiercing,
            Vec2::new(12., 0.),
        ))]);
        for _ in 0..120 {
            w.step(&[]);
        }
        assert_eq!(w.decorations().count(), 0);
        let memory = fog.update(&w, &away, 3.).scene;
        assert_eq!(memory.decorations.len(), 1);
        assert!(memory.decorations[0].1 < 1.);
        assert!(
            fog.update(&w, &w.player_vision(0.), 4.)
                .scene
                .decorations
                .is_empty()
        );
    }
}
