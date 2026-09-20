use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StationKind {
    Repair,
    Radar,
}
impl StationKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Repair => "REPAIR STATION",
            Self::Radar => "RADAR STATION",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StationSettings {
    pub capture_time: f32,
    pub capture_radius: f32,
    pub repair_rate: f32,
    pub radar_radius: f32,
}
impl Default for StationSettings {
    fn default() -> Self {
        Self {
            capture_time: 5.0,
            capture_radius: 5.0,
            repair_rate: 10.0,
            radar_radius: 20.0,
        }
    }
}
impl StationSettings {
    pub(super) fn validate(self) -> Result<(), String> {
        for (label, value, min, max) in [
            ("Capture time / seconds", self.capture_time, 0.1, 120.0),
            ("Capture radius / metres", self.capture_radius, 2.0, 100.0),
            ("Repair rate / HP per second", self.repair_rate, 0.0, 100.0),
            ("Radar radius / metres", self.radar_radius, 1.0, 200.0),
        ] {
            if !value.is_finite() || !(min..=max).contains(&value) {
                return Err(format!("{label} must be {min}–{max}."));
            }
        }
        Ok(())
    }
}
