use bevy::asset::Handle;
use bevy::ecs::bundle::Bundle;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EventReader;
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, ParamSet, Query, Res, Single, SystemParam};
use bevy::math::Vec2;
use bevy::sprite::{ColorMaterial, MeshMaterial2d};
use bevy::text::Text2d;
use bevy::transform::components::{GlobalTransform, Transform};
use bevy_mod_config::{self, ReadConfig};
use math::{Length, Position};
use omniatc::level::ground;
use omniatc::{QueryTryLog, try_log_return};

use super::{Conf, SegmentTypeConfRead, vis};
use crate::render::twodim::{Zorder, camera};
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
    camera:                 Single<'w, &'static GlobalTransform, With<camera::Layout>>,
    conf:                   ReadConfig<'w, 's, Conf>,
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
        let conf = self.conf.read();

        let Some((segment, segment_label, has_segment, has_segment_label)) =
            self.segment_query.log_get(segment_entity)
        else {
            return;
        };

        let [alpha, beta] = try_log_return!(
            self.endpoint_query.get_many([segment.alpha, segment.beta]),
            expect "Segment endpoints {:?}, {:?} have invalid reference",
            segment.alpha, segment.beta,
        );
        let (alpha_trimmed, beta_trimmed) =
            trim_segment(alpha.position, beta.position, conf.intersection_size);

        if let Some(&HasViewable(viewable_entity)) = has_segment {
            let mut viewable_query = self.viewable_label_queries.p0();
            let Some((mut tf, mut thickness)) = viewable_query.log_get_mut(viewable_entity) else {
                return;
            };
            shapes::set_square_line_transform_relative(&mut tf, alpha_trimmed.0, beta_trimmed.0);
            thickness.0 = conf.segment_thickness;
        } else {
            self.commands.spawn((
                IsViewableOf(segment_entity),
                self.shapes.line_from_to(
                    conf.segment_thickness,
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
            let Some(mut tf) = label_query.log_get_mut(label_entity) else { return };
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
                            &conf.taxiway,
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
                            &conf.apron,
                            self.materials.apron_label.clone().expect("initialized at startup"),
                        ),
                        vis::ApronLabelMarker,
                    ));
                }
            }
        }
    }
}

/// Components shared between taxiway and apron labels.
fn common_label_bundle(
    segment_entity: Entity,
    name: &str,
    alpha: Position<Vec2>,
    beta: Position<Vec2>,
    conf: &SegmentTypeConfRead,
    material: Handle<ColorMaterial>,
) -> impl Bundle {
    (
        IsLabelOf(segment_entity),
        Text2d(name.to_string()),
        billboard::Label {
            offset:   alpha.midpoint(beta) - Position::ORIGIN,
            distance: conf.label_distance,
        },
        billboard::MaintainRotation,
        billboard::MaintainScale { size: conf.label_size },
        conf.label_anchor,
        MeshMaterial2d(material.clone()),
    )
}

fn trim_segment(
    a: Position<Vec2>,
    b: Position<Vec2>,
    length: Length<f32>,
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
