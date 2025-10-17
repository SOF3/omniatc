use bevy::app::App;
use bevy::ecs::schedule::graph::GraphInfo;
use bevy::ecs::schedule::{
    Chain, IntoScheduleConfigs, Schedulable, ScheduleConfigs, ScheduleLabel, SystemSet,
};
use itertools::Itertools;

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
