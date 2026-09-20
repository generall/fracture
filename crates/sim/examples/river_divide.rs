//! Build the large battlefield from the user's hand-drawn, Dota-inspired map.
//! cargo run --release --locked -p sim --example river_divide
use sim::level::{
    EnemyActivity, Level, LevelItem, StationKind, StationSettings, Team, fence_asset, rectangle,
    wall_asset,
};
use sim::{GrainMode, LibraryPreset, ObjectAsset, SpecialWeapon, TerrainKind, Vec2, World};

// Coordinate system of docs/maps/river-valley-concept.svg.
fn point(x: f32, y: f32) -> [f32; 2] {
    [(x - 510.) * SCALE, (y - 597.) * SCALE]
}
fn object(level: &mut Level, asset: ObjectAsset, x: f32, y: f32, angle: f32) {
    level.items.push(LevelItem::Object {
        asset: Box::new(asset),
        position: point(x, y),
        angle,
        cutouts: vec![],
    });
}
fn house(level: &mut Level, name: &str, x: f32, y: f32, w: f32, h: f32, angle: f32) {
    let mut asset = wall_asset(Vec2::new(w, h));
    asset.name = name.into();
    asset.color = Some([176, 122, 69]);
    asset.regenerate_grains(GrainMode::Bricks, [1.5, 1.0], 17);
    asset.fit_grain_budget();
    // A real building footprint: four walls, a hollow interior and opposed
    // 3.5 m entrances. Both halves attach to small foundation corners.
    let cut = |width: f32, height: f32| {
        rectangle(Vec2::new(width, height) * 0.5)
            .try_into()
            .unwrap()
    };
    level.items.push(LevelItem::Object {
        asset: Box::new(asset),
        position: point(x, y),
        angle,
        cutouts: vec![cut(w - 2., h - 2.), cut(3.5, h + 1.)],
    });
    for dx in [-w * 0.5 + 0.5, w * 0.5 - 0.5] {
        let offset = Vec2::from_angle(angle).rotate(Vec2::new(dx, -h * 0.5 + 0.5)) / SCALE;
        level.items.push(LevelItem::Solid {
            outline: rectangle(Vec2::splat(0.6)),
            position: point(x + offset.x, y + offset.y),
            angle,
            cutouts: vec![],
        });
    }
}
fn rock(level: &mut Level, name: &str, x: f32, y: f32, size: [f32; 2], angle: f32) {
    let mut asset = LibraryPreset::Rock.asset();
    asset.name = name.into();
    for p in &mut asset.outline {
        p[0] *= size[0] / 2.71;
        p[1] *= size[1] / 2.47;
    }
    // Broad formations need coarse grains to leave physics budget for combat.
    asset.regenerate_grains(GrainMode::Stones, [3.0, 3.0], 31);
    asset.fit_grain_budget();
    asset.color = Some([111, 105, 94]);
    object(level, asset, x, y, angle);
}

