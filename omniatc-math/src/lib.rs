#![warn(clippy::pedantic)]
#![cfg_attr(feature = "precommit-checks", deny(warnings, clippy::pedantic, clippy::dbg_macro))]
#![allow(clippy::needless_pass_by_value, clippy::type_complexity)] // too many false positives from system params.
#![allow(clippy::collapsible_else_if)] // this is usually intentional
#![cfg_attr(not(feature = "precommit-checks"), allow(dead_code, unused_variables, unused_imports))]
#![cfg_attr(feature = "precommit-checks", allow(dead_code))] // TODO remove this in the future
#![cfg_attr(feature = "rust-analyzer", warn(warnings, clippy::pedantic, clippy::dbg_macro))] // TODO remove this in the future
#![cfg_attr(feature = "rust-analyzer", allow(unused_imports))]
// TODO remove this in the future

// we don't really want to read the mathematical constants in this file.
#![allow(clippy::excessive_precision, clippy::unreadable_literal)]

use std::{cmp, fmt, iter, ops};

mod units;
pub use units::*;

mod alg2d;
pub use alg2d::*;

mod physics;
pub use physics::*;

pub mod sweep;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Sign {
    Negative,
    Zero,
    Positive,
}

pub trait Between<U>: PartialOrd<U> {
    fn between_inclusive(&self, min: &U, max: &U) -> bool { self >= min && self <= max }
}

impl<T: PartialOrd<U>, U> Between<U> for T {}

/// Returns `start`, `start+interval`, `start+interval+interval`, ... until `end`.
/// The second last item is between `end - interval` and `end`, and is not equal to `end`.
///
/// # Panics
/// Panics if `interval` is not a finite positive or negative value.
pub fn range_steps<T, U>(mut start: T, end: T, interval: U) -> impl Iterator<Item = T> + Clone
where
    T: Copy + PartialOrd + ops::AddAssign<U>,
    U: fmt::Debug + Copy + Default + PartialOrd,
{
    let more_extreme = match interval.partial_cmp(&U::default()) {
        Some(cmp::Ordering::Less) => |a: T, b: T| a <= b,
        Some(cmp::Ordering::Greater) => |a, b| a >= b,
        _ => panic!("interval {interval:?} must be a finite positive or negative"),
    };

    let mut fuse = Some(end).filter(|_| more_extreme(end, start));

    iter::from_fn(move || {
        let output = start;
        if more_extreme(output, end) {
            fuse.take()
        } else {
            start += interval;
            Some(output)
        }
    })
}
