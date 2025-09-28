use bevy::app::{self, App, Plugin};
use bevy::asset::{Assets, Handle, RenderAssetUsages};
use bevy::color::Color;
use bevy::ecs::bundle::Bundle;
use bevy::ecs::component::Component;
use bevy::ecs::entity::{Entity, EntityHashSet};
use bevy::ecs::event::EventReader;
use bevy::ecs::query::{QueryData, With};
use bevy::ecs::relationship::{Relationship, RelationshipTarget};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Local, ParamSet, Query, Res, ResMut, Single, SystemParam};
use bevy::math::{Dir2, Vec2};
use bevy::render::mesh::{Mesh, Mesh2d, PrimitiveTopology, VertexAttributeValues};
use bevy::render::view::Visibility;
use bevy::sprite::{Anchor, ColorMaterial, MeshMaterial2d};
use bevy::text::Text2d;
use bevy::transform::components::GlobalTransform;
use bevy_mod_config::{self, AppExt, Config, ReadConfig, ReadConfigChange};
use itertools::Itertools;
use math::{Angle, Heading, Length, LengthUnit, Position};
use omniatc::QueryTryLog;
use omniatc::level::aerodrome::Aerodrome;
use omniatc::level::ground;

use crate::render::twodim::{Zorder, camera};
use crate::util::{AnchorConf, billboard};
use crate::{ConfigManager, render};

#[cfg(test)]
mod tests;

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
        app.add_systems(app::Update, regenerate_system.in_set(render::SystemSets::Spawn));
        app.add_systems(app::Update, update_visibility_system.in_set(render::SystemSets::Update));
    }
}

fn update_visibility_system(
    conf: ReadConfig<Conf>,
    camera: Single<&GlobalTransform, With<camera::Layout>>,
    mesh_query: Query<(&mut Visibility, &MeshType)>,
) {
    let pixel_width = Length::new(camera.scale().x);
    for (mut vis, mesh_type) in mesh_query {
        let min_width = match mesh_type {
            MeshType::TaxiwayCenterline => conf.read().taxiway.centerline_render_zoom,
            MeshType::ApronCenterline => conf.read().apron.centerline_render_zoom,
            MeshType::TaxiwayBackground => conf.read().taxiway.background_render_zoom,
            MeshType::ApronBackground => conf.read().apron.background_render_zoom,
            MeshType::TaxiwayLabel => conf.read().taxiway.label_render_zoom,
            MeshType::ApronLabel => conf.read().apron.label_render_zoom,
        };

        if pixel_width <= min_width {
            *vis = Visibility::Visible;
        } else {
            *vis = Visibility::Hidden;
        }
    }
}

const SCALE_REGEN_BASE: f32 = 1.2;

fn regenerate_system(mut params: ParamSet<(FindOutdatedAerodromesParam, RegenerateMeshParam)>) {
    let (aerodromes_outdated, min_width) = params.p0().find_outdated();

    for aerodrome in aerodromes_outdated {
        params.p1().regenerate(aerodrome, min_width);
    }
}

#[derive(SystemParam)]
struct FindOutdatedAerodromesParam<'w, 's> {
    events:            EventReader<'w, 's, ground::ChangedEvent>,
    conf:              ReadConfig<'w, 's, Conf>,
    camera:            Single<'w, &'static GlobalTransform, With<camera::Layout>>,
    aerodrome_query:   Query<'w, 's, Entity, With<Aerodrome>>,
    last_render_width: Local<'s, Option<Length<f32>>>,
}

impl FindOutdatedAerodromesParam<'_, '_> {
    fn find_outdated(&mut self) -> (EntityHashSet, Length<f32>) {
        let conf = self.conf.read();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "log() of small values would not be truncated"
        )]
        let scale_y =
            SCALE_REGEN_BASE.powi(self.camera.scale().y.log(SCALE_REGEN_BASE).floor() as i32);
        let min_width = Length::new(scale_y) * conf.segment_thickness;

        let mut aerodromes_outdated = EntityHashSet::new();
        if self.last_render_width.replace(min_width) == Some(min_width) {
            for &ground::ChangedEvent { aerodrome } in self.events.read() {
                aerodromes_outdated.insert(aerodrome);
            }
        } else {
            aerodromes_outdated.extend(self.aerodrome_query.iter());
            self.events.read().for_each(drop);
        }
        (aerodromes_outdated, min_width)
    }
}

