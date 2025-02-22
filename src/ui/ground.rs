//! Displays ground structures of an aerodrome.

use std::mem;

use bevy::app::{self, App, Plugin};
use bevy::asset::{self, Assets};
use bevy::color::Color;
use bevy::math::{Dir2, Mat2, Vec2};
use bevy::prelude::{
    BuildChildren, Camera2d, Commands, Component, Entity, EntityCommands, EventReader,
    GlobalTransform, IntoSystemConfigs, Local, Mut, Query, Res, ResMut, Resource, Single,
    Transform, Visibility, With, Without,
};
use bevy::sprite::{ColorMaterial, MeshMaterial2d};
use bevy::text::Text2d;
use itertools::Itertools;
use omniatc_core::level::{aerodrome, ground};
use omniatc_core::math::range_steps;
use omniatc_core::units::{Angle, Distance, Position};

use super::{billboard, shapes, SystemSets, Zorder};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<Config>();
        app.init_resource::<ColorMaterials>();

        app.add_systems(app::Startup, init_color_materials);
        app.add_systems(app::Update, spawn_aerodrome_system.in_set(SystemSets::RenderSpawn));
        app.add_systems(app::Update, respawn_segment_system.in_set(SystemSets::RenderSpawn));
        app.add_systems(
            app::Update,
            respawn_endpoint_system.in_set(SystemSets::RenderSpawn).after(respawn_segment_system),
        );
        app.add_systems(app::Update, maintain_visibility_system.in_set(SystemSets::RenderMove));
    }
}

#[derive(Resource, Default)]
struct ColorMaterials {
    runway:        Option<asset::Handle<ColorMaterial>>,
    taxiway:       Option<asset::Handle<ColorMaterial>>,
    taxiway_label: Option<asset::Handle<ColorMaterial>>,
    apron:         Option<asset::Handle<ColorMaterial>>,
    apron_label:   Option<asset::Handle<ColorMaterial>>,
}

fn init_color_materials(
    mut handles: ResMut<ColorMaterials>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    config: Res<Config>,
) {
    handles.runway = Some(materials.add(ColorMaterial::from_color(Color::NONE)));
    handles.taxiway = Some(materials.add(ColorMaterial::from_color(config.taxiway_color)));
    handles.taxiway_label =
        Some(materials.add(ColorMaterial::from_color(config.taxiway_label_color)));
    handles.apron = Some(materials.add(ColorMaterial::from_color(config.apron_color)));
    handles.apron_label = Some(materials.add(ColorMaterial::from_color(config.apron_label_color)));
}

/// Marks a segment viewable entity.
#[derive(Component)]
struct SegmentViewable;

#[derive(Component)]
struct TaxiwayLabelViewable;

#[derive(Component)]
struct ApronLabelViewable;

/// Reference to the segment viewable entity.
#[derive(Component)]
struct SegmentViewableRef {
    viewable: Entity,
    label:    Entity,
}

/// Reference to the container entity of endpoint viewable entities.
#[derive(Component)]
struct EndpointViewablesRef(Entity);

fn spawn_aerodrome_system(mut events: EventReader<aerodrome::SpawnEvent>, mut commands: Commands) {
    events.read().for_each(|&aerodrome::SpawnEvent(entity)| {
        commands.entity(entity).insert((Transform::IDENTITY, Visibility::Inherited));
    });
}

