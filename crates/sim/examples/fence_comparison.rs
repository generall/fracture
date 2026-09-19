//! Run all fence variants and export their recipe plus a synchronized review frame.
use sim::experiment::{Experiment, Runner};

fn main() {
    let mut experiment = Experiment::fence_comparison();
    let default = sim::LibraryPreset::MetalFence.asset();
    let mut selected = experiment.variants[experiment.favorite.unwrap()]
        .object
        .clone();
    selected.name = default.name.clone();
    assert_eq!(
        selected, default,
        "accepted trial must match the metal fence starter"
    );
    let mut runner = Runner::new(experiment.clone()).unwrap();
    while runner.advance().is_some() {}
    assert!(!runner.memory_limited);
    for (variant, recording) in experiment.variants.iter().zip(&runner.recordings) {
        let metrics = &recording.frames.last().unwrap().metrics;
        let tank_id = recording
            .state(0)
            .geometry
            .iter()
            .find(|g| g.material == 4)
            .unwrap()
            .id;
        let mut drift = 0.0_f32;
        let mut heading = 0.0_f32;
        let mut cleared_at = None;
        for f in &recording.frames {
            if let Some(p) = f.poses.iter().find(|p| p.0 == tank_id) {
                drift = drift.max(p.1[1].abs());
                heading = heading.max(p.2.abs().to_degrees());
                if p.1[0] > 6.0 {
                    cleared_at = Some(f.tick as f32 / 120.0);
                    break;
                }
            }
        }
        let tank = recording
            .frames
            .last()
            .unwrap()
            .poses
            .iter()
            .find(|p| p.0 == tank_id)
            .unwrap();
        println!(
            "  tank x {:.3}, drift {:.3}m, heading {:.2} degrees, cleared at {:?}s",
            tank.1[0], drift, heading, cleared_at
        );

        println!(
            "{}: strain {:.1}%, work {:.2} J, {} cracks, {} failed solves",
            variant.name,
            metrics.plastic_strain * 100.0,
            metrics.plastic_work,
            metrics.broken,
            metrics.unconverged
        );
        assert_eq!(metrics.unconverged, 0);
        assert_eq!(
            metrics.broken, 0,
            "fences should stay connected while bending"
        );
    }
    experiment.cursor_tick = 240;
    experiment.review = runner
        .recordings
        .iter()
        .map(|r| r.state(experiment.cursor_tick))
        .collect();
    std::fs::write("web/experiments/fences.json", experiment.to_json().unwrap()).unwrap();
}
