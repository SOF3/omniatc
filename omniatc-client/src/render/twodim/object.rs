use std::time::Duration;

use bevy::app::{self, App, Plugin};
use bevy::asset::AssetServer;
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EventReader;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::query::{self, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy::ecs::system::{Commands, ParamSet, Query, Res};
use bevy::render::view::Visibility;
use bevy::sprite::{Anchor, Sprite};
use bevy::text::Text2d;
use bevy::transform::components::Transform;
use omniatc::level::object::{self, Object};
use omniatc::level::plane;
use omniatc::math::TROPOPAUSE_ALTITUDE;
use omniatc::units::{Distance, Position};
use omniatc::util::EnumScheduleConfig;
use omniatc_macros::Config;
use serde::{Deserialize, Serialize};

use super::Zorder;
use crate::config::{self, AppExt as _};
use crate::render;
use crate::util::billboard;

mod base_color;

mod label;
use label::IsLabelOf;

pub mod preview;
mod separation_ring;
mod track;
mod vector;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_config::<Conf>();
        app.add_systems(
            app::Update,
            handle_config_change_system.in_set(render::SystemSets::Reload),
        );
        app.add_systems(app::Update, spawn_plane_system.in_set(render::SystemSets::Spawn));
        app.add_systems(
            app::Update,
            maintain_plane_system
                .in_set(render::SystemSets::Update)
                .after_all::<SetColorThemeSystemSet>(),
        );
        app.add_plugins(separation_ring::Plug);
        app.add_plugins(vector::Plug);
        app.add_plugins(track::Plug);
        app.add_plugins(preview::Plug);
        app.add_plugins(base_color::Plug);
        omniatc::util::configure_ordered_system_sets::<SetColorThemeSystemSet>(app, app::Update);
    }
}

#[derive(Component)]
#[relationship(relationship_target = HasSprite)]
struct IsSpriteOf(Entity);

#[derive(Component)]
#[relationship_target(relationship = IsSpriteOf, linked_spawn)]
struct HasSprite(Entity);

