use bevy_math::{Dir2, Vec2};

use super::{Line, LineIndex, LineIntersection, LineSweeper};
use crate::{Distance, Position};

#[test]
fn sweep_cross_aligned() {
    let intersects: Vec<_> = LineSweeper::new(
        |index| match index {
            LineIndex(0) => Line {
                alpha:          Position::from_origin_nm(-1., 0.),
                beta:           Position::from_origin_nm(1., 0.),
                need_intersect: true,
            },
            LineIndex(1) => Line {
                alpha:          Position::from_origin_nm(0.5, -1.),
                beta:           Position::from_origin_nm(0.5, 1.),
                need_intersect: true,
            },
            _ => unreachable!(),
        },
        2,
        Distance(0.0001),
        Dir2::EAST,
    )
    .unwrap()
    .intersections()
    .collect();
    assert_eq!(intersects.len(), 1);
    assert!((intersects[0].dot.into_inner() - 0.5).abs() < 0.0001);
    assert!(
        intersects[0].position.distance_cmp(Position::from_origin_nm(0.5, 0.))
            < Distance::from_nm(0.0001)
    );
    assert_eq!(intersects[0].lines, [LineIndex(0), LineIndex(1)]);
}

macro_rules! sweep_quad_complete_graph {
    ($($name:ident($x:expr, $y:expr) $({
        $($($point:ident),* => $point_dot:expr;)*
    })?;)*) => {$(
        paste::paste! {
            #[test]
            fn [< sweep_quad_complete_graph_ $name >]() {
                #[allow(unused_assignments, unused_mut)]
                let mut expect_dot = None::<fn(TestPoint) -> f32>;
                $(
                    expect_dot = Some(|point| match point {
                        $($(TestPoint::$point)|* => $point_dot,)*
                    });
                )?
                sweep_quad_complete_graph(Dir2::new(Vec2::new($x, $y)).unwrap(), expect_dot);
            }

            #[test]
            fn [< sweep_quad_complete_graph_ $name _merged >]() {
                sweep_quad_complete_graph_merged(Dir2::new(Vec2::new($x, $y)).unwrap());
            }
        }
    )*}
}

sweep_quad_complete_graph! {
    east(1., 0.) {
        West => -1.;
        Center, South, North => 0.;
        East => 1.;
    };
    southeast(1., -1.) {
        West, North => -(0.5f32.sqrt());
        Center => 0.;
        South, East => 0.5f32.sqrt();
    };
    south(0., -1.) {
        North => -1.;
        Center, East, West => 0.;
        South => 1.;
    };
    southwest(-1., -1.) {
        East, North => -(0.5f32.sqrt());
        Center => 0.;
        South, West => 0.5f32.sqrt();
    };
    west(-1., 0.) {
        East => -1.;
        Center, South, North => 0.;
        West => 1.;
    };
    northwest(-1., 1.) {
        East, South => -(0.5f32.sqrt());
        Center => 0.;
        North, West => 0.5f32.sqrt();
    };
    north(0., 1.) {
        South => -1.;
        Center, East, West => 0.;
        North => 1.;
    };
    northeast(1., 1.) {
        West, South => -(0.5f32.sqrt());
        Center => 0.;
        North, East => 0.5f32.sqrt();
    };
    irrational(std::f32::consts::E, 1.);
}

fn quad_complete_graph_sweeper(sweep_dir: Dir2) -> LineSweeper<impl Fn(LineIndex) -> Line> {
    let points = [
        Position::from_origin_nm(-1., 0.), // west
        Position::from_origin_nm(1., 0.),  // east
        Position::from_origin_nm(0., -1.), // south
        Position::from_origin_nm(0., 1.),  // north
    ];
    let lines: Vec<Line> = (0..4)
        .flat_map(|i| {
            ((i + 1)..4).map(move |j| Line {
                alpha:          points[i],
                beta:           points[j],
                need_intersect: true,
            })
        })
        .collect();

    let lines_len = lines.len();
    LineSweeper::new(move |LineIndex(index)| lines[index], lines_len, Distance(0.0001), sweep_dir)
        .unwrap()
}