fn main() -> Result<(), String> {
    let mut level = Level {
        name: "River Divide / River Valley".into(),
        size: [160., 160.],
        enemy_activity: EnemyActivity::Conquer,
        items: vec![],
        ..Level::default()
    };
    let mut land = Landscape::new();
    // This layout uses the approved concept's drawing coordinates. Curved
    // Paths are rasterized directly into the half-metre terrain mask.
    let west = [
        [157., 945.],
        [172., 790.],
        [190., 651.],
        [219., 535.],
        [219., 472.],
        [235., 397.],
        [296., 377.],
        [383., 335.],
        [490., 280.],
        [580., 264.],
        [654., 239.],
        [775., 222.],
        [879., 234.],
    ];
    let middle = [
        [157., 945.],
        [270., 855.],
        [358., 763.],
        [435., 674.],
        [414., 651.],
        [430., 625.],
        [455., 625.],
        [550., 625.],
        [550., 590.],
        [565., 550.],
        [675., 550.],
        [717., 424.],
        [795., 311.],
        [879., 234.],
    ];
    let east = [
        [157., 945.],
        [300., 950.],
        [400., 950.],
        [481., 929.],
        [620., 875.],
        [717., 849.],
        [766., 777.],
        [839., 759.],
        [898., 671.],
        [895., 615.],
        [900., 510.],
        [894., 432.],
        [898., 315.],
        [879., 234.],
    ];
    for route in [&west[..], &middle[..], &east[..]] {
        land.path(TerrainKind::Road, route, 8.);
    }
    for route in [
        &[
            [198., 603.],
            [230., 655.],
            [258., 722.],
            [310., 758.],
            [372., 742.],
        ][..],
        &[
            [432., 295.],
            [455., 354.],
            [474., 410.],
            [490., 468.],
            [551., 486.],
            [612., 500.],
            [665., 487.],
        ][..],
        &[
            [372., 742.],
            [450., 754.],
            [508., 785.],
            [595., 843.],
            [675., 861.],
        ][..],
        &[
            [717., 424.],
            [735., 510.],
            [773., 565.],
            [825., 599.],
            [871., 615.],
        ][..],
        &[
            [219., 472.],
            [142., 443.],
            [135., 390.],
            [171., 354.],
            [235., 397.],
        ][..],
        &[[235., 397.], [233., 330.], [215., 288.], [159., 250.]][..],
        &[[383., 335.], [411., 277.], [380., 238.], [352., 243.]][..],
        &[[296., 377.], [303., 430.], [287., 468.], [254., 487.]][..],
    ] {
        land.path(TerrainKind::Road, route, 4.);
    }
    for outline in [
        &[
            [477., 820.],
            [495., 783.],
            [533., 778.],
            [565., 753.],
            [605., 761.],
            [643., 761.],
            [675., 797.],
            [675., 842.],
            [630., 887.],
            [578., 914.],
            [535., 901.],
            [492., 880.],
        ][..],
        &[
            [764., 522.],
            [754., 475.],
            [778., 440.],
            [824., 440.],
            [854., 462.],
            [880., 497.],
            [868., 533.],
            [878., 577.],
            [853., 615.],
            [822., 636.],
            [790., 620.],
            [759., 581.],
        ][..],
    ] {
        land.polygon(TerrainKind::Elevation, outline);
    }
    for outline in [
        &[
            [747., 691.],
            [773., 653.],
            [791., 642.],
            [811., 660.],
            [790., 717.],
            [771., 746.],
            [748., 741.],
        ][..],
        &[
            [796., 876.],
            [822., 866.],
            [841., 880.],
            [887., 938.],
            [886., 965.],
            [865., 964.],
            [839., 938.],
        ][..],
        &[
            [425., 963.],
            [467., 944.],
            [498., 951.],
            [528., 974.],
            [485., 999.],
            [449., 1009.],
            [417., 990.],
        ][..],
        &[
            [858., 570.],
            [886., 563.],
            [908., 584.],
            [909., 636.],
            [887., 645.],
            [864., 631.],
        ][..],
    ] {
        land.polygon(TerrainKind::TallGrass, outline);
    }
    // The river separates the banks. Two permanent side crossings remain
    // available if either of the central bridges is destroyed.
    for river in [
        &[
            [286., 130.],
            [294., 214.],
            [333., 278.],
            [334., 345.],
            [385., 448.],
            [476., 520.],
        ][..],
        &[
            [476., 520.],
            [472., 571.],
            [489., 626.],
            [517., 666.],
            [568., 690.],
            [610., 667.],
        ][..],
        &[
            [476., 520.],
            [530., 510.],
            [596., 535.],
            [634., 581.],
            [642., 622.],
            [610., 667.],
        ][..],
        &[
            [610., 667.],
            [676., 716.],
            [715., 760.],
            [725., 807.],
            [766., 891.],
            [842., 971.],
            [891., 1070.],
        ][..],
    ] {
        land.path(TerrainKind::Water, river, 7.5);
    }
    land.path(
        TerrainKind::Road,
        &[[292., 399.], [335., 369.], [380., 333.]],
        11.,
    );
    land.path(
        TerrainKind::Sand,
        &[[684., 868.], [719., 850.], [740., 803.], [766., 777.]],
        11.,
    );
    for (x, y) in [(157., 945.), (879., 234.)] {
        land.disc(TerrainKind::Road, x, y, 10.);
    }
    // Slower permanent access from both banks, looping downstream before
    // returning onto the island. These sand causeways survive bridge loss.
    land.path(
        TerrainKind::Sand,
        &[
            [430., 690.],
            [441., 753.],
            [496., 765.],
            [531., 741.],
            [552., 699.],
            [561., 651.],
            [553., 603.],
        ],
        6.,
    );
    land.path(
        TerrainKind::Sand,
        &[
            [704., 561.],
            [725., 600.],
            [713., 668.],
            [665., 700.],
            [618., 725.],
            [573., 711.],
            [552., 699.],
        ],
        6.,
    );
    for (x, y) in [(135., 960.), (885., 225.)] {
        land.disc(TerrainKind::Road, x, y, 6.5);
    }
    // Floors belong to the buildings. Terrain precedes the structural walls.
    let houses = [
        ("Village / farmhouse", 163., 399., 10.4, 6.9, -15.),
        ("Village / cottage", 187., 319., 9.1, 7.4, 19.),
        ("Village / store", 265., 275., 10.9, 7.6, -12.),
        ("Village / east cottage", 455., 238., 8.4, 7.7, 9.),
        ("Village / barn", 380., 219., 12.7, 8.1, 12.),
        ("Village / workshop", 414., 399., 11.4, 8.1, -28.),
        ("Village / small house", 266., 444., 8.4, 6.3, 2.),
        ("Village / south barn", 170., 509., 12.3, 7.4, 12.),
    ];
    for &(_, x, y, w, h, angle) in &houses {
        let rotation = Vec2::from_angle(angle * std::f32::consts::PI / 180.);
        let outline: Vec<_> = rectangle(Vec2::new(w, h) * 0.5)
            .into_iter()
            .map(|p| {
                let p = rotation.rotate(Vec2::from(p)) / SCALE + Vec2::new(x, y);
                p.to_array()
            })
            .collect();
        land.polygon(TerrainKind::Sand, &outline);
    }
    for (name, x, y, w, h, angle) in houses {
        house(&mut level, name, x, y, w, h, angle.to_radians());
    }
    // Wider, axis-aligned spans match the existing bridge mechanic while
    // retaining the island and two-bank approach of the concept.
    for (a, b) in [([440., 598.], [560., 648.]), ([540., 525.], [680., 575.])] {
        level.items.push(LevelItem::Bridge {
            min: point(a[0], a[1]),
            max: point(b[0], b[1]),
            vertical: false,
            flipped: false,
        });
    }
    for (name, x, y, size, angle) in [
        ("Valley / west ridge", 350., 613., [10., 33.], 0.66),
        ("Valley / east ridge", 589., 387., [9., 30.], 0.67),
        ("Middle / south cover", 368., 704., [5., 7.], 0.2),
        ("Middle / west approach", 380., 665., [6., 4.], 0.),
        ("Middle / east approach", 647., 510., [4., 6.], 0.1),
        ("Middle / north cover", 665., 455., [6., 4.], 0.),
        ("Flank / river bend", 788., 752., [4., 7.], 0.2),
        ("Flank / upper bank", 810., 665., [6., 4.], 0.),
        ("Flank / lower bank", 650., 921., [6., 4.], 0.),
        ("Flank / crossing cover", 789., 817., [4., 5.], 0.4),
        ("Base / red cover", 114., 891., [7., 4.], -0.3),
        ("Base / blue cover", 937., 290., [7., 4.], -0.3),
    ] {
        rock(&mut level, name, x, y, size, angle);
    }
    for (x, y, length, angle) in [
        (125., 497., 7., 1.5),
        (163., 529., 10., 0.2),
        (382., 187., 12., 0.15),
        (817., 925., 7., 0.8),
    ] {
        let mut asset = fence_asset(Vec2::new(length, 0.5));
        asset.name = "Village / yard fence".into();
        object(&mut level, asset, x, y, angle);
    }
    for (kind, x, y) in [
        (StationKind::Repair, 298., 343.),
        (StationKind::Radar, 588., 618.),
    ] {
        level.items.push(LevelItem::Station {
            station: kind,
            position: point(x, y),
            settings: StationSettings {
                capture_time: 7.,
                capture_radius: 8.,
                repair_rate: 8.,
                radar_radius: 30.,
            },
        });
    }
    for (weapon, x, y) in [
        (SpecialWeapon::Laser, 159., 250.),
        (SpecialWeapon::Rocket, 570., 596.),
    ] {
        level.items.push(LevelItem::Pickup {
            weapon,
            position: point(x, y),
            respawn_time: 60.,
            charges: 1,
            cooldown: 0.,
        });
    }
    // Props cluster at gardens, yards and field margins, not in the main lanes.
    for (x, y) in [
        (123., 361.),
        (239., 295.),
        (310., 264.),
        (449., 435.),
        (204., 546.),
        (552., 766.),
        (608., 905.),
        (808., 440.),
        (829., 631.),
    ] {
        for j in 0..4 {
            let offset = Vec2::from_angle(j as f32 * 2.399963) * (5. + j as f32 * 4.);
            object(
                &mut level,
                LibraryPreset::Shrub.asset(),
                x + offset.x,
                y + offset.y,
                j as f32,
            );
        }
    }
    for (x, y) in [
        (145., 447.),
        (154., 452.),
        (363., 252.),
        (374., 254.),
        (397., 438.),
        (403., 450.),
        (161., 545.),
        (172., 547.),
    ] {
        object(&mut level, LibraryPreset::Crate.asset(), x, y, 0.15);
    }
    for (x, y, angle) in [
        (197., 492., 0.2),
        (397., 188., 0.15),
        (433., 362., -0.5),
        (624., 929., 0.2),
    ] {
        object(&mut level, LibraryPreset::Timber.asset(), x, y, angle);
    }
    for (team, x, y, angle, fx, fy) in [
        (Team::Red, 192., 920., -0.7853982, 135., 960.),
        (Team::Blue, 828., 265., 2.3561945, 885., 225.),
    ] {
        level.items.push(LevelItem::Spawn {
            team,
            position: point(x, y),
            angle,
        });
        level.items.push(LevelItem::Flag {
            team,
            position: point(fx, fy),
        });
    }
    for (x, y, angle) in [(135., 960., 0.), (885., 225., std::f32::consts::PI)] {
        flag_compound(&mut level, x, y, angle);
    }
    land.road_shoulders(&level);
    land.write(&mut level);
    verify_pickup_ground(&level)?;
    if std::env::args().any(|arg| arg == "--drive") {
        verify_routes(&level)?;
    }
    let json = level.to_json()?;
    let loaded = Level::from_json(&json)?;
    for team in [Team::Blue, Team::Red] {
        let world = World::from_level(&loaded, team)?;
        println!(
            "{} spawn and bridge banks valid; {} material cells",
            team.label(),
            world.stats().cells
        );
    }
    // A short static check catches intersecting authored geometry without a
    // long gameplay run on every map revision.
    let mut quiet = loaded.clone();
    quiet.enemy_activity = EnemyActivity::Static;
    let mut world = World::from_level(&quiet, Team::Red)?;
    for _ in 0..120 {
        world.step(&[]);
    }
    if world.stats().broken != 0 || world.stats().explosions != 0 {
        return Err("Map damaged itself at rest".into());
    }
    if std::fs::read_to_string("levels/river-divide.json")
        .ok()
        .as_ref()
        != Some(&json)
    {
        std::fs::write("levels/river-divide.json", &json).map_err(|e| e.to_string())?;
    }
    println!(
        "Saved {} items to levels/river-divide.json",
        level.items.len()
    );
    Ok(())
}

