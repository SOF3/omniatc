use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EventReader;
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, Query, Res, SystemParam};
use bevy::math::Vec2;
use bevy::sprite::MeshMaterial2d;
use bevy::transform::components::Transform;
use either::Either;
use itertools::Itertools;
use omniatc::level::ground;
use omniatc::units::{Angle, Distance, Position};
use omniatc::util::manage_entity_vec;
use omniatc::{try_log, try_log_return};

use super::{vis, Conf};
use crate::config;
use crate::render::twodim::Zorder;
use crate::util::shapes;

#[cfg(test)]
mod tests;

pub(super) fn regenerate_system(
    mut events: EventReader<ground::EndpointChangedEvent>,
    mut params: RegenerateParam,
) {
    for &ground::EndpointChangedEvent(endpoint_entity) in events.read() {
        params.regenerate(endpoint_entity);
    }
}

#[derive(SystemParam)]
pub(super) struct RegenerateParam<'w, 's> {
    commands:            Commands<'w, 's>,
    endpoint_query:      Query<'w, 's, (&'static ground::Endpoint, Option<&'static TurnCurveList>)>,
    endpoint_peer_query: Query<'w, 's, &'static ground::Endpoint>,
    segment_query:       Query<'w, 's, &'static ground::Segment>,
    curve_query:         Query<'w, 's, &'static mut Transform, With<TurnCurveOf>>,
    shapes:              Res<'w, shapes::Meshes>,
    conf:                config::Read<'w, 's, Conf>,
    materials:           Res<'w, super::ColorMaterials>,
    viewable_query:      Query<'w, 's, ()>,
}

impl RegenerateParam<'_, '_> {
    fn regenerate(&mut self, endpoint_entity: Entity) {
        let (&ground::Endpoint { position: intersect, ref adjacency }, curves) = try_log_return!(
            self.endpoint_query.get(endpoint_entity),
            expect "{endpoint_entity:?} is not a endpoint entity"
        );

        let all_curve_parts = adjacency.iter().array_combinations().filter_map(|segment_pair| {
            let [Some(other_pos1), Some(other_pos2)] = segment_pair.map(|&segment_entity| {
                let segment = try_log!(
                    self.segment_query.get(segment_entity),
                    expect "endpoint adjacency list must contain segment entity: {segment_entity:?}"
                    or return None
                );
                let other_endpoint = try_log!(
                    segment.other_endpoint(endpoint_entity),
                    expect "segment {segment_entity:?} is an adjacency of {endpoint_entity:?} and must reference it back"
                    or return None
                );
                let &ground::Endpoint{position:other_position,..} = try_log!(
                    self.endpoint_peer_query.get(other_endpoint),
                    expect "segment endpoint {other_endpoint:?} must be another endpoint entity"
                    or return None
                );
                Some(other_position)
            }) else { return None };

            Some(compute_curve_points(
                intersect,
                [other_pos1, other_pos2],
                self.conf.intersection_size,
                self.conf.arc_interval,
            ).tuple_windows::<(_, _)>())
        }).flatten();

        let Self { ref shapes, ref conf, ref materials, ref mut curve_query, .. } = *self;
        manage_entity_vec(
            endpoint_entity,
            curves,
            &mut (curve_query, all_curve_parts),
            |_, (_, curve_parts)| {
                let (start, end) = curve_parts.next()?;
                Some((
                    vis::EndpointMarker,
                    shapes.line_from_to(
                        conf.segment_thickness,
                        Zorder::GroundSegmentCenterline,
                        start,
                        end,
                    ),
                    MeshMaterial2d(materials.taxiway.clone().expect("initialized at startup")),
                ))
            },
            |_, (curve_query, curve_parts), curve_entity| {
                let Some((start, end)) = curve_parts.next() else { return Err(()) };

                let mut curve_tf = try_log!(
                    curve_query.get_mut(curve_entity),
                    expect "TurnCurveList must contain valid TurnCurveOf members"
                    or return Err(())
                );

                shapes::set_square_line_transform(&mut curve_tf, start, end);
                Ok(())
            },
            &mut self.commands,
        );
    }
}

fn compute_curve_points(
    intersect: Position<Vec2>,
    other_positions: [Position<Vec2>; 2],
    intersection_size: Distance<f32>,
    arc_interval: Angle<f32>,
) -> impl Iterator<Item = Position<Vec2>> {
    /// Base distance used for radials from the intersect center point
    const DISTANCE_UNIT: Distance<f32> = Distance(1.);

    let [radial1, radial2] =
        other_positions.map(|pos| (pos - intersect).normalize_to_magnitude(DISTANCE_UNIT));

    let dot = radial1.0.dot(radial2.0);
    if dot / DISTANCE_UNIT.magnitude_squared().0 < -0.99 {
        // almost a straight line
        Either::Left(
            [
                intersect + radial1 * (intersection_size / DISTANCE_UNIT),
                intersect + radial2 * (intersection_size / DISTANCE_UNIT),
            ]
            .into_iter(),
        )
    } else {
        // (turn_center_radial - radial1) is orthogonal to radial1, vice versa.
        // 2 / (1 + u.v) is the magnitude of the vector that satisfies this property.
        let turn_center_radial =
            (radial1 + radial2).normalize_to_magnitude(DISTANCE_UNIT * (2. / (1. + dot)).sqrt());

        let turn_radius_scaled = (radial1 - turn_center_radial).magnitude_exact();

        let heading1 = (radial1 - turn_center_radial).heading();
        let heading2 = (radial2 - turn_center_radial).heading();
        let turn_dir = heading1.closest_distance(heading2);

        let num_curves_float = (turn_dir / arc_interval).abs().ceil();
        #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        // ceil(abs(x)) for a small value is exactly equal for f32 and usize
        let num_curves = num_curves_float as usize;

        let curve_points = (0..=num_curves).map(move |curve_number| {
            #[expect(clippy::cast_precision_loss)] // curve_number is a small value
            let curve_number_float = curve_number as f32;

            let heading = if curve_number == num_curves {
                heading2
            } else {
                heading1 + turn_dir / num_curves_float * curve_number_float
            };

            let pos = turn_center_radial + turn_radius_scaled.with_heading(heading);

            // `pos` is the position of the curve point relative to `intersect`
            // when `intersection_size` is equal to `DISTANCE_UNIT`.
            intersect + pos * (intersection_size / DISTANCE_UNIT)
        });
        Either::Right(curve_points)
    }
}

#[derive(Component)]
#[relationship(relationship_target = TurnCurveList)]
struct TurnCurveOf(Entity);

#[derive(Component)]
#[relationship_target(relationship = TurnCurveOf, linked_spawn)]
struct TurnCurveList(Vec<Entity>);
