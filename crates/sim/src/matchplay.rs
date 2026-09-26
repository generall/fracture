//! Match setup and results, independent of rendering and local input.
use crate::{
    Event, TICK_DT, World,
    level::{EnemyActivity, Level, LevelItem, Team},
};

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct MatchConfig {
    pub team: Team,
    pub time_limit_seconds: Option<u32>,
    pub opponent: EnemyActivity,
}
impl Default for MatchConfig {
    fn default() -> Self {
        Self {
            team: Team::Blue,
            time_limit_seconds: Some(600),
            opponent: EnemyActivity::Conquer,
        }
    }
}
impl MatchConfig {
    pub fn create_world(self, level: &Level) -> Result<World, String> {
        if self
            .time_limit_seconds
            .is_some_and(|seconds| seconds == 0 || seconds > 3600)
        {
            return Err(
                "Match length must be between 1 second and 60 minutes, or unlimited.".into(),
            );
        }
        if !matches!(
            self.opponent,
            EnemyActivity::Conquer | EnemyActivity::Attack
        ) {
            return Err("Choose an opponent that fights or captures the flag.".into());
        }
        for team in [Team::Blue, Team::Red] {
            if !level
                .items
                .iter()
                .any(|item| matches!(item, LevelItem::Spawn { team: t, .. } if *t == team))
            {
                return Err(format!(
                    "Add a {} spawn to this map before starting.",
                    team.label()
                ));
            }
            if !level
                .items
                .iter()
                .any(|item| matches!(item, LevelItem::Flag { team: t, .. } if *t == team))
            {
                return Err(format!(
                    "Add a {} flag to this map before starting.",
                    team.label()
                ));
            }
        }
        let mut recipe = level.clone();
        recipe.enemy_activity = self.opponent;
        World::from_level(&recipe, self.team)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Outcome {
    Captured(Team),
    TimeLimit,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct MatchProgress {
    pub config: MatchConfig,
    pub elapsed_ticks: u64,
    pub tanks_lost: [u32; 2],
    pub outcome: Option<Outcome>,
}
impl MatchProgress {
    pub fn new(config: MatchConfig) -> Self {
        Self {
            config,
            elapsed_ticks: 0,
            tanks_lost: [0; 2],
            outcome: None,
        }
    }
    pub fn record_tick(&mut self, winner: Option<Team>, events: &[Event]) {
        if self.outcome.is_some() {
            return;
        }
        self.elapsed_ticks += 1;
        for event in events {
            if let Event::TankDestroyed { team, .. } = event {
                self.tanks_lost[usize::from(*team == Team::Red)] += 1;
            }
        }
        self.outcome = winner.map(Outcome::Captured);
        if self.outcome.is_none()
            && self
                .config
                .time_limit_seconds
                .is_some_and(|seconds| self.elapsed_ticks >= u64::from(seconds) * 120)
        {
            self.outcome = Some(Outcome::TimeLimit);
        }
    }
    pub fn elapsed_seconds(&self) -> f32 {
        self.elapsed_ticks as f32 * TICK_DT
    }
    pub fn remaining_seconds(&self) -> Option<f32> {
        self.config
            .time_limit_seconds
            .map(|s| (s as f32 - self.elapsed_seconds()).max(0.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn match_requires_both_teams_and_flags_and_does_not_modify_the_map() {
        let level = Level::default();
        for team in [Team::Blue, Team::Red] {
            let world = MatchConfig {
                team,
                ..Default::default()
            }
            .create_world(&level)
            .unwrap();
            assert_eq!(world.team(), team);
            assert_eq!(world.tanks().count(), 2);
        }
        assert_eq!(level.enemy_activity, EnemyActivity::Static);
        for index in 0..4 {
            let mut invalid = level.clone();
            invalid.items.remove(index);
            assert!(MatchConfig::default().create_world(&invalid).is_err());
        }
        let mut invalid = level;
        if let LevelItem::Spawn { position, .. } = &mut invalid.items[0] {
            *position = [1000.0, 0.0];
        }
        assert!(MatchConfig::default().create_world(&invalid).is_err());
    }
    #[test]
    fn time_limit_is_fixed_tick_and_capture_wins_on_the_last_tick() {
        let config = MatchConfig {
            time_limit_seconds: Some(1),
            ..Default::default()
        };
        let mut progress = MatchProgress::new(config);
        for _ in 0..119 {
            progress.record_tick(None, &[]);
        }
        assert_eq!(progress.outcome, None);
        let mut captured = progress.clone();
        progress.record_tick(None, &[]);
        assert_eq!(progress.outcome, Some(Outcome::TimeLimit));
        progress.record_tick(Some(Team::Blue), &[]);
        assert_eq!(progress.outcome, Some(Outcome::TimeLimit));
        assert_eq!(progress.elapsed_ticks, 120);
        captured.record_tick(Some(Team::Red), &[]);
        assert_eq!(captured.outcome, Some(Outcome::Captured(Team::Red)));
        let rematch = MatchProgress::new(config);
        assert_eq!(rematch.elapsed_ticks, 0);
        assert_eq!(rematch.outcome, None);
    }
    #[test]
    fn unlimited_matches_end_only_on_capture() {
        let mut progress = MatchProgress::new(MatchConfig {
            time_limit_seconds: None,
            ..Default::default()
        });
        for _ in 0..1000 {
            progress.record_tick(None, &[]);
        }
        assert_eq!(progress.remaining_seconds(), None);
        assert_eq!(progress.outcome, None);
        progress.record_tick(Some(Team::Blue), &[]);
        assert_eq!(progress.outcome, Some(Outcome::Captured(Team::Blue)));
    }
}
