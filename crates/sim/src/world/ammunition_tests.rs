use super::*;

fn material_area(w: &World) -> f32 {
    w.pieces
        .iter()
        .filter(|p| p.active && p.destructible())
        .map(|p| p.shape.area())
        .sum()
}

#[test]
fn projectile_profiles_conserve_launch_momentum_and_have_distinct_blasts() {
    let mut energy = Vec::new();
    let mut radius = Vec::new();
    for ammo in [
        Ammunition::HighExplosive,
        Ammunition::ArmorPiercing,
        Ammunition::Rocket,
    ] {
        let mut w = World::from_object(&crate::LibraryPreset::Wall.asset()).unwrap();
        w.fire(Shot::new(ammo, Vector::new(3.0, 0.0)), &mut vec![]);
        let shell = w.shells[0];
        let tank = &w.physics.bodies[w.pieces[w.launcher].body];
        assert!((tank.linvel() * tank.mass() + shell.velocity * shell.mass).length() < 0.001);
        energy.push(0.5 * shell.mass * shell.velocity.length_squared());
        let mut blast = 0.0;
        for _ in 0..120 {
            for event in w.step(&[]) {
                if let Event::Exploded { radius, .. } = event {
                    blast = radius;
                }
            }
        }
        radius.push(blast);
        assert!(w.shells.is_empty(), "spent rounds must disappear");
    }
    assert!(energy[1] > energy[0] * 3.0 && energy[2] < energy[0] * 0.1);
    assert!(radius[0] > radius[1] * 2.0 && radius[1] > 0.0);
    assert!(radius[2] > radius[0] * 2.0);
}

#[test]
fn switching_ammunition_cannot_bypass_reload_and_invalid_shots_do_nothing() {
    let mut w = World::new(Scene::Custom);
    w.fire(
        Shot::new(Ammunition::HighExplosive, Vector::new(3.0, 0.0)),
        &mut vec![],
    );
    for ammo in Ammunition::ALL {
        w.fire(Shot::new(ammo, Vector::new(3.0, 0.0)), &mut vec![]);
    }
    assert_eq!(w.shots, 1);
    let mut w = World::new(Scene::Custom);
    for ammo in Ammunition::ALL {
        let mut shot = Shot::new(ammo, Vector::new(3.0, 0.0));
        shot.mass = f32::NAN;
        w.fire(shot, &mut vec![]);
        w.fire(Shot::new(ammo, Vector::new(f32::NAN, 0.0)), &mut vec![]);
    }
    assert_eq!(w.shots, 0);
    assert_eq!(w.stats().kinetic_energy, 0.0);
}

fn coarse_triangle() -> crate::ObjectAsset {
    let mut a = crate::ObjectAsset::preset(crate::ObjectPreset::Plate);
    a.outline = vec![[-0.8, -0.9], [0.8, -0.9], [0.0, 0.9]];
    a.refinement = 0;
    a
}

#[test]
fn wide_laser_refines_and_removes_bounded_small_cells_without_forces() {
    for mut asset in [
        coarse_triangle(),
        crate::LibraryPreset::Barrel.asset(),
        crate::LibraryPreset::Rock.asset(),
    ] {
        asset.components.explosive = Some(crate::Explosive::default());
        let mut w = World::from_object(&asset).unwrap();
        let initial = material_area(&w);
        let command = [Command::Fire(Shot::new(
            Ammunition::Laser,
            Vector::new(3.0, 0.0),
        ))];
        w.step(&command);
        assert!(w.refinements > 0);
        assert!((material_area(&w) + w.ablated_area - initial).abs() < 1e-5);
        for _ in 1..240 {
            let before_count = w.ablated;
            let before_area = w.ablated_area;
            w.step(&command);
            assert!(w.ablated - before_count <= 8);
            assert!(w.ablated_area - before_area <= refinement::LASER_TILE_AREA * 8.0 + 1e-6);
        }
        assert!(w.ablated > 0 && w.refinements > 1);
        assert!((material_area(&w) + w.ablated_area - initial).abs() < 1e-4);
        assert!(
            w.stats().kinetic_energy < 1e-6,
            "laser supplied kinetic energy"
        );
        assert_eq!(
            w.physics.bodies[w.pieces[w.launcher].body].linvel(),
            Vector::ZERO
        );
        assert_eq!(w.broken, 0);
        assert_eq!(w.explosions, 0);
        assert!(w.shells.is_empty());
        assert_eq!(w.stats().unconverged, 0);
    }
}

