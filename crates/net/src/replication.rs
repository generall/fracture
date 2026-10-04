//! Per-player interest sets and persistent geometry with quantized motion deltas.
use serde::{Deserialize, Serialize};
use sim::{BodyView, Material, Shape, TankFragment, Vec2, Vision, replica::Frame};
use std::collections::BTreeMap;

/// Clients are trusted with the whole scene and compute sight locally; only
/// host physics diagnostics are withheld.
pub fn strip_diagnostics(frame: &mut Frame) {
    frame.stats = sim::Stats {
        ticks: frame.stats.ticks,
        ..Default::default()
    };
}
pub fn visible_events(events: &[sim::Event], vision: &Vision, player: usize) -> Vec<sim::Event> {
    events
        .iter()
        .copied()
        .filter(|event| {
            use sim::Event::*;
            match event {
                TankDestroyed { details, .. } if details.id == player => true,
                Fired { position, .. }
                | Fractured { position, .. }
                | Exploded { position, .. }
                | Impacted { position, .. }
                | Collided { position, .. }
                | Buckled { position, .. }
                | Bent { position, .. }
                | DecorationDestroyed { position, .. }
                | BridgeCollapsed { position, .. }
                | TankDestroyed { position, .. }
                | TankRespawned { position, .. }
                | TankSunk { position, .. } => vision.contains(*position),
                FlagCaptured { .. } => true,
                MuzzleBlocked | LaserLimited => false,
            }
        })
        .collect()
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Definition {
    bridge_deck: bool,
    id: u32,
    object: Option<usize>,
    chunk: u32,
    origin: usize,
    shape: Shape,
    revision: u32,
    fragment: Option<TankFragment>,
    edges: u8,
    material: Material,
    color: Option<[u8; 3]>,
    fixed: bool,
    sight: bool,
}
impl Definition {
    fn capture(b: &BodyView) -> Self {
        Self {
            bridge_deck: b.bridge_deck,
            id: b.id as u32,
            object: b.object,
            chunk: b.chunk,
            origin: b.origin,
            shape: b.shape,
            revision: b.shape_revision,
            fragment: b.tank_fragment,
            edges: b.edges,
            material: b.material,
            color: b.color,
            fixed: b.fixed,
            sight: b.blocks_sight,
        }
    }
    fn body(&self, m: Motion) -> BodyView {
        let position = Vec2::new(m.position[0] as f32, m.position[1] as f32) * 0.001;
        let angle = m.angle as f32 * std::f32::consts::PI / 32767.0;
        BodyView {
            bridge_deck: self.bridge_deck,
            id: self.id as usize,
            object: self.object,
            chunk: self.chunk,
            origin: self.origin,
            position,
            previous_position: position,
            angle,
            previous_angle: angle,
            velocity: Vec2::new(m.velocity[0] as f32, m.velocity[1] as f32) * 0.01,
            shape: self.shape,
            shape_revision: self.revision,
            tank_fragment: self.fragment,
            plastic_strain: m.plastic as f32 * 0.001,
            edges: self.edges,
            material: self.material,
            color: self.color,
            load: m.load as f32 / 32.0,
            sleeping: m.sleeping,
            fixed: self.fixed,
            opacity: m.opacity as f32 / 255.0,
            blocks_sight: self.sight,
        }
    }
}
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Motion {
    position: [i32; 2],
    angle: i16,
    velocity: [i16; 2],
    opacity: u8,
    load: u8,
    plastic: u16,
    sleeping: bool,
}
impl Motion {
    fn capture(b: &BodyView) -> Self {
        let angle = (b.angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        Self {
            position: [
                (b.position.x * 1000.0).round() as i32,
                (b.position.y * 1000.0).round() as i32,
            ],
            angle: (angle * 32767.0 / std::f32::consts::PI).round() as i16,
            velocity: [
                (b.velocity.x * 100.0).round() as i16,
                (b.velocity.y * 100.0).round() as i16,
            ],
            opacity: (b.opacity * 255.0).round() as u8,
            load: (b.load * 32.0).round() as u8,
            plastic: (b.plastic_strain * 1000.0).round() as u16,
            sleeping: b.sleeping,
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Delta {
    pub sequence: u64,
    pub base: u64,
    pub definitions: Vec<Definition>,
    pub motion: Vec<(u32, Motion)>,
    pub hidden: Vec<u32>,
    pub frame: Frame,
    pub vision: Option<Vision>,
}
/// Body records captured once per host snapshot and shared by every peer.
pub struct Records(Vec<(u32, Definition, Motion)>);
impl Records {
    pub fn capture(bodies: &[BodyView]) -> Self {
        let mut records: Vec<_> = bodies
            .iter()
            .map(|b| (b.id as u32, Definition::capture(b), Motion::capture(b)))
            .collect();
        records.sort_unstable_by_key(|r| r.0);
        Self(records)
    }
}
/// Call only when the entire resulting frame can be queued. Reliable ordered
/// transport means the next delta can use the last queued sequence as its base.
#[derive(Default)]
pub struct Encoder {
    vision: Option<Vision>,
    sequence: u64,
    /// Last queued records, sorted by ID, for a linear merge against the next.
    sent: Vec<(u32, Definition, Motion)>,
}
impl Encoder {
    /// Single-peer convenience: capture the frame's own bodies as records.
    pub fn encode_frame(&mut self, mut frame: Frame) -> Delta {
        let records = Records::capture(&std::mem::take(&mut frame.bodies));
        self.encode(frame, &records)
    }
    pub fn encode(&mut self, mut frame: Frame, records: &Records) -> Delta {
        assert!(frame.bodies.is_empty(), "bodies travel as shared records");
        let vision = if self.vision.as_ref() != Some(&frame.vision) {
            self.vision = Some(frame.vision.clone());
            Some(std::mem::take(&mut frame.vision))
        } else {
            frame.vision = Vision::default();
            None
        };
        let base = self.sequence;
        self.sequence += 1;
        let (mut definitions, mut motion, mut hidden) = (vec![], vec![], vec![]);
        let mut old = self.sent.iter().peekable();
        for (id, d, m) in &records.0 {
            while let Some((gone, ..)) = old.next_if(|o| o.0 < *id) {
                hidden.push(*gone);
            }
            match old.next_if(|o| o.0 == *id) {
                Some((_, od, om)) => {
                    if od != d {
                        definitions.push(d.clone());
                    }
                    if om != m {
                        motion.push((*id, *m));
                    }
                }
                None => {
                    definitions.push(d.clone());
                    motion.push((*id, *m));
                }
            }
        }
        hidden.extend(old.map(|o| o.0));
        self.sent.clone_from(&records.0);
        Delta {
            sequence: self.sequence,
            vision,
            base,
            definitions,
            motion,
            hidden,
            frame,
        }
    }
}
#[derive(Default)]
pub struct Decoder {
    vision: Option<Vision>,
    sequence: u64,
    definitions: BTreeMap<u32, Definition>,
    motion: BTreeMap<u32, Motion>,
}
impl Decoder {
    pub fn decode(&mut self, mut delta: Delta) -> Result<Frame, String> {
        if delta.base != self.sequence || delta.sequence != self.sequence + 1 {
            return Err("Snapshot sequence gap".into());
        }
        if !delta.frame.bodies.is_empty() {
            return Err("Unexpected inline geometry".into());
        }
        if let Some(vision) = delta.vision {
            self.vision = Some(vision);
        }
        delta.frame.vision = self.vision.clone().ok_or("Missing visibility state")?;
        for d in delta.definitions {
            self.definitions.insert(d.id, d);
        }
        if self.definitions.len() > 250_000 {
            return Err("Geometry limit exceeded".into());
        }
        for id in delta.hidden {
            self.motion.remove(&id);
        }
        for (id, m) in delta.motion {
            if !self.definitions.contains_key(&id) {
                return Err("Unknown geometry".into());
            }
            self.motion.insert(id, m);
        }
        delta.frame.bodies = self
            .motion
            .iter()
            .map(|(id, m)| self.definitions[id].body(*m))
            .collect();
        self.sequence = delta.sequence;
        Ok(delta.frame)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use sim::{
        World,
        level::{Level, Team},
    };
    fn through_wire(world: &World) -> World {
        let bytes =
            crate::codec::encode(&Encoder::default().encode_frame(Frame::capture(world))).unwrap();
        let (delta, _) = crate::codec::decode::<Delta>(&bytes).unwrap().unwrap();
        let frame = Decoder::default().decode(delta).unwrap();
        World::from_replica(sim::replica::Map::capture(world), frame)
    }
    #[test]
    fn bridge_deck_rendering_tag_survives_wire_without_hiding_supports() {
        let world = World::new(sim::Scene::Terrain);
        let client = through_wire(&world);
        let deck: Vec<_> = world.bodies().filter(|b| b.bridge_deck).collect();
        assert!(!deck.is_empty());
        let object = deck[0].object;
        assert!(world.bodies().any(|b| b.object == object && !b.bridge_deck));
        let received: BTreeMap<_, _> = client.bodies().map(|b| (b.id, b.bridge_deck)).collect();
        for body in world.bodies() {
            assert_eq!(received.get(&body.id), Some(&body.bridge_deck));
        }
    }
    #[test]
    fn bridge_repair_progress_and_completion_survive_wire_updates() {
        let world = World::new(sim::Scene::Terrain);
        let mut frame = Frame::capture(&world);
        let mut encoder = Encoder::default();
        let mut decoder = Decoder::default();
        for progress in [0.0, 0.5, 1.0] {
            frame.bridges[0].intact = progress == 1.0;
            frame.bridges[0].repairs[1] = sim::BridgeRepairView {
                team: Some(Team::Red),
                progress,
                contested: progress == 0.5,
            };
            let bytes = crate::codec::encode(&encoder.encode_frame(frame.clone())).unwrap();
            let (delta, _) = crate::codec::decode::<Delta>(&bytes).unwrap().unwrap();
            let received = decoder.decode(delta).unwrap();
            assert_eq!(received.bridges[0].repairs, frame.bridges[0].repairs);
            assert_eq!(received.bridges[0].intact, progress == 1.0);
        }
    }
    #[test]
    fn public_airdrop_and_removal_survive_wire_updates() {
        let world = World::from_level(&Level::default(), Team::Blue).unwrap();
        let mut frame = Frame::capture(&world);
        let drop = sim::AirdropView {
            position: Vec2::new(15.0, 9.0),
            weapon: sim::SpecialWeapon::Rocket,
            remaining: 30.0,
        };
        assert!(
            !frame.vision.contains(drop.position),
            "drop is outside player's vision"
        );
        let mut encoder = Encoder::default();
        let mut decoder = Decoder::default();
        for expected in [Some(drop), None] {
            frame.airdrop = expected;
            strip_diagnostics(&mut frame);
            let bytes = crate::codec::encode(&encoder.encode_frame(frame.clone())).unwrap();
            let (delta, _) = crate::codec::decode::<Delta>(&bytes).unwrap().unwrap();
            let received = decoder.decode(delta).unwrap();
            let client = World::from_replica(sim::replica::Map::capture(&world), received);
            assert_eq!(client.airdrop(), expected);
        }
    }

    #[test]
    fn rear_shot_detection_survives_wire_and_expires_on_clients() {
        use sim::{
            level::LevelItem,
            multiplayer::{MultiplayerWorld, PlayerInput},
        };
        let mut level = Level::default();
        if let LevelItem::Spawn { position, .. } = &mut level.items[0] {
            *position = [0.0, 0.0];
        }
        if let LevelItem::Spawn { position, .. } = &mut level.items[1] {
            *position = [-8.0, 0.0];
        }
        let mut game = MultiplayerWorld::new(&level, &[Team::Blue, Team::Red]).unwrap();
        let enemy = game.tank_id(1).unwrap();
        let mut inputs = [
            PlayerInput {
                aim: Vec2::new(20.0, 0.0),
                ..Default::default()
            },
            PlayerInput {
                aim: Vec2::new(-20.0, 0.0),
                fire: true,
                ..Default::default()
            },
        ];
        game.step(&inputs);
        let client = through_wire(game.for_player(0).unwrap());
        assert!(client.player_vision(0.0).firing_tank_visible(enemy));
        assert!(!client.player_vision(0.0).contains(Vec2::new(-8.0, 0.0)));
        inputs[1].fire = false;
        for _ in 0..119 {
            game.step(&inputs);
        }
        let client = through_wire(game.for_player(0).unwrap());
        assert!(!client.player_vision(0.0).firing_tank_visible(enemy));
    }
    #[test]
    fn client_vision_from_wire_matches_host_including_cover() {
        let mut level = Level::default();
        if let sim::level::LevelItem::Spawn { position, .. } = &mut level.items[1] {
            *position = [-6.0, 0.0];
        }
        level.items.push(sim::level::LevelItem::Object {
            asset: Box::new(sim::LibraryPreset::Wall.asset()),
            position: [-12.0, 1.5],
            angle: std::f32::consts::FRAC_PI_2,
            cutouts: vec![],
        });
        let world = World::from_level(&level, Team::Blue).unwrap();
        let replica = through_wire(&world);
        let host = world.player_vision(world.player_aim().angle);
        let client = replica.player_vision(0.0);
        let (mut agree, mut hidden) = (0, 0);
        for x in -40..=40 {
            for y in -30..=30 {
                let p = Vec2::new(x as f32 * 0.5 - 4.0, y as f32 * 0.5);
                if host.contains(p) == client.contains(p) {
                    agree += 1;
                }
                hidden +=
                    usize::from(!host.contains(p) && p.distance(world.launcher_position()) < 12.0);
            }
        }
        assert!(
            hidden > 50,
            "the wall must actually occlude part of the cone"
        );
        assert!(agree >= 81 * 61 - 10, "client sight diverges: {agree}");
        let enemy = world.tanks().find(|t| t.team == Team::Red).unwrap().id;
        assert!(replica.bodies().any(|b| b.id == enemy));
        assert_eq!(client.tank_visibility(enemy), host.tank_visibility(enemy));
    }
    #[test]
    fn enemy_outside_client_vision_is_replicated_but_not_visible() {
        let w = World::from_level(&Level::default(), Team::Blue).unwrap();
        let enemy = w.tanks().find(|t| t.team == Team::Red).unwrap();
        let replica = through_wire(&w);
        let enemy_body = replica.bodies().find(|b| b.id == enemy.id).unwrap();
        assert!(!replica.player_vision(0.0).contains(enemy_body.position));
    }
    #[test]
    fn shape_changes_update_cached_geometry_without_resending_stationary_motion() {
        let world = World::from_level(&Level::default(), Team::Blue).unwrap();
        let mut frame = Frame::capture(&world);
        let mut encoder = Encoder::default();
        let mut decoder = Decoder::default();
        decoder.decode(encoder.encode_frame(frame.clone())).unwrap();
        frame.bodies[0].shape = Shape::Ball { radius: 2.0 };
        frame.bodies[0].shape_revision += 1;
        let delta = encoder.encode_frame(frame);
        assert_eq!(delta.definitions.len(), 1);
        assert!(delta.motion.is_empty());
        assert!(delta.vision.is_none());
        let frame = decoder.decode(delta).unwrap();
        assert!(matches!(frame.bodies[0].shape, Shape::Ball { radius: 2.0 }));
    }
    #[test]
    fn stationary_geometry_is_sent_once_removed_bodies_stop_and_reentry_restores() {
        let w = World::from_level(&Level::default(), Team::Blue).unwrap();
        let frame = Frame::capture(&w);
        let mut encoder = Encoder::default();
        let mut decoder = Decoder::default();
        let first = encoder.encode_frame(frame.clone());
        let bytes = crate::codec::encode(&first).unwrap();
        let (wire, _) = crate::codec::decode::<Delta>(&bytes).unwrap().unwrap();
        let decoded = decoder.decode(wire).unwrap();
        assert_eq!(decoded.bodies.len(), frame.bodies.len());
        for (a, b) in decoded.bodies.iter().zip(&frame.bodies) {
            assert!(a.position.distance(b.position) < 0.001);
        }
        let second = encoder.encode_frame(frame.clone());
        assert!(second.definitions.is_empty());
        assert!(second.motion.is_empty());
        decoder.decode(second).unwrap();
        let mut hidden = frame.clone();
        hidden.bodies.clear();
        let hidden = encoder.encode_frame(hidden);
        assert_eq!(hidden.hidden.len(), frame.bodies.len());
        assert!(decoder.decode(hidden).unwrap().bodies.is_empty());
        let reentry = encoder.encode_frame(frame.clone());
        assert_eq!(
            decoder.decode(reentry).unwrap().bodies.len(),
            frame.bodies.len()
        );
    }
    #[test]
    fn map_binary_round_trip_includes_authored_terrain() {
        let level =
            Level::from_json(include_str!("../../../web/levels/quarry-crossing.json")).unwrap();
        let w = World::from_level(&level, Team::Blue).unwrap();
        let map = sim::replica::Map::capture(&w);
        let bytes = crate::codec::encode(&map).unwrap();
        let (decoded, _) = crate::codec::decode::<sim::replica::Map>(&bytes)
            .unwrap()
            .unwrap();
        assert_eq!(decoded.terrain, map.terrain);
        assert_eq!(decoded.mask, map.mask);
    }
}
