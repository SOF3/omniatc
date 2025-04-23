use bevy::app::{self, App, Plugin};
use bevy::asset::AssetServer;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EventReader;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::query::{self};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::render::view::Visibility;
use bevy::sprite::{Anchor, Sprite};
use bevy::text::Text2d;
use bevy::transform::components::Transform;
use omniatc_core::level::object::{self, Object};
use omniatc_core::level::plane;
use omniatc_core::units::Distance;

use super::Zorder;
use crate::config::{self, AppExt as _};
use crate::render;
use crate::util::billboard;

mod label;
use label::IsLabelOf;

mod separation_ring;

pub struct Plug;

#[derive(Resource)]
pub struct Conf {
    /// Sprite for planes.
    plane_sprite_path:         String,
    /// Size of plane sprites.
    plane_sprite_size:         f32,
    /// Size of object labels.
    label_size:                f32,
    /// Distance of object labels from the object center, in screen coordinates.
    label_distance:            f32,
    /// Direction of the object relative to the label.
    label_anchor:              Anchor,
    /// World radius of the separation ring.
    separation_ring_radius:    Distance<f32>,
    /// Viewport thickness of the separation ring.
    separation_ring_thickness: f32,
}

impl Default for Conf {
    fn default() -> Self {
        Self {
            plane_sprite_path:         "sprites/plane.png".into(),
            plane_sprite_size:         1.0,
            label_size:                0.5,
            label_distance:            50.,
            label_anchor:              Anchor::BottomLeft,
            separation_ring_radius:    Distance::from_nm(1.5),
            separation_ring_thickness: 2.,
        }
    }
}

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_config::<Conf>();
        app.add_systems(app::Update, spawn_plane_system.in_set(render::SystemSets::Spawn));
        app.add_systems(app::Update, maintain_plane_system.in_set(render::SystemSets::Update));
        app.add_plugins(separation_ring::Plug);
    }
}

#[derive(Component)]
#[relationship(relationship_target = HasSprite)]
struct IsSpriteOf(Entity);

#[derive(Component)]
#[relationship_target(relationship = IsSpriteOf, linked_spawn)]
struct HasSprite(Entity);

fn spawn_plane_system(
    mut commands: Commands,
    conf: config::Read<Conf>,
    mut spawn_events: EventReader<plane::SpawnEvent>,
    asset_server: Res<AssetServer>,
) {
    for &plane::SpawnEvent(plane_entity) in spawn_events.read() {
        commands.entity(plane_entity).insert((Transform::IDENTITY, Visibility::Visible));

        commands.spawn((
            IsSpriteOf(plane_entity),
            ChildOf(plane_entity),
            Zorder::ObjectSprite.local_translation(),
            Sprite::from_image(asset_server.load(&conf.plane_sprite_path)),
            billboard::MaintainScale { size: conf.plane_sprite_size },
        ));
        commands.spawn((
            IsLabelOf(plane_entity),
            ChildOf(plane_entity),
            Zorder::ObjectLabel.local_translation(),
            billboard::MaintainScale { size: conf.label_size },
            billboard::MaintainRotation,
            billboard::Label { offset: Distance::ZERO, distance: conf.label_distance },
            Text2d::new(""),
            conf.label_anchor,
        ));
        commands.spawn((ChildOf(plane_entity), Zorder::ObjectSeparationRing.local_translation()));
    }
}

fn maintain_plane_system(
    conf: config::Read<Conf>,
    mut object_query: Query<(
        &HasSprite,
        &Object,
        &object::Rotation,
        label::ObjectData,
        &mut Transform,
    )>,
    mut sprite_query: Query<&mut Transform, (query::With<IsSpriteOf>, query::Without<Object>)>,
    mut label_writer: label::Writer,
) {
    object_query.iter_mut().for_each(
        |(&HasSprite(sprite_entity), object, object_rot, label_data, mut object_tf)| {
            object_tf.translation = Zorder::base_translation(object.position);

            if let Ok(mut sprite_tf) = sprite_query.get_mut(sprite_entity) {
                sprite_tf.rotation = object_rot.0;
            }

            label_data.write_label(&conf, &mut label_writer);
        },
    );
}