#[test]
fn laser_stops_at_fixtures_and_can_sever_a_free_frame_without_kicking_it() {
    let mut w = World::from_object(&coarse_triangle()).unwrap();
    w.add_box(
        Vector::new(1.0, 0.0),
        Vector::new(0.1, 2.0),
        Material::Anchor,
        true,
    );
    for _ in 0..120 {
        w.step(&[Command::Fire(Shot::new(
            Ammunition::Laser,
            Vector::new(3.0, 0.0),
        ))]);
    }
    assert_eq!(w.refinements + w.ablated, 0);
    assert!(w.laser_beam().unwrap().end.x < 1.01);
    let mut w = World::from_object(&coarse_triangle()).unwrap();
    for _ in 0..1800 {
        w.step(&[Command::Fire(Shot::new(
            Ammunition::Laser,
            Vector::new(3.0, 0.0),
        ))]);
    }
    assert!(
        w.chunk_members().len() > 1,
        "a completed cut must separate connected bodies"
    );
    assert!(w.stats().kinetic_energy < 1e-6);
    assert_eq!(w.unconverged, 0);
}

#[test]
fn ap_breaks_more_material_near_the_hit_than_he_with_a_smaller_blast_radius() {
    for asset in [
        crate::LibraryPreset::Wall.asset(),
        crate::LibraryPreset::Rock.asset(),
        crate::ObjectAsset::workshop_preset(crate::ObjectPreset::Plate),
    ] {
        let mut results = Vec::new();
        for ammo in [Ammunition::HighExplosive, Ammunition::ArmorPiercing] {
            let mut w = World::from_object(&asset).unwrap();
            let command = [Command::Fire(Shot::new(ammo, Vector::new(3.0, 0.0)))];
            let mut impact = None;
            let mut radius = 0.0;
            let mut fractures = Vec::new();
            for tick in 0..240 {
                for event in w.step(if tick == 0 { &command } else { &[] }) {
                    match event {
                        Event::Exploded {
                            position,
                            radius: r,
                        } => {
                            impact = Some(position);
                            radius = r;
                        }
                        Event::Fractured { position, .. } => fractures.push(position),
                        _ => {}
                    }
                }
            }
            let near = fractures
                .iter()
                .filter(|p| p.distance(impact.unwrap()) <= 0.65)
                .count();
            results.push((near, radius));
            assert_eq!(w.unconverged, 0);
        }
        assert!(
            results[1].0 > results[0].0,
            "{}: local HE/AP damage {results:?}",
            asset.name
        );
        assert!(results[1].1 < results[0].1 * 0.5);
    }
}

#[test]
fn laser_cuts_at_ten_pulses_per_second_but_the_beam_tracks_aim_between_pulses() {
    let mut w = World::from_object(&crate::LibraryPreset::Wall.asset()).unwrap();
    let shot = |y| Command::Fire(Shot::new(Ammunition::Laser, Vector::new(3.0, y)));
    w.step(&[shot(0.0)]);
    let first = w.laser_beam().unwrap().end;
    for _ in 0..10 {
        w.step(&[shot(0.8)]);
    }
    assert!(w.laser_beam().unwrap().end.distance(first) > 0.5);
    assert_eq!(w.shots, 1);
    assert!(w.refinements > 0);
    for _ in 11..240 {
        w.step(&[shot(0.0)]);
    }
    assert_eq!(w.shots, 20);
    assert!(w.refinements + w.ablated <= 160);
    assert!(
        w.ablated_area > 0.03,
        "special laser must cut more material"
    );
    let before = w.refinements + w.ablated;
    for _ in 0..24 {
        w.step(&[]);
    }
    assert_eq!(w.refinements + w.ablated, before);
    assert!(w.laser_beam().is_none());
}