fn sweep_quad_complete_graph(sweep_dir: Dir2, expect_dot: Option<fn(TestPoint) -> f32>) {
    let mut intersects: Vec<_> = quad_complete_graph_sweeper(sweep_dir).intersections().collect();
    intersects.sort_by_key(|intersect| sorted(intersect.lines));

    assert_eq!(intersects.len(), 13);

    assert_intersection(
        &intersects[0],
        [0, 1],
        TestPoint::West,
        expect_dot,
        "west-east * west-south",
    );
    assert_intersection(
        &intersects[1],
        [0, 2],
        TestPoint::West,
        expect_dot,
        "west-east * west-north",
    );
    assert_intersection(
        &intersects[2],
        [0, 3],
        TestPoint::East,
        expect_dot,
        "west-east * east-south",
    );
    assert_intersection(
        &intersects[3],
        [0, 4],
        TestPoint::East,
        expect_dot,
        "west-east * east-north",
    );
    assert_intersection(
        &intersects[4],
        [0, 5],
        TestPoint::Center,
        expect_dot,
        "west-east * south-north",
    );
    assert_intersection(
        &intersects[5],
        [1, 2],
        TestPoint::West,
        expect_dot,
        "west-south * west-north",
    );
    assert_intersection(
        &intersects[6],
        [1, 3],
        TestPoint::South,
        expect_dot,
        "west-south * east-south",
    );
    assert_intersection(
        &intersects[7],
        [1, 5],
        TestPoint::South,
        expect_dot,
        "west-south * south-north",
    );
    assert_intersection(
        &intersects[8],
        [2, 4],
        TestPoint::North,
        expect_dot,
        "west-north * east-north",
    );
    assert_intersection(
        &intersects[9],
        [2, 5],
        TestPoint::North,
        expect_dot,
        "west-north * south-north",
    );
    assert_intersection(
        &intersects[10],
        [3, 4],
        TestPoint::East,
        expect_dot,
        "east-south * east-north",
    );
    assert_intersection(
        &intersects[11],
        [3, 5],
        TestPoint::South,
        expect_dot,
        "east-south * south-north",
    );
    assert_intersection(
        &intersects[12],
        [4, 5],
        TestPoint::North,
        expect_dot,
        "east-north * south-north",
    );
}

fn sorted<const N: usize, T: Ord>(mut array: [T; N]) -> [T; N] {
    array.sort();
    array
}

#[derive(Clone, Copy)]
enum TestPoint {
    West,
    East,
    South,
    North,
    Center,
}

fn assert_intersection(
    actual: &LineIntersection,
    expect_lines: [usize; 2],
    expect_point: TestPoint,
    expect_dot: Option<fn(TestPoint) -> f32>,
    message: &str,
) {
    assert_eq!(sorted(actual.lines), expect_lines.map(LineIndex), "{message} lines mismatch");

    let actual_point_round = <[f32; 2]>::from((actual.position.get() * 10000.).round() / 10000.);
    #[expect(clippy::float_cmp)] // comparison of rounded floats
    {
        assert_eq!(
            actual_point_round,
            match expect_point {
                TestPoint::West => [-1., 0.],
                TestPoint::East => [1., 0.],
                TestPoint::South => [0., -1.],
                TestPoint::North => [0., 1.],
                TestPoint::Center => [0., 0.],
            },
            "{message} position mismatch"
        );
    }

    if let Some(expect_dot) = expect_dot {
        assert!(
            (actual.dot.into_inner() - expect_dot(expect_point)).abs() < 1e-4,
            "{message} dot mismatch: {:?} != {:?}",
            actual.dot,
            expect_dot(expect_point)
        );
    }
}

fn sweep_quad_complete_graph_merged(sweep_dir: Dir2) {
    let groups: Vec<_> = quad_complete_graph_sweeper(sweep_dir).intersections_merged().collect();

    assert_groups(
        groups,
        [
            ExpectGroup { len: 1, position: Position::from_origin_nm(0., 0.) },
            ExpectGroup { len: 3, position: Position::from_origin_nm(0., -1.) },
            ExpectGroup { len: 3, position: Position::from_origin_nm(0., 1.) },
            ExpectGroup { len: 3, position: Position::from_origin_nm(-1., 0.) },
            ExpectGroup { len: 3, position: Position::from_origin_nm(1., 0.) },
        ],
        Distance::from_nm(0.0001),
    );
}

#[derive(Debug)]
struct ExpectGroup {
    len:      usize,
    position: Position<Vec2>,
}

impl ExpectGroup {
    fn matches(&self, group: &[LineIntersection], epsilon: Distance<f32>) -> bool {
        group.len() == self.len
            && group.iter().all(|item| item.position.distance_cmp(self.position) < epsilon)
    }
}

fn assert_groups(
    groups: Vec<Vec<LineIntersection>>,
    expect_groups: impl IntoIterator<Item = ExpectGroup>,
    epsilon: Distance<f32>,
) {
    let mut expect_groups: Vec<_> = expect_groups.into_iter().collect();

    assert_eq!(groups.len(), expect_groups.len());
    'groups: for group in groups {
        for (i, expect) in expect_groups.iter().enumerate() {
            if expect.matches(&group, epsilon) {
                expect_groups.swap_remove(i);
                continue 'groups;
            }
        }

        panic!("Unexpected group {group:?}, expected one of {expect_groups:?}");
    }
}