// Optional real-physics drive through each corridor. Keep normal regeneration
// quick; use --drive after changing roads, crossing widths or nearby cover.
fn verify_routes(level: &Level) -> Result<(), String> {
    use sim::Command;
    let mut trial = level.clone();
    trial.enemy_activity = EnemyActivity::Static;
    trial.items.retain(|item| {
        !matches!(
            item,
            LevelItem::Spawn {
                team: Team::Blue,
                ..
            }
        )
    });
    let routes: [(&str, &[[f32; 2]]); 4] = [
        (
            "village",
            &[
                [172., 880.],
                [172., 790.],
                [190., 651.],
                [219., 535.],
                [219., 472.],
                [235., 397.],
                [292., 399.],
                [335., 369.],
                [380., 333.],
                [490., 280.],
                [580., 264.],
                [654., 239.],
                [775., 222.],
                [805., 225.],
                [825., 225.],
                [885., 225.],
            ],
        ),
        (
            "radar",
            &[
                [270., 855.],
                [358., 763.],
                [435., 674.],
                [414., 651.],
                [430., 625.],
                [455., 625.],
                [550., 625.],
                [550., 590.],
                [565., 550.],
                [675., 550.],
                [717., 424.],
                [795., 311.],
                [825., 280.],
                [825., 225.],
                [885., 225.],
            ],
        ),
        (
            "highlands",
            &[
                [300., 950.],
                [400., 950.],
                [481., 929.],
                [620., 875.],
                [684., 868.],
                [719., 850.],
                [740., 803.],
                [766., 777.],
                [766., 777.],
                [839., 759.],
                [898., 671.],
                [895., 615.],
                [900., 510.],
                [894., 432.],
                [898., 315.],
                [825., 280.],
                [825., 225.],
                [885., 225.],
            ],
        ),
        (
            "sand",
            &[
                [270., 855.],
                [358., 763.],
                [430., 690.],
                [441., 753.],
                [496., 765.],
                [531., 741.],
                [552., 699.],
                [561., 651.],
                [553., 603.],
                [570., 596.],
                [553., 603.],
                [561., 651.],
                [552., 699.],
                [573., 711.],
                [618., 725.],
                [665., 700.],
                [713., 668.],
                [725., 600.],
                [704., 561.],
                [717., 424.],
                [795., 311.],
                [825., 280.],
                [825., 225.],
                [885., 225.],
            ],
        ),
    ];
    for (name, route) in routes {
        if let Some(filter) =
            std::env::args().find_map(|a| a.strip_prefix("--route=").map(str::to_owned))
            && filter != name
        {
            continue;
        }
        let mut route_level = trial.clone();
        if name == "sand" {
            route_level
                .items
                .retain(|i| !matches!(i, LevelItem::Bridge { .. }));
        }
        let mut world = World::from_level(&route_level, Team::Red)?;
        let mut next = 0;
        let mut finished = false;
        for tick in 0..24000 {
            if world.winner() == Some(Team::Red) {
                println!(
                    "Drive / {name}: enemy flag reached in {:.1} simulation seconds",
                    tick as f32 / 120.
                );
                finished = true;
                break;
            }
            if !world.tank_alive() {
                return Err(format!("Drive / {name}: tank destroyed"));
            }
            let (_, position, angle) = world.tank_poses().next().unwrap();
            let delta = Vec2::from(point(route[next][0], route[next][1])) - position;
            if delta.length() < 0.6 && next + 1 < route.len() {
                if std::env::args().any(|a| a == "--trace") {
                    println!("{name} waypoint {next}: {:.1}s", tick as f32 / 120.);
                }
                next += 1;
                continue;
            }
            let error = (delta.y.atan2(delta.x) - angle + std::f32::consts::PI)
                .rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            let forward = if error.abs() < 0.2 {
                (delta.length() * 0.6).min(0.85)
            } else {
                0.
            };
            let turn = (error * 1.6).clamp(-0.8, 0.8);
            world.step(&[Command::Tracks {
                left: forward + turn,
                right: forward - turn,
            }]);
        }
        if !finished {
            return Err(format!(
                "Drive / {name}: stalled at waypoint {next}, position {:?}",
                world.launcher_position()
            ));
        }
    }
    Ok(())
}

