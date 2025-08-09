use std::time::SystemTime;

use math::{Angle, Heading};

pub mod billboard;
pub mod shapes;

macro_rules! new_type_id {
    () => {{
        struct Anonymous;
        bevy_egui::egui::Id::new(std::any::TypeId::of::<Anonymous>())
    }};
}
pub(crate) use new_type_id;

#[cfg(target_family = "wasm")]
pub fn time_now() -> SystemTime {
    SystemTime::UNIX_EPOCH + std::time::Duration::from_secs_f64(js_sys::Date::now() * 1e-3)
}

#[cfg(not(target_family = "wasm"))]
pub fn time_now() -> SystemTime { SystemTime::now() }

pub fn heading_to_approx_name(heading: Heading) -> &'static str {
    let dirs = [
        ("north", Heading::NORTH),
        ("east", Heading::EAST),
        ("south", Heading::SOUTH),
        ("west", Heading::WEST),
        ("northeast", Heading::NORTH + Angle::RIGHT / 2.),
        ("southeast", Heading::EAST + Angle::RIGHT / 2.),
        ("southwest", Heading::SOUTH + Angle::RIGHT / 2.),
        ("northwest", Heading::WEST + Angle::RIGHT / 2.),
    ];
    for (name, dir) in dirs {
        if heading.closest_distance(dir).abs() <= Angle::RIGHT / 4. {
            return name;
        }
    }

    unreachable!("Heading must be within 22.5\u{b0} of one of the 8 directions")
}

mod anchor;
pub use anchor::AnchorConf;
