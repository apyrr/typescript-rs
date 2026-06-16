use std::{
    hash::{Hash, Hasher},
    marker::PhantomData,
    num::NonZeroU32,
    ops::ControlFlow,
    slice,
    sync::{
        Mutex, Weak,
        atomic::{AtomicU64, Ordering},
    },
};

use smallvec::SmallVec;
use ts_collections::{Arena, Idx, IdxRange, RawIdx};
use ts_core as core;

use crate::{Kind, NodeFlags, ast::SourceFileData, ast_generated::*, modifierflags::ModifierFlags};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StoreId(u64);

impl StoreId {
    #[inline]
    pub(crate) fn get(self) -> u64 {
        self.0
    }

    #[inline]
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

static NEXT_STORE_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_AST_NODE_ID: AtomicU64 = AtomicU64::new(0);
const NODE_ID_BLOCK_SIZE: u64 = 4096;

impl Default for StoreId {
    fn default() -> Self {
        Self(NEXT_STORE_ID.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct AstNodeId(NonZeroU32);

impl AstNodeId {
    #[inline(always)]
    pub(crate) fn from_idx(idx: Idx<NodeHeader>) -> Self {
        let raw = idx.into_raw().into_u32();
        let packed = raw
            .checked_add(1)
            .expect("AST node index exceeds u32 payload space");
        Self(NonZeroU32::new(packed).expect("packed AST node id must be non-zero"))
    }

    #[inline(always)]
    pub(crate) fn to_idx(self) -> Idx<NodeHeader> {
        Idx::from_raw(RawIdx::from_u32(self.0.get() - 1))
    }

    #[inline(always)]
    pub(crate) fn get(self) -> u32 {
        self.0.get()
    }
}

#[inline(always)]
fn pack_idx<T>(idx: Idx<T>) -> NonZeroU32 {
    let raw = idx.into_raw().into_u32();
    let packed = raw
        .checked_add(1)
        .expect("AST arena index exceeds u32 payload space");
    NonZeroU32::new(packed).expect("packed AST arena id must be non-zero")
}

#[inline(always)]
fn unpack_idx<T>(raw: NonZeroU32) -> Idx<T> {
    Idx::from_raw(RawIdx::from_u32(raw.get() - 1))
}

macro_rules! define_packed_arena_id {
    ($id:ident, $optional:ident, $record:ty) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub(crate) struct $id {
            store_id: StoreId,
            raw: NonZeroU32,
            _ty: PhantomData<fn() -> $record>,
        }

        impl $id {
            #[inline(always)]
            fn from_idx(store_id: StoreId, idx: Idx<$record>) -> Self {
                Self {
                    store_id,
                    raw: pack_idx(idx),
                    _ty: PhantomData,
                }
            }

            #[inline(always)]
            fn to_idx(self) -> Idx<$record> {
                unpack_idx(self.raw)
            }

            pub(crate) fn store_id(self) -> StoreId {
                self.store_id
            }

            pub(crate) fn assert_store(self, store_id: StoreId) {
                assert_eq!(
                    self.store_id, store_id,
                    "arena id belongs to a different AST store"
                );
            }
        }

        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
        pub(crate) struct $optional(Option<$id>);

        impl $optional {
            pub const fn none() -> Self {
                Self(None)
            }

            pub(crate) fn some(id: $id) -> Self {
                Self(Some(id))
            }

            pub(crate) fn from_option(value: Option<$id>) -> Self {
                Self(value)
            }

            pub(crate) fn get(self) -> Option<$id> {
                self.0
            }

            pub(crate) fn is_some(self) -> bool {
                self.0.is_some()
            }

            pub(crate) fn is_none(self) -> bool {
                self.0.is_none()
            }
        }
    };
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct OptionalAstNodeId(Option<Node>);

impl OptionalAstNodeId {
    pub const fn none() -> Self {
        Self(None)
    }

    pub(crate) fn some(node: Node) -> Self {
        Self(Some(node))
    }

    pub(crate) fn from_option(node: Option<Node>) -> Self {
        Self(node)
    }

    pub(crate) fn get(self) -> Option<Node> {
        self.0
    }

    pub fn is_some(self) -> bool {
        self.0.is_some()
    }

    pub fn is_none(self) -> bool {
        self.0.is_none()
    }
}

pub(crate) trait RequiredAstNodeId {
    fn required_node(self, store_id: StoreId) -> Node;
}

impl RequiredAstNodeId for AstNodeId {
    fn required_node(self, store_id: StoreId) -> Node {
        Node::new(store_id, self)
    }
}

impl RequiredAstNodeId for Node {
    fn required_node(self, _store_id: StoreId) -> Node {
        self
    }
}

impl RequiredAstNodeId for OptionalAstNodeId {
    fn required_node(self, _store_id: StoreId) -> Node {
        self.get().expect("required AST child is missing")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NodeListRecord {
    loc: core::TextRange,
    range: core::TextRange,
    entries: IdxRange<NodeListEntry>,
    missing: bool,
    has_trailing_comma: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct NodeListEntry {
    node: Node,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ModifierListRecord {
    list: NodeListId,
    modifier_flags: ModifierFlags,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RawNodeSliceRecord {
    entries: IdxRange<RawNodeSliceEntry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct RawNodeSliceEntry {
    node: OptionalAstNodeId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RawStringSliceRecord {
    entries: IdxRange<String>,
}

fn is_foreign_node(store_id: StoreId, node: Node) -> bool {
    node.store_id() != store_id
}

#[derive(Clone, Copy)]
pub(crate) struct NodeListView<'a> {
    id: NodeListId,
    loc: core::TextRange,
    range: core::TextRange,
    missing: bool,
    has_trailing_comma: bool,
    entries: &'a [NodeListEntry],
}

impl<'a> NodeListView<'a> {
    fn new(
        id: NodeListId,
        loc: core::TextRange,
        range: core::TextRange,
        missing: bool,
        has_trailing_comma: bool,
        entries: &'a [NodeListEntry],
    ) -> Self {
        Self {
            id,
            loc,
            range,
            missing,
            has_trailing_comma,
            entries,
        }
    }

    pub(crate) fn id(self) -> NodeListId {
        self.id
    }

    pub(crate) fn len(self) -> usize {
        self.entries.len()
    }

    pub(crate) fn is_empty(self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn loc(self) -> core::TextRange {
        self.loc
    }

    pub(crate) fn pos(self) -> i32 {
        self.loc.pos()
    }

    pub(crate) fn end(self) -> i32 {
        self.loc.end()
    }

    pub(crate) fn range(self) -> core::TextRange {
        self.range
    }

    pub(crate) fn is_missing(self) -> bool {
        self.missing
    }

    pub(crate) fn has_trailing_comma(self) -> bool {
        self.has_trailing_comma
    }

    pub(crate) fn iter(self) -> NodeListIter<'a> {
        NodeListIter {
            entries: self.entries.iter(),
        }
    }

    pub(crate) fn first(self) -> Option<Node> {
        self.entries.first().map(|entry| entry.node)
    }

    pub(crate) fn last(self) -> Option<Node> {
        self.entries.last().map(|entry| entry.node)
    }

    pub(crate) fn same_list(self, other: NodeListView<'_>) -> bool {
        self.id == other.id
    }
}

impl<'a> IntoIterator for NodeListView<'a> {
    type Item = Node;
    type IntoIter = NodeListIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub struct NodeListIter<'a> {
    entries: slice::Iter<'a, NodeListEntry>,
}

impl Iterator for NodeListIter<'_> {
    type Item = Node;

    fn next(&mut self) -> Option<Self::Item> {
        self.entries.next().map(|entry| entry.node)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.entries.size_hint()
    }
}

impl DoubleEndedIterator for NodeListIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.entries.next_back().map(|entry| entry.node)
    }
}

impl ExactSizeIterator for NodeListIter<'_> {}

#[derive(Clone, Copy)]
pub(crate) struct ModifierListView<'a> {
    id: ModifierListId,
    list: NodeListView<'a>,
    modifier_flags: ModifierFlags,
}

impl<'a> ModifierListView<'a> {
    fn new(id: ModifierListId, list: NodeListView<'a>, modifier_flags: ModifierFlags) -> Self {
        Self {
            id,
            list,
            modifier_flags,
        }
    }

    pub(crate) fn id(self) -> ModifierListId {
        self.id
    }

    pub(crate) fn modifier_flags(self) -> ModifierFlags {
        self.modifier_flags
    }

    pub(crate) fn nodes(self) -> NodeListView<'a> {
        self.list
    }
}

#[derive(Clone, Copy)]
pub(crate) struct RawNodeSliceView<'a> {
    entries: &'a [RawNodeSliceEntry],
}

impl<'a> RawNodeSliceView<'a> {
    fn new(entries: &'a [RawNodeSliceEntry]) -> Self {
        Self { entries }
    }

    pub(crate) fn iter(
        self,
    ) -> impl ExactSizeIterator<Item = Option<Node>> + DoubleEndedIterator + 'a {
        self.entries.iter().map(move |entry| entry.node.get())
    }
}

#[derive(Clone, Copy)]
pub(crate) struct RawStringSliceView<'a> {
    entries: &'a [String],
}

impl<'a> RawStringSliceView<'a> {
    fn new(entries: &'a [String]) -> Self {
        Self { entries }
    }