// Run-length merging keeps the half-metre curved landscape inexpensive to
// render and leaves all terrain editable with the existing editor tools.
const SCALE: f32 = 160. / 910.;
const GRID: usize = 320;
struct Landscape {
    cells: Vec<TerrainKind>,
}
impl Landscape {
    fn new() -> Self {
        Self {
            cells: vec![TerrainKind::Ground; GRID * GRID],
        }
    }
    fn paint(&mut self, kind: TerrainKind, contains: impl Fn(Vec2) -> bool) {
        for y in 1..GRID - 1 {
            for x in 1..GRID - 1 {
                let p = Vec2::new(
                    55. + (x as f32 + 0.5) * 910. / GRID as f32,
                    142. + (y as f32 + 0.5) * 910. / GRID as f32,
                );
                if contains(p) {
                    self.cells[y * GRID + x] = kind;
                }
            }
        }
    }
    fn disc(&mut self, kind: TerrainKind, x: f32, y: f32, r: f32) {
        self.paint(kind, |p| {
            p.distance_squared(Vec2::new(x, y)) <= (r / SCALE).powi(2)
        });
    }
    fn polygon(&mut self, kind: TerrainKind, points: &[[f32; 2]]) {
        self.paint(kind, |p| {
            let mut inside = false;
            for i in 0..points.len() {
                let a = Vec2::from(points[i]);
                let b = Vec2::from(points[(i + 1) % points.len()]);
                if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x
                {
                    inside = !inside;
                }
            }
            inside
        });
    }
    fn path(&mut self, kind: TerrainKind, points: &[[f32; 2]], width: f32) {
        let mut samples = Vec::new();
        for i in 0..points.len() - 1 {
            let a = Vec2::from(points[i.saturating_sub(1)]);
            let b = Vec2::from(points[i]);
            let c = Vec2::from(points[i + 1]);
            let d = Vec2::from(points[(i + 2).min(points.len() - 1)]);
            for j in 0..12 {
                let t = j as f32 / 12.;
                samples.push(
                    0.5 * ((2. * b)
                        + (-a + c) * t
                        + (2. * a - 5. * b + 4. * c - d) * t * t
                        + (-a + 3. * b - 3. * c + d) * t * t * t),
                );
            }
        }
        samples.push(Vec2::from(*points.last().unwrap()));
        let radius = width / SCALE * 0.5;
        let lo = samples
            .iter()
            .copied()
            .fold(Vec2::splat(f32::INFINITY), Vec2::min)
            - Vec2::splat(radius);
        let hi = samples
            .iter()
            .copied()
            .fold(Vec2::splat(f32::NEG_INFINITY), Vec2::max)
            + Vec2::splat(radius);
        self.paint(kind, |p| {
            if p.cmplt(lo).any() || p.cmpgt(hi).any() {
                return false;
            }
            samples.windows(2).any(|s| {
                let delta = s[1] - s[0];
                let t = ((p - s[0]).dot(delta) / delta.length_squared().max(1e-6)).clamp(0., 1.);
                p.distance_squared(s[0] + delta * t) <= radius * radius
            })
        });
    }
    fn road_shoulders(&mut self, level: &Level) {
        // Final half-metre raster cells keep a one-metre natural shoulder at
        // rock edges; the routes above take the actual detours around ridges.
        let rocks: Vec<_> = level
            .items
            .iter()
            .filter_map(|item| match item {
                LevelItem::Object { asset, .. } if asset.surface == sim::Surface::Rock => {
                    Some(item.world_outline())
                }
                _ => None,
            })
            .collect();
        for y in 0..GRID {
            for x in 0..GRID {
                if !matches!(
                    self.cells[y * GRID + x],
                    TerrainKind::Road | TerrainKind::Sand
                ) {
                    continue;
                }
                let p = Vec2::new((x as f32 + 0.5) * 0.5 - 80., (y as f32 + 0.5) * 0.5 - 80.);
                if rocks.iter().any(|poly| {
                    let mut inside = false;
                    for i in 0..poly.len() {
                        let a = poly[i];
                        let b = poly[(i + 1) % poly.len()];
                        let d = b - a;
                        if p.distance(a + d * ((p - a).dot(d) / d.length_squared()).clamp(0., 1.))
                            < 1.1
                        {
                            return true;
                        }
                        if (a.y > p.y) != (b.y > p.y)
                            && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x
                        {
                            inside = !inside;
                        }
                    }
                    inside
                }) {
                    self.cells[y * GRID + x] = TerrainKind::Ground;
                }
            }
        }
    }
    fn write(&self, level: &mut Level) {
        let mut rectangles: Vec<(TerrainKind, usize, usize, usize, usize)> = vec![];
        let mut previous: std::collections::BTreeMap<(u8, usize, usize), usize> =
            std::collections::BTreeMap::new();
        for y in 0..GRID {
            let mut row = std::collections::BTreeMap::new();
            let mut x = 0;
            while x < GRID {
                let kind = self.cells[y * GRID + x];
                let start = x;
                x += 1;
                while x < GRID && self.cells[y * GRID + x] == kind {
                    x += 1;
                }
                if kind == TerrainKind::Ground {
                    continue;
                }
                let key = (kind as u8, start, x);
                let id = if let Some(&id) = previous.get(&key) {
                    rectangles[id].4 = y + 1;
                    id
                } else {
                    let id = rectangles.len();
                    rectangles.push((kind, start, y, x, y + 1));
                    id
                };
                row.insert(key, id);
            }
            previous = row;
        }
        for (terrain, x, y, max_x, max_y) in rectangles {
            level.terrain_mask.paint(
                terrain,
                Vec2::new(x as f32 * 0.5 - 80., y as f32 * 0.5 - 80.),
                Vec2::new(max_x as f32 * 0.5 - 80., max_y as f32 * 0.5 - 80.),
                level.size.into(),
            );
        }
    }
}

