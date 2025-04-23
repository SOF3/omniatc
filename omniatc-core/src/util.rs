use std::fmt;

#[macro_export]
macro_rules! try_log {
    (
        $expr:expr,
        expect $must:literal $(
            (
                $($must_args:expr),* $(,)?
            )
        )?
        or $never:expr
    ) => {
        if let Some(value) = $crate::util::TryLog::convert_or_log(
            $expr,
            format_args!($must, $($($must_args),*)?),
        ) {
            value
        } else {
            $never
        }
    }
}

pub use try_log;

#[macro_export]
macro_rules! try_log_return {
    ($expr:expr, expect $must:literal $(, $($must_args:expr),*)? $(,)?) => {
        $crate::try_log!($expr, expect $must $(($($must_args),*))? or return)
    }
}

pub use try_log_return;

/// An expression that can be used for `$expr` in [`try_log!`].
pub trait TryLog<T> {
    /// Returns the successful result as `Some`, or log the error with `must`.
    fn convert_or_log(this: Self, must: impl fmt::Display) -> Option<T>;
}

impl<T> TryLog<T> for Option<T> {
    fn convert_or_log(this: Self, must: impl fmt::Display) -> Option<T> {
        if let Some(value) = this {
            Some(value)
        } else {
            bevy::log::error!("{must}");
            None
        }
    }
}

impl<T, E: fmt::Display> TryLog<T> for Result<T, E> {
    fn convert_or_log(this: Self, must: impl fmt::Display) -> Option<T> {
        match this {
            Ok(value) => Some(value),
            Err(err) => {
                bevy::log::error!("{must}: {err}");
                None
            }
        }
    }
}
