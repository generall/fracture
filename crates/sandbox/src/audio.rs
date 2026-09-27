//! Presentation-only mixer. Music follows sight; simulation events drive effects.
mod panel;
mod settings;
use crate::{game, lab::Lab};
use macroquad::{audio::*, prelude::*};
use sim::{Ammunition, Event, Scene};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub(crate) enum Clip {
    March,
    Battle,
    He,
    Ap,
    Rocket,
    Plasma,
    Impact,
    Collision,
    Fracture,
    Explosion,
    Splash,
    Idle,
    Engine,
    Tracks,
    Laser,
    Charge,
    Pickup,
    Reload,
    Respawn,
    Victory,
    Defeat,
    Countdown,
    Go,
    BarrelCrush,
    FenceCrunch,
    TracksRoad,
    TracksSand,
    TracksGrass,
    TracksBridge,
    ShrubBreak,
    CrateBreak,
    TimberBreak,
}
const COUNT: usize = 32;
impl Clip {
    fn feedback(self) -> bool {
        matches!(
            self,
            Self::Pickup
                | Self::Reload
                | Self::Respawn
                | Self::Victory
                | Self::Defeat
                | Self::Countdown
                | Self::Go
        )
    }
}
const LOOPS: [Clip; 11] = [
    Clip::March,
    Clip::Battle,
    Clip::Idle,
    Clip::Engine,
    Clip::Tracks,
    Clip::Laser,
    Clip::Charge,
    Clip::TracksRoad,
    Clip::TracksSand,
    Clip::TracksGrass,
    Clip::TracksBridge,
];
#[derive(Clone, Copy)]
pub(crate) struct Cue {
    clip: Clip,
    position: sim::Vec2,
    gain: f32,
}
impl Cue {
    pub(crate) fn local(clip: Clip, world: &sim::World) -> Self {
        Self {
            clip,
            position: world.launcher_position(),
            gain: match clip {
                Clip::Pickup => 0.35,
                Clip::Reload => 0.22,
                _ => 1.0,
            },
        }
    }
    pub(crate) fn enqueue(self, cues: &mut Vec<Self>, listener: sim::Vec2) {
        if let Some(existing) = cues.iter_mut().find(|cue| cue.clip == self.clip) {
            if self.gain * distance_gain((self.position - listener).length())
                > existing.gain * distance_gain((existing.position - listener).length())
            {
                *existing = self;
            }
        } else {
            cues.push(self);
        }
    }
    pub(crate) fn event(event: &Event, team: sim::level::Team) -> Option<Self> {
        let (clip, position, gain) = match *event {
            Event::Fired {
                ammunition,
                position,
                ..
            } => (
                match ammunition {
                    Ammunition::HighExplosive => Clip::He,
                    Ammunition::ArmorPiercing => Clip::Ap,
                    Ammunition::Rocket => Clip::Rocket,
                    Ammunition::Plasma => Clip::Plasma,
                    Ammunition::Laser => return None, // sustained beam is mixed separately
                },
                position,
                1.0,
            ),
            Event::Buckled { position, work } => (
                Clip::BarrelCrush,
                position,
                (work / 60.0).sqrt().clamp(0.25, 0.85),
            ),
            Event::Bent { position, work } => (
                Clip::FenceCrunch,
                position,
                (work / 80.0).sqrt().clamp(0.25, 0.8),
            ),
            Event::DecorationDestroyed { position, kind } => {
                let (clip, gain) = match kind {
                    sim::DecorationKind::Shrub => (Clip::ShrubBreak, 0.18),
                    sim::DecorationKind::Crate => (Clip::CrateBreak, 0.26),
                    sim::DecorationKind::Timber => (Clip::TimberBreak, 0.28),
                };
                (clip, position, gain)
            }
            Event::Impacted { position, .. } => (Clip::Impact, position, 0.6),
            Event::Fractured { position, .. } => (Clip::Fracture, position, 0.5),
            Event::Exploded { position, radius } => {
                (Clip::Explosion, position, (radius / 5.0).clamp(0.4, 1.0))
            }
            Event::Collided { position, energy } => (
                Clip::Collision,
                position,
                (energy / 2500.0).sqrt().clamp(0.2, 0.85),
            ),
            Event::BridgeCollapsed { position } => (Clip::Fracture, position, 1.0),
            Event::TankDestroyed { position, .. } => (Clip::Explosion, position, 1.0),
            Event::TankSunk { position } => (Clip::Splash, position, 0.7),
            Event::TankRespawned {
                position,
                team: spawned,
            } if spawned == team => (Clip::Respawn, position, 0.4),
            _ => return None,
        };
        Some(Self {
            clip,
            position,
            gain,
        })
    }
}