fn respawn_segment_system(
    config: Res<Config>,
    mut events: EventReader<ground::SegmentChangedEvent>,
    mut commands: Commands,
    segment_query: Query<(&ground::Segment, &ground::SegmentLabel, Option<&SegmentViewableRef>)>,
    endpoint_query: Query<&ground::Endpoint>,
    draw_line: shapes::DrawLine,
    materials: Res<ColorMaterials>,
) {
    for &ground::SegmentChangedEvent(segment_entity) in events.read() {
        let Ok((segment, segment_label, viewable_ref)) = segment_query.get(segment_entity) else {
            bevy::log::error!("{segment_entity:?} is not a segment entity");
            continue;
        };
        if let Some(viewable) = viewable_ref {
            maintain_segment(
                &config,
                segment,
                segment_label,
                &endpoint_query,
                &mut commands.entity(viewable.viewable),
                &draw_line,
                &materials,
            );
            maintain_segment_label(
                &config,
                segment,
                segment_label,
                &endpoint_query,
                &mut commands.entity(viewable.label),
                &materials,
            );
        } else {
            let mut viewable =
                commands.spawn((bevy::core::Name::new("SegmentViewable"), SegmentViewable));
            viewable.set_parent(segment_entity);
            maintain_segment(
                &config,
                segment,
                segment_label,
                &endpoint_query,
                &mut viewable,
                &draw_line,
                &materials,
            );
            let viewable_entity = viewable.id();

            let mut label = commands.spawn(bevy::core::Name::new("SegmentLabel"));
            label.set_parent(segment_entity);
            maintain_segment_label(
                &config,
                segment,
                segment_label,
                &endpoint_query,
                &mut label,
                &materials,
            );
            let label_entity = label.id();

            commands.entity(segment_entity).insert((
                SegmentViewableRef { viewable: viewable_entity, label: label_entity },
                Transform::IDENTITY,
                Visibility::Inherited,
            ));
        }
    }
}

fn maintain_segment(
    config: &Res<Config>,
    segment: &ground::Segment,
    segment_label: &ground::SegmentLabel,
    endpoint_query: &Query<&ground::Endpoint>,
    viewable: &mut EntityCommands,
    draw_line: &shapes::DrawLine,
    materials: &ColorMaterials,
) {
    let Ok([alpha, beta]) = endpoint_query.get_many([segment.alpha, segment.beta]) else {
        bevy::log::error!(
            "Segment endpoints {:?}, {:?} have invalid reference",
            segment.alpha,
            segment.beta
        );
        return;
    };

    viewable.insert((
        draw_line.bundle(
            alpha.position
                + (beta.position - alpha.position).normalize_to_magnitude(config.intersection_size),
            beta.position
                + (alpha.position - beta.position).normalize_to_magnitude(config.intersection_size),
            config.segment_thickness,
            Zorder::GroundSegmentCenterline,
        ),
        MeshMaterial2d(match segment_label {
            ground::SegmentLabel::RunwayPair(..) => {
                materials.runway.clone().expect("initialized at startup")
            }
            ground::SegmentLabel::Taxiway { .. } => {
                materials.taxiway.clone().expect("initialized at startup")
            }
            ground::SegmentLabel::Apron { .. } => {
                materials.apron.clone().expect("initialized at startup")
            }
        }),
    ));
}

fn maintain_segment_label(
    config: &Res<Config>,
    segment: &ground::Segment,
    segment_label: &ground::SegmentLabel,
    endpoint_query: &Query<&ground::Endpoint>,
    entity: &mut EntityCommands,
    materials: &ColorMaterials,
) {
    let Ok([alpha, beta]) = endpoint_query.get_many([segment.alpha, segment.beta]) else {
        bevy::log::error!(
            "Segment endpoints {:?}, {:?} have invalid reference",
            segment.alpha,
            segment.beta
        );
        return;
    };

    let label_pos = alpha.position.midpoint(beta.position);
    match segment_label {
        ground::SegmentLabel::RunwayPair(..) => {}
        ground::SegmentLabel::Taxiway { name } => {
            entity.insert((
                TaxiwayLabelViewable,
                Text2d::new(name),
                billboard::MaintainScale { size: config.taxiway_label_size },
                billboard::MaintainRotation,
                billboard::Label { offset: label_pos - Position::ORIGIN, distance: 0. },
                MeshMaterial2d(materials.taxiway_label.clone().expect("initialized at startup")),
            ));
        }
        ground::SegmentLabel::Apron { name } => {
            entity.insert((
                ApronLabelViewable,
                Text2d::new(name),
                billboard::MaintainScale { size: config.apron_label_size },
                billboard::MaintainRotation,
                billboard::Label { offset: label_pos - Position::ORIGIN, distance: 0. },
                MeshMaterial2d(materials.apron_label.clone().expect("initialized at startup")),
            ));
        }
    }
}

/// Marks the endpoint viewable owner entity.
///
/// All viewable entities are owned by this endpoint.
/// Each viewable entity represents one pair of segments.
#[derive(Component)]
struct EndpointViewableOwner;

#[derive(Component)]
struct EndpointViewable;

/// Reference to the viewable owner entity from the `GroundEndpoint` entity.
#[derive(Component)]
struct EndpointViewableOwnerRef(Entity);

