use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{QueryData, QueryFilter};
use bevy::ecs::system::Query;
use bevy::ecs::world::World;

pub trait QueryWith<C: Component> {
    fn get(&self, entity: Entity) -> Option<&C>;
}

impl<C: Component> QueryWith<C> for World {
    fn get(&self, entity: Entity) -> Option<&C> { World::get(self, entity) }
}

impl<C: Component> QueryWith<C> for Query<'_, '_, &C> {
    fn get(&self, entity: Entity) -> Option<&C> { Query::get(self, entity).ok() }
}

pub struct MapQuery<'w, 's, D: QueryData, F: QueryFilter, M> {
    pub query: Query<'w, 's, D, F>,
    pub map:   M,
}

impl<C, D, F, M> QueryWith<C> for MapQuery<'_, '_, D, F, M>
where
    C: Component,
    D: QueryData,
    F: QueryFilter,
    // &'w () to circumvent E0582
    M: for<'w> Fn(<D::ReadOnly as QueryData>::Item<'w, '_>, &'w ()) -> &'w C,
{
    fn get(&self, entity: Entity) -> Option<&C> {
        self.query.get(entity).ok().map(|v| (self.map)(v, &()))
    }
}
