//! Optional native benchmark instrumentation; no clocks or storage in normal builds.

macro_rules! profile {
    ($world:ident, $stage:ident, $body:block) => {{
        #[cfg(feature = "physics-profile")]
        let started = std::time::Instant::now();
        let result = $body;
        #[cfg(feature = "physics-profile")]
        {
            $world.profile.$stage += started.elapsed().as_secs_f64() * 1000.0;
        }
        result
    }};
}
pub(super) use profile;

/// Milliseconds accumulated since `take_physics_profile`. Rebuild, structural
/// solve and all AI fields are nested: do not add them to top-level stage totals.
#[cfg(feature = "physics-profile")]
#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct PhysicsProfile {
    pub control: f64,
    pub turrets: f64,
    pub driving: f64,
    pub projectiles: f64,
    pub fracture: f64,
    pub contacts: f64,
    pub rapier: f64,
    pub plasticity: f64,
    pub cleanup: f64,
    pub rebuild: f64,
    pub structural_solve: f64,
    /// Nested inside control: AI retreat clearance checks.
    pub ai_clearance: f64,
    pub ai_navigation: f64,
    pub ai_sight: f64,
    pub ai_obstruction: f64,
}

#[cfg(feature = "physics-profile")]
impl std::ops::AddAssign for PhysicsProfile {
    fn add_assign(&mut self, rhs: Self) {
        self.control += rhs.control;
        self.turrets += rhs.turrets;
        self.driving += rhs.driving;
        self.projectiles += rhs.projectiles;
        self.fracture += rhs.fracture;
        self.contacts += rhs.contacts;
        self.rapier += rhs.rapier;
        self.plasticity += rhs.plasticity;
        self.cleanup += rhs.cleanup;
        self.rebuild += rhs.rebuild;
        self.structural_solve += rhs.structural_solve;
        self.ai_clearance += rhs.ai_clearance;
        self.ai_navigation += rhs.ai_navigation;
        self.ai_sight += rhs.ai_sight;
        self.ai_obstruction += rhs.ai_obstruction;
    }
}

impl super::World {
    #[cfg(feature = "physics-profile")]
    pub fn take_physics_profile(&mut self) -> PhysicsProfile {
        std::mem::take(&mut self.profile)
    }

    /// Dedicated pool for native experiments. The ordinary game remains serial.
    #[cfg(feature = "parallel")]
    pub fn configure_physics_threads(&mut self, count: usize) -> Result<(), String> {
        if count == 0 {
            return Err("physics thread count must be positive".into());
        }
        self.physics
            .configure_thread_pool(count)
            .map_err(|e| e.to_string())
    }
}