    pub(crate) fn iter(self) -> impl ExactSizeIterator<Item = &'a str> + DoubleEndedIterator {
        self.entries.iter().map(String::as_str)
    }
}

define_packed_arena_id!(NodeListId, OptionalNodeListId, NodeListRecord);
define_packed_arena_id!(ModifierListId, OptionalModifierListId, ModifierListRecord);
define_packed_arena_id!(RawNodeSliceId, OptionalRawNodeSliceId, RawNodeSliceRecord);
define_packed_arena_id!(
    RawStringSliceId,
    OptionalRawStringSliceId,
    RawStringSliceRecord
);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Node {
    store_id: StoreId,
    id: AstNodeId,
}

pub struct StoreNodeMap<T> {
    store_id: StoreId,
    values: Vec<Option<T>>,
}

pub struct NodeSideTable<T> {
    buckets: Vec<NodeSideTableBucket<T>>,
    entry_count: usize,
}

#[derive(Clone, Copy)]
pub struct NodeSideTableStore<'a, T> {
    store_id: StoreId,
    bucket: Option<&'a NodeSideTableBucket<T>>,
}

struct NodeSideTableBucket<T> {
    store_id: StoreId,
    values: Vec<Option<T>>,
    entry_count: usize,
}

impl<T> Default for NodeSideTable<T> {
    fn default() -> Self {
        Self {
            buckets: Vec::new(),
            entry_count: 0,
        }
    }
}

impl<T> NodeSideTable<T> {
    pub fn is_empty(&self) -> bool {
        self.entry_count == 0
    }

    pub fn clear(&mut self) {
        self.buckets.clear();
        self.entry_count = 0;
    }

    fn bucket_index(&self, store_id: StoreId) -> Result<usize, usize> {
        self.buckets
            .binary_search_by_key(&store_id, |bucket| bucket.store_id)
    }

    fn bucket(&self, store_id: StoreId) -> Option<&NodeSideTableBucket<T>> {
        self.bucket_index(store_id)
            .ok()
            .map(|index| &self.buckets[index])
    }

    fn bucket_mut(&mut self, store_id: StoreId) -> &mut NodeSideTableBucket<T> {
        let index = match self.bucket_index(store_id) {
            Ok(index) => index,
            Err(index) => {
                self.buckets.insert(
                    index,
                    NodeSideTableBucket {
                        store_id,
                        values: Vec::new(),
                        entry_count: 0,
                    },
                );
                index
            }
        };
        &mut self.buckets[index]
    }

    #[inline(always)]
    fn index(node: Node) -> usize {
        node.id().get() as usize
    }

    pub fn store(&self, store_id: StoreId) -> NodeSideTableStore<'_, T> {
        NodeSideTableStore {
            store_id,
            bucket: self.bucket(store_id),
        }
    }

    pub fn contains_key(&self, node: Node) -> bool {
        self.get(node).is_some()
    }

    pub fn get(&self, node: Node) -> Option<&T> {
        self.bucket(node.store_id())
            .and_then(|bucket| bucket.values.get(Self::index(node)))
            .and_then(Option::as_ref)
    }

    pub fn get_copied(&self, node: Node) -> Option<T>
    where
        T: Copy,
    {
        self.get(node).copied()
    }

    pub fn get_cloned(&self, node: Node) -> Option<T>
    where
        T: Clone,
    {
        self.get(node).cloned()
    }

    pub fn insert(&mut self, node: Node, value: T) -> Option<T> {
        let index = Self::index(node);
        let bucket = self.bucket_mut(node.store_id());
        if index >= bucket.values.len() {
            bucket.values.resize_with(index + 1, || None);
        }
        let previous = bucket.values[index].replace(value);
        if previous.is_none() {
            bucket.entry_count += 1;
            self.entry_count += 1;
        }
        previous
    }

    pub fn remove(&mut self, node: Node) -> Option<T> {
        let bucket_index = self.bucket_index(node.store_id()).ok()?;
        let bucket = &mut self.buckets[bucket_index];
        let removed = bucket
            .values
            .get_mut(Self::index(node))
            .and_then(Option::take);
        if removed.is_some() {
            bucket.entry_count -= 1;
            self.entry_count -= 1;
        }
        removed
    }

    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.buckets
            .iter()
            .flat_map(|bucket| bucket.values.iter().filter_map(Option::as_ref))
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.buckets
            .iter_mut()
            .flat_map(|bucket| bucket.values.iter_mut().filter_map(Option::as_mut))
    }

    pub fn for_each_value(&self, mut f: impl FnMut(&T)) {
        for bucket in &self.buckets {
            for value in &bucket.values {
                if let Some(value) = value.as_ref() {
                    f(value);
                }
            }
        }
    }

    pub fn for_each_value_mut(&mut self, mut f: impl FnMut(&mut T)) {
        for bucket in &mut self.buckets {
            for value in &mut bucket.values {
                if let Some(value) = value.as_mut() {
                    f(value);
                }
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (Node, &T)> {
        self.buckets.iter().flat_map(|bucket| {
            bucket
                .values
                .iter()
                .enumerate()
                .filter_map(move |(index, value)| {
                    let raw = NonZeroU32::new(u32::try_from(index).ok()?)?;
                    value
                        .as_ref()
                        .map(|value| (Node::new(bucket.store_id, AstNodeId(raw)), value))
                })
        })
    }

    pub fn for_each(&self, mut f: impl FnMut(Node, &T)) {
        for bucket in &self.buckets {
            for (index, value) in bucket.values.iter().enumerate() {
                let Some(value) = value.as_ref() else {
                    continue;
                };
                let Ok(index) = u32::try_from(index) else {
                    continue;
                };
                let Some(raw) = NonZeroU32::new(index) else {
                    continue;
                };
                f(Node::new(bucket.store_id, AstNodeId(raw)), value);
            }
        }
    }

    pub fn append(&mut self, other: &mut Self) {
        for other_bucket in other.buckets.drain(..) {
            let mut added = 0;
            let bucket = self.bucket_mut(other_bucket.store_id);
            if bucket.values.len() < other_bucket.values.len() {
                bucket
                    .values
                    .resize_with(other_bucket.values.len(), || None);
            }
            for (index, value) in other_bucket.values.into_iter().enumerate() {
                if let Some(value) = value {
                    if bucket.values[index].is_none() {
                        added += 1;
                    }
                    bucket.values[index] = Some(value);
                }
            }
            bucket.entry_count += added;
            self.entry_count += added;
        }
        other.entry_count = 0;
    }

    pub(crate) fn append_store_map(&mut self, other: &mut StoreNodeMap<T>) {
        let other_values = std::mem::take(&mut other.values);
        if other_values.is_empty() {
            return;
        }

        let bucket = self.bucket_mut(other.store_id);
        if bucket.values.len() < other_values.len() {
            bucket.values.resize_with(other_values.len(), || None);
        }
        let mut added = 0;
        for (index, value) in other_values.into_iter().enumerate() {
            if let Some(value) = value {
                if bucket.values[index].is_none() {
                    added += 1;
                }
                bucket.values[index] = Some(value);
            }
        }
        bucket.entry_count += added;
        self.entry_count += added;
    }
}

impl<'a, T> NodeSideTableStore<'a, T> {
    fn debug_assert_same_store(&self, node: Node) {
        debug_assert_eq!(
            node.store_id(),
            self.store_id,
            "node side table store view cannot index a node from another AST store"
        );
    }

    pub fn is_empty(&self) -> bool {
        self.bucket.map_or(true, |bucket| bucket.entry_count == 0)
    }

    pub fn get(&self, node: Node) -> Option<&'a T> {
        if node.store_id() != self.store_id {
            return None;
        }
        self.get_same_store(node)
    }

    pub fn get_same_store(&self, node: Node) -> Option<&'a T> {
        self.debug_assert_same_store(node);
        self.bucket
            .and_then(|bucket| bucket.values.get(NodeSideTable::<T>::index(node)))
            .and_then(Option::as_ref)
    }

    pub fn contains_key_same_store(&self, node: Node) -> bool {
        self.get_same_store(node).is_some()
    }

    pub fn get_copied(&self, node: Node) -> Option<T>
    where
        T: Copy,
    {
        self.get(node).copied()
    }

    pub fn get_cloned(&self, node: Node) -> Option<T>
    where
        T: Clone,
    {
        self.get(node).cloned()
    }

    pub fn get_copied_same_store(&self, node: Node) -> Option<T>
    where
        T: Copy,
    {
        self.get_same_store(node).copied()
    }

    pub fn get_cloned_same_store(&self, node: Node) -> Option<T>
    where
        T: Clone,
    {
        self.get_same_store(node).cloned()
    }

    pub fn values(&self) -> impl Iterator<Item = &'a T> {
        self.bucket
            .into_iter()
            .flat_map(|bucket| bucket.values.iter().filter_map(Option::as_ref))
    }

    pub fn iter(&self) -> impl Iterator<Item = (Node, &'a T)> {
        self.bucket.into_iter().flat_map(|bucket| {
            bucket
                .values
                .iter()
                .enumerate()
                .filter_map(move |(index, value)| {
                    let raw = NonZeroU32::new(u32::try_from(index).ok()?)?;
                    value
                        .as_ref()
                        .map(|value| (Node::new(bucket.store_id, AstNodeId(raw)), value))
                })
        })
    }
}