#[derive(SystemParam)]
struct RegenerateMeshParam<'w, 's> {
    aerodrome_query: Query<'w, 's, RegenerateAerodromeData>,
    steps: ParamSet<
        'w,
        's,
        (RegenerateLinesParam<'w, 's>, CommitContext<'w, 's>, RegenerateLabelsParam<'w, 's>),
    >,
    materials:       Res<'w, ColorMaterials>,
    conf:            ReadConfig<'w, 's, Conf>,
}

impl RegenerateMeshParam<'_, '_> {
    fn regenerate(
        &mut self,
        aerodrome_entity: Entity,
        centerline_width: Length<f32>,
    ) -> Option<()> {
        let conf = self.conf.read();

        let aerodrome = self.aerodrome_query.log_get(aerodrome_entity)?;

        let mut taxiway_centerline_positions = Vec::new();
        let mut apron_centerline_positions = Vec::new();
        let mut taxiway_background_positions = Vec::new();
        let mut apron_background_positions = Vec::new();

        {
            let regen_lines = self.steps.p0();
            for &segment_entity in aerodrome.segments.segments() {
                regen_lines.push_segment(
                    segment_entity,
                    &mut taxiway_centerline_positions,
                    &mut apron_centerline_positions,
                    |_| centerline_width,
                )?;
                regen_lines.push_segment(
                    segment_entity,
                    &mut taxiway_background_positions,
                    &mut apron_background_positions,
                    |segment| segment.width,
                )?;
            }

            for &endpoint_entity in aerodrome.endpoints.endpoints() {
                regen_lines.push_endpoint(
                    endpoint_entity,
                    &mut taxiway_centerline_positions,
                    |_, _| centerline_width,
                    &conf,
                )?;
                regen_lines.push_endpoint(
                    endpoint_entity,
                    &mut taxiway_background_positions,
                    |from, to| from.width.min(to.width),
                    &conf,
                )?;
            }
        }

        {
            let mut commit_ctx = self.steps.p1();
            commit_ctx.commit_mesh(
                aerodrome.taxiway_centerline_mesh,
                self.materials.taxiway_centerline.as_ref().expect("initialized during startup"),
                taxiway_centerline_positions,
                aerodrome_entity,
                Zorder::GroundSegmentCenterline,
                MeshType::TaxiwayCenterline,
            );
            commit_ctx.commit_mesh(
                aerodrome.taxiway_background_mesh,
                self.materials.taxiway_background.as_ref().expect("initialized during startup"),
                taxiway_background_positions,
                aerodrome_entity,
                Zorder::GroundSegmentBackground,
                MeshType::TaxiwayBackground,
            );
            commit_ctx.commit_mesh(
                aerodrome.apron_centerline_mesh,
                self.materials.apron_centerline.as_ref().expect("initialized during startup"),
                apron_centerline_positions,
                aerodrome_entity,
                Zorder::GroundSegmentCenterline,
                MeshType::ApronCenterline,
            );
            commit_ctx.commit_mesh(
                aerodrome.apron_background_mesh,
                self.materials.apron_background.as_ref().expect("initialized during startup"),
                apron_background_positions,
                aerodrome_entity,
                Zorder::GroundSegmentBackground,
                MeshType::ApronBackground,
            );
        }

        {
            let mut regen_labels = self.steps.p2();
            regen_labels
                .commands
                .entity(aerodrome_entity)
                .despawn_related::<AerodromeHasSegmentLabels>();

            for &segment_entity in aerodrome.segments.segments() {
                regen_labels.maybe_draw_label(aerodrome_entity, segment_entity, &conf);
            }
        }

        Some(())
    }
}

#[derive(QueryData)]
struct RegenerateAerodromeData {
    segments:                &'static ground::AerodromeSegments,
    endpoints:               &'static ground::AerodromeEndpoints,
    taxiway_centerline_mesh: Option<&'static AerodromeHasTaxiwayCenterlineMesh>,
    taxiway_background_mesh: Option<&'static AerodromeHasTaxiwayBackgroundMesh>,
    apron_centerline_mesh:   Option<&'static AerodromeHasApronCenterlineMesh>,
    apron_background_mesh:   Option<&'static AerodromeHasApronBackgroundMesh>,
}

