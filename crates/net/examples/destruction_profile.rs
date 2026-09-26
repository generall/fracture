//! Headless worst case for mass destruction: two tanks shell a dense yard of
//! walls and explosive barrels for 20 seconds. Reports host tick, snapshot,
//! client decode/apply (including client-side sight) and wire size per peer.
use sim::{
    LibraryPreset, Vec2, World,
    level::{Level, LevelItem, Team},
    multiplayer::{MultiplayerWorld, PlayerInput},
    replica::{Frame, Map},
};
use std::time::Instant;
use tank_net::{
    codec,
    replication::{self, Decoder, Encoder, Records},
};

fn report(name: &str, mut values: Vec<f64>) {
    values.sort_by(f64::total_cmp);
    let pct = |p: f64| values[((values.len() - 1) as f64 * p) as usize];
    println!(
        "{name}: median={:.2} p95={:.2} p99={:.2} max={:.2}",
        pct(0.5),
        pct(0.95),
        pct(0.99),
        pct(1.0)
    );
}

fn main() {
    let mut level = Level::default();
    for (i, x) in [-6.0f32, -3.0, 0.0, 3.0, 6.0].into_iter().enumerate() {
        for y in [-6.0f32, -2.0, 2.0, 6.0] {
            // Alternate columns: masonry, then explosive barrels.
            let preset = if i.is_multiple_of(2) {
                LibraryPreset::Wall
            } else {
                LibraryPreset::ExplosiveBarrel
            };
            level.items.push(LevelItem::Object {
                asset: Box::new(preset.asset()),
                position: [x, y],
                angle: std::f32::consts::FRAC_PI_2,
                cutouts: vec![],
            });
        }
    }
    let mut game = MultiplayerWorld::new(&level, &[Team::Blue, Team::Red]).unwrap();
    let mut encoders = [Encoder::default(), Encoder::default()];
    let mut decoder = Decoder::default();
    let mut replica: Option<World> = None;
    let (mut tick, mut snapshot, mut decode, mut apply, mut size) =
        (vec![], vec![], vec![], vec![], vec![]);
    for t in 0..120 * 20 {
        let inputs: Vec<_> = (0..2)
            .map(|i| PlayerInput {
                aim: Vec2::new(0.0, ((t as f32 * 0.01) + i as f32).sin() * 6.0),
                fire: t % 12 == 0,
                ..Default::default()
            })
            .collect();
        let started = Instant::now();
        game.step(&inputs);
        tick.push(started.elapsed().as_secs_f64() * 1e3);
        if t % 4 != 0 {
            continue;
        }
        // Mirrors the host: shared records, one delta per peer.
        let started = Instant::now();
        game.prepare_snapshot();
        let mut shared = Frame::capture(game.for_player(0).unwrap());
        let records = Records::capture(&std::mem::take(&mut shared.bodies));
        let mut wire = vec![];
        for (i, encoder) in encoders.iter_mut().enumerate() {
            let mut frame = Frame::for_player(game.for_player(i).unwrap(), &shared);
            replication::strip_diagnostics(&mut frame);
            wire.push(codec::encode(&encoder.encode(frame, &records)).unwrap());
        }
        snapshot.push(started.elapsed().as_secs_f64() * 1e3);
        size.push(wire[0].len() as f64 / 1024.0);
        let started = Instant::now();
        let (delta, _) = codec::decode(&wire[0]).unwrap().unwrap();
        let frame = decoder.decode(delta).unwrap();
        decode.push(started.elapsed().as_secs_f64() * 1e3);
        let started = Instant::now();
        match &mut replica {
            None => replica = Some(World::from_replica(Map::capture(game.world()), frame)),
            Some(r) => r.apply_replica(frame),
        }
        apply.push(started.elapsed().as_secs_f64() * 1e3);
    }
    println!("bodies at end: {}", game.world().bodies().count());
    report("host tick ms", tick);
    report("host snapshot ms (2 peers)", snapshot);
    report("client decode ms", decode);
    report("client apply incl. sight ms", apply);
    report("snapshot KiB per peer", size);
}
