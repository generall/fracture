//! Frozen bot workers shared by the headless harness and native spectator.
use super::{Config, InputPlan, Map, Observation, Telemetry};
use crate::multiplayer::PlayerInput;
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{self, BufRead, BufReader, BufWriter, Write},
    process::{Child, ChildStdin, Command, Stdio},
    time::Duration,
};
#[derive(Serialize, Deserialize)]
pub struct Decision {
    pub input: PlayerInput,
    #[serde(default)]
    pub aim_path: Vec<crate::Vec2>,
    pub telemetry: Telemetry,
    pub cpu_ms: f64,
}
impl Decision {
    pub fn plan(&self) -> InputPlan {
        InputPlan {
            input: self.input,
            aim_path: self.aim_path.clone(),
        }
    }
}
pub struct Worker {
    child: Child,
    input: BufWriter<ChildStdin>,
    output: std::sync::mpsc::Receiver<io::Result<String>>,
    pub config: Config,
    raw_config: serde_json::Value,
}
impl Worker {
    pub fn new(binary: &str, config: &str, map: Map) -> Result<Self, Box<dyn std::error::Error>> {
        let raw_config: serde_json::Value = serde_json::from_reader(File::open(config)?)?;
        let config: Config = serde_json::from_value(raw_config.clone())?;
        config.validate()?;
        let mut child = Command::new(binary)
            .arg("--worker")
            .env("FRACTURE_SILENT", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let mut input = BufWriter::new(child.stdin.take().unwrap());
        serde_json::to_writer(
            &mut input,
            &serde_json::json!({"config":raw_config,"map":map}),
        )?;
        writeln!(input)?;
        input.flush()?;
        let stdout = child.stdout.take().unwrap();
        let (sender, output) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            input,
            output,
            config,
            raw_config,
        })
    }
    pub fn decide(&mut self, o: &Observation) -> Result<Decision, Box<dyn std::error::Error>> {
        serde_json::to_writer(&mut self.input, o)?;
        writeln!(self.input)?;
        self.input.flush()?;
        let line = self.output.recv_timeout(Duration::from_secs(5))??;
        if line.is_empty() {
            return Err("bot exited before replying".into());
        }
        let d: Decision = serde_json::from_str(&line)?;
        if !d.plan().valid() {
            return Err("bot returned invalid human input".into());
        }
        Ok(d)
    }
    pub fn reset(&mut self, map: Map) -> Result<(), Box<dyn std::error::Error>> {
        write!(self.input, "RESET ")?;
        serde_json::to_writer(
            &mut self.input,
            &serde_json::json!({"config":self.raw_config,"map":map}),
        )?;
        writeln!(self.input)?;
        self.input.flush()?;
        Ok(())
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
