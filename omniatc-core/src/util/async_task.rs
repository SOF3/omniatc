use std::future::Future;

use bevy::app::{self, App, Plugin};
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EntityEvent;
use bevy::ecs::observer;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, IntoObserverSystem, ResMut};
use bevy::tasks::{self, IoTaskPool, Task};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<AsyncManager>();
        app.add_systems(app::FixedPreUpdate, poll_async_system);
    }
}

/// Return value of [`run_async`].
pub struct RunAsync<R>(Task<R>);

/// Spawns an async task on the global I/O task pool,
/// returning a handle to optionally run an observer on completion.
pub fn run_async<R: Send + Sync + 'static>(
    task: impl Future<Output = R> + Send + 'static,
) -> RunAsync<R> {
    let task = IoTaskPool::get().spawn(task);
    RunAsync(task)
}

pub fn run_async_local<R: Send + Sync + 'static>(
    task: impl Future<Output = R> + 'static,
) -> RunAsync<R> {
    let task = IoTaskPool::get().spawn_local(task);
    RunAsync(task)
}

impl<R: Send + Sync + 'static> RunAsync<R> {
    // TODO: refactor to support `then: impl FnOnce(R, P)` instead.
    /// Tracks the progress of the task in the background,
    /// triggering the observer in `then` when the task completes.
    ///
    /// The task submitted to `run_async` still executes witout calling this function.
    /// [`AsyncManager`] is only used for running an observer with world context
    /// when the task completes, but is not responsible for driving the task itself.
    pub fn then<M>(
        self,
        commands: &mut Commands,
        manager: &mut AsyncManager,
        then: impl IntoObserverSystem<AsyncResultTrigger<R>, (), M>,
    ) {
        let mut task = self.0;
        let observer_id = commands.spawn_empty().observe(then).id();
        manager.0.push(Box::new(move |commands| {
            match tasks::block_on(tasks::poll_once(&mut task)) {
                Some(result) => {
                    commands
                        .entity(observer_id)
                        .trigger(move |entity| AsyncResultTrigger(entity, Some(result)))
                        .despawn();
                    AsyncPollResult::Done
                }
                _ => AsyncPollResult::Pending,
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
#[derive(EntityEvent)]
pub struct AsyncResultTrigger<R>(#[event_target] Entity, Option<R>);

impl<R> AsyncResultTrigger<R> {
    /// # Panics
    /// Panics if called more than once.
    pub fn get(&mut self) -> R {
        self.1.take().expect("AsyncResultTrigger::get() should only be called once")
    }
}

pub type AsyncResult<'w, 't, R> = observer::On<'w, 't, AsyncResultTrigger<R>>;

/// Drives async tasks to execute in a bevy app.
///
/// Contains a list of running async tasks.
/// Each task is polled once every frame,
/// and [`Done`](AsyncPollResult::Done) tasks are removed from the list.
#[derive(Resource, Default)]
pub struct AsyncManager(Vec<AsyncPoll>);

type AsyncPoll = Box<dyn FnMut(&mut Commands) -> AsyncPollResult + Send + Sync>;

fn poll_async_system(mut poll_list: ResMut<AsyncManager>, mut commands: Commands) {
    poll_list.0.extract_if(.., |poll| poll(&mut commands) == AsyncPollResult::Done).for_each(drop);
}