impl<T> StoreNodeMap<T> {
    fn with_capacity(store_id: StoreId, capacity: usize) -> Self {
        Self {
            store_id,
            values: Vec::with_capacity(capacity),
        }
    }

    #[inline]
    fn index(&self, node: Node) -> usize {
        assert_eq!(
            node.store_id(),
            self.store_id,
            "node map cannot index a node from another AST store"
        );
        Self::index_same_store(node)
    }

    #[inline(always)]
    fn index_same_store(node: Node) -> usize {
        node.id().get() as usize
    }

    pub fn contains_key(&self, node: Node) -> bool {
        self.get(node).is_some()
    }

    pub fn get(&self, node: Node) -> Option<&T> {
        self.values.get(self.index(node)).and_then(Option::as_ref)
    }

    pub fn get_same_store(&self, node: Node) -> Option<&T> {
        self.values
            .get(Self::index_same_store(node))
            .and_then(Option::as_ref)
    }

    pub fn get_mut(&mut self, node: Node) -> Option<&mut T> {
        let index = self.index(node);
        self.values.get_mut(index).and_then(Option::as_mut)
    }

    pub fn get_copied(&self, node: Node) -> Option<T>
    where
        T: Copy,
    {
        self.get(node).copied()
    }

    pub fn get_copied_same_store(&self, node: Node) -> Option<T>
    where
        T: Copy,
    {
        self.get_same_store(node).copied()
    }

    pub fn insert(&mut self, node: Node, value: T) -> Option<T> {
        let index = self.index(node);
        if index >= self.values.len() {
            self.values.resize_with(index + 1, || None);
        }
        self.values[index].replace(value)
    }

    pub fn insert_same_store(&mut self, node: Node, value: T) -> Option<T> {
        let index = Self::index_same_store(node);
        if index >= self.values.len() {
            self.values.resize_with(index + 1, || None);
        }
        self.values[index].replace(value)
    }

    pub fn get_or_insert_with_same_store(&mut self, node: Node, f: impl FnOnce() -> T) -> &mut T {
        let index = Self::index_same_store(node);
        if index >= self.values.len() {
            self.values.resize_with(index + 1, || None);
        }
        self.values[index].get_or_insert_with(f)
    }

    pub fn get_or_insert_with(&mut self, node: Node, f: impl FnOnce() -> T) -> &mut T {
        let index = self.index(node);
        if index >= self.values.len() {
            self.values.resize_with(index + 1, || None);
        }
        self.values[index].get_or_insert_with(f)
    }

    pub fn remove(&mut self, node: Node) -> Option<T> {
        let index = self.index(node);
        self.values.get_mut(index).and_then(Option::take)
    }

    pub fn remove_same_store(&mut self, node: Node) -> Option<T> {
        let index = Self::index_same_store(node);
        self.values.get_mut(index).and_then(Option::take)
    }
}

pub type Declaration = Node;
pub type DeclarationName = Node;
pub type IdentifierNode = Node;
pub type ImportDeclarationNode = Node;
pub type ExportDeclarationNode = Node;
pub type ExportSpecifierNode = Node;
pub type PropertyName = Node;
pub type Statement = Node;
pub type TokenNode = Node;
pub type TypeNode = Node;
pub type TypePredicateNodeNode = Node;
pub type ConditionalTypeNodeNode = Node;
pub type JsxAttributeLike = Node;
pub type CallLikeExpression = Node;
pub type ClassLikeDeclaration = Node;

impl Hash for Node {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.store_id.hash(state);
        self.id.hash(state);
    }
}

impl Node {
    #[inline(always)]
    pub(crate) fn new(store_id: StoreId, id: AstNodeId) -> Self {
        Self { store_id, id }
    }

    #[inline(always)]
    pub fn store_id(self) -> StoreId {
        self.store_id
    }

    #[inline(always)]
    pub(crate) fn id(self) -> AstNodeId {
        self.id
    }
}

macro_rules! define_ast_payload_storage {
    (SourceFile => $source_field:ident : $source_ty:ty, $( $variant:ident => $field:ident : $ty:ty, )*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub(crate) enum NodePayloadTag {
            SourceFile,
            $( $variant, )*
        }

        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub(crate) struct NodePayloadId {
            tag: NodePayloadTag,
            raw: RawIdx,
        }

        impl NodePayloadId {
            pub(crate) const fn new(tag: NodePayloadTag, raw: RawIdx) -> Self {
                Self { tag, raw }
            }

            pub(crate) const fn tag(self) -> NodePayloadTag {
                self.tag
            }

            pub(crate) const fn raw(self) -> RawIdx {
                self.raw
            }
        }

        #[derive(Default)]
        pub struct AstPayloads {
            pub(crate) $source_field: Arena<$source_ty>,
            $( pub(crate) $field: Arena<$ty>, )*
        }

        impl AstPayloads {
            pub(crate) fn $source_field(&self, id: NodePayloadId) -> &$source_ty {
                assert_eq!(id.tag, NodePayloadTag::SourceFile, "payload tag mismatch");
                &self.$source_field[Idx::from_raw(id.raw)]
            }

            $(
                pub(crate) fn $field(&self, id: NodePayloadId) -> &$ty {
                    assert_eq!(id.tag, NodePayloadTag::$variant, "payload tag mismatch");
                    &self.$field[Idx::from_raw(id.raw)]
                }

            )*

            pub(crate) fn clone_payload(&mut self, id: NodePayloadId) -> NodePayloadId {
                match id.tag {
                    NodePayloadTag::SourceFile => {
                        panic!("SourceFile payloads cannot be shallow-cloned");
                    }
                    $(
                        NodePayloadTag::$variant => {
                            let cloned = self.$field[Idx::from_raw(id.raw)].clone();
                            let raw = self.$field.alloc(cloned).into_raw();
                            NodePayloadId::new(NodePayloadTag::$variant, raw)
                        }
                    )*
                }
            }
        }
    };
}

