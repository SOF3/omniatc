use bevy::asset::Handle;
use bevy::core_pipeline::core_2d::Camera2d;
use bevy::ecs::bundle::Bundle;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EventReader;
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, ParamSet, Query, Res, Single, SystemParam};
use bevy::math::Vec2;
use bevy::sprite::{Anchor, ColorMaterial, MeshMaterial2d};
use bevy::text::Text2d;
use bevy::transform::components::{GlobalTransform, Transform};
use math::{Distance, Position};
use omniatc::level::ground;
use omniatc::try_log_return;

use super::{vis, Conf};
use crate::config;
use crate::render::twodim::Zorder;
use crate::util::{billboard, shapes};

pub(super) fn regenerate_system(
    mut events: EventReader<ground::SegmentChangedEvent>,
    mut params: RegenerateParam,
) {
    for &ground::SegmentChangedEvent(segment_entity) in events.read() {
        params.regenerate(segment_entity);
    }
}

#[derive(SystemParam)]
pub(super) struct RegenerateParam<'w, 's> {
    commands:               Commands<'w, 's>,
    segment_query: Query<
        'w,
        's,
        (
            &'static ground::Segment,
            &'static ground::SegmentLabel,
            Option<&'static HasViewable>,
            Option<&'static HasLabel>,
        ),
    >,
    endpoint_query:         Query<'w, 's, &'static ground::Endpoint>,
    shapes:                 Res<'w, shapes::Meshes>,
    camera:                 Single<'w, &'static GlobalTransform, With<Camera2d>>,
    conf:                   config::Read<'w, 's, Conf>,
    materials:              Res<'w, super::ColorMaterials>,
    viewable_label_queries: ParamSet<
        'w,
        's,
        (
            Query<'w, 's, (&'static mut Transform, &'static mut shapes::MaintainThickness)>,
            Query<'w, 's, &'static mut Transform>,
        ),
    >,
}

impl RegenerateParam<'_, '_> {
    fn regenerate(&mut self, segment_entity: Entity) {
        let (segment, segment_label, has_segment, has_segment_label) = try_log_return!(
            self.segment_query.get(segment_entity),
            expect "{segment_entity:?} is not a segment entity"
        );

        let [alpha, beta] = try_log_return!(
            self.endpoint_query.get_many([segment.alpha, segment.beta]),
            expect "Segment endpoints {:?}, {:?} have invalid reference",
            segment.alpha, segment.beta,
        );
        let (alpha_trimmed, beta_trimmed) =
            trim_segment(alpha.position, beta.position, self.conf.intersection_size);

        if let Some(&HasViewable(viewable_entity)) = has_segment {
            let mut viewable_query = self.viewable_label_queries.p0();
            let (mut tf, mut thickness) = try_log_return!(viewable_query.get_mut(viewable_entity), expect "HasViewable must reference valid viewable {viewable_entity:?}");
            shapes::set_square_line_transform_relative(&mut tf, alpha_trimmed.0, beta_trimmed.0);
            thickness.0 = self.conf.segment_thickness;
        } else {
            self.commands.spawn((
                IsViewableOf(segment_entity),
                self.shapes.line_from_to(
                    self.conf.segment_thickness,
                    Zorder::GroundSegmentCenterline,
                    alpha_trimmed,
                    beta_trimmed,
                    &self.camera,
                ),
                MeshMaterial2d(match segment_label {
                    ground::SegmentLabel::RunwayPair(..) => {
                        self.materials.runway.clone().expect("initialized at startup")
                    }
                    ground::SegmentLabel::Taxiway { .. } => {
                        self.materials.taxiway.clone().expect("initialized at startup")
                    }
                    ground::SegmentLabel::Apron { .. } => {
                        self.materials.apron.clone().expect("initialized at startup")
                    }
                }),
                vis::SegmentMarker,
            ));
        }

        if let Some(&HasLabel(label_entity)) = has_segment_label {
            let mut label_query = self.viewable_label_queries.p1();
            let mut tf = try_log_return!(label_query.get_mut(label_entity), expect "HasLabel must reference valid label {label_entity:?}");
            shapes::set_square_line_transform_relative(&mut tf, alpha_trimmed.0, beta_trimmed.0);
        } else {
            match segment_label {
                ground::SegmentLabel::RunwayPair(..) => {}
                ground::SegmentLabel::Taxiway { name } => {
                    self.commands.spawn((
                        common_label_bundle(
                            segment_entity,
                            name,
                            alpha.position,
                            beta.position,
                            LabelConfig::taxiway(&self.conf),
                            self.materials.taxiway_label.clone().expect("initialized at startup"),
                        ),
                        vis::TaxiwayLabelMarker,
                    ));
                }
                ground::SegmentLabel::Apron { name } => {
                    self.commands.spawn((
                        common_label_bundle(
                            segment_entity,
                            name,
                            alpha.position,
                            beta.position,
                            LabelConfig::apron(&self.conf),
                            self.materials.apron_label.clone().expect("initialized at startup"),
                        ),
                        vis::ApronLabelMarker,
                    ));
                }
            }
        }
    }
}

struct LabelConfig {
    size:     f32,
    distance: f32,
    anchor:   Anchor,
}

impl LabelConfig {
    fn taxiway(conf: &Conf) -> Self {
        Self {
            size:     conf.taxiway_label_size,
            distance: conf.taxiway_label_distance,
            anchor:   conf.taxiway_label_anchor,
        }
    }

    fn apron(conf: &Conf) -> Self {
        Self {
            size:     conf.apron_label_size,
            distance: conf.apron_label_distance,
            anchor:   conf.apron_label_anchor,
        }
    }
}

/// Components shared between taxiway and apron labels.
fn common_label_bundle(
    segment_entity: Entity,
    name: &str,
    alpha: Position<Vec2>,
    beta: Position<Vec2>,
    label_conf: LabelConfig,
    material: Handle<ColorMaterial>,
) -> impl Bundle {
    (
        IsLabelOf(segment_entity),
        Text2d(name.to_string()),
        billboard::Label {
            offset:   alpha.midpoint(beta) - Position::ORIGIN,
            distance: label_conf.distance,
        },
        billboard::MaintainRotation,
        billboard::MaintainScale { size: label_conf.size },
        label_conf.anchor,
        MeshMaterial2d(material.clone()),
    )
}

fn trim_segment(
    a: Position<Vec2>,
    b: Position<Vec2>,
    length: Distance<f32>,
) -> (Position<Vec2>, Position<Vec2>) {
    let a_to_b_length = (b - a).normalize_to_magnitude(length);
    (a + a_to_b_length, b - a_to_b_length)
}

#[derive(Component)]
#[relationship(relationship_target = HasViewable)]
struct IsViewableOf(Entity);

#[derive(Component)]
#[relationship_target(relationship = IsViewableOf, linked_spawn)]
struct HasViewable(Entity);

#[derive(Component)]
#[relationship(relationship_target = HasLabel)]
struct IsLabelOf(Entity);

#[derive(Component)]
#[relationship_target(relationship = IsLabelOf, linked_spawn)]
struct HasLabel(Entity);
