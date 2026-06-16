#![forbid(unsafe_code)]
mod arena;
#[cfg(test)]
mod arena_test;
mod cow;
mod multimap;
mod ordered_map;
#[cfg(test)]
mod ordered_map_test;
mod ordered_set;
#[cfg(test)]
mod ordered_set_test;
mod set;
mod syncmap;
#[cfg(test)]
mod syncmap_test;
mod syncset;

pub use gxhash::{GxBuildHasher, HashMapExt as FastHashMapExt, HashSetExt as FastHashSetExt};

pub use arena::{Arena, ArenaMap, Idx, IdxRange, RawIdx};
pub use cow::{CopyOnWriteMap, CopyOnWriteMapScope, CopyOnWriteSet, CopyOnWriteSetScope};
pub use multimap::{MultiMap, group_by, new_multi_map_with_size_hint};
pub use ordered_map::{
    MapEntry, OrderedMap, diff_ordered_maps, diff_ordered_maps_func, new_ordered_map_from_list,
    new_ordered_map_with_size_hint,
};
pub use ordered_set::{OrderedSet, new_ordered_set_with_size_hint};
pub use set::{Set, SetOptionExt, new_set_from_items, new_set_with_size_hint};
pub use syncmap::SyncMap;
pub use syncset::SyncSet;

pub type FastHashMap<K, V> = gxhash::HashMap<K, V>;
pub type FastHashSet<T> = gxhash::HashSet<T>;
