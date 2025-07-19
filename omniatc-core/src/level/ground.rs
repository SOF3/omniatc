use bevy::app::{App, Plugin};
use bevy::ecs::world::EntityWorldMut;
use bevy::math::{Dir2, Vec2};
use bevy::prelude::{Component, Entity, EntityCommand, Event, Name};
use math::{Length, Position, Speed};
use smallvec::SmallVec;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_event::<SegmentChangedEvent>();
        app.add_event::<EndpointChangedEvent>();
    }
}

/// The aerodrome owning a segment.
#[derive(Component)]
#[relationship_target(relationship = SegmentOf, linked_spawn)]
pub struct AerodromeSegments(Vec<Entity>);

impl AerodromeSegments {
    #[must_use]
    pub fn segments(&self) -> &[Entity] { &self.0 }
}

/// The aerodrome owning a segment.
#[derive(Component)]
#[relationship(relationship_target = AerodromeSegments)]
pub struct SegmentOf(pub Entity);

/// The aerodrome owning a endpoint.
#[derive(Component)]
#[relationship_target(relationship = EndpointOf, linked_spawn)]
pub struct AerodromeEndpoints(Vec<Entity>);

impl AerodromeEndpoints {
    #[must_use]
    pub fn endpoints(&self) -> &[Entity] { &self.0 }
}

/// The aerodrome owning a endpoint.
#[derive(Component)]
#[relationship(relationship_target = AerodromeEndpoints)]
pub struct EndpointOf(pub Entity);

/// The runway owning a segment.
///
/// This is not a relationship component because there are two owner runways for each segment.
#[derive(Component)]
pub struct SegmentOfRunway(pub [Entity; 2]);

/// The segments owned by a runway.
///
/// This is not a relationship component because there are two owner runways for each segment.
#[derive(Component, Default)]
pub struct RunwaySegments(pub Vec<Entity>);

/// A segment of a ground path to taxi on.
///
/// # Component topology
/// - is always a child entity of an [aerodrome](super::aerodrome) entity.
/// - always has a [`SegmentLabel`] component.
/// - is referenced by [`RunwaySegmentList`] from a runway entity if it belongs to a runway.
#[derive(Component)]
pub struct Segment {
    /// An [`Endpoint`] entity.
    pub alpha:     Entity,
    /// An [`Endpoint`] entity.
    pub beta:      Entity,
    pub width:     Length<f32>,
    pub max_speed: Speed<f32>,
    pub elevation: Position<f32>,
}

impl Segment {
    /// Returns the endpoint that is not equal to `not`.
    ///
    /// Returns `None` if `not` is not exactly one of the two endpoints.
    #[must_use]
    pub fn other_endpoint(&self, not: Entity) -> Option<Entity> {
        if self.alpha == not && self.beta != not {
            Some(self.beta)
        } else if self.alpha != not && self.beta == not {
            Some(self.alpha)
        } else {
            None
        }
    }

    #[must_use]
    pub fn direction_from(&self, from: Entity) -> Option<SegmentDirection> {
        if self.alpha == from {
            Some(SegmentDirection::AlphaToBeta)
        } else if self.beta == from {
            Some(SegmentDirection::BetaToAlpha)
        } else {
            None
        }
    }

    #[must_use]
    pub fn direction_to(&self, to: Entity) -> Option<SegmentDirection> {
        if self.alpha == to {
            Some(SegmentDirection::BetaToAlpha)
        } else if self.beta == to {
            Some(SegmentDirection::AlphaToBeta)
        } else {
            None
        }
    }

    #[must_use]
    pub fn by_direction(&self, direction: SegmentDirection) -> (Entity, Entity) {
        match direction {
            SegmentDirection::AlphaToBeta => (self.alpha, self.beta),
            SegmentDirection::BetaToAlpha => (self.beta, self.alpha),
        }
    }

