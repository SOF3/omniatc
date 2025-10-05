use bevy::app::{self, App, Plugin};
use bevy::asset::AssetServer;
use bevy::camera::visibility::Visibility;
use bevy::ecs::change_detection::{DetectChangesMut, Mut};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::message::MessageReader;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::sprite::{Anchor, Sprite, Text2d};
use bevy::transform::components::Transform;
use bevy_mod_config::{AppExt, Config, ReadConfig};
use math::Length;
use omniatc::level::waypoint::{self, Waypoint};

use super::Zorder;
use crate::util::{AnchorConf, billboard};
use crate::{ConfigManager, render};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_config::<ConfigManager, Conf>("2d:waypoint");
        app.add_systems(app::Update, spawn_system.in_set(render::SystemSets::Spawn));
        app.add_systems(app::Update, move_system.in_set(render::SystemSets::Update));
    }
}

#[derive(Component)]
#[relationship(relationship_target = HasSprite)]
struct IsSpriteOf(Entity);

#[derive(Component)]
#[relationship_target(relationship = IsSpriteOf, linked_spawn)]
struct HasSprite(Entity);

#[derive(Component)]
#[relationship(relationship_target = HasLabel)]
struct IsLabelOf(Entity);

#[derive(Component)]
#[relationship_target(relationship = IsLabelOf, linked_spawn)]
struct HasLabel(Entity);

fn spawn_system(
    mut commands: Commands,
    mut spawns: MessageReader<waypoint::SpawnMessage>,
    conf: ReadConfig<Conf>,
    asset_server: Res<AssetServer>,
    waypoint_query: Query<&Waypoint>,
) {
    let conf = conf.read();

    for &waypoint::SpawnMessage(waypoint_entity) in spawns.read() {
        let waypoint = waypoint_query.get(waypoint_entity).expect("waypoint was just spawned");

        commands.entity(waypoint_entity).insert((Transform::IDENTITY, Visibility::Visible));

        if let Some(sprite_path) = Conf::sprite_path(waypoint.display_type) {
            commands.spawn((
                IsSpriteOf(waypoint_entity),
                ChildOf(waypoint_entity),
                Zorder::WaypointSprite.local_translation(),
                Sprite::from_image(asset_server.load(sprite_path)),
                billboard::MaintainScale { size: conf.sprite_size },
                billboard::MaintainRotation,
            ));
        }

        if waypoint.display_type.should_display_label() {
            commands.spawn((
                IsLabelOf(waypoint_entity),
                ChildOf(waypoint_entity),
                Zorder::WaypointLabel.local_translation(),
                billboard::MaintainScale { size: conf.label_size },
                billboard::MaintainRotation,
                billboard::Label { offset: Length::ZERO, distance: conf.label_distance },
                Text2d::new(waypoint.name.as_str()),
                conf.label_anchor,
            ));
        }
    }
}

fn move_system(mut waypoint_query: Query<(&Waypoint, &mut Transform)>) {
    waypoint_query.iter_mut().for_each(|(waypoint, tf)| {
        Mut::map_unchanged(tf, |tf| &mut tf.translation)
            .set_if_neq(Zorder::base_translation(waypoint.position));
    });
}

#[derive(Config)]
struct Conf {
    /// Size of waypoint sprites.
    #[config(default = 0.7, min = 0.0, max = 5.0)]
    sprite_size:    f32,
    #[config(default = 0.6, min = 0.0, max = 3.0)]
    label_size:     f32,
    #[config(default = 30.0, min = 0.0, max = 100.0)]
    label_distance: f32,
    #[config(default = Anchor::BOTTOM_CENTER)]
    label_anchor:   AnchorConf,
}

impl Conf {
    fn sprite_path(display_type: waypoint::DisplayType) -> Option<&'static str> {
        match display_type {
            waypoint::DisplayType::Vor => Some("sprites/vor.png"),
            waypoint::DisplayType::Dme => Some("sprites/dme.png"),
            waypoint::DisplayType::VorDme => Some("sprites/vor-dme.png"),
            waypoint::DisplayType::Waypoint => Some("sprites/waypoint.png"),
            waypoint::DisplayType::None | waypoint::DisplayType::Runway => None,
        }
    }
}
