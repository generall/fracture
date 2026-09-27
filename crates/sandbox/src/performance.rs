//! Opt-in native full-game workload. Never compiled into ordinary packages.
use crate::{game::Game, lab::Lab};
use macroquad::prelude::*;
use std::time::Instant;

pub(crate) struct Run {
    scenario: String,
    seconds: f64,
    started: Instant,
    warmup_until: Instant,
    starting_tick: u64,
    next_report: f64,
    previous: Option<Instant>,
    stage: Instant,
    current: Vec<f64>,
    frames: Vec<Vec<f64>>,
    physics: sim::PhysicsProfile,
    slow_frames: Vec<serde_json::Value>,
    targets: Vec<sim::Vec2>,
    output: std::path::PathBuf,
}

impl Run {
    pub fn from_environment(game: &mut Game, lab: &mut Lab) -> Option<Self> {
        let scenario = std::env::var("FRACTURE_PERF_SCENARIO").ok()?;
        assert_eq!(std::env::var("FRACTURE_SILENT").as_deref(), Ok("1"));
        game.select_map(match scenario.as_str() {
            "river" => 0,
            "classic" => 1,
            _ => panic!("FRACTURE_PERF_SCENARIO must be river or classic"),
        });
        game.config.opponent = sim::level::EnemyActivity::Attack;
        game.config.time_limit_seconds = None;
        game.start(lab);
        assert_eq!(game.screen as u32, crate::game::Screen::Match as u32);
        game.countdown = 0.0;
        let origin = lab.world.launcher_position();
        let mut points: Vec<_> = lab
            .world
            .bodies()
            .filter(|b| matches!(b.material, sim::Material::Masonry | sim::Material::Rock))
            .map(|b| b.position)
            .filter(|p| p.distance(origin) > 3.0)
            .collect();
        points.sort_by(|a, b| {
            a.distance_squared(origin)
                .total_cmp(&b.distance_squared(origin))
        });
        let mut targets: Vec<sim::Vec2> = vec![];
        for p in points {
            if targets.iter().all(|q| q.distance(p) > 3.0) {
                targets.push(p);
            }
            if targets.len() == 12 {
                break;
            }
        }
        assert!(!targets.is_empty());
        let seconds: f64 = std::env::var("FRACTURE_PERF_SECONDS")
            .unwrap_or_else(|_| "120".into())
            .parse()
            .expect("positive duration");
        assert!(seconds.is_finite() && seconds > 0.0);
        let output = std::env::var_os("FRACTURE_PERF_OUTPUT")
            .map(std::path::PathBuf::from)
            .expect("set FRACTURE_PERF_OUTPUT");
        let now = Instant::now();
        Some(Self {
            scenario,
            seconds,
            started: now,
            warmup_until: now + std::time::Duration::from_secs(3),
            starting_tick: 0,
            next_report: 10.0,
            previous: None,
            stage: now,
            current: vec![],
            frames: vec![],
            physics: Default::default(),
            slow_frames: vec![],
            targets,
            output,
        })
    }

    pub fn begin_frame(&mut self, lab: &mut Lab) -> bool {
        let now = Instant::now();
        if now < self.warmup_until {
            self.current.clear();
            self.stage = now;
            return false;
        }
        if let Some(previous) = self.previous {
            let mut sample = vec![(now - previous).as_secs_f64() * 1000.0];
            sample.append(&mut self.current);
            assert_eq!(sample.len(), 8, "all gameplay stages must be measured");
            let physics = lab.world.take_physics_profile();
            self.physics += physics;
            if sample[0] > 33.334 {
                self.slow_frames.push(serde_json::json!({
                    "frame":self.frames.len(), "wall_ms":sample[0],
                    "cpu_ms":&sample[1..], "physics":physics,
                }));
            }
            self.frames.push(sample);
        } else {
            self.started = now;
            self.starting_tick = lab.match_progress.as_ref().unwrap().elapsed_ticks;
            lab.world.take_physics_profile();
            self.current.clear();
        }
        self.previous = Some(now);
        self.stage = now;
        let wall = (now - self.started).as_secs_f64();
        if wall >= self.next_report {
            eprintln!(
                "Performance: {wall:.1}s, {} frames, {:.2}s simulated",
                self.frames.len(),
                (lab.match_progress.as_ref().unwrap().elapsed_ticks - self.starting_tick) as f64
                    / 120.0
            );
            self.next_report += 10.0;
        }
        if wall < self.seconds {
            return false;
        }
        let names = [
            "frame",
            "input_audio",
            "physics_effects",
            "camera",
            "vision",
            "fog",
            "draw_submit",
            "ui",
        ];
        let mut summary = serde_json::Map::new();
        for (index, name) in names.iter().enumerate() {
            let mut values: Vec<_> = self.frames.iter().map(|r| r[index]).collect();
            values.sort_by(f64::total_cmp);
            let p = |fraction: f64| values[((values.len() - 1) as f64 * fraction) as usize];
            summary.insert(
                (*name).into(),
                serde_json::json!({
                    "mean_ms": values.iter().sum::<f64>() / values.len() as f64,
                    "p50_ms":p(0.5), "p95_ms":p(0.95), "p99_ms":p(0.99), "max_ms":p(1.0)
                }),
            );
        }
        let report = serde_json::json!({
            "scenario":self.scenario, "wall_seconds":wall, "frames":self.frames.len(),
            "average_fps":self.frames.len() as f64 / wall,
            "over_33ms":self.frames.iter().filter(|r| r[0] > 33.334).count(),
            "over_50ms":self.frames.iter().filter(|r| r[0] > 50.0).count(),
            "warmup_seconds":3,
            "viewport":[screen_width(), screen_height()],
            "simulation_seconds":(lab.match_progress.as_ref().unwrap().elapsed_ticks - self.starting_tick) as f64 / 120.0,
            "stats":lab.world.stats(), "stages":summary, "columns":names, "samples_ms":self.frames,
            "physics_stages_total_ms":self.physics, "slow_frames":self.slow_frames,
        });
        std::fs::write(&self.output, serde_json::to_vec(&report).unwrap())
            .expect("write frame report");
        eprintln!("Performance report: {}", self.output.display());
        true
    }

    pub fn mark(&mut self, name: &str) {
        const STAGES: [&str; 7] = ["input", "physics", "camera", "vision", "fog", "draw", "ui"];
        assert_eq!(name, STAGES[self.current.len()]);
        let now = Instant::now();
        self.current.push((now - self.stage).as_secs_f64() * 1000.0);
        self.stage = now;
    }

    pub fn input(&mut self, lab: &mut Lab) -> [f32; 2] {
        let tick = lab.match_progress.as_ref().unwrap().elapsed_ticks;
        let target = self.targets[(tick as usize / 480) % self.targets.len()];
        let delta = target - lab.world.launcher_position();
        lab.keyboard_aim.point_stick(vec2(delta.x, delta.y));
        lab.firing = true;
        lab.fire();
        // Alternate stationary bombardment with driving into damaged walls.
        if (tick / 1200).is_multiple_of(2) {
            [0.0; 2]
        } else {
            let heading = lab
                .world
                .bodies()
                .find(|b| b.id == lab.world.player_tank_id())
                .unwrap()
                .angle;
            crate::controls::stick_tracks(vec2(delta.x, delta.y).normalize_or_zero(), heading)
        }
    }
}
