mod comparison;
mod comparison_storage;
mod controls;
#[cfg(target_arch = "wasm32")]
mod diagnostics;
mod editor;
mod editor_storage;
#[cfg(not(target_arch = "wasm32"))]
mod file_dialog;
mod fog;
mod lab;
mod level_editor;
mod level_storage;
mod navigation;
mod object_library;
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

fn window_conf() -> Conf {
    Conf {
        window_title: "Fracture — proving ground".into(),
        window_width: 1440,
        window_height: 900,
        high_dpi: true,
        sample_count: 4,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut font = load_ttf_font_from_bytes(include_bytes!("../assets/LiberationMono-Bold.ttf"))
        .expect("bundled UI font");
    font.set_filter(FilterMode::Linear);
    macroquad::text::set_default_font(font);
    let mut lab = Lab::new();
    let mut editor = editor::Editor::new();
    let mut map_editor = level_editor::LevelEditor::new();
    let mut comparison = match comparison_storage::startup() {
        1 => Some(comparison::Comparison::barrels()),
        2 => Some(comparison::Comparison::crush()),
        3 => Some(comparison::Comparison::barrel_shot(true)),
        4 => Some(comparison::Comparison::barrel_shot(false)),
        5 => Some(comparison::Comparison::barrel_release()),
        6 => Some(comparison::Comparison::fences()),
        _ => None,
    };
    let mut editing_comparison = false;
    let mut first_frame = true;
    loop {
        // Loading and font initialization are not elapsed simulation time.
        let wall_dt = if first_frame {
            first_frame = false;
            0.0
        } else {
            get_frame_time().min(0.25)
        };
        let scale = (screen_width() / 1440.0)
            .min(screen_height() / 900.0)
            .max(0.25);
        let size = vec2(screen_width() / scale, screen_height() / scale);
        let mouse = Vec2::from(mouse_position()) / scale;
        if !map_editor.active
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
            set_default_camera();
            next_frame().await;
            continue;
        }
        comparison::publish_closed();
        if !editor.active && is_key_pressed(KeyCode::O) {
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
            set_default_camera();
            next_frame().await;
            continue;
        }
        editor::publish_closed();
        let chamber = Rect::new(0.0, 80.0, size.x, size.y - 177.0);
        let mut viewport;
        if is_key_pressed(KeyCode::H) {
            lab.inspector.help = !lab.inspector.help;
        }
        let in_chamber = chamber.contains(mouse) && !lab.inspector.captures(mouse, size);

        if in_chamber {
            if is_mouse_button_down(MouseButton::Middle) {
                lab.navigation.pan(mouse - lab.last_mouse, chamber);
            }
            let wheel = mouse_wheel().1;
            if wheel != 0.0 {
                if lab.world.scene() == Scene::Level {
                    lab.navigation.scroll_level(wheel, mouse, chamber);
                } else {
                    lab.navigation.scroll(wheel, mouse, chamber);
                }
            }
        }
        lab.navigation.advance(wall_dt, chamber);
        viewport = Viewport::new(chamber, lab.navigation.center, lab.navigation.zoom);
        if in_chamber {
            lab.aim = viewport.world(mouse);
        }
        lab.last_mouse = mouse;

        if is_key_pressed(KeyCode::Space) {
            lab.paused = !lab.paused;
            lab.accumulator = 0.0;
        }
        if is_key_pressed(KeyCode::T) {
            lab.slow = !lab.slow;
        }
        if is_key_pressed(KeyCode::Tab) {
            lab.overlay = !lab.overlay;
        }
        if is_key_pressed(KeyCode::V) && lab.world.scene() == Scene::Level {
            lab.fog_enabled = !lab.fog_enabled;
        }
        if is_key_pressed(KeyCode::R) {
            lab.reset(lab.world.scene());
        }
        if is_key_pressed(KeyCode::Home) {
            lab.fit_camera();
        }
        for binding in controls::SCENES {
            if is_key_pressed(binding.key) {
                lab.reset(binding.scene);
            }
        }
        for (key, ammunition) in [KeyCode::Key5, KeyCode::Key6]
            .into_iter()
            .zip(sim::Ammunition::MAIN)
        {
            if is_key_pressed(key) {
                lab.select_ammunition(ammunition);
            }
        }
        if is_key_pressed(KeyCode::B) {
            lab.select_ammunition(lab.inspector.ammunition.next());
        }
        lab.firing =
            (in_chamber && is_mouse_button_down(MouseButton::Left)) || is_key_down(KeyCode::F);
        if (in_chamber && is_mouse_button_pressed(MouseButton::Left)) || is_key_pressed(KeyCode::F)
        {
            lab.fire();
        }
        if in_chamber && is_mouse_button_pressed(MouseButton::Right) {
            lab.pending
                .retain(|c| !matches!(c, sim::Command::ActivateSpecial));
            lab.pending.push(sim::Command::ActivateSpecial);
        }
        let single_step = is_key_pressed(KeyCode::N);

        clear_background(ui::PAPER);
        let mut camera = Camera2D::from_display_rect(Rect::new(0.0, 0.0, size.x, size.y));
        // Macroquad flips the screen render target relative to display_rect.
        // Our world and UI both use screen coordinates with positive Y downward.
        camera.zoom.y = -camera.zoom.y;
        set_camera(&camera);
        let tracks = controls::tracks();
        if single_step {
            lab.paused = true;
            lab.accumulator = 0.0;
            lab.tick(tracks);
        } else if !lab.paused {
            lab.accumulator += f64::from(wall_dt) * if lab.slow { 0.2 } else { 1.0 };
            let mut steps = 0;
            while lab.accumulator >= f64::from(TICK_DT) && steps < 12 {
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

        let alpha = if lab.paused {
            1.0
        } else {
            (lab.accumulator / f64::from(TICK_DT)) as f32
        };
        viewport = Viewport::new(chamber, lab.navigation.center, lab.navigation.zoom);
        let fog_active = lab.fog_enabled && lab.world.scene() == Scene::Level;
        if fog_active {
            lab.vision = lab.world.player_vision(lab.world.player_aim().angle);
            lab.fog.get_or_insert_with(fog::Fog::default).observe(
                &lab.world,
                &lab.vision,
                get_time(),
            );
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
            lab.aim,
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
        let actions = lab.inspector.draw(
            size,
            mouse,
            &lab.world,
            lab.paused,
            lab.slow,
            lab.overlay,
            lab.tick_ms,
            fog_active,
        );
        for action in actions {
            match action {
                Action::Scene(scene) => lab.reset(scene),
                Action::Reset => lab.reset(lab.world.scene()),
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
        #[cfg(target_arch = "wasm32")]
        {
            diagnostics::publish(&lab.world);
            diagnostics::publish_camera(&lab.navigation);
            diagnostics::publish_ammunition(lab.inspector.ammunition);
        }
        set_default_camera();
        next_frame().await;
    }
}