#[derive(Default)]
struct MusicDirector {
    blend: f32,
    hold: f32,
}
impl MusicDirector {
    fn update(&mut self, dt: f32, enemy_visible: bool, in_match: bool, paused: bool) {
        if paused && in_match {
            return;
        }
        if !in_match {
            self.hold = 0.0;
        } else if enemy_visible {
            self.hold = 4.0;
        } else {
            self.hold = (self.hold - dt).max(0.0);
        }
        let target = if in_match && self.hold > 0.0 {
            1.0
        } else {
            0.0
        };
        self.blend = approach(
            self.blend,
            target,
            dt / if target > self.blend { 2.0 } else { 3.0 },
        );
    }
    fn gains(&self) -> [f32; 2] {
        let phase = self.blend * std::f32::consts::FRAC_PI_2;
        [phase.cos().max(0.0), phase.sin()]
    }
}
fn approach(value: f32, target: f32, step: f32) -> f32 {
    value + (target - value).clamp(-step, step)
}
fn distance_gain(distance: f32) -> f32 {
    if distance >= 45.0 {
        0.0
    } else {
        (1.0 - distance / 45.0).powi(2)
    }
}

// Base tread clatter plus quiet surface textures; the engine keeps its identity.
fn terrain_track_gains(terrain: sim::TerrainKind) -> [f32; 5] {
    use sim::TerrainKind::*;
    match terrain {
        Ground => [0.14, 0.0, 0.0, 0.0, 0.0],
        Road => [0.11, 0.045, 0.0, 0.0, 0.0],
        Sand => [0.075, 0.0, 0.065, 0.0, 0.0],
        TallGrass => [0.10, 0.0, 0.0, 0.05, 0.0],
        Bridge => [0.10, 0.0, 0.0, 0.0, 0.055],
        Elevation => [0.125, 0.0, 0.025, 0.0, 0.0],
        Water | Cliff => [0.0; 5],
    }
}

fn blend_track_surface(gains: &mut [f32; 5], terrain: sim::TerrainKind, dt: f32) {
    // About 350 ms to settle, independent of frame rate. Never restart a loop
    // at a terrain boundary, including the intact-bridge override.
    let blend = 1.0 - (-dt.max(0.0) / 0.12).exp();
    for (gain, target) in gains.iter_mut().zip(terrain_track_gains(terrain)) {
        *gain += (target - *gain) * blend;
    }
}