// Both compounds use identical wall sections and a 4 m gate, rotated between
// teams. Masonry can be breached; only the small corner foundations are fixed.
fn flag_compound(level: &mut Level, x: f32, y: f32, angle: f32) {
    let rotation = Vec2::from_angle(angle);
    for (dx, dy, w, h) in [
        (-6., 0., 1., 12.),
        (0., -6., 12., 1.),
        (0., 6., 12., 1.),
        (6., -4., 1., 4.),
        (6., 4., 1., 4.),
    ] {
        let mut asset = wall_asset(Vec2::new(w, h));
        asset.name = "Base / protective wall".into();
        asset.regenerate_grains(GrainMode::Bricks, [1.2, 0.8], 7);
        asset.fit_grain_budget();
        let offset = rotation.rotate(Vec2::new(dx, dy)) / SCALE;
        object(level, asset, x + offset.x, y + offset.y, angle);
    }
    for dx in [-6., 6.] {
        for dy in [-6., 6.] {
            let offset = rotation.rotate(Vec2::new(dx, dy)) / SCALE;
            level.items.push(LevelItem::Solid {
                outline: rectangle(Vec2::splat(0.6)),
                position: point(x + offset.x, y + offset.y),
                angle,
                cutouts: vec![],
            });
        }
    }
}
fn verify_pickup_ground(level: &Level) -> Result<(), String> {
    let w = World::from_level(level, Team::Red)?;
    for item in &level.items {
        if let LevelItem::Pickup {
            position, weapon, ..
        } = item
        {
            for dx in [-0.8, 0., 0.8] {
                for dy in [-0.8, 0., 0.8] {
                    let terrain = w.terrain_at(Vec2::from(*position) + Vec2::new(dx, dy));
                    if terrain.blocks_tanks() || terrain == TerrainKind::Bridge {
                        return Err(format!("{weapon:?} pickup needs permanent dry ground"));
                    }
                }
            }
        }
    }
    Ok(())
}
