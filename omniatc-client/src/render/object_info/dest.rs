use bevy::ecs::query::QueryData;
use bevy::ecs::system::{Query, SystemParam};
use bevy_egui::egui;
use omniatc::level::aerodrome::Aerodrome;
use omniatc::level::object;
use omniatc::level::waypoint::Waypoint;
use omniatc::try_log_return;

use super::Writer;

#[derive(QueryData)]
pub struct ObjectQuery {
    dest: &'static object::Destination,
}

#[derive(SystemParam)]
pub struct WriteParams<'w, 's> {
    aerodrome: Query<'w, 's, &'static Aerodrome>,
    waypoint:  Query<'w, 's, &'static Waypoint>,
}

impl Writer for ObjectQuery {
    type SystemParams<'w, 's> = WriteParams<'w, 's>;

    fn title() -> &'static str { "Destination" }

    fn should_show(_this: &Self::Item<'_>) -> bool { true }

    fn show(this: &Self::Item<'_>, ui: &mut egui::Ui, params: &mut Self::SystemParams<'_, '_>) {
        ui.label(match *this.dest {
            object::Destination::Landing { aerodrome } => {
                let data = try_log_return!(
                    params.aerodrome.get(aerodrome),
                    expect "Unknown aerodrome {aerodrome:?}"
                );
                format!("Arrival at {}", &data.name)
            }
            object::Destination::VacateAnyRunway => String::from("Land at any runway and vacate"),
            object::Destination::ReachWaypoint { min_altitude, waypoint_proximity } => {
                let mut waypoint_name = None;
                if let Some((waypoint_entity, _)) = waypoint_proximity {
                    if let Ok(data) = params.waypoint.get(waypoint_entity) {
                        waypoint_name = Some(&data.name);
                    }
                }

                match (min_altitude, waypoint_name) {
                    (Some(altitude), Some(waypoint)) => {
                        format!(
                            "Reach {waypoint:?} and climb past {:.0}ft",
                            altitude.amsl().into_feet()
                        )
                    }
                    (Some(altitude), None) => {
                        format!("Climb past {:.0}ft", altitude.amsl().into_feet())
                    }
                    (None, Some(waypoint)) => format!("Reach {waypoint:?}"),
                    (None, None) => String::from("None"),
                }
            }
        });
    }
}
