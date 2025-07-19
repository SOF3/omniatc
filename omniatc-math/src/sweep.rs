use std::collections::{BinaryHeap, HashMap};
use std::{cmp, iter, mem};

use bevy_math::{Dir2, Vec2};
use ordered_float::NotNan;

use crate::{line_intersect, Length, Position};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy)]
pub struct Line {
    pub alpha:          Position<Vec2>,
    pub beta:           Position<Vec2>,
    /// Intersections are only computed for a pair of lines
    /// when `need_intersect` is true on *either* line.
    pub need_intersect: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LineIndex(pub usize);

#[derive(Clone)]
pub struct LineSweeper<F> {
    line_fn:   F,
    num_lines: usize,
    /// Intersections within this distance from the end of line segments
    /// are considered on the lines.
    /// This is to prevent numerical issues when line segments only intersect at their ends.
    epsilon:   Length<f32>,
    sweep_dir: Dir2,
    ortho_dir: Dir2,

    starts: Vec<StartEntry>,
    ends:   Vec<EndEntry>,
}

impl<F> LineSweeper<F>
where
    F: Fn(LineIndex) -> Line,
{
    /// Creates a new sweeper for iterating over intersections at the given direction.
    ///
    /// This function involves internally building an index of `lines` along `sweep_dir`,
    /// which may be an expensive operation if `lines.len()` is large.
    /// Consider cloning the result if multiple calls to this function have the same parameters.
    ///
    /// # Errors
    /// Returns an error if inputs result in non-finite values.
    ///
    /// # Panics
    /// Panics if epsilon is not a finite value.
    pub fn new(
        line_fn: F,
        num_lines: usize,
        epsilon: Length<f32>,
        sweep_dir: Dir2,
    ) -> Result<Self, Error> {
        // rotate sweep_dir clockwise by 90 degrees
        let ortho_dir = Dir2::new_unchecked(Vec2::new(sweep_dir.y, -sweep_dir.x));

        let (mut starts, mut ends) = (0..num_lines)
            .map(|line_index| {
                let line_index = LineIndex(line_index);
                let line = line_fn(line_index);

                let alpha_dot = NotNan::new(line.alpha.get().dot(*sweep_dir))?;
                let beta_dot = NotNan::new(line.beta.get().dot(*sweep_dir))?;
                let alpha_ortho = NotNan::new(line.alpha.get().dot(*ortho_dir))?;
                let beta_ortho = NotNan::new(line.beta.get().dot(*ortho_dir))?;

                let (mut start, mut end) = if alpha_dot < beta_dot {
                    (
                        StartEntry {
                            line_index,
                            start_pos: line.alpha,
                            start_dot_minus_epsilon: alpha_dot,
                            end_dot_plus_epsilon: beta_dot,
                            start_ortho: alpha_ortho,
                            end_ortho: beta_ortho,
                        },
                        EndEntry { line_index, end_pos: line.beta, end_dot_plus_epsilon: beta_dot },
                    )
                } else {
                    (
                        StartEntry {
                            line_index,
                            start_pos: line.beta,
                            start_dot_minus_epsilon: beta_dot,
                            end_dot_plus_epsilon: alpha_dot,
                            start_ortho: beta_ortho,
                            end_ortho: alpha_ortho,
                        },
                        EndEntry {
                            line_index,
                            end_pos: line.alpha,
                            end_dot_plus_epsilon: alpha_dot,
                        },
                    )
                };

                {
                    let epsilon = NotNan::new(epsilon.0).expect("epsilon must not be nan");
                    start.start_dot_minus_epsilon -= epsilon;
                    start.end_dot_plus_epsilon += epsilon;
                    end.end_dot_plus_epsilon += epsilon;
                }

                Ok((start, end))
            })
            .collect::<Result<(Vec<_>, Vec<_>), Error>>()?;

        starts.sort_by_key(|entry| entry.start_dot_minus_epsilon);
        ends.sort_by_key(|entry| entry.end_dot_plus_epsilon);

        Ok(Self { line_fn, num_lines, epsilon, sweep_dir, ortho_dir, starts, ends })
    }

    /// Iterate over all pairs of intersecting line segments.
    pub fn intersections(&self) -> impl Iterator<Item = LineIntersection> + use<'_, F> {
        let mut state = State {
            sweeper:         self,
            starts:          &self.starts[..],
            ends:            &self.ends[..],
            active_need:     HashMap::new(),
            active_need_not: HashMap::new(),
            intersections:   BinaryHeap::new(),
        };

        iter::from_fn(move || match state.poll() {
            Poll::Continue => Some(None),
            Poll::Yield(value) => Some(Some(value)),
            Poll::End => None,
        })
        .flatten()
    }