    #[must_use]
    pub fn contains_pos(
        &self,
        alpha: Position<Vec2>,
        beta: Position<Vec2>,
        pos: Position<Vec2>,
    ) -> bool {
        let ab = beta - alpha;
        let Ok(ab_dir) = Dir2::new(ab.0) else { return false };
        let ap = pos - alpha;
        let ap_on_ab = ap.project_onto_dir(ab_dir);
        let horiz_dir = Dir2::new_unchecked(Vec2 { x: ab_dir.y, y: -ab_dir.x });
        ab.magnitude_cmp() >= ap_on_ab
            && !ap_on_ab.is_negative()
            && self.width >= ap.project_onto_dir(horiz_dir).abs()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentDirection {
    AlphaToBeta,
    BetaToAlpha,
}

/// Identifies a segment.
///
/// Multiple segments may have the same label.
#[derive(Component, Clone, Debug)]
pub enum SegmentLabel {
    /// The segment is part of a taxiway.
    Taxiway { name: String },
    /// The segment is part of a runway.
    RunwayPair([Entity; 2]),
    /// The segment is the path leading into an apron.
    Apron { name: String },
}

impl PartialEq for SegmentLabel {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (&SegmentLabel::RunwayPair(self_rwy), &SegmentLabel::RunwayPair(other_rwy)) => {
                self_rwy == other_rwy || self_rwy == [other_rwy[1], other_rwy[0]]
            }
            (
                SegmentLabel::Taxiway { name: self_twy },
                SegmentLabel::Taxiway { name: other_twy },
            ) => self_twy == other_twy,
            (
                SegmentLabel::Apron { name: self_apron },
                SegmentLabel::Apron { name: other_apron },
            ) => self_apron == other_apron,
            _ => false,
        }
    }
}

impl Eq for SegmentLabel {}

/// The intersection between segments.
#[derive(Component)]
pub struct Endpoint {
    pub position:  Position<Vec2>,
    /// Unordered list of [`Segment`] entities connected to this endpoint.
    pub adjacency: SmallVec<[Entity; 4]>,
}

pub struct SpawnSegment {
    pub segment: Segment,
    pub label:   SegmentLabel,
}
impl EntityCommand for SpawnSegment {
    fn apply(self, mut entity: EntityWorldMut) {
        let alpha_endpoint = self.segment.alpha;
        let beta_endpoint = self.segment.beta;

        if let SegmentLabel::RunwayPair(runways) = self.label {
            entity.insert(SegmentOfRunway(runways));
            let entity_id = entity.id();
            entity.world_scope(|world| {
                for runway_id in runways {
                    let mut runway_ref = world.entity_mut(runway_id);
                    runway_ref.insert_if_new(RunwaySegments(Vec::new()));
                    let mut runway_segments =
                        runway_ref.get_mut::<RunwaySegments>().expect("just inserted");
                    runway_segments.0.push(entity_id);
                }
            });
        }

        entity.insert((
            self.segment,
            Name::new(format!("GroundSegment {:?}", &self.label)),
            self.label,
        ));

        let entity_id = entity.id();
        entity.world_scope(|world| {
            world.send_event(SegmentChangedEvent(entity_id));

            for endpoint in [alpha_endpoint, beta_endpoint] {
                world
                    .get_mut::<Endpoint>(endpoint)
                    .expect("invalid endpoint reference in spawned segment")
                    .adjacency
                    .push(entity_id);
                world.send_event(EndpointChangedEvent(endpoint));
            }
        });
    }
}

pub struct SpawnEndpoint {
    pub position: Position<Vec2>,
}
impl EntityCommand for SpawnEndpoint {
    fn apply(self, mut entity: EntityWorldMut) {
        entity.insert((
            Endpoint { position: self.position, adjacency: SmallVec::new() },
            Name::new("GroundEndpoint"),
        ));
        let entity_id = entity.id();
        entity.world_scope(|world| world.send_event(EndpointChangedEvent(entity_id)));
    }
}

/// Dispatched after a segment is spawned or updated.
#[derive(Event)]
pub struct SegmentChangedEvent(pub Entity);

/// Dispatched after an endpoint is spawned or updated.
#[derive(Event)]
pub struct EndpointChangedEvent(pub Entity);
