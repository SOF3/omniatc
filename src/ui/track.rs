//! Display points indicating an object trail.

use bevy::app::{self, App, Plugin};
use bevy::asset::{self, Assets};
use bevy::color::Color;
use bevy::prelude::{
    BuildChildren, Children, Circle, Commands, Component, DespawnRecursiveExt, DetectChangesMut,
    Entity, EventReader, IntoSystemConfigs, Mesh, Mesh2d, Mut, Parent, Query, Res, ResMut,
    Resource, Transform, Visibility, Without,
};
use bevy::sprite::{ColorMaterial, MeshMaterial2d};
use omniatc_core::level::object;

use super::{billboard, SystemSets, Zorder};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<Config>();
        app.init_resource::<PointSprite>();
        app.add_systems(
            app::Startup,
            |mut point_sprite: ResMut<PointSprite>, mut meshes: ResMut<Assets<Mesh>>| {
                let handle = meshes.add(Circle::new(1.));
                point_sprite.mesh = Some(handle);
            },
        );
        app.add_systems(app::Update, spawn_trail_owner_system.in_set(SystemSets::RenderSpawn));
        app.add_systems(
            app::Update,
            manage_trail_system
                .ambiguous_with(spawn_trail_owner_system)
                .in_set(SystemSets::RenderSpawn),
        );
        app.add_systems(app::Update, move_trail_point_system.in_set(SystemSets::RenderMove));
    }
}

/// Component on object entities referencing its child trail owner entity.
#[derive(Component)]
pub struct TrailOwnerRef(pub Entity);

/// Component on trail owner entity to display the trail.
#[derive(Component)]
pub struct TrailDisplay {
    pub focused: bool,
}

#[derive(Component)]
struct TrailPoint {
    deque_reverse_offset: usize,
}

fn spawn_trail_owner_system(mut events: EventReader<object::SpawnEvent>, mut commands: Commands) {
    events.read().for_each(|&object::SpawnEvent(entity)| {
        let trail_owner = commands
            .spawn((
                bevy::core::Name::new("TrailOwner"),
                TrailDisplay { focused: false },
                Transform::IDENTITY,
                Visibility::Inherited,
            ))
            .id();

        let mut entity_ref = commands.entity(entity);
        entity_ref.add_child(trail_owner);
        entity_ref.insert(TrailOwnerRef(trail_owner));
    });
}

#[derive(Resource, Default)]
struct PointSprite {
    mesh: Option<asset::Handle<Mesh>>,
}

fn manage_trail_system(
    config: Res<Config>,
    point_sprite: Res<PointSprite>,
    mut commands: Commands,
    object_query: Query<(&TrailOwnerRef, &object::Track)>,
    owner_query: Query<(&TrailDisplay, Option<&Children>)>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    object_query.iter().for_each(|(&TrailOwnerRef(trail_owner), log)| {
        let Ok((display, children)) = owner_query.get(trail_owner) else {
            bevy::log::warn!("dangling trail owner reference {trail_owner:?}");
            return;
        };

        let required = if display.focused {
            log.log.len()
        } else {
            log.log.len().min(config.unfocused_display_length)
        };

        let circle_sprite = point_sprite.mesh.as_ref().expect("initialized during startup");

        for index in children.map_or(0, |children| children.len())..required {
            commands.entity(trail_owner).with_child((
                bevy::core::Name::new("TrailPoint"),
                TrailPoint { deque_reverse_offset: index },
                billboard::MaintainScale { size: config.trail_point_size },
                Mesh2d(circle_sprite.clone()),
                MeshMaterial2d(
                    materials.add(ColorMaterial { color: Color::WHITE, ..Default::default() }),
                ),
            ));
        }
        for &entity in children.and_then(|children| children.get(required..)).into_iter().flatten()
        {
            commands.entity(entity).despawn_recursive();
        }
    });
}

fn move_trail_point_system(
    mut point_query: Query<(&TrailPoint, &mut Transform, &Parent)>,
    owner_query: Query<&Parent, Without<TrailPoint>>,
    object_query: Query<&object::Track>,
) {
    point_query.iter_mut().for_each(|(point, tf, owner_entity)| {
        if let Ok(object_entity) = owner_query.get(owner_entity.get()) {
            if let Ok(track) = object_query.get(object_entity.get()) {
                if let Some(&position) =
                    track.log.get(track.log.len().saturating_sub(point.deque_reverse_offset + 1))
                {
                    Mut::map_unchanged(tf, |tf| &mut tf.translation)
                        .set_if_neq(position.get().with_z(Zorder::ObjectTrack.into_z()));
                } else {
                    bevy::log::warn!("track log is shorter than point entity list");
                }
            } else {
                bevy::log::warn!("object entity {object_entity:?} has no track log");
            }
        } else {
            bevy::log::warn!("owner entity {owner_entity:?} has no parent");
        }
    });
}

#[derive(Resource)]
pub struct Config {
    pub unfocused_display_length: usize,
    pub trail_point_size:         f32,
}

impl Default for Config {
    fn default() -> Self { Self { unfocused_display_length: 10, trail_point_size: 1. } }
}