define_ast_payload_storage! {
    SourceFile => source_file: SourceFileData,
    Token => token: Token,
    Identifier => identifier: Identifier,
    PrivateIdentifier => private_identifier: PrivateIdentifier,
    QualifiedName => qualified_name: QualifiedName,
    ComputedPropertyName => computed_property_name: ComputedPropertyName,
    Decorator => decorator: Decorator,
    EmptyStatement => empty_statement: EmptyStatement,
    IfStatement => if_statement: IfStatement,
    DoStatement => do_statement: DoStatement,
    WhileStatement => while_statement: WhileStatement,
    ForStatement => for_statement: ForStatement,
    ForInOrOfStatement => for_in_or_of_statement: ForInOrOfStatement,
    BreakStatement => break_statement: BreakStatement,
    ContinueStatement => continue_statement: ContinueStatement,
    ReturnStatement => return_statement: ReturnStatement,
    WithStatement => with_statement: WithStatement,
    SwitchStatement => switch_statement: SwitchStatement,
    CaseBlock => case_block: CaseBlock,
    CaseOrDefaultClause => case_or_default_clause: CaseOrDefaultClause,
    ThrowStatement => throw_statement: ThrowStatement,
    TryStatement => try_statement: TryStatement,
    CatchClause => catch_clause: CatchClause,
    DebuggerStatement => debugger_statement: DebuggerStatement,
    LabeledStatement => labeled_statement: LabeledStatement,
    ExpressionStatement => expression_statement: ExpressionStatement,
    Block => block: Block,
    VariableStatement => variable_statement: VariableStatement,
    VariableDeclaration => variable_declaration: VariableDeclaration,
    VariableDeclarationList => variable_declaration_list: VariableDeclarationList,
    BindingPattern => binding_pattern: BindingPattern,
    ParameterDeclaration => parameter_declaration: ParameterDeclaration,
    BindingElement => binding_element: BindingElement,
    MissingDeclaration => missing_declaration: MissingDeclaration,
    FunctionDeclaration => function_declaration: FunctionDeclaration,
    ClassDeclaration => class_declaration: ClassDeclaration,
    ClassExpression => class_expression: ClassExpression,
    HeritageClause => heritage_clause: HeritageClause,
    InterfaceDeclaration => interface_declaration: InterfaceDeclaration,
    TypeAliasDeclaration => type_alias_declaration: TypeAliasDeclaration,
    EnumMember => enum_member: EnumMember,
    EnumDeclaration => enum_declaration: EnumDeclaration,
    ModuleBlock => module_block: ModuleBlock,
    NotEmittedStatement => not_emitted_statement: NotEmittedStatement,
    NotEmittedTypeElement => not_emitted_type_element: NotEmittedTypeElement,
    ImportDeclaration => import_declaration: ImportDeclaration,
    ExternalModuleReference => external_module_reference: ExternalModuleReference,
    NamespaceImport => namespace_import: NamespaceImport,
    NamedImports => named_imports: NamedImports,
    ExportAssignment => export_assignment: ExportAssignment,
    NamespaceExportDeclaration => namespace_export_declaration: NamespaceExportDeclaration,
    NamespaceExport => namespace_export: NamespaceExport,
    NamedExports => named_exports: NamedExports,
    ExportSpecifier => export_specifier: ExportSpecifier,
    CallSignatureDeclaration => call_signature_declaration: CallSignatureDeclaration,
    ConstructSignatureDeclaration => construct_signature_declaration: ConstructSignatureDeclaration,
    ConstructorDeclaration => constructor_declaration: ConstructorDeclaration,
    GetAccessorDeclaration => get_accessor_declaration: GetAccessorDeclaration,
    SetAccessorDeclaration => set_accessor_declaration: SetAccessorDeclaration,
    IndexSignatureDeclaration => index_signature_declaration: IndexSignatureDeclaration,
    MethodSignatureDeclaration => method_signature_declaration: MethodSignatureDeclaration,
    MethodDeclaration => method_declaration: MethodDeclaration,
    PropertySignatureDeclaration => property_signature_declaration: PropertySignatureDeclaration,
    PropertyDeclaration => property_declaration: PropertyDeclaration,
    SemicolonClassElement => semicolon_class_element: SemicolonClassElement,
    ClassStaticBlockDeclaration => class_static_block_declaration: ClassStaticBlockDeclaration,
    OmittedExpression => omitted_expression: OmittedExpression,
    KeywordExpression => keyword_expression: KeywordExpression,
    StringLiteral => string_literal: StringLiteral,
    NumericLiteral => numeric_literal: NumericLiteral,
    BigIntLiteral => big_int_literal: BigIntLiteral,
    RegularExpressionLiteral => regular_expression_literal: RegularExpressionLiteral,
    NoSubstitutionTemplateLiteral => no_substitution_template_literal: NoSubstitutionTemplateLiteral,
    BinaryExpression => binary_expression: BinaryExpression,
    PrefixUnaryExpression => prefix_unary_expression: PrefixUnaryExpression,
    PostfixUnaryExpression => postfix_unary_expression: PostfixUnaryExpression,
    YieldExpression => yield_expression: YieldExpression,
    ArrowFunction => arrow_function: ArrowFunction,
    FunctionExpression => function_expression: FunctionExpression,
    AsExpression => as_expression: AsExpression,
    SatisfiesExpression => satisfies_expression: SatisfiesExpression,
    ConditionalExpression => conditional_expression: ConditionalExpression,
    PropertyAccessExpression => property_access_expression: PropertyAccessExpression,
    ElementAccessExpression => element_access_expression: ElementAccessExpression,
    CallExpression => call_expression: CallExpression,
    NewExpression => new_expression: NewExpression,
    MetaProperty => meta_property: MetaProperty,
    NonNullExpression => non_null_expression: NonNullExpression,
    SpreadElement => spread_element: SpreadElement,
    TemplateExpression => template_expression: TemplateExpression,
    TemplateSpan => template_span: TemplateSpan,
    TaggedTemplateExpression => tagged_template_expression: TaggedTemplateExpression,
    ParenthesizedExpression => parenthesized_expression: ParenthesizedExpression,
    ArrayLiteralExpression => array_literal_expression: ArrayLiteralExpression,
    ObjectLiteralExpression => object_literal_expression: ObjectLiteralExpression,
    SpreadAssignment => spread_assignment: SpreadAssignment,
    PropertyAssignment => property_assignment: PropertyAssignment,
    ShorthandPropertyAssignment => shorthand_property_assignment: ShorthandPropertyAssignment,
    DeleteExpression => delete_expression: DeleteExpression,
    TypeOfExpression => type_of_expression: TypeOfExpression,
    VoidExpression => void_expression: VoidExpression,
    AwaitExpression => await_expression: AwaitExpression,
    TypeAssertion => type_assertion: TypeAssertion,
    KeywordTypeNode => keyword_type_node: KeywordTypeNode,
    UnionTypeNode => union_type_node: UnionTypeNode,
    IntersectionTypeNode => intersection_type_node: IntersectionTypeNode,
    ConditionalTypeNode => conditional_type_node: ConditionalTypeNode,
    TypeOperatorNode => type_operator_node: TypeOperatorNode,
    InferTypeNode => infer_type_node: InferTypeNode,
    ArrayTypeNode => array_type_node: ArrayTypeNode,
    IndexedAccessTypeNode => indexed_access_type_node: IndexedAccessTypeNode,
    TypeReferenceNode => type_reference_node: TypeReferenceNode,
    ExpressionWithTypeArguments => expression_with_type_arguments: ExpressionWithTypeArguments,
    LiteralTypeNode => literal_type_node: LiteralTypeNode,
    ThisTypeNode => this_type_node: ThisTypeNode,
    TypePredicateNode => type_predicate_node: TypePredicateNode,
    ImportAttribute => import_attribute: ImportAttribute,
    ImportAttributes => import_attributes: ImportAttributes,
    TypeQueryNode => type_query_node: TypeQueryNode,
    MappedTypeNode => mapped_type_node: MappedTypeNode,
    TypeLiteralNode => type_literal_node: TypeLiteralNode,
    TupleTypeNode => tuple_type_node: TupleTypeNode,
    NamedTupleMember => named_tuple_member: NamedTupleMember,
    OptionalTypeNode => optional_type_node: OptionalTypeNode,
    RestTypeNode => rest_type_node: RestTypeNode,
    ParenthesizedTypeNode => parenthesized_type_node: ParenthesizedTypeNode,
    FunctionTypeNode => function_type_node: FunctionTypeNode,
    ConstructorTypeNode => constructor_type_node: ConstructorTypeNode,
    TemplateHead => template_head: TemplateHead,
    TemplateMiddle => template_middle: TemplateMiddle,
    TemplateTail => template_tail: TemplateTail,
    TemplateLiteralTypeNode => template_literal_type_node: TemplateLiteralTypeNode,
    TemplateLiteralTypeSpan => template_literal_type_span: TemplateLiteralTypeSpan,
    SyntheticExpression => synthetic_expression: SyntheticExpression,
    PartiallyEmittedExpression => partially_emitted_expression: PartiallyEmittedExpression,
    JsxElement => jsx_element: JsxElement,
    JsxAttributes => jsx_attributes: JsxAttributes,
    JsxNamespacedName => jsx_namespaced_name: JsxNamespacedName,
    JsxOpeningElement => jsx_opening_element: JsxOpeningElement,
    JsxSelfClosingElement => jsx_self_closing_element: JsxSelfClosingElement,
    JsxFragment => jsx_fragment: JsxFragment,
    JsxOpeningFragment => jsx_opening_fragment: JsxOpeningFragment,
    JsxClosingFragment => jsx_closing_fragment: JsxClosingFragment,
    JsxAttribute => jsx_attribute: JsxAttribute,
    JsxSpreadAttribute => jsx_spread_attribute: JsxSpreadAttribute,
    JsxClosingElement => jsx_closing_element: JsxClosingElement,
    JsxExpression => jsx_expression: JsxExpression,
    JsxText => jsx_text: JsxText,
    SyntaxList => syntax_list: SyntaxList,
    ModuleDeclaration => module_declaration: ModuleDeclaration,
    ImportEqualsDeclaration => import_equals_declaration: ImportEqualsDeclaration,
    ExportDeclaration => export_declaration: ExportDeclaration,
    ImportTypeNode => import_type_node: ImportTypeNode,
    ImportClause => import_clause: ImportClause,
    ImportSpecifier => import_specifier: ImportSpecifier,
    TypeParameterDeclaration => type_parameter_declaration: TypeParameterDeclaration,
    SyntheticReferenceExpression => synthetic_reference_expression: SyntheticReferenceExpression,
}

