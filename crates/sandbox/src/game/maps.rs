//! The game's two bundled battlefields. Sandbox examples are kept separate.
use sim::level::Level;

pub(super) const MAPS: [(&str, &str, &str); 2] = [
    (
        "RIVER VALLEY",
        "River Valley",
        include_str!("../../../../levels/dota-level.json"),
    ),
    (
        "CLASSIC TANKS 2",
        "Classic Tanks 2",
        include_str!("../../../../levels/classic-tanks-2.json"),
    ),
];

pub(super) fn load(index: usize) -> Level {
    let (_, name, json) = MAPS[index];
    let mut map = Level::from_json(json).expect("bundled game map is valid");
    map.name = name.into();
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::{level::Team, matchplay::MatchConfig};

    #[test]
    fn bundled_maps_preserve_source_content_and_start_for_both_teams() {
        for (index, (_, name, json)) in MAPS.iter().enumerate() {
            let map = load(index);
            let mut source = Level::from_json(json).unwrap();
            source.name = (*name).into();
            assert_eq!(map.to_json().unwrap(), source.to_json().unwrap());
            for team in [Team::Blue, Team::Red] {
                MatchConfig {
                    team,
                    ..MatchConfig::default()
                }
                .create_world(&map)
                .unwrap_or_else(|e| panic!("{name}, {team:?}: {e}"));
            }
        }
    }
}