fn respawn_endpoint_system(
    config: Res<Config>,
    mut events: EventReader<ground::EndpointChangedEvent>,
    mut commands: Commands,
    endpoint_query: Query<(&ground::Endpoint, Option<&EndpointViewableOwnerRef>)>,
    segment_query: Query<&ground::Segment>,
    draw_line: shapes::DrawLine,
    materials: Res<ColorMaterials>,
) {
    for &ground::EndpointChangedEvent(endpoint) in events.read() {
        commands.entity(endpoint).insert((Transform::IDENTITY, Visibility::Inherited));

        let (&ground::Endpoint { position: intersect_position, ref adjacency }, owner_ref) =
            endpoint_query
                .get(endpoint)
                .expect("EndpointChangedEvent contains a non-endpoint entity");
        let owner = match owner_ref {
            None => {
                let mut owner = commands.spawn((
                    EndpointViewableOwner,
                    bevy::core::Name::new("EndpointViewableOwner"),
                    Transform::IDENTITY,
                    Visibility::Inherited,
                ));
                owner.set_parent(endpoint);
                let owner = owner.id();
                commands.entity(endpoint).insert(EndpointViewableOwnerRef(owner));
                owner
            }
            Some(owner_ref) => owner_ref.0,
        };

        for seg_entity_pair in adjacency.iter().array_combinations() {
            let [Some((p1, d1)), Some((p2, d2))] = seg_entity_pair.map(|&seg_entity| {
                let Ok(seg) = segment_query.get(seg_entity) else {
                    bevy::log::error!(
                        "endpoint adjacency list contains non-segment entity {seg_entity:?}"
                    );
                    return None;
                };
                let Some(other_endpoint) = seg.other_endpoint(endpoint) else {
                    bevy::log::error!(
                        "segment {seg_entity:?} is an adjacency of {endpoint:?} but does not \
                         contain the endpoint"
                    );
                    return None;
                };
                let Ok((&ground::Endpoint { position: other_position, .. }, _)) =
                    endpoint_query.get(other_endpoint)
                else {
                    bevy::log::error!(
                        "segment {seg_entity:?} contains invalid endpoint entity {endpoint:?}"
                    );
                    return None;
                };
                let Ok::<Dir2, _>(direction) = (other_position - intersect_position).0.try_into()
                else {
                    bevy::log::error!("segment {seg_entity:?} contains identical endpoints");
                    return None;
                };
                Some((intersect_position + config.intersection_size * direction, direction))
            }) else {
                continue;
            };

            let mat = Mat2::from_cols(*d1, *d2).transpose();
            if mat.determinant().abs() < 0.0001 {
                // `mat` is singular when `d1` and `d2` are almost parallel.
                // In that case we just draw a straight line to connect them.
                commands.entity(owner).with_child((
                    draw_line.bundle(
                        p1,
                        p2,
                        config.segment_thickness,
                        Zorder::GroundSegmentCenterline,
                    ),
                    MeshMaterial2d(materials.taxiway.clone().expect("initialized at startup")),
                    EndpointViewable,
                ));
            } else {
                // Consider d1.dot(center - p1) = d2.dot(center - p2) = 0
                let center = Position(Distance(
                    mat.inverse() * Vec2::new(d1.dot(p1.get()), d2.dot(p2.get())),
                ));

                for (start, end) in
                    arc_points(center, p1, intersect_position, p2, config.arc_interval)
                        .tuple_windows()
                {
                    commands.entity(owner).with_child((
                        draw_line.bundle(
                            start,
                            end,
                            config.segment_thickness,
                            Zorder::GroundSegmentCenterline,
                        ),
                        MeshMaterial2d(materials.taxiway.clone().expect("initialized at startup")),
                        EndpointViewable,
                    ));
                }
            }
        }
    }
}

fn arc_points(
    center: Position<Vec2>,
    start: Position<Vec2>,
    through: Position<Vec2>,
    end: Position<Vec2>,
    arc_interval: Angle<f32>,
) -> impl Iterator<Item = Position<Vec2>> {
    let radius = center.distance_exact(start);

    let start_heading = (start - center).heading();
    let through_heading = (through - center).heading();
    let end_heading = (end - center).heading();

    let mut dir = start_heading.closer_direction_to(end_heading);
    if start_heading.distance(end_heading, dir).abs()
        < start_heading.distance(through_heading, dir).abs()
    {
        dir = -dir;
    }

    let dist = start_heading.distance(end_heading, dir);
    range_steps(Angle::ZERO, dist, arc_interval.copysign(dist))
        .map(move |offset| center + radius * (start_heading + offset).into_dir2())
}

