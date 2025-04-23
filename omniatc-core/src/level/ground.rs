use bevy::app::{App, Plugin};
use bevy::ecs::world::EntityWorldMut;
use bevy::math::Vec2;
use bevy::prelude::{Component, Entity, EntityCommand, Event, Name};
use smallvec::SmallVec;

use crate::units::Position;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_event::<SegmentChangedEvent>();
        app.add_event::<EndpointChangedEvent>();
    }
}

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

/// The intersection between segments.
#[derive(Component)]
pub struct Endpoint {
    pub position:  Position<Vec2>,
    /// Unordered list of [`Segment`] entities connected to this endpoint.
    pub adjacency: SmallVec<[Entity; 3]>,
}

pub struct SpawnSegment {
    pub segment: Segment,
    pub label:   SegmentLabel,
}
impl EntityCommand for SpawnSegment {
    fn apply(self, mut entity: EntityWorldMut) {
        let alpha_endpoint = self.segment.alpha;
        let beta_endpoint = self.segment.beta;

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
