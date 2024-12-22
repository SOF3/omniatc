use bevy::app::{self, App, Plugin};
use bevy::asset::AssetServer;
use bevy::math::Vec3;
use bevy::prelude::{
    BuildChildren, ChildBuild, Commands, Component, EventReader,
    IntoSystemConfigs, Query, Res, Resource, Transform, Visibility,
};
use bevy::sprite::{Anchor, Sprite};
use bevy::text::Text2d;

use super::{billboard, SystemSets, Zorder};
use crate::level::waypoint::{self, DisplayType, Waypoint};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<Config>();

        app.add_systems(
            app::Update,
            spawn_waypoint_viewable_system.in_set(SystemSets::RenderSpawn),
        );
    }
}

fn spawn_waypoint_viewable_system(
    mut commands: Commands,
    mut events: EventReader<waypoint::SpawnEvent>,
    config: Res<Config>,
    asset_server: Res<AssetServer>,
    waypoint_query: Query<&Waypoint>,
) {
    for &waypoint::SpawnEvent(entity) in events.read() {
        let waypoint = waypoint_query.get(entity).expect("waypoint was just spawned");
        if waypoint.display_type == DisplayType::None {
            return;
        }

        commands
            .entity(entity)
            .insert((
                Transform::from_translation(waypoint.position.with_z(0.)),
                Visibility::Visible,
            ))
            .with_children(|b| {
                b.spawn((
                    Sprite::from_image(
                        asset_server.load(sprite_path_for_display_type(waypoint.display_type)),
                    ),
                    Transform::from_translation(Vec3::ZERO.with_z(Zorder::Waypoint.to_z())),
                    billboard::MaintainScale { size: config.icon_size },
                    billboard::MaintainRotation,
                    WaypointViewable,
                ));
                b.spawn((
                    Text2d::new(waypoint.name.clone()),
                    Transform::from_translation(Vec3::ZERO.with_z(Zorder::WaypointLabel.to_z())),
                    billboard::MaintainScale { size: config.label_size },
                    billboard::MaintainRotation,
                    billboard::Label { distance: config.label_distance },
                    Anchor::BottomCenter,
                    LabelViewable,
                ));
            });
    }
}

fn sprite_path_for_display_type(dt: DisplayType) -> &'static str {
    match dt {
        DisplayType::Vor => "sprites/vor.png",
        DisplayType::Waypoint => "sprites/waypoint.png",
        DisplayType::None => unreachable!(),
    }
}

#[derive(Component)]
pub struct WaypointViewable;

#[derive(Component)]
struct LabelViewable;

#[derive(Resource)]
pub struct Config {
    pub icon_size:      f32,
    pub label_size:     f32,
    pub label_distance: f32,
}

impl Default for Config {
    fn default() -> Self { Self { icon_size: 0.7, label_size: 0.4, label_distance: 30. } }
}
