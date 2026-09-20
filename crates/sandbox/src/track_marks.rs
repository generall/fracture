//! Distance-spaced, temporary tread prints. These never enter the physics world.
use crate::view::Viewport;
use macroquad::prelude::{Color, draw_line, vec2};
use sim::{TerrainKind, Vec2, Vision, World};
use std::collections::{BTreeMap, VecDeque};

const SPACING: f32 = 0.18;
const LIFETIME: f32 = 10.0;
const MAX_MARKS: usize = 8192;
const HALF_WIDTH: f32 = 0.15;

struct Trail {
    points: [Vec2; 2],
    angle: f32,
    distance: [f32; 2],
    tick: u64,
}
struct Mark {
    center: Vec2,
    side: Vec2,
    born: u64,
    seen: bool,
}
#[derive(Default)]
pub(crate) struct TrackMarks {
    tick: u64,
    trails: BTreeMap<usize, Trail>,
    marks: VecDeque<Mark>,
}

fn points(center: Vec2, angle: f32) -> [Vec2; 2] {
    let side = Vec2::from_angle(angle).perp();
    [center - side * 0.65, center + side * 0.65]
}

impl TrackMarks {
    /// Seed new/respawned tanks before stepping so the first movement is recorded.
    pub(crate) fn seed(&mut self, world: &World) {
        for (id, center, angle) in world.tank_poses() {
            self.trails.entry(id).or_insert(Trail {
                points: points(center, angle),
                angle,
                distance: [0.0; 2],
                tick: self.tick,
            });
        }
    }

    pub(crate) fn update(&mut self, world: &World) {
        self.tick += 1;
        while self
            .marks
            .front()
            .is_some_and(|m| (self.tick - m.born) as f32 * sim::TICK_DT >= LIFETIME)
        {
            self.marks.pop_front();
        }
        for (id, center, angle) in world.tank_poses() {
            let Some(trail) = self.trails.get_mut(&id) else {
                continue;
            };
            let next = points(center, angle);
            let delta_angle = (angle - trail.angle)
                .sin()
                .atan2((angle - trail.angle).cos());
            for (i, &end) in next.iter().enumerate() {
                let start = trail.points[i];
                let length = start.distance(end);
                // Ignore contact jitter and don't draw lines across respawns/teleports.
                if length > 2.0 {
                    trail.distance[i] = 0.0;
                    continue;
                }
                if length < 0.0001 {
                    continue;
                }
                let mut along = SPACING - trail.distance[i];
                while along <= length {
                    let t = along / length;
                    let point = start.lerp(end, t);
                    let forward = Vec2::from_angle(trail.angle + delta_angle * t);
                    let side = forward.perp() * HALF_WIDTH;
                    // Check the tread footprint, not just the hull center, at terrain edges.
                    let on_ground = [
                        point,
                        point - side - forward * 0.035,
                        point - side + forward * 0.035,
                        point + side - forward * 0.035,
                        point + side + forward * 0.035,
                    ]
                    .into_iter()
                    .all(|p| {
                        matches!(
                            world.terrain_at(p),
                            TerrainKind::Ground | TerrainKind::Elevation
                        )
                    });
                    if on_ground {
                        self.marks.push_back(Mark {
                            center: point,
                            side,
                            born: self.tick,
                            seen: false,
                        });
                    }
                    along += SPACING;
                }
                trail.distance[i] = (trail.distance[i] + length) % SPACING;
            }
            trail.points = next;
            trail.angle = angle;
            trail.tick = self.tick;
        }
        self.trails.retain(|_, trail| trail.tick == self.tick);
        while self.marks.len() > MAX_MARKS {
            self.marks.pop_front();
        }
    }

    pub(crate) fn draw(&mut self, viewport: &Viewport, vision: Option<&Vision>) {
        for mark in &mut self.marks {
            let visible = vision.is_none_or(|v| v.contains(mark.center));
            mark.seen |= visible;
            if !mark.seen {
                continue;
            }
            let age = (self.tick - mark.born) as f32 * sim::TICK_DT;
            // Hold the print, then fade over its final three simulation seconds.
            let opacity =
                ((LIFETIME - age) / 3.0).clamp(0.0, 1.0) * if visible { 0.55 } else { 0.20 };
            let a = mark.center - mark.side;
            let b = mark.center + mark.side;
            let a = viewport.screen(vec2(a.x, a.y));
            let b = viewport.screen(vec2(b.x, b.y));
            draw_line(
                a.x,
                a.y,
                b.x,
                b.y,
                0.07 * viewport.scale,
                Color::new(0.28, 0.25, 0.20, opacity),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::level::{Level, LevelItem, Team};

    fn world(kind: TerrainKind, min_y: f32) -> World {
        let level = Level {
            size: [24.0, 15.0],
            items: vec![
                LevelItem::Spawn {
                    team: Team::Blue,
                    position: [-4.0, 0.0],
                    angle: 0.0,
                },
                LevelItem::Terrain {
                    terrain: kind,
                    min: [-10.0, min_y],
                    max: [10.0, 5.0],
                },
            ],
            ..Level::default()
        };
        World::from_level(&level, Team::Blue).unwrap()
    }

    fn move_tank(world: &mut World, marks: &mut TrackMarks, x: f32) {
        marks.seed(world);
        world
            .configure_test_tank(4500.0, Vec2::new(x, 0.0), 3.0)
            .unwrap();
        marks.update(world);
    }

    #[test]
    fn moving_treads_mark_only_ground_and_expire_after_ten_seconds() {
        for kind in [
            TerrainKind::Ground,
            TerrainKind::Elevation,
            TerrainKind::Road,
            TerrainKind::TallGrass,
            TerrainKind::Sand,
        ] {
            let mut world = world(kind, -5.0);
            let mut marks = TrackMarks::default();
            for i in 1..=20 {
                move_tank(&mut world, &mut marks, -4.0 + i as f32 * 0.1);
            }
            assert_eq!(
                !marks.marks.is_empty(),
                matches!(kind, TerrainKind::Ground | TerrainKind::Elevation),
                "{kind:?}"
            );
            if matches!(kind, TerrainKind::Ground | TerrainKind::Elevation) {
                assert!(marks.marks.iter().any(|m| m.center.y < 0.0));
                assert!(marks.marks.iter().any(|m| m.center.y > 0.0));
                let count = marks.marks.len();
                for _ in 0..120 {
                    marks.update(&world);
                }
                assert_eq!(
                    marks.marks.len(),
                    count,
                    "standing still leaves no new marks"
                );
                let born = marks.marks.back().unwrap().born;
                while marks.tick - born < 1199 {
                    marks.update(&world);
                }
                assert!(!marks.marks.is_empty());
                marks.update(&world);
                assert!(marks.marks.is_empty(), "all prints expire at ten seconds");
            }
        }
    }

    #[test]
    fn terrain_is_checked_per_tread_and_teleports_do_not_draw_a_trail() {
        let mut world = world(TerrainKind::Road, 0.0);
        let mut marks = TrackMarks::default();
        // Reverse travel still leaves tracks on the ground side only.
        for i in 1..=20 {
            move_tank(&mut world, &mut marks, -4.0 - i as f32 * 0.1);
        }
        assert!(!marks.marks.is_empty());
        assert!(marks.marks.iter().all(|m| m.center.y < 0.0));
        let count = marks.marks.len();
        move_tank(&mut world, &mut marks, 4.0);
        assert_eq!(marks.marks.len(), count);
    }
}
