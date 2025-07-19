use bevy::app::{self, App, Plugin};
use bevy::asset::{Assets, Handle};
use bevy::color::Color;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Res, ResMut};
use bevy::sprite::{Anchor, ColorMaterial};
use bevy_mod_config::{self, AppExt, Config, ReadConfig, ReadConfigChange};
use math::{Angle, Length, LengthUnit};

use crate::util::AnchorConf;
use crate::{render, ConfigManager};

mod endpoint;
mod segment;
mod vis;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_config::<ConfigManager, Conf>("2d:aerodrome");
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
        conf: ReadConfig<Conf>,
    ) {
        let conf = conf.read();

        handles.runway = Some(materials.add(ColorMaterial::from_color(Color::NONE)));
        handles.taxiway = Some(materials.add(ColorMaterial::from_color(conf.taxiway.color)));
        handles.taxiway_label =
            Some(materials.add(ColorMaterial::from_color(conf.taxiway.label_color)));
        handles.apron = Some(materials.add(ColorMaterial::from_color(conf.apron.color)));
        handles.apron_label =
            Some(materials.add(ColorMaterial::from_color(conf.apron.label_color)));
    }

    fn reload_config_system(
        handles: Res<Self>,
        mut materials: ResMut<Assets<ColorMaterial>>,
        mut conf: ReadConfigChange<Conf>,
    ) {
        if !conf.consume_change() {
            return;
        }
        let conf = conf.read();

        for (handle, color) in [
            (&handles.taxiway, conf.taxiway.color),
            (&handles.taxiway_label, conf.taxiway.label_color),
            (&handles.apron, conf.apron.color),
            (&handles.apron_label, conf.apron.label_color),
        ] {
            materials
                .get_mut(handle.as_ref().expect("initialized during startup"))
                .expect("asset from strong reference must exist")
                .color = color;
        }
    }
}

#[derive(Config)]
#[config(expose(read))]
struct Conf {
    /// Thickness of non-runway segments in screen coordinates.
    #[config(default = 1.2, min = 0.0, max = 5.0)]
    segment_thickness:    f32,
    /// Minimum zoom level (in maximum distance per pixel) to display segments.
    #[config(default = Length::from_meters(50.0), min = Length::ZERO, max = Length::from_meters(500.), unit = LengthUnit::Meters)]
    segment_render_zoom:  Length<f32>,
    /// Distance of the curved intersection turn from the extrapolated intersection point.
    #[config(default = Length::from_meters(50.0), min = Length::from_meters(1.), max = Length::from_meters(200.), unit = LengthUnit::Meters)]
    intersection_size:    Length<f32>,
    /// Density of straight lines to interpolate a curved intersection turn.
    #[config(default = Angle::from_degrees(15.0), min = Angle::from_degrees(1.), max = Angle::RIGHT)]
    arc_interval:         Angle,
    /// Minimum zoom level (in maximum distance per pixel) to display endpoint turns.
    #[config(default = Length::from_meters(30.0), min = Length::ZERO, max = Length::from_meters(500.), unit = LengthUnit::Meters)]
    endpoint_render_zoom: Length<f32>,
    taxiway:              SegmentTypeConf,
    apron:                SegmentTypeConf,
}

#[derive(Config)]
#[config(expose(read))]
struct SegmentTypeConf {
    /// Color of the segments.
    color:             Color,
    /// Minimum zoom level (in maximum distance per pixel) to display segment labels.
    #[config(default = Length::from_meters(15.0), min = Length::ZERO, max = Length::from_meters(500.), unit = LengthUnit::Meters)]
    label_render_zoom: Length<f32>,
    /// Size of segment labels.
    #[config(default = 0.5, min = 0.0, max = 5.0)]
    label_size:        f32,
    /// Distance of segment labels from the center point in screen coordinates.
    #[config(default = 0.1, min = 0.0, max = 50.0)]
    label_distance:    f32,
    /// Direction of segment labels from the center point.
    #[config(default = Anchor::BottomCenter)]
    label_anchor:      AnchorConf,
    /// Color of segment labels.
    #[config(default = Color::WHITE)]
    label_color:       Color,
}
