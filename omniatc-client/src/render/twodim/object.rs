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
use bevy_mod_config::{self, AppExt as _, Config, ReadConfig, ReadConfigChange};
use math::{Length, Position, TROPOPAUSE_ALTITUDE};
use omniatc::level::object::{self, Object};
use omniatc::level::plane;
use omniatc::util::EnumScheduleConfig;
use serde::{Deserialize, Serialize};

use super::Zorder;
use crate::util::{billboard, AnchorConf};
use crate::{render, ConfigManager};

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
        app.init_config::<ConfigManager, Conf>("2d:object");
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
        (Commands, ReadConfig<Conf>, Res<AssetServer>),
        separation_ring::SpawnSubsystemParam,
        vector::SpawnSubsystemParam,
    )>,
) {
    for &plane::SpawnEvent(plane_entity) in spawn_events.read() {
        let (mut commands, conf, asset_server) = params.p0();
        let conf = conf.read();

        commands.entity(plane_entity).insert((
            Transform::IDENTITY,
            Visibility::Visible,
            ColorTheme { body: Color::WHITE, ring: Color::WHITE, vector: Color::WHITE },
        ));

        commands.spawn((
            IsSpriteOf(plane_entity),
            ChildOf(plane_entity),
            Zorder::ObjectSprite.local_translation(),
            Sprite::from_image(asset_server.load(conf.plane.sprite.path())),
            billboard::MaintainScale { size: conf.plane.sprite_size },
        ));
        commands.spawn((
            IsLabelOf(plane_entity),
            ChildOf(plane_entity),
            Zorder::ObjectLabel.local_translation(),
            billboard::MaintainScale { size: conf.plane.label_size },
            billboard::MaintainRotation,
            billboard::Label { offset: Length::ZERO, distance: conf.plane.label_distance },
            Text2d::new(""),
            conf.plane.label_anchor,
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
    conf: ReadConfig<Conf>,
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
    let conf = conf.read();

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

            label_data.write_label(&conf.plane, &mut label_writer);
        },
    );
}

fn handle_config_change_system(
    mut conf: ReadConfigChange<Conf>,
    mut queries: ParamSet<(
        Query<(&mut Sprite, &mut billboard::MaintainScale), With<IsSpriteOf>>,
        Query<(&mut billboard::MaintainScale, &mut billboard::Label, &mut Anchor), With<IsLabelOf>>,
    )>,
    asset_server: Res<AssetServer>,
) {
    if !conf.consume_change() {
        return;
    }
    let conf = conf.read();

    for (mut sprite, mut scale) in queries.p0() {
        *sprite = Sprite::from_image(asset_server.load(conf.plane.sprite.path()));
        scale.size = conf.plane.sprite_size;
    }

    for (mut scale, mut label, mut anchor) in queries.p1() {
        scale.size = conf.plane.label_size;
        label.distance = conf.plane.label_distance;
        *anchor = conf.plane.label_anchor;
    }
}

#[derive(Config)]
#[config(expose(read))]
struct Conf {
    plane:           PlaneConf,
    separation_ring: SeparationRingConf,
    vector:          VectorLineConf,
    track:           TrackConf,
    preview_line:    PreviewLineConf,
}

#[derive(Config)]
#[config(expose(read))]
struct PlaneConf {
    /// Sprite for planes.
    sprite:         SpriteType,
    /// Size of plane sprites.
    #[config(default = 1.0, min = 0.0, max = 5.0)]
    sprite_size:    f32,
    /// Size of object labels.
    #[config(default = 0.5, min = 0.0, max = 3.0)]
    label_size:     f32,
    /// Distance of object labels from the object center, in screen coordinates.
    #[config(default = 50.0, min = 0., max = 300.)]
    label_distance: f32,
    /// Direction of the object relative to the label.
    #[config(default = Anchor::BottomLeft)]
    label_anchor:   AnchorConf,
    /// Object color will be based on this scheme.
    color_scheme:   base_color::Scheme,
}

#[derive(Config)]
#[config(expose(read))]
struct SeparationRingConf {
    /// World radius of the separation ring.
    #[config(
        default = Length::from_nm(1.5),
        min = Length::ZERO,
        max = Length::from_nm(10.0),
        precision = Some(Length::from_nm(0.5)),
    )]
    radius:       Length<f32>,
    /// Thickness of the separation ring in screen coordinates.
    #[config(default = 0.5, min = 0.0, max = 10.0)]
    thickness:    f32,
    /// Object separation ring color will be based on this scheme.
    color_scheme: base_color::Scheme,
}

#[derive(Config)]
#[config(expose(read))]
struct VectorLineConf {
    #[config(default = Duration::from_secs(60), min = Duration::ZERO, max = Duration::from_secs(300))]
    lookahead_time: Duration,
    /// Thickness of the vector line in screen coordinates.
    #[config(default = 0.5, min = 0., max = 10.)]
    thickness:      f32,
    /// Object ground speed vector color will be based on this scheme.
    color_scheme:   base_color::Scheme,
}

#[derive(Config)]
#[config(expose(read))]
struct TrackConf {
    /// Maximum number of track points for unfocused objects.
    #[config(default = 5, min = 0, max = 100)]
    normal_max_points:   u32,
    /// Size of track points.
    #[config(default = 1.0, min = 0.0, max = 3.0)]
    point_size:          f32,
    /// Color of track points at base altitude.
    #[config(default = Color::srgb(0.8, 0.4, 0.6))]
    point_base_color:    Color,
    /// Base altitude for track point coloring.
    #[config(default = Position::SEA_LEVEL)]
    point_base_altitude: Position<f32>,
    /// Color of track points at top altitude.
    #[config(default = Color::srgb(0.4, 0.8, 0.6))]
    point_top_color:     Color,
    /// Top altitude for track point coloring.
    #[config(default = TROPOPAUSE_ALTITUDE)]
    point_top_altitude:  Position<f32>,
}

#[derive(Config)]
#[config(expose(read))]
struct PreviewLineConf {
    /// Thickness of planned track preview line.
    #[config(default = 1.0)]
    thickness:         f32,
    /// Color of planned track preview line.
    #[config(default = Color::srgb(0.9, 0.7, 0.8))]
    color_normal:      Color,
    /// Color of planned track preview line when setting heading.
    #[config(default = Color::srgb(0.9, 0.9, 0.6))]
    color_set_heading: Color,
    /// Color of available route presets from the current target waypoint.
    #[config(default = Color::srgb(0.5, 0.6, 0.8))]
    color_preset:      Color,
}

#[derive(
    strum::EnumIter, Clone, Copy, PartialEq, Eq, strum::Display, Serialize, Deserialize, Config,
)]
#[config(expose(read))]
enum SpriteType {
    #[strum(message = "Plane")]
    Plane,
}

impl SpriteTypeRead {
    fn path(&self) -> &'static str {
        match self {
            Self::Plane => "sprites/plane.png",
        }
    }
}
