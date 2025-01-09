//! Displays runway.
//!
//! Spawns extra child entities for the runway entity:
//! - A runway viewable entity for the runway strip.
//! - A localizer viewable entity for the localizer line.
//! - A glide point owner entity, which owns the glide point entities.

use bevy::app::{self, App, Plugin};
use bevy::asset::{Assets, Handle};
use bevy::color::Color;
use bevy::math::Vec3;
use bevy::prelude::{
    Annulus, BuildChildren, Camera2d, Children, Commands, Component, DespawnRecursiveExt,
    DetectChangesMut, Entity, EventReader, GlobalTransform, IntoSystemConfigs, Mesh, Mesh2d, Mut,
    Or, Parent, Query, Rectangle, Res, ResMut, Resource, Single, Transform, Visibility, With,
};
use bevy::sprite::{ColorMaterial, MeshMaterial2d};

use super::{billboard, SystemSets, Zorder};
use crate::level::runway::{self, Runway};
use crate::level::waypoint::{self, Navaid, Waypoint};
use crate::math::SEA_ALTITUDE;
use crate::units::Distance;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<Config>();
        app.init_resource::<Meshes>();
        app.add_systems(
            app::Startup,
            |mut store: ResMut<Meshes>, mut meshes: ResMut<Assets<Mesh>>| {
                store.square = Some(meshes.add(Rectangle::new(1., 1.)));
                store.annulus = Some(meshes.add(Annulus::new(0.9, 1.)));
            },
        );
        app.add_systems(app::Update, spawn_viewable_system.in_set(SystemSets::RenderSpawn));
        app.add_systems(
            app::Update,
            maintain_localizer_viewable_system.in_set(SystemSets::RenderMove),
        );
        app.add_systems(
            app::Update,
            maintain_glide_point_system
                .after(maintain_localizer_viewable_system)
                .in_set(SystemSets::RenderMove),
        );
    }
}

#[derive(Resource, Default)]
struct Meshes {
    square:  Option<Handle<Mesh>>,
    annulus: Option<Handle<Mesh>>,
}

/// Marks the runway strip viewable entity.
#[derive(Component)]
struct StripViewable;

/// Marks the localizer viewable entity.
#[derive(Component)]
struct LocalizerViewable;

/// Horizontal length of the displayed localizer.
#[derive(Component)]
struct LocalizerDisplayLength(Distance<f32>);

/// Marks the entity that owns the glide point entities.
#[derive(Component)]
struct GlidePointOwner;

/// Extension component on the runway entity that references the glide point owner entity.
#[derive(Component)]
struct GlidePointOwnerRef(Entity);

/// Marks glide point viewabl entities.
#[derive(Component)]
struct GlidePoint;

fn spawn_viewable_system(
    mut commands: Commands,
    config: Res<Config>,
    mut events: EventReader<runway::SpawnEvent>,
    meshes: Res<Meshes>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    runway_query: Query<&Runway>,
) {
    for &runway::SpawnEvent(runway_entity) in events.read() {
        let runway = runway_query.get(runway_entity).expect("runway just spawned");

        let glide_point_owner = commands
            .spawn((
                GlidePointOwner,
                bevy::core::Name::new("GlidePointOwner"),
                Transform::IDENTITY,
                Visibility::Visible,
            ))
            .id();

        let mut runway_ref = commands.entity(runway_entity);
        runway_ref.with_child((
            LocalizerViewable,
            bevy::core::Name::new("LocalizerViewable"),
            Mesh2d(meshes.square.clone().expect("initialized at startup")),
            MeshMaterial2d(
                materials
                    .add(ColorMaterial { color: config.localizer_color, ..Default::default() }),
            ),
        ));
        runway_ref.add_child(glide_point_owner);

        runway_ref.with_child((
            StripViewable,
            bevy::core::Name::new("StripViewable"),
            Mesh2d(meshes.square.clone().expect("initialized at startup")),
            MeshMaterial2d(
                materials.add(ColorMaterial { color: config.strip_color, ..Default::default() }),
            ),
            // x = width, y = length
            Transform {
                translation: Zorder::RunwayStrip.pos2_to_translation(
                    runway.display_start.horizontal().lerp(runway.display_end.horizontal(), 0.5),
                ),
                rotation:    (runway.display_end - runway.display_start)
                    .horizontal()
                    .heading()
                    .into_rotation_quat(),
                scale:       Vec3::new(
                    runway.display_width.0,
                    runway.display_start.distance_exact(runway.display_end).0,
                    1.,
                ),
            },
        ));

        runway_ref
            .insert((LocalizerDisplayLength(Distance(0.)), GlidePointOwnerRef(glide_point_owner)));
    }
}

