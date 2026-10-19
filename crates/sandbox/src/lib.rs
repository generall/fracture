#[cfg(all(feature = "ai-arena", not(target_arch = "wasm32")))]
mod arena;
mod audio;
#[cfg(all(feature = "ai-arena", not(target_arch = "wasm32")))]
pub use arena::{run as run_ai_arena, window_conf as ai_arena_window_conf};
mod comparison;
mod comparison_storage;
mod controller;
mod controller_ui;
mod controls;
#[cfg(target_arch = "wasm32")]
mod diagnostics;
mod editor;
mod editor_storage;
#[cfg(not(target_arch = "wasm32"))]
mod file_dialog;
mod fog;
mod game;
mod game_storage;
mod lab;
#[cfg(not(target_arch = "wasm32"))]
mod lan;
mod level_editor;
mod level_storage;
mod navigation;
mod object_library;
#[cfg(all(feature = "performance-test", not(target_arch = "wasm32")))]
mod performance;
mod terrain_view;
mod track_marks;
mod ui;
mod view;
mod wreck_effects;

use lab::Lab;
use macroquad::prelude::*;
use sim::{Scene, TICK_DT};
use ui::Action;
use view::Viewport;

/// The executable selects its application; flags and URLs cannot switch it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Application {
    Game,
    Sandbox,
}

pub fn window_conf(application: Application) -> Conf {
    Conf {
        window_title: match application {
            Application::Game => "Fracture — tank combat",
            Application::Sandbox => "Fracture — sandbox & editors",
        }
        .into(),
        window_width: if deck_mode() { 1280 } else { 1440 },
        window_height: if deck_mode() { 800 } else { 900 },
        fullscreen: deck_mode(),
        high_dpi: true,
        sample_count: 4,
        ..Default::default()
    }
}

fn deck_mode() -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var("SteamDeck").as_deref() == Ok("1") || std::env::args().any(|a| a == "--deck")
    }
    #[cfg(target_arch = "wasm32")]
    {
        // Browser preview/test of the explicitly selected handheld controls.
        #[link(wasm_import_module = "env")]
        unsafe extern "C" {
            fn controller_handheld_mode() -> u32;
        }
        unsafe { controller_handheld_mode() != 0 }
    }
}

