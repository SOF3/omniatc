use std::mem;

use bevy::app::{self, App, Plugin};
use bevy::asset::Assets;
use bevy::color::{Alpha, Color};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EventReader;
use bevy::ecs::query::{With, Without};
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Local, Query, Res, ResMut, Single, SystemParam};
use bevy::math::Vec2;
use bevy::sprite::{AlphaMode2d, ColorMaterial, MeshMaterial2d};
use bevy::transform::components::GlobalTransform;
use bevy_mod_config::{self, AppExt, Config, ReadConfig};
use itertools::Itertools;
use math::Length;
use omniatc::level::wake;
use omniatc::{QueryTryLog, try_log};
use smallvec::SmallVec;

use super::Zorder;
use crate::render::twodim::camera;
use crate::util::shapes;
use crate::{ConfigManager, render};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_config::<ConfigManager, Conf>("2d:wake");

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
    conf: ReadConfig<Conf>,
    mut last_display: Local<bool>,
    mut params: SpawnVortexParams,
    sprite_query: Query<Entity, With<IsSpriteOf>>,
    vortex_query: Query<(Entity, &'static wake::Vortex), Without<HasSprite>>,
) {
    let conf = conf.read();

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
    conf:      ReadConfig<'w, 's, Conf>,
    materials: ResMut<'w, Assets<ColorMaterial>>,
    camera:    Single<'w, &'static GlobalTransform, With<camera::Layout>>,
}

fn spawn_vortex(vortex_entity: Entity, vortex: &wake::Vortex, params: &mut SpawnVortexParams) {
    let conf = params.conf.read();

    let side_iter = [(-1., -1.), (-1., 1.), (1., 1.), (1., -1.)]
        .into_iter()
        .map(|(dx, dy)| {
            vortex.position.horizontal() + Length::from_nm(0.375) * Vec2 { x: dx, y: dy }
        })
        .circular_tuple_windows();
    for (from, to) in side_iter {
        params.commands.spawn((
            IsSpriteOf(vortex_entity),
            params.meshes.line_from_to(
                conf.square_thickness,
                Zorder::WakeOverlay,
                from,
                to,
                &params.camera,
            ),
            MeshMaterial2d(params.materials.add(ColorMaterial {
                color: conf.color_for_intensity(vortex.intensity),
                alpha_mode: AlphaMode2d::Blend,
                ..Default::default()
            })),
        ));
    }
}

fn update_system(
    conf: ReadConfig<Conf>,
    vortex_query: Query<&wake::Vortex>,
    sprite_query: Query<(&IsSpriteOf, &MeshMaterial2d<ColorMaterial>)>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let conf = conf.read();

    for (&IsSpriteOf(vortex_entity), MeshMaterial2d(handle)) in sprite_query {
        let Some(vortex) = vortex_query.log_get(vortex_entity) else { continue };
        let material = try_log!(materials.get_mut(handle), expect "material referenced by strong handle must exist" or continue);
        material.color = conf.color_for_intensity(vortex.intensity);
    }
}

#[derive(Config)]
#[config(expose(read))]
struct Conf {
    /// Display wake configuration overlay.
    display:                 bool,
    /// Thickness of vortex overlay squares.
    #[config(default = 0.5)]
    square_thickness:        f32,
    /// Color of vortex overlay squares at full opacity.
    #[config(default = Color::srgb(0.3, 0.2, 0.9))]
    square_color:            Color,
    /// Intensity of vortex overlay squares to reach full opacity.
    #[config(default = 300e3)]
    square_opaque_intensity: f32,
}

impl ConfRead<'_> {
    fn color_for_intensity(&self, intensity: wake::Intensity) -> Color {
        #[expect(clippy::cast_precision_loss)] // acceptable precision loss
        let opacity = (intensity.0 as f32) / self.square_opaque_intensity;
        let mut out = self.square_color;
        out.set_alpha(out.alpha() * opacity.clamp(0.0, 1.0));
        out
    }
}