#[derive(SystemParam)]
struct RegenerateLinesParam<'w, 's> {
    endpoint_query: Query<'w, 's, &'static ground::Endpoint>,
    segment_query:  Query<'w, 's, (&'static ground::Segment, &'static ground::SegmentLabel)>,
}

impl RegenerateLinesParam<'_, '_> {
    fn push_segment(
        &self,
        segment_entity: Entity,
        taxiway_positions: &mut impl DrawLineSegment,
        apron_positions: &mut impl DrawLineSegment,
        width_from_segment: impl Fn(&ground::Segment) -> Length<f32>,
    ) -> Option<()> {
        let (segment, segment_label) = self.segment_query.log_get(segment_entity)?;
        let width = width_from_segment(segment);
        match segment_label {
            ground::SegmentLabel::RunwayPair { .. } => {} // runway strips are rendered in the runway plugin
            ground::SegmentLabel::Taxiway { .. } => {
                let [alpha, beta] =
                    self.endpoint_query.log_get_many([segment.alpha, segment.beta])?;
                taxiway_positions.draw_segment_trunc(
                    alpha.position,
                    alpha.adjacency.len() > 1,
                    beta.position,
                    beta.adjacency.len() > 1,
                    width,
                );
            }
            ground::SegmentLabel::Apron { .. } => {
                let [alpha, beta] =
                    self.endpoint_query.log_get_many([segment.alpha, segment.beta])?;
                apron_positions.draw_segment_trunc(
                    alpha.position,
                    alpha.adjacency.len() > 1,
                    beta.position,
                    beta.adjacency.len() > 1,
                    width,
                );
            }
        }

        Some(())
    }

    pub fn push_endpoint(
        &self,
        endpoint_entity: Entity,
        taxiway_positions: &mut impl DrawLineSegment,
        width_from_segments: impl Fn(&ground::Segment, &ground::Segment) -> Length<f32>,
        conf: &ConfRead,
    ) -> Option<()> {
        let endpoint = self.endpoint_query.log_get(endpoint_entity)?;

        for (from_entity, to_entity) in endpoint.adjacency.iter().tuple_combinations() {
            let [(from, _), (to, _)] =
                self.segment_query.log_get_many([*from_entity, *to_entity])?;

            let from_peer = from
                .other_endpoint(endpoint_entity)
                .expect("adjacent segment of endpoint must contain the endpoint");
            let from_dir = (self.endpoint_query.log_get(from_peer)?.position - endpoint.position)
                .dir()
                .expect("endpoints of segment should not be colocated");

            let to_peer = to
                .other_endpoint(endpoint_entity)
                .expect("adjacent segment of endpoint must contain the endpoint");
            let to_dir = (self.endpoint_query.log_get(to_peer)?.position - endpoint.position)
                .dir()
                .expect("endpoints of segment should not be colocated");

            push_curve(
                taxiway_positions,
                endpoint.position,
                [from.width * 0.5, to.width * 0.5],
                [from_dir, to_dir],
                width_from_segments(from, to),
                conf.curve_segment_length,
            );
        }

        Some(())
    }
}

const CURVE_EXTENSION_EPSILON: Length<f32> = Length::from_meters(0.001);

