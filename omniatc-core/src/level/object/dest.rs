use bevy::prelude::{Component, Entity};
use math::{Distance, Position};

/// Objective for the flight.
#[derive(Component)]
pub enum Destination {
    /// Object can be handed over upon vacating a runway in the specific aerodrome.
    Landing { aerodrome: Entity },
    /// Object can be handed over upon vacating any runway.
    VacateAnyRunway,
    // TODO: apron/taxiway arrival.
    /// Reach a given waypoint and a given altitude.
    ///
    /// Either condition is set to `None` upon completion.
    /// The control of the object is completed when both are `None`.
    ReachWaypoint {
        min_altitude:       Option<Position<f32>>,
        waypoint_proximity: Option<(Entity, Distance<f32>)>,
    },
}