pub(crate) struct Audio {
    sounds: Vec<Sound>,
    settings: settings::Settings,
    director: MusicDirector,
    volumes: [f32; LOOPS.len()],
    terrain_tracks: [f32; 5],
    cooldowns: [f32; COUNT],
    ticks: u64,
    countdown: i32,
    finished: bool,
    was_gameplay: bool,
    charging: bool,
    panel_open: bool,
    pub(crate) notice: f32,
    played: u32,
}
impl Audio {
    pub(crate) async fn load() -> Self {
        macro_rules! asset {
            ($file:literal) => {
                include_bytes!(concat!("../assets/audio/", $file, ".ogg")).as_slice()
            };
        }
        let mut sounds = Vec::new();
        for bytes in [
            asset!("march"),
            asset!("battle"),
            asset!("he"),
            asset!("ap"),
            asset!("rocket"),
            asset!("plasma"),
            asset!("impact"),
            asset!("collision"),
            asset!("fracture"),
            asset!("explosion"),
            asset!("splash"),
            asset!("idle"),
            asset!("engine"),
            asset!("tracks"),
            asset!("laser"),
            asset!("charge"),
            asset!("pickup"),
            asset!("reload"),
            asset!("respawn"),
            asset!("victory"),
            asset!("defeat"),
            asset!("countdown"),
            asset!("go"),
            asset!("barrel-crush"),
            asset!("fence-crunch"),
            asset!("tracks-road"),
            asset!("tracks-sand"),
            asset!("tracks-grass"),
            asset!("tracks-bridge"),
            asset!("shrub-break"),
            asset!("crate-break"),
            asset!("timber-break"),
        ] {
            sounds.push(load_sound_from_bytes(bytes).await.expect("bundled audio"));
        }
        for clip in LOOPS.into_iter().filter(|clip| *clip != Clip::Charge) {
            play_sound(
                &sounds[clip as usize],
                PlaySoundParams {
                    looped: true,
                    volume: 0.0,
                },
            );
        }
        Self {
            sounds,
            settings: settings::Settings::load(),
            director: MusicDirector::default(),
            volumes: [0.0; LOOPS.len()],
            terrain_tracks: terrain_track_gains(sim::TerrainKind::Ground),
            cooldowns: [0.0; COUNT],
            ticks: 0,
            countdown: 0,
            finished: false,
            was_gameplay: false,
            charging: false,
            panel_open: false,
            notice: 0.0,
            played: 0,
        }
    }
    pub(crate) fn update(&mut self, dt: f32, lab: &mut Lab, game: &game::Game, gameplay: bool) {
        self.notice = (self.notice - dt).max(0.0);
        let changed = self.settings.keys();
        if changed {
            self.settings.save();
            self.notice = 3.0;
            // Stop existing transients when muting/changing FX; don't unmute old shots.
            self.stop_effects();
        }
        let ticks = lab.world.stats().ticks;
        if ticks < self.ticks || gameplay != self.was_gameplay {
            self.stop_effects();
            self.cooldowns.fill(0.0);
            self.finished = false;
            self.countdown = 0;
            self.director = MusicDirector::default();
        }
        self.ticks = ticks;
        self.was_gameplay = gameplay;
        for cooldown in &mut self.cooldowns {
            *cooldown = (*cooldown - dt).max(0.0);
        }
        let in_match = gameplay && lab.world.scene() == Scene::Level;
        let result = lab.match_progress.as_ref().and_then(|p| p.outcome);
        let paused = gameplay
            && (self.panel_open
                || lab.paused
                || (game.screen == game::Screen::Match && !game.input_allowed(lab)));
        let live = gameplay && !paused && result.is_none();
        let visible = live
            && lab.world.tank_alive()
            && lab.world.tanks().any(|t| {
                t.team != lab.world.team()
                    && t.health > 0.0
                    && (!lab.fog_enabled
                        || lab.fog.as_ref().is_some_and(|fog| {
                            // Use the same live silhouettes the player sees. Dim
                            // remembered silhouettes never trigger battle music.
                            fog.scene()
                                .bodies
                                .iter()
                                .any(|b| b.id == t.id && b.opacity > 0.32)
                        }))
            });
        self.director
            .update(dt, visible, in_match && result.is_none(), paused);
        let master = self.settings.master_gain();
        let music = self.settings.music_gain()
            * master
            * if paused || result.is_some() {
                0.55
            } else {
                1.0
            };
        let fx = self.settings.effects_gain() * master;
        let score = self.director.gains();
        let speeds = lab.world.track_speeds();
        let movement = ((speeds[0].abs() + speeds[1].abs()) / 9.0).clamp(0.0, 1.0);
        blend_track_surface(
            &mut self.terrain_tracks,
            lab.world.terrain_at(lab.world.launcher_position()),
            dt,
        );
        let throttle = crate::controls::tracks()
            .iter()
            .map(|v| v.abs())
            .sum::<f32>()
            * 0.5;
        let engine = if live && lab.world.tank_alive() {
            fx
        } else {
            0.0
        };
        let listener = lab.world.launcher_position();
        let mut beam: f32 = 0.0;
        let mut charge: f32 = 0.0;
        if live {
            for weapon in lab.world.weapon_effects() {
                let gain = distance_gain(weapon.muzzle.distance(listener));
                if weapon.laser.is_some() {
                    beam = beam.max(fx * 0.16 * gain);
                }
                if weapon.charging > 0.0 {
                    charge = charge.max(fx * (0.10 + (1.0 - weapon.charging) * 0.16) * gain);
                }
            }
        }
        let charging = charge > 0.0;
        if charging && !self.charging {
            play_sound(
                &self.sounds[Clip::Charge as usize],
                PlaySoundParams {
                    looped: true,
                    volume: 0.0,
                },
            );
        } else if !charging && self.charging {
            stop_sound(&self.sounds[Clip::Charge as usize]);
        }
        self.charging = charging;
        let targets = [
            score[0] * music * 0.27,
            score[1] * music * 0.25,
            engine * 0.12 * (1.0 - movement * 0.65),
            engine * 0.16 * (movement.max(throttle * 0.6)),
            engine * self.terrain_tracks[0] * movement,
            beam,
            charge,
            engine * self.terrain_tracks[1] * movement,
            engine * self.terrain_tracks[2] * movement,
            engine * self.terrain_tracks[3] * movement,
            engine * self.terrain_tracks[4] * movement,
        ];
        for (i, (&clip, &target)) in LOOPS.iter().zip(&targets).enumerate() {
            self.volumes[i] = if self.settings.muted {
                0.0
            } else {
                approach(self.volumes[i], target, dt * 0.8)
            };
            set_sound_volume(&self.sounds[clip as usize], self.volumes[i]);
        }
        if game.screen == game::Screen::Match {
            let countdown = game.countdown.ceil() as i32;
            if !game.menu && countdown != self.countdown {
                let clip = if countdown > 0 {
                    Clip::Countdown
                } else {
                    Clip::Go
                };
                self.play(clip, 0.25 * master);
            }
            self.countdown = countdown;
            if let Some(result) = result
                && !self.finished
            {
                self.finished = true;
                let clip = if matches!(result, sim::matchplay::Outcome::Captured(team) if team == lab.world.team())
                {
                    Clip::Victory
                } else {
                    Clip::Defeat
                };
                self.play(clip, 0.5 * master);
            }
        }
        // Coalesce an entire rendered frame: many fractured cells are one crash.
        let listener = lab.world.launcher_position();
        let mut strongest = [0.0_f32; COUNT];
        for cue in lab.audio_cues.drain(..) {
            if gameplay {
                let gain = cue.gain * distance_gain((cue.position - listener).length());
                strongest[cue.clip as usize] = strongest[cue.clip as usize].max(gain);
            }
        }
        let mut budget = 6;
        for (index, strength) in strongest.into_iter().enumerate() {
            let category_gain = if [
                Clip::Pickup,
                Clip::Reload,
                Clip::Respawn,
                Clip::Victory,
                Clip::Defeat,
                Clip::Countdown,
                Clip::Go,
            ]
            .iter()
            .any(|clip| *clip as usize == index)
            {
                master * self.settings.feedback_gain()
            } else {
                fx
            };
            if strength > 0.01 && self.cooldowns[index] <= 0.0 && budget > 0 && category_gain > 0.0
            {
                play_sound(
                    &self.sounds[index],
                    PlaySoundParams {
                        looped: false,
                        volume: strength * category_gain * 0.42,
                    },
                );
                self.played += 1;
                budget -= 1;
                self.cooldowns[index] =
                    if [Clip::BarrelCrush as usize, Clip::FenceCrunch as usize].contains(&index) {
                        0.28
                    } else if index == Clip::Collision as usize {
                        0.2
                    } else if [
                        Clip::ShrubBreak as usize,
                        Clip::CrateBreak as usize,
                        Clip::TimberBreak as usize,
                    ]
                    .contains(&index)
                    {
                        0.16
                    } else {
                        0.09
                    };
            }
        }
        #[cfg(target_arch = "wasm32")]
        diagnostics::publish(self, visible);
    }
    fn play(&mut self, clip: Clip, gain: f32) {
        let gain = gain
            * if clip.feedback() {
                self.settings.feedback_gain()
            } else {
                1.0
            };
        if gain > 0.0 {
            play_sound(
                &self.sounds[clip as usize],
                PlaySoundParams {
                    looped: false,
                    volume: gain,
                },
            );
            self.played += 1;
        }
    }
    fn stop_effects(&self) {
        for (index, sound) in self.sounds.iter().enumerate() {
            if !LOOPS.iter().any(|clip| *clip as usize == index) {
                stop_sound(sound);
            }
        }
    }
    pub(crate) fn draw_controls(&mut self, size: Vec2, mouse: Vec2, always: bool) {
        if !always && self.notice <= 0.0 {
            return;
        }
        let settings = Rect::new(size.x - 260.0, size.y - 38.0, 240.0, 34.0);
        if crate::ui::button(
            settings,
            crate::controller_ui::prompt("SOUND SETTINGS / F7", "SOUND SETTINGS / Y"),
            mouse,
            false,
        ) {
            self.panel_open = true;
        }
        if crate::controller_ui::active() || size.x < 1400.0 {
            return;
        }
        let labels = [
            format!(
                "F8 SOUND {}",
                if self.settings.muted { "OFF" } else { "ON" }
            ),
            format!("F9 MUSIC {}%", self.settings.music),
            format!("F10 EFFECTS {}%", self.settings.effects),
        ];
        for (slot, label) in labels.iter().enumerate() {
            let rect = Rect::new(
                size.x * 0.5 - 294.0 + slot as f32 * 196.0,
                size.y - 30.0,
                192.0,
                26.0,
            );
            let hovered = rect.contains(mouse);
            draw_rectangle(
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                if hovered {
                    crate::ui::RULE
                } else {
                    crate::ui::PAPER
                },
            );
            crate::ui::text(label, rect.x + 8.0, rect.y + 18.0, 14.0, crate::ui::MUTED);
            if hovered && is_mouse_button_pressed(MouseButton::Left) {
                self.settings.change(slot);
                self.settings.save();
                self.stop_effects();
                self.notice = 3.0;
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod diagnostics {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
    static BLEND: AtomicU32 = AtomicU32::new(0);
    static VISIBLE: AtomicU32 = AtomicU32::new(0);
    static PLAYED: AtomicU32 = AtomicU32::new(0);
    static MUTED: AtomicU32 = AtomicU32::new(0);
    static ENGINE: AtomicU32 = AtomicU32::new(0);
    static SETTINGS: AtomicU32 = AtomicU32::new(0);
    pub fn publish(audio: &Audio, visible: bool) {
        SETTINGS.store(audio.panel_open as u32, Relaxed);
        BLEND.store(audio.director.blend.to_bits(), Relaxed);
        VISIBLE.store(visible as u32, Relaxed);
        PLAYED.store(audio.played, Relaxed);
        MUTED.store(audio.settings.muted as u32, Relaxed);
        ENGINE.store(audio.volumes[3].to_bits(), Relaxed);
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn audio_settings_open() -> u32 {
        SETTINGS.load(Relaxed)
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn audio_battle_mix() -> f32 {
        f32::from_bits(BLEND.load(Relaxed))
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn audio_enemy_visible() -> u32 {
        VISIBLE.load(Relaxed)
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn audio_effects_played() -> u32 {
        PLAYED.load(Relaxed)
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn audio_muted() -> u32 {
        MUTED.load(Relaxed)
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn audio_engine_gain() -> f32 {
        f32::from_bits(ENGINE.load(Relaxed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decorative_destruction_uses_quiet_material_cues_and_coalesces() {
        let mut cues = Vec::new();
        for kind in [
            sim::DecorationKind::Shrub,
            sim::DecorationKind::Crate,
            sim::DecorationKind::Timber,
        ] {
            for _ in 0..20 {
                let cue = Cue::event(
                    &Event::DecorationDestroyed {
                        kind,
                        position: sim::Vec2::ZERO,
                    },
                    sim::level::Team::Blue,
                )
                .unwrap();
                assert!(cue.gain > 0.0 && cue.gain <= 0.28);
                assert!(!cue.clip.feedback());
                cue.enqueue(&mut cues, sim::Vec2::ZERO);
            }
        }
        assert_eq!(
            cues.len(),
            3,
            "one cue per material even for a cluster of props"
        );
        assert_eq!(
            cues.iter().map(|cue| cue.clip).collect::<Vec<_>>(),
            vec![Clip::ShrubBreak, Clip::CrateBreak, Clip::TimberBreak]
        );
    }
    #[test]
    fn terrain_textures_blend_without_boundary_jumps_or_frame_rate_dependence() {
        use sim::TerrainKind::*;
        let mut gains = terrain_track_gains(Ground);
        blend_track_surface(&mut gains, Road, 1.0 / 60.0);
        assert!(gains[0] < 0.14 && gains[0] > 0.11);
        assert!(gains[1] > 0.0 && gains[1] < 0.045);
        // Changing direction on the boundary blends both remaining tails.
        blend_track_surface(&mut gains, Sand, 1.0 / 60.0);
        assert!(gains[1] > 0.0 && gains[2] > 0.0);
        for surface in [
            Road, Sand, TallGrass, Bridge, Elevation, Ground, Water, Cliff,
        ] {
            let mut coarse = gains;
            let mut fine = gains;
            for _ in 0..30 {
                blend_track_surface(&mut coarse, surface, 1.0 / 30.0);
            }
            for _ in 0..120 {
                blend_track_surface(&mut fine, surface, 1.0 / 120.0);
            }
            for ((a, b), target) in coarse
                .into_iter()
                .zip(fine)
                .zip(terrain_track_gains(surface))
            {
                assert!((a - b).abs() < 1e-6);
                assert!((a - target).abs() < 0.0001);
                assert!(a >= 0.0);
            }
            assert!(fine.iter().sum::<f32>() < 0.16, "surface mix stays subtle");
        }
    }
    #[test]
    fn music_holds_contact_crossfades_and_exits() {
        let mut m = MusicDirector::default();
        m.update(0.5, true, true, false);
        assert!((m.blend - 0.25).abs() < 1e-5);
        for _ in 0..4 {
            m.update(0.5, true, true, false);
        }
        assert_eq!(m.blend, 1.0);
        m.update(2.0, false, true, false);
        assert_eq!(m.blend, 1.0);
        m.update(9.0, false, true, true);
        assert_eq!(m.blend, 1.0);
        assert_eq!(m.hold, 2.0);
        m.update(3.0, false, false, false);
        assert_eq!(m.blend, 0.0);
        for i in 0..11 {
            m.blend = i as f32 / 10.0;
            let g = m.gains();
            assert!((g[0] * g[0] + g[1] * g[1] - 1.0).abs() < 1e-5);
        }
    }
    #[test]
    fn each_projectile_has_its_own_sound_and_distant_events_are_inaudible() {
        let mut clips = vec![];
        for ammunition in [
            Ammunition::HighExplosive,
            Ammunition::ArmorPiercing,
            Ammunition::Rocket,
            Ammunition::Plasma,
        ] {
            let cue = Cue::event(
                &Event::Fired {
                    ammunition,
                    position: sim::Vec2::ZERO,
                    velocity: sim::Vec2::ZERO,
                },
                sim::level::Team::Blue,
            )
            .unwrap();
            assert!(!clips.contains(&cue.clip));
            clips.push(cue.clip);
        }
        assert_eq!(distance_gain(0.0), 1.0);
        assert_eq!(distance_gain(45.0), 0.0);
        assert!(distance_gain(20.0) < distance_gain(5.0));
    }
    #[test]
    fn many_fractures_coalesce_without_dropping_weapon_cues() {
        let mut cues = Vec::new();
        for i in (0..1000).rev() {
            Cue {
                clip: Clip::Fracture,
                position: sim::Vec2::new(i as f32, 0.0),
                gain: 1.0,
            }
            .enqueue(&mut cues, sim::Vec2::ZERO);
        }
        Cue {
            clip: Clip::Plasma,
            position: sim::Vec2::ZERO,
            gain: 1.0,
        }
        .enqueue(&mut cues, sim::Vec2::ZERO);
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].position, sim::Vec2::ZERO);
        assert_eq!(cues[1].clip, Clip::Plasma);
    }
    #[test]
    fn crushing_materials_have_distinct_cues_and_notifications_are_subdued() {
        let team = sim::level::Team::Blue;
        let barrel = Cue::event(
            &Event::Buckled {
                position: sim::Vec2::ZERO,
                work: 100.0,
            },
            team,
        )
        .unwrap();
        let fence = Cue::event(
            &Event::Bent {
                position: sim::Vec2::ZERO,
                work: 100.0,
            },
            team,
        )
        .unwrap();
        assert_eq!(barrel.clip, Clip::BarrelCrush);
        assert_eq!(fence.clip, Clip::FenceCrunch);
        let world = sim::World::new(sim::Scene::Masonry);
        assert!(Cue::local(Clip::Pickup, &world).gain <= 0.35);
        assert!(Cue::local(Clip::Reload, &world).gain <= 0.22);
    }
}