    /// Iterate over all pairs of intersecting line segments
    /// that occur in the sweep direction *after* `position`,
    /// excluding points that occur within the `epsilon` radius from `position`.
    pub fn intersections_after(
        &self,
        position: Position<Vec2>,
    ) -> impl Iterator<Item = LineIntersection> + use<'_, F> {
        self.intersections().skip_while(move |intersect| {
            intersect.dot.into_inner() + self.epsilon.0 < self.dot(position)
        })
    }

    /// Iterate over all groups of line segments within epsilon.
    pub fn intersections_merged(&self) -> impl Iterator<Item = Vec<LineIntersection>> + use<'_, F> {
        struct Merge<I> {
            recv:    I,
            buf:     Vec<LineIntersection>,
            epsilon: Length<f32>,
        }

        impl<I> Merge<I>
        where
            I: Iterator<Item = LineIntersection>,
        {
            fn fill_buf(&mut self) {
                loop {
                    let need_push = match &self.buf[..] {
                        [] => true,
                        // I could just handle `[first]` as a separate pattern,
                        // but @dyslexicsteak told me this is fun and not to remove it
                        [first @ last] | [first, .., last] => {
                            (last.dot - first.dot).into_inner() < self.epsilon.0
                        }
                    };

                    if need_push {
                        let Some(intersect) = self.recv.next() else { return };
                        self.buf.push(intersect);
                    } else {
                        break;
                    }
                }
            }
        }

        impl<I> Iterator for Merge<I>
        where
            I: Iterator<Item = LineIntersection>,
        {
            type Item = Vec<LineIntersection>;

            fn next(&mut self) -> Option<Self::Item> {
                self.fill_buf();

                // TODO replcae this with `extract_if` when it is stable.
                // For now we just allocate a separate Vec.

                let mut buf = mem::take(&mut self.buf).into_iter();
                let Some(seed) = buf.next() else {
                    return None; // no more intersections to return
                };

                let mut output = Vec::new();

                for intersect in buf {
                    if (intersect.dot - seed.dot).abs() < self.epsilon.0
                        && (intersect.ortho - seed.ortho).abs() < self.epsilon.0
                    {
                        output.push(intersect);
                    } else {
                        self.buf.push(intersect);
                    }
                }

                output.push(seed);
                Some(output)
            }
        }

        Merge { recv: self.intersections(), buf: Vec::new(), epsilon: (self.epsilon) }
    }

    fn dot(&self, position: Position<Vec2>) -> f32 { position.get().dot(*self.sweep_dir) }
}

