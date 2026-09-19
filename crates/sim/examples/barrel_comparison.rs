//! Export a barrel recipe and review checkpoint. Pass --crush for the six
//! deformation-limit trials, --shot for friction/restitution, or neither for
//! the ten yield/hardening variants. Add --free to --shot to omit the backstop.
//! --release exports the new soft-impact bounce study.
use sim::experiment::{Experiment, Runner};
fn main() {
    let crush = std::env::args().any(|s| s == "--crush");
    let shot = std::env::args().any(|s| s == "--shot");
    let release = std::env::args().any(|s| s == "--release");
    let free = shot && std::env::args().any(|s| s == "--free");
    let mut e = if release {
        Experiment::barrel_release_comparison()
    } else if free {
        Experiment::barrel_free_shot_comparison()
    } else if shot {
        Experiment::barrel_shot_comparison()
    } else if crush {
        Experiment::crush_comparison()
    } else {
        Experiment::barrel_comparison()
    };
    let mut runner = Runner::new(e.clone()).unwrap();
    while runner.advance().is_some() {}
    for (v, r) in e.variants.iter().zip(&runner.recordings) {
        let m = &r.frames.last().unwrap().metrics;
        let state = r.state(e.duration_ticks);
        let mut lo = sim::Vec2::splat(f32::INFINITY);
        let mut hi = sim::Vec2::splat(f32::NEG_INFINITY);
        for pose in &state.frame.poses {
            let geometry = state.geometry.iter().find(|g| g.id == pose.0).unwrap();
            if geometry.material != 2 {
                continue;
            }
            let (sin, cos) = pose.2.sin_cos();
            for v in &geometry.vertices[..geometry.count as usize] {
                let p = sim::Vec2::new(
                    cos * v[0] - sin * v[1] + pose.1[0],
                    sin * v[0] + cos * v[1] + pose.1[1],
                );
                lo = lo.min(p);
                hi = hi.max(p);
            }
        }
        println!(
            "{} / {:.1} m/s / offset {:+.2} m / yield {:.1} kPa: {} chunks, {} cracks, strain {:.1}%, plastic work {:.2}J, size {:.3} x {:.3}m, {} failed solves",
            v.name,
            if shot || release {
                v.impact.shell_speed
            } else {
                v.impact.tank_speed
            },
            v.impact.aim[1],
            v.object.components.plasticity.unwrap().yield_stress / 1000.0,
            m.chunks,
            m.broken,
            m.plastic_strain * 100.0,
            m.plastic_work,
            hi.x - lo.x,
            hi.y - lo.y,
            m.unconverged
        );
        if shot {
            println!(
                "  friction {:.2} / bounce {:.2} / center X {:.3}m",
                v.object.friction,
                v.object.restitution,
                (lo.x + hi.x) * 0.5
            );
        }
    }
    e.cursor_tick = if shot || release {
        120
    } else if crush {
        720
    } else {
        180
    };
    e.review = runner
        .recordings
        .iter()
        .map(|r| r.state(e.cursor_tick))
        .collect();
    std::fs::create_dir_all("web/experiments").unwrap();
    let path = if release {
        "web/experiments/barrel-release.json"
    } else if free {
        "web/experiments/barrel-shot-free.json"
    } else if shot {
        "web/experiments/barrel-shot.json"
    } else if crush {
        "web/experiments/crush.json"
    } else {
        "web/experiments/barrels.json"
    };
    std::fs::write(path, e.to_json().unwrap()).unwrap();
}
