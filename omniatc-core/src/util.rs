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
        {
            #[allow(clippy::question_mark)]
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
}

use bevy::app::App;
use bevy::ecs::schedule::graph::GraphInfo;
use bevy::ecs::schedule::{
    Chain, IntoScheduleConfigs, Schedulable, ScheduleConfigs, ScheduleLabel, SystemSet,
};
use itertools::Itertools;
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

pub fn configure_ordered_system_sets<E: strum::IntoEnumIterator + SystemSet + Clone>(
    app: &mut App,
    schedule: impl ScheduleLabel + Clone,
) {
    for (before, after) in E::iter().tuple_windows() {
        app.configure_sets(schedule.clone(), before.before(after));
    }
}

pub trait EnumScheduleConfig<T: Schedulable<Metadata = GraphInfo, GroupMetadata = Chain>, Marker>:
    IntoScheduleConfigs<T, Marker>
{
    fn after_all<E: strum::IntoEnumIterator + SystemSet>(self) -> ScheduleConfigs<T> {
        let mut configs = self.into_configs();
        for set in E::iter() {
            configs = configs.after(set);
        }
        configs
    }

    fn before_all<E: strum::IntoEnumIterator + SystemSet>(self) -> ScheduleConfigs<T> {
        let mut configs = self.into_configs();
        for set in E::iter() {
            configs = configs.before(set);
        }
        configs
    }
}

impl<C, T, Marker> EnumScheduleConfig<T, Marker> for C
where
    T: Schedulable<Metadata = GraphInfo, GroupMetadata = Chain>,
    C: IntoScheduleConfigs<T, Marker>,
{
}
