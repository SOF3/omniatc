use bevy::app::App;
use bevy::ecs::system::SystemState;
use math::{Distance, Position, Speed};

use super::GroundSpeedCalculator;
use crate::level::object::RefAltitudeType;

#[test]
fn test_estimate_altitude_change() {
    let mut app = App::new();
    let world = app.world_mut();
    let mut state = SystemState::<GroundSpeedCalculator>::new(world);

    let result = state.get(world).estimate_altitude_change(
        [Position::from_origin_nm(0., 0.), Position::from_origin_nm(0., 10.)],
        Speed::from_fpm(-600.), // 600ft per 60 seconds, 1000ft per 100 seconds
        Speed::from_knots(360.), // TAS is higher, >0.1nm per second, <100 seconds for 10nm
        Position::from_amsl_feet(0.),
        RefAltitudeType::End,
        Distance::from_nm(1.),
    );

    assert!(result.amsl() < Distance::from_feet(1000.)); // <1000ft over <100 seconds
    assert!(result.amsl() > Distance::from_feet(990.));
    // actual value is somewhere near 993 ft, but we are using an approximation function here.
}
