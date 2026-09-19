//! Portable experiment recipes and recorded visual checkpoints. Worlds are
//! reconstructed from their recipe, avoiding engine-specific physics snapshots.
use crate::{Ammunition, ObjectAsset};
use serde::{Deserialize, Serialize};

mod presets;
mod recording;
mod runner;
mod sweep;
pub use recording::{Frame, Geometry, Metrics, PoseRecord, Recording, VisualState};
pub use runner::Runner;
pub use sweep::Sweep;
#[cfg(test)]
mod tests;

pub const ENGINE_REVISION: &str = "experiment-terrain-bridges-13";
pub const MAX_VARIANTS: usize = 10;
pub const MAX_EXPERIMENT_BYTES: usize = 8 * 1024 * 1024;
pub const FRAME_TICKS: u32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImpactKind {
    Shell,
    Tank,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Impact {
    pub kind: ImpactKind,
    #[serde(default)]
    pub ammunition: Ammunition,
    pub shell_mass: f32,
    pub shell_speed: f32,
    pub tank_mass: f32,
    pub tank_speed: f32,
    /// Tank approach direction in degrees: 0 travels right; +90 travels down.
    #[serde(default)]
    pub angle_degrees: f32,
    /// Shell target in local object coordinates (object at (3,0)). For tank
    /// trials, aim[1] is the offset perpendicular to the approach direction.
    pub aim: [f32; 2],
    /// A rigid backstop 15 cm behind the object's initial right edge.
    #[serde(default, skip_serializing_if = "is_false")]
    pub backstop: bool,
}
fn is_false(value: &bool) -> bool {
    !value
}
impl Default for Impact {
    fn default() -> Self {
        Self {
            kind: ImpactKind::Shell,
            ammunition: Ammunition::HighExplosive,
            shell_mass: 6.0,
            shell_speed: 220.0,
            tank_mass: 4500.0,
            tank_speed: 4.6,
            angle_degrees: 0.0,
            aim: [0.0; 2],
            backstop: false,
        }
    }
}

impl Impact {
    pub fn select_ammunition(&mut self, ammunition: Ammunition) {
        self.ammunition = ammunition;
        (self.shell_mass, self.shell_speed) = ammunition.defaults();
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Variant {
    pub name: String,
    pub object: ObjectAsset,
    pub impact: Impact,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Experiment {
    pub version: u32,
    pub engine: String,
    pub name: String,
    pub variants: Vec<Variant>,
    /// When enabled, every case uses the same loading recipe.
    #[serde(default)]
    pub shared_impact: bool,
    pub duration_ticks: u32,
    pub cursor_tick: u32,
    pub favorite: Option<usize>,
    /// Exact visual state being reviewed when saved; replay uses the recipe.
    pub review: Vec<VisualState>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Archive {
    pub experiments: Vec<Experiment>,
}
impl Archive {
    pub fn from_json(json: &str) -> Result<Self, String> {
        if json.len() > MAX_EXPERIMENT_BYTES {
            return Err("Saved library exceeds 8 MiB.".into());
        }
        let archive: Self = serde_json::from_str(json).map_err(|e| e.to_string())?;
        if archive.experiments.len() > 12 {
            return Err("Library holds at most 12 experiments. Export older trials.".into());
        }
        for e in &archive.experiments {
            e.validate()?;
        }
        Ok(archive)
    }
    pub fn to_json(&self) -> Result<String, String> {
        for e in &self.experiments {
            e.validate()?;
        }
        let json = serde_json::to_string(self).map_err(|e| e.to_string())?;
        if self.experiments.len() > 12 || json.len() > MAX_EXPERIMENT_BYTES {
            return Err("Library is full. Export the experiment to a file.".into());
        }
        Ok(json)
    }
}

impl Experiment {
    pub fn invalidate(&mut self) {
        self.cursor_tick = 0;
        self.favorite = None;
        self.review.clear();
        self.engine = ENGINE_REVISION.into();
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("Unsupported experiment version.".into());
        }
        if !(1..=MAX_VARIANTS).contains(&self.variants.len())
            || !(120..=1200).contains(&self.duration_ticks)
            || self.cursor_tick > self.duration_ticks
        {
            return Err("Use 1–10 variants and a duration of 1–10 seconds.".into());
        }
        if self.favorite.is_some_and(|i| i >= self.variants.len()) {
            return Err("Selected variant does not exist.".into());
        }
        if self.shared_impact
            && self
                .variants
                .iter()
                .any(|v| v.impact != self.variants[0].impact)
        {
            return Err("Shared test settings must match across every case.".into());
        }
        if self.name.trim().is_empty()
            || self.name.chars().count() > 48
            || self.name.chars().any(char::is_control)
            || self.engine.len() > 80
        {
            return Err("Experiment name must contain 1–48 printable characters.".into());
        }
        for (i, v) in self.variants.iter().enumerate() {
            if v.name.trim().is_empty()
                || v.name.chars().count() > 32
                || v.name.chars().any(char::is_control)
            {
                return Err(format!(
                    "Variant {} needs a printable name of at most 32 characters.",
                    i + 1
                ));
            }
            v.object
                .mesh()
                .map_err(|e| format!("Variant {}: {e}", i + 1))?;
            let (mass_min, mass_max) = v.impact.ammunition.mass_range();
            let (speed_min, speed_max) = v.impact.ammunition.speed_range();
            for (name, value, min, max) in [
                ("Projectile mass", v.impact.shell_mass, mass_min, mass_max),
                (
                    "Projectile speed",
                    v.impact.shell_speed,
                    speed_min,
                    speed_max,
                ),
                ("Tank mass", v.impact.tank_mass, 500.0, 30_000.0),
                ("Tank speed", v.impact.tank_speed, 0.1, 12.0),
            ] {
                if !value.is_finite() || !(min..=max).contains(&value) {
                    return Err(format!(
                        "Variant {}: {name} must be between {min} and {max}.",
                        i + 1
                    ));
                }
            }
            if !v.impact.angle_degrees.is_finite() || v.impact.angle_degrees.abs() > 180.0 {
                return Err("Tank approach angle must be between -180 and 180 degrees.".into());
            }
            if v.impact.aim.iter().any(|v| !v.is_finite())
                || v.impact.aim[0].abs() > 4.5
                || v.impact.aim[1].abs() > 5.5
            {
                return Err("Impact point must stay inside workshop bounds.".into());
            }
        }
        if !self.review.is_empty() && self.review.len() != self.variants.len() {
            return Err("Saved views must match the variant count.".into());
        }
        for state in &self.review {
            state.validate()?;
            if state.frame.tick != self.cursor_tick {
                return Err("Saved views must use the same selected time.".into());
            }
        }
        Ok(())
    }
    pub fn to_json(&self) -> Result<String, String> {
        self.validate()?;
        let json = serde_json::to_string(self).map_err(|e| e.to_string())?;
        if json.len() > MAX_EXPERIMENT_BYTES {
            return Err("Experiment exceeds the 8 MiB file limit.".into());
        }
        Ok(json)
    }
    pub fn from_json(json: &str) -> Result<Self, String> {
        if json.len() > MAX_EXPERIMENT_BYTES {
            return Err("Experiment exceeds the 8 MiB file limit.".into());
        }
        let value: Self =
            serde_json::from_str(json).map_err(|e| format!("Invalid experiment: {e}"))?;
        value.validate()?;
        Ok(value)
    }
}
