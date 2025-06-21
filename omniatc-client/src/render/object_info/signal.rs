
use bevy::ecs::query::QueryData;
use bevy::ecs::system::{Query, SystemParam};
use bevy_egui::egui;
use omniatc::level::navaid::{self, Navaid};
use omniatc::level::waypoint::Waypoint;
use omniatc::try_log;

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
            let (navaid, waypoint_ref) = try_log!(
                params.navaid_query.get(navaid_id),
                expect "navaid::ObjectUsageList should reference valid navaids"
                or continue
            );
            let waypoint = try_log!(
                params.waypoint_query.get(waypoint_ref.0),
                expect "OwnerWaypoint should reference a valid waypoint"
                or continue
            );

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
