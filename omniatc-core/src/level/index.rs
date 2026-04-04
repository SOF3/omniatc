use std::collections::HashMap;
use std::marker::PhantomData;

use bevy::app::{self, App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::QueryFilter;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::math::Vec3;
use math::Position;
use oktree::prelude::{
    Aabb as TreeAabb, Octree as RawOctree, Position as TreePosition, TUVec3 as TreeVec3,
};

use super::SystemSets;

#[cfg(test)]
mod tests;

/// Per-axis spatial precision: `u8` gives 256 bins per axis.
///
/// Keep this as a type alias so the precision can be changed in one place later.
type Precision = u8;

/// Unsigned integer used for `oktree` coordinates.
///
/// Must be wide enough to hold `Precision::MAX + Precision::MAX` without overflow, because
/// `oktree` computes AABB centers as `(min + max) >> 1` using plain integer addition.
/// With `Precision = u8` (max 255), the sum can reach 510, so `u16` is the minimum.
type TreeUnsigned = u16;

const PRECISION_BIN_COUNT: f32 = Precision::MAX as f32 + 1.0;
const INITIAL_HALF_EXTENT: f32 = 128.0;

/// Generic spatial index plugin.
///
/// The plugin queries `(Entity, &C)` with filter `Qf`, extracts each entity position via `PosFn`,
/// and rebuilds a typed [`OctreeIndex`] resource in [`SystemSets::UpdateIndex`] each update tick.
pub struct Plug<C, Qf, PosFn> {
    extractor: PosFn,
    marker:    PhantomData<fn() -> (C, Qf)>,
}

impl<C, Qf, PosFn> Plug<C, Qf, PosFn>
where
    PosFn: Fn(&C) -> Position<Vec3> + Send + Sync + 'static,
{
    #[must_use]
    pub fn new(extractor: PosFn) -> Self { Self { extractor, marker: PhantomData } }
}

impl<C, Qf, PosFn> Plugin for Plug<C, Qf, PosFn>
where
    C: Component,
    Qf: QueryFilter + 'static,
    PosFn: Fn(&C) -> Position<Vec3> + Clone + Send + Sync + 'static,
{
    fn build(&self, app: &mut App) {
        app.init_resource::<OctreeIndex<C, Qf>>();
        app.insert_resource(IndexExtractor::<C, Qf, PosFn> {
            extractor: self.extractor.clone(),
            marker:    PhantomData,
        });
        app.add_systems(
            app::Update,
            rebuild_index_system::<C, Qf, PosFn>.in_set(SystemSets::UpdateIndex),
        );
    }
}

fn rebuild_index_system<C, Qf, PosFn>(
    query: Query<(Entity, &C), Qf>,
    extractor: Res<IndexExtractor<C, Qf, PosFn>>,
    mut index: ResMut<OctreeIndex<C, Qf>>,
) where
    C: Component,
    Qf: QueryFilter + 'static,
    PosFn: Fn(&C) -> Position<Vec3> + Send + Sync + 'static,
{
    let extract = &extractor.extractor;
    index.rebuild(
        query.iter().map(|(entity, component)| Entry { entity, position: extract(component) }),
    );
}

/// Indexed entity payload kept in the octree.
#[derive(Debug, Clone, Copy)]
pub struct Entry {
    pub entity:   Entity,
    pub position: Position<Vec3>,
}

#[derive(Debug, Clone)]
struct CellEntry {
    cell:    TreeVec3<TreeUnsigned>,
    entries: Vec<Entry>,
}

impl TreePosition for CellEntry {
    type U = TreeUnsigned;

    fn position(&self) -> TreeVec3<Self::U> { self.cell }
}

/// Octree-backed container for indexed entities.
///
/// Current stub behavior buckets entities by quantized cells and stores full-precision world
/// positions in each bucket.
pub struct Octree {
    tree: RawOctree<TreeUnsigned, CellEntry>,
}

impl Default for Octree {
    fn default() -> Self { Self { tree: new_tree(0) } }
}

impl Octree {
    pub fn rebuild(&mut self, entries: impl IntoIterator<Item = Entry>) {
        let entries = entries.into_iter().collect::<Vec<_>>();
        let mapping = Mapping::from_entries(&entries);

        let mut by_cell = HashMap::new();
        for entry in entries {
            by_cell
                .entry(mapping.position_to_cell(entry.position))
                .or_insert_with(Vec::new)
                .push(entry);
        }

        self.tree = new_tree(by_cell.len());
        for (cell, entries) in by_cell {
            self.tree.insert(CellEntry { cell, entries }).expect("cell insert must fit in tree");
        }
    }

    #[must_use]
    pub fn len(&self) -> usize { self.tree.iter().map(|cell| cell.entries.len()).sum() }

    #[must_use]
    pub fn is_empty(&self) -> bool { self.tree.is_empty() }

    pub fn iter(&self) -> impl Iterator<Item = &Entry> {
        self.tree.iter().flat_map(|cell| cell.entries.iter())
    }

    /// Returns all indexed entities with positions inside the inclusive world-space bounds.
    pub fn entities_in_bounds(
        &self,
        min: Position<Vec3>,
        max: Position<Vec3>,
    ) -> impl Iterator<Item = Entity> + '_ {
        let min = min.get();
        let max = max.get();

        self.iter().filter_map(move |entry| {
            let position = entry.position.get();
            if (position.cmpge(min) & position.cmple(max)).all() {
                Some(entry.entity)
            } else {
                None
            }
        })
    }
}

