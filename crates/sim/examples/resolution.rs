//! Compare rigid motion and structural work as authoring detail increases.
use sim::{Command, ObjectAsset, ObjectPreset, Shot, Vec2, World};
use std::time::Instant;
fn main() {
    for breakable in [false, true] {
        for detail in 0..=4 {
            let mut a = ObjectAsset::preset(ObjectPreset::Rock);
            a.refinement = detail;
            if !breakable {
                a.tension = 1e9;
                a.compression = 1e9;
                a.shear = 1e9;
            }
            let mut w = World::from_object(&a).unwrap();
            let cells = w.stats().cells;
            let mut times = Vec::new();
            let mut iterations = 0;
            let mut residual = 0.0_f64;
            for tick in 0..120 {
                let commands = if tick == 0 {
                    vec![Command::Fire(Shot {
                        ammunition: sim::Ammunition::HighExplosive,
                        target: Vec2::new(3.0, 0.0),
                        mass: 6.0,
                        speed: 220.0,
                    })]
                } else {
                    vec![]
                };
                let start = Instant::now();
                w.step(&commands);
                times.push(start.elapsed().as_secs_f64() * 1000.0);
                iterations = iterations.max(w.stats().structural_iterations);
                residual = residual.max(w.stats().structural_residual);
            }
            times.sort_by(f64::total_cmp);
            println!(
                "breakable {breakable}, detail {detail}: {cells} cells, {} chunks; p95 {:.3} ms, peak {:.3} ms; iterations {iterations}, residual {residual:.2e}, unconverged {}",
                w.stats().chunks,
                times[114],
                times[119],
                w.stats().unconverged
            );
        }
    }
}
