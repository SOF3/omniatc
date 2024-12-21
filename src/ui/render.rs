use bevy::app::{self, App, Plugin};
use bevy::asset::AssetServer;
use bevy::color::Color;
use bevy::math::{Vec3, Vec3Swizzles};
use bevy::prelude::{
    BuildChildren, Camera2d, ChildBuild, Commands, Component, Entity, EventReader, GlobalTransform,
    IntoSystemConfigs, Parent, Query, Res, Resource, Single, Transform, Visibility, With,
};
use bevy::sprite::Sprite;
use bevy::text::Text2d;

use super::SystemSets;
use crate::level::{object, plane};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<DisplayConfig>();

        app.add_systems(app::Update, spawn_plane_viewable_system.in_set(SystemSets::RenderSpawn));
        app.add_systems(
            app::Update,
            (maintain_target_system, maintain_label_system)
                .in_set(SystemSets::RenderMove),
        );
    }
}

/// Marker component indicating that the entity is the viewable entity showing a target sprite.
#[derive(Component)]
struct TargetViewable;

/// Marker component indicating that the entity is the viewable entity showing a label text.
#[derive(Component)]
struct LabelViewable;

fn spawn_plane_viewable_system(
    mut commands: Commands,
    mut events: EventReader<plane::SpawnEvent>,
    asset_server: Res<AssetServer>,
) {
    for &plane::SpawnEvent(entity) in events.read() {
        commands.entity(entity).insert((Transform::IDENTITY, Visibility::Visible)).with_children(
            |b| {
                b.spawn((
                    Sprite::from_image(asset_server.load("sprites/target.png")),
                    TargetViewable,
                ));
                b.spawn((Text2d::new(""), LabelViewable));
            },
        );
    }
}

fn maintain_target_system(
    parent_query: Query<(&object::Rotation, &object::Position)>,
    mut target_query: Query<(Entity, &Parent, &mut Transform, &mut Sprite), With<TargetViewable>>,
    config: Res<DisplayConfig>,
    camera_transform: Single<&GlobalTransform, With<Camera2d>>,
) {
    target_query.iter_mut().for_each(|(entity, parent, mut transform, mut sprite)| {
        let Ok((rotation, position)) = parent_query.get(parent.get()) else {
            bevy::log::warn_once!("target entity {entity:?} parent {parent:?} is not an object");
            return;
        };

        transform.translation = (position.0.xy(), 0.5).into();
        transform.rotation = rotation.0;
        transform.scale =
            Vec3::new(config.target_size, config.target_size, 1.) * camera_transform.scale();

        sprite.color = Color::srgb((position.0.z / 10.).clamp(0., 1.), 1., 1.);
    });
}

fn maintain_label_system(
    parent_query: Query<(&object::GroundSpeed, &object::Position)>,
    mut label_query: Query<(Entity, &Parent, &mut Text2d, &mut Transform), With<LabelViewable>>,
    config: Res<DisplayConfig>,
    camera_transform: Single<&GlobalTransform, With<Camera2d>>,
) {
    label_query.iter_mut().for_each(|(entity, parent, mut label, mut transform)| {
        let Ok((speed, position)) = parent_query.get(parent.get()) else {
            bevy::log::warn_once!("target entity {entity:?} parent {parent:?} is not an object");
            return;
        };

        label.0 = format!(
            "Speed {:.3}, {:.3}\nPosition {:.3}, {:.3}",
            speed.0.x, speed.0.y, position.0.x, position.0.y
        );
        transform.translation = (position.0.xy(), 0.5).into();
        transform.scale =
            Vec3::new(config.label_size, config.label_size, 1.) * camera_transform.scale();
    });
}

#[derive(Resource)]
pub struct DisplayConfig {
    pub target_size: f32,
    pub label_size:  f32,
}

impl Default for DisplayConfig {
    fn default() -> Self { Self { target_size: 1., label_size: 0.5 } }
}
