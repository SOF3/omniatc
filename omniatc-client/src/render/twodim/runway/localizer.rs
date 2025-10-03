use bevy::asset::Assets;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::sprite_render::{ColorMaterial, MeshMaterial2d};
use bevy::transform::components::Transform;
use bevy_mod_config::{self, ReadConfig};
use math::Length;
use omniatc::level::runway::Runway;
use omniatc::{QueryTryLog, try_log_return};

use super::Conf;
use crate::render::twodim::Zorder;
use crate::util::shapes;

#[derive(SystemParam)]
pub struct SpawnParam<'w, 's> {
    commands:  Commands<'w, 's>,
    shapes:    Res<'w, shapes::Meshes>,
    conf:      ReadConfig<'w, 's, Conf>,
    materials: ResMut<'w, Assets<ColorMaterial>>,
}

impl SpawnParam<'_, '_> {
    pub fn spawn(&mut self, runway: Entity) {
        let conf = self.conf.read();

        self.commands.spawn((
            IsLocalizerOf(runway),
            ChildOf(runway),
            self.shapes.line(conf.localizer_thickness, Zorder::Localizer),
            MeshMaterial2d(
                self.materials
                    .add(ColorMaterial { color: conf.localizer_color, ..Default::default() }),
            ),
        ));
    }
}

#[derive(SystemParam)]
pub struct UpdateParam<'w, 's> {
    conf:            ReadConfig<'w, 's, Conf>,
    localizer_query: Query<
        'w,
        's,
        (
            &'static mut Transform,
            &'static MeshMaterial2d<ColorMaterial>,
            &'static mut shapes::MaintainThickness,
        ),
        With<IsLocalizerOf>,
    >,
    materials:       ResMut<'w, Assets<ColorMaterial>>,
}

impl UpdateParam<'_, '_> {
    pub fn update(
        &mut self,
        runway: &Runway,
        &HasLocalizer(entity): &HasLocalizer,
        localizer_length: Length<f32>,
    ) {
        let conf = self.conf.read();

        let Some((mut line_tf, material_handle, mut thickness)) =
            self.localizer_query.log_get_mut(entity)
        else {
            return;
        };

        let material = try_log_return!(self.materials.get_mut(&material_handle.0), expect "asset referenced by strong handle must exist");
        material.color = conf.localizer_color;

        let localizer_length = runway.landing_length.normalize_to_magnitude(-localizer_length);
        shapes::set_square_line_transform_relative(&mut line_tf, Length::ZERO, localizer_length);

        thickness.0 = conf.localizer_thickness;
    }
}

#[derive(Component)]
#[relationship(relationship_target = HasLocalizer)]
pub struct IsLocalizerOf(Entity);

#[derive(Component)]
#[relationship_target(relationship = IsLocalizerOf, linked_spawn)]
pub struct HasLocalizer(Entity);