impl AstPayloads {
    pub(crate) fn source_file_mut(&mut self, id: NodePayloadId) -> &mut SourceFileData {
        assert_eq!(id.tag(), NodePayloadTag::SourceFile, "payload tag mismatch");
        &mut self.source_file[Idx::from_raw(id.raw())]
    }
}

pub(crate) struct NodeHeader {
    pub(crate) kind: Kind,
    pub(crate) flags: NodeFlags,
    pub(crate) loc: core::TextRange,
    pub(crate) parent: OptionalAstNodeId,
    pub(crate) payload: NodePayloadId,
    pub(crate) node_id: AtomicU64,
}

impl Clone for NodeHeader {
    fn clone(&self) -> Self {
        Self {
            kind: self.kind,
            flags: self.flags,
            loc: self.loc,
            parent: self.parent,
            payload: self.payload,
            node_id: AtomicU64::new(0),
        }
    }
}

impl NodeHeader {
    pub(crate) fn new(
        kind: Kind,
        flags: NodeFlags,
        loc: core::TextRange,
        payload: NodePayloadId,
    ) -> Self {
        Self {
            kind,
            flags,
            loc,
            parent: OptionalAstNodeId::none(),
            payload,
            node_id: AtomicU64::new(0),
        }
    }
}

#[derive(Default)]
pub struct AstLists {
    node_lists: Arena<NodeListRecord>,
    node_list_entries: Arena<NodeListEntry>,
    modifier_lists: Arena<ModifierListRecord>,
    raw_node_slices: Arena<RawNodeSliceRecord>,
    raw_node_slice_entries: Arena<RawNodeSliceEntry>,
    raw_string_slices: Arena<RawStringSliceRecord>,
    raw_string_slice_entries: Arena<String>,
    foreign_node_entries: usize,
}

impl AstLists {
    pub(crate) fn alloc_node_list(
        &mut self,
        store_id: StoreId,
        loc: core::TextRange,
        range: core::TextRange,
        nodes: impl IntoIterator<Item = Node>,
        missing: bool,
        has_trailing_comma: bool,
    ) -> NodeListId {
        let mut foreign_node_entries = 0;
        let entries = self
            .node_list_entries
            .alloc_many(nodes.into_iter().map(|node| {
                if is_foreign_node(store_id, node) {
                    foreign_node_entries += 1;
                }
                NodeListEntry { node }
            }));
        self.foreign_node_entries += foreign_node_entries;
        NodeListId::from_idx(
            store_id,
            self.node_lists.alloc(NodeListRecord {
                loc,
                range,
                entries,
                missing,
                has_trailing_comma,
            }),
        )
    }

    pub(crate) fn alloc_modifier_list(
        &mut self,
        store_id: StoreId,
        list: NodeListId,
        modifier_flags: ModifierFlags,
    ) -> ModifierListId {
        ModifierListId::from_idx(
            store_id,
            self.modifier_lists.alloc(ModifierListRecord {
                list,
                modifier_flags,
            }),
        )
    }

    pub(crate) fn alloc_raw_node_slice(
        &mut self,
        store_id: StoreId,
        nodes: impl IntoIterator<Item = OptionalAstNodeId>,
    ) -> RawNodeSliceId {
        let mut foreign_node_entries = 0;
        let entries = self
            .raw_node_slice_entries
            .alloc_many(nodes.into_iter().map(|node| {
                if let Some(node) = node.get()
                    && is_foreign_node(store_id, node)
                {
                    foreign_node_entries += 1;
                }
                RawNodeSliceEntry { node }
            }));
        self.foreign_node_entries += foreign_node_entries;
        RawNodeSliceId::from_idx(
            store_id,
            self.raw_node_slices.alloc(RawNodeSliceRecord { entries }),
        )
    }

    pub(crate) fn alloc_raw_string_slice(
        &mut self,
        store_id: StoreId,
        strings: impl IntoIterator<Item = String>,
    ) -> RawStringSliceId {
        let entries = self.raw_string_slice_entries.alloc_many(strings);
        RawStringSliceId::from_idx(
            store_id,
            self.raw_string_slices
                .alloc(RawStringSliceRecord { entries }),
        )
    }
}

#[derive(Default)]
pub struct AstSemantics;

#[derive(Default)]
struct NodeIdAllocator {
    next: u64,
    end: u64,
}

impl NodeIdAllocator {
    fn allocate(&mut self) -> crate::ids::NodeId {
        if self.next == self.end {
            let start = NEXT_AST_NODE_ID.fetch_add(NODE_ID_BLOCK_SIZE, Ordering::Relaxed) + 1;
            self.next = start;
            self.end = start + NODE_ID_BLOCK_SIZE;
        }

        let id = self.next;
        self.next += 1;
        id
    }
}

pub struct AstStore {
    store_id: StoreId,
    self_weak: Mutex<Weak<AstStore>>,
    node_ids: Mutex<NodeIdAllocator>,
    nodes: Arena<NodeHeader>,
    payloads: AstPayloads,
    lists: AstLists,
    synthetic_parents: NodeSideTable<Node>,
    synthetic_parent_revision: u64,
    _semantics: AstSemantics,
}

impl AsRef<AstStore> for AstStore {
    fn as_ref(&self) -> &AstStore {
        self
    }
}

