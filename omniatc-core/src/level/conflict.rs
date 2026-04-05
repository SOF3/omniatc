//! Midair separation conflict detection.
//!
//! A conflict exists when two airborne objects simultaneously violate both the horizontal
//! and vertical separation minima. Each conflict pair is tracked as a dedicated entity
//! carrying [`PairState`].
//!
//! When a pair becomes active, a [`message::Message`] (class [`message::Class::Urgent`])
//! is inserted on the pair entity so the client message display shows the warning automatically.
//! It is removed when the objects separate again.
//!
//! Objects with at least one active conflict pair have the [`ActiveObject`] marker component
//! inserted; it is removed when no active pairs remain.

use std::marker::PhantomData;
use std::time::Duration;

use bevy::app::{self, App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::entity::{Entity, EntityHashMap};
use bevy::ecs::message::MessageReader;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query};
use bevy_mod_config::{AppExt, Config, ConfigFieldFor, Manager};
use math::Length;

use super::{SystemSets, message, object, score};

/// Generic conflict-detection plugin parameterised by the same config manager as the rest
/// of the level plugins.
pub struct Plug<M>(PhantomData<M>);

impl<M> Default for Plug<M> {
    fn default() -> Self { Self(PhantomData) }
}

impl<M: Manager + Default> Plugin for Plug<M>
where
    Conf: ConfigFieldFor<M>,
{
    fn build(&self, app: &mut App) {
        app.init_config::<M, Conf>("core:conflict");
        app.add_systems(app::Update, handle_despawn_system.in_set(SystemSets::Statistics));
        app.add_systems(app::Update, detect::system().after(handle_despawn_system));
    }
}

/// Per-object index of known conflict pairs.
///
/// Inserted automatically on every [`object::Object`] via `#[require]`.
///
/// **Key**: a peer object entity.
/// **Value**: the dedicated [`PairState`] entity for the relationship between this object
/// and the peer.
/// Both objects in a pair reference the **same** pair entity.
#[derive(Component, Default)]
pub struct Record {
    peers: EntityHashMap<Entity>,
}

impl Record {
    /// Returns the pair entity for the given peer, if one exists.
    #[must_use]
    pub fn pair_for(&self, peer: Entity) -> Option<Entity> { self.peers.get(&peer).copied() }
}

/// Persistent state stored on a dedicated pair entity — one per conflict pair.
///
/// Both objects' [`Record`] entries point to this entity.
/// Spawned when a pair first violates separation minima; despawned only when one
/// of the two objects is removed from the world.
#[derive(Component)]
pub struct PairState {
    /// The object with the lower entity index in the pair.
    pub entity_a:  Entity,
    /// The object with the higher entity index in the pair.
    pub entity_b:  Entity,
    /// Total time this pair has violated separation minima, including non-contiguous frames.
    pub cumul:     Duration,
    /// Whether the pair is currently in violation this tick.
    pub is_active: bool,

    /// Score penalty accumulated but not yet committed to [`Score`].
    ///
    /// Score deductions must be integers (`Score(i32)`),
    /// but the per-frame penalty formula yields a continuous floating-point value.
    /// The fractional remainder is kept here
    /// and added to future frames so that no penalty is lost to rounding.
    uncommitted_score: f32,
}

impl PairState {
    /// Takes the integer portion of the accumulated uncommitted score, leaving any fractional
    /// remainder for future frames.
    fn take_uncommitted_score(&mut self) -> i32 {
        if self.uncommitted_score < 1.0 {
            return 0;
        }
        let taken = self.uncommitted_score.floor();
        self.uncommitted_score -= taken;
        #[expect(clippy::cast_possible_truncation, reason = "overflow would be pathological")]
        let taken = taken as i32;
        taken
    }
}

/// Marker component inserted on an object while it has at least one active conflict pair.
///
/// Removed when no active pairs remain.
/// The client uses this to change visual indicators such as the separation ring color.
#[derive(Component)]
pub struct ActiveObject;

/// Configuration for conflict detection, keyed `core:conflict`.
#[derive(Config)]
pub struct Conf {
    /// Horizontal separation minimum.
    #[config(default = Length::from_nm(3.0), min = Length::ZERO, max = Length::from_nm(20.0))]
    pub horiz_sep:        Length<f32>,
    /// Vertical separation minimum.
    #[config(default = Length::from_feet(1000.0), min = Length::ZERO, max = Length::from_feet(10000.0))]
    pub vert_sep:         Length<f32>,
    /// Multiplier applied to the conflict penalty.
    #[config(default = 100.0, min = 0.0, max = 10000.0)]
    pub score_multiplier: f32,
}

mod detect;

fn handle_despawn_system(
    mut despawn_reader: MessageReader<object::DespawnMessage>,
    pair_query: Query<(Entity, &PairState)>,
    mut record_query: Query<&mut Record>,
    mut commands: Commands,
) {
    let dead_entities: Vec<Entity> = despawn_reader.read().map(|msg| msg.0).collect();
    if dead_entities.is_empty() {
        return;
    }

    let to_remove = pair_query.iter().filter_map(|(pair_entity, pair_state)| {
        if dead_entities.contains(&pair_state.entity_a) {
            Some((pair_entity, pair_state.entity_b, pair_state.entity_a))
        } else if dead_entities.contains(&pair_state.entity_b) {
            Some((pair_entity, pair_state.entity_a, pair_state.entity_b))
        } else {
            None
        }
    });

    for (pair_entity, survivor, dead) in to_remove {
        if let Ok(mut record) = record_query.get_mut(survivor) {
            record.peers.remove(&dead);
        }
        commands.entity(pair_entity).despawn();
    }
}