fn maintain_localizer_viewable_system(
    mut viewable_query: Query<(Entity, &mut Transform, &Parent), With<LocalizerViewable>>,
    mut runway_query: Query<(&Runway, &Waypoint, &mut LocalizerDisplayLength, &Children)>,
    navaid_query: Query<&Navaid, Or<(With<waypoint::Visual>, With<waypoint::HasCriticalRegion>)>>,
    config: Res<Config>,
    camera: Single<&GlobalTransform, With<Camera2d>>,
) {
    viewable_query.iter_mut().for_each(|(viewable_entity, mut tf, runway_ref)| {
        let Ok((runway, waypoint, mut localizer_length_store, children)) =
            runway_query.get_mut(runway_ref.get())
        else {
            bevy::log::error!(
                "parent {runway_ref:?} of runway viewable {viewable_entity:?} must be a runway \
                 entity"
            );
            return;
        };

        let mut localizer_length = None::<Distance<f32>>;
        for &child in children {
            let Ok(navaid) = navaid_query.get(child) else { continue };
            let max = localizer_length.get_or_insert_default();
            *max = max.max(navaid.max_dist_horizontal);
        }

        let localizer_length = localizer_length.unwrap_or_else(|| {
            bevy::log::warn!("no visual navaid children for runway {runway_ref:?}");
            Distance(1.)
        });
        localizer_length_store.0 = localizer_length;

        // Orientation: x = line width, y = localizer length
        tf.translation = Zorder::Localizer.pos2_to_translation(
            waypoint.position.horizontal()
                - runway.usable_length.with_magnitude(localizer_length) * 0.5,
        );
        tf.rotation = runway.usable_length.heading().into_rotation_quat();
        tf.scale = Vec3::new(config.localizer_width * camera.scale().y, localizer_length.0, 1.);
    });
}

fn maintain_glide_point_system(
    mut commands: Commands,
    runway_query: Query<(&Runway, &Waypoint, &LocalizerDisplayLength, &GlidePointOwnerRef)>,
    children_query: Query<&Children>,
    mut glide_point_query: Query<&mut Transform, With<GlidePoint>>,
    config: Res<Config>,
    meshes: Res<Meshes>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    runway_query.iter().for_each(
        |(
            runway,
            waypoint,
            &LocalizerDisplayLength(localizer_length),
            &GlidePointOwnerRef(owner_ref),
        )| {
            #[allow(clippy::cast_possible_truncation)] // f32 -> i32 for a reasonably small value
            let first_mult = (waypoint.position.vertical().amsl() / config.glide_point_density)
                .ceil() as i32
                + 1;
            #[allow(clippy::cast_possible_truncation)] // f32 -> i32 for a reasonably small value
            let last_mult = ((waypoint.position.vertical()
                + localizer_length * runway.glide_angle.tan())
            .amsl()
                / config.glide_point_density)
                .floor() as i32;
            let point_count = usize::try_from(last_mult - first_mult + 1).unwrap_or(0);

            let children =
                children_query.get(owner_ref).map(|children| &children[..]).unwrap_or_default();

            for &to_delete in children.get(point_count..).unwrap_or_default() {
                if let Some(entity) = commands.get_entity(to_delete) {
                    entity.despawn_recursive();
                }
            }

            for point in 0..point_count {
                #[allow(
                    clippy::cast_precision_loss,
                    clippy::cast_possible_truncation,
                    clippy::cast_possible_wrap
                )]
                let altitude =
                    SEA_ALTITUDE + config.glide_point_density * (first_mult + point as i32) as f32;
                let distance = (altitude - waypoint.position.vertical()) / runway.glide_angle.tan();
                let pos = Zorder::LocalizerGlidePoint.pos2_to_translation(
                    waypoint.position.horizontal() - runway.usable_length.with_magnitude(distance),
                );

                if let Some(&point_entity) = children.get(point) {
                    let Ok(point_tf) = glide_point_query.get_mut(point_entity) else {
                        bevy::log::error!(
                            "glide point owner {owner_ref:?} must not have non-GlidePoint child \
                             {point_entity:?}"
                        );
                        continue;
                    };
                    Mut::map_unchanged(point_tf, |tf| &mut tf.translation).set_if_neq(pos);
                } else {
                    commands.entity(owner_ref).with_child((
                        GlidePoint,
                        billboard::MaintainScale { size: config.glide_point_size },
                        Transform::from_translation(pos),
                        Mesh2d(meshes.annulus.clone().expect("initialized during startup")),
                        MeshMaterial2d(materials.add(ColorMaterial {
                            color: config.localizer_color,
                            ..Default::default()
                        })),
                    ));
                }
            }
        },
    );
}

#[derive(Resource)]
pub struct Config {
    /// Color of the runway strip.
    pub strip_color:         Color,
    /// Color of the localizer.
    pub localizer_color:     Color,
    /// Screen width of the localizer.
    pub localizer_width:     f32,
    /// A glide point is displayed on the displayed localizer line
    /// if a plane on the glidepath should be at a height
    /// that is an integer multiple of `glidepath_density`.
    pub glide_point_density: Distance<f32>,
    /// Display size of a glide point.
    pub glide_point_size:    f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            strip_color:         Color::srgb(0.4, 0.9, 0.2),
            localizer_color:     Color::WHITE,
            localizer_width:     0.3,
            glide_point_density: Distance::from_feet(1000.),
            glide_point_size:    3.0,
        }
    }
}