impl AstStore {
    pub fn new() -> Self {
        Self {
            store_id: StoreId::default(),
            self_weak: Mutex::new(Weak::new()),
            node_ids: Mutex::new(NodeIdAllocator::default()),
            nodes: Arena::new(),
            payloads: AstPayloads::default(),
            lists: AstLists::default(),
            synthetic_parents: NodeSideTable::default(),
            synthetic_parent_revision: 0,
            _semantics: AstSemantics,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            store_id: StoreId::default(),
            self_weak: Mutex::new(Weak::new()),
            node_ids: Mutex::new(NodeIdAllocator::default()),
            nodes: Arena::with_capacity(capacity),
            payloads: AstPayloads::default(),
            lists: AstLists::default(),
            synthetic_parents: NodeSideTable::default(),
            synthetic_parent_revision: 0,
            _semantics: AstSemantics,
        }
    }

    #[inline(always)]
    pub fn store_id(&self) -> StoreId {
        self.store_id
    }

    pub fn new_node_map<T>(&self) -> StoreNodeMap<T> {
        StoreNodeMap::with_capacity(self.store_id, 0)
    }

    pub(crate) fn new_node_map_with_capacity<T>(&self, capacity: usize) -> StoreNodeMap<T> {
        StoreNodeMap::with_capacity(self.store_id, capacity)
    }

    pub(crate) fn set_self_weak(&mut self, self_weak: Weak<AstStore>) {
        *self.self_weak.get_mut().unwrap() = self_weak;
    }

    pub(crate) fn self_weak(&self) -> Weak<AstStore> {
        self.self_weak.lock().unwrap().clone()
    }

    pub(crate) fn payloads(&self) -> &AstPayloads {
        &self.payloads
    }

    pub(crate) fn payloads_mut(&mut self) -> &mut AstPayloads {
        &mut self.payloads
    }

    pub(crate) fn local_node_id(&self, node: Node) -> Node {
        self.assert_same_store(node);
        node
    }

    pub(crate) fn optional_local_node_id(&self, node: Option<Node>) -> OptionalAstNodeId {
        OptionalAstNodeId::from_option(node.map(|node| self.local_node_id(node)))
    }

    pub(crate) fn alloc_node_list(
        &mut self,
        loc: core::TextRange,
        range: core::TextRange,
        nodes: impl IntoIterator<Item = Node>,
    ) -> NodeListId {
        let store_id = self.store_id;
        let nodes = nodes.into_iter().map(move |node| {
            assert_eq!(
                node.store_id(),
                store_id,
                "node belongs to a different AST store"
            );
            node
        });
        self.lists
            .alloc_node_list(store_id, loc, range, nodes, false, false)
    }

    pub(crate) fn alloc_node_list_with_trailing_comma(
        &mut self,
        loc: core::TextRange,
        range: core::TextRange,
        nodes: impl IntoIterator<Item = Node>,
        has_trailing_comma: bool,
    ) -> NodeListId {
        let store_id = self.store_id;
        let nodes = nodes.into_iter().map(move |node| {
            assert_eq!(
                node.store_id(),
                store_id,
                "node belongs to a different AST store"
            );
            node
        });
        self.lists
            .alloc_node_list(store_id, loc, range, nodes, false, has_trailing_comma)
    }

    pub(crate) fn alloc_missing_node_list(
        &mut self,
        loc: core::TextRange,
        range: core::TextRange,
    ) -> NodeListId {
        self.lists
            .alloc_node_list(self.store_id, loc, range, std::iter::empty(), true, false)
    }

    pub(crate) fn alloc_modifier_list(
        &mut self,
        loc: core::TextRange,
        range: core::TextRange,
        modifiers: impl IntoIterator<Item = Node>,
        modifier_flags: ModifierFlags,
    ) -> ModifierListId {
        let list = self.alloc_node_list(loc, range, modifiers);
        self.lists
            .alloc_modifier_list(self.store_id, list, modifier_flags)
    }

    pub(crate) fn alloc_raw_node_slice(
        &mut self,
        nodes: impl IntoIterator<Item = Option<Node>>,
    ) -> RawNodeSliceId {
        let store_id = self.store_id;
        let nodes = nodes.into_iter().map(move |node| {
            OptionalAstNodeId::from_option(node.map(|node| {
                assert_eq!(
                    node.store_id(),
                    store_id,
                    "node belongs to a different AST store"
                );
                node
            }))
        });
        self.lists.alloc_raw_node_slice(store_id, nodes)
    }

    pub(crate) fn alloc_raw_string_slice(
        &mut self,
        strings: impl IntoIterator<Item = impl Into<String>>,
    ) -> RawStringSliceId {
        self.lists
            .alloc_raw_string_slice(self.store_id, strings.into_iter().map(Into::into))
    }

    pub(crate) fn foreign_nodes_in_aggregate_storage(&self) -> Vec<Node> {
        if self.lists.foreign_node_entries == 0 {
            return Vec::new();
        }

        let mut nodes = Vec::new();
        for list in self.lists.node_lists.values() {
            for entry in &self.lists.node_list_entries[list.entries.clone()] {
                if is_foreign_node(self.store_id, entry.node) {
                    nodes.push(entry.node);
                }
            }
        }
        for slice in self.lists.raw_node_slices.values() {
            for entry in &self.lists.raw_node_slice_entries[slice.entries.clone()] {
                if let Some(node) = entry.node.get()
                    && node.store_id() != self.store_id
                {
                    nodes.push(node);
                }
            }
        }
        nodes
    }

    fn count_foreign_nodes_in_aggregate_storage(&self) -> usize {
        self.lists
            .node_lists
            .values()
            .map(|list| {
                self.lists.node_list_entries[list.entries.clone()]
                    .iter()
                    .filter(|entry| is_foreign_node(self.store_id, entry.node))
                    .count()
            })
            .sum::<usize>()
            + self
                .lists
                .raw_node_slices
                .values()
                .map(|slice| {
                    self.lists.raw_node_slice_entries[slice.entries.clone()]
                        .iter()
                        .filter(|entry| {
                            entry
                                .node
                                .get()
                                .is_some_and(|node| is_foreign_node(self.store_id, node))
                        })
                        .count()
                })
                .sum::<usize>()
    }

    pub(crate) fn replace_aggregate_nodes(&mut self, replacements: &NodeSideTable<Node>) {
        let mut replaced_foreign_nodes = 0;
        let node_list_ranges = self
            .lists
            .node_lists
            .values()
            .map(|list| list.entries.clone())
            .collect::<Vec<_>>();
        for range in node_list_ranges {
            for idx in range {
                let entry = &mut self.lists.node_list_entries[idx];
                if entry.node.store_id() == self.store_id {
                    continue;
                }
                if let Some(replacement) = replacements.get_copied(entry.node) {
                    assert_eq!(replacement.store_id(), self.store_id);
                    entry.node = replacement;
                    replaced_foreign_nodes += 1;
                }
            }
        }

        let raw_slice_ranges = self
            .lists
            .raw_node_slices
            .values()
            .map(|slice| slice.entries.clone())
            .collect::<Vec<_>>();
        for range in raw_slice_ranges {
            for idx in range {
                let entry = &mut self.lists.raw_node_slice_entries[idx];
                if let Some(node) = entry.node.get() {
                    if node.store_id() == self.store_id {
                        continue;
                    }
                    if let Some(replacement) = replacements.get_copied(node) {
                        assert_eq!(replacement.store_id(), self.store_id);
                        entry.node = OptionalAstNodeId::some(replacement);
                        replaced_foreign_nodes += 1;
                    }
                }
            }
        }

        if replaced_foreign_nodes > self.lists.foreign_node_entries {
            self.lists.foreign_node_entries = self.count_foreign_nodes_in_aggregate_storage();
        } else {
            self.lists.foreign_node_entries -= replaced_foreign_nodes;
        }
    }

    pub(crate) fn alloc_header(
        &mut self,
        kind: Kind,
        flags: NodeFlags,
        loc: core::TextRange,
        payload: NodePayloadId,
    ) -> Node {
        assert!(
            payload.matches_kind(kind),
            "payload {:?} is not valid for kind {:?}",
            payload.tag(),
            kind
        );
        let raw = self.nodes.alloc(NodeHeader::new(kind, flags, loc, payload));
        Node::new(self.store_id, AstNodeId::from_idx(raw))
    }

