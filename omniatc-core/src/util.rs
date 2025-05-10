use std::fmt;
use std::future::Future;

use bevy::app::{self, App, Plugin};
use bevy::ecs::bundle::Bundle;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::Event;
use bevy::ecs::observer::Trigger;
use bevy::ecs::relationship::{Relationship, RelationshipTarget};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::graph::GraphInfo;
use bevy::ecs::schedule::{
    Chain, IntoScheduleConfigs, Schedulable, ScheduleConfigs, ScheduleLabel, SystemSet,
};
use bevy::ecs::system::{Commands, IntoObserverSystem, IntoSystem, ResMut};
use bevy::tasks::{self, IoTaskPool, Task};
use itertools::Itertools;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<AsyncPollList>();
        app.add_systems(app::FixedPreUpdate, poll_async_system);
    }
}

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

pub fn manage_entity_vec<C, X, NB>(
    list_entity: Entity,
    list: Option<&C>,
    ctx: &mut X,
    mut spawn_fn: impl FnMut(usize, &mut X) -> Option<NB>,
    mut update_fn: impl FnMut(usize, &mut X, Entity) -> Result<(), ()>,
    commands: &mut Commands,
) where
    C: Component + RelationshipTarget,
    NB: Bundle,
{
    let mut entities = list.into_iter().flat_map(C::iter).peekable();

    let mut index = 0;
    let mut fused = false;
    while !fused {
        if let Some(&entity) = entities.peek() {
            if update_fn(index, ctx, entity).is_ok() {
                entities.next();
            } else {
                fused = true;
            }
        } else {
            if let Some(bundle) = spawn_fn(index, ctx) {
                commands.spawn((bundle, <C::Relationship as Relationship>::from(list_entity)));
            } else {
                fused = true;
            }
        }

        index += 1;
    }

    for entity in entities {
        commands.entity(entity).despawn();
    }
}

pub struct RunAsync<R>(Task<R>);

pub fn run_async<R: Send + Sync + 'static>(
    task: impl Future<Output = R> + Send + 'static,
) -> RunAsync<R> {
    let task = IoTaskPool::get().spawn(task);
    RunAsync(task)
}

impl<R: Send + Sync + 'static> RunAsync<R> {
    // TODO: refactor to support `then: impl FnOnce(R, P)` instead.
    pub fn then<M>(
        self,
        commands: &mut Commands,
        poll_list: &mut AsyncPollList,
        then: impl IntoObserverSystem<AsyncResultTrigger<R>, (), M>,
    ) {
        let mut task = self.0;
        let handler = commands.add_observer(then).id();
        poll_list.0.push(Box::new(move |commands| {
            if let Some(result) = tasks::block_on(tasks::poll_once(&mut task)) {
                commands.entity(handler).trigger(AsyncResultTrigger(Some(result))).despawn();
                AsyncPollResult::Done
            } else {
                AsyncPollResult::Pending
            }
        }));
    }
}

#[derive(PartialEq, Eq)]
enum AsyncPollResult {
    Done,
    Pending,
}

/// Wraps the result of a [`run_async`] task.
#[derive(Event)]
pub struct AsyncResultTrigger<R>(Option<R>);

impl<R> AsyncResultTrigger<R> {
    /// # Panics
    /// Panics if called more than once.
    pub fn get(&mut self) -> R {
        self.0.take().expect("AsyncResult.get() should only be called once")
    }
}

pub type AsyncResult<'w, R> = Trigger<'w, AsyncResultTrigger<R>>;

#[derive(Resource, Default)]
pub struct AsyncPollList(Vec<AsyncPoll>);

type AsyncPoll = Box<dyn FnMut(&mut Commands) -> AsyncPollResult + Send + Sync>;

fn poll_async_system(mut poll_list: ResMut<AsyncPollList>, mut commands: Commands) {
    poll_list.0.extract_if(.., |poll| poll(&mut commands) == AsyncPollResult::Done).for_each(drop);
}