impl<F> State<'_, F>
where
    F: Fn(LineIndex) -> Line,
{
    fn poll(&mut self) -> Poll {
        // Select the next event in the sweep direction to implement Bentley–Ottmann algorithm.
        match (self.starts.first(), self.ends.first(), self.intersections.peek()) {
            (None, None, None) => Poll::End,
            (None, None, Some(_)) => self.yield_intersect(),
            (None, Some(_), None) => self.yield_end(),
            (None, Some(&EndEntry { end_dot_plus_epsilon: end_dot, .. }), Some(intersect)) => {
                if intersect.dot <= end_dot {
                    self.yield_intersect()
                } else {
                    self.yield_end()
                }
            }
            (Some(_), None, None) => self.yield_start(),
            (
                Some(&StartEntry { start_dot_minus_epsilon: start_dot, .. }),
                None,
                Some(intersect),
            ) => {
                if intersect.dot <= start_dot {
                    self.yield_intersect()
                } else {
                    self.yield_start()
                }
            }
            (
                Some(&StartEntry { start_dot_minus_epsilon: start_dot, .. }),
                Some(&EndEntry { end_dot_plus_epsilon: end_dot, .. }),
                None,
            ) => {
                if end_dot <= start_dot {
                    self.yield_end()
                } else {
                    self.yield_start()
                }
            }
            (
                Some(&StartEntry { start_dot_minus_epsilon: start_dot, .. }),
                Some(&EndEntry { end_dot_plus_epsilon: end_dot, .. }),
                Some(intersect),
            ) => {
                if intersect.dot <= start_dot && intersect.dot <= end_dot {
                    self.yield_intersect()
                } else if end_dot <= start_dot {
                    self.yield_end()
                } else {
                    self.yield_start()
                }
            }
        }
    }

    fn yield_start(&mut self) -> Poll {
        /// Equal to `1f32.to_degrees().cos().powi(2)`.
        /// If the square of the dot product between two unit vectors exceeds this constant,
        /// the angle between them is less than 1 degree, which we consider as parallel.
        const COS_EPSILON_SQ: f32 = 0.9996954;

        fn almost_parallel(v1: Vec2, v2: Vec2) -> bool {
            v1.dot(v2).powi(2) / v1.length_squared() / v2.length_squared() > COS_EPSILON_SQ
        }

        let &entry =
            slice_take_first(&mut self.starts).expect("yield_start called when starts is empty");
        let epsilon = NotNan::new(self.sweeper.epsilon.0).expect("epsilon must not be nan");
        let entry_need_intersect = (self.sweeper.line_fn)(entry.line_index).need_intersect;

        for active in self.active_need.values().chain(
            entry_need_intersect.then(|| self.active_need_not.values()).into_iter().flatten(),
        ) {
            let mut intersect = None;
            if almost_parallel(active.dot_ortho_dir(epsilon), entry.dot_ortho_dir(epsilon)) {
                // We assume the segments cannot overlap, so just check for the endpoint equality
                for active_v in [active.start_dot_ortho(epsilon), active.end_dot_ortho(epsilon)] {
                    for entry_v in [entry.start_dot_ortho(epsilon), entry.end_dot_ortho(epsilon)] {
                        if active_v.distance_squared(entry_v) < epsilon.into_inner() {
                            intersect = Some(active_v);
                        }
                    }
                }
            } else {
                let (entry_t, active_t) = line_intersect(
                    entry.start_dot_ortho(epsilon),
                    entry.dot_ortho_dir(epsilon),
                    active.start_dot_ortho(epsilon),
                    active.dot_ortho_dir(epsilon),
                );
                let allowed_range = (-epsilon.into_inner())..=(1. + epsilon.into_inner());
                if allowed_range.contains(&entry_t) && allowed_range.contains(&active_t) {
                    intersect = Some(
                        entry.start_dot_ortho(epsilon) + entry.dot_ortho_dir(epsilon) * entry_t,
                    );
                }
            }

            if let Some(Vec2 { x: intersect_dot, y: intersect_ortho }) = intersect {
                let intersect_position = Position::ORIGIN
                    + Length::new(
                        *self.sweeper.sweep_dir * intersect_dot
                            + *self.sweeper.ortho_dir * intersect_ortho,
                    );

                self.intersections.push(LineIntersection {
                    dot:      NotNan::new(intersect_dot).expect("intersection is nan"),
                    ortho:    NotNan::new(intersect_ortho).expect("intersection is nan"),
                    lines:    [active.line_index, entry.line_index],
                    position: intersect_position,
                });
            }
        }

        if entry_need_intersect { &mut self.active_need } else { &mut self.active_need_not }
            .insert(entry.line_index, entry);
        Poll::Continue
    }

    fn yield_end(&mut self) -> Poll {
        let entry =
            slice_take_first(&mut self.ends).expect("yield_start called when starts is empty");
        let entry_need_intersect = (self.sweeper.line_fn)(entry.line_index).need_intersect;
        if entry_need_intersect { &mut self.active_need } else { &mut self.active_need_not }
            .remove(&entry.line_index);
        Poll::Continue
    }

    fn yield_intersect(&mut self) -> Poll {
        let intersect = self.intersections.pop().expect("yield_start called when starts is empty");
        Poll::Yield(intersect)
    }
}

