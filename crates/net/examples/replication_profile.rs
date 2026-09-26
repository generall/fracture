use sim::{
    level::{Level, Team},
    multiplayer::MultiplayerWorld,
    replica::Frame,
};
use std::time::Instant;
fn main() {
    let mut game = MultiplayerWorld::new(
        &Level::river_valley(),
        &[
            Team::Blue,
            Team::Red,
            Team::Blue,
            Team::Red,
            Team::Blue,
            Team::Red,
        ],
    )
    .unwrap();
    let mut times = [0u128; 6];
    let mut bytes = 0;
    for _ in 0..20 {
        let world = game.for_player(0).unwrap();
        let t = Instant::now();
        let bodies: Vec<_> = world.bodies().collect();
        times[0] += t.elapsed().as_micros();
        let t = Instant::now();
        let _ = world.stats();
        times[1] += t.elapsed().as_micros();
        let t = Instant::now();
        game.prepare_snapshot();
        let shared = Frame::capture(game.for_player(0).unwrap());
        times[2] += t.elapsed().as_micros();
        for i in 0..6 {
            let t = Instant::now();
            let mut frame = Frame::for_player(game.for_player(i).unwrap(), &shared);
            times[3] += t.elapsed().as_micros();
            let t = Instant::now();
            tank_net::replication::strip_diagnostics(&mut frame);
            times[4] += t.elapsed().as_micros();
            let t = Instant::now();
            bytes += tank_net::codec::encode(&frame).unwrap().len();
            times[5] += t.elapsed().as_micros();
        }
        assert!(!bodies.is_empty());
    }
    for (label, t) in [
        "collect bodies",
        "physics stats",
        "shared capture incl vision",
        "six personal captures incl vision",
        "six filters",
        "six compression",
    ]
    .iter()
    .zip(times)
    {
        println!("{label}: {:.3}ms", t as f64 / 20.0 / 1000.0);
    }
    println!("Raw snapshot bytes per peer {}", bytes / 120);
}
