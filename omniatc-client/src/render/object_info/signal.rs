use bevy::ecs::query::QueryData;
use bevy::ecs::system::{Query, SystemParam};
use bevy_egui::egui;
use omniatc::QueryTryLog;
use omniatc::level::navaid::{self, Navaid};
use omniatc::level::waypoint::Waypoint;

use super::Writer;

#[derive(QueryData)]
pub struct ObjectQuery {
    navaids: Option<&'static navaid::ObjectUsageList>,
}

#[derive(SystemParam)]
pub struct WriteRouteParams<'w, 's> {
    waypoint_query: Query<'w, 's, &'static Waypoint>,
    navaid_query:   Query<'w, 's, (&'static Navaid, &'static navaid::OwnerWaypoint)>,
}

impl Writer for ObjectQuery {
    type SystemParams<'w, 's> = WriteRouteParams<'w, 's>;

    fn title() -> &'static str { "Signal" }

    fn should_show(_this: &Self::Item<'_>) -> bool { true }

    fn show(this: &Self::Item<'_>, ui: &mut egui::Ui, params: &mut Self::SystemParams<'_, '_>) {
        for &navaid_id in this.navaids.iter().flat_map(|v| &v.0) {
            let Some((navaid, waypoint_ref)) = params.navaid_query.log_get(navaid_id) else {
                continue;
            };
            let Some(waypoint) = params.waypoint_query.log_get(waypoint_ref.0) else { continue };

            ui.label(match navaid.kind {
                navaid::Kind::Visual => format!("{} visual contact", &waypoint.name),
                navaid::Kind::Localizer => format!("{} ILS available", &waypoint.name),
                navaid::Kind::Vor => format!("{} VOR available", &waypoint.name),
                navaid::Kind::Dme => format!("{} DME available", &waypoint.name),
                navaid::Kind::Gnss => "GNSS usable".into(),
            });
        }
    }
}