fn push_curve(
    positions: &mut impl DrawLineSegment,
    tangent_intersect: Position<Vec2>,
    [from_offset, to_offset]: [Length<f32>; 2],
    [from_dir, to_dir]: [Dir2; 2],
    width: Length<f32>,
    curve_segment_length: Length<f32>,
) {
    let offset = from_offset.min(to_offset);

    let from_heading = Heading::from_vec2(*from_dir);
    let to_heading = Heading::from_vec2(*to_dir);
    let offsets_angle = from_heading.closest_distance(to_heading).abs();
    let angular_delta = Angle::STRAIGHT - offsets_angle;
    let turn_radius = offset / (angular_delta * 0.5).acute_signed_tan();

    let from_pos = tangent_intersect + offset * from_dir;
    let to_pos = tangent_intersect + offset * to_dir;
    let turn_center = tangent_intersect
        + turn_radius / (angular_delta * 0.5).cos() * from_heading.closest_midpoint(to_heading);

    // draw short segments of curve_segment_length to fill the curve
    let arc_length = turn_radius.radius_to_arc(angular_delta);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "ceil() of small values would not be truncated"
    )]
    #[expect(clippy::cast_sign_loss, reason = "arc_length must be positive")]
    let pieces = (arc_length / curve_segment_length).ceil().min(90.0) as u16;

    let start_heading = (from_pos - turn_center).heading();
    let end_heading = (to_pos - turn_center).heading();
    let signed_angle = start_heading.closest_distance(end_heading) / f32::from(pieces);
    for step in 0..pieces {
        positions.draw_segment(
            turn_center + turn_radius * (start_heading + signed_angle * f32::from(step)),
            turn_center + turn_radius * (start_heading + signed_angle * f32::from(step + 1)),
            width,
        );
    }

    // draw the extension straight segment due to inconsistent offsets
    if from_offset > to_offset + CURVE_EXTENSION_EPSILON {
        positions.draw_segment(
            tangent_intersect + to_offset * from_dir,
            tangent_intersect + from_offset * from_dir,
            width,
        );
    }
    if to_offset > from_offset + CURVE_EXTENSION_EPSILON {
        positions.draw_segment(
            tangent_intersect + from_offset * to_dir,
            tangent_intersect + to_offset * to_dir,
            width,
        );
    }
}

trait DrawLineSegment {
    fn draw_segment(&mut self, from: Position<Vec2>, to: Position<Vec2>, width: Length<f32>) {
        self.draw_segment_trunc(from, true, to, true, width);
    }
    fn draw_segment_trunc(
        &mut self,
        from: Position<Vec2>,
        from_trunc: bool,
        to: Position<Vec2>,
        to_trunc: bool,
        width: Length<f32>,
    );
}

impl DrawLineSegment for Vec<[f32; 3]> {
    fn draw_segment_trunc(
        &mut self,
        mut from: Position<Vec2>,
        from_trunc: bool,
        mut to: Position<Vec2>,
        to_trunc: bool,
        width: Length<f32>,
    ) {
        // Length of half the segment width, in the direction from `from` to `to`.
        let half_width = (to - from).normalize_to_magnitude(width * 0.5);
        if from_trunc {
            from += half_width;
        }
        if to_trunc {
            to -= half_width;
        }

        let normal = half_width.rotate_right_angle_clockwise();

        let from_ac = from.0 - normal;
        let from_cw = from.0 + normal;
        let to_ac = to.0 - normal;
        let to_cw = to.0 + normal;

        self.extend_from_slice(
            &[from_ac, from_cw, to_ac, to_ac, from_cw, to_cw].map(|p| [p.x().0, p.y().0, 0.0]),
        );
    }
}

#[derive(SystemParam)]
struct CommitContext<'w, 's> {
    mesh_query: Query<'w, 's, &'static Mesh2d>,
    commands:   Commands<'w, 's>,
    meshes:     ResMut<'w, Assets<Mesh>>,
    materials:  ResMut<'w, Assets<ColorMaterial>>,
}

#[derive(Component)]
enum MeshType {
    TaxiwayCenterline,
    ApronCenterline,
    TaxiwayBackground,
    ApronBackground,
    TaxiwayLabel,
    ApronLabel,
}

impl CommitContext<'_, '_> {
    fn commit_mesh<R>(
        &mut self,
        mesh_entity: Option<&R>,
        material: &Handle<ColorMaterial>,
        positions: Vec<[f32; 3]>,
        aerodrome: Entity,
        zorder: Zorder,
        mesh_type: MeshType,
    ) where
        R: RelationshipTarget + Copy + Into<Entity>,
    {
        match mesh_entity {
            None => {
                let mesh = self.meshes.add(
                    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::all())
                        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions),
                );

                self.commands.spawn((
                    Mesh2d(mesh),
                    MeshMaterial2d(material.clone()),
                    zorder.local_translation(),
                    <R::Relationship as Relationship>::from(aerodrome),
                    mesh_type,
                ));
            }
            Some(rel) => {
                let mesh_entity: Entity = (*rel).into();
                let mesh_handle = self
                    .mesh_query
                    .get(mesh_entity)
                    .expect("linked child entity must be a mesh entity")
                    .0
                    .clone();
                let mesh = self
                    .meshes
                    .get_mut(&mesh_handle)
                    .expect("asset from strong reference must exist");
                let Some(VertexAttributeValues::Float32x3(mesh_positions)) =
                    mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
                else {
                    panic!("Position attribute was initialized as Float32x3 during spawn");
                };
                *mesh_positions = positions;
            }
        }
    }
}

