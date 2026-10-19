//! Human play uses the normal fog, camera, controls and authoritative physics.
//! Full-field recordings are separate and never become a player's view/input.
use super::*;
use crate::{
    controls,
    fog::Fog,
    navigation::{LookAhead, Navigation},
};
use sim::{Ammunition, VehicleKind};
use std::{
    fs::File,
    io::{BufWriter, Write},
};

const TEAMS: [Team; 3] = [Team::Blue, Team::Red, Team::Red];

struct Round {
    game: MultiplayerWorld,
    bots: [Worker; 2],
    plans: [InputPlan; 2],
    history: History,
    decisions: BufWriter<File>,
    events: BufWriter<File>,
    hud: Hud,
    tick: u32,
    seed: u64,
    started: bool,
    finished: bool,
}
impl Round {
    fn new(settings: &Settings) -> Result<Self, String> {
        let mut game = MultiplayerWorld::new(&settings.level(), &TEAMS)?;
        game.configure_airdrops(true, 42);
        game.require_vehicle_choices();
        let mut make = |slot: usize| {
            let mut map = game.for_player(slot + 1).unwrap().ai_map();
            map.seed = player_seed(settings.seed, slot * 2 + 1);
            map.team_slot = slot;
            Worker::new(&settings.agents[1], &settings.configs[1], map).map_err(|e| e.to_string())
        };
        let bots = [make(0)?, make(1)?];
        if !bots.iter().all(|b| b.config.name.starts_with("v11-")) {
            return Err("The 1v2 challenge requires the frozen v11 opponents.".into());
        }
        game.for_player(0);
        let names = ["Human".into(), bots[0].config.name.clone()];
        let history = History::with_roster(
            &settings.directory,
            game.world(),
            &names,
            settings.seed,
            &TEAMS,
        )?;
        let open = |suffix: &str| {
            File::create(history.path.with_extension(suffix))
                .map(BufWriter::new)
                .map_err(|e| e.to_string())
        };
        let decisions = open("decisions.jsonl")?;
        let events = open("events.jsonl")?;
        let manifest_path = PathBuf::from(&settings.configs[1]).with_file_name("manifest.json");
        let manifest: serde_json::Value =
            serde_json::from_reader(File::open(manifest_path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        std::fs::write(history.path.with_extension("challenge.json"), serde_json::to_vec_pretty(&serde_json::json!({
            "mode":"human-1v2", "human_player":0, "teams":TEAMS,
            "opponent":manifest,"seed":settings.seed,"duration_seconds":ROUND_SECONDS,
            "respawn_seconds":sim::RESPAWN_SECONDS,"human_view":"normal fog and camera",
            "inputs":".inputs.jsonl contains the three applied controls every physics tick",
            "decisions":".decisions.jsonl contains raw restricted worker observations and decisions; v11 applies its own reaction queue",
            "observer":".frames and .vision contain full-field review snapshots; never used by human or bot controls"
        })).unwrap()).map_err(|e| e.to_string())?;
        Ok(Self {
            game,
            bots,
            plans: std::array::from_fn(|_| InputPlan::default()),
            history,
            decisions,
            events,
            hud: Hud {
                tick: 0,
                telemetry: std::array::from_fn(|_| Telemetry::default()),
                kills: [0; 2],
                captures: [0; 2],
                stalled: [false; ARENA_PLAYERS],
                vision: std::array::from_fn(|_| vec![]),
            },
            tick: 0,
            seed: settings.seed,
            started: false,
            finished: false,
        })
    }
    fn done(&self) -> bool {
        self.tick >= ROUND_TICKS || self.game.world().winner().is_some()
    }
    fn step(&mut self, human: PlayerInput) -> Result<(), String> {
        if self.done() {
            return Ok(());
        }
        if !self.started {
            if human.deploy.is_none() {
                return Ok(());
            }
            self.started = true;
        }
        if self.tick.is_multiple_of(12) {
            let geometry = self.game.world().ai_geometry();
            self.hud.vision = std::array::from_fn(|_| vec![]);
            let human_view = self
                .game
                .for_player(0)
                .unwrap()
                .ai_observation_from(&geometry);
            self.hud.vision[0] = human_view.regions.clone();
            line(
                &mut self.decisions,
                &serde_json::json!({"tick":self.tick,"player":0,"human_view":human_view}),
            )?;
            for slot in 0..2 {
                let observation = self
                    .game
                    .for_player(slot + 1)
                    .unwrap()
                    .ai_observation_from(&geometry);
                self.hud.vision[1].extend(observation.regions.clone());
                let decision = self.bots[slot]
                    .decide(&observation)
                    .map_err(|e| e.to_string())?;
                self.plans[slot] = decision.plan();
                self.hud.telemetry[slot + 1] = decision.telemetry.clone();
                line(
                    &mut self.decisions,
                    &serde_json::json!({"tick":self.tick,"player":slot+1,"observation":observation,"decision":decision}),
                )?;
            }
        }
        let applied = [
            human,
            self.plans[0].input_at((self.tick % 12) as usize),
            self.plans[1].input_at((self.tick % 12) as usize),
        ];
        self.history
            .input(serde_json::json!({"tick":self.tick,"inputs":applied}))?;
        let events = self.game.step(&applied);
        self.tick += 1;
        self.game.for_player(0);
        for event in &events {
            match event {
                Event::FlagCaptured { team, .. } => {
                    self.hud.captures[usize::from(*team == Team::Red)] += 1
                }
                Event::TankDestroyed { team, details, .. } if !details.mounted => {
                    self.hud.kills[usize::from(*team == Team::Blue)] += 1
                }
                _ => {}
            }
        }
        if !events.is_empty() {
            line(
                &mut self.events,
                &serde_json::json!({"tick":self.tick,"events":events}),
            )?;
        }
        if self.tick.is_multiple_of(12) || self.done() {
            self.hud.tick = self.tick;
            let mut frame = replica::Frame::capture(self.game.world());
            frame.weapons = self.game.weapon_effects();
            frame.vision = sim::Vision::default();
            self.history.capture(frame, self.hud.clone())?;
        }
        if self.tick.is_multiple_of(120) {
            self.decisions.flush().map_err(|e| e.to_string())?;
            self.events.flush().map_err(|e| e.to_string())?;
        }
        if self.done() {
            self.finish()?;
        }
        Ok(())
    }
    fn finish(&mut self) -> Result<(), String> {
        if self.finished {
            return Ok(());
        }
        self.history.finish()?;
        self.decisions.flush().map_err(|e| e.to_string())?;
        self.events.flush().map_err(|e| e.to_string())?;
        std::fs::write(self.history.path.with_extension("summary.json"),serde_json::to_vec_pretty(&serde_json::json!({
            "mode":"human-1v2","players":3,"teams":TEAMS,"human_player":0,"seed":self.seed,
            "ticks":self.tick,"seconds":self.tick as f32/120.0,"complete":self.done(),
            "winner":self.game.world().winner(),"captures":self.hud.captures,"kills":self.hud.kills,
            "opponents":[self.bots[0].config.name,self.bots[1].config.name]
        })).unwrap()).map_err(|e|e.to_string())?;
        self.finished = true;
        Ok(())
    }
}
fn line(writer: &mut impl Write, value: &serde_json::Value) -> Result<(), String> {
    serde_json::to_writer(&mut *writer, value).map_err(|e| e.to_string())?;
    writeln!(writer).map_err(|e| e.to_string())
}

pub(super) async fn run(settings: &Settings) -> Result<(), String> {
    let mut round = Round::new(settings)?;
    let mut pad = controller::Controller::new();
    let mut navigation = Navigation::default();
    let mut keyboard = controls::KeyboardAim::default();
    let mut fog = Fog::default();
    let mut tracks = TrackMarks::default();
    let mut selection = 1usize;
    let mut ammo = Ammunition::HighExplosive;
    let mut mortar_range = 20.0;
    let mut aim = Vec2::X;
    let mut last_mouse = Vec2::from(mouse_position());
    let mut fire_pending = false;
    let mut special_pending = false;
    let mut deployment = None;
    let mut paused = false;
    let mut accumulator = 0.0_f64;
    let started = get_time();
    let mut status_at = 0.0;
    let mut screenshot = settings.screenshot.clone();
    loop {
        let dt = get_frame_time().min(0.1);
        let actions = pad.poll(dt, crate::deck_mode());
        let mouse = Vec2::from(mouse_position());
        show_mouse(!pad.active);
        if settings
            .exit_after
            .is_some_and(|s| get_time() - started >= s)
        {
            round.finish()?;
            break;
        }
        if is_key_pressed(KeyCode::Escape) || actions.pressed(controller::MENU) {
            paused = !paused;
            accumulator = 0.0;
        }
        if paused && actions.pressed(controller::BACK) {
            paused = false;
        }
        let done = round.done();
        let world = round.game.for_player(0).unwrap();
        let alive = world.tank_alive();
        let kind = world.player_vehicle();
        let position = v(world.launcher_position());
        let barrel = world.player_aim().angle;
        let rect = Rect::new(0.0, 0.0, screen_width(), screen_height());
        navigation.expand_game_view(rect);
        navigation.advance_game_zoom(
            dt,
            rect,
            world.player_sight_range(),
            world.player_in_radar_zone(),
        );
        navigation.follow_tank(
            position,
            Some(if pad.active || keyboard.active() {
                LookAhead::Direction(Vec2::from_angle(barrel))
            } else {
                LookAhead::Pointer(mouse)
            }),
            dt,
            rect,
        );
        if mouse_wheel().1 != 0.0 {
            navigation.scroll_game(
                mouse_wheel().1,
                rect,
                world.player_sight_range(),
                world.player_in_radar_zone(),
            );
        }
        if actions.navigation.y != 0.0
            && (actions.held(controller::UP) || actions.held(controller::DOWN))
            && alive
        {
            navigation.scroll_game(
                -actions.navigation.y,
                rect,
                world.player_sight_range(),
                world.player_in_radar_zone(),
            );
        }
        let viewport = view::Viewport::new(rect, navigation.center, navigation.zoom);
        if alive && !paused && !done {
            keyboard.update(
                controls::turret_turn(),
                mouse != last_mouse && !pad.active,
                barrel,
                dt,
            );
            if pad.active && !keyboard.active() {
                keyboard.point_stick(Vec2::from_angle(barrel));
            }
            keyboard.point_stick(actions.aim);
            if kind == VehicleKind::Mortar {
                mortar_range = controls::advance_mortar_range(mortar_range, actions.aim, dt);
                if let Some(turn) = controls::turret_turn() {
                    mortar_range = (mortar_range + turn * dt * 8.0).clamp(6.0, 36.0);
                }
            }
            aim = if keyboard.active() {
                if kind == VehicleKind::Mortar {
                    position + Vec2::from_angle(barrel) * mortar_range
                } else {
                    keyboard.target(position).unwrap()
                }
            } else {
                viewport.world(mouse)
            };
            fire_pending |= is_mouse_button_pressed(MouseButton::Left)
                || is_key_pressed(KeyCode::Space)
                || is_key_pressed(KeyCode::F)
                || actions.pressed(controller::FIRE);
            special_pending |= is_mouse_button_pressed(MouseButton::Right)
                || is_key_pressed(KeyCode::G)
                || actions.pressed(controller::SPECIAL);
            if is_key_pressed(KeyCode::B) || actions.pressed(controller::AMMO) {
                ammo = ammo.next();
            }
            if is_key_pressed(KeyCode::Key5) {
                ammo = Ammunition::HighExplosive;
            }
            if is_key_pressed(KeyCode::Key6) {
                ammo = Ammunition::ArmorPiercing;
            }
        }
        last_mouse = mouse;
        let drive = if !alive || paused || done {
            [0.0; 2]
        } else if actions.drive != Vec2::ZERO {
            let hull = world
                .tank_poses()
                .find(|(id, _, _)| *id == world.player_tank_id())
                .map_or(barrel, |(_, _, angle)| angle);
            if kind == VehicleKind::Scout {
                controls::stick_car(actions.drive, hull)
            } else {
                controls::stick_tracks(actions.drive, hull)
            }
        } else {
            controls::tracks(kind)
        };
        let held_fire = is_mouse_button_down(MouseButton::Left)
            || is_key_down(KeyCode::Space)
            || is_key_down(KeyCode::F)
            || actions.held(controller::FIRE);
        let vision = world.player_vision(barrel);
        fog.observe(world, &vision, get_time());
        clear_background(ui::PAPER);
        view::draw_world(
            world,
            &viewport,
            alive.then_some(aim),
            1.0,
            false,
            &[],
            &[],
            &[],
            &mut tracks,
            Some((fog.scene(), &vision)),
        );
        draw_rectangle(
            14.0,
            12.0,
            410.0,
            61.0,
            Color {
                a: 0.85,
                ..ui::PAPER
            },
        );
        ui::text("YOU / BLUE   vs   2 x V11 / RED", 26.0, 37.0, 20.0, ui::INK);
        ui::text(
            &format!(
                "{:03.0}s   HP {:.0}   {}",
                (ROUND_TICKS - round.tick) as f32 / 120.0,
                world
                    .tanks()
                    .find(|t| t.id == world.player_tank_id())
                    .map_or(0.0, |t| t.health),
                ammo.label()
            ),
            26.0,
            60.0,
            17.0,
            ui::CYAN,
        );
        if let Some(weapon) = world.carried_special() {
            ui::right_text(
                &format!(
                    "{} x{} / LT SPECIAL",
                    weapon.label(),
                    world.special_charges()
                ),
                screen_width() - 22.0,
                38.0,
                18.0,
                ui::VIOLET,
            );
        }
        if !alive && !done && !paused {
            draw_rectangle(
                0.0,
                0.0,
                screen_width(),
                screen_height(),
                Color::new(0.0, 0.01, 0.03, 0.75),
            );
            center_text(
                "YOU vs TWO V11 BOTS",
                screen_width() * 0.5,
                screen_height() * 0.5 - 95.0,
                28.0,
                ui::INK,
            );
            let message = if !round.started {
                "CHOOSE A VEHICLE / A TO START".into()
            } else {
                format!(
                    "RESPAWN {:.1}s / CHOOSE YOUR VEHICLE",
                    world.player_respawn_remaining().unwrap_or(0.0)
                )
            };
            center_text(
                &message,
                screen_width() * 0.5,
                screen_height() * 0.5 - 57.0,
                19.0,
                ui::MUTED,
            );
            if actions.navigation.x != 0.0 {
                selection = (selection as isize + actions.navigation.x.signum() as isize)
                    .rem_euclid(3) as usize;
            }
            for (i, vehicle) in VehicleKind::ALL.into_iter().enumerate() {
                let card = Rect::new(
                    screen_width() * 0.5 - 330.0 + i as f32 * 225.0,
                    screen_height() * 0.5 - 18.0,
                    210.0,
                    74.0,
                );
                if ui::button(card, vehicle.label(), mouse, i == selection) {
                    selection = i;
                    deployment = Some(vehicle);
                }
            }
            for (i, key) in [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3]
                .into_iter()
                .enumerate()
            {
                if is_key_pressed(key) {
                    selection = i;
                    deployment = Some(VehicleKind::ALL[i]);
                }
            }
            if actions.pressed(controller::CONFIRM) || is_key_pressed(KeyCode::Enter) {
                deployment = Some(VehicleKind::ALL[selection]);
            }
            center_text(
                "LEFT STICK MOVE / RIGHT STICK AIM / RT FIRE / LT SPECIAL",
                screen_width() * 0.5,
                screen_height() * 0.5 + 103.0,
                17.0,
                ui::MUTED,
            );
            center_text(
                "4 MINUTES / 10s RESPAWN / RECORDING AUTOMATICALLY",
                screen_width() * 0.5,
                screen_height() * 0.5 + 135.0,
                17.0,
                ui::MUTED,
            );
        }
        if done || paused {
            draw_rectangle(
                0.0,
                0.0,
                screen_width(),
                screen_height(),
                Color::new(0.0, 0.01, 0.03, 0.7),
            );
            let title = if !done {
                "PAUSED / MENU TO RESUME"
            } else {
                match round.game.world().winner() {
                    Some(Team::Blue) => "YOU CAPTURED THE FLAG",
                    Some(Team::Red) => "V11 CAPTURED THE FLAG",
                    _ => "TIME / ROUND RECORDED",
                }
            };
            center_text(
                title,
                screen_width() * 0.5,
                screen_height() * 0.5 - 25.0,
                30.0,
                ui::INK,
            );
            if done {
                center_text(
                    "A / ENTER TO PLAY AGAIN",
                    screen_width() * 0.5,
                    screen_height() * 0.5 + 25.0,
                    20.0,
                    ui::CYAN,
                );
                if actions.pressed(controller::CONFIRM) || is_key_pressed(KeyCode::Enter) {
                    round.finish()?;
                    round = Round::new(settings)?;
                    fog = Fog::default();
                    tracks = TrackMarks::default();
                    navigation = Navigation::default();
                    keyboard = controls::KeyboardAim::default();
                    accumulator = 0.0;
                    deployment = None;
                    fire_pending = false;
                    special_pending = false;
                }
            }
        }
        if !paused && !round.done() && (round.started || deployment.is_some()) {
            accumulator += f64::from(dt);
            let mut steps = 0;
            while accumulator >= f64::from(TICK_DT) && steps < 12 {
                let input = PlayerInput {
                    tracks: drive,
                    aim: sim::Vec2::new(aim.x, aim.y),
                    ammunition: ammo,
                    fire: fire_pending || (kind == VehicleKind::Scout && held_fire),
                    special: special_pending,
                    deploy: deployment.take(),
                };
                round.step(input)?;
                fire_pending = false;
                special_pending = false;
                accumulator -= f64::from(TICK_DT);
                steps += 1;
                if round.done() {
                    break;
                }
            }
        } else {
            accumulator = 0.0;
            fire_pending = false;
            special_pending = false;
        }
        if get_time() - status_at > 1.0 {
            status_at = get_time();
            std::fs::write(round.history.path.with_extension("status.json"),serde_json::to_vec(&serde_json::json!({"mode":"human-1v2","players":3,"teams":[1,2],"human_player":0,"tick":round.tick,"waiting_for_vehicle":!round.started,"paused":paused,"complete":round.done(),"fps":get_fps(),"recording":round.history.path,"opponents":[round.bots[0].config.name,round.bots[1].config.name]})).unwrap()).map_err(|e|e.to_string())?;
        }
        if is_key_pressed(KeyCode::F12) || screenshot.is_some() && get_time() - started > 2.0 {
            let destination = screenshot
                .take()
                .unwrap_or_else(|| round.history.path.with_extension("screenshot.png"));
            get_screen_data().export_png(destination.to_str().unwrap());
        }
        next_frame().await;
    }
    Ok(())
}

fn center_text(label: &str, x: f32, y: f32, size: f32, color: Color) {
    let width = measure_text(label, None, size as u16, 1.0).width;
    ui::text(label, x - width * 0.5, y, size, color);
}