    pub(crate) fn shallow_clone_node(&mut self, node: Node) -> Node {
        self.assert_same_store(node);
        let (kind, flags, loc, payload) = {
            let header = self.header(node);
            (
                header.kind,
                header.flags,
                header.loc,
                self.payloads.clone_payload(header.payload),
            )
        };
        self.alloc_header(kind, flags, loc, payload)
    }

    #[inline(always)]
    pub(crate) fn header(&self, node: Node) -> &NodeHeader {
        self.assert_same_store(node);
        &self.nodes[node.id().to_idx()]
    }

    #[inline(always)]
    pub(crate) fn header_mut(&mut self, node: Node) -> &mut NodeHeader {
        self.assert_same_store(node);
        &mut self.nodes[node.id().to_idx()]
    }

    #[inline(always)]
    pub fn kind(&self, node: Node) -> Kind {
        self.header(node).kind
    }

    #[inline(always)]
    pub fn flags(&self, node: Node) -> NodeFlags {
        self.header(node).flags
    }

    #[inline(always)]
    pub fn loc(&self, node: Node) -> core::TextRange {
        self.header(node).loc
    }

    pub(crate) fn set_loc(&mut self, node: Node, loc: core::TextRange) {
        self.assert_same_store(node);
        self.header_mut(node).loc = loc;
    }

    pub(crate) fn set_flags(&mut self, node: Node, flags: NodeFlags) {
        self.assert_same_store(node);
        self.header_mut(node).flags = flags;
    }

    pub(crate) fn add_flags(&mut self, node: Node, flags: NodeFlags) {
        self.assert_same_store(node);
        self.header_mut(node).flags |= flags;
    }

    pub(crate) fn remove_flags(&mut self, node: Node, flags: NodeFlags) {
        self.assert_same_store(node);
        self.header_mut(node).flags &= !flags;
    }

    pub fn get_node_id(&self, node: Node) -> crate::ids::NodeId {
        self.assert_same_store(node);
        let id = &self.header(node).node_id;
        let mut current = id.load(Ordering::Acquire);
        if current == 0 {
            let mut node_ids = self.node_ids.lock().unwrap();
            current = id.load(Ordering::Acquire);
            if current == 0 {
                current = node_ids.allocate();
                id.store(current, Ordering::Release);
            }
        }
        current
    }

    pub fn as_source_file(&self, node: Node) -> &SourceFileData {
        self.payloads().source_file(self.header(node).payload)
    }

    pub(crate) fn as_source_file_mut(&mut self, node: Node) -> &mut SourceFileData {
        let payload = self.header(node).payload;
        self.payloads_mut().source_file_mut(payload)
    }

    pub(crate) fn node_from_id(&self, id: impl RequiredAstNodeId) -> Node {
        id.required_node(self.store_id)
    }

    pub(crate) fn optional_node_from_id(&self, id: OptionalAstNodeId) -> Option<Node> {
        id.get()
    }

    pub(crate) fn node_list(&self, id: NodeListId) -> NodeListView<'_> {
        id.assert_store(self.store_id);
        let record = &self.lists.node_lists[id.to_idx()];
        NodeListView::new(
            id,
            record.loc,
            record.range,
            record.missing,
            record.has_trailing_comma,
            &self.lists.node_list_entries[record.entries.clone()],
        )
    }

    pub(crate) fn optional_node_list(&self, id: OptionalNodeListId) -> Option<NodeListView<'_>> {
        id.get().map(|id| self.node_list(id))
    }

    pub(crate) fn modifier_list(&self, id: ModifierListId) -> ModifierListView<'_> {
        id.assert_store(self.store_id);
        let record = &self.lists.modifier_lists[id.to_idx()];
        ModifierListView::new(id, self.node_list(record.list), record.modifier_flags)
    }

    pub(crate) fn optional_modifier_list(
        &self,
        id: OptionalModifierListId,
    ) -> Option<ModifierListView<'_>> {
        id.get().map(|id| self.modifier_list(id))
    }

    pub(crate) fn raw_node_slice(&self, id: RawNodeSliceId) -> RawNodeSliceView<'_> {
        id.assert_store(self.store_id);
        let record = &self.lists.raw_node_slices[id.to_idx()];
        RawNodeSliceView::new(&self.lists.raw_node_slice_entries[record.entries.clone()])
    }

    pub(crate) fn optional_raw_node_slice(
        &self,
        id: OptionalRawNodeSliceId,
    ) -> Option<RawNodeSliceView<'_>> {
        id.get().map(|id| self.raw_node_slice(id))
    }

    pub(crate) fn raw_string_slice(&self, id: RawStringSliceId) -> RawStringSliceView<'_> {
        id.assert_store(self.store_id);
        let record = &self.lists.raw_string_slices[id.to_idx()];
        RawStringSliceView::new(&self.lists.raw_string_slice_entries[record.entries.clone()])
    }

    pub(crate) fn optional_raw_string_slice(
        &self,
        id: OptionalRawStringSliceId,
    ) -> Option<RawStringSliceView<'_>> {
        id.get().map(|id| self.raw_string_slice(id))
    }

    pub fn parent(&self, node: Node) -> Option<Node> {
        if let Some(parent) = self.synthetic_parents.get_copied(node) {
            return Some(parent);
        }
        self.header(node).parent.get()
    }

    pub fn original_parent(&self, node: Node) -> Option<Node> {
        self.header(node).parent.get()
    }

    pub fn synthetic_parent_revision(&self) -> u64 {
        self.synthetic_parent_revision
    }

    pub(crate) fn set_parent(&mut self, node: Node, parent: Option<Node>) {
        self.assert_same_store(node);
        if let Some(parent) = parent {
            self.assert_same_store(parent);
        }
        self.header_mut(node).parent = OptionalAstNodeId::from_option(parent);
    }

    pub(crate) fn set_synthetic_parent(&mut self, node: Node, parent: Option<Node>) {
        self.assert_same_store(node);
        let previous = if let Some(parent) = parent {
            self.synthetic_parents.insert(node, parent)
        } else {
            self.synthetic_parents.remove(node)
        };
        if previous != parent {
            self.synthetic_parent_revision = self.synthetic_parent_revision.wrapping_add(1);
        }
    }

    pub fn for_each_present_child<F>(&self, node: Node, mut visitor: F) -> ControlFlow<()>
    where
        F: FnMut(Node) -> ControlFlow<()>,
    {
        self.for_each_child(node, |child| {
            let Some(child) = child else {
                return ControlFlow::Continue(());
            };
            visitor(child)
        })
    }

    pub(crate) fn set_parent_in_children(&mut self, parent: Node) {
        self.assert_same_store(parent);
        let mut children = SmallVec::<[Node; 8]>::new();
        let result = self.for_each_present_child(parent, |child| {
            children.push(child);
            ControlFlow::Continue(())
        });
        debug_assert_eq!(result, ControlFlow::Continue(()));
        for child in children {
            self.set_parent(child, Some(parent));
        }
    }

    pub(crate) fn set_parent_recursive(&mut self, parent: Node) {
        self.assert_same_store(parent);
        let mut stack = vec![parent];
        while let Some(parent) = stack.pop() {
            let mut children = SmallVec::<[Node; 8]>::new();
            let result = self.for_each_present_child(parent, |child| {
                children.push(child);
                ControlFlow::Continue(())
            });
            debug_assert_eq!(result, ControlFlow::Continue(()));
            for child in children {
                self.set_parent(child, Some(parent));
                stack.push(child);
            }
        }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    #[inline(always)]
    pub(crate) fn assert_same_store(&self, node: Node) {
        assert_eq!(
            node.store_id(),
            self.store_id,
            "node belongs to a different AST store"
        );
    }
}

