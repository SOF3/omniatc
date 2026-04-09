use std::iter;
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
    _ph:       PhantomData<fn() -> (C, Qf)>,
}

impl<C, Qf, PosFn> Plug<C, Qf, PosFn>
where
    PosFn: Fn(&C) -> Position<Vec3> + Send + Sync + 'static,
{
    #[must_use]
    pub fn new(extractor: PosFn) -> Self { Self { extractor, _ph: PhantomData } }
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
struct OctreeNode {
    cell:    TreeVec3<TreeUnsigned>,
    entries: Vec<Entry>,
}

impl TreePosition for OctreeNode {
    type U = TreeUnsigned;

    fn position(&self) -> TreeVec3<Self::U> { self.cell }
}

/// Octree-backed container for indexed entities.
///
/// Current stub behavior buckets entities by quantized cells and stores full-precision world
/// positions in each bucket.
pub struct Octree {
    tree:    RawOctree<TreeUnsigned, OctreeNode>,
    mapping: Mapping,
}

impl Default for Octree {
    fn default() -> Self {
        Self { tree: new_tree(), mapping: Mapping::from_entries(iter::empty()) }
    }
}

impl Octree {
    pub fn rebuild(&mut self, entries: impl IntoIterator<Item = Entry> + Clone) {
        let mapping = Mapping::from_entries(entries.clone());

        self.tree = new_tree();
        self.mapping = mapping;

        for entry in entries {
            let cell = mapping.position_to_cell(entry.position);
            self.tree
                .entry(cell)
                .or_insert_with(|| OctreeNode { cell, entries: Vec::new() })
                .entries
                .push(entry);
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
        [min, max]: [Position<Vec3>; 2],
    ) -> impl Iterator<Item = Entity> + '_ {
        let min_cell = self.mapping.position_to_cell(min);
        let max_cell = self.mapping.position_to_cell(max);

        (min_cell.x..=max_cell.x)
            .flat_map(move |x| (min_cell.y..=max_cell.y).map(move |y| (x, y)))
            .flat_map(move |(x, y)| (min_cell.z..=max_cell.z).map(move |z| TreeVec3::new(x, y, z)))
            .filter_map(|cell| self.tree.get(&cell))
            .flat_map(|node| &node.entries)
            .filter(move |entry| {
                entry.position.get().cmple(max.get()).all()
                    && entry.position.get().cmpge(min.get()).all()
            })
            .map(|entry| entry.entity)
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
    pub fn rebuild(&mut self, entries: impl IntoIterator<Item = Entry> + Clone) {
        self.octree.rebuild(entries);
    }

    #[must_use]
    pub fn octree(&self) -> &Octree { &self.octree }

    pub fn entries(&self) -> impl Iterator<Item = &Entry> { self.octree.iter() }

    /// Returns all indexed entities with positions inside the inclusive world-space bounds.
    pub fn entities_in_bounds(
        &self,
        [min, max]: [Position<Vec3>; 2],
    ) -> impl Iterator<Item = Entity> + '_ {
        self.octree.entities_in_bounds([min, max])
    }
}

#[derive(Resource)]
struct IndexExtractor<C, Qf = (), PosFn = fn(&C) -> Position<Vec3>> {
    extractor: PosFn,
    marker:    PhantomData<fn() -> (C, Qf)>,
}

/// Maps float coordinates to octree cell coordinates.
#[derive(Debug, Clone, Copy)]
struct Mapping {
    half_extent: f32,
}

impl Mapping {
    fn from_entries(entries: impl IntoIterator<Item = Entry>) -> Self {
        let max_abs = entries.into_iter().map(|entry| entry.position.get()).fold(
            0f32,
            |max_abs, position| {
                max_abs.max(position.x.abs().max(position.y.abs()).max(position.z.abs()))
            },
        );

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

fn new_tree() -> RawOctree<TreeUnsigned, OctreeNode> {
    let max = TreeVec3::splat(TreeUnsigned::from(Precision::MAX));
    let aabb = TreeAabb::from_min_max(TreeVec3::zero(), max);
    RawOctree::from_aabb(aabb)
}
