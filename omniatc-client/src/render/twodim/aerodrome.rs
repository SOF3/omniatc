use bevy::app::{self, App, Plugin};
use bevy::asset::{Assets, Handle};
use bevy::color::Color;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Res, ResMut};
use bevy::sprite::{Anchor, ColorMaterial};
use omniatc_core::units::{Angle, Distance, DistanceUnit};
use omniatc_macros::Config;

use crate::config::{self, AppExt};
use crate::render;

mod endpoint;
mod segment;
mod vis;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_config::<Conf>();
        app.init_resource::<ColorMaterials>();
        app.add_systems(app::Startup, ColorMaterials::init_system);
        app.add_systems(
            app::Update,
            ColorMaterials::reload_config_system.in_set(render::SystemSets::Reload),
        );
        app.add_systems(app::Update, segment::regenerate_system.in_set(render::SystemSets::Spawn));
        app.add_systems(app::Update, endpoint::regenerate_system.in_set(render::SystemSets::Spawn));
        vis::add_plugins(app);
    }
}

#[derive(Resource, Default)]
struct ColorMaterials {
    runway:        Option<Handle<ColorMaterial>>,
    taxiway:       Option<Handle<ColorMaterial>>,
    taxiway_label: Option<Handle<ColorMaterial>>,
    apron:         Option<Handle<ColorMaterial>>,
    apron_label:   Option<Handle<ColorMaterial>>,
}

impl ColorMaterials {
    fn init_system(
        mut handles: ResMut<Self>,
        mut materials: ResMut<Assets<ColorMaterial>>,
        conf: config::Read<Conf>,
    ) {
        handles.runway = Some(materials.add(ColorMaterial::from_color(Color::NONE)));
        handles.taxiway = Some(materials.add(ColorMaterial::from_color(conf.taxiway_color)));
        handles.taxiway_label =
            Some(materials.add(ColorMaterial::from_color(conf.taxiway_label_color)));
        handles.apron = Some(materials.add(ColorMaterial::from_color(conf.apron_color)));
        handles.apron_label =
            Some(materials.add(ColorMaterial::from_color(conf.apron_label_color)));
    }

    fn reload_config_system(
        handles: Res<Self>,
        mut materials: ResMut<Assets<ColorMaterial>>,
        mut conf: config::Read<Conf>,
    ) {
        let Some(conf) = conf.consume_change() else { return };

        for (handle, color) in [
            (&handles.taxiway, conf.taxiway_color),
            (&handles.taxiway_label, conf.taxiway_label_color),
            (&handles.apron, conf.apron_color),
            (&handles.apron_label, conf.apron_label_color),
        ] {
            materials
                .get_mut(handle.as_ref().expect("initialized during startup"))
                .expect("asset from strong reference must exist")
                .color = color;
        }
    }
}

#[derive(Resource, Config)]
#[config(id = "aerodrome", name = "Aerodrome (2D)")]
struct Conf {
    /// Thickness of non-runway segments in screen coordinates.
    #[config(min = 0., max = 5.)]
    segment_thickness:   f32,
    /// Minimum zoom level (in maximum distance per pixel) to display segments.
    #[config(min = Distance::ZERO, max = Distance::from_meters(500.), precision = Distance::from_meters(10.), unit = DistanceUnit::Meters)]
    segment_render_zoom: Distance<f32>,
    /// Distance of the curved intersection turn from the extrapolated intersection point.
    #[config(min = Distance::from_meters(1.), max = Distance::from_meters(200.), unit = DistanceUnit::Meters)]
    intersection_size:   Distance<f32>,
    /// Density of straight lines to interpolate a curved intersection turn.
    #[config(min = Angle::from_degrees(1.), max = Angle::RIGHT)]
    arc_interval:        Angle<f32>,

    /// Color of taxiways.
    taxiway_color: Color,
    /// Color of aprons.
    apron_color:   Color,

    /// Minimum zoom level (in maximum distance per pixel) to display endpoint turns.
    #[config(min = Distance::ZERO, max = Distance::from_meters(500.), precision = Distance::from_meters(10.), unit = DistanceUnit::Meters)]
    endpoint_render_zoom: Distance<f32>,

    /// Minimum zoom level (in maximum distance per pixel) to display taxiway labels.
    #[config(min = Distance::ZERO, max = Distance::from_meters(500.), precision = Distance::from_meters(10.), unit = DistanceUnit::Meters)]
    taxiway_label_render_zoom: Distance<f32>,
    /// Size of taxiway labels.
    #[config(min = 0., max = 5.)]
    taxiway_label_size:        f32,
    /// Distance of taxiway labels from the center point in screen coordinates.
    #[config(min = 0., max = 50.)]
    taxiway_label_distance:    f32,
    /// Direction of taxiway labels from the center point.
    taxiway_label_anchor:      Anchor,
    /// Color of taxiway labels.
    taxiway_label_color:       Color,

    /// Minimum zoom level (in maximum distance per pixel) to display apron labels.
    #[config(min = Distance::ZERO, max = Distance::from_meters(500.), precision = Distance::from_meters(10.), unit = DistanceUnit::Meters)]
    apron_label_render_zoom: Distance<f32>,
    /// Size of apron labels.
    #[config(min = 0., max = 5.)]
    apron_label_size:        f32,
    /// Distance of apron labels from the center point in screen coordinates.
    #[config(min = 0., max = 50.)]
    apron_label_distance:    f32,
    /// Direction of apron labels from the center point.
    apron_label_anchor:      Anchor,
    /// Color of apron labels.
    apron_label_color:       Color,
}

impl Default for Conf {
    fn default() -> Self {
        Self {
            segment_thickness:   1.2,
            segment_render_zoom: Distance::from_meters(50.),
            intersection_size:   Distance::from_meters(50.),
            arc_interval:        Angle::RIGHT / 4.,

            taxiway_color: Color::srgb(0.9, 0.9, 0.2),
            apron_color:   Color::srgb(0.8, 0.5, 0.1),

            endpoint_render_zoom: Distance::from_meters(30.),

            taxiway_label_render_zoom: Distance::from_meters(15.),
            taxiway_label_size:        0.45,
            taxiway_label_distance:    0.,
            taxiway_label_anchor:      Anchor::BottomCenter,
            taxiway_label_color:       Color::WHITE,

            apron_label_render_zoom: Distance::from_meters(10.),
            apron_label_size:        0.5,
            apron_label_distance:    0.,
            apron_label_anchor:      Anchor::BottomCenter,
            apron_label_color:       Color::WHITE,
        }
    }
}