#[derive(SystemParam)]
struct RegenerateLabelsParam<'w, 's> {
    endpoint_query: Query<'w, 's, &'static ground::Endpoint>,
    segment_query: Query<
        'w,
        's,
        (&'static ground::Segment, &'static ground::SegmentLabel),
        With<ground::SegmentShouldDisplayLabel>,
    >,
    commands:       Commands<'w, 's>,
    materials:      Res<'w, ColorMaterials>,
}

impl RegenerateLabelsParam<'_, '_> {
    fn maybe_draw_label(
        &mut self,
        aerodrome_entity: Entity,
        segment_entity: Entity,
        conf: &ConfRead,
    ) {
        let Ok((segment, label)) = self.segment_query.get(segment_entity) else { return };
        let Some(alpha) = self.endpoint_query.log_get(segment.alpha) else { return };
        let Some(beta) = self.endpoint_query.log_get(segment.beta) else { return };

        match label {
            ground::SegmentLabel::RunwayPair(..) => {} // runway labels are rendered separately
            ground::SegmentLabel::Taxiway { name } => {
                self.commands.spawn((
                    common_label_bundle(
                        aerodrome_entity,
                        name,
                        alpha.position,
                        beta.position,
                        &conf.taxiway,
                        self.materials.taxiway_label.as_ref().expect("initialized during startup"),
                    ),
                    MeshType::TaxiwayLabel,
                ));
            }
            ground::SegmentLabel::Apron { name } => {
                self.commands.spawn((
                    common_label_bundle(
                        aerodrome_entity,
                        name,
                        alpha.position,
                        beta.position,
                        &conf.apron,
                        self.materials.apron_label.as_ref().expect("initialized during startup"),
                    ),
                    MeshType::ApronLabel,
                ));
            }
        }
    }
}

/// Components shared between taxiway and apron labels.
fn common_label_bundle(
    aerodrome_entity: Entity,
    name: &str,
    alpha: Position<Vec2>,
    beta: Position<Vec2>,
    conf: &SegmentTypeConfRead,
    material: &Handle<ColorMaterial>,
) -> impl Bundle {
    (
        SegmentLabelsOfAerodrome(aerodrome_entity),
        Text2d(name.to_string()),
        billboard::Label {
            offset:   alpha.midpoint(beta) - Position::ORIGIN,
            distance: conf.label_distance,
        },
        billboard::MaintainRotation,
        billboard::MaintainScale { size: conf.label_size },
        conf.label_anchor,
        Zorder::GroundSegmentLabel.local_translation(),
        MeshMaterial2d(material.clone()),
    )
}

#[derive(Component)]
#[relationship(relationship_target = AerodromeHasTaxiwayCenterlineMesh)]
struct TaxiwayCenterlineMeshOfAerodrome(pub Entity);

#[derive(Component)]
#[relationship_target(relationship = TaxiwayCenterlineMeshOfAerodrome, linked_spawn)]
#[derive(Clone, Copy, derive_more::Into)]
struct AerodromeHasTaxiwayCenterlineMesh(Entity);

#[derive(Component)]
#[relationship(relationship_target = AerodromeHasTaxiwayBackgroundMesh)]
struct TaxiwayBackgroundMeshOfAerodrome(pub Entity);

#[derive(Component)]
#[relationship_target(relationship = TaxiwayBackgroundMeshOfAerodrome, linked_spawn)]
#[derive(Clone, Copy, derive_more::Into)]
struct AerodromeHasTaxiwayBackgroundMesh(Entity);

#[derive(Component)]
#[relationship(relationship_target = AerodromeHasApronCenterlineMesh)]
struct ApronCenterlineMeshOfAerodrome(pub Entity);

#[derive(Component)]
#[relationship_target(relationship = ApronCenterlineMeshOfAerodrome, linked_spawn)]
#[derive(Clone, Copy, derive_more::Into)]
struct AerodromeHasApronCenterlineMesh(Entity);

#[derive(Component)]
#[relationship(relationship_target = AerodromeHasApronBackgroundMesh)]
struct ApronBackgroundMeshOfAerodrome(pub Entity);

