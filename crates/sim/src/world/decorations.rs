use super::*;
use crate::{DecorationKind, DecorationView};
use rapier2d::parry::{query::RayCast, shape::Cuboid};

pub(super) struct Decoration {
    view: DecorationView,
    destroyed: Option<f32>,
}
impl World {
    pub(super) fn add_decoration(
        &mut self,
        kind: DecorationKind,
        position: Vector,
        angle: f32,
        size: Vector,
    ) {
        let id = self.decorations.len();
        self.decorations.push(Decoration {
            view: DecorationView {
                id,
                kind,
                position,
                angle,
                size,
            },
            destroyed: None,
        });
    }
    pub fn decorations(&self) -> impl Iterator<Item = DecorationView> + '_ {
        self.decorations
            .iter()
            .filter(|d| d.destroyed.is_none())
            .map(|d| d.view)
    }
    pub fn decoration_bursts(&self) -> impl Iterator<Item = (DecorationView, f32)> + '_ {
        self.decorations.iter().filter_map(|d| {
            d.destroyed
                .filter(|age| *age < 0.8)
                .map(|age| (d.view, age))
        })
    }
    pub(super) fn sweep_decorations(&mut self, start: Vector, end: Vector, radius: f32) {
        let ray = Ray::new(start, end - start);
        for d in &mut self.decorations {
            if d.destroyed.is_some() {
                continue;
            }
            let shape = Cuboid::new(d.view.size * 0.5 + Vector::splat(radius));
            if shape
                .cast_ray(&Pose::new(d.view.position, d.view.angle), &ray, 1., true)
                .is_some()
            {
                d.destroyed = Some(0.);
            }
        }
    }
    pub(super) fn blast_decorations(&mut self, position: Vector, radius: f32) {
        // Reuse the weapon obstruction query: props behind solid cover survive.
        let victims: Vec<_> = self
            .decorations
            .iter()
            .filter(|d| d.destroyed.is_none())
            .filter(|d| {
                d.view.position.distance(position) <= radius + d.view.size.min_element() * 0.5
            })
            .filter(|d| {
                !self.physics.colliders.iter().any(|(_, c)| {
                    self.weapon_target(c).is_some()
                        && c.shape()
                            .cast_ray(
                                c.position(),
                                &Ray::new(position, d.view.position - position),
                                1.,
                                true,
                            )
                            .is_some()
                })
            })
            .map(|d| d.view.id)
            .collect();
        for id in victims {
            self.decorations[id].destroyed = Some(0.);
        }
    }
    pub(super) fn update_decorations(&mut self) {
        let tanks: Vec<_> = std::iter::once(self.launcher)
            .chain(
                self.tanks
                    .iter()
                    .map(|t| t.id)
                    .filter(|&id| id != self.launcher),
            )
            .filter(|&id| self.tank_is_alive(id))
            .flat_map(|id| {
                self.physics.bodies[self.pieces[id].body]
                    .colliders()
                    .iter()
                    .copied()
            })
            .collect();
        for d in &mut self.decorations {
            if let Some(age) = &mut d.destroyed {
                *age += TICK_DT;
                continue;
            }
            let shape = Cuboid::new(d.view.size * 0.5);
            let pose = Pose::new(d.view.position, d.view.angle);
            if tanks.iter().any(|h| {
                let c = &self.physics.colliders[*h];
                rapier2d::parry::query::intersection_test(&pose, &shape, c.position(), c.shape())
                    .unwrap_or(false)
            }) {
                d.destroyed = Some(0.);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::{Level, LevelItem, Team};
    fn map() -> Level {
        Level {
            items: vec![
                LevelItem::Spawn {
                    team: Team::Blue,
                    position: [-8., 0.],
                    angle: 0.,
                },
                LevelItem::Object {
                    asset: Box::new(crate::LibraryPreset::Crate.asset()),
                    position: [0., 0.],
                    angle: 0.3,
                    cutouts: vec![],
                },
            ],
            ..Level::default()
        }
    }
    #[test]
    fn decorations_have_no_physics_and_shells_create_only_timed_visuals() {
        let level = Level::from_json(&map().to_json().unwrap()).unwrap();
        let mut w = World::from_level(&level, Team::Blue).unwrap();
        let mut bare = level.clone();
        bare.items.pop();
        let baseline = World::from_level(&bare, Team::Blue).unwrap();
        assert_eq!(w.physics.bodies.len(), baseline.physics.bodies.len());
        assert_eq!(w.physics.colliders.len(), baseline.physics.colliders.len());
        assert_eq!(w.decorations().count(), 1);
        w.step(&[Command::Fire(Shot::new(
            Ammunition::ArmorPiercing,
            Vector::new(10., 0.),
        ))]);
        for _ in 0..120 {
            if w.decorations().count() == 0 {
                break;
            }
            w.step(&[]);
        }
        assert_eq!(w.decorations().count(), 0);
        assert_eq!(w.decoration_bursts().count(), 1);
        assert_eq!(w.physics.bodies.len(), baseline.physics.bodies.len());
        for _ in 0..120 {
            w.step(&[]);
        }
        assert_eq!(w.decoration_bursts().count(), 0);
        assert_eq!(w.stats().broken, 0);
    }
    #[test]
    fn decorations_react_to_tracks_laser_and_blast_but_cover_shields_them() {
        let mut w = World::from_level(&map(), Team::Blue).unwrap();
        for _ in 0..600 {
            w.step(&[Command::Tracks {
                left: 1.,
                right: 1.,
            }]);
            if w.decorations().count() == 0 {
                break;
            }
        }
        assert_eq!(w.decorations().count(), 0);
        let mut w = World::from_level(&map(), Team::Blue).unwrap();
        w.fire_laser(
            Vector::new(-6., 0.),
            Vector::X,
            Vector::X,
            true,
            &mut vec![],
        );
        assert_eq!(w.decorations().count(), 0);
        let mut w = World::from_level(&map(), Team::Blue).unwrap();
        w.blast_decorations(Vector::new(-2., 0.), 3.);
        assert_eq!(w.decorations().count(), 0);
        let mut level = map();
        level.items.push(LevelItem::Solid {
            outline: crate::level::rectangle(Vector::new(0.25, 2.)),
            position: [-2., 0.],
            angle: 0.,
            cutouts: vec![],
        });
        let mut w = World::from_level(&level, Team::Blue).unwrap();
        w.fire_laser(
            Vector::new(-6., 0.),
            Vector::X,
            Vector::X,
            true,
            &mut vec![],
        );
        w.blast_decorations(Vector::new(-4., 0.), 5.);
        assert_eq!(w.decorations().count(), 1);
    }
    #[test]
    fn decorations_neither_carve_walls_nor_merge_and_eraser_removes_whole_prop() {
        let mut level = map();
        let prop = level.items[1].clone();
        assert!(!prop.can_join(&prop));
        level.items.insert(
            1,
            LevelItem::Object {
                asset: Box::new(crate::level::wall_asset(Vector::splat(4.))),
                position: [0., 0.],
                angle: 0.,
                cutouts: vec![],
            },
        );
        let meshes = level.resolved_meshes().unwrap();
        assert!((meshes[1].as_ref().unwrap().area - 16.).abs() < 0.01);
        assert!(meshes[2].is_none());
        let mut level = map();
        assert_eq!(
            level
                .erase_rectangle(Vector::new(0.1, -0.1), Vector::new(0.3, 0.1))
                .unwrap(),
            1
        );
        assert_eq!(level.items.len(), 1);
    }
}
