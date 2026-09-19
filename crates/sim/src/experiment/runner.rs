//! Synchronized fixed-tick execution and bounded recording memory.
use super::*;
use crate::{Command, Event, Shot, Vec2, World};

// Finish a shared frame before stopping at the recording budget.
const MAX_RECORDED_POSES: usize = 2_000_000;

pub struct Runner {
    pub experiment: Experiment,
    pub(super) worlds: Vec<World>,
    pub recordings: Vec<Recording>,
    pub completed_tick: u32,
    next_variant: usize,
    flashes: Vec<Vec<([f32; 2], f32, u32)>>,
    peak_elastic: Vec<f64>,
    recorded_poses: usize,
    pub memory_limited: bool,
}
impl Runner {
    pub fn new(experiment: Experiment) -> Result<Self, String> {
        experiment.validate()?;
        let mut worlds = Vec::new();
        for v in &experiment.variants {
            let mut w = World::from_object(&v.object)?;
            if v.impact.backstop {
                w.add_test_backstop(&v.object);
            }
            let (position, speed) = if v.impact.kind == ImpactKind::Tank {
                let left = v
                    .object
                    .outline
                    .iter()
                    .map(|p| p[0])
                    .fold(f32::INFINITY, f32::min)
                    + 3.0;
                (
                    Vec2::new(left - 1.08 - 0.4, v.impact.aim[1]),
                    v.impact.tank_speed,
                )
            } else {
                (Vec2::new(-7.0, 0.0), 0.0)
            };
            let (position, angle) = if v.impact.kind == ImpactKind::Tank
                && v.impact.angle_degrees != 0.0
            {
                let angle = v.impact.angle_degrees.to_radians();
                let forward = Vec2::new(angle.cos(), angle.sin());
                let side = Vec2::new(-forward.y, forward.x);
                let near = v
                    .object
                    .outline
                    .iter()
                    .map(|p| Vec2::from(*p).dot(forward))
                    .fold(f32::INFINITY, f32::min);
                (
                    Vec2::new(3.0, 0.0) + forward * (near - 1.08 - 0.4) + side * v.impact.aim[1],
                    angle,
                )
            } else {
                (position, 0.0)
            };
            w.configure_test_tank_angle(v.impact.tank_mass, position, speed, angle)?;
            worlds.push(w);
        }
        let count = worlds.len();
        let mut runner = Self {
            experiment,
            worlds,
            recordings: (0..count).map(|_| Recording::new()).collect(),
            completed_tick: 0,
            next_variant: 0,
            flashes: vec![vec![]; count],
            peak_elastic: vec![0.0; count],
            recorded_poses: 0,
            memory_limited: false,
        };
        for i in 0..count {
            runner.capture(i, 0);
        }
        Ok(runner)
    }
    pub fn finished(&self) -> bool {
        self.memory_limited || self.completed_tick >= self.experiment.duration_ticks
    }
    /// One variant micro-batch per call so the UI can bound wall-time work.
    pub fn advance(&mut self) -> Option<usize> {
        if self.finished() {
            return None;
        }
        let i = self.next_variant;
        let v = &self.experiment.variants[i];
        let commands = if v.impact.kind == ImpactKind::Tank {
            vec![Command::Tracks {
                left: 1.0,
                right: 1.0,
            }]
        } else if self.completed_tick == 0 || v.impact.ammunition.automatic() {
            vec![Command::Fire(Shot {
                ammunition: v.impact.ammunition,
                target: Vec2::from(v.impact.aim) + Vec2::new(3.0, 0.0),
                mass: v.impact.shell_mass,
                speed: v.impact.shell_speed,
            })]
        } else {
            vec![]
        };
        let events = self.worlds[i].step(&commands);
        let tick = self.completed_tick + 1;
        for e in events {
            if let Event::Exploded { position, radius } = e {
                self.flashes[i].push((position.to_array(), radius, tick + 36));
            }
        }
        self.flashes[i].retain(|(_, _, end)| *end > tick);
        self.peak_elastic[i] = self.peak_elastic[i].max(self.worlds[i].stats().elastic_energy);
        if tick.is_multiple_of(FRAME_TICKS) || tick == self.experiment.duration_ticks {
            self.capture(i, tick);
        }
        self.next_variant += 1;
        if self.next_variant == self.worlds.len() {
            self.next_variant = 0;
            self.completed_tick = tick;
            if tick.is_multiple_of(FRAME_TICKS) && self.recorded_poses >= MAX_RECORDED_POSES {
                self.memory_limited = true;
            }
        }
        Some(i)
    }
    fn capture(&mut self, index: usize, tick: u32) {
        self.recorded_poses += self.recordings[index].capture(
            &self.worlds[index],
            tick,
            &self.flashes[index],
            self.peak_elastic[index],
        );
    }
}