#[derive(Component)]
#[relationship_target(relationship = ApronBackgroundMeshOfAerodrome, linked_spawn)]
#[derive(Clone, Copy, derive_more::Into)]
struct AerodromeHasApronBackgroundMesh(Entity);

#[derive(Component)]
#[relationship(relationship_target = AerodromeHasSegmentLabels)]
struct SegmentLabelsOfAerodrome(pub Entity);

#[derive(Component)]
#[relationship_target(relationship = SegmentLabelsOfAerodrome, linked_spawn)]
struct AerodromeHasSegmentLabels(Vec<Entity>);

#[derive(Resource, Default)]
struct ColorMaterials {
    runway:             Option<Handle<ColorMaterial>>,
    taxiway_centerline: Option<Handle<ColorMaterial>>,
    taxiway_background: Option<Handle<ColorMaterial>>,
    taxiway_label:      Option<Handle<ColorMaterial>>,
    apron_centerline:   Option<Handle<ColorMaterial>>,
    apron_background:   Option<Handle<ColorMaterial>>,
    apron_label:        Option<Handle<ColorMaterial>>,
}

impl ColorMaterials {
    fn init_system(
        mut handles: ResMut<Self>,
        mut materials: ResMut<Assets<ColorMaterial>>,
        conf: ReadConfig<Conf>,
    ) {
        let conf = conf.read();

        handles.runway = Some(materials.add(ColorMaterial::from_color(Color::NONE)));
        handles.taxiway_centerline =
            Some(materials.add(ColorMaterial::from_color(conf.taxiway.centerline_color)));
        handles.taxiway_background =
            Some(materials.add(ColorMaterial::from_color(conf.taxiway.background_color)));
        handles.taxiway_label =
            Some(materials.add(ColorMaterial::from_color(conf.taxiway.label_color)));
        handles.apron_centerline =
            Some(materials.add(ColorMaterial::from_color(conf.apron.centerline_color)));
        handles.apron_background =
            Some(materials.add(ColorMaterial::from_color(conf.apron.background_color)));
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
            (&handles.taxiway_centerline, conf.taxiway.centerline_color),
            (&handles.taxiway_background, conf.taxiway.background_color),
            (&handles.taxiway_label, conf.taxiway.label_color),
            (&handles.apron_centerline, conf.apron.centerline_color),
            (&handles.apron_background, conf.apron.background_color),
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
    /// Density of straight lines to interpolate a curved intersection turn.
    #[config(default = Length::from_meters(1.0), min = Length::from_meters(0.1), max = Length::from_meters(100.0))]
    curve_segment_length: Length<f32>,
    taxiway:              SegmentTypeConf,
    apron:                SegmentTypeConf,
}

#[derive(Config)]
#[config(expose(read))]
struct SegmentTypeConf {
    /// Color of the segment centerline.
    #[config(default = Color::srgb(0.7, 0.7, 0.3))]
    centerline_color:       Color,
    /// Color of the segment background.
    #[config(default = Color::srgb(0.3, 0.3, 0.3))]
    background_color:       Color,
    /// Minimum zoom level (in maximum distance per pixel) to display centerline.
    #[config(default = Length::from_meters(50.0), min = Length::ZERO, max = Length::from_meters(500.), unit = LengthUnit::Meters)]
    centerline_render_zoom: Length<f32>,
    /// Minimum zoom level (in maximum distance per pixel) to display segment background.
    #[config(default = Length::from_meters(20.0), min = Length::ZERO, max = Length::from_meters(500.), unit = LengthUnit::Meters)]
    background_render_zoom: Length<f32>,
    /// Minimum zoom level (in maximum distance per pixel) to display segment labels.
    #[config(default = Length::from_meters(15.0), min = Length::ZERO, max = Length::from_meters(500.), unit = LengthUnit::Meters)]
    label_render_zoom:      Length<f32>,
    /// Size of segment labels.
    #[config(default = 0.5, min = 0.0, max = 5.0)]
    label_size:             f32,
    /// Distance of segment labels from the center point in screen coordinates.
    #[config(default = 0.1, min = 0.0, max = 50.0)]
    label_distance:         f32,
    /// Direction of segment labels from the center point.
    #[config(default = Anchor::BottomCenter)]
    label_anchor:           AnchorConf,
    /// Color of segment labels.
    #[config(default = Color::WHITE)]
    label_color:            Color,
}