/// Typed resource that holds the current octree for a `(C, Qf)` plugin instantiation.
#[derive(Resource)]
pub struct OctreeIndex<C, Qf = ()> {
    octree: Octree,
    marker: PhantomData<fn() -> (C, Qf)>,
}

impl<C, Qf> Default for OctreeIndex<C, Qf> {
    fn default() -> Self { Self { octree: Octree::default(), marker: PhantomData } }
}

impl<C, Qf> OctreeIndex<C, Qf> {
    pub fn rebuild(&mut self, entries: impl IntoIterator<Item = Entry>) {
        self.octree.rebuild(entries);
    }

    #[must_use]
    pub fn octree(&self) -> &Octree { &self.octree }

    pub fn entries(&self) -> impl Iterator<Item = &Entry> { self.octree.iter() }

    /// Returns all indexed entities with positions inside the inclusive world-space bounds.
    pub fn entities_in_bounds(
        &self,
        min: Position<Vec3>,
        max: Position<Vec3>,
    ) -> impl Iterator<Item = Entity> + '_ {
        self.octree.entities_in_bounds(min, max)
    }
}

#[derive(Resource)]
struct IndexExtractor<C, Qf = (), PosFn = fn(&C) -> Position<Vec3>> {
    extractor: PosFn,
    marker:    PhantomData<fn() -> (C, Qf)>,
}

#[derive(Debug, Clone, Copy)]
struct Mapping {
    half_extent: f32,
}

impl Mapping {
    fn from_entries(entries: &[Entry]) -> Self {
        let max_abs =
            entries.iter().map(|entry| entry.position.get()).fold(0f32, |max_abs, position| {
                max_abs.max(position.x.abs().max(position.y.abs()).max(position.z.abs()))
            });

        let mut half_extent = INITIAL_HALF_EXTENT;
        while max_abs > half_extent {
            half_extent *= 2.0;
        }

        Self { half_extent }
    }

    fn position_to_cell(self, position: Position<Vec3>) -> TreeVec3<TreeUnsigned> {
        let position = position.get();
        TreeVec3::new(
            TreeUnsigned::from(self.axis_to_bin(position.x)),
            TreeUnsigned::from(self.axis_to_bin(position.y)),
            TreeUnsigned::from(self.axis_to_bin(position.z)),
        )
    }

    fn axis_to_bin(self, value: f32) -> Precision {
        let normalized = ((value + self.half_extent) / (2.0 * self.half_extent)).clamp(0.0, 1.0);
        let scaled = (normalized * PRECISION_BIN_COUNT).floor();

        if scaled >= PRECISION_BIN_COUNT {
            return Precision::MAX;
        }

        #[expect(clippy::cast_possible_truncation, reason = "scaled is clamped to precision bins")]
        #[expect(clippy::cast_sign_loss, reason = "scaled is clamped to non-negative")]
        {
            scaled as Precision
        }
    }
}

fn new_tree(capacity: usize) -> RawOctree<TreeUnsigned, CellEntry> {
    let max = TreeUnsigned::from(Precision::MAX);
    let aabb = TreeAabb::from_min_max(TreeVec3::zero(), TreeVec3::splat(max));
    RawOctree::from_aabb_with_capacity(aabb, capacity)
}
