use bevy::ecs::query::With;
use bevy::ecs::system::{Single, SystemParam};
use bevy::math::{Quat, Vec2, Vec3, Vec3Swizzles};
use bevy::transform::components::GlobalTransform;
use math::{Angle, Heading, Length};

pub mod billboard;
pub mod shapes;

macro_rules! new_type_id {
    () => {
        $crate::util::new_type_id!(Anonymous)
    };
    ($name:ident) => {{
        struct $name;
        bevy_egui::egui::Id::new((stringify!($name), std::any::TypeId::of::<$name>()))
    }};
}
pub(crate) use new_type_id;

#[must_use]
pub fn heading_to_approx_name(heading: Heading) -> &'static str {
    let dirs = [
        ("north", Heading::NORTH),
        ("east", Heading::EAST),
        ("south", Heading::SOUTH),
        ("west", Heading::WEST),
        ("northeast", Heading::NORTH + Angle::RIGHT / 2.),
        ("southeast", Heading::EAST + Angle::RIGHT / 2.),
        ("southwest", Heading::SOUTH + Angle::RIGHT / 2.),
        ("northwest", Heading::WEST + Angle::RIGHT / 2.),
    ];
    for (name, dir) in dirs {
        if heading.closest_distance(dir).abs() <= Angle::RIGHT / 4. {
            return name;
        }
    }

    unreachable!("Heading must be within 22.5\u{b0} of one of the 8 directions")
}

#[derive(SystemParam)]
pub struct ActiveCamera2d<'w, 's> {
    camera: Single<'w, 's, &'static GlobalTransform, With<twodim::camera::Layout>>,
}

impl ActiveCamera2d<'_, '_> {
    #[must_use]
    pub fn rotation(&self) -> Quat { self.camera.rotation() }

    #[must_use]
    pub fn scale(&self) -> f32 { self.camera.scale().x }

    #[must_use]
    pub fn pixel_length(&self) -> Length<f32> { Length::new(self.scale()) }

    #[must_use]
    pub fn affine_transform(&self, vec: Vec2) -> Vec2 {
        (self.camera.affine().matrix3 * Vec3::from((vec, 0.))).xy()
    }
}

mod anchor;
pub use anchor::AnchorConf;

use crate::render::twodim;