#[derive(Default)]
struct LastVis {
    segment:       Visibility,
    endpoint:      Visibility,
    taxiway_label: Visibility,
    apron_label:   Visibility,
}

fn maintain_visibility_system(
    camera: Single<&GlobalTransform, With<Camera2d>>,
    mut segment_query: Query<
        &mut Visibility,
        (
            With<SegmentViewable>,
            Without<EndpointViewableOwner>,
            Without<TaxiwayLabelViewable>,
            Without<ApronLabelViewable>,
        ),
    >,
    mut endpoint_query: Query<
        &mut Visibility,
        (With<EndpointViewableOwner>, Without<TaxiwayLabelViewable>, Without<ApronLabelViewable>),
    >,
    mut taxiway_label_query: Query<
        &mut Visibility,
        (With<TaxiwayLabelViewable>, Without<ApronLabelViewable>),
    >,
    mut apron_label_query: Query<&mut Visibility, With<ApronLabelViewable>>,
    mut last_vis: Local<LastVis>,
    config: Res<Config>,
) {
    fn update<'a>(
        pixel_width: Distance<f32>,
        zoom: Distance<f32>,
        last: &mut Visibility,
        query: impl Iterator<Item = Mut<'a, Visibility>>,
    ) {
        let vis = if zoom > pixel_width { Visibility::Inherited } else { Visibility::Hidden };
        if mem::replace(last, vis) != vis {
            query.for_each(|mut comp| *comp = vis);
        }
    }

    let pixel_width = Distance(camera.scale().x);

    update(
        pixel_width,
        config.segment_render_zoom,
        &mut last_vis.segment,
        segment_query.iter_mut(),
    );
    update(
        pixel_width,
        config.endpoint_render_zoom,
        &mut last_vis.endpoint,
        endpoint_query.iter_mut(),
    );
    update(
        pixel_width,
        config.taxiway_label_render_zoom,
        &mut last_vis.taxiway_label,
        taxiway_label_query.iter_mut(),
    );
    update(
        pixel_width,
        config.apron_label_render_zoom,
        &mut last_vis.apron_label,
        apron_label_query.iter_mut(),
    );
}

#[derive(Resource)]
pub struct Config {
    segment_thickness:   f32,
    /// Minimum zoom level (in maximum distance per pixel) to display segments.
    segment_render_zoom: Distance<f32>,
    /// Distance of the curved intersection turn from the extrapolated intersection point.
    intersection_size:   Distance<f32>,
    /// Density of straight lines to interpolate a curved intersection turn.
    arc_interval:        Angle<f32>,

    taxiway_color: Color,
    apron_color:   Color,

    /// Minimum zoom level (in maximum distance per pixel) to display endpoint turns.
    endpoint_render_zoom: Distance<f32>,

    /// Minimum zoom level (in maximum distance per pixel) to display taxiway labels.
    taxiway_label_render_zoom: Distance<f32>,
    taxiway_label_size:        f32,
    taxiway_label_color:       Color,

    /// Minimum zoom level (in maximum distance per pixel) to display apron labels.
    apron_label_render_zoom: Distance<f32>,
    apron_label_size:        f32,
    apron_label_color:       Color,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            segment_thickness:   1.2,
            segment_render_zoom: Distance::from_meters(100.),
            intersection_size:   Distance::from_meters(50.),
            arc_interval:        Angle::RIGHT / 8.,

            taxiway_color: Color::srgb(0.9, 0.9, 0.2),
            apron_color:   Color::srgb(0.8, 0.5, 0.1),

            endpoint_render_zoom: Distance::from_meters(50.),

            taxiway_label_render_zoom: Distance::from_meters(20.),
            taxiway_label_size:        0.4,
            taxiway_label_color:       Color::WHITE,

            apron_label_render_zoom: Distance::from_meters(10.),
            apron_label_size:        0.5,
            apron_label_color:       Color::WHITE,
        }
    }
}