#[test]
fn cutting_a_moving_cell_removes_its_mass_without_accelerating_survivors() {
    let mut w = World::from_object(&coarse_triangle()).unwrap();
    let mut id = w.pieces.iter().position(Piece::destructible).unwrap();
    while w.pieces[id].shape.area() > refinement::LASER_TILE_AREA {
        let child = w.pieces.len();
        assert!(w.laser_cut(id));
        id = child;
    }
    let handle = w.pieces[id].body;
    w.physics.bodies[handle].set_linvel(Vector::new(2.0, 3.0), true);
    w.physics.bodies[handle].set_angvel(0.7, true);
    let before: Vec<_> = w
        .pieces
        .iter()
        .enumerate()
        .filter(|(_, p)| p.active && p.destructible())
        .map(|(i, p)| {
            (
                i,
                w.piece_pose(i),
                p.mass(),
                w.physics.bodies[p.body].velocity_at_point(w.piece_pose(i).translation),
            )
        })
        .collect();
    let removed = w.pieces[id].mass();
    assert!(w.laser_cut(id));
    assert!(!w.pieces[id].active);
    let mut remaining_mass = 0.0;
    for &(i, pose, mass, velocity) in &before {
        if i == id {
            continue;
        }
        remaining_mass += w.pieces[i].mass();
        assert_eq!(w.pieces[i].mass(), mass);
        assert!(w.piece_pose(i).translation.distance(pose.translation) < 1e-5);
        let body = &w.physics.bodies[w.pieces[i].body];
        assert!(body.velocity_at_point(pose.translation).distance(velocity) < 1e-4);
        assert!((body.angvel() - 0.7).abs() < 1e-4);
    }
    assert!((remaining_mass + removed - before.iter().map(|p| p.2).sum::<f32>()).abs() < 1e-3);
}

#[test]
fn level_cutting_budget_remains_bounded_after_removing_the_starting_geometry_limit() {
    let mut w = World::from_object(&coarse_triangle()).unwrap();
    w.scene = Scene::Level;
    w.refinements = 2047; // Only four of the 8192 runtime records remain.
    let id = w.pieces.iter().position(Piece::destructible).unwrap();
    let first_child = w.pieces.len();
    assert!(w.laser_cut(id));
    assert_eq!(w.pieces.len(), first_child + 4);
    let area = material_area(&w);
    assert!(!w.laser_cut(first_child));
    assert_eq!(w.pieces.len(), first_child + 4);
    assert_eq!(material_area(&w), area);
    assert_eq!(w.ablated, 0);
    assert_eq!(w.refinement_limited, 1);
}

#[test]
fn laser_detail_limit_never_erases_an_oversized_cell() {
    let mut w = World::from_object(&coarse_triangle()).unwrap();
    let id = w.pieces.iter().position(Piece::destructible).unwrap();
    w.pieces[id].refinement = u8::MAX;
    let area = material_area(&w);
    assert!(!w.laser_cut(id));
    assert_eq!(material_area(&w), area);
    assert_eq!(w.ablated, 0);
    assert_eq!(w.refinements, 0);
    assert_eq!(w.refinement_limited, 1);
}

#[test]
fn cutting_fading_debris_preserves_its_deadline_without_double_counting_removal() {
    let mut w = World::new(Scene::Custom);
    let id = w.add_triangle(
        [
            Vector::new(3.0, 0.0),
            Vector::new(3.3, 0.0),
            Vector::new(3.0, 0.2),
        ],
        Material::Panel,
    );
    w.rebuild_chunks();
    let initial = material_area(&w);
    w.pieces[id].debris_remaining = Some(0.5);
    let child = w.pieces.len();
    assert!(w.laser_cut(id));
    assert_eq!(w.stats().cleared_area, 0.0);
    assert!(
        w.pieces[child..]
            .iter()
            .all(|p| p.debris_remaining == Some(0.5))
    );
    assert!(w.laser_cut(child));
    assert_eq!(w.stats().cleared_area, 0.0);
    assert!(w.ablated_area > 0.0);
    for _ in 0..61 {
        w.step(&[]);
    }
    assert_eq!(material_area(&w), 0.0);
    assert_eq!(w.cleared, 3);
    assert!((w.stats().cleared_area + w.ablated_area - initial).abs() < 1e-6);
}
