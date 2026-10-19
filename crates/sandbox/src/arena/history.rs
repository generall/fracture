//! Bounded four-minute observer recordings with five-second seek checkpoints.
use sim::{World, ai::Telemetry, replica};
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};
use tank_net::{
    codec,
    replication::{Decoder, Delta, Encoder},
};

#[derive(Clone)]
pub(super) struct Hud {
    pub tick: u32,
    pub telemetry: [Telemetry; sim::ai::ARENA_PLAYERS],
    pub kills: [u32; 2],
    pub captures: [u32; 2],
    pub stalled: [bool; sim::ai::ARENA_PLAYERS],
    pub vision: [Vec<sim::Vision>; 2],
}
pub(super) struct History {
    pub map: replica::Map,
    pub frames: Vec<(Delta, Hud)>,
    checkpoints: Vec<(usize, Decoder)>,
    encoder: Encoder,
    decoder: Decoder,
    stream: BufWriter<File>,
    inputs: BufWriter<File>,
    vision: BufWriter<File>,
    pub path: PathBuf,
}
impl History {
    pub fn new(
        directory: &Path,
        world: &World,
        names: &[String; 2],
        seed: u64,
    ) -> Result<Self, String> {
        Self::with_roster(directory, world, names, seed, &sim::ai::ARENA_TEAMS)
    }
    pub(super) fn with_roster(
        directory: &Path,
        world: &World,
        names: &[String; 2],
        seed: u64,
        teams: &[sim::level::Team],
    ) -> Result<Self, String> {
        std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let path = directory.join(format!("round-{stamp}"));
        let map = replica::Map::capture(world);
        std::fs::write(
            path.with_extension("meta.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
            "respawn_seconds":sim::RESPAWN_SECONDS,"players_per_team":if teams.len()==4 {Some(2)} else {None},"players":teams.len(),"teams":teams,"map":map,"versions":names,"seed":seed,"duration_seconds":sim::ai::ROUND_SECONDS,"snapshot_hz":10,
            "format":"tank_net::codec packets containing replication::Delta; observer only",
            "vision_format":".vision: codec packets (tick:u32, [Vec<(origin_mm:[i32;2],boundary_mm:Vec<[i32;2]>)>;2]); observer only"}))
            .unwrap(),
        )
        .map_err(|e| e.to_string())?;
        Ok(Self {
            map,
            frames: vec![],
            checkpoints: vec![],
            encoder: Encoder::default(),
            decoder: Decoder::default(),
            stream: BufWriter::new(
                File::create(path.with_extension("frames")).map_err(|e| e.to_string())?,
            ),
            vision: BufWriter::new(
                File::create(path.with_extension("vision")).map_err(|e| e.to_string())?,
            ),
            inputs: BufWriter::new(
                File::create(path.with_extension("inputs.jsonl")).map_err(|e| e.to_string())?,
            ),
            path,
        })
    }
    pub fn capture(&mut self, frame: replica::Frame, hud: Hud) -> Result<(), String> {
        let delta = self.encoder.encode_frame(frame);
        if self.frames.len().is_multiple_of(50) {
            self.checkpoints
                .push((self.frames.len(), self.decoder.clone()));
        }
        self.decoder.advance(delta.clone())?;
        self.stream
            .write_all(&codec::encode(&delta).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let point = |p: sim::Vec2| [(p.x * 1000.0).round() as i32, (p.y * 1000.0).round() as i32];
        let packed: [Vec<_>; 2] = std::array::from_fn(|side| {
            hud.vision[side]
                .iter()
                .map(|r| {
                    (
                        point(r.origin),
                        r.boundary.iter().copied().map(point).collect::<Vec<_>>(),
                    )
                })
                .collect()
        });
        self.vision
            .write_all(&codec::encode(&(hud.tick, packed)).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        if hud.tick.is_multiple_of(120) {
            self.stream.flush().map_err(|e| e.to_string())?;
            self.inputs.flush().map_err(|e| e.to_string())?;
            self.vision.flush().map_err(|e| e.to_string())?;
        }
        self.frames.push((delta, hud));
        Ok(())
    }
    pub fn input(&mut self, value: serde_json::Value) -> Result<(), String> {
        serde_json::to_writer(&mut self.inputs, &value).map_err(|e| e.to_string())?;
        writeln!(self.inputs).map_err(|e| e.to_string())
    }
    pub fn seek(&self, index: usize) -> Result<World, String> {
        let (start, decoder) = self
            .checkpoints
            .iter()
            .rev()
            .find(|(i, _)| *i <= index)
            .ok_or("No recorded frame")?;
        let mut decoder = decoder.clone();
        for (delta, _) in &self.frames[*start..index] {
            decoder.advance(delta.clone())?;
        }
        let frame = decoder.decode(self.frames[index].0.clone())?;
        Ok(World::from_replica(self.map.clone(), frame))
    }
    pub fn finish(&mut self) -> Result<(), String> {
        self.stream.flush().map_err(|e| e.to_string())?;
        self.inputs.flush().map_err(|e| e.to_string())?;
        self.vision.flush().map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recorded_team_vision_and_seek_preserve_observer_state() {
        let directory = std::env::temp_dir().join(format!(
            "fracture-arena-history-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let world =
            World::from_level(&sim::level::Level::default(), sim::level::Team::Blue).unwrap();
        let vision = world.ai_observation().regions;
        let mut history =
            History::new(&directory, &world, &["blue".into(), "red".into()], 42).unwrap();
        let hud = Hud {
            tick: 0,
            telemetry: Default::default(),
            kills: [0; 2],
            captures: [0; 2],
            stalled: [false; sim::ai::ARENA_PLAYERS],
            vision: [vision.clone(), vec![]],
        };
        history
            .capture(replica::Frame::capture(&world), hud)
            .unwrap();
        history.finish().unwrap();
        type Packed = [Vec<([i32; 2], Vec<[i32; 2]>)>; 2];
        let bytes = std::fs::read(history.path.with_extension("vision")).unwrap();
        let ((tick, packed), consumed) = codec::decode::<(u32, Packed)>(&bytes).unwrap().unwrap();
        assert_eq!(tick, 0);
        assert_eq!(consumed, bytes.len());
        assert_eq!(packed[0].len(), vision.len());
        assert!(packed[1].is_empty());
        assert_eq!(packed[0][0].1.len(), vision[0].boundary.len());
        assert_eq!(
            history.seek(0).unwrap().tanks().count(),
            world.tanks().count()
        );
        assert!(history.frames[0].1.vision[0] == vision);
        drop(history);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
