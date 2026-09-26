use super::*;
use crate::level::{StationKind, StationSettings, Team};

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct StationView {
    pub kind: StationKind,
    pub position: Vector,
    pub settings: StationSettings,
    pub object: usize,
    pub owner: Option<Team>,
    pub capturing: Option<Team>,
    /// Fraction of the current neutralization/capture stage, from zero to one.
    pub progress: f32,
    pub contested: bool,
}

pub(super) struct Station {
    view: StationView,
    elapsed: f32,
}
impl Station {
    pub(super) fn new(
        kind: StationKind,
        position: Vector,
        settings: StationSettings,
        object: usize,
    ) -> Self {
        Self {
            view: StationView {
                kind,
                position,
                settings,
                object,
                owner: None,
                capturing: None,
                progress: 0.0,
                contested: false,
            },
            elapsed: 0.0,
        }
    }
    fn capture(&mut self, blue: bool, red: bool, dt: f32) {
        self.view.contested = blue && red;
        if self.view.contested {
            return;
        }
        let present = if blue {
            Some(Team::Blue)
        } else if red {
            Some(Team::Red)
        } else {
            None
        };
        if present.is_none() || present == self.view.owner {
            self.view.capturing = None;
            self.elapsed = 0.0;
        } else {
            if self.view.capturing != present {
                self.elapsed = 0.0;
            }
            self.view.capturing = present;
            self.elapsed += dt;
            if self.elapsed + 1e-5 >= self.view.settings.capture_time {
                if self.view.owner.is_some() {
                    self.view.owner = None;
                } else {
                    self.view.owner = present;
                    self.view.capturing = None;
                }
                self.elapsed = 0.0;
            }
        }
        self.view.progress = (self.elapsed / self.view.settings.capture_time).min(1.0);
    }
}