fn spawn_plane_system(
    mut spawn_events: EventReader<plane::SpawnEvent>,
    mut params: ParamSet<(
        (Commands, config::Read<Conf>, Res<AssetServer>),
        separation_ring::SpawnSubsystemParam,
        vector::SpawnSubsystemParam,
    )>,
) {
    for &plane::SpawnEvent(plane_entity) in spawn_events.read() {
        let (mut commands, conf, asset_server) = params.p0();

        commands.entity(plane_entity).insert((
            Transform::IDENTITY,
            Visibility::Visible,
            ColorTheme { body: Color::WHITE, ring: Color::WHITE, vector: Color::WHITE },
        ));

        commands.spawn((
            IsSpriteOf(plane_entity),
            ChildOf(plane_entity),
            Zorder::ObjectSprite.local_translation(),
            Sprite::from_image(asset_server.load(conf.plane_sprite.path())),
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

        separation_ring::spawn_subsystem(plane_entity, &mut params.p1());
        vector::spawn_subsystem(plane_entity, &mut params.p2());
    }
}

/// Extension component on an object to indicate the colors of its viewable entities.
#[derive(Component)]
pub struct ColorTheme {
    pub body:   Color,
    pub ring:   Color,
    pub vector: Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet, strum::EnumIter)]
pub enum SetColorThemeSystemSet {
    BaseConfig,
    Alert,
    UserInteract,
}

fn maintain_plane_system(
    conf: config::Read<Conf>,
    mut object_query: Query<(
        &HasSprite,
        &Object,
        &object::Rotation,
        label::ObjectData,
        &mut Transform,
        &ColorTheme,
    )>,
    mut sprite_query: Query<
        (&mut Sprite, &mut Transform),
        (query::With<IsSpriteOf>, query::Without<Object>),
    >,
    mut label_writer: label::Writer,
) {
    object_query.iter_mut().for_each(
        |(
            &HasSprite(sprite_entity),
            object,
            object_rot,
            label_data,
            mut object_tf,
            color_theme,
        )| {
            object_tf.translation = Zorder::base_translation(object.position);

            if let Ok((mut sprite, mut sprite_tf)) = sprite_query.get_mut(sprite_entity) {
                sprite.color = color_theme.body;
                sprite_tf.rotation = object_rot.0;
            }

            label_data.write_label(&conf, &mut label_writer);
        },
    );
}

fn handle_config_change_system(
    mut conf: config::Read<Conf>,
    mut queries: ParamSet<(
        Query<(&mut Sprite, &mut billboard::MaintainScale), With<IsSpriteOf>>,
        Query<(&mut billboard::MaintainScale, &mut billboard::Label, &mut Anchor), With<IsLabelOf>>,
    )>,
    asset_server: Res<AssetServer>,
) {
    let Some(conf) = conf.consume_change() else {
        return;
    };

    for (mut sprite, mut scale) in queries.p0() {
        *sprite = Sprite::from_image(asset_server.load(conf.plane_sprite.path()));
        scale.size = conf.plane_sprite_size;
    }

    for (mut scale, mut label, mut anchor) in queries.p1() {
        scale.size = conf.label_size;
        label.distance = conf.label_distance;
        *anchor = conf.label_anchor;
    }
}

#[derive(Resource, Config)]
#[config(id = "2d/object", name = "Objects (2D)")]
struct Conf {
    /// Sprite for planes.
    plane_sprite:              SpriteType,
    /// Size of plane sprites.
    #[config(min = 0., max = 5.)]
    plane_sprite_size:         f32,
    /// Size of object labels.
    #[config(min = 0., max = 3.)]
    label_size:                f32,
    /// Distance of object labels from the object center, in screen coordinates.
    #[config(min = 0., max = 300.)]
    label_distance:            f32,
    /// Direction of the object relative to the label.
    label_anchor:              Anchor,
    /// World radius of the separation ring.
    #[config(min = Distance::ZERO, max = Distance::from_nm(10.), precision = Distance::from_nm(0.5))]
    separation_ring_radius:    Distance<f32>,
    /// Thickness of the separation ring in screen coordinates.
    #[config(min = 0., max = 10.)]
    separation_ring_thickness: f32,

    #[config(min = Duration::ZERO, max = Duration::from_secs(300))]
    vector_lookahead_time: Duration,
    /// Thickness of the vector line in screen coordinates.
    #[config(min = 0., max = 10.)]
    vector_thickness:      f32,

    /// Maximum number of track points for unfocused objects.
    #[config(min = 0, max = 100)]
    track_normal_max_points:   u32,
    /// Size of track points.
    #[config(min = 0., max = 3.)]
    track_point_size:          f32,
    /// Color of track points at base altitude.
    track_point_base_color:    Color,
    /// Base altitude for track point coloring.
    track_point_base_altitude: Position<f32>,
    /// Color of track points at top altitude.
    track_point_top_color:     Color,
    /// Top altitude for track point coloring.
    track_point_top_altitude:  Position<f32>,

    /// Thickness of planned track preview line.
    preview_line_thickness:         f32,
    /// Color of planned track preview line.
    preview_line_color_normal:      Color,
    /// Color of planned track preview line when setting heading.
    preview_line_color_set_heading: Color,
    /// Color of available route presets from the current target waypoint.
    preview_line_color_preset:      Color,

    /// Object color will be based on this scheme.
    base_color_scheme:   base_color::Scheme,
    /// Object separation ring color will be based on this scheme.
    ring_color_scheme:   base_color::Scheme,
    /// Object ground speed vector color will be based on this scheme.
    vector_color_scheme: base_color::Scheme,
}

impl Default for Conf {
    fn default() -> Self {
        Self {
            plane_sprite:                   SpriteType::Plane,
            plane_sprite_size:              1.0,
            label_size:                     0.5,
            label_distance:                 50.,
            label_anchor:                   Anchor::BottomLeft,
            separation_ring_radius:         Distance::from_nm(1.5),
            separation_ring_thickness:      0.5,
            vector_lookahead_time:          Duration::from_secs(60),
            vector_thickness:               0.5,
            track_normal_max_points:        5,
            track_point_size:               1.0,
            track_point_base_color:         Color::srgb(0.8, 0.4, 0.6),
            track_point_base_altitude:      Position::SEA_LEVEL,
            track_point_top_color:          Color::srgb(0.4, 0.8, 0.6),
            track_point_top_altitude:       TROPOPAUSE_ALTITUDE,
            preview_line_color_normal:      Color::srgb(0.9, 0.7, 0.8),
            preview_line_color_set_heading: Color::srgb(0.9, 0.9, 0.6),
            preview_line_color_preset:      Color::srgb(0.5, 0.6, 0.8),
            preview_line_thickness:         1.,
            base_color_scheme:              base_color::Scheme::default(),
            ring_color_scheme:              base_color::Scheme::default(),
            vector_color_scheme:            base_color::Scheme::default(),
        }
    }
}

#[derive(strum::EnumIter, Clone, Copy, PartialEq, Eq, strum::Display, Serialize, Deserialize)]
enum SpriteType {
    #[strum(message = "Plane")]
    Plane,
}

impl config::EnumField for SpriteType {}

impl SpriteType {
    fn path(self) -> &'static str {
        match self {
            Self::Plane => "sprites/plane.png",
        }
    }
}
