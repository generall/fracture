//! Measure wall separation and later travel with the same floor friction.
use sim::experiment::{Experiment, Runner, VisualState};

fn span(state: &VisualState) -> (sim::Vec2, sim::Vec2) {
    let mut lo = sim::Vec2::splat(f32::INFINITY);
    let mut hi = sim::Vec2::splat(f32::NEG_INFINITY);
    for pose in &state.frame.poses {
        let g = state.geometry.iter().find(|g| g.id == pose.0).unwrap();
        if g.material != 2 {
            continue;
        }
        let (sin, cos) = pose.2.sin_cos();
        for v in &g.vertices[..g.count as usize] {
            let p = sim::Vec2::new(
                cos * v[0] - sin * v[1] + pose.1[0],
                sin * v[0] + cos * v[1] + pose.1[1],
            );
            lo = lo.min(p);
            hi = hi.max(p);
        }
    }
    (lo, hi)
}

fn main() {
    let gentle = std::env::args().any(|s| s == "--gentle");
    let soft = std::env::args().any(|s| s == "--soft");
    for backstop in [true, false] {
        let mut e = Experiment::barrel_shot_comparison();
        let reference = e.variants[1].clone();
        let values = if gentle {
            vec![0.6, 0.62, 0.64, 0.66, 0.68, 0.7]
        } else {
            vec![0.04, 0.08, 0.12, 0.16, 0.2, 0.25, 0.3, 0.4, 0.5, 0.7]
        };
        e.favorite = None;
        e.variants = values
            .into_iter()
            .map(|bounce| {
                let mut v = reference.clone();
                v.object.restitution = if soft || gentle { 0.04 } else { bounce };
                if soft || gentle {
                    v.object.soft_bounce = Some(sim::SoftBounce {
                        restitution: if gentle { bounce } else { 0.7 },
                        speed: if gentle { 16.0 } else { bounce * 40.0 },
                    });
                }
                v.impact.backstop = backstop;
                v
            })
            .collect();
        let mut runner = Runner::new(e.clone()).unwrap();
        while runner.advance().is_some() {}
        for (v, r) in e.variants.iter().zip(&runner.recordings) {
            let (lo, hi) = span(&r.state(e.duration_ticks));
            let (early_lo, early_hi) = span(&r.state(24));
            println!(
                "backstop={backstop} soft={:?} bounce={:.2} friction={:.2} final gap={:.3}m center X={:.3}m X at 0.2s={:.3}m",
                v.object.soft_bounce,
                v.object.restitution,
                v.object.friction,
                3.8 - hi.x,
                (lo.x + hi.x) * 0.5,
                (early_lo.x + early_hi.x) * 0.5
            );
        }
    }
}