impl World {
    pub fn stations(&self) -> impl Iterator<Item = StationView> + '_ {
        self.stations
            .iter()
            .map(|s| s.view)
            .chain(self.replica.iter().flat_map(|f| f.stations.iter().copied()))
    }
    pub fn player_in_radar_zone(&self) -> bool {
        if let Some(frame) = &self.replica {
            return frame.radar;
        }
        self.radar_covers(self.team(), self.launcher_position())
    }
    pub(super) fn radar_covers(&self, team: Team, position: Vector) -> bool {
        self.stations().any(|s| {
            s.kind == StationKind::Radar
                && s.owner == Some(team)
                && s.position.distance_squared(position) <= s.settings.radar_radius.powi(2)
        })
    }
    pub(super) fn update_stations(&mut self) {
        if self.stations.is_empty() {
            return;
        }
        let tanks: Vec<_> = self
            .tanks
            .iter()
            .enumerate()
            .filter(|(_, t)| self.tank_is_alive(t.id))
            .map(|(index, t)| (index, t.team, self.piece_pose(t.id).translation))
            .collect();
        for station in &mut self.stations {
            let nearby: Vec<_> = tanks
                .iter()
                .filter(|(_, _, p)| {
                    p.distance_squared(station.view.position)
                        <= station.view.settings.capture_radius.powi(2)
                })
                .collect();
            station.capture(
                nearby.iter().any(|(_, team, _)| *team == Team::Blue),
                nearby.iter().any(|(_, team, _)| *team == Team::Red),
                TICK_DT,
            );
            if station.view.kind == StationKind::Repair {
                for &&(index, team, _) in &nearby {
                    if station.view.owner == Some(team) {
                        self.tanks[index].health = (self.tanks[index].health
                            + station.view.settings.repair_rate * TICK_DT)
                            .min(100.0);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::{Level, LevelItem, rectangle};

    fn world(kind: StationKind) -> World {
        let mut level = Level::default();
        level.items.push(LevelItem::Station {
            station: kind,
            position: [0.0, 0.0],
            settings: StationSettings {
                capture_time: 1.0,
                ..StationSettings::default()
            },
        });
        World::from_level(&level, Team::Blue).unwrap()
    }
    fn move_tank(world: &mut World, team: Team, x: f32) {
        let id = world.tanks.iter().find(|t| t.team == team).unwrap().id;
        world.physics.bodies[world.pieces[id].body].set_translation(Vector::new(x, 0.0), true);
    }
    fn advance(world: &mut World, ticks: usize) {
        for _ in 0..ticks {
            world.update_stations();
        }
    }

    #[test]
    fn stations_stay_neutral_when_abandoned_after_neutralizing() {
        let mut station = Station::new(
            StationKind::Repair,
            Vector::ZERO,
            StationSettings::default(),
            0,
        );
        station.capture(true, false, 5.0);
        assert_eq!(station.view.owner, Some(Team::Blue));
        station.capture(false, true, 5.0);
        assert_eq!(station.view.owner, None);
        station.capture(false, true, 2.0);
        station.capture(false, false, 1.0);
        assert_eq!(station.view.owner, None);
        assert_eq!(station.view.progress, 0.0);
        station.capture(false, true, 4.0);
        assert_eq!(station.view.owner, None);
        station.capture(false, true, 1.0);
        assert_eq!(station.view.owner, Some(Team::Red));
    }

    #[test]
    fn stations_capture_pause_reset_and_change_owner() {
        let mut w = world(StationKind::Repair);
        assert_eq!(w.stations().next().unwrap().owner, None);
        move_tank(&mut w, Team::Blue, -3.0);
        advance(&mut w, 60);
        let halfway = w.stations().next().unwrap();
        assert_eq!(halfway.owner, None);
        assert!((halfway.progress - 0.5).abs() < 0.001);
        move_tank(&mut w, Team::Red, 3.0);
        advance(&mut w, 180);
        let contested = w.stations().next().unwrap();
        assert!(contested.contested);
        assert_eq!(contested.progress, halfway.progress);
        move_tank(&mut w, Team::Red, 18.0);
        advance(&mut w, 60);
        assert_eq!(w.stations().next().unwrap().owner, Some(Team::Blue));
        move_tank(&mut w, Team::Blue, -18.0);
        advance(&mut w, 120);
        assert_eq!(w.stations().next().unwrap().owner, Some(Team::Blue));
        move_tank(&mut w, Team::Red, 3.0);
        advance(&mut w, 60);
        move_tank(&mut w, Team::Red, 18.0);
        advance(&mut w, 1);
        assert_eq!(w.stations().next().unwrap().progress, 0.0);
        move_tank(&mut w, Team::Red, 3.0);
        advance(&mut w, 119);
        assert_eq!(w.stations().next().unwrap().owner, Some(Team::Blue));
        advance(&mut w, 1);
        assert_eq!(w.stations().next().unwrap().owner, None);
        assert_eq!(w.stations().next().unwrap().capturing, Some(Team::Red));
        advance(&mut w, 119);
        assert_eq!(w.stations().next().unwrap().owner, None);
        advance(&mut w, 1);
        assert_eq!(w.stations().next().unwrap().owner, Some(Team::Red));
    }

    #[test]
    fn stations_repair_only_living_allies_in_range_at_configured_rate() {
        let mut w = world(StationKind::Repair);
        move_tank(&mut w, Team::Blue, -3.0);
        advance(&mut w, 120);
        w.stations[0].view.settings.repair_rate = 24.0;
        for tank in &mut w.tanks {
            tank.health = 50.0;
        }
        move_tank(&mut w, Team::Red, 3.0);
        advance(&mut w, 120);
        assert!((w.tanks[0].health - 74.0).abs() < 0.01);
        assert_eq!(w.tanks[1].health, 50.0);
        advance(&mut w, 240);
        assert_eq!(w.tanks[0].health, 100.0);
        w.tanks[0].health = 50.0;
        move_tank(&mut w, Team::Blue, -18.0);
        advance(&mut w, 60);
        assert_eq!(w.tanks[0].health, 50.0);
        w.tanks[1].health = 0.0;
        advance(&mut w, 120);
        assert_eq!(w.tanks[1].health, 0.0);
        assert_eq!(w.stations().next().unwrap().owner, Some(Team::Blue));
        assert_eq!(w.stations().next().unwrap().progress, 0.0);
    }

    #[test]
    fn player_radar_zone_requires_friendly_ownership_and_configured_coverage() {
        let mut w = world(StationKind::Radar);
        w.stations[0].view.settings.radar_radius = 10.0;
        move_tank(&mut w, Team::Blue, -3.0);
        assert!(!w.player_in_radar_zone());
        w.stations[0].view.owner = Some(Team::Blue);
        assert!(w.player_in_radar_zone());
        move_tank(&mut w, Team::Blue, -11.0);
        assert!(!w.player_in_radar_zone());
        move_tank(&mut w, Team::Blue, -3.0);
        w.stations[0].view.owner = Some(Team::Red);
        assert!(!w.player_in_radar_zone());
    }

    #[test]
    fn stations_radar_reveals_through_cover_and_transfers_with_ownership() {
        let mut w = world(StationKind::Radar);
        w.stations[0].view.settings.radar_radius = 10.0;
        move_tank(&mut w, Team::Blue, -3.0);
        let target = Vector::new(8.0, 0.0); // Behind the solid station building.
        assert!(!w.player_vision(std::f32::consts::PI).contains(target));
        advance(&mut w, 120);
        assert!(w.player_vision(std::f32::consts::PI).contains(target));
        assert!(
            !w.player_vision(std::f32::consts::PI)
                .contains(Vector::new(11.0, 0.0))
        );
        w.tanks[0].health = 0.0;
        assert!(
            w.player_vision(0.0).contains(target),
            "radar survives its owner's death"
        );
        move_tank(&mut w, Team::Red, 3.0);
        advance(&mut w, 120);
        assert!(!w.player_vision(0.0).contains(target));
        assert_eq!(w.stations().next().unwrap().owner, None);
        assert!(!w.radar_covers(Team::Blue, target));
        assert!(!w.radar_covers(Team::Red, target));
        advance(&mut w, 120);
        assert!(w.radar_covers(Team::Red, target));
        assert!(!w.radar_covers(Team::Blue, target));
    }

    #[test]
    fn stations_round_trip_settings_and_carve_overlapping_walls() {
        let mut level = Level::default();
        level.items.push(LevelItem::Object {
            asset: Box::new(crate::level::wall_asset(Vector::splat(4.0))),
            position: [0.0, 0.0],
            angle: 0.0,
            cutouts: vec![],
        });
        level.items.push(LevelItem::Station {
            station: StationKind::Repair,
            position: [0.0, 0.0],
            settings: StationSettings::default(),
        });
        assert_eq!(Level::from_json(&level.to_json().unwrap()).unwrap(), level);
        let mesh = level.resolved_meshes().unwrap();
        assert!((mesh[4].as_ref().unwrap().area - 12.0).abs() < 0.01);
        assert_eq!(mesh[5].as_ref().unwrap().area, 4.0);
        World::from_level(&level, Team::Blue).unwrap();
        if let LevelItem::Station { settings, .. } = &mut level.items[5] {
            settings.capture_time = -1.0;
        }
        assert!(level.validate().is_err());
        level.items.pop();
        level.items.push(LevelItem::Station {
            station: StationKind::Radar,
            position: [0.0, 0.0],
            settings: StationSettings::default(),
        });
        level.items.push(LevelItem::Solid {
            outline: rectangle(Vector::ONE),
            position: [0.0, 0.0],
            angle: 0.0,
            cutouts: vec![],
        });
        assert!(
            level.resolved_meshes().is_err(),
            "station footprints cannot be partly removed"
        );
    }

    #[test]
    fn stations_capture_runs_in_simulation_step() {
        let mut w = world(StationKind::Repair);
        move_tank(&mut w, Team::Blue, -3.0);
        for _ in 0..121 {
            w.step(&[]);
        }
        assert_eq!(w.stations().next().unwrap().owner, Some(Team::Blue));
    }
}
