//! Recreate the supplied classic tanks screenshot as an editable level.
//! Run: cargo run --locked -p sim --example classic_tanks
use sim::level::{EnemyActivity, Level, LevelItem, Team, rectangle, wall_asset};
use sim::{GrainMode, Vec2, World};

// Screenshot playfield: (48,48)..(1680,1008). Thirty-two pixels per metre
// preserves its proportions and leaves 3 m lanes for the 1.64 m wide tanks.
fn point(x: f32, y: f32) -> Vec2 {
    Vec2::new((x - 864.0) / 32.0, (y - 528.0) / 32.0)
}
fn brick(level: &mut Level, name: &str, x: f32, y: f32, w: f32, h: f32) {
    let mut asset = wall_asset(Vec2::new(w, h) / 32.0);
    asset.name = name.into();
    asset.color = Some([183, 87, 18]);
    asset.regenerate_grains(GrainMode::Bricks, [1.5, 0.75], 7);
    asset.fit_grain_budget();
    level.items.push(LevelItem::Object {
        cutouts: vec![],
        asset: Box::new(asset),
        position: point(x + w * 0.5, y + h * 0.5).to_array(),
        angle: 0.0,
    });
}
fn steel(level: &mut Level, x: f32, y: f32, w: f32, h: f32) {
    level.items.push(LevelItem::Solid {
        cutouts: vec![],
        outline: rectangle(Vec2::new(w, h) / 64.0),
        position: point(x + w * 0.5, y + h * 0.5).to_array(),
        angle: 0.0,
    });
}

fn main() -> Result<(), String> {
    let mut level = Level {
        name: "Classic Tanks / First Battlefield".into(),
        size: [55.0, 34.0],
        enemy_activity: EnemyActivity::Conquer,
        items: vec![],
        ..Level::default()
    };
    // Full fixed perimeter; corners meet without overlapping geometry.
    for (x, y, w, h) in [
        (0.0, 0.0, 1728.0, 48.0),
        (0.0, 1008.0, 1728.0, 48.0),
        (0.0, 48.0, 48.0, 960.0),
        (1680.0, 48.0, 48.0, 960.0),
    ] {
        steel(&mut level, x, y, w, h);
    }

    for (i, x) in [144.0, 336.0, 528.0, 720.0, 912.0, 1104.0, 1296.0, 1488.0]
        .into_iter()
        .enumerate()
    {
        let height = if i == 3 || i == 4 { 336.0 } else { 240.0 };
        brick(&mut level, "Upper brick column", x, 144.0, 96.0, height);
    }
    steel(&mut level, 816.0, 240.0, 96.0, 96.0);
    brick(&mut level, "Left cross wall", 240.0, 480.0, 384.0, 96.0);
    brick(&mut level, "Right cross wall", 1104.0, 480.0, 384.0, 96.0);
    steel(&mut level, 48.0, 528.0, 96.0, 48.0);
    steel(&mut level, 1584.0, 528.0, 96.0, 48.0);
    brick(&mut level, "Right steel cap", 1584.0, 504.0, 96.0, 24.0);

    // One connected H-shaped object, rather than three overlapping rectangles.
    let mut center = wall_asset(Vec2::new(9.0, 6.0));
    center.name = "Central H wall".into();
    center.color = Some([183, 87, 18]);
    center.outline = vec![
        [-4.5, -3.0],
        [-1.5, -3.0],
        [-1.5, -1.5],
        [1.5, -1.5],
        [1.5, -3.0],
        [4.5, -3.0],
        [4.5, 3.0],
        [1.5, 3.0],
        [1.5, 1.5],
        [-1.5, 1.5],
        [-1.5, 3.0],
        [-4.5, 3.0],
    ];
    center.regenerate_grains(GrainMode::Bricks, [1.5, 0.75], 7);
    center.fit_grain_budget();
    level.items.push(LevelItem::Object {
        cutouts: vec![],
        asset: Box::new(center),
        position: point(864.0, 672.0).to_array(),
        angle: 0.0,
    });

    for x in [144.0, 336.0, 528.0, 1104.0, 1296.0, 1488.0] {
        brick(&mut level, "Lower brick column", x, 672.0, 96.0, 240.0);
    }
    // The eagle's brick enclosure. A thinner section lets an approaching
    // Conquer tank attempt a breach while the large columns remain obstacles.
    let mut base = wall_asset(Vec2::new(6.0, 4.5));
    base.name = "Eagle base enclosure".into();
    base.color = Some([183, 87, 18]);
    base.thickness = 0.1;
    base.outline = vec![
        [-3.0, -2.25],
        [3.0, -2.25],
        [3.0, 2.25],
        [1.5, 2.25],
        [1.5, -0.75],
        [-1.5, -0.75],
        [-1.5, 2.25],
        [-3.0, 2.25],
    ];
    base.regenerate_grains(GrainMode::Bricks, [1.5, 0.75], 7);
    base.fit_grain_budget();
    level.items.push(LevelItem::Object {
        cutouts: vec![],
        asset: Box::new(base),
        position: point(864.0, 936.0).to_array(),
        angle: 0.0,
    });

    level.items.extend([
        LevelItem::Spawn {
            team: Team::Blue,
            position: point(672.0, 960.0).to_array(),
            angle: -std::f32::consts::FRAC_PI_2,
        },
        LevelItem::Spawn {
            team: Team::Red,
            position: point(192.0, 96.0).to_array(),
            angle: std::f32::consts::FRAC_PI_2,
        },
        LevelItem::Flag {
            team: Team::Blue,
            position: point(864.0, 960.0).to_array(),
        },
        // The original has no enemy flag; this supplies Fracture's win condition.
        LevelItem::Flag {
            team: Team::Red,
            position: point(864.0, 96.0).to_array(),
        },
    ]);
    let json = level.to_json()?;
    for team in [Team::Blue, Team::Red] {
        let world = World::from_level(&level, team)?;
        println!(
            "{} spawn valid; {} material cells",
            team.label(),
            world.stats().cells
        );
    }
    let mut playtest = World::from_level(&level, Team::Blue)?;
    let enemy_id = playtest.tanks().find(|t| t.team == Team::Red).unwrap().id;
    let enemy_start = playtest
        .bodies()
        .find(|b| b.id == enemy_id)
        .unwrap()
        .position;
    for _ in 0..600 {
        playtest.step(&[]);
    }
    let travelled = playtest
        .bodies()
        .find(|b| b.id == enemy_id)
        .unwrap()
        .position
        .distance(enemy_start);
    if travelled < 1.0 {
        return Err("Enemy cannot leave its starting corridor.".into());
    }
    println!("Conquer playtest: enemy moved {travelled:.1} m in five seconds");
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "levels/classic-tanks.json".into());
    if let Some(parent) = std::path::Path::new(&path).parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    println!("Saved {} items to {path}", level.items.len());
    Ok(())
}
