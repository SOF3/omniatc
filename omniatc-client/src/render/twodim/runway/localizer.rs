use bevy::asset::Assets;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::sprite::{ColorMaterial, MeshMaterial2d};
use bevy::transform::components::Transform;
use omniatc_core::level::runway::Runway;
use omniatc_core::try_log_return;
use omniatc_core::units::Distance;

use super::Conf;
use crate::config;
use crate::render::twodim::Zorder;
use crate::util::shapes;

#[derive(SystemParam)]
pub struct SpawnParam<'w, 's> {
    commands:  Commands<'w, 's>,
    shapes:    Res<'w, shapes::Meshes>,
    conf:      config::Read<'w, 's, Conf>,
    materials: ResMut<'w, Assets<ColorMaterial>>,
}

impl SpawnParam<'_, '_> {
    pub fn spawn(&mut self, runway: Entity) {
        self.commands.spawn((
            IsLocalizerOf(runway),
            ChildOf(runway),
            self.shapes.line(self.conf.localizer_thickness, Zorder::Localizer),
            MeshMaterial2d(
                self.materials
                    .add(ColorMaterial { color: self.conf.localizer_color, ..Default::default() }),
            ),
        ));
    }
}

#[derive(SystemParam)]
pub struct UpdateParam<'w, 's> {
    conf:            config::Read<'w, 's, Conf>,
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
        localizer_length: Distance<f32>,
    ) {
        let (mut line_tf, material_handle, mut thickness) = try_log_return!(self.localizer_query.get_mut(entity), expect "HasLocalizer should reference a localizer entity with transform");

        let material = try_log_return!(self.materials.get_mut(&material_handle.0), expect "asset referenced by strong handle must exist");
        material.color = self.conf.localizer_color;

        let localizer_length = runway.landing_length.normalize_to_magnitude(-localizer_length);
        shapes::set_square_line_transform(&mut line_tf, localizer_length);

        thickness.0 = self.conf.localizer_thickness;
    }
}

#[derive(Component)]
#[relationship(relationship_target = HasLocalizer)]
pub struct IsLocalizerOf(Entity);

#[derive(Component)]
#[relationship_target(relationship = IsLocalizerOf, linked_spawn)]
pub struct HasLocalizer(Entity);
