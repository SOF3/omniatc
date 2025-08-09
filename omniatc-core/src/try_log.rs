use std::any::type_name;

use bevy::ecs::change_detection::Mut;
use bevy::ecs::component::{Component, Mutable};
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{QueryData, QueryFilter};
use bevy::ecs::system::Query;
use bevy::ecs::world::{EntityRef, EntityWorldMut, World};

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

pub trait QueryExtSuper {
    type Read<'a>;
    type Write<'a>;
}

pub trait QueryExt: QueryExtSuper {
    fn log_get(&self, entity: Entity) -> Option<Self::Read<'_>>;

    fn log_get_mut(&mut self, entity: Entity) -> Option<Self::Write<'_>>;
}

impl<D, F> QueryExtSuper for Query<'_, '_, D, F>
where
    D: QueryData,
    F: QueryFilter,
{
    type Read<'a> = <D::ReadOnly as QueryData>::Item<'a>;
    type Write<'a> = D::Item<'a>;
}

impl<D, F> QueryExt for Query<'_, '_, D, F>
where
    D: QueryData,
    F: QueryFilter,
{
    fn log_get(&self, entity: Entity) -> Option<<D::ReadOnly as QueryData>::Item<'_>> {
        match self.get(entity) {
            Ok(value) => Some(value),
            Err(err) => {
                bevy::log::error!("Expected {entity:?} to match query: {err}");
                None
            }
        }
    }

    fn log_get_mut(&mut self, entity: Entity) -> Option<D::Item<'_>> {
        match self.get_mut(entity) {
            Ok(value) => Some(value),
            Err(err) => {
                bevy::log::error!("Expected {entity:?} to match query: {err}");
                None
            }
        }
    }
}

pub trait WorldExt {
    fn log_get<T: Component>(&self, entity: Entity) -> Option<&T>;

    fn log_get_mut<T: Component<Mutability = Mutable>>(
        &mut self,
        entity: Entity,
    ) -> Option<Mut<'_, T>>;
}

impl WorldExt for World {
    fn log_get<T: Component>(&self, entity: Entity) -> Option<&T> {
        if let Some(value) = self.get::<T>(entity) {
            Some(value)
        } else {
            bevy::log::error!("Expected {entity:?} to have component {}", type_name::<T>());
            None
        }
    }

    fn log_get_mut<T: Component<Mutability = Mutable>>(
        &mut self,
        entity: Entity,
    ) -> Option<Mut<'_, T>> {
        if let Some(value) = self.get_mut::<T>(entity) {
            Some(value)
        } else {
            bevy::log::error!("Expected {entity:?} to have component {}", type_name::<T>());
            None
        }
    }
}

pub trait EntityRefExt {
    fn log_get<T: Component>(&self) -> Option<&T>;
}

impl EntityRefExt for EntityRef<'_> {
    fn log_get<T: Component>(&self) -> Option<&T> {
        if let Some(value) = self.get::<T>() {
            Some(value)
        } else {
            bevy::log::error!("Expected {:?} to have component {}", self.id(), type_name::<T>());
            None
        }
    }
}

pub trait EntityWorldMutExt {
    fn log_get<T: Component>(&self) -> Option<&T>;

    fn log_get_mut<T: Component<Mutability = Mutable>>(&mut self) -> Option<Mut<'_, T>>;
}

impl EntityWorldMutExt for EntityWorldMut<'_> {
    fn log_get<T: Component>(&self) -> Option<&T> {
        if let Some(value) = self.get::<T>() {
            Some(value)
        } else {
            bevy::log::error!("Expected {:?} to have component {}", self.id(), type_name::<T>());
            None
        }
    }

    fn log_get_mut<T: Component<Mutability = Mutable>>(&mut self) -> Option<Mut<'_, T>> {
        let id = self.id(); // polonius does not like this being in the match arm
        if let Some(value) = self.get_mut::<T>() {
            Some(value)
        } else {
            bevy::log::error!("Expected {:?} to have component {}", id, type_name::<T>());
            None
        }
    }
}