pub async fn run(application: Application) {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(directory) = std::env::var_os("FRACTURE_DATA_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).expect("create game save directory");
        std::env::set_current_dir(&directory).expect("open game save directory");
    }
    let mut font = load_ttf_font_from_bytes(include_bytes!("../assets/LiberationMono-Bold.ttf"))
        .expect("bundled UI font");
    font.set_filter(FilterMode::Linear);
    macroquad::text::set_default_font(font);
    let mut audio = audio::Audio::load().await;
    let mut lab = Lab::new();
    let mut controller = controller::Controller::new();
    let mut editor = editor::Editor::new();
    let mut map_editor = level_editor::LevelEditor::new();
    let mut comparison = match if application == Application::Sandbox {
        comparison_storage::startup()
    } else {
        0
    } {
        1 => Some(comparison::Comparison::barrels()),
        2 => Some(comparison::Comparison::crush()),
        3 => Some(comparison::Comparison::barrel_shot(true)),
        4 => Some(comparison::Comparison::barrel_shot(false)),
        5 => Some(comparison::Comparison::barrel_release()),
        6 => Some(comparison::Comparison::fences()),
        _ => None,
    };
    let mut game = game::Game::new(application == Application::Sandbox);
    #[cfg(all(feature = "performance-test", not(target_arch = "wasm32")))]
    let mut performance = performance::Run::from_environment(&mut game, &mut lab);
    let mut editing_comparison = false;
    let mut first_frame = true;
    let mut fullscreen = deck_mode();
    let handheld = deck_mode();
    let mut cursor_visible = true;
    loop {
        #[cfg(not(target_arch = "wasm32"))]
        if game.exit_requested {
            break;
        }
        #[cfg(all(feature = "performance-test", not(target_arch = "wasm32")))]
        if let Some(run) = &mut performance
            && run.begin_frame(&mut lab)
        {
            break;
        }
        // Loading and font initialization are not elapsed simulation time.
        let wall_dt = if first_frame {
            first_frame = false;
            0.0
        } else {
            get_frame_time().min(0.25)
        };
        let pad = controller.poll(wall_dt, handheld);
        if is_key_pressed(KeyCode::F11) {
            fullscreen = !fullscreen;
            set_fullscreen(fullscreen);
        }
        let game_ui = game.screen != game::Screen::Sandbox;
        let scale = if game_ui && (screen_width() < 1440.0 || screen_height() < 900.0) {
            (screen_width() / 1280.0)
                .min(screen_height() / 720.0)
                .clamp(0.25, 1.0)
        } else {
            (screen_width() / 1440.0)
                .min(screen_height() / 900.0)
                .max(0.25)
        };
        let size = vec2(screen_width() / scale, screen_height() / scale);
        let pointer_editor =
            editor.active || map_editor.active || comparison.as_ref().is_some_and(|c| c.active);
        let pointer_enabled = !handheld || !controller.active || pointer_editor;
        let pointer_activated = !cursor_visible && pointer_enabled;
        if cursor_visible != pointer_enabled {
            show_mouse(pointer_enabled);
            cursor_visible = pointer_enabled;
        }
        // A hidden pointer cannot hover UI, steer the camera or aim/fire.
        let raw_mouse = Vec2::from(mouse_position()) / scale;
        let mouse = if pointer_enabled {
            raw_mouse
        } else {
            Vec2::splat(f32::MIN)
        };
        #[cfg(not(target_arch = "wasm32"))]
        game.poll_lan(&mut lab, wall_dt);
        let gameplay = game.screen == game::Screen::Match
            || (game.screen == game::Screen::Sandbox
                && !map_editor.active
                && !editor.active
                && !comparison.as_ref().is_some_and(|c| c.active));
        if game.screen == game::Screen::Match && (pad.disconnected || get_frame_time() > 1.0) {
            game.menu = true;
            lab.pending.clear();
            lab.firing = false;
        }
        let sound_settings = audio.settings_frame(pad);
        let outcome = lab
            .match_progress
            .as_ref()
            .is_some_and(|p| p.outcome.is_some());
        let deploying = (game.screen == game::Screen::Match && !lab.world.tank_alive())
            || (gameplay && lab.world.player_choosing_vehicle());
        let context = game.screen as u64 * 32
            + u64::from(game.menu)
            + u64::from(outcome) * 2
            + u64::from(sound_settings) * 4
            + u64::from(deploying) * 8;
        let menu_navigation = sound_settings
            || matches!(
                game.screen,
                game::Screen::Home | game::Screen::Lobby | game::Screen::Lan
            )
            || deploying
            || (game.screen == game::Screen::Match && (game.menu || outcome));
        controller_ui::begin(
            context,
            controller.active,
            if handheld {
                menu_navigation
            } else {
                game_ui || sound_settings || deploying
            },
            pad,
        );
        audio.update(wall_dt, &mut lab, &game, gameplay);
        if sound_settings {
            #[cfg(not(target_arch = "wasm32"))]
            game.lan_input(&mut lab, false, [0.0; 2]);
            lab.pending.clear();
            lab.firing = false;
            lab.accumulator = 0.0;
            clear_background(ui::PAPER);
            let mut camera = Camera2D::from_display_rect(Rect::new(0.0, 0.0, size.x, size.y));
            camera.zoom.y = -camera.zoom.y;
            set_camera(&camera);
            audio.draw_settings(size, mouse);
            set_default_camera();
            next_frame().await;
            continue;
        }
        #[cfg(target_arch = "wasm32")]
        game::publish(&game, &lab);
        if matches!(
            game.screen,
            game::Screen::Home | game::Screen::Lobby | game::Screen::Lan
        ) {
            editor::publish_closed();
            comparison::publish_closed();
            level_editor::publish_closed();
            clear_background(ui::PAPER);
            let mut camera = Camera2D::from_display_rect(Rect::new(0.0, 0.0, size.x, size.y));
            camera.zoom.y = -camera.zoom.y;
            set_camera(&camera);
            if let Some(action) = game.draw_menu(size, mouse) {
                apply_game_action(action, &mut game, &mut lab);
            }
            audio.draw_controls(
                size,
                mouse,
                matches!(game.screen, game::Screen::Home | game::Screen::Lobby),
            );
            controller_ui::draw_keyboard(size, mouse);
            set_default_camera();
            next_frame().await;
            continue;
        }
        let proper_match = game.screen == game::Screen::Match;
        if proper_match {
            if (is_key_pressed(KeyCode::Escape)
                || pad.pressed(controller::MENU)
                || (game.menu && pad.pressed(controller::BACK)))
                && lab
                    .match_progress
                    .as_ref()
                    .is_some_and(|p| p.outcome.is_none())
            {
                game.menu = !game.menu;
            }
            #[cfg(all(feature = "performance-test", not(target_arch = "wasm32")))]
            if performance.is_some() {
                // The scripted fixture runs without manual resume after window
                // activation; normal launches retain disconnect/suspend pauses.
                game.menu = false;
            }
            game.advance(wall_dt, &mut lab);
        }
        if !proper_match
            && !map_editor.active
            && !editor.active
            && !comparison.as_ref().is_some_and(|c| c.active)
            && is_key_pressed(KeyCode::M)
        {
            map_editor.open();
            lab.pending.clear();
            lab.firing = false;
            lab.accumulator = 0.0;
            next_frame().await;
            continue;
        }
        if map_editor.active {
            editor::publish_closed();
            comparison::publish_closed();
            clear_background(ui::PAPER);
            let mut camera = Camera2D::from_display_rect(Rect::new(0.0, 0.0, size.x, size.y));
            camera.zoom.y = -camera.zoom.y;
            set_camera(&camera);
            if let Some(action) = map_editor.draw(size, mouse, wall_dt, scale * screen_dpi_scale())
            {
                match action {
                    level_editor::Action::Close => {}
                    level_editor::Action::Play(level, team) => {
                        lab.custom_level = Some((*level, team));
                        lab.reset(Scene::Level);
                        lab.paused = false;
                        lab.slow = false;
                    }
                }
                map_editor.active = false;
                lab.accumulator = 0.0;
                lab.pending.clear();
                lab.firing = false;
                lab.last_mouse = mouse;
            }
            audio.draw_controls(
                size,
                mouse,
                matches!(game.screen, game::Screen::Home | game::Screen::Lobby),
            );
            set_default_camera();
            next_frame().await;
            continue;
        }
        level_editor::publish_closed();
        if let Some(range) = comparison.as_mut().filter(|r| r.active) {
            editor::publish_closed();
            clear_background(ui::PAPER);
            let mut camera = Camera2D::from_display_rect(Rect::new(0.0, 0.0, size.x, size.y));
            camera.zoom.y = -camera.zoom.y;
            set_camera(&camera);
            if let Some(action) = range.draw(size, mouse, wall_dt, scale * screen_dpi_scale()) {
                range.active = false;
                match action {
                    comparison::Action::Close => {
                        editor.use_variant(range.selected_object());
                        editing_comparison = true;
                    }
                    comparison::Action::Use(asset) => {
                        editor.use_variant(asset);
                        editing_comparison = false;
                    }
                    comparison::Action::Edit(asset) => {
                        editor.use_variant(asset);
                        editing_comparison = true;
                    }
                }
                lab.accumulator = 0.0;
                lab.last_mouse = mouse;
            }
            audio.draw_controls(
                size,
                mouse,
                matches!(game.screen, game::Screen::Home | game::Screen::Lobby),
            );
            set_default_camera();
            next_frame().await;
            continue;
        }
        comparison::publish_closed();
        if !proper_match && !editor.active && is_key_pressed(KeyCode::O) {
            editor.open();
            lab.accumulator = 0.0;
            lab.pending.clear();
            // Do not let the opening key close the editor in the same frame.
            next_frame().await;
            continue;
        }
        if editor.active {
            clear_background(ui::PAPER);
            let mut camera = Camera2D::from_display_rect(Rect::new(0.0, 0.0, size.x, size.y));
            camera.zoom.y = -camera.zoom.y;
            set_camera(&camera);
            if let Some(action) = editor.draw(size, mouse, wall_dt, scale * screen_dpi_scale()) {
                match action {
                    editor::EditorAction::Close => editor.active = false,
                    editor::EditorAction::BarrelShot => {
                        comparison = Some(comparison::Comparison::barrel_shot(true));
                        editing_comparison = false;
                        editor.active = false;
                    }
                    editor::EditorAction::Crush => {
                        comparison = Some(comparison::Comparison::crush());
                        editing_comparison = false;
                        editor.active = false;
                    }
                    editor::EditorAction::Yard => {
                        lab.reset(Scene::ObjectYard);
                        lab.paused = false;
                        lab.slow = false;
                        editor.active = false;
                        editing_comparison = false;
                    }
                    editor::EditorAction::Compare(asset) => {
                        if editing_comparison {
                            if let Some(range) = &mut comparison {
                                range.replace_selected(asset);
                            } else {
                                comparison = Some(comparison::Comparison::new(asset));
                            }
                        } else {
                            comparison = Some(comparison::Comparison::new(asset));
                        }
                        editing_comparison = false;
                        editor.active = false;
                    }
                    editor::EditorAction::Test(asset) => {
                        lab.custom_object = Some(asset);
                        lab.reset(Scene::Custom);
                        lab.paused = false;
                        lab.slow = false;
                        editor.active = false;
                    }
                }
                lab.last_mouse = mouse;
                lab.accumulator = 0.0;
            }
            audio.draw_controls(
                size,
                mouse,
                matches!(game.screen, game::Screen::Home | game::Screen::Lobby),
            );
            set_default_camera();
            next_frame().await;
            continue;
        }
        editor::publish_closed();
        let chamber = if proper_match {
            let full = Rect::new(0.0, 0.0, size.x, size.y);
            lab.navigation.expand_game_view(full);
            full
        } else {
            Rect::new(0.0, 80.0, size.x, size.y - 177.0)
        };
        let mut viewport;
        if is_key_pressed(KeyCode::H) || pad.pressed(controller::HELP) {
            if proper_match {
                game.help = !game.help;
            } else {
                lab.inspector.help = !lab.inspector.help;
            }
        }
        let accepts_input = if proper_match {
            game.input_allowed(&lab)
        } else {
            !lab.world.player_choosing_vehicle()
        };
        let in_chamber = accepts_input
            && chamber.contains(mouse)
            && if proper_match {
                !game.captures(mouse)
            } else {
                !lab.inspector.captures(mouse, size)
            };
        let game_camera = lab.world.scene() == Scene::Level;

        if in_chamber {
            if !game_camera && is_mouse_button_down(MouseButton::Middle) {
                lab.navigation.pan(mouse - lab.last_mouse, chamber);
            }
            let wheel = mouse_wheel().1;
            if wheel != 0.0 {
                if game_camera {
                    lab.navigation.scroll_game(
                        wheel,
                        chamber,
                        lab.world.player_sight_range(),
                        lab.world.player_in_radar_zone(),
                    );
                } else {
                    lab.navigation.scroll(wheel, mouse, chamber);
                }
            }
        }
        if accepts_input
            && game_camera
            && pad.navigation.y != 0.0
            && (pad.held(controller::UP) || pad.held(controller::DOWN))
        {
            lab.navigation.scroll_game(
                -pad.navigation.y,
                chamber,
                lab.world.player_sight_range(),
                lab.world.player_in_radar_zone(),
            );
        }
        if game_camera {
            lab.navigation.advance_game_zoom(
                wall_dt,
                chamber,
                lab.world.player_sight_range(),
                lab.world.player_in_radar_zone(),
            );
        } else {
            lab.navigation.advance(wall_dt, chamber);
        }
        viewport = Viewport::new(chamber, lab.navigation.center, lab.navigation.zoom);
        if !proper_match && is_key_pressed(KeyCode::P) {
            lab.paused = !lab.paused;
            lab.accumulator = 0.0;
        }
        if !proper_match && is_key_pressed(KeyCode::T) {
            lab.slow = !lab.slow;
        }
        if !proper_match && is_key_pressed(KeyCode::Tab) {
            lab.overlay = !lab.overlay;
        }
        if !proper_match && is_key_pressed(KeyCode::V) && lab.world.scene() == Scene::Level {
            lab.fog_enabled = !lab.fog_enabled;
        }
        if !proper_match && is_key_pressed(KeyCode::U) {
            lab.choose_sandbox_vehicle();
        }
        if !proper_match && is_key_pressed(KeyCode::R) {
            lab.reset(lab.world.scene());
        }
        if is_key_pressed(KeyCode::Home) {
            lab.fit_camera();
        }
        for binding in controls::SCENES {
            if !proper_match && is_key_pressed(binding.key) {
                lab.reset(binding.scene);
            }
        }
        for (key, ammunition) in [KeyCode::Key5, KeyCode::Key6]
            .into_iter()
            .zip(sim::Ammunition::MAIN)
        {
            if accepts_input && is_key_pressed(key) {
                lab.select_ammunition(ammunition);
            }
        }
        if accepts_input && (is_key_pressed(KeyCode::B) || pad.pressed(controller::AMMO)) {
            lab.select_ammunition(lab.inspector.ammunition.next());
        }
        if accepts_input {
            let aim_dt = if lab.paused {
                0.0
            } else {
                wall_dt * if lab.slow { 0.2 } else { 1.0 }
            };
            lab.keyboard_aim.update(
                controls::turret_turn(),
                in_chamber && (pointer_activated || mouse != lab.last_mouse) && !controller.active,
                lab.world.player_aim().angle,
                aim_dt,
            );
            if controller.active && !lab.keyboard_aim.active() {
                lab.keyboard_aim
                    .point_stick(Vec2::from_angle(lab.world.player_aim().angle));
            }
            lab.keyboard_aim.point_stick(pad.aim);
            if lab.world.player_vehicle() == sim::VehicleKind::Mortar {
                lab.mortar_range =
                    controls::advance_mortar_range(lab.mortar_range, pad.aim, aim_dt);
                if let Some(turn) = controls::turret_turn() {
                    lab.mortar_range = (lab.mortar_range + turn * aim_dt * 8.0).clamp(6.0, 36.0);
                }
            }
        }
        if in_chamber && !lab.keyboard_aim.active() {
            lab.aim = viewport.world(mouse);
        }
        lab.refresh_keyboard_aim();
        lab.last_mouse = raw_mouse;
        lab.firing = accepts_input
            && ((in_chamber && is_mouse_button_down(MouseButton::Left))
                || is_key_down(KeyCode::F)
                || is_key_down(KeyCode::Space)
                || pad.held(controller::FIRE));
        if accepts_input
            && ((in_chamber && is_mouse_button_pressed(MouseButton::Left))
                || is_key_pressed(KeyCode::F)
                || is_key_pressed(KeyCode::Space)
                || pad.pressed(controller::FIRE))
        {
            lab.fire();
        }
        if accepts_input
            && ((in_chamber && is_mouse_button_pressed(MouseButton::Right))
                || is_key_pressed(KeyCode::G)
                || pad.pressed(controller::SPECIAL))
        {
            lab.pending
                .retain(|c| !matches!(c, sim::Command::ActivateSpecial));
            lab.pending.push(sim::Command::ActivateSpecial);
        }
        let single_step = !proper_match && is_key_pressed(KeyCode::N);

        clear_background(ui::PAPER);
        let mut camera = Camera2D::from_display_rect(Rect::new(0.0, 0.0, size.x, size.y));
        // Macroquad flips the screen render target relative to display_rect.
        // Our world and UI both use screen coordinates with positive Y downward.
        camera.zoom.y = -camera.zoom.y;
        set_camera(&camera);
        let tracks = if !accepts_input {
            [0.0; 2]
        } else if pad.drive != Vec2::ZERO {
            lab.world
                .bodies()
                .find(|b| b.id == lab.world.player_tank_id())
                .map_or([0.0; 2], |tank| {
                    if lab.world.player_vehicle() == sim::VehicleKind::Scout {
                        controls::stick_car(pad.drive, tank.angle)
                    } else {
                        controls::stick_tracks(pad.drive, tank.angle)
                    }
                })
        } else {
            controls::tracks(lab.world.player_vehicle())
        };
        #[cfg(all(feature = "performance-test", not(target_arch = "wasm32")))]
        let tracks = performance
            .as_mut()
            .map_or(tracks, |run| run.input(&mut lab));
        #[cfg(all(feature = "performance-test", not(target_arch = "wasm32")))]
        if let Some(run) = &mut performance {
            run.mark("input");
        }
        if lab.world.is_replica() {
            #[cfg(not(target_arch = "wasm32"))]
            game.lan_input(&mut lab, accepts_input, tracks);
            lab.advance_effects(wall_dt);
        } else if single_step {
            lab.paused = true;
            lab.accumulator = 0.0;
            lab.tick(tracks);
        } else if !lab.paused {
            lab.accumulator += f64::from(wall_dt) * if lab.slow { 0.2 } else { 1.0 };
            let mut steps = 0;
            // A costly impact must not cause a cascade of catch-up frames.
            // Finish every physics tick; cap only extra wall-time work in a
            // match. Material experiments retain their existing step policy.
            let deadline = if proper_match {
                get_time() + 0.012
            } else {
                f64::INFINITY
            };
            while lab.accumulator >= f64::from(TICK_DT)
                && steps < 12
                && (steps == 0 || get_time() < deadline)
            {
                lab.tick(tracks);
                lab.accumulator -= f64::from(TICK_DT);
                steps += 1;
            }
            if lab.accumulator >= f64::from(TICK_DT) {
                lab.accumulator %= f64::from(TICK_DT);
                lab.overloaded = 2.0;
            }
        }
        lab.overloaded = (lab.overloaded - wall_dt).max(0.0);
        if let Some((_, remaining)) = &mut lab.notice {
            *remaining -= wall_dt;
            if *remaining <= 0.0 {
                lab.notice = None;
            }
        }

        #[cfg(all(feature = "performance-test", not(target_arch = "wasm32")))]
        if let Some(run) = &mut performance {
            run.mark("physics");
        }
        let replica_alpha = {
            #[cfg(not(target_arch = "wasm32"))]
            {
                game.lan.as_ref().map_or(1.0, |lan| lan.alpha())
            }
            #[cfg(target_arch = "wasm32")]
            {
                1.0
            }
        };
        let alpha = if lab.world.is_replica() {
            replica_alpha
        } else if lab.paused {
            1.0
        } else {
            (lab.accumulator / f64::from(TICK_DT)) as f32
        };
        if lab.world.scene() == Scene::Level {
            // Update after physics, using the same interpolated pose as drawing.
            let tank = lab
                .world
                .bodies()
                .find(|b| b.id == lab.world.player_tank_id())
                .unwrap();
            let position = Vec2::new(tank.previous_position.x, tank.previous_position.y)
                .lerp(Vec2::new(tank.position.x, tank.position.y), alpha);
            lab.navigation.advance_game_zoom(
                0.0,
                chamber,
                lab.world.player_sight_range(),
                lab.world.player_in_radar_zone(),
            );
            lab.navigation.follow_tank(
                position,
                if lab.keyboard_aim.active() {
                    Some(navigation::LookAhead::Direction(Vec2::from_angle(
                        lab.world.player_aim().angle,
                    )))
                } else {
                    in_chamber.then_some(navigation::LookAhead::Pointer(mouse))
                },
                wall_dt,
                chamber,
            );
        }
        viewport = Viewport::new(chamber, lab.navigation.center, lab.navigation.zoom);
        if in_chamber && !lab.keyboard_aim.active() {
            lab.aim = viewport.world(mouse);
        }
        lab.refresh_keyboard_aim();
        #[cfg(all(feature = "performance-test", not(target_arch = "wasm32")))]
        if let Some(run) = &mut performance {
            run.mark("camera");
        }
        let fog_active = lab.fog_enabled && lab.world.scene() == Scene::Level;
        if fog_active {
            lab.vision = lab.world.player_vision(lab.world.player_aim().angle);
        }
        #[cfg(all(feature = "performance-test", not(target_arch = "wasm32")))]
        if let Some(run) = &mut performance {
            run.mark("vision");
        }
        if fog_active {
            lab.fog.get_or_insert_with(fog::Fog::default).observe(
                &lab.world,
                &lab.vision,
                get_time(),
            );
        }
        #[cfg(all(feature = "performance-test", not(target_arch = "wasm32")))]
        if let Some(run) = &mut performance {
            run.mark("fog");
        }
        // One renderer and one screen-space scissor, with or without fog.
        let pixel_scale = scale * screen_dpi_scale();
        unsafe {
            get_internal_gl().quad_gl.scissor(Some((
                (chamber.x * pixel_scale) as i32,
                (chamber.y * pixel_scale) as i32,
                (chamber.w * pixel_scale) as i32,
                (chamber.h * pixel_scale) as i32,
            )));
        }
        view::draw_world(
            &lab.world,
            &viewport,
            (pointer_enabled
                || (lab.world.player_vehicle() == sim::VehicleKind::Mortar
                    && lab.world.tank_alive()))
            .then_some(lab.aim),
            alpha,
            lab.overlay,
            &lab.sparks,
            &lab.blasts,
            &lab.wreck_effects,
            &mut lab.track_marks,
            if fog_active {
                Some((lab.fog.as_ref().unwrap().scene(), &lab.vision))
            } else {
                None
            },
        );
        unsafe {
            get_internal_gl().quad_gl.scissor(None);
        }
        #[cfg(all(feature = "performance-test", not(target_arch = "wasm32")))]
        if let Some(run) = &mut performance {
            run.mark("draw");
        }
        let actions = if proper_match {
            if let Some(action) = game.draw_hud(size, mouse, &mut lab) {
                apply_game_action(action, &mut game, &mut lab);
            }
            vec![]
        } else if lab.world.player_choosing_vehicle() {
            vec![]
        } else {
            lab.inspector.draw(
                size,
                mouse,
                &lab.world,
                lab.paused,
                lab.slow,
                lab.overlay,
                lab.tick_ms,
                fog_active,
            )
        };
        for action in actions {
            match action {
                Action::Scene(scene) => lab.reset(scene),
                Action::Reset => lab.reset(lab.world.scene()),
                Action::Vehicle => lab.choose_sandbox_vehicle(),
                Action::Pause => {
                    lab.paused = !lab.paused;
                    lab.accumulator = 0.0;
                }
                Action::Slow => lab.slow = !lab.slow,
                Action::Overlay => lab.overlay = !lab.overlay,
                Action::Fog => lab.fog_enabled = !lab.fog_enabled,
                Action::Step => {
                    lab.paused = true;
                    lab.accumulator = 0.0;
                    lab.tick(tracks);
                }
                Action::Editor => {
                    editor.open();
                    lab.accumulator = 0.0;
                    lab.pending.clear();
                }
                Action::LevelEditor => {
                    map_editor.open();
                    lab.accumulator = 0.0;
                    lab.pending.clear();
                    lab.firing = false;
                }
                Action::Ammunition(ammunition) => lab.select_ammunition(ammunition),
            }
        }

        if !proper_match || game.input_allowed(&lab) {
            if let Some(team) = lab.world.winner() {
                ui::toast(
                    chamber,
                    &format!(
                        "{} VICTORY / ENEMY FLAG CAPTURED / R RESTART / M EDIT",
                        team.label()
                    ),
                );
            } else if !lab.world.tank_alive() {
                if let Some(seconds) = lab.world.player_respawn_remaining() {
                    ui::toast(
                        chamber,
                        &if seconds > 0.0 {
                            format!("TANK DESTROYED / RESPAWN IN {seconds:.1} S")
                        } else {
                            "WAITING FOR A CLEAR SPAWN".into()
                        },
                    );
                } else {
                    ui::toast(chamber, "TANK LOST IN WATER / R TO REBUILD");
                }
            } else if lab.overloaded > 0.0 {
                ui::toast(chamber, "Simulation catching up — elapsed time was capped.");
            } else if let Some((message, _)) = &lab.notice {
                ui::toast(chamber, message);
            }
        }
        if !proper_match && lab.world.player_choosing_vehicle() {
            game.draw_deployment(size, mouse, &mut lab);
        }
        #[cfg(target_arch = "wasm32")]
        {
            diagnostics::publish(&lab.world);
            diagnostics::publish_camera(&lab.navigation);
            diagnostics::publish_ammunition(lab.inspector.ammunition);
            diagnostics::publish_mortar_range(lab.mortar_range);
            diagnostics::publish_aim(lab.aim.x, lab.aim.y);
        }
        if !proper_match
            || game.menu
            || lab
                .match_progress
                .as_ref()
                .is_some_and(|p| p.outcome.is_some())
        {
            audio.draw_controls(size, mouse, proper_match);
        }
        #[cfg(all(feature = "performance-test", not(target_arch = "wasm32")))]
        if let Some(run) = &mut performance {
            run.mark("ui");
        }
        set_default_camera();
        next_frame().await;
    }
}

fn apply_game_action(action: game::Action, game: &mut game::Game, lab: &mut Lab) {
    #[cfg(not(target_arch = "wasm32"))]
    if game.lan_action(&action, lab) {
        return;
    }
    match action {
        game::Action::EnterLan => {
            #[cfg(not(target_arch = "wasm32"))]
            {
                // Gameplay keys can remain in Macroquad's text-input queue.
                while get_char_pressed().is_some() {}
                game.lan = Some(lan::Lan::default());
                game.screen = game::Screen::Lan;
            }
        }
        game::Action::EnterLobby => game.enter_lobby(),
        game::Action::Start => game.start(lab),
        game::Action::Resume => game.menu = false,
        game::Action::Leave => game.return_to_lobby(lab),
        #[cfg(not(target_arch = "wasm32"))]
        game::Action::Exit => game.exit_requested = true,
    }
    lab.pending.clear();
    lab.firing = false;
    lab.accumulator = 0.0;
}
