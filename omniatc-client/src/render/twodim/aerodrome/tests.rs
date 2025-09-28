use std::io::Cursor;

use bevy::app::App;
use bevy::ecs::system::{EntityCommand, SystemState};
use bevy::math::Vec2;
use bevy_mod_config::{AppExt, ReadConfig, manager};
use math::{Heading, Length, Position, Speed};
use omniatc::level::aerodrome::Aerodrome;
use omniatc::level::ground;

const REF_POS: Position<Vec2> = Position::from_origin_nm(0.0, 0.0);
// An arbitrary small unit at the same magnitude as one meter for more readable debug output.
const UNIT: Length<f32> = Length::from_nm(0.001);
const WIDTH: Length<f32> = Length::from_nm(0.025);
const HALF_WIDTH: Length<f32> = Length::from_nm(0.0125);
const ELEVATION: Position<f32> = Position::new(0.1);

const UNITS_PER_HALF_WIDTH: f32 = 0.0125 / 0.001;
fn pos(x: f32, y: f32) -> Position<Vec2> { REF_POS + Length::from_components(UNIT * x, UNIT * y) }

#[test]
fn push_endpoint_aligned_right_angle() {
    test_push_endpoint(
        &[pos(0.0, 0.0), pos(100.0, 0.0), pos(0.0, 100.0)],
        &[(0, 1, "A"), (0, 2, "B")],
        HALF_WIDTH,
        [
            LineSegment {
                start: pos(UNITS_PER_HALF_WIDTH, 0.0),
                end:   pos(UNITS_PER_HALF_WIDTH, UNITS_PER_HALF_WIDTH)
                    + HALF_WIDTH * Heading::SOUTHWEST,
                width: WIDTH,
            },
            LineSegment {
                start: pos(UNITS_PER_HALF_WIDTH, UNITS_PER_HALF_WIDTH)
                    + HALF_WIDTH * Heading::SOUTHWEST,
                end:   pos(0.0, UNITS_PER_HALF_WIDTH),
                width: WIDTH,
            },
        ],
    );
}

fn test_push_endpoint(
    endpoints: &[Position<Vec2>],
    segments: &[(usize, usize, &'static str)],
    curve_segment_length: Length<f32>,
    expect: impl IntoIterator<Item = LineSegment>,
) {
    let mut app = App::new();

    app.init_config::<manager::serde::Json, super::Conf>("conf");
    let manager =
        app.world().resource::<manager::Instance<manager::serde::Json>>().instance.clone();
    let conf_json = format!(
        r#"{{
        "conf.curve_segment_length": {curve_segment_length}
    }}"#,
        curve_segment_length = curve_segment_length.0
    );
    manager.from_reader(app.world_mut(), Cursor::new(conf_json.into_bytes())).unwrap();

    let endpoint_ids: Vec<_>;

    {
        let world = app.world_mut();
        let aerodrome = world
            .spawn((Aerodrome {
                id:        0,
                code:      String::new(),
                name:      String::new(),
                elevation: ELEVATION,
            },))
            .id();

        endpoint_ids = endpoints
            .iter()
            .map(|&position| {
                let id = world.spawn_empty().id();
                ground::SpawnEndpoint { position, aerodrome }.apply(world.entity_mut(id));
                id
            })
            .collect();

        for &(alpha, beta, label) in segments {
            let id = world.spawn_empty().id();
            ground::SpawnSegment {
                segment: ground::Segment {
                    alpha:     endpoint_ids[alpha],
                    beta:      endpoint_ids[beta],
                    width:     WIDTH,
                    max_speed: Speed::from_knots(40.0),
                    elevation: ELEVATION,
                },
                label: ground::SegmentLabel::Taxiway { name: label.into() },
                aerodrome,
                display_label: false,
            }
            .apply(world.entity_mut(id));
        }
    }

    let output = {
        let mut state = SystemState::<(super::RegenerateLinesParam, ReadConfig<super::Conf>)>::new(
            app.world_mut(),
        );
        let (param, conf) = state.get_mut(app.world_mut());
        let mut output = MockDrawLineSegment(Vec::new());

        for endpoint in endpoint_ids {
            param.push_endpoint(endpoint, &mut output, |_, _| WIDTH, &conf.read());
        }

        output.0
    };

    let expect = expect.into_iter().collect::<Vec<_>>();
    assert_eq!(output.len(), expect.len(), "output and expect differ in length");

    let mut fail = false;
    for (index, (actual_segment, expect_segment)) in output.iter().zip(expect.iter()).enumerate() {
        if actual_segment.start.distance_cmp(expect_segment.start) > UNIT * 0.01 {
            fail = true;
            eprintln!(
                "segment {index} start: expected {:?}, got {:?}",
                expect_segment.start, actual_segment.start,
            );
        }

        if actual_segment.end.distance_cmp(expect_segment.end) > UNIT * 0.01 {
            fail = true;
            eprintln!(
                "segment {index} end: expected {:?}, got {:?}",
                expect_segment.end, actual_segment.end,
            );
        }

        if (actual_segment.width - expect_segment.width).abs() > UNIT * 0.01 {
            fail = true;
            eprintln!(
                "segment {index} width: expected {:?}, got {:?}",
                expect_segment.width, actual_segment.width,
            );
        }
    }
    assert!(!fail, "output and expect differ");
}

struct MockDrawLineSegment(Vec<LineSegment>);

struct LineSegment {
    start: Position<Vec2>,
    end:   Position<Vec2>,
    width: Length<f32>,
}

impl super::DrawLineSegment for MockDrawLineSegment {
    fn draw_segment_trunc(
        &mut self,
        start: Position<Vec2>,
        _: bool,
        end: Position<Vec2>,
        _: bool,
        width: Length<f32>,
    ) {
        self.0.push(LineSegment { start, end, width });
    }
}