impl Default for AstStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alloc_token(store: &mut AstStore) -> Node {
        let raw = store.payloads_mut().token.alloc(Token {}).into_raw();
        store.alloc_header(
            Kind::Unknown,
            NodeFlags::NONE,
            core::undefined_text_range(),
            NodePayloadId::new(NodePayloadTag::Token, raw),
        )
    }

    #[test]
    fn new_node_map_should_start_empty_even_when_store_has_nodes() {
        let mut store = AstStore::new();
        let node = alloc_token(&mut store);

        let map = store.new_node_map::<usize>();

        assert_eq!(store.len(), 1);
        assert!(map.values.is_empty());
        assert!(!map.contains_key(node));
    }

    #[test]
    fn new_node_map_with_capacity_should_reserve_without_entries() {
        let store = AstStore::new();

        let map = store.new_node_map_with_capacity::<usize>(16);

        assert!(map.values.is_empty());
        assert!(map.values.capacity() >= 16);
    }

    #[test]
    fn node_map_insert_and_get_or_insert_should_grow_lazily() {
        let mut store = AstStore::new();
        let first = alloc_token(&mut store);
        let second = alloc_token(&mut store);
        let mut map = store.new_node_map::<usize>();

        assert_eq!(map.insert(second, 20), None);
        assert_eq!(map.get_copied(second), Some(20));
        assert_eq!(map.get_copied(first), None);

        *map.get_or_insert_with(first, || 10) += 1;

        assert_eq!(map.get_copied(first), Some(11));
        assert_eq!(map.get_copied(second), Some(20));
    }

    #[test]
    fn node_side_table_store_view_should_only_read_one_store_bucket() {
        let mut store = AstStore::new();
        let first = alloc_token(&mut store);
        let second = alloc_token(&mut store);
        let mut other_store = AstStore::new();
        let foreign = alloc_token(&mut other_store);
        let mut table = NodeSideTable::default();

        table.insert(second, 20);
        table.insert(foreign, 30);
        table.insert(first, 10);

        let store_view = table.store(store.store_id());

        assert_eq!(store_view.get_copied(first), Some(10));
        assert_eq!(store_view.get_copied(second), Some(20));
        assert_eq!(store_view.get_copied(foreign), None);
        assert_eq!(
            store_view.values().copied().collect::<Vec<_>>(),
            vec![10, 20]
        );
        assert_eq!(
            store_view
                .iter()
                .map(|(node, value)| (node, *value))
                .collect::<Vec<_>>(),
            vec![(first, 10), (second, 20)]
        );
    }

    #[test]
    fn node_side_table_should_append_store_node_map() {
        let mut store = AstStore::new();
        let first = alloc_token(&mut store);
        let second = alloc_token(&mut store);
        let mut other_store = AstStore::new();
        let foreign = alloc_token(&mut other_store);
        let mut table = NodeSideTable::default();
        let mut map = store.new_node_map::<usize>();

        table.insert(foreign, 30);
        map.insert(second, 20);
        map.insert(first, 10);

        table.append_store_map(&mut map);

        assert!(map.values.is_empty());
        assert_eq!(table.get_copied(first), Some(10));
        assert_eq!(table.get_copied(second), Some(20));
        assert_eq!(table.get_copied(foreign), Some(30));
    }

    #[test]
    fn node_side_table_is_empty_should_track_mutations_without_scanning() {
        let mut store = AstStore::new();
        let first = alloc_token(&mut store);
        let second = alloc_token(&mut store);
        let mut table = NodeSideTable::default();

        assert!(table.is_empty());

        assert_eq!(table.insert(first, 10), None);
        assert!(!table.is_empty());
        assert!(!table.store(store.store_id()).is_empty());

        assert_eq!(table.insert(first, 11), Some(10));
        assert!(!table.is_empty());

        assert_eq!(table.remove(first), Some(11));
        assert!(table.is_empty());
        assert!(table.store(store.store_id()).is_empty());

        let mut other = NodeSideTable::default();
        other.insert(first, 20);
        other.insert(second, 30);
        table.append(&mut other);

        assert!(!table.is_empty());
        assert!(other.is_empty());

        let mut map = store.new_node_map::<usize>();
        map.insert(first, 40);
        table.append_store_map(&mut map);

        assert!(!table.is_empty());
        assert_eq!(table.get_copied(first), Some(40));
        assert_eq!(table.get_copied(second), Some(30));

        assert_eq!(table.remove(first), Some(40));
        assert!(!table.is_empty());
        assert_eq!(table.remove(second), Some(30));
        assert!(table.is_empty());
    }

    #[test]
    fn replace_aggregate_nodes_should_only_probe_foreign_entries() {
        let mut store = AstStore::new();
        let local = alloc_token(&mut store);
        let wrong_local_replacement = alloc_token(&mut store);
        let imported_replacement = alloc_token(&mut store);
        let mut source = AstStore::new();
        let foreign = alloc_token(&mut source);
        let range = core::undefined_text_range();

        let list = store.alloc_node_list(range, range, [local, wrong_local_replacement]);
        let list_entries = store.lists.node_lists[list.to_idx()].entries.clone();
        let list_first = list_entries.start();
        let list_second = list_entries.clone().nth(1).unwrap();
        store.lists.node_list_entries[list_second].node = foreign;

        let slice = store.alloc_raw_node_slice([Some(local), Some(wrong_local_replacement)]);
        let slice_entries = store.lists.raw_node_slices[slice.to_idx()].entries.clone();
        let slice_first = slice_entries.start();
        let slice_second = slice_entries.clone().nth(1).unwrap();
        store.lists.raw_node_slice_entries[slice_second].node = OptionalAstNodeId::some(foreign);

        let mut replacements = NodeSideTable::default();
        replacements.insert(local, wrong_local_replacement);
        replacements.insert(foreign, imported_replacement);

        store.replace_aggregate_nodes(&replacements);

        assert_eq!(store.lists.node_list_entries[list_first].node, local);
        assert_eq!(
            store.lists.node_list_entries[list_second].node,
            imported_replacement
        );
        assert_eq!(
            store.lists.raw_node_slice_entries[slice_first].node.get(),
            Some(local)
        );
        assert_eq!(
            store.lists.raw_node_slice_entries[slice_second].node.get(),
            Some(imported_replacement)
        );
    }

    #[test]
    fn aggregate_storage_should_track_foreign_entries() {
        let mut store = AstStore::new();
        let local = alloc_token(&mut store);
        let imported_list_replacement = alloc_token(&mut store);
        let imported_slice_replacement = alloc_token(&mut store);
        let mut source = AstStore::new();
        let foreign_list_node = alloc_token(&mut source);
        let foreign_slice_node = alloc_token(&mut source);
        let range = core::undefined_text_range();

        store.lists.alloc_node_list(
            store.store_id(),
            range,
            range,
            [local, foreign_list_node],
            false,
            false,
        );
        store.lists.alloc_raw_node_slice(
            store.store_id(),
            [
                OptionalAstNodeId::some(foreign_slice_node),
                OptionalAstNodeId::none(),
            ],
        );

        assert_eq!(store.lists.foreign_node_entries, 2);
        assert_eq!(
            store.foreign_nodes_in_aggregate_storage(),
            vec![foreign_list_node, foreign_slice_node]
        );

        let mut replacements = NodeSideTable::default();
        replacements.insert(foreign_list_node, imported_list_replacement);
        replacements.insert(foreign_slice_node, imported_slice_replacement);

        store.replace_aggregate_nodes(&replacements);

        assert_eq!(store.lists.foreign_node_entries, 0);
        assert!(store.foreign_nodes_in_aggregate_storage().is_empty());
    }

    #[test]
    fn local_aggregate_storage_should_keep_foreign_entry_count_empty() {
        let mut store = AstStore::new();
        let first = alloc_token(&mut store);
        let second = alloc_token(&mut store);
        let range = core::undefined_text_range();

        store.alloc_node_list(range, range, [first, second]);
        store.alloc_raw_node_slice([Some(first), None]);

        assert_eq!(store.lists.foreign_node_entries, 0);
        assert!(store.foreign_nodes_in_aggregate_storage().is_empty());
    }

    #[test]
    #[should_panic(expected = "node map cannot index a node from another AST store")]
    fn node_map_should_reject_nodes_from_other_stores() {
        let store = AstStore::new();
        let mut other = AstStore::new();
        let foreign = alloc_token(&mut other);
        let map = store.new_node_map::<usize>();

        map.get(foreign);
    }
}
