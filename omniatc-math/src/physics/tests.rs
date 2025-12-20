use crate::{
    Barometrics, ISA_SEA_LEVEL_PRESSURE, ISA_SEA_LEVEL_TEMPERATURE, ISA_TROPOPAUSE_PRESSURE,
    ISA_TROPOPAUSE_TEMPERATURE, Length, Position, Pressure, Speed, TROPOPAUSE_ALTITUDE, Temp,
    TempDelta, compute_barometric,
};

#[test]
fn test_barometric_isa_sea_level() {
    assert_barometric(
        compute_barometric(
            Position::from_amsl_feet(0.0),
            ISA_SEA_LEVEL_PRESSURE,
            ISA_SEA_LEVEL_TEMPERATURE,
        ),
        AssertBarometrics {
            pressure:          ISA_SEA_LEVEL_PRESSURE,
            pressure_altitude: Position::from_amsl_feet(0.0),
            temperature:       ISA_SEA_LEVEL_TEMPERATURE,
            tas_for_200_kias:  Speed::from_knots(200.0),
        },
    );
}

#[test]
fn test_barometric_isa_tropopause() {
    assert_barometric(
        compute_barometric(TROPOPAUSE_ALTITUDE, ISA_SEA_LEVEL_PRESSURE, ISA_SEA_LEVEL_TEMPERATURE),
        AssertBarometrics {
            pressure:          ISA_TROPOPAUSE_PRESSURE,
            pressure_altitude: TROPOPAUSE_ALTITUDE,
            temperature:       ISA_TROPOPAUSE_TEMPERATURE,
            tas_for_200_kias:  Speed::from_knots(366.9),
        },
    );
}

#[test]
fn test_barometric_isa_fl100() {
    assert_barometric(
        compute_barometric(
            Position::from_amsl_feet(10000.0),
            ISA_SEA_LEVEL_PRESSURE,
            ISA_SEA_LEVEL_TEMPERATURE,
        ),
        AssertBarometrics {
            pressure:          Pressure::from_pascals(264361.0),
            pressure_altitude: Position::from_amsl_feet(10000.0),
            temperature:       Temp::from_celsius(-4.8),
            tas_for_200_kias:  Speed::from_knots(232.7),
        },
    );
}

#[test]
fn test_barometric_isa_fl430() {
    assert_barometric(
        compute_barometric(
            Position::from_amsl_feet(43000.0),
            ISA_SEA_LEVEL_PRESSURE,
            ISA_SEA_LEVEL_TEMPERATURE,
        ),
        AssertBarometrics {
            pressure:          Pressure::from_pascals(19115.0),
            pressure_altitude: Position::from_amsl_feet(43000.0),
            temperature:       Temp::from_celsius(-56.5),
            tas_for_200_kias:  Speed::from_knots(433.2),
        },
    );
}

#[test]
fn test_barometric_low_pressure() {
    assert_barometric(
        compute_barometric(
            Position::from_amsl_feet(10000.0),
            Pressure::from_pascals(98000.0),
            ISA_SEA_LEVEL_TEMPERATURE,
        ),
        AssertBarometrics {
            pressure:          Pressure::from_pascals(257634.0),
            // low pressure => pressure altitude is higher than true altitude
            pressure_altitude: Position::from_amsl_feet(10876.0),
            temperature:       Temp::from_celsius(-4.8),
            tas_for_200_kias:  Speed::from_knots(236.6),
        },
    );
}

#[test]
fn test_barometric_high_pressure() {
    assert_barometric(
        compute_barometric(
            Position::from_amsl_feet(10000.0),
            Pressure::from_pascals(103000.0),
            ISA_SEA_LEVEL_TEMPERATURE,
        ),
        AssertBarometrics {
            pressure:          Pressure::from_pascals(271089.0),
            // high pressure => pressure altitude is lower than true altitude
            pressure_altitude: Position::from_amsl_feet(9123.0),
            temperature:       Temp::from_celsius(-4.8),
            tas_for_200_kias:  Speed::from_knots(230.8),
        },
    );
}

#[test]
fn test_barometric_low_temp() {
    assert_barometric(
        compute_barometric(
            Position::from_amsl_feet(10000.0),
            ISA_SEA_LEVEL_PRESSURE,
            Temp::from_celsius(5.0),
        ),
        AssertBarometrics {
            pressure:          Pressure::from_pascals(271089.0),
            pressure_altitude: Position::from_amsl_feet(9123.0),
            temperature:       Temp::from_celsius(-14.8),
            // lower temperature => higher air density => lower TAS
            tas_for_200_kias:  Speed::from_knots(230.0),
        },
    );
}

#[test]
fn test_barometric_high_temp() {
    assert_barometric(
        compute_barometric(
            Position::from_amsl_feet(10000.0),
            ISA_SEA_LEVEL_PRESSURE,
            Temp::from_celsius(25.0),
        ),
        AssertBarometrics {
            pressure:          Pressure::from_pascals(271089.0),
            pressure_altitude: Position::from_amsl_feet(9123.0),
            temperature:       Temp::from_celsius(5.2),
            // higher temperature => lower air density => higher TAS
            tas_for_200_kias:  Speed::from_knots(235.5),
        },
    );
}

struct AssertBarometrics {
    pressure:          Pressure,
    pressure_altitude: Position<f32>,
    temperature:       Temp,
    tas_for_200_kias:  Speed<f32>,
}

fn assert_barometric(actual: Barometrics, expect: AssertBarometrics) {
    actual.pressure.assert_approx(actual.pressure, Pressure::from_pascals(1.0)).unwrap();
    actual.pressure_altitude.assert_near(actual.pressure_altitude, Length::from_feet(1.0)).unwrap();
    actual.temp.assert_approx(expect.temperature, TempDelta::from_kelvins(0.1)).unwrap();
    actual
        .true_airspeed(Speed::from_knots(200.0))
        .assert_approx(expect.tas_for_200_kias, Speed::from_knots(0.1))
        .unwrap();
}
