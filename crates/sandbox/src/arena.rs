//! Native full-battlefield spectator. Frozen workers receive restricted views;
//! observer recordings and replay worlds never feed either controller.
mod challenge;
mod history;
use crate::{controller, track_marks::TrackMarks, ui, view, wreck_effects::WreckEffect};
use history::{History, Hud};
use macroquad::prelude::*;
use sim::{
    Event, TICK_DT, World,
    ai::{
        ARENA_PLAYERS, ARENA_TEAMS, InputPlan, ROUND_SECONDS, ROUND_TICKS, Telemetry, player_seed,
        process::Worker,
    },
    level::{Level, Team},
    multiplayer::{MultiplayerWorld, PlayerInput},
    replica,
};
use std::path::PathBuf;

pub fn window_conf() -> Conf {
    let mut conf = crate::window_conf(crate::Application::Game);
    conf.window_title = "Fracture — AI battlefield".into();
    conf
}
struct Settings {
    agents: [String; 2],
    configs: [String; 2],
    directory: PathBuf,
    map: usize,
    exit_after: Option<f64>,
    screenshot: Option<PathBuf>,
    seed: u64,
}
impl Settings {
    fn new() -> Self {
        let root = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let args: Vec<_> = std::env::args().collect();
        let option = |key: &str| {
            args.iter()
                .position(|s| s == key)
                .and_then(|i| args.get(i + 1))
                .cloned()
        };
        let data = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(std::env::var_os("HOME").unwrap()).join(".local/share")
            });
        Self {
            agents: [
                option("--blue-agent")
                    .unwrap_or_else(|| root.join("agents/blue/agent").to_string_lossy().into()),
                option("--red-agent")
                    .unwrap_or_else(|| root.join("agents/red/agent").to_string_lossy().into()),
            ],
            configs: [
                option("--blue-config").unwrap_or_else(|| {
                    root.join("agents/blue/config.json")
                        .to_string_lossy()
                        .into()
                }),
                option("--red-config").unwrap_or_else(|| {
                    root.join("agents/red/config.json").to_string_lossy().into()
                }),
            ],
            directory: option("--record-dir")
                .map(PathBuf::from)
                .unwrap_or_else(|| data.join("fracture-ai-arena")),
            map: usize::from(option("--map").as_deref() == Some("classic")),
            exit_after: option("--test-seconds").and_then(|s| s.parse().ok()),
            screenshot: option("--screenshot").map(PathBuf::from),
            seed: option("--seed")
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(|| {
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos() as u64
                }),
        }
    }
    fn level(&self) -> Level {
        Level::from_json(if self.map == 0 {
            include_str!("../../../levels/dota-level.json")
        } else {
            include_str!("../../../levels/classic-tanks-2.json")
        })
        .unwrap()
    }
}
struct Session {
    game: MultiplayerWorld,
    seed: u64,
    bots: [Worker; ARENA_PLAYERS],
    inputs: [PlayerInput; ARENA_PLAYERS],
    plans: [InputPlan; ARENA_PLAYERS],
    plan_tick: u32,
    telemetry: [Telemetry; ARENA_PLAYERS],
    vision: [Vec<sim::Vision>; 2],
    history: History,
    tick: u32,
    ids: [usize; ARENA_PLAYERS],
    names: [String; 2],
    kills: [u32; 2],
    captures: [u32; 2],
    anchor: [sim::Vec2; ARENA_PLAYERS],
    anchor_tick: [u32; ARENA_PLAYERS],
    stalled: [bool; ARENA_PLAYERS],
    sparks: Vec<view::Spark>,
    blasts: Vec<view::Blast>,
    wrecks: Vec<WreckEffect>,
    tracks: TrackMarks,
}
impl Session {
    fn new(settings: &Settings) -> Result<Self, String> {
        let mut game = MultiplayerWorld::new(&settings.level(), &ARENA_TEAMS)?;
        game.configure_airdrops(true, 42);
        game.require_vehicle_choices();
        let mut make = |side: usize| {
            let mut map = game.for_player(side).unwrap().ai_map();
            map.seed = player_seed(settings.seed, side);
            map.team_slot = side / 2;
            Worker::new(
                settings.agents[side % 2].as_str(),
                settings.configs[side % 2].as_str(),
                map,
            )
            .map_err(|e| e.to_string())
        };
        let bots = [make(0)?, make(1)?, make(2)?, make(3)?];
        let names = std::array::from_fn(|i| bots[i].config.name.clone());
        let ids = std::array::from_fn(|i| game.tank_id(i).unwrap());
        let anchor = std::array::from_fn(|i| game.for_player(i).unwrap().launcher_position());
        game.for_player(0);
        let history = History::new(&settings.directory, game.world(), &names, settings.seed)?;
        let mut session = Self {
            game,
            seed: settings.seed,
            bots,
            inputs: [PlayerInput::default(); ARENA_PLAYERS],
            plans: std::array::from_fn(|_| InputPlan::default()),
            plan_tick: 0,
            telemetry: std::array::from_fn(|_| Telemetry::default()),
            vision: std::array::from_fn(|_| vec![]),
            history,
            tick: 0,
            ids,
            names,
            kills: [0; 2],
            captures: [0; 2],
            anchor,
            anchor_tick: [0; ARENA_PLAYERS],
            stalled: [false; ARENA_PLAYERS],
            sparks: vec![],
            blasts: vec![],
            wrecks: vec![],
            tracks: TrackMarks::default(),
        };
        session.decide()?;
        session.capture()?;
        Ok(session)
    }
    fn hud(&self) -> Hud {
        Hud {
            tick: self.tick,
            telemetry: self.telemetry.clone(),
            kills: self.kills,
            captures: self.captures,
            stalled: self.stalled,
            vision: self.vision.clone(),
        }
    }
    fn decide(&mut self) -> Result<(), String> {
        self.plan_tick = self.tick;
        let mut perceived = vec![];
        let mut actors = vec![];
        self.vision = std::array::from_fn(|_| vec![]);
        let geometry = self.game.world().ai_geometry();
        for side in 0..ARENA_PLAYERS {
            let observation = self
                .game
                .for_player(side)
                .unwrap()
                .ai_observation_from(&geometry);
            actors.push(serde_json::json!({"player":side,"position":observation.position,
                "vehicle":observation.vehicle,"alive":observation.alive,"health":observation.health}));
            self.vision[side % 2].extend(observation.regions.iter().cloned());
            let decision = self.bots[side]
                .decide(&observation)
                .map_err(|e| e.to_string())?;
            self.inputs[side] = decision.input;
            self.plans[side] = decision.plan();
            self.telemetry[side] = decision.telemetry;
            if !observation.alive || observation.position.distance(self.anchor[side]) > 0.75 {
                self.anchor[side] = observation.position;
                self.anchor_tick[side] = self.tick;
            }
            self.stalled[side] = observation.alive
                && self.tick - self.anchor_tick[side] > 360
                && !matches!(
                    self.telemetry[side].mode.as_str(),
                    "combat" | "engage" | "clear wall"
                )
                && !(self.telemetry[side].mode == "observe/aim"
                    && self.telemetry[side].role == "interceptor");
            perceived.push(observation.targets);
        }
        self.history.input(serde_json::json!({"tick":self.tick,"actors":actors,"inputs":self.inputs,"plans":self.plans,"telemetry":self.telemetry,"visible_targets":perceived}))?;
        Ok(())
    }
    fn capture(&mut self) -> Result<(), String> {
        self.game.for_player(0);
        let mut frame = replica::Frame::capture(self.game.world());
        frame.weapons = self.game.weapon_effects();
        frame.vision = sim::Vision::default();
        self.history.capture(frame, self.hud())
    }
    fn step(&mut self) -> Result<(), String> {
        if self.tick >= ROUND_TICKS {
            return Ok(());
        }
        self.tracks.seed(self.game.world());
        let applied = std::array::from_fn::<_, ARENA_PLAYERS, _>(|i| {
            self.plans[i].input_at((self.tick - self.plan_tick) as usize)
        });
        let events = self.game.step(&applied);
        self.tick += 1;
        self.tracks.update(self.game.world());
        for blast in &mut self.blasts {
            blast.life -= TICK_DT;
        }
        self.blasts.retain(|b| b.life > 0.0);
        for spark in &mut self.sparks {
            spark.life -= TICK_DT;
            spark.position += spark.velocity * TICK_DT;
        }
        self.sparks.retain(|s| s.life > 0.0);
        for wreck in &mut self.wrecks {
            wreck.advance(TICK_DT);
        }
        self.wrecks.retain(WreckEffect::alive);
        for event in &events {
            match *event {
                Event::TankDestroyed {
                    team,
                    details,
                    position,
                } => {
                    if !details.mounted {
                        self.kills[usize::from(team == Team::Blue)] += 1;
                    }
                    self.wrecks.push(WreckEffect::new(
                        position,
                        team,
                        details,
                        self.tick as usize + details.id,
                    ));
                }
                Event::Exploded { position, radius } => self.blasts.push(view::Blast {
                    position: vec2(position.x, position.y),
                    radius,
                    life: 0.45,
                }),
                Event::Fired {
                    position, velocity, ..
                } => self.sparks.push(view::Spark {
                    position: vec2(position.x, position.y),
                    velocity: vec2(velocity.x, velocity.y).normalize_or_zero() * 2.0,
                    life: 0.15,
                }),
                _ => {}
            }
        }
        if !events.is_empty() {
            self.history
                .input(serde_json::json!({"tick":self.tick,"events":events}))?;
        }
        if let Some(team) = self.game.world().winner() {
            self.captures[usize::from(team == Team::Red)] += 1;
            self.game = MultiplayerWorld::new(
                &Level::from_json(&self.level_json()).unwrap(),
                &ARENA_TEAMS,
            )?;
            self.game.configure_airdrops(true, 42);
            self.game.require_vehicle_choices();
            for side in 0..ARENA_PLAYERS {
                let mut map = self.game.for_player(side).unwrap().ai_map();
                map.seed = player_seed(
                    self.seed
                        .wrapping_add(self.captures.iter().sum::<u32>() as u64),
                    side,
                );
                map.team_slot = side / 2;
                self.bots[side].reset(map).map_err(|e| e.to_string())?;
            }
            self.decide()?;
        }
        if self.tick.is_multiple_of(12) {
            self.decide()?;
            self.capture()?;
        }
        if self.tick == ROUND_TICKS {
            self.history.finish()?;
            std::fs::write(self.history.path.with_extension("summary.json"),serde_json::to_vec_pretty(&serde_json::json!({
                "respawn_seconds":sim::RESPAWN_SECONDS,"players_per_team":2,"seed":self.seed,"ticks":self.tick,"seconds":ROUND_SECONDS,"versions":self.names,"kills":self.kills,"captures":self.captures,
                "objective_winner":match self.captures[0].cmp(&self.captures[1]) { std::cmp::Ordering::Greater=>Some(0),std::cmp::Ordering::Less=>Some(1),_=>None }})).unwrap()).map_err(|e|e.to_string())?;
        }
        Ok(())
    }
    fn level_json(&self) -> String {
        // The observer stores authored recipes separately from worker input.
        if self.history.map.size.x > 100.0 {
            include_str!("../../../levels/dota-level.json")
        } else {
            include_str!("../../../levels/classic-tanks-2.json")
        }
        .into()
    }
}
fn button(rect: Rect, label: &str, mouse: Vec2) -> bool {
    let hover = rect.contains(mouse);
    draw_rectangle(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        if hover { ui::RULE } else { ui::PAPER },
    );
    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, ui::MUTED);
    ui::text(label, rect.x + 10.0, rect.y + 24.0, 17.0, ui::INK);
    hover && is_mouse_button_pressed(MouseButton::Left)
}
fn v(p: sim::Vec2) -> Vec2 {
    vec2(p.x, p.y)
}
fn viewport(size: sim::Vec2) -> view::Viewport {
    let rect = Rect::new(16.0, 65.0, screen_width() - 306.0, screen_height() - 122.0);
    let mut viewport = view::Viewport::new(rect, Vec2::ZERO, 1.0);
    viewport.scale = ((rect.w - 28.0) / size.x).min((rect.h - 28.0) / size.y);
    viewport
}
fn markers(
    world: &World,
    view: &view::Viewport,
    ids: [usize; ARENA_PLAYERS],
    hud: &Hud,
    routes: bool,
) {
    for (side, &id) in ids.iter().enumerate() {
        let Some((_, position, _)) = world.tank_poses().find(|(tank, _, _)| *tank == id) else {
            continue;
        };
        let color = if side % 2 == 0 { ui::CYAN } else { ui::PINK };
        let p = view.screen(v(position));
        draw_circle_lines(
            p.x,
            p.y,
            9.0,
            2.0,
            if hud.stalled[side] { ORANGE } else { color },
        );
        ui::text(
            &format!("{}{}", if side % 2 == 0 { "B" } else { "R" }, side / 2 + 1),
            p.x + 10.0,
            p.y - 8.0,
            18.0,
            color,
        );
        if routes {
            let waypoint = view.screen(Vec2::from(hud.telemetry[side].waypoint));
            draw_line(p.x, p.y, waypoint.x, waypoint.y, 1.8, color);
            draw_circle_lines(waypoint.x, waypoint.y, 4.0, 1.0, color);
        }
    }
}
// Render each team's polygons opaquely into one mask, then tint the union
// once. Overlapping allied sources must not produce darker patches.
#[derive(Default)]
struct TeamVision {
    masks: Option<[RenderTarget; 2]>,
    key: Option<(u32, u32, u32)>,
}
impl TeamVision {
    fn draw(&mut self, hud: &Hud, view: &view::Viewport) {
        let width = screen_width() as u32;
        let height = screen_height() as u32;
        if self.key.is_none_or(|(_, w, h)| w != width || h != height) {
            self.masks = Some(std::array::from_fn(|_| render_target(width, height)));
            self.key = None;
        }
        let masks = self.masks.as_ref().unwrap();
        if self.key != Some((hud.tick / 12, width, height)) {
            for (side, mask) in masks.iter().enumerate() {
                let mut camera =
                    Camera2D::from_display_rect(Rect::new(0.0, 0.0, width as f32, height as f32));
                camera.render_target = Some(mask.clone());
                set_camera(&camera);
                clear_background(BLANK);
                for region in &hud.vision[side] {
                    let origin = view.screen(v(region.origin));
                    for (a, b) in region
                        .boundary
                        .iter()
                        .zip(region.boundary.iter().cycle().skip(1))
                        .take(region.boundary.len())
                    {
                        draw_triangle(origin, view.screen(v(*a)), view.screen(v(*b)), WHITE);
                    }
                }
            }
            set_default_camera();
            self.key = Some((hud.tick / 12, width, height));
        }
        for (side, mask) in masks.iter().enumerate() {
            let tint = if side == 0 {
                Color::new(0.05, 0.5, 1.0, 0.25)
            } else {
                Color::new(1.0, 0.15, 0.28, 0.25)
            };
            draw_texture_ex(
                &mask.texture,
                0.0,
                0.0,
                tint,
                DrawTextureParams {
                    flip_y: true,
                    ..Default::default()
                },
            );
            for region in &hud.vision[side] {
                for (a, b) in region
                    .boundary
                    .iter()
                    .zip(region.boundary.iter().cycle().skip(1))
                    .take(region.boundary.len())
                {
                    let a = view.screen(v(*a));
                    let b = view.screen(v(*b));
                    draw_line(a.x, a.y, b.x, b.y, 0.8, Color { a: 0.55, ..tint });
                }
            }
        }
    }
}
pub async fn run() {
    let font =
        load_ttf_font_from_bytes(include_bytes!("../assets/LiberationMono-Bold.ttf")).unwrap();
    macroquad::text::set_default_font(font);
    let mut settings = Settings::new();
    if std::env::args().any(|arg| arg == "--play-v11") {
        if let Err(e) = challenge::run(&settings).await {
            error_screen(e).await;
        }
        return;
    }
    let mut session = match Session::new(&settings) {
        Ok(s) => s,
        Err(e) => {
            error_screen(e).await;
            return;
        }
    };
    let mut pad = controller::Controller::new();
    let mut paused = false;
    let mut slow = false;
    let mut routes = true;
    let mut vision_overlay = TeamVision::default();
    let mut vision_enabled = true;
    let mut accumulator = 0.0;
    let mut review: Option<(usize, World)> = None;
    let mut status = String::new();
    let mut pin = None;
    let started = get_time();
    let mut status_at = 0.0;
    loop {
        let frame_started = std::time::Instant::now();
        let dt = get_frame_time().min(0.25);
        let actions = pad.poll(dt, crate::deck_mode());
        let mouse = if pad.active {
            Vec2::splat(f32::MIN)
        } else {
            Vec2::from(mouse_position())
        };
        show_mouse(!pad.active);
        if settings
            .exit_after
            .is_some_and(|s| get_time() - started >= s)
        {
            session.history.finish().ok();
            break;
        }
        if is_key_pressed(KeyCode::Escape) {
            break;
        }
        let done = session.tick >= ROUND_TICKS;
        if get_time() - status_at > 1.0 {
            status_at = get_time();
            let value = serde_json::json!({"players":ARENA_PLAYERS,"teams":[2,2],"respawn_seconds":sim::RESPAWN_SECONDS,
                "bots":session.ids.iter().enumerate().filter_map(|(player,id)|session.game.world().tanks().find(|t|t.id==*id).map(|t|serde_json::json!({"player":player,"vehicle":t.vehicle,"health":t.health,"respawn_remaining":t.respawn_remaining}))).collect::<Vec<_>>(),"tick":session.tick,"paused":paused,"review":review.as_ref().map(|r|r.0),
                "frame_ms":get_frame_time()*1000.0,"fps":get_fps(),"width":screen_width(),"height":screen_height(),"frames":session.history.frames.len()});
            let _ = std::fs::write(
                session.history.path.with_extension("status.json"),
                serde_json::to_vec(&value).unwrap(),
            );
        }
        let x = screen_width() - 278.0;
        clear_background(ui::PAPER);
        let pause_click = button(
            Rect::new(x, 390.0, 258.0, 34.0),
            if done && review.is_none() {
                "A / SPACE  PLAY REPLAY"
            } else if paused {
                "A / SPACE  RESUME"
            } else {
                "A / SPACE  PAUSE"
            },
            mouse,
        );
        if pause_click
            || is_key_pressed(KeyCode::Space)
            || actions.pressed(controller::CONFIRM)
            || actions.pressed(controller::MENU)
        {
            if done && review.is_none() {
                review = Some((0, session.history.seek(0).unwrap()));
                paused = false;
            } else {
                paused = !paused;
            }
            accumulator = 0.0;
        }
        let restart = button(
            Rect::new(x, 435.0, 258.0, 34.0),
            "X / R  REPLAY ROUND",
            mouse,
        );
        let map = button(Rect::new(x, 480.0, 258.0, 34.0), "Y / M  NEXT MAP", mouse);
        if restart
            || map
            || is_key_pressed(KeyCode::R)
            || is_key_pressed(KeyCode::M)
            || actions.pressed(controller::HELP)
            || actions.pressed(controller::SETTINGS)
        {
            if map || is_key_pressed(KeyCode::M) || actions.pressed(controller::SETTINGS) {
                settings.map = 1 - settings.map;
            }
            session = match Session::new(&settings) {
                Ok(s) => s,
                Err(e) => {
                    status = e;
                    paused = true;
                    continue;
                }
            };
            paused = false;
            review = None;
            accumulator = 0.0;
            pin = None;
            vision_overlay.key = None;
            status.clear();
        }
        if button(
            Rect::new(x, 525.0, 258.0, 34.0),
            if slow {
                "LB / S  SPEED 0.25x"
            } else {
                "LB / S  SPEED 1x"
            },
            mouse,
        ) || is_key_pressed(KeyCode::S)
            || actions.pressed(controller::AMMO)
        {
            slow = !slow;
        }
        if button(Rect::new(x, 570.0, 258.0, 34.0), "D / B  GOAL LINES", mouse)
            || is_key_pressed(KeyCode::D)
            || actions.pressed(controller::BACK)
        {
            routes = !routes;
        }
        if is_key_pressed(KeyCode::V) || actions.pressed(controller::SPECIAL) {
            vision_enabled = !vision_enabled;
        }
        let bar = Rect::new(24.0, screen_height() - 43.0, screen_width() - 340.0, 20.0);
        if (paused || done)
            && mouse.x > bar.x
            && mouse.x < bar.right()
            && mouse.y > bar.y - 12.0
            && mouse.y < bar.bottom() + 12.0
            && is_mouse_button_down(MouseButton::Left)
        {
            let index = (((mouse.x - bar.x) / bar.w) * (session.history.frames.len() - 1) as f32)
                .round() as usize;
            review = Some((index, session.history.seek(index).unwrap()));
            paused = true;
        }
        let direction = if is_key_pressed(KeyCode::Left) || actions.pressed(controller::LEFT) {
            -1
        } else if is_key_pressed(KeyCode::Right) || actions.pressed(controller::RIGHT) {
            1
        } else {
            0
        };
        if direction != 0 && (paused || done) {
            let current = review
                .as_ref()
                .map_or(session.history.frames.len() - 1, |r| r.0);
            let index = (current as isize + direction * 10)
                .clamp(0, session.history.frames.len() as isize - 1)
                as usize;
            review = Some((index, session.history.seek(index).unwrap()));
            paused = true;
        }
        if !paused {
            accumulator += dt as f64 * if slow { 0.25 } else { 1.0 };
            if let Some((index, _)) = &mut review {
                if accumulator >= 0.1 {
                    accumulator %= 0.1;
                    let next = (*index + 1).min(session.history.frames.len() - 1);
                    review = Some((next, session.history.seek(next).unwrap()));
                    if next == session.history.frames.len() - 1 {
                        review = None;
                    }
                }
            } else if !done {
                let deadline = get_time() + 0.012;
                let mut steps = 0;
                while accumulator >= TICK_DT as f64
                    && steps < 12
                    && (steps == 0 || get_time() < deadline)
                {
                    if let Err(e) = session.step() {
                        status = e;
                        paused = true;
                        break;
                    }
                    accumulator -= TICK_DT as f64;
                    steps += 1;
                    if session.tick >= ROUND_TICKS {
                        break;
                    }
                }
                if accumulator >= TICK_DT as f64 {
                    accumulator %= TICK_DT as f64;
                }
            } else {
                accumulator = 0.0;
            }
        }
        let world = review
            .as_ref()
            .map_or_else(|| session.game.world(), |r| &r.1);
        let hud = review
            .as_ref()
            .map_or_else(|| session.hud(), |r| session.history.frames[r.0].1.clone());
        let view = viewport(session.history.map.size);
        let mut replay_tracks = TrackMarks::default();
        let (sparks, blasts, wrecks, tracks) = if review.is_some() {
            (&[][..], &[][..], &[][..], &mut replay_tracks)
        } else {
            (
                &session.sparks[..],
                &session.blasts[..],
                &session.wrecks[..],
                &mut session.tracks,
            )
        };
        view::draw_world(
            world, &view, None, 1.0, false, sparks, blasts, wrecks, tracks, None,
        );
        if vision_enabled {
            vision_overlay.draw(&hud, &view);
        }
        markers(world, &view, session.ids, &hud, routes);
        if view.rect.contains(mouse) && is_mouse_button_pressed(MouseButton::Left) {
            pin = Some(view.world(mouse));
        }
        if let Some(point) = pin {
            let p = view.screen(point);
            draw_circle_lines(p.x, p.y, 7.0, 2.0, ORANGE);
        }
        // Draw chrome after the battlefield so scenery cannot cover controls.
        draw_rectangle(0.0, 0.0, screen_width(), 59.0, ui::PAPER);
        ui::text("AI BATTLEFIELD / 2v2", 20.0, 27.0, 24.0, ui::INK);
        ui::text(
            if settings.map == 0 {
                "RIVER VALLEY"
            } else {
                "CLASSIC TANKS"
            },
            20.0,
            48.0,
            16.0,
            ui::MUTED,
        );
        ui::right_text(
            &format!(
                "{:05.1} / {}.0 s  {}",
                hud.tick as f32 / 120.0,
                ROUND_SECONDS,
                if paused {
                    "PAUSED"
                } else if review.is_some() {
                    "REPLAY"
                } else if done {
                    match hud.captures[0].cmp(&hud.captures[1]) {
                        std::cmp::Ordering::Greater => "BLUE WINS",
                        std::cmp::Ordering::Less => "RED WINS",
                        std::cmp::Ordering::Equal => "DRAW",
                    }
                } else {
                    "LIVE"
                }
            ),
            screen_width() - 20.0,
            30.0,
            20.0,
            ui::INK,
        );
        ui::text("FULL FIELD / NORMAL BOT SIGHT", x, 72.0, 14.0, ui::MUTED);
        for side in 0..2 {
            let y = 105.0 + side as f32 * 131.0;
            let color = if side == 0 { ui::CYAN } else { ui::PINK };
            ui::text(if side == 0 { "BLUE" } else { "RED" }, x, y, 20.0, color);
            ui::text(&session.names[side], x, y + 23.0, 15.0, ui::INK);
            let first = world.tanks().find(|t| t.id == session.ids[side]);
            let second = world.tanks().find(|t| t.id == session.ids[side + 2]);
            ui::text(
                &format!(
                    "HP {:3.0}/{:3.0} K {} F {}",
                    first.map_or(0.0, |t| t.health),
                    second.map_or(0.0, |t| t.health),
                    hud.kills[side],
                    hud.captures[side]
                ),
                x,
                y + 47.0,
                16.0,
                ui::INK,
            );
            for (row, player, tank) in [(0, side, first), (1, side + 2, second)] {
                let vehicle = tank.map_or("DEPLOY", |t| match t.vehicle {
                    sim::VehicleKind::Scout => "SCOUT",
                    sim::VehicleKind::Tank => "TANK",
                    sim::VehicleKind::Mortar => "MORTAR",
                });
                let state = if let Some(remaining) = tank.and_then(|t| t.respawn_remaining) {
                    format!("RESPAWN {:.0}s", remaining.ceil())
                } else if hud.stalled[player] {
                    "STALLED".into()
                } else {
                    hud.telemetry[player].mode.clone()
                };
                let label = format!(
                    "{}{} {} {} {}",
                    if side == 0 { "B" } else { "R" },
                    player / 2 + 1,
                    vehicle,
                    match hud.telemetry[player].role.as_str() {
                        "breacher" => "BREACH",
                        "support" => "SUPPORT",
                        "guard" => "GUARD",
                        "flank" => "FLANK",
                        _ => "",
                    },
                    state
                );
                ui::text(
                    &label.chars().take(31).collect::<String>(),
                    x,
                    y + 70.0 + row as f32 * 21.0,
                    13.0,
                    if hud.stalled[player] {
                        ORANGE
                    } else {
                        ui::MUTED
                    },
                );
            }
        }
        ui::text("PAUSE THEN SEEK / D-PAD L R", x, 367.0, 14.0, ui::MUTED);
        // Buttons already drawn above; the map is fitted away from this column.
        ui::text("BLUE / RED TINT = TEAM SIGHT", x, 626.0, 13.0, ui::MUTED);
        ui::text("LT / V SIGHT  /  CLICK TO MARK", x, 647.0, 13.0, ui::MUTED);
        if let Some(p) = pin {
            ui::text(
                &format!("MARK  X {:.1} / Y {:.1}", p.x, p.y),
                x,
                672.0,
                16.0,
                ORANGE,
            );
        }
        if !status.is_empty() {
            ui::text(
                &status.chars().take(32).collect::<String>(),
                x,
                688.0,
                14.0,
                ui::PINK,
            );
        }
        draw_rectangle(bar.x, bar.y, bar.w, bar.h, ui::RULE);
        draw_rectangle(
            bar.x,
            bar.y,
            bar.w * hud.tick as f32 / ROUND_TICKS as f32,
            bar.h,
            ui::CYAN,
        );
        ui::text(
            "PAUSE TO REVIEW  /  R REPLAYS THE SAME ROUND",
            24.0,
            screen_height() - 6.0,
            13.0,
            ui::MUTED,
        );
        if is_key_pressed(KeyCode::F12)
            || (settings.screenshot.is_some() && get_time() - started > 2.0)
        {
            let path = settings
                .screenshot
                .take()
                .unwrap_or_else(|| session.history.path.with_extension("screenshot.png"));
            get_screen_data().export_png(&path.to_string_lossy());
        }
        if crate::deck_mode()
            && let Some(delay) =
                std::time::Duration::from_secs_f64(1.0 / 60.0).checked_sub(frame_started.elapsed())
        {
            // Deck's compositor needs the no-vsync workaround. Bound the
            // spectator's render rate explicitly so review screens leave CPU
            // time for native match evaluation instead of rendering uncapped.
            std::thread::sleep(delay);
        }
        next_frame().await;
    }
}
async fn error_screen(error: String) {
    eprintln!("AI battlefield: {error}");
    loop {
        clear_background(ui::PAPER);
        ui::text("AI BATTLEFIELD / CANNOT START", 30.0, 60.0, 26.0, ui::PINK);
        for (i, line) in error.as_bytes().chunks(90).enumerate() {
            ui::text(
                &String::from_utf8_lossy(line),
                30.0,
                110.0 + i as f32 * 25.0,
                18.0,
                ui::INK,
            );
        }
        ui::text(
            "ESC TO CLOSE",
            30.0,
            screen_height() - 35.0,
            18.0,
            ui::MUTED,
        );
        if is_key_pressed(KeyCode::Escape) {
            break;
        }
        next_frame().await;
    }
}
