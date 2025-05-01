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
            IsStripOf(runway),
            ChildOf(runway),
            self.shapes.line(self.conf.strip_thickness, Zorder::RunwayStrip),
            MeshMaterial2d(
                self.materials
                    .add(ColorMaterial { color: self.conf.strip_color, ..Default::default() }),
            ),
        ));
    }
}

#[derive(SystemParam)]
pub struct UpdateParam<'w, 's> {
    conf:        config::Read<'w, 's, Conf>,
    strip_query: Query<
        'w,
        's,
        (
            &'static mut Transform,
            &'static MeshMaterial2d<ColorMaterial>,
            &'static mut shapes::MaintainThickness,
        ),
        With<IsStripOf>,
    >,
    materials:   ResMut<'w, Assets<ColorMaterial>>,
}

impl UpdateParam<'_, '_> {
    pub fn update(&mut self, runway: &Runway, &HasStrip(entity): &HasStrip) {
        let (mut line_tf, material_handle, mut thickness) = try_log_return!(self.strip_query.get_mut(entity), expect "HasStrip should reference a strip entity with transform");

        let material = try_log_return!(self.materials.get_mut(&material_handle.0), expect "asset referenced by strong handle must exist");
        material.color = self.conf.strip_color;

        shapes::set_square_line_transform_relative(
            &mut line_tf,
            Distance::ZERO,
            runway.landing_length,
        );

        thickness.0 = self.conf.strip_thickness;
    }
}

#[derive(Component)]
#[relationship(relationship_target = HasStrip)]
pub struct IsStripOf(Entity);

#[derive(Component)]
#[relationship_target(relationship = IsStripOf, linked_spawn)]
pub struct HasStrip(Entity);
