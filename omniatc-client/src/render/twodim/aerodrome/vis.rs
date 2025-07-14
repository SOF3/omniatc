use std::marker::PhantomData;

use bevy::app::{self, App, Plugin};
use bevy::core_pipeline::core_2d::Camera2d;
use bevy::ecs::change_detection::DetectChangesMut;
use bevy::ecs::component::Component;
use bevy::ecs::query::With;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Query, Single, SystemParam};
use bevy::render::view::Visibility;
use bevy::transform::components::GlobalTransform;
use math::Distance;

use super::Conf;
use crate::{config, render};

#[derive(Component)]
pub struct SegmentMarker;

#[derive(Component)]
pub struct EndpointMarker;

#[derive(Component)]
pub struct TaxiwayLabelMarker;

#[derive(Component)]
pub struct ApronLabelMarker;

pub fn add_plugins(app: &mut App) {
    app.add_plugins((
        Plug::<SegmentMarker, _>::new(|conf| conf.segment_render_zoom),
        Plug::<EndpointMarker, _>::new(|conf| conf.endpoint_render_zoom),
        Plug::<TaxiwayLabelMarker, _>::new(|conf| conf.taxiway_label_render_zoom),
        Plug::<ApronLabelMarker, _>::new(|conf| conf.apron_label_render_zoom),
    ));
}

struct Plug<ViewableFilter, GetConf> {
    get_conf: GetConf,
    _ph:      PhantomData<ViewableFilter>,
}

impl<
        ViewableFilter: Component,
        GetConf: Fn(&Conf) -> Distance<f32> + Copy + Send + Sync + 'static,
    > Plug<ViewableFilter, GetConf>
{
    #[allow(clippy::new_ret_no_self)]
    pub fn new(get_conf: GetConf) -> impl Plugin { Self { get_conf, _ph: PhantomData } }
}

impl<
        ViewableFilter: Component,
        GetConf: Fn(&Conf) -> Distance<f32> + Copy + Send + Sync + 'static,
    > Plugin for Plug<ViewableFilter, GetConf>
{
    fn build(&self, app: &mut App) {
        let get_conf = self.get_conf;
        app.add_systems(
            app::Update,
            (move |mut params: MaintainParams<ViewableFilter>| params.system(get_conf))
                .in_set(render::SystemSets::Update),
        );
    }
}

#[derive(SystemParam)]
struct MaintainParams<'w, 's, ViewableFilter: Component> {
    camera:    Single<'w, &'static GlobalTransform, With<Camera2d>>,
    conf:      config::Read<'w, 's, Conf>,
    vis_query: Query<'w, 's, &'static mut Visibility, With<ViewableFilter>>,
}

impl<ViewableFilter: Component> MaintainParams<'_, '_, ViewableFilter> {
    fn system<GetConf: Fn(&Conf) -> Distance<f32>>(&mut self, get_conf: GetConf) {
        let pixel_width = Distance::new(self.camera.scale().x);
        let zoom = get_conf(&self.conf);
        let vis = if zoom > pixel_width { Visibility::Inherited } else { Visibility::Hidden };

        self.vis_query.iter_mut().for_each(|mut comp| {
            comp.set_if_neq(vis);
        });
    }
}
