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
        app.init_resource::<AsyncPollList>();
        app.add_systems(app::FixedPreUpdate, poll_async_system);
    }
}

pub struct RunAsync<R>(Task<R>);

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
    pub fn then<M>(
        self,
        commands: &mut Commands,
        poll_list: &mut AsyncPollList,
        then: impl IntoObserverSystem<AsyncResultTrigger<R>, (), M>,
    ) {
        let mut task = self.0;
        let handler = commands.add_observer(then).id();
        poll_list.0.push(Box::new(move |commands| {
            match tasks::block_on(tasks::poll_once(&mut task)) {
                Some(result) => {
                    commands
                        .entity(handler)
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

#[derive(Resource, Default)]
pub struct AsyncPollList(Vec<AsyncPoll>);

type AsyncPoll = Box<dyn FnMut(&mut Commands) -> AsyncPollResult + Send + Sync>;

fn poll_async_system(mut poll_list: ResMut<AsyncPollList>, mut commands: Commands) {
    poll_list.0.extract_if(.., |poll| poll(&mut commands) == AsyncPollResult::Done).for_each(drop);
}