struct State<'a, F> {
    sweeper:         &'a LineSweeper<F>,
    starts:          &'a [StartEntry],
    ends:            &'a [EndEntry],
    /// Active lines that `need_intersect`.
    active_need:     HashMap<LineIndex, StartEntry>,
    /// Active lines that `!need_intersect`.
    active_need_not: HashMap<LineIndex, StartEntry>,
    /// Possible intersections not yet yielded.
    intersections:   BinaryHeap<LineIntersection>,
}

#[derive(Debug, Clone, Copy)]
struct StartEntry {
    line_index:              LineIndex,
    start_pos:               Position<Vec2>,
    start_dot_minus_epsilon: NotNan<f32>,
    end_dot_plus_epsilon:    NotNan<f32>,
    start_ortho:             NotNan<f32>,
    end_ortho:               NotNan<f32>,
}

impl StartEntry {
    fn start_dot(self, epsilon: NotNan<f32>) -> NotNan<f32> {
        self.start_dot_minus_epsilon + epsilon
    }
    fn end_dot(self, epsilon: NotNan<f32>) -> NotNan<f32> { self.end_dot_plus_epsilon - epsilon }

    fn dot_ortho_dir(self, epsilon: NotNan<f32>) -> Vec2 {
        Vec2::new(
            (self.end_dot(epsilon) - self.start_dot(epsilon)).into_inner(),
            (self.end_ortho - self.start_ortho).into_inner(),
        )
    }

    fn start_dot_ortho(self, epsilon: NotNan<f32>) -> Vec2 {
        Vec2::new(self.start_dot(epsilon).into_inner(), self.start_ortho.into_inner())
    }

    fn end_dot_ortho(self, epsilon: NotNan<f32>) -> Vec2 {
        Vec2::new(self.end_dot(epsilon).into_inner(), self.end_ortho.into_inner())
    }
}

#[derive(Clone, Copy)]
struct EndEntry {
    line_index:           LineIndex,
    end_pos:              Position<Vec2>,
    end_dot_plus_epsilon: NotNan<f32>,
}

#[derive(Debug, Clone)]
pub struct LineIntersection {
    dot:          NotNan<f32>,
    ortho:        NotNan<f32>,
    pub position: Position<Vec2>,
    pub lines:    [LineIndex; 2],
}

impl PartialEq for LineIntersection {
    fn eq(&self, other: &Self) -> bool { self.dot == other.dot && self.lines == other.lines }
}

impl Eq for LineIntersection {}

impl PartialOrd for LineIntersection {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> { Some(self.cmp(other)) }
}

impl Ord for LineIntersection {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.dot.cmp(&other.dot).then_with(|| self.lines.cmp(&other.lines))
    }
}

enum Poll {
    Continue,
    Yield(LineIntersection),
    End,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Non-finite float encountered")]
    NonFiniteFloat,
    #[error("Ground line has zero length")]
    ZeroLengthLine,
}

impl From<ordered_float::FloatIsNan> for Error {
    fn from(_: ordered_float::FloatIsNan) -> Self { Self::NonFiniteFloat }
}

fn slice_take_first<'a, T>(slice: &mut &'a [T]) -> Option<&'a T> {
    match mem::take(slice).split_first() {
        Some((first, rest)) => {
            *slice = rest;
            Some(first)
        }
        _ => None,
    }
}
