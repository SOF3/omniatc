use std::mem;

use bevy::app::{self, App, Plugin};
use bevy::asset::Assets;
use bevy::color::{Alpha, Color};
use bevy::core_pipeline::core_2d::Camera2d;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EventReader;
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Local, Query, Res, ResMut, Single, SystemParam};
use bevy::math::Vec2;
use bevy::sprite::{AlphaMode2d, ColorMaterial, MeshMaterial2d};
use bevy::transform::components::GlobalTransform;
use itertools::Itertools;
use math::Distance;
use omniatc::level::wake;
use omniatc::try_log;
use omniatc_macros::Config;
use smallvec::SmallVec;

use super::Zorder;
use crate::config::{self, AppExt};
use crate::render;
use crate::util::shapes;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_config::<Conf>();

        app.add_systems(app::Update, spawn_system.in_set(render::SystemSets::Spawn));
        app.add_systems(app::Update, update_system.in_set(render::SystemSets::Update));
    }
}

#[derive(Component)]
#[relationship(relationship_target = HasSprite)]
struct IsSpriteOf(Entity);

#[derive(Component)]
#[relationship_target(relationship = IsSpriteOf, linked_spawn)]
struct HasSprite(SmallVec<[Entity; 4]>);

fn spawn_system(
    mut events: EventReader<wake::SpawnEvent>,
    conf: config::Read<Conf>,
    mut last_display: Local<bool>,
    mut params: SpawnVortexParams,
    sprite_query: Query<Entity, With<IsSpriteOf>>,
    vortex_query: Query<(Entity, &'static wake::Vortex), Without<HasSprite>>,
) {
    match (mem::replace(&mut *last_display, conf.display), conf.display) {
        (false, true) => {
            for (vortex_entity, vortex) in vortex_query {
                spawn_vortex(vortex_entity, vortex, &mut params);
            }
        }
        (true, false) => {
            for sprite in sprite_query {
                params.commands.entity(sprite).despawn();
            }
        }
        (true, true) => {
            for &wake::SpawnEvent(vortex_entity) in events.read() {
                let (_, vortex) = vortex_query.get(vortex_entity).expect("vortex was just spawned");
                spawn_vortex(vortex_entity, vortex, &mut params);
            }
        }
        (false, false) => {}
    }
}

#[derive(SystemParam)]
struct SpawnVortexParams<'w, 's> {
    commands:  Commands<'w, 's>,
    meshes:    Res<'w, shapes::Meshes>,
    conf:      config::Read<'w, 's, Conf>,
    materials: ResMut<'w, Assets<ColorMaterial>>,
    camera:    Single<'w, &'static GlobalTransform, With<Camera2d>>,
}

fn spawn_vortex(vortex_entity: Entity, vortex: &wake::Vortex, params: &mut SpawnVortexParams) {
    let side_iter = [(-1., -1.), (-1., 1.), (1., 1.), (1., -1.)]
        .into_iter()
        .map(|(dx, dy)| {
            vortex.position.horizontal() + Distance::from_nm(0.375) * Vec2 { x: dx, y: dy }
        })
        .circular_tuple_windows();
    for (from, to) in side_iter {
        params.commands.spawn((
            IsSpriteOf(vortex_entity),
            params.meshes.line_from_to(
                params.conf.square_thickness,
                Zorder::WakeOverlay,
                from,
                to,
                &params.camera,
            ),
            MeshMaterial2d(params.materials.add(ColorMaterial {
                color: params.conf.color_for_intensity(vortex.intensity),
                alpha_mode: AlphaMode2d::Blend,
                ..Default::default()
            })),
        ));
    }
}

fn update_system(
    conf: config::Read<Conf>,
    vortex_query: Query<&wake::Vortex>,
    sprite_query: Query<(&IsSpriteOf, &MeshMaterial2d<ColorMaterial>)>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    for (&IsSpriteOf(vortex_entity), MeshMaterial2d(handle)) in sprite_query {
        let vortex = try_log!(vortex_query.get(vortex_entity), expect "parent vortex of sprite must exist" or continue);
        let material = try_log!(materials.get_mut(handle), expect "material referenced by strong handle must exist" or continue);
        material.color = conf.color_for_intensity(vortex.intensity);
    }
}

#[derive(Config, Resource)]
#[config(id = "wake", name = "Wake")]
struct Conf {
    /// Display wake configuration overlay.
    display:                 bool,
    /// Thickness of vortex overlay squares.
    square_thickness:        f32,
    /// Color of vortex overlay squares at full opacity.
    square_color:            Color,
    /// Intensity of vortex overlay squares to reach full opacity.
    square_opaque_intensity: f32,
}

impl Default for Conf {
    fn default() -> Self {
        Self {
            display:                 false,
            square_thickness:        0.5,
            square_color:            Color::srgba(0.3, 0.2, 0.9, 1.0),
            square_opaque_intensity: 300e3,
        }
    }
}

impl Conf {
    fn color_for_intensity(&self, intensity: wake::Intensity) -> Color {
        #[expect(clippy::cast_precision_loss)] // acceptable precision loss
        let opacity = (intensity.0 as f32) / self.square_opaque_intensity;
        let mut out = self.square_color;
        out.set_alpha(out.alpha() * opacity.clamp(0.0, 1.0));
        out
    }
}
