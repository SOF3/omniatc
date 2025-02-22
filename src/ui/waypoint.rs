use bevy::app::{self, App, Plugin};
use bevy::asset::AssetServer;
use bevy::prelude::{
    BuildChildren, ChildBuild, Commands, Component, EventReader, IntoSystemConfigs, Query, Res,
    Resource, Transform, Visibility,
};
use bevy::sprite::{Anchor, Sprite};
use bevy::text::Text2d;
use omniatc_core::level::waypoint::{self, DisplayType, Waypoint};
use omniatc_core::units::Position;

use super::{billboard, SystemSets, Zorder};

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

        commands.entity(entity).insert((Transform::IDENTITY, Visibility::Visible)).with_children(
            |b| {
                if let Some(sprite_path) = display_type_sprite(waypoint.display_type) {
                    b.spawn((
                        Sprite::from_image(asset_server.load(sprite_path)),
                        Transform::from_translation(
                            waypoint.position.get().with_z(Zorder::Waypoint.into_z()),
                        ),
                        billboard::MaintainScale { size: config.icon_size },
                        billboard::MaintainRotation,
                        IconViewable,
                    ));
                }

                if let Some(label_config) =
                    display_type_label_config(&config, waypoint.display_type)
                {
                    b.spawn((
                        Text2d::new(waypoint.name.clone()),
                        Transform::from_translation(
                            waypoint.position.get().with_z(Zorder::WaypointLabel.into_z()),
                        ),
                        billboard::MaintainScale { size: label_config.size },
                        billboard::MaintainRotation,
                        billboard::Label {
                            offset:   waypoint.position.horizontal() - Position::ORIGIN,
                            distance: label_config.distance,
                        },
                        Anchor::BottomCenter,
                        LabelViewable,
                    ));
                }
            },
        );
    }
}

fn display_type_sprite(display_type: DisplayType) -> Option<&'static str> {
    match display_type {
        DisplayType::Vor => Some("sprites/vor.png"),
        DisplayType::Dme => Some("sprites/dme.png"),
        DisplayType::VorDme => Some("sprites/vor-dme.png"),
        DisplayType::Waypoint => Some("sprites/waypoint.png"),
        DisplayType::None | DisplayType::Runway => None,
    }
}

fn display_type_label_config(config: &Config, display_type: DisplayType) -> Option<&LabelConfig> {
    match display_type {
        DisplayType::Waypoint => Some(&config.waypoint_label),
        DisplayType::Vor => Some(&config.vor_label),
        DisplayType::Dme => Some(&config.dme_label),
        DisplayType::VorDme => Some(&config.vor_dme_label),
        DisplayType::Runway => Some(&config.runway_label),
        DisplayType::None => None,
    }
}

#[derive(Component)]
pub struct IconViewable;

#[derive(Component)]
struct LabelViewable;

#[derive(Resource)]
pub struct Config {
    pub icon_size:      f32,
    pub waypoint_label: LabelConfig,
    pub vor_label:      LabelConfig,
    pub dme_label:      LabelConfig,
    pub vor_dme_label:  LabelConfig,
    pub runway_label:   LabelConfig,
}

pub struct LabelConfig {
    size:     f32,
    distance: f32,
    anchor:   Anchor,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            icon_size:      0.7,
            waypoint_label: LabelConfig {
                size:     0.4,
                distance: 30.,
                anchor:   Anchor::BottomCenter,
            },
            vor_label:      LabelConfig {
                size:     0.4,
                distance: 30.,
                anchor:   Anchor::BottomCenter,
            },
            dme_label:      LabelConfig {
                size:     0.4,
                distance: 30.,
                anchor:   Anchor::BottomCenter,
            },
            vor_dme_label:  LabelConfig {
                size:     0.4,
                distance: 30.,
                anchor:   Anchor::BottomCenter,
            },
            runway_label:   LabelConfig {
                size:     0.4,
                distance: 30.,
                anchor:   Anchor::BottomCenter,
            },
        }
    }
}
