//! Playable simulation session, queued commands and transient effects.
use crate::{
    navigation,
    ui::Inspector,
    view::{Blast, Spark},
};
use macroquad::prelude::*;
use sim::{Command, Event, Scene, Shot, TICK_DT, World};

pub(crate) struct Lab {
    pub(crate) world: World,
    pub(crate) custom_level: Option<(sim::level::Level, sim::level::Team)>,
    pub(crate) custom_object: Option<sim::ObjectAsset>,
    pub(crate) inspector: Inspector,
    pub(crate) paused: bool,
    pub(crate) slow: bool,
    pub(crate) overlay: bool,
    pub(crate) accumulator: f64,
    pub(crate) aim: Vec2,
    pub(crate) pending: Vec<Command>,
    pub(crate) firing: bool,
    pub(crate) sparks: Vec<Spark>,
    pub(crate) blasts: Vec<Blast>,
    pub(crate) navigation: navigation::Navigation,
    pub(crate) last_mouse: Vec2,
    pub(crate) tick_ms: f64,
    pub(crate) overloaded: f32,
    pub(crate) notice: Option<(String, f32)>,
}

impl Lab {
    pub(crate) fn new() -> Self {
        Self {
            world: World::new(Scene::Masonry),
            custom_object: None,
            custom_level: None,
            inspector: Inspector::default(),
            paused: false,
            slow: false,
            overlay: false,
            accumulator: 0.0,
            aim: vec2(3.0, 0.0),
            pending: Vec::new(),
            firing: false,
            sparks: Vec::new(),
            blasts: Vec::new(),
            navigation: navigation::Navigation::default(),
            last_mouse: Vec2::ZERO,
            tick_ms: 0.0,
            overloaded: 0.0,
            notice: None,
        }
    }

    pub(crate) fn reset(&mut self, scene: Scene) {
        self.world = if scene == Scene::Level {
            self.custom_level
                .as_ref()
                .and_then(|(level, team)| World::from_level(level, *team).ok())
                .unwrap_or_else(|| World::new(Scene::Masonry))
        } else if scene == Scene::Custom {
            self.custom_object
                .as_ref()
                .and_then(|a| World::from_object(a).ok())
                .unwrap_or_else(|| World::new(Scene::Masonry))
        } else {
            World::new(scene)
        };
        self.pending.clear();
        self.firing = false;
        self.sparks.clear();
        self.blasts.clear();
        self.accumulator = 0.0;
        self.tick_ms = 0.0;
        self.notice = None;
        self.overloaded = 0.0;
        self.fit_camera();
        self.aim = vec2(3.0, 0.0);
        if scene == Scene::Level {
            let p = self.world.launcher_position();
            let direction = if self.world.team() == sim::level::Team::Blue {
                1.0
            } else {
                -1.0
            };
            self.aim = vec2(p.x + 5.0 * direction, p.y);
        }
    }

    pub(crate) fn fit_camera(&mut self) {
        self.navigation = navigation::Navigation::default();
        if self.world.scene() == Scene::Level
            && let Some((level, _)) = &self.custom_level
        {
            self.navigation = navigation::Navigation::fitted(
                Vec2::ZERO,
                (24.0 / level.size[0]).min(15.0 / level.size[1]),
            );
        }
    }

    pub(crate) fn fire(&mut self) {
        // Bound queued input while paused. A click replaces the pending shot.
        self.pending.retain(|c| !matches!(c, Command::Fire(_)));
        self.pending.push(Command::Fire(Shot {
            ammunition: self.inspector.ammunition,
            target: sim::Vec2::new(self.aim.x, self.aim.y),
            mass: self.inspector.mass,
            speed: self.inspector.speed,
        }));
        if self.paused {
            self.notice = Some(("Shot queued. Step or resume to fire.".into(), 3.0));
        }
    }

    pub(crate) fn tick(&mut self, tracks: [f32; 2]) {
        if self.firing
            && self.inspector.ammunition.automatic()
            && !self.pending.iter().any(|c| matches!(c, Command::Fire(_)))
        {
            self.fire();
        }
        self.pending.push(Command::Tracks {
            left: tracks[0],
            right: tracks[1],
        });
        let start = get_time();
        let events = self.world.step(&self.pending);
        let elapsed = (get_time() - start) * 1_000.0;
        self.tick_ms = self.tick_ms * 0.9 + elapsed * 0.1;
        self.pending.clear();
        for spark in &mut self.sparks {
            spark.position += spark.velocity * TICK_DT;
            spark.life -= TICK_DT;
        }
        self.sparks.retain(|spark| spark.life > 0.0);
        for blast in &mut self.blasts {
            blast.life -= TICK_DT;
        }
        self.blasts.retain(|blast| blast.life > 0.0);
        for (index, event) in events.iter().enumerate() {
            match *event {
                Event::Fractured { position, normal } | Event::Impacted { position, normal } => {
                    for i in 0..4 {
                        let angle = (index * 13 + i * 37) as f32 * 0.71;
                        let direction = vec2(angle.cos(), angle.sin());
                        self.sparks.push(Spark {
                            position: vec2(position.x, position.y),
                            velocity: direction * (1.0 + i as f32 * 0.45)
                                + vec2(normal.x, normal.y) * 0.5,
                            life: 0.3 + i as f32 * 0.025,
                        });
                    }
                }
                Event::Fired { position, velocity } => {
                    self.notice = None;
                    self.sparks.push(Spark {
                        position: vec2(position.x, position.y),
                        velocity: vec2(velocity.x, velocity.y).normalize_or_zero() * 2.0,
                        life: 0.1,
                    });
                }
                Event::Exploded { position, radius } => {
                    let position = vec2(position.x, position.y);
                    self.blasts.push(Blast {
                        position,
                        radius,
                        life: 0.45,
                    });
                    for i in 0..36 {
                        let angle = i as f32 * 2.399_963;
                        self.sparks.push(Spark {
                            position,
                            velocity: Vec2::from_angle(angle) * (2.0 + (i % 7) as f32 * 0.8),
                            life: 0.18 + (i % 9) as f32 * 0.04,
                        });
                    }
                }
                Event::BridgeCollapsed { .. } => {
                    self.notice = Some(("BRIDGE LOST / CROSSING CLOSED".into(), 5.0))
                }
                Event::FlagCaptured { team, .. } => {
                    self.notice = Some((
                        format!("{} VICTORY / ENEMY FLAG CAPTURED", team.label()),
                        10.0,
                    ));
                }
                Event::TankDestroyed { team, .. } => {
                    self.notice = Some((
                        format!("{} TANK DESTROYED / RESPAWN IN 3 S", team.label()),
                        2.0,
                    ));
                }
                Event::TankRespawned { team, .. } => {
                    self.notice = Some((format!("{} TANK RESPAWNED", team.label()), 1.0));
                }
                Event::TankSunk { .. } => {
                    self.notice = Some(("TANK LOST / R TO REBUILD".into(), 5.0))
                }
                Event::MuzzleBlocked => self.notice = Some(("BARREL OBSTRUCTED".into(), 2.0)),
                Event::LaserLimited => {
                    self.notice = Some(("CUTTING DETAIL LIMIT / R TO REBUILD".into(), 2.0))
                }
            }
        }
    }

    pub(crate) fn select_ammunition(&mut self, ammunition: sim::Ammunition) {
        self.inspector.ammunition = ammunition;
        (self.inspector.mass, self.inspector.speed) = ammunition.defaults();
        self.pending.retain(|c| !matches!(c, Command::Fire(_)));
    }
}
