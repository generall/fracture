//! Fire, smoke and a cosmetic turret above the independently simulated wreck.
use crate::{
    ui::{CYAN, INK, MUTED, PINK},
    view::{Viewport, tank_parts},
};
use macroquad::prelude::*;
use sim::{Ammunition, DestructionCause as Cause, TankDestruction, level::Team};
const LIFETIME: f32 = 2.5;
fn tint(c: Color, a: f32) -> Color {
    Color { a: c.a * a, ..c }
}
fn noise(n: usize) -> f32 {
    ((n as f32 * 12.9898).sin() * 43758.547).fract().abs()
}
fn v(p: sim::Vec2) -> Vec2 {
    vec2(p.x, p.y)
}

pub(crate) struct WreckEffect {
    position: Vec2,
    details: TankDestruction,
    color: Color,
    seed: usize,
    age: f32,
}
impl WreckEffect {
    pub fn new(position: sim::Vec2, team: Team, details: TankDestruction, seed: usize) -> Self {
        Self {
            position: v(position),
            details,
            color: match team {
                Team::Blue => CYAN,
                Team::Red => PINK,
                Team::Neutral => MUTED,
            },
            seed,
            age: 0.0,
        }
    }
    pub fn advance(&mut self, dt: f32) {
        self.age += dt;
    }
    pub fn alive(&self) -> bool {
        self.age < LIFETIME
    }
    pub fn draw(&self, view: &Viewport, alpha: f32) {
        let t = self.age + alpha * sim::TICK_DT;
        let fade = ((LIFETIME - t) / 0.6).clamp(0.0, 1.0);
        if fade <= 0.0 {
            return;
        }
        let at = view.screen(self.position);
        if self.details.impact.cause == Cause::Collapse {
            for i in 0..3 {
                let age = t - i as f32 * 0.18;
                if (0.0..1.3).contains(&age) {
                    draw_circle_lines(
                        at.x,
                        at.y,
                        (0.7 + age * 1.4) * view.scale,
                        1.3,
                        tint(CYAN, (1.0 - age / 1.3) * 0.5),
                    );
                }
            }
            return;
        }
        let explosive = matches!(
            self.details.impact.cause,
            Cause::Blast | Cause::Shell(Ammunition::HighExplosive)
        );
        let ram = self.details.impact.cause == Cause::Ram;
        let laser = self.details.impact.cause == Cause::Laser;
        let direction = if self.details.impact.direction.length_squared() > 0.001 {
            v(self.details.impact.direction).normalize()
        } else {
            Vec2::from_angle(self.details.hull_angle)
        };
        let strength = if explosive {
            1.0
        } else if ram {
            0.3
        } else if laser {
            0.5
        } else {
            0.7
        };
        let lift = 5.5 * strength;
        let flight = lift / 5.0;
        let height = if t <= flight {
            (lift * t - 5.0 * t * t).max(0.0)
        } else {
            0.14 * (-10.0 * (t - flight)).exp() * ((t - flight) * 20.0).sin().abs()
        };
        let drift_time = (1.0 - (-1.6 * t).exp()) / 1.6;
        let drift = (direction * (0.7 + 2.8 * strength)
            + v(self.details.velocity).clamp_length_max(5.0) * 0.2)
            * drift_time;
        let spin = (noise(self.seed + 17) - 0.5) * (3.0 + 8.0 * strength);
        if self.details.vehicle == sim::VehicleKind::Tank {
            tank_parts::draw_turret(
                view,
                self.position + drift,
                self.details.turret_angle + spin * drift_time,
                height,
                fade,
                self.color,
            );
        } else {
            let p = view.screen(self.position + drift) - vec2(0.0, height * view.scale);
            let direction = Vec2::from_angle(self.details.turret_angle + spin * drift_time);
            let length = if self.details.vehicle == sim::VehicleKind::Scout {
                0.5
            } else {
                0.9
            };
            draw_line(
                p.x,
                p.y,
                p.x + direction.x * view.scale * length,
                p.y + direction.y * view.scale * length,
                if self.details.vehicle == sim::VehicleKind::Scout {
                    2.0
                } else {
                    7.0
                },
                tint(self.color, fade),
            );
        }
        // Fuel fire stays at the known death position; it never follows unseen fragments.
        let hot = Color::from_rgba(255, 190, 70, 255);
        let red = Color::from_rgba(244, 88, 36, 255);
        let burn = (1.0 - t / 2.1).max(0.0) * fade;
        for i in 0..6 {
            let r = noise(self.seed + i * 11);
            let p =
                at + vec2((r - 0.5) * 1.1, (noise(self.seed + i + 9) - 0.5) * 0.65) * view.scale;
            let flicker = 0.75 + 0.25 * (t * 24.0 + i as f32 * 2.3).sin();
            let h = (0.22 + strength * 0.55) * flicker * view.scale;
            let w = (0.10 + strength * 0.08) * view.scale;
            let tip = p + vec2((t * 17.0 + i as f32).sin() * w * 0.4, -h);
            draw_circle(p.x, p.y, w * 1.8, tint(red, burn * 0.12));
            draw_triangle(
                p - vec2(w, 0.0),
                p + vec2(w, 0.0),
                tip,
                tint(red, burn * 0.7),
            );
            draw_triangle(
                p - vec2(w * 0.5, 0.0),
                p + vec2(w * 0.5, 0.0),
                p.lerp(tip, 0.75),
                tint(hot, burn * 0.9),
            );
            draw_circle(p.x, p.y, w * 0.28, tint(INK, burn * 0.7));
        }
        // Expanding translucent puffs rise and drift, with staggered birth times.
        for i in 0..15 {
            let age = t - i as f32 * 0.10;
            if !(0.0..1.8).contains(&age) {
                continue;
            }
            let r = noise(self.seed + 81 + i);
            let position =
                self.position + vec2((r - 0.5) * 0.8 + age * 0.22, -age * (0.7 + strength * 0.3));
            let p = view.screen(position);
            let opacity = ((age / 0.15).min(1.0) * (1.0 - age / 1.8) * fade * 0.17).max(0.0);
            let smoke = Color::from_rgba(105, 104, 112, 255);
            draw_circle(
                p.x,
                p.y,
                (0.16 + age * (0.32 + strength * 0.2)) * view.scale,
                tint(smoke, opacity),
            );
        }
        if t < 0.3 {
            let p = view.screen(v(self.details.impact.point));
            let flash = 1.0 - t / 0.3;
            draw_circle(
                p.x,
                p.y,
                (0.10 + strength * 0.3) * view.scale * flash,
                tint(if laser { CYAN } else { hot }, flash * 0.6),
            );
        }
    }
}
