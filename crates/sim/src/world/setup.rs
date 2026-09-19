//! Validated world construction and repeatable test fixtures.
use super::*;

impl World {
    pub fn new(scene: Scene) -> Self {
        let mut physics = PhysicsWorld {
            gravity: Vector::ZERO,
            ..Default::default()
        };
        // Contact impulses are read after each explicit microstep. Rapier's
        // internal TGS substep count stays one so their time interval is known.
        physics.integration_parameters = IntegrationParameters {
            dt: TICK_DT / SUBSTEPS as f32,
            num_solver_iterations: 1,
            num_internal_pgs_iterations: 12,
            max_ccd_substeps: 1,
            ..Default::default()
        };
        let mut world = Self {
            plastic_work: 0.0,
            physics,
            pieces: Vec::new(),
            bonds: Vec::new(),
            bending_joints: Vec::new(),
            launcher: 0,
            tanks: vec![],
            enemy: None,
            arena_size: Vector::new(24.0, 15.0),
            scene,
            tick: 0,
            shots: 0,
            peak_load: 0.0,
            shells: Vec::new(),
            blast_pulses: Vec::new(),
            explosions: 0,
            cleared: 0,
            broken: 0,
            topology_changed: false,
            reload: 0.0,
            last_ammunition: Ammunition::HighExplosive,
            laser_beam: None,
            ablated: 0,
            ablated_area: 0.0,
            track_travel: [0.0; 2],
            loads: Vec::new(),
            structural_cache: std::collections::BTreeMap::new(),
            impact_serial: 0,
            refinements: 0,
            refinement_limited: 0,
            fracture_work: 0.0,
            structural_residual: 0.0,
            elastic_energy: 0.0,
            structural_iterations: 0,
            unconverged: 0,
            tank_speed_limit: 6.5,
            objects: Vec::new(),
            terrain: Vec::new(),
            bridges: Vec::new(),
            water_colliders: Vec::new(),
            tank_fall: None,
            flags: Vec::new(),
            team: crate::level::Team::Blue,
            winner: None,
        };
        world.add_box(
            Vector::new(0.0, -7.5),
            Vector::new(12.0, 0.15),
            Material::Boundary,
            true,
        );
        world.add_box(
            Vector::new(0.0, 7.5),
            Vector::new(12.0, 0.15),
            Material::Boundary,
            true,
        );
        world.add_box(
            Vector::new(-12.0, 0.0),
            Vector::new(0.15, 7.5),
            Material::Boundary,
            true,
        );
        world.add_box(
            Vector::new(12.0, 0.0),
            Vector::new(0.15, 7.5),
            Material::Boundary,
            true,
        );
        world.launcher = world.add_tank();
        match scene {
            Scene::Masonry => world.masonry(),
            Scene::Cantilever => world.cantilever(),
            Scene::LooseBlocks => world.loose_blocks(),
            Scene::Custom | Scene::Level => {}
            Scene::ObjectYard => world.object_yard(),
            Scene::Terrain => world.terrain_range(),
            Scene::BarrelRubble => world.barrel_rubble(),
        }
        world.rebuild_chunks();
        world
    }

    pub fn from_object(object: &crate::ObjectAsset) -> Result<Self, String> {
        let mesh = object.mesh()?;
        let mut world = Self::new(Scene::Custom);
        world.add_object(object, &mesh, Vector::new(3.0, 0.0));
        world.rebuild_chunks();
        Ok(world)
    }

    /// Build a small multi-object experiment, with all initial geometry validated
    /// before creating the world. Runtime instances retain independent fuses.
    pub fn from_objects(objects: &[(crate::ObjectAsset, Vector)]) -> Result<Self, String> {
        if objects.len() > 32 {
            return Err("Use at most 32 objects per test scene.".into());
        }
        let mut meshes = Vec::new();
        for (a, p) in objects {
            let mesh = a.mesh()?;
            if !p.is_finite()
                || a.outline
                    .iter()
                    .any(|q| (q[0] + p.x).abs() > 11.5 || (q[1] + p.y).abs() > 7.1)
            {
                return Err("Object placement lies outside the arena.".into());
            }
            meshes.push(mesh);
        }
        if meshes.iter().map(|m| m.triangles.len() + 1).sum::<usize>() + 5 > 8192 {
            return Err("Test scene exceeds the 8192-piece limit.".into());
        }
        let mut world = Self::new(Scene::Custom);
        for ((a, p), m) in objects.iter().zip(&meshes) {
            world.add_object(a, m, *p);
        }
        world.rebuild_chunks();
        Ok(world)
    }

    pub(crate) fn add_test_backstop(&mut self, object: &crate::ObjectAsset) {
        let right = object
            .outline
            .iter()
            .map(|p| p[0])
            .fold(f32::NEG_INFINITY, f32::max)
            + 3.0;
        self.add_box(
            Vector::new(right + 0.35, 0.0),
            Vector::new(0.2, 2.0),
            Material::Anchor,
            true,
        );
    }
}
