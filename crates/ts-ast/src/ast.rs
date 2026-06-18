#![expect(dead_code, reason = "ported AST API is ahead of current callers")]

use std::{
    any::Any,
    hash::{Hash, Hasher},
    ops::{ControlFlow, Deref},
    sync::{Arc, Mutex, OnceLock, Weak},
};

use ts_collections::{FastHashMap as HashMap, FastHashMapExt};
use ts_core as core;
use ts_tspath as tspath;

use crate::OuterExpressionKinds;
use crate::arena::{
    AstNodeId, AstStore, ModifierListId, ModifierListView, Node, NodeListId, NodeListIter,
    NodeListView, NodePayloadId, NodePayloadTag, NodeSideTable, OptionalAstNodeId,
    OptionalModifierListId, OptionalNodeListId, OptionalRawNodeSliceId, OptionalRawStringSliceId,
    RawNodeSliceId, RawNodeSliceView, RawStringSliceId, RawStringSliceView, StoreId,
};
use crate::ast_generated::*;
use crate::diagnostic::Diagnostic;
use crate::ids::{LocalAstId, NodeId, SourceId, SourceSnapshotId, StableNodeId};
use crate::kind_generated::Kind;
use crate::modifierflags::ModifierFlags;
use crate::nodeflags::NodeFlags;
use crate::parseoptions::{ExternalModuleIndicatorOptions, SourceFileParseOptions};
use crate::positionmap::{PositionMap, compute_position_map};
use crate::subtreefacts::SubtreeFacts;
use crate::symbol::*;
use crate::tokenflags::TokenFlags;

mod xxh3 {
    pub type Uint128 = u128;
}

#[derive(Clone, Default)]
pub struct NodeFactoryHooks {
    pub on_create: Option<Arc<dyn Fn(Node) -> NodeFlags + Send + Sync>>, // Hooks the creation of a node.
    pub on_update: Option<Arc<dyn Fn(&AstStore, Node, Node) + Send + Sync>>, // Hooks the updating of a node.
    pub on_clone: Option<Arc<dyn Fn(&AstStore, Node, Node) + Send + Sync>>, // Hooks the cloning of a node.
}

pub fn new_node_factory(hooks: NodeFactoryHooks) -> NodeFactory {
    NodeFactory::new(hooks)
}

pub type Expression = Node;
pub type StringLiteralLike = Node;
pub type StringLiteralNode = Node;
pub type SignatureDeclaration = Node;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeList {
    id: NodeListId,
}

impl NodeList {
    pub(crate) fn from_id(id: NodeListId) -> Self {
        Self { id }
    }

    pub(crate) fn id(self) -> NodeListId {
        self.id
    }

    pub fn store_id(self) -> StoreId {
        self.id.store_id()
    }

    pub(crate) fn assert_store(self, store_id: StoreId) {
        self.id.assert_store(store_id);
    }

    pub fn iter(self, store: &AstStore) -> impl Iterator<Item = Node> + '_ {
        self.id.assert_store(store.store_id());
        store.node_list(self.id).iter()
    }
}

pub trait IntoNodeList {
    fn into_node_list(self) -> NodeList;
}

impl IntoNodeList for NodeList {
    fn into_node_list(self) -> NodeList {
        self
    }
}

pub trait IntoOptionalNodeList {
    fn into_optional_node_list(self) -> Option<NodeList>;
}

impl IntoOptionalNodeList for NodeList {
    fn into_optional_node_list(self) -> Option<NodeList> {
        Some(self)
    }
}

impl IntoOptionalNodeList for Option<NodeList> {
    fn into_optional_node_list(self) -> Option<NodeList> {
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ModifierList {
    id: ModifierListId,
}

impl ModifierList {
    pub(crate) fn from_id(id: ModifierListId) -> Self {
        Self { id }
    }

    pub(crate) fn id(self) -> ModifierListId {
        self.id
    }

    pub fn store_id(self) -> StoreId {
        self.id.store_id()
    }

    pub(crate) fn assert_store(self, store_id: StoreId) {
        self.id.assert_store(store_id);
    }
}

pub trait IntoModifierList {
    fn into_modifier_list(self) -> ModifierList;
}

impl IntoModifierList for ModifierList {
    fn into_modifier_list(self) -> ModifierList {
        self
    }
}

pub trait IntoOptionalModifierList {
    fn into_optional_modifier_list(self) -> Option<ModifierList>;
}

impl IntoOptionalModifierList for ModifierList {
    fn into_optional_modifier_list(self) -> Option<ModifierList> {
        Some(self)
    }
}

impl IntoOptionalModifierList for Option<ModifierList> {
    fn into_optional_modifier_list(self) -> Option<ModifierList> {
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RawNodeSlice {
    id: RawNodeSliceId,
}

impl RawNodeSlice {
    pub(crate) fn from_id(id: RawNodeSliceId) -> Self {
        Self { id }
    }

    pub(crate) fn id(self) -> RawNodeSliceId {
        self.id
    }

    pub fn store_id(self) -> StoreId {
        self.id.store_id()
    }

    pub(crate) fn assert_store(self, store_id: StoreId) {
        self.id.assert_store(store_id);
    }
}

pub trait IntoRawNodeSlice {
    fn into_raw_node_slice(self) -> RawNodeSlice;
}

impl IntoRawNodeSlice for RawNodeSlice {
    fn into_raw_node_slice(self) -> RawNodeSlice {
        self
    }
}

pub trait IntoOptionalRawNodeSlice {
    fn into_optional_raw_node_slice(self) -> Option<RawNodeSlice>;
}

impl IntoOptionalRawNodeSlice for RawNodeSlice {
    fn into_optional_raw_node_slice(self) -> Option<RawNodeSlice> {
        Some(self)
    }
}

impl IntoOptionalRawNodeSlice for Option<RawNodeSlice> {
    fn into_optional_raw_node_slice(self) -> Option<RawNodeSlice> {
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RawStringSlice {
    id: RawStringSliceId,
}

impl RawStringSlice {
    pub(crate) fn from_id(id: RawStringSliceId) -> Self {
        Self { id }
    }

    pub(crate) fn id(self) -> RawStringSliceId {
        self.id
    }

    pub(crate) fn assert_store(self, store_id: StoreId) {
        self.id.assert_store(store_id);
    }
}

pub trait IntoRawStringSlice {
    fn into_raw_string_slice(self) -> RawStringSlice;
}

impl IntoRawStringSlice for RawStringSlice {
    fn into_raw_string_slice(self) -> RawStringSlice {
        self
    }
}

pub(crate) trait IntoOptionalRawStringSlice {
    fn into_optional_raw_string_slice(self) -> Option<RawStringSlice>;
}

impl IntoOptionalRawStringSlice for RawStringSlice {
    fn into_optional_raw_string_slice(self) -> Option<RawStringSlice> {
        Some(self)
    }
}

impl IntoOptionalRawStringSlice for Option<RawStringSlice> {
    fn into_optional_raw_string_slice(self) -> Option<RawStringSlice> {
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeListPositionKey(NodeListId);

impl NodeListPositionKey {
    pub fn from_node_list(list: NodeList) -> Self {
        Self(list.id())
    }

    pub(crate) fn from_node_list_id(id: NodeListId) -> Self {
        Self(id)
    }
}

pub trait NodeFactoryCoercible {
    fn as_node_factory(&mut self) -> &mut NodeFactory;
}

impl NodeFactoryCoercible for NodeFactory {
    fn as_node_factory(&mut self) -> &mut NodeFactory {
        self
    }
}

#[derive(Clone, Copy)]
pub struct SourceNodeList<'a> {
    store: &'a AstStore,
    id: NodeListId,
}

impl<'a> SourceNodeList<'a> {
    pub(crate) fn new(store: &'a AstStore, id: NodeListId) -> Self {
        id.assert_store(store.store_id());
        Self { store, id }
    }

    pub fn store(self) -> &'a AstStore {
        self.store
    }

    pub fn source_ref(self) -> SourceNodeListRef {
        SourceNodeListRef {
            store_id: self.store.store_id(),
            id: self.id,
        }
    }

    pub(crate) fn id(self) -> NodeListId {
        self.id
    }

    pub fn position_key(self) -> NodeListPositionKey {
        NodeListPositionKey::from_node_list_id(self.id)
    }

    pub(crate) fn view(self) -> NodeListView<'a> {
        self.store.node_list(self.id)
    }

    pub fn len(self) -> usize {
        self.view().len()
    }

    pub fn is_empty(self) -> bool {
        self.view().is_empty()
    }

    pub fn loc(self) -> core::TextRange {
        self.view().loc()
    }

    pub fn pos(self) -> i32 {
        self.view().pos()
    }

    pub fn end(self) -> i32 {
        self.view().end()
    }

    pub fn range(self) -> core::TextRange {
        self.view().range()
    }

    pub fn is_missing(self) -> bool {
        self.view().is_missing()
    }

    pub fn has_trailing_comma(self) -> bool {
        self.view().has_trailing_comma()
    }

    pub fn iter(self) -> impl Iterator<Item = Node> + 'a {
        self.view().iter()
    }

    pub fn first(self) -> Option<Node> {
        self.view().first()
    }

    pub fn last(self) -> Option<Node> {
        self.view().last()
    }

    pub fn nodes(self) -> Vec<Node> {
        self.iter().collect()
    }

    pub fn same_list(self, other: SourceNodeList<'_>) -> bool {
        self.view().same_list(other.view())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SourceNodeListRef {
    store_id: StoreId,
    id: NodeListId,
}

impl SourceNodeListRef {
    pub fn store_id(self) -> StoreId {
        self.store_id
    }

    pub(crate) fn id(self) -> NodeListId {
        self.id
    }

    pub fn position_key(self) -> NodeListPositionKey {
        NodeListPositionKey::from_node_list_id(self.id)
    }

    pub fn resolve(self, store: &AstStore) -> SourceNodeList<'_> {
        assert_eq!(
            self.store_id,
            store.store_id(),
            "source node list ref resolved against a different AST store"
        );
        SourceNodeList::new(store, self.id)
    }
}

#[derive(Clone, Debug)]
pub struct SourceNodeListInput {
    source: SourceNodeListRef,
    loc: core::TextRange,
    range: core::TextRange,
    has_trailing_comma: bool,
    nodes: Vec<Node>,
}

impl SourceNodeListInput {
    pub fn from_source(source: SourceNodeList<'_>) -> Self {
        Self {
            source: source.source_ref(),
            loc: source.loc(),
            range: source.range(),
            has_trailing_comma: source.has_trailing_comma(),
            nodes: source.iter().collect(),
        }
    }

    pub fn source_ref(&self) -> SourceNodeListRef {
        self.source
    }

    pub fn store_id(&self) -> StoreId {
        self.source.store_id()
    }

    pub(crate) fn id(&self) -> NodeListId {
        self.source.id()
    }

    pub fn as_node_list(&self) -> NodeList {
        NodeList::from_id(self.source.id())
    }

    pub fn resolve<'a>(&self, store: &'a AstStore) -> SourceNodeList<'a> {
        self.source.resolve(store)
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn loc(&self) -> core::TextRange {
        self.loc
    }

    pub fn range(&self) -> core::TextRange {
        self.range
    }

    pub fn has_trailing_comma(&self) -> bool {
        self.has_trailing_comma
    }

    pub fn iter(&self) -> impl Iterator<Item = Node> + '_ {
        self.nodes.iter().copied()
    }

    pub fn nodes(&self) -> Vec<Node> {
        self.nodes.clone()
    }
}

impl<'a> IntoIterator for SourceNodeList<'a> {
    type Item = Node;
    type IntoIter = NodeListIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.view().into_iter()
    }
}

#[derive(Clone, Copy)]
pub struct SourceModifierList<'a> {
    store: &'a AstStore,
    id: ModifierListId,
}

impl<'a> SourceModifierList<'a> {
    pub(crate) fn new(store: &'a AstStore, id: ModifierListId) -> Self {
        id.assert_store(store.store_id());
        Self { store, id }
    }

    pub fn store(self) -> &'a AstStore {
        self.store
    }

    pub fn source_ref(self) -> SourceModifierListRef {
        SourceModifierListRef {
            store_id: self.store.store_id(),
            id: self.id,
        }
    }

    pub(crate) fn id(self) -> ModifierListId {
        self.id
    }

    pub(crate) fn view(self) -> ModifierListView<'a> {
        self.store.modifier_list(self.id)
    }

    pub fn nodes(self) -> SourceNodeList<'a> {
        SourceNodeList::new(self.store, self.view().nodes().id())
    }

    pub fn loc(self) -> core::TextRange {
        self.nodes().loc()
    }

    pub fn range(self) -> core::TextRange {
        self.nodes().range()
    }

    pub fn modifier_flags(self) -> ModifierFlags {
        self.view().modifier_flags()
    }

    pub fn is_empty(self) -> bool {
        self.nodes().is_empty()
    }

    pub fn iter(self) -> NodeListIter<'a> {
        self.nodes().into_iter()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SourceModifierListRef {
    store_id: StoreId,
    id: ModifierListId,
}

impl SourceModifierListRef {
    pub fn store_id(self) -> StoreId {
        self.store_id
    }

    pub(crate) fn id(self) -> ModifierListId {
        self.id
    }

    pub fn resolve(self, store: &AstStore) -> SourceModifierList<'_> {
        assert_eq!(
            self.store_id,
            store.store_id(),
            "source modifier list ref resolved against a different AST store"
        );
        SourceModifierList::new(store, self.id)
    }
}

#[derive(Clone, Debug)]
pub struct SourceModifierListInput {
    source: SourceModifierListRef,
    loc: core::TextRange,
    range: core::TextRange,
    modifier_flags: ModifierFlags,
    nodes: Vec<Node>,
}

impl SourceModifierListInput {
    pub fn from_source(source: SourceModifierList<'_>) -> Self {
        Self {
            source: source.source_ref(),
            loc: source.loc(),
            range: source.range(),
            modifier_flags: source.modifier_flags(),
            nodes: source.iter().collect(),
        }
    }

    pub fn source_ref(&self) -> SourceModifierListRef {
        self.source
    }

    pub fn store_id(&self) -> StoreId {
        self.source.store_id()
    }

    pub(crate) fn id(&self) -> ModifierListId {
        self.source.id()
    }

    pub fn as_modifier_list(&self) -> ModifierList {
        ModifierList::from_id(self.source.id())
    }

    pub fn resolve<'a>(&self, store: &'a AstStore) -> SourceModifierList<'a> {
        self.source.resolve(store)
    }

    pub fn loc(&self) -> core::TextRange {
        self.loc
    }

    pub fn range(&self) -> core::TextRange {
        self.range
    }

    pub fn modifier_flags(&self) -> ModifierFlags {
        self.modifier_flags
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn nodes(&self) -> Vec<Node> {
        self.nodes.clone()
    }

    pub fn iter(&self) -> impl Iterator<Item = Node> + '_ {
        self.nodes.iter().copied()
    }
}

impl<'a> IntoIterator for SourceModifierList<'a> {
    type Item = Node;
    type IntoIter = NodeListIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.view().nodes().into_iter()
    }
}

#[derive(Clone, Copy)]
pub struct SourceRawNodeSlice<'a> {
    store: &'a AstStore,
    id: RawNodeSliceId,
}

impl<'a> SourceRawNodeSlice<'a> {
    pub(crate) fn new(store: &'a AstStore, id: RawNodeSliceId) -> Self {
        id.assert_store(store.store_id());
        Self { store, id }
    }

    pub fn store(self) -> &'a AstStore {
        self.store
    }

    pub fn source_ref(self) -> SourceRawNodeSliceRef {
        SourceRawNodeSliceRef {
            store_id: self.store.store_id(),
            id: self.id,
        }
    }

    pub(crate) fn id(self) -> RawNodeSliceId {
        self.id
    }

    pub(crate) fn view(self) -> RawNodeSliceView<'a> {
        self.store.raw_node_slice(self.id)
    }

    pub fn iter(self) -> impl ExactSizeIterator<Item = Option<Node>> + DoubleEndedIterator + 'a {
        self.view().iter()
    }

    pub fn nodes(self) -> Vec<Option<Node>> {
        self.iter().collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SourceRawNodeSliceRef {
    store_id: StoreId,
    id: RawNodeSliceId,
}

impl SourceRawNodeSliceRef {
    pub fn store_id(self) -> StoreId {
        self.store_id
    }

    pub(crate) fn id(self) -> RawNodeSliceId {
        self.id
    }

    pub fn resolve(self, store: &AstStore) -> SourceRawNodeSlice<'_> {
        assert_eq!(
            self.store_id,
            store.store_id(),
            "source raw node slice ref resolved against a different AST store"
        );
        SourceRawNodeSlice::new(store, self.id)
    }
}

#[derive(Clone, Debug)]
pub struct SourceRawNodeSliceInput {
    source: SourceRawNodeSliceRef,
    nodes: Vec<Option<Node>>,
}

impl SourceRawNodeSliceInput {
    pub fn from_source(source: SourceRawNodeSlice<'_>) -> Self {
        Self {
            source: source.source_ref(),
            nodes: source.iter().collect(),
        }
    }

    pub fn source_ref(&self) -> SourceRawNodeSliceRef {
        self.source
    }

    pub fn store_id(&self) -> StoreId {
        self.source.store_id()
    }

    pub(crate) fn id(&self) -> RawNodeSliceId {
        self.source.id()
    }

    pub fn as_raw_node_slice(&self) -> RawNodeSlice {
        RawNodeSlice::from_id(self.source.id())
    }

    pub fn resolve<'a>(&self, store: &'a AstStore) -> SourceRawNodeSlice<'a> {
        self.source.resolve(store)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = Option<Node>> + '_ {
        self.nodes.iter().copied()
    }
}

impl IntoIterator for SourceRawNodeSlice<'_> {
    type Item = Option<Node>;
    type IntoIter = std::vec::IntoIter<Option<Node>>;

    fn into_iter(self) -> Self::IntoIter {
        self.nodes().into_iter()
    }
}

#[derive(Clone, Copy)]
pub struct SourceRawStringSlice<'a> {
    store: &'a AstStore,
    id: RawStringSliceId,
}

impl<'a> SourceRawStringSlice<'a> {
    pub(crate) fn new(store: &'a AstStore, id: RawStringSliceId) -> Self {
        id.assert_store(store.store_id());
        Self { store, id }
    }

    pub fn store(self) -> &'a AstStore {
        self.store
    }

    pub fn source_ref(self) -> SourceRawStringSliceRef {
        SourceRawStringSliceRef {
            store_id: self.store.store_id(),
            id: self.id,
        }
    }

    pub(crate) fn id(self) -> RawStringSliceId {
        self.id
    }

    pub(crate) fn view(self) -> RawStringSliceView<'a> {
        self.store.raw_string_slice(self.id)
    }

    pub fn iter(self) -> impl ExactSizeIterator<Item = &'a str> + DoubleEndedIterator {
        self.view().iter()
    }

    pub fn strings(self) -> Vec<&'a str> {
        self.iter().collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SourceRawStringSliceRef {
    store_id: StoreId,
    id: RawStringSliceId,
}

impl SourceRawStringSliceRef {
    pub fn store_id(self) -> StoreId {
        self.store_id
    }

    pub(crate) fn id(self) -> RawStringSliceId {
        self.id
    }

    pub fn resolve(self, store: &AstStore) -> SourceRawStringSlice<'_> {
        assert_eq!(
            self.store_id,
            store.store_id(),
            "source raw string slice ref resolved against a different AST store"
        );
        SourceRawStringSlice::new(store, self.id)
    }
}

#[derive(Clone, Debug)]
pub struct SourceRawStringSliceInput {
    source: SourceRawStringSliceRef,
    strings: Vec<Arc<str>>,
}

impl SourceRawStringSliceInput {
    pub fn from_source(source: SourceRawStringSlice<'_>) -> Self {
        Self {
            source: source.source_ref(),
            strings: source.iter().map(Arc::<str>::from).collect(),
        }
    }

    pub fn source_ref(&self) -> SourceRawStringSliceRef {
        self.source
    }

    pub fn store_id(&self) -> StoreId {
        self.source.store_id()
    }

    pub(crate) fn id(&self) -> RawStringSliceId {
        self.source.id()
    }

    pub fn as_raw_string_slice(&self) -> RawStringSlice {
        RawStringSlice::from_id(self.source.id())
    }

    pub fn resolve<'a>(&self, store: &'a AstStore) -> SourceRawStringSlice<'a> {
        self.source.resolve(store)
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> + '_ {
        self.strings.iter().map(AsRef::as_ref)
    }
}

impl<'a> IntoIterator for SourceRawStringSlice<'a> {
    type Item = &'a str;
    type IntoIter = std::vec::IntoIter<&'a str>;

    fn into_iter(self) -> Self::IntoIter {
        self.strings().into_iter()
    }
}

#[derive(Default)]
pub struct AstTraversalState {
    preserved_nodes: crate::arena::NodeSideTable<Node>,
    preserved_node_lists: HashMap<NodeListId, NodeListId>,
    preserved_modifier_lists: HashMap<ModifierListId, ModifierListId>,
    preserved_raw_node_slices: HashMap<RawNodeSliceId, RawNodeSliceId>,
    preserved_raw_string_slices: HashMap<RawStringSliceId, RawStringSliceId>,
    cloned_nodes: crate::arena::NodeSideTable<Node>,
    cloned_node_lists: HashMap<NodeListId, NodeListId>,
    cloned_modifier_lists: HashMap<ModifierListId, ModifierListId>,
    cloned_raw_node_slices: HashMap<RawNodeSliceId, RawNodeSliceId>,
}

#[derive(Default)]
pub struct AstImportState {
    traversal: AstTraversalState,
}

impl AstImportState {
    pub fn new() -> Self {
        Self {
            traversal: AstTraversalState::new(),
        }
    }

    pub fn preserve_node(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        node: Node,
    ) -> Node {
        self.traversal.preserve_node(source, factory, node)
    }

    pub fn preserve_optional_node(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        node: Option<Node>,
    ) -> Option<Node> {
        node.map(|node| self.preserve_node(source, factory, node))
    }

    pub(crate) fn preserve_node_list(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        list: NodeListId,
    ) -> NodeListId {
        self.traversal.preserve_node_list(source, factory, list)
    }

    pub(crate) fn preserve_optional_node_list(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        list: Option<NodeListId>,
    ) -> Option<NodeListId> {
        list.map(|list| self.preserve_node_list(source, factory, list))
    }

    pub(crate) fn preserve_source_node_list_id(
        &mut self,
        factory: &mut NodeFactory,
        list: SourceNodeList<'_>,
    ) -> NodeListId {
        self.preserve_node_list(list.store(), factory, list.id())
    }

    pub fn preserve_source_node_list(
        &mut self,
        factory: &mut NodeFactory,
        list: SourceNodeList<'_>,
    ) -> NodeList {
        NodeList::from_id(self.preserve_source_node_list_id(factory, list))
    }

    pub fn preserve_optional_source_node_list(
        &mut self,
        factory: &mut NodeFactory,
        list: Option<SourceNodeList<'_>>,
    ) -> Option<NodeList> {
        list.map(|list| self.preserve_source_node_list(factory, list))
    }

    pub fn preserve_source_node_list_input(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        list: &SourceNodeListInput,
    ) -> NodeList {
        if list.store_id() == factory.store().store_id() {
            return NodeList::from_id(list.id());
        }
        NodeList::from_id(self.preserve_node_list(source, factory, list.id()))
    }

    pub(crate) fn preserve_modifier_list(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        modifiers: ModifierListId,
    ) -> ModifierListId {
        self.traversal
            .preserve_modifier_list(source, factory, modifiers)
    }

    pub(crate) fn preserve_optional_modifier_list(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        modifiers: Option<ModifierListId>,
    ) -> Option<ModifierListId> {
        modifiers.map(|modifiers| self.preserve_modifier_list(source, factory, modifiers))
    }

    pub(crate) fn preserve_source_modifier_list_id(
        &mut self,
        factory: &mut NodeFactory,
        modifiers: SourceModifierList<'_>,
    ) -> ModifierListId {
        self.preserve_modifier_list(modifiers.store(), factory, modifiers.id())
    }

    pub fn preserve_source_modifier_list(
        &mut self,
        factory: &mut NodeFactory,
        modifiers: SourceModifierList<'_>,
    ) -> ModifierList {
        ModifierList::from_id(self.preserve_source_modifier_list_id(factory, modifiers))
    }

    pub fn preserve_optional_source_modifier_list(
        &mut self,
        factory: &mut NodeFactory,
        modifiers: Option<SourceModifierList<'_>>,
    ) -> Option<ModifierList> {
        modifiers.map(|modifiers| self.preserve_source_modifier_list(factory, modifiers))
    }

    pub fn preserve_source_modifier_list_input(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        modifiers: &SourceModifierListInput,
    ) -> ModifierList {
        if modifiers.store_id() == factory.store().store_id() {
            return ModifierList::from_id(modifiers.id());
        }
        ModifierList::from_id(self.preserve_modifier_list(source, factory, modifiers.id()))
    }

    pub(crate) fn preserve_raw_node_slice(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        nodes: RawNodeSliceId,
    ) -> RawNodeSliceId {
        self.traversal
            .preserve_raw_node_slice(source, factory, nodes)
    }

    pub(crate) fn preserve_source_raw_node_slice_id(
        &mut self,
        factory: &mut NodeFactory,
        nodes: SourceRawNodeSlice<'_>,
    ) -> RawNodeSliceId {
        self.preserve_raw_node_slice(nodes.store(), factory, nodes.id())
    }

    pub fn preserve_source_raw_node_slice(
        &mut self,
        factory: &mut NodeFactory,
        nodes: SourceRawNodeSlice<'_>,
    ) -> RawNodeSlice {
        RawNodeSlice::from_id(self.preserve_source_raw_node_slice_id(factory, nodes))
    }

    pub(crate) fn preserve_optional_source_raw_node_slice(
        &mut self,
        factory: &mut NodeFactory,
        nodes: Option<SourceRawNodeSlice<'_>>,
    ) -> Option<RawNodeSliceId> {
        nodes.map(|nodes| self.preserve_source_raw_node_slice_id(factory, nodes))
    }

    pub(crate) fn preserve_raw_string_slice(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        strings: RawStringSliceId,
    ) -> RawStringSliceId {
        self.traversal
            .preserve_raw_string_slice(source, factory, strings)
    }

    pub fn preserve_source_raw_node_slice_input(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        nodes: &SourceRawNodeSliceInput,
    ) -> RawNodeSlice {
        if nodes.store_id() == factory.store().store_id() {
            return RawNodeSlice::from_id(nodes.id());
        }
        RawNodeSlice::from_id(self.preserve_raw_node_slice(source, factory, nodes.id()))
    }

    pub(crate) fn preserve_source_raw_string_slice_id(
        &mut self,
        factory: &mut NodeFactory,
        strings: SourceRawStringSlice<'_>,
    ) -> RawStringSliceId {
        self.preserve_raw_string_slice(strings.store(), factory, strings.id())
    }

    pub fn preserve_source_raw_string_slice(
        &mut self,
        factory: &mut NodeFactory,
        strings: SourceRawStringSlice<'_>,
    ) -> RawStringSlice {
        RawStringSlice::from_id(self.preserve_source_raw_string_slice_id(factory, strings))
    }

    pub(crate) fn preserve_optional_source_raw_string_slice(
        &mut self,
        factory: &mut NodeFactory,
        strings: Option<SourceRawStringSlice<'_>>,
    ) -> Option<RawStringSliceId> {
        strings.map(|strings| self.preserve_source_raw_string_slice_id(factory, strings))
    }

    pub fn preserve_source_raw_string_slice_input(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        strings: &SourceRawStringSliceInput,
    ) -> RawStringSlice {
        if strings.store_id() == factory.store().store_id() {
            return RawStringSlice::from_id(strings.id());
        }
        RawStringSlice::from_id(self.preserve_raw_string_slice(source, factory, strings.id()))
    }

    pub fn store_for<'a>(
        source: &'a AstStore,
        factory: &'a NodeFactory,
        node: Node,
    ) -> &'a AstStore {
        AstTraversalState::store_for(source, factory, node)
    }

    pub fn preserved_node(&self, factory: &NodeFactory, source: Node) -> Option<Node> {
        self.traversal.preserved_node(factory, source)
    }

    pub fn record_cloned_node(
        &mut self,
        source_store: StoreId,
        factory: &NodeFactory,
        source: Node,
        imported: Node,
    ) -> Node {
        self.traversal
            .record_cloned_node(source_store, factory, source, imported)
    }

    pub fn clone_node_from_store(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        node: Node,
    ) -> Node {
        self.traversal.clone_node_from_store(source, factory, node)
    }

    pub(crate) fn clone_node_list_from_store(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        list: NodeListId,
    ) -> NodeListId {
        self.traversal
            .clone_node_list_from_store(source, factory, list)
    }

    pub fn clone_source_node_list(
        &mut self,
        factory: &mut NodeFactory,
        list: SourceNodeList<'_>,
    ) -> NodeList {
        NodeList::from_id(self.clone_node_list_from_store(list.store(), factory, list.id()))
    }

    pub fn clone_source_node_list_input(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        list: &SourceNodeListInput,
    ) -> NodeList {
        if list.store_id() == factory.store().store_id() {
            return NodeList::from_id(list.id());
        }
        NodeList::from_id(self.clone_node_list_from_store(source, factory, list.id()))
    }

    pub(crate) fn clone_modifier_list_from_store(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        modifiers: ModifierListId,
    ) -> ModifierListId {
        self.traversal
            .clone_modifier_list_from_store(source, factory, modifiers)
    }

    pub fn clone_source_modifier_list(
        &mut self,
        factory: &mut NodeFactory,
        modifiers: SourceModifierList<'_>,
    ) -> ModifierList {
        ModifierList::from_id(self.clone_modifier_list_from_store(
            modifiers.store(),
            factory,
            modifiers.id(),
        ))
    }

    pub fn clone_source_modifier_list_input(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        modifiers: &SourceModifierListInput,
    ) -> ModifierList {
        if modifiers.store_id() == factory.store().store_id() {
            return ModifierList::from_id(modifiers.id());
        }
        ModifierList::from_id(self.clone_modifier_list_from_store(source, factory, modifiers.id()))
    }

    pub(crate) fn clone_raw_node_slice_from_store(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        nodes: RawNodeSliceId,
    ) -> RawNodeSliceId {
        self.traversal
            .clone_raw_node_slice_from_store(source, factory, nodes)
    }

    pub fn clone_source_raw_node_slice(
        &mut self,
        factory: &mut NodeFactory,
        nodes: SourceRawNodeSlice<'_>,
    ) -> RawNodeSlice {
        RawNodeSlice::from_id(self.clone_raw_node_slice_from_store(
            nodes.store(),
            factory,
            nodes.id(),
        ))
    }

    pub fn clone_source_raw_node_slice_input(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        nodes: &SourceRawNodeSliceInput,
    ) -> RawNodeSlice {
        if nodes.store_id() == factory.store().store_id() {
            return RawNodeSlice::from_id(nodes.id());
        }
        RawNodeSlice::from_id(self.clone_raw_node_slice_from_store(source, factory, nodes.id()))
    }

    pub fn preserved_source_node_matches(
        &self,
        factory: &NodeFactory,
        source: Option<Node>,
        output: Option<Node>,
    ) -> bool {
        self.traversal
            .preserved_source_node_matches(factory, source, output)
    }

    pub(crate) fn preserved_source_node_list_matches(
        &self,
        source_store: &AstStore,
        factory: &NodeFactory,
        source: Option<NodeListId>,
        output: Option<NodeListId>,
    ) -> bool {
        self.traversal
            .preserved_source_node_list_matches(source_store, factory, source, output)
    }

    pub fn preserved_source_node_list_view_matches(
        &self,
        factory: &NodeFactory,
        source: Option<SourceNodeList<'_>>,
        output: Option<NodeList>,
    ) -> bool {
        let Some(source) = source else {
            return output.is_none();
        };
        self.preserved_source_node_list_matches(
            source.store(),
            factory,
            Some(source.id()),
            output.map(NodeList::id),
        )
    }

    pub fn preserved_source_node_list_input_matches(
        &self,
        source_store: &AstStore,
        factory: &NodeFactory,
        source: Option<&SourceNodeListInput>,
        output: Option<NodeList>,
    ) -> bool {
        let Some(source) = source else {
            return output.is_none();
        };
        self.preserved_source_node_list_matches(
            source_store,
            factory,
            Some(source.id()),
            output.map(NodeList::id),
        )
    }

    pub(crate) fn preserved_source_modifier_list_matches(
        &self,
        source_store: &AstStore,
        factory: &NodeFactory,
        source: Option<ModifierListId>,
        output: Option<ModifierListId>,
    ) -> bool {
        self.traversal
            .preserved_source_modifier_list_matches(source_store, factory, source, output)
    }

    pub fn preserved_source_modifier_list_view_matches(
        &self,
        factory: &NodeFactory,
        source: Option<SourceModifierList<'_>>,
        output: Option<ModifierList>,
    ) -> bool {
        let Some(source) = source else {
            return output.is_none();
        };
        self.preserved_source_modifier_list_matches(
            source.store(),
            factory,
            Some(source.id()),
            output.map(ModifierList::id),
        )
    }

    pub fn preserved_source_modifier_list_input_matches(
        &self,
        source_store: &AstStore,
        factory: &NodeFactory,
        source: Option<&SourceModifierListInput>,
        output: Option<ModifierList>,
    ) -> bool {
        let Some(source) = source else {
            return output.is_none();
        };
        self.preserved_source_modifier_list_matches(
            source_store,
            factory,
            Some(source.id()),
            output.map(ModifierList::id),
        )
    }

    pub(crate) fn preserved_source_raw_node_slice_matches(
        &self,
        source_store: &AstStore,
        factory: &NodeFactory,
        source: Option<RawNodeSliceId>,
        output: Option<RawNodeSliceId>,
    ) -> bool {
        self.traversal.preserved_source_raw_node_slice_matches(
            source_store,
            factory,
            source,
            output,
        )
    }

    pub fn preserved_source_raw_node_slice_view_matches(
        &self,
        factory: &NodeFactory,
        source: Option<SourceRawNodeSlice<'_>>,
        output: Option<RawNodeSlice>,
    ) -> bool {
        let Some(source) = source else {
            return output.is_none();
        };
        self.preserved_source_raw_node_slice_matches(
            source.store(),
            factory,
            Some(source.id()),
            output.map(RawNodeSlice::id),
        )
    }

    pub fn preserved_source_raw_node_slice_input_matches(
        &self,
        source_store: &AstStore,
        factory: &NodeFactory,
        source: Option<&SourceRawNodeSliceInput>,
        output: Option<RawNodeSlice>,
    ) -> bool {
        let Some(source) = source else {
            return output.is_none();
        };
        self.preserved_source_raw_node_slice_matches(
            source_store,
            factory,
            Some(source.id()),
            output.map(RawNodeSlice::id),
        )
    }

    pub(crate) fn preserved_source_raw_string_slice_matches(
        &self,
        source_store: &AstStore,
        factory: &NodeFactory,
        source: Option<RawStringSliceId>,
        output: Option<RawStringSliceId>,
    ) -> bool {
        self.traversal.preserved_source_raw_string_slice_matches(
            source_store,
            factory,
            source,
            output,
        )
    }

    pub fn preserved_source_raw_string_slice_view_matches(
        &self,
        factory: &NodeFactory,
        source: Option<SourceRawStringSlice<'_>>,
        output: Option<RawStringSlice>,
    ) -> bool {
        let Some(source) = source else {
            return output.is_none();
        };
        self.preserved_source_raw_string_slice_matches(
            source.store(),
            factory,
            Some(source.id()),
            output.map(RawStringSlice::id),
        )
    }

    pub fn preserved_source_raw_string_slice_input_matches(
        &self,
        source_store: &AstStore,
        factory: &NodeFactory,
        source: Option<&SourceRawStringSliceInput>,
        output: Option<RawStringSlice>,
    ) -> bool {
        let Some(source) = source else {
            return output.is_none();
        };
        self.preserved_source_raw_string_slice_matches(
            source_store,
            factory,
            Some(source.id()),
            output.map(RawStringSlice::id),
        )
    }

    pub fn flatten_visited_node(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        visited: Node,
        out: &mut Vec<Node>,
    ) {
        self.traversal
            .flatten_visited_node(source, factory, visited, out)
    }

    pub fn append_visit_slice_result(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        original: Node,
        visited: Option<Node>,
        out: &mut Vec<Node>,
    ) {
        self.traversal
            .append_visit_slice_result(source, factory, original, visited, out)
    }

    pub fn append_raw_node_slice_result(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        original: Option<Node>,
        result: Option<Node>,
        out: &mut Vec<Option<Node>>,
    ) {
        self.traversal
            .append_raw_node_slice_result(source, factory, original, result, out)
    }

    pub fn update_source_file_from_store(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        node: Node,
        statements: impl IntoOptionalNodeList,
        end_of_file_token: impl Into<Option<Node>>,
    ) -> Node {
        let source_data = source.as_source_file(node);
        let metadata = SourceFileCopyMetadata::from_source(source_data)
            .map_nodes(node, |node| self.preserve_node(source, factory, node));
        let updated = factory.update_source_file_from_store_with_mapped_metadata(
            source,
            node,
            source_data,
            metadata.metadata,
            statements,
            end_of_file_token,
        );
        factory.restore_source_file_self_references(updated, metadata.self_references);
        updated
    }

    pub fn lift_to_block(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        node: Option<Node>,
    ) -> Option<Node> {
        self.traversal.lift_to_block(source, factory, node)
    }

    pub fn record_preserved_node(
        &mut self,
        source_store: StoreId,
        factory: &mut NodeFactory,
        source: Node,
        imported: Node,
    ) -> Node {
        self.traversal
            .record_preserved_node(source_store, factory, source, imported)
    }
}

impl AstTraversalState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn store_for<'a>(
        source: &'a AstStore,
        factory: &'a NodeFactory,
        node: Node,
    ) -> &'a AstStore {
        if node.store_id() == factory.store().store_id() {
            factory.store()
        } else {
            assert_eq!(
                node.store_id(),
                source.store_id(),
                "traversal cannot resolve node from unrelated AST store"
            );
            source
        }
    }

    pub fn preserve_node(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        node: Node,
    ) -> Node {
        if node.store_id() == factory.store().store_id() {
            return node;
        }
        assert_eq!(
            node.store_id(),
            source.store_id(),
            "traversal cannot preserve node from unrelated AST store"
        );
        if let Some(imported) = self.preserved_nodes.get_copied(node)
            && imported.store_id() == factory.store().store_id()
        {
            return imported;
        }
        let imported = factory.deep_clone_node_from_store_preserve_location(source, node);
        self.preserved_nodes.insert(node, imported);
        imported
    }

    pub fn clone_node_from_store(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        node: Node,
    ) -> Node {
        if node.store_id() == factory.store().store_id() {
            return node;
        }
        assert_eq!(
            node.store_id(),
            source.store_id(),
            "traversal cannot clone node from unrelated AST store"
        );
        if let Some(imported) = self.cloned_nodes.get_copied(node) {
            return imported;
        }
        let imported = factory.deep_clone_node_from_store_preserve_location(source, node);
        self.cloned_nodes.insert(node, imported);
        imported
    }

    pub(crate) fn preserve_node_list(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        list: NodeListId,
    ) -> NodeListId {
        list.assert_store(source.store_id());
        if source.store_id() == factory.store().store_id() {
            return list;
        }
        if let Some(imported) = self.preserved_node_lists.get(&list) {
            return *imported;
        }
        let source_list = source.node_list(list);
        let preserved_nodes = source_list
            .iter()
            .map(|node| self.preserve_node(source, factory, node))
            .collect::<Vec<_>>();
        let imported = factory
            .new_node_list_with_trailing_comma(
                source_list.loc(),
                source_list.range(),
                preserved_nodes,
                source_list.has_trailing_comma(),
            )
            .id();
        self.preserved_node_lists.insert(list, imported);
        imported
    }

    pub(crate) fn clone_node_list_from_store(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        list: NodeListId,
    ) -> NodeListId {
        list.assert_store(source.store_id());
        if source.store_id() == factory.store().store_id() {
            return list;
        }
        if let Some(imported) = self.cloned_node_lists.get(&list) {
            return *imported;
        }
        let source_list = source.node_list(list);
        let preserved_nodes = source_list
            .iter()
            .map(|node| self.clone_node_from_store(source, factory, node))
            .collect::<Vec<_>>();
        let imported = factory
            .new_node_list_with_trailing_comma(
                source_list.loc(),
                source_list.range(),
                preserved_nodes,
                source_list.has_trailing_comma(),
            )
            .id();
        self.cloned_node_lists.insert(list, imported);
        imported
    }

    pub(crate) fn preserve_modifier_list(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        modifiers: ModifierListId,
    ) -> ModifierListId {
        modifiers.assert_store(source.store_id());
        if source.store_id() == factory.store().store_id() {
            return modifiers;
        }
        if let Some(imported) = self.preserved_modifier_lists.get(&modifiers) {
            return *imported;
        }
        let source_modifiers = source.modifier_list(modifiers);
        let source_nodes = source_modifiers.nodes();
        let preserved_nodes = source_nodes
            .iter()
            .map(|node| self.preserve_node(source, factory, node))
            .collect::<Vec<_>>();
        let imported = factory.new_preserved_modifier_list(
            source_nodes.loc(),
            source_nodes.range(),
            preserved_nodes,
            source_modifiers.modifier_flags(),
        );
        self.preserved_modifier_lists.insert(modifiers, imported);
        imported
    }

    pub(crate) fn clone_modifier_list_from_store(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        modifiers: ModifierListId,
    ) -> ModifierListId {
        modifiers.assert_store(source.store_id());
        if source.store_id() == factory.store().store_id() {
            return modifiers;
        }
        if let Some(imported) = self.cloned_modifier_lists.get(&modifiers) {
            return *imported;
        }
        let source_modifiers = source.modifier_list(modifiers);
        let source_nodes = source_modifiers.nodes();
        let preserved_nodes = source_nodes
            .iter()
            .map(|node| self.clone_node_from_store(source, factory, node))
            .collect::<Vec<_>>();
        let imported = factory.new_preserved_modifier_list(
            source_nodes.loc(),
            source_nodes.range(),
            preserved_nodes,
            source_modifiers.modifier_flags(),
        );
        self.cloned_modifier_lists.insert(modifiers, imported);
        imported
    }

    pub(crate) fn preserve_raw_node_slice(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        nodes: RawNodeSliceId,
    ) -> RawNodeSliceId {
        nodes.assert_store(source.store_id());
        if source.store_id() == factory.store().store_id() {
            return nodes;
        }
        if let Some(imported) = self.preserved_raw_node_slices.get(&nodes) {
            return *imported;
        }
        let preserved_nodes = source
            .raw_node_slice(nodes)
            .iter()
            .map(|node| node.map(|node| self.preserve_node(source, factory, node)))
            .collect::<Vec<_>>();
        let imported = factory.new_raw_node_slice(preserved_nodes).id();
        self.preserved_raw_node_slices.insert(nodes, imported);
        imported
    }

    pub(crate) fn clone_raw_node_slice_from_store(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        nodes: RawNodeSliceId,
    ) -> RawNodeSliceId {
        nodes.assert_store(source.store_id());
        if source.store_id() == factory.store().store_id() {
            return nodes;
        }
        if let Some(imported) = self.cloned_raw_node_slices.get(&nodes) {
            return *imported;
        }
        let preserved_nodes = source
            .raw_node_slice(nodes)
            .iter()
            .map(|node| node.map(|node| self.clone_node_from_store(source, factory, node)))
            .collect::<Vec<_>>();
        let imported = factory.new_raw_node_slice(preserved_nodes).id();
        self.cloned_raw_node_slices.insert(nodes, imported);
        imported
    }

    pub(crate) fn preserve_raw_string_slice(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        strings: RawStringSliceId,
    ) -> RawStringSliceId {
        strings.assert_store(source.store_id());
        if source.store_id() == factory.store().store_id() {
            return strings;
        }
        if let Some(imported) = self.preserved_raw_string_slices.get(&strings) {
            return *imported;
        }
        let imported = factory.deep_clone_raw_string_slice_from_store(source, strings);
        self.preserved_raw_string_slices.insert(strings, imported);
        imported
    }

    pub fn record_preserved_node(
        &mut self,
        source_store_id: StoreId,
        factory: &NodeFactory,
        source: Node,
        imported: Node,
    ) -> Node {
        if source.store_id() == factory.store().store_id() {
            return imported;
        }
        assert_eq!(
            source.store_id(),
            source_store_id,
            "traversal cannot record preserved node from unrelated AST store"
        );
        assert!(
            imported.store_id() == factory.store().store_id(),
            "traversal preserve record must point at the output AST store"
        );
        if let Some(existing) = self.preserved_nodes.get_copied(source) {
            return existing;
        }
        self.preserved_nodes.insert(source, imported);
        imported
    }

    pub fn record_cloned_node(
        &mut self,
        source_store_id: StoreId,
        factory: &NodeFactory,
        source: Node,
        cloned: Node,
    ) -> Node {
        if source.store_id() == factory.store().store_id() {
            return cloned;
        }
        assert_eq!(
            source.store_id(),
            source_store_id,
            "traversal cannot record clone from unrelated AST store"
        );
        assert_eq!(
            cloned.store_id(),
            factory.store().store_id(),
            "traversal clone record must point at the output AST store"
        );
        if let Some(existing) = self.cloned_nodes.get_copied(source) {
            return existing;
        }
        self.cloned_nodes.insert(source, cloned);
        cloned
    }

    pub fn preserved_node(&self, factory: &NodeFactory, source: Node) -> Option<Node> {
        if source.store_id() == factory.store().store_id() {
            return None;
        }
        self.preserved_nodes.get_copied(source)
    }

    pub fn preserved_source_node_matches(
        &self,
        factory: &NodeFactory,
        source: Option<Node>,
        output: Option<Node>,
    ) -> bool {
        match (source, output) {
            (None, None) => true,
            (Some(source), Some(output)) if source.store_id() == factory.store().store_id() => {
                source == output
            }
            (Some(source), Some(output)) => {
                source == output || self.preserved_nodes.get_copied(source) == Some(output)
            }
            _ => false,
        }
    }

    pub(crate) fn preserved_source_node_list_matches(
        &self,
        source_store: &AstStore,
        factory: &NodeFactory,
        source: Option<NodeListId>,
        output: Option<NodeListId>,
    ) -> bool {
        match (source, output) {
            (None, None) => true,
            (Some(source), Some(output)) if source == output => true,
            (Some(source), Some(output))
                if source_store.store_id() == factory.store().store_id() =>
            {
                source == output
            }
            (Some(source), Some(output)) => self.preserved_node_lists.get(&source) == Some(&output),
            _ => false,
        }
    }

    pub(crate) fn preserved_source_modifier_list_matches(
        &self,
        source_store: &AstStore,
        factory: &NodeFactory,
        source: Option<ModifierListId>,
        output: Option<ModifierListId>,
    ) -> bool {
        match (source, output) {
            (None, None) => true,
            (Some(source), Some(output)) if source == output => true,
            (Some(source), Some(output))
                if source_store.store_id() == factory.store().store_id() =>
            {
                source == output
            }
            (Some(source), Some(output)) => {
                self.preserved_modifier_lists.get(&source) == Some(&output)
            }
            _ => false,
        }
    }

    pub(crate) fn preserved_source_raw_node_slice_matches(
        &self,
        source_store: &AstStore,
        factory: &NodeFactory,
        source: Option<RawNodeSliceId>,
        output: Option<RawNodeSliceId>,
    ) -> bool {
        match (source, output) {
            (None, None) => true,
            (Some(source), Some(output)) if source == output => true,
            (Some(source), Some(output))
                if source_store.store_id() == factory.store().store_id() =>
            {
                source == output
            }
            (Some(source), Some(output)) => {
                self.preserved_raw_node_slices.get(&source) == Some(&output)
            }
            _ => false,
        }
    }

    pub(crate) fn preserved_source_raw_string_slice_matches(
        &self,
        source_store: &AstStore,
        factory: &NodeFactory,
        source: Option<RawStringSliceId>,
        output: Option<RawStringSliceId>,
    ) -> bool {
        match (source, output) {
            (None, None) => true,
            (Some(source), Some(output)) if source == output => true,
            (Some(source), Some(output))
                if source_store.store_id() == factory.store().store_id() =>
            {
                source == output
            }
            (Some(source), Some(output)) => {
                self.preserved_raw_string_slices.get(&source) == Some(&output)
            }
            _ => false,
        }
    }

    pub fn flatten_visited_node(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        visited: Node,
        out: &mut Vec<Node>,
    ) {
        let store = Self::store_for(source, factory, visited);
        if store.kind(visited) == Kind::SyntaxList {
            let syntax_list = store.as_syntax_list(visited);
            let nodes = store
                .raw_node_slice(syntax_list.children)
                .iter()
                .flatten()
                .collect::<Vec<_>>();
            for node in nodes {
                out.push(self.preserve_node(source, factory, node));
            }
        } else {
            out.push(self.preserve_node(source, factory, visited));
        }
    }

    pub fn append_visit_slice_result(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        original: Node,
        visited: Option<Node>,
        out: &mut Vec<Node>,
    ) {
        match visited {
            Some(visited) if visited == original => {
                out.push(self.preserve_node(source, factory, original))
            }
            Some(visited) => self.flatten_visited_node(source, factory, visited, out),
            None => {}
        }
    }

    pub fn append_raw_node_slice_result(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        original: Option<Node>,
        result: Option<Node>,
        out: &mut Vec<Option<Node>>,
    ) {
        match (original, result) {
            (Some(node), Some(result)) if result == node => {
                out.push(Some(self.preserve_node(source, factory, node)))
            }
            (_, Some(result)) => {
                let result = self.preserve_node(source, factory, result);
                assert!(
                    factory.store().kind(result) != Kind::SyntaxList,
                    "raw node slices preserve slot structure and cannot be replaced by SyntaxList"
                );
                out.push(Some(result));
            }
            (_, None) => out.push(None),
        }
    }

    pub fn lift_to_block(
        &mut self,
        source: &AstStore,
        factory: &mut NodeFactory,
        node: Option<Node>,
    ) -> Option<Node> {
        let Some(node) = node else {
            let statements = factory.new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                Vec::<Node>::new(),
            );
            return Some(factory.new_block(statements, true));
        };
        let store = Self::store_for(source, factory, node);
        let nodes: Vec<Node> = if store.kind(node) == Kind::SyntaxList {
            store
                .raw_node_slice(store.as_syntax_list(node).children)
                .iter()
                .flatten()
                .collect()
        } else {
            vec![node]
        };
        let nodes = nodes
            .into_iter()
            .map(|node| self.preserve_node(source, factory, node))
            .collect::<Vec<_>>();
        let lifted = if nodes.len() == 1 {
            nodes[0]
        } else {
            let list = factory.new_node_list(
                core::undefined_text_range(),
                core::undefined_text_range(),
                nodes,
            );
            factory.new_block(list, true)
        };
        assert!(
            Self::store_for(source, factory, lifted).kind(lifted) != Kind::SyntaxList,
            "the result of visiting and lifting a Node may not be SyntaxList"
        );
        Some(lifted)
    }
}

pub struct AstImporter<'a, 'factory> {
    source: &'a AstStore,
    factory: &'factory mut NodeFactory,
    state: AstImportState,
}

impl<'a, 'factory> AstImporter<'a, 'factory> {
    pub fn new(source: &'a AstStore, factory: &'factory mut NodeFactory) -> Self {
        Self {
            source,
            factory,
            state: AstImportState::new(),
        }
    }

    pub fn source(&self) -> &'a AstStore {
        self.source
    }

    pub fn factory(&mut self) -> &mut NodeFactory {
        self.factory
    }

    pub fn preserve_node(&mut self, node: Node) -> Node {
        self.state.preserve_node(self.source, self.factory, node)
    }

    pub fn preserve_optional_node(&mut self, node: Option<Node>) -> Option<Node> {
        node.map(|node| self.preserve_node(node))
    }

    pub fn preserve_source_node_list(&mut self, list: SourceNodeList<'_>) -> NodeList {
        self.state.preserve_source_node_list(self.factory, list)
    }

    pub fn preserve_optional_source_node_list(
        &mut self,
        list: Option<SourceNodeList<'_>>,
    ) -> Option<NodeList> {
        self.state
            .preserve_optional_source_node_list(self.factory, list)
    }

    pub fn preserve_source_modifier_list(
        &mut self,
        modifiers: SourceModifierList<'_>,
    ) -> ModifierList {
        self.state
            .preserve_source_modifier_list(self.factory, modifiers)
    }

    pub fn preserve_optional_source_modifier_list(
        &mut self,
        modifiers: Option<SourceModifierList<'_>>,
    ) -> Option<ModifierList> {
        self.state
            .preserve_optional_source_modifier_list(self.factory, modifiers)
    }

    fn map_source_file_metadata(
        &mut self,
        source_file: Node,
        metadata: SourceFileCopyMetadata,
    ) -> MappedSourceFileMetadata {
        if self.source.store_id() == self.factory.store().store_id() {
            return MappedSourceFileMetadata {
                metadata,
                self_references: SourceFileSelfReferences::default(),
            };
        }

        metadata.map_nodes(source_file, |node| self.preserve_node(node))
    }

    pub fn update_source_file(
        &mut self,
        node: Node,
        statements: impl IntoOptionalNodeList,
        end_of_file_token: impl Into<Option<Node>>,
    ) -> Node {
        assert_eq!(
            node.store_id(),
            self.source.store_id(),
            "cannot update a source file from unrelated AST store"
        );
        let source_data = self.source.as_source_file(node);
        let statements = statements.into_optional_node_list().map(|list| {
            list.assert_store(self.factory.store().store_id());
            list.id()
        });
        let end_of_file_token = end_of_file_token.into();
        let source_unchanged = self.state.preserved_source_node_list_matches(
            self.source,
            self.factory,
            Some(source_data.statements),
            statements,
        ) && self.state.preserved_source_node_matches(
            self.factory,
            source_data.end_of_file_token,
            end_of_file_token,
        );
        if source_unchanged {
            let imported = self.state.preserve_node(self.source, self.factory, node);
            return self.state.record_preserved_node(
                self.source.store_id(),
                self.factory,
                node,
                imported,
            );
        }
        let mapped_metadata =
            self.map_source_file_metadata(node, SourceFileCopyMetadata::from_source(source_data));
        let updated = self
            .factory
            .update_source_file_from_store_with_mapped_metadata(
                self.source,
                node,
                source_data,
                mapped_metadata.metadata,
                statements.map(NodeList::from_id),
                end_of_file_token,
            );
        self.factory
            .restore_source_file_self_references(updated, mapped_metadata.self_references);
        updated
    }
}

pub trait NodeSliceTraversal {
    fn visit_slice_node(&mut self, node: Node) -> Option<Node>;
    fn import_slice_node(&mut self, node: Node) -> Node;
    fn append_visited_slice_node(
        &mut self,
        original: Node,
        visited: Option<Node>,
        out: &mut Vec<Node>,
    );
}

pub fn visit_slice_with<T, I>(traversal: &mut T, nodes: I) -> Option<Vec<Node>>
where
    T: NodeSliceTraversal + ?Sized,
    I: Clone + IntoIterator<Item = Node>,
    I::IntoIter: ExactSizeIterator,
{
    for (index, node) in nodes.clone().into_iter().enumerate() {
        let visited = traversal.visit_slice_node(node);
        if visited == Some(node) {
            continue;
        }

        let mut result = Vec::with_capacity(nodes.clone().into_iter().len());
        result.extend(
            nodes
                .clone()
                .into_iter()
                .take(index)
                .map(|node| traversal.import_slice_node(node)),
        );
        traversal.append_visited_slice_node(node, visited, &mut result);

        for node in nodes.clone().into_iter().skip(index + 1) {
            let visited = traversal.visit_slice_node(node);
            traversal.append_visited_slice_node(node, visited, &mut result);
        }

        return Some(result);
    }

    None
}

pub trait RawNodeSliceTraversal {
    fn visit_raw_slice_node(&mut self, node: Option<Node>) -> Option<Node>;
    fn import_raw_slice_node(&mut self, node: Node) -> Node;
    fn append_visited_raw_slice_node(
        &mut self,
        original: Option<Node>,
        visited: Option<Node>,
        out: &mut Vec<Option<Node>>,
    );
}

pub fn visit_raw_node_slice_with<T>(
    traversal: &mut T,
    nodes: SourceRawNodeSlice<'_>,
) -> Option<Vec<Option<Node>>>
where
    T: RawNodeSliceTraversal + ?Sized,
{
    for (index, node) in nodes.iter().enumerate() {
        let visited = traversal.visit_raw_slice_node(node);
        if visited == node {
            continue;
        }

        let mut result = Vec::with_capacity(nodes.iter().len());
        result.extend(
            nodes
                .iter()
                .take(index)
                .map(|node| node.map(|node| traversal.import_raw_slice_node(node))),
        );
        traversal.append_visited_raw_slice_node(node, visited, &mut result);

        for node in nodes.iter().skip(index + 1) {
            let visited = traversal.visit_raw_slice_node(node);
            traversal.append_visited_raw_slice_node(node, visited, &mut result);
        }

        return Some(result);
    }

    None
}

pub(crate) fn visit_source_node_from_id<'source, T>(runtime: &T, node: Node, id: AstNodeId) -> Node
where
    T: AstVisitEachChildRuntime<'source> + ?Sized,
{
    runtime.source_store_for_node(node).node_from_id(id)
}

pub(crate) fn visit_optional_source_node_from_id<'source, T>(
    runtime: &T,
    node: Node,
    id: OptionalAstNodeId,
) -> Option<Node>
where
    T: AstVisitEachChildRuntime<'source> + ?Sized,
{
    runtime
        .source_store_for_node(node)
        .optional_node_from_id(id)
}

pub(crate) fn visit_source_node_list_input_from_id<'source, T>(
    runtime: &T,
    node: Node,
    id: NodeListId,
) -> SourceNodeListInput
where
    T: AstVisitEachChildRuntime<'source> + ?Sized,
{
    let source = runtime.source_store_for_node(node);
    SourceNodeListInput::from_source(SourceNodeList::new(source, id))
}

pub(crate) fn visit_optional_source_node_list_input_from_id<'source, T>(
    runtime: &T,
    node: Node,
    id: OptionalNodeListId,
) -> Option<SourceNodeListInput>
where
    T: AstVisitEachChildRuntime<'source> + ?Sized,
{
    id.get()
        .map(|id| visit_source_node_list_input_from_id(runtime, node, id))
}

pub(crate) fn visit_source_modifier_list_input_from_id<'source, T>(
    runtime: &T,
    node: Node,
    id: ModifierListId,
) -> SourceModifierListInput
where
    T: AstVisitEachChildRuntime<'source> + ?Sized,
{
    let source = runtime.source_store_for_node(node);
    SourceModifierListInput::from_source(SourceModifierList::new(source, id))
}

pub(crate) fn visit_optional_source_modifier_list_input_from_id<'source, T>(
    runtime: &T,
    node: Node,
    id: OptionalModifierListId,
) -> Option<SourceModifierListInput>
where
    T: AstVisitEachChildRuntime<'source> + ?Sized,
{
    id.get()
        .map(|id| visit_source_modifier_list_input_from_id(runtime, node, id))
}

pub(crate) fn visit_source_raw_node_slice_input_from_id<'source, T>(
    runtime: &T,
    node: Node,
    id: RawNodeSliceId,
) -> SourceRawNodeSliceInput
where
    T: AstVisitEachChildRuntime<'source> + ?Sized,
{
    let source = runtime.source_store_for_node(node);
    SourceRawNodeSliceInput::from_source(SourceRawNodeSlice::new(source, id))
}

pub(crate) fn visit_optional_source_raw_node_slice_input_from_id<'source, T>(
    runtime: &T,
    node: Node,
    id: OptionalRawNodeSliceId,
) -> Option<SourceRawNodeSliceInput>
where
    T: AstVisitEachChildRuntime<'source> + ?Sized,
{
    id.get()
        .map(|id| visit_source_raw_node_slice_input_from_id(runtime, node, id))
}

pub(crate) fn visit_source_raw_string_slice_from_id<'source, T>(
    runtime: &T,
    node: Node,
    id: RawStringSliceId,
) -> RawStringSlice
where
    T: AstVisitEachChildRuntime<'source> + ?Sized,
{
    let source = runtime.source_store_for_node(node);
    id.assert_store(source.store_id());
    RawStringSlice::from_id(id)
}

pub(crate) fn visit_optional_source_raw_string_slice_from_id<'source, T>(
    runtime: &T,
    node: Node,
    id: OptionalRawStringSliceId,
) -> Option<RawStringSlice>
where
    T: AstVisitEachChildRuntime<'source> + ?Sized,
{
    id.get()
        .map(|id| visit_source_raw_string_slice_from_id(runtime, node, id))
}

pub trait AstVisitEachChildRuntime<'source> {
    fn source_store(&self) -> &AstStore;
    fn factory(&self) -> &NodeFactory;
    fn factory_mut(&mut self) -> &mut NodeFactory;
    fn preserved_node(&self, source: Node) -> Option<Node>;
    fn preserve_node(&mut self, node: Node) -> Node;
    fn record_preserved_node(&mut self, source: Node, imported: Node) -> Node;
    fn preserved_source_node_matches(&self, source: Option<Node>, output: Option<Node>) -> bool;
    fn update_source_file_from_visited(
        &mut self,
        node: Node,
        statements: Option<NodeList>,
        end_of_file_token: Option<Node>,
        source_unchanged: bool,
    ) -> Node;
    fn visit_node(&mut self, node: Option<Node>) -> Option<Node>;
    fn visit_token(&mut self, node: Option<Node>) -> Option<Node>;
    fn visit_function_body(&mut self, node: Option<Node>) -> Option<Node>;
    fn visit_iteration_body(&mut self, node: Option<Node>) -> Option<Node>;
    fn visit_embedded_statement(&mut self, node: Option<Node>) -> Option<Node>;

    fn source_store_for_node(&self, node: Node) -> &AstStore {
        AstTraversalState::store_for(self.source_store(), self.factory(), node)
    }

    fn source_store_for_store_id(&self, store_id: StoreId) -> &AstStore {
        if store_id == self.factory().store().store_id() {
            self.factory().store()
        } else {
            assert_eq!(
                store_id,
                self.source_store().store_id(),
                "visitor runtime cannot resolve unrelated AST store"
            );
            self.source_store()
        }
    }

    fn import_update_node(&mut self, node: Option<Node>) -> Option<Node> {
        node.map(|node| {
            if node.store_id() == self.factory().store().store_id() {
                node
            } else {
                self.preserve_node(node)
            }
        })
    }

    fn import_update_node_list(&mut self, list: NodeList) -> NodeList {
        if list.store_id() == self.factory().store().store_id() {
            return list;
        }

        let source = {
            let store = self.source_store_for_store_id(list.store_id());
            SourceNodeListInput::from_source(SourceNodeList::new(store, list.id()))
        };
        let nodes = source
            .iter()
            .map(|node| self.preserve_node(node))
            .collect::<Vec<_>>();
        self.factory_mut().new_node_list_with_trailing_comma(
            source.loc(),
            source.range(),
            nodes,
            source.has_trailing_comma(),
        )
    }

    fn import_update_modifier_list(&mut self, list: ModifierList) -> ModifierList {
        if list.store_id() == self.factory().store().store_id() {
            return list;
        }

        let source = {
            let store = self.source_store_for_store_id(list.store_id());
            SourceModifierListInput::from_source(SourceModifierList::new(store, list.id()))
        };
        let nodes = source
            .iter()
            .map(|node| self.preserve_node(node))
            .collect::<Vec<_>>();
        self.factory_mut().new_modifier_list(
            source.loc(),
            source.range(),
            nodes,
            source.modifier_flags(),
        )
    }

    fn import_update_raw_node_slice(&mut self, slice: RawNodeSlice) -> RawNodeSlice {
        if slice.store_id() == self.factory().store().store_id() {
            return slice;
        }

        let source = {
            let store = self.source_store_for_store_id(slice.store_id());
            SourceRawNodeSliceInput::from_source(SourceRawNodeSlice::new(store, slice.id()))
        };
        let nodes = source
            .iter()
            .map(|node| node.map(|node| self.preserve_node(node)))
            .collect::<Vec<_>>();
        self.factory_mut().new_raw_node_slice(nodes)
    }

    fn import_update_nodes(&mut self, nodes: Vec<Node>) -> Vec<Node> {
        nodes
            .into_iter()
            .map(|node| {
                self.import_update_node(Some(node))
                    .expect("required child missing")
            })
            .collect()
    }

    fn import_update_raw_nodes(&mut self, nodes: Vec<Option<Node>>) -> Vec<Option<Node>> {
        nodes
            .into_iter()
            .map(|node| {
                let imported = self.import_update_node(node);
                if let Some(imported) = imported {
                    assert!(
                        self.factory().store().kind(imported) != Kind::SyntaxList,
                        "raw node slices preserve slot structure and cannot be replaced by SyntaxList"
                    );
                }
                imported
            })
            .collect()
    }

    fn visit_nodes_input(&mut self, nodes: Option<SourceNodeListInput>) -> Option<NodeList> {
        let nodes = nodes?;
        let mut visited = Vec::with_capacity(nodes.len());
        let mut changed = false;
        for node in nodes.iter() {
            match self.visit_node(Some(node)) {
                Some(visited_node) if visited_node == node => visited.push(node),
                Some(visited_node) => {
                    changed = true;
                    let children = {
                        let store = self.source_store_for_node(visited_node);
                        if store.kind(visited_node) == Kind::SyntaxList {
                            store
                                .syntax_list_children(visited_node)
                                .expect("SyntaxList should have children")
                                .iter()
                                .flatten()
                                .collect::<Vec<_>>()
                        } else {
                            vec![visited_node]
                        }
                    };
                    visited.extend(children);
                }
                None => changed = true,
            }
        }
        if changed {
            let visited = self.import_update_nodes(visited);
            Some(self.factory_mut().new_node_list_with_trailing_comma(
                nodes.loc(),
                nodes.range(),
                visited,
                nodes.has_trailing_comma(),
            ))
        } else {
            Some(nodes.as_node_list())
        }
    }

    fn visit_modifiers_input(
        &mut self,
        modifiers: Option<SourceModifierListInput>,
    ) -> Option<ModifierList> {
        let modifiers = modifiers?;
        let mut visited = Vec::with_capacity(modifiers.nodes().len());
        let mut changed = false;
        for node in modifiers.iter() {
            match self.visit_node(Some(node)) {
                Some(visited_node) if visited_node == node => visited.push(node),
                Some(visited_node) => {
                    changed = true;
                    let children = {
                        let store = self.source_store_for_node(visited_node);
                        if store.kind(visited_node) == Kind::SyntaxList {
                            store
                                .syntax_list_children(visited_node)
                                .expect("SyntaxList should have children")
                                .iter()
                                .flatten()
                                .collect::<Vec<_>>()
                        } else {
                            vec![visited_node]
                        }
                    };
                    visited.extend(children);
                }
                None => changed = true,
            }
        }
        if changed {
            let visited = self.import_update_nodes(visited);
            Some(self.factory_mut().new_modifier_list(
                modifiers.loc(),
                modifiers.range(),
                visited,
                modifiers.modifier_flags(),
            ))
        } else {
            Some(modifiers.as_modifier_list())
        }
    }

    fn visit_parameters_input(&mut self, nodes: Option<SourceNodeListInput>) -> Option<NodeList> {
        self.visit_nodes_input(nodes)
    }

    fn visit_top_level_statements_input(
        &mut self,
        nodes: Option<SourceNodeListInput>,
    ) -> Option<NodeList> {
        self.visit_nodes_input(nodes)
    }

    fn visit_raw_node_slice_input(
        &mut self,
        nodes: Option<SourceRawNodeSliceInput>,
    ) -> Option<RawNodeSlice> {
        let nodes = nodes?;
        let mut visited = Vec::with_capacity(nodes.iter().len());
        let mut changed = false;
        for node in nodes.iter() {
            match (node, node.and_then(|node| self.visit_node(Some(node)))) {
                (Some(original), Some(visited_node)) if visited_node == original => {
                    visited.push(Some(original));
                }
                (_, Some(visited_node)) => {
                    changed = true;
                    visited.push(Some(visited_node));
                }
                (None, None) => visited.push(None),
                (Some(_), None) => {
                    changed = true;
                    visited.push(None);
                }
            }
        }
        if changed {
            let visited = self.import_update_raw_nodes(visited);
            Some(self.factory_mut().new_raw_node_slice(visited))
        } else {
            Some(nodes.as_raw_node_slice())
        }
    }

    fn preserved_source_node_list_input_matches(
        &self,
        source: Option<&SourceNodeListInput>,
        output: Option<NodeList>,
    ) -> bool {
        let Some(source) = source else {
            return output.is_none();
        };
        self.preserved_source_node_list_input_matches_default(source, output)
    }

    fn preserved_source_modifier_list_input_matches(
        &self,
        source: Option<&SourceModifierListInput>,
        output: Option<ModifierList>,
    ) -> bool {
        let Some(source) = source else {
            return output.is_none();
        };
        self.preserved_source_modifier_list_input_matches_default(source, output)
    }

    fn preserved_source_raw_node_slice_input_matches(
        &self,
        source: Option<&SourceRawNodeSliceInput>,
        output: Option<RawNodeSlice>,
    ) -> bool {
        let Some(source) = source else {
            return output.is_none();
        };
        self.preserved_source_raw_node_slice_input_matches_default(source, output)
    }

    fn preserved_source_raw_string_slice_input_matches(
        &self,
        source: Option<&SourceRawStringSliceInput>,
        output: Option<RawStringSlice>,
    ) -> bool {
        let Some(source) = source else {
            return output.is_none();
        };
        self.preserved_source_raw_string_slice_input_matches_default(source, output)
    }

    fn preserved_source_node_list_input_matches_default(
        &self,
        source: &SourceNodeListInput,
        output: Option<NodeList>,
    ) -> bool {
        let source_store = self.source_store_for_store_id(source.store_id());
        AstImportState::default().preserved_source_node_list_input_matches(
            source_store,
            self.factory(),
            Some(source),
            output,
        )
    }

    fn preserved_source_modifier_list_input_matches_default(
        &self,
        source: &SourceModifierListInput,
        output: Option<ModifierList>,
    ) -> bool {
        let source_store = self.source_store_for_store_id(source.store_id());
        AstImportState::default().preserved_source_modifier_list_input_matches(
            source_store,
            self.factory(),
            Some(source),
            output,
        )
    }

    fn preserved_source_raw_node_slice_input_matches_default(
        &self,
        source: &SourceRawNodeSliceInput,
        output: Option<RawNodeSlice>,
    ) -> bool {
        let source_store = self.source_store_for_store_id(source.store_id());
        AstImportState::default().preserved_source_raw_node_slice_input_matches(
            source_store,
            self.factory(),
            Some(source),
            output,
        )
    }

    fn preserved_source_raw_string_slice_input_matches_default(
        &self,
        source: &SourceRawStringSliceInput,
        output: Option<RawStringSlice>,
    ) -> bool {
        let source_store = self.source_store_for_store_id(source.store_id());
        AstImportState::default().preserved_source_raw_string_slice_input_matches(
            source_store,
            self.factory(),
            Some(source),
            output,
        )
    }
}

pub(crate) fn set_parent_in_children(store: &mut AstStore, node: Node) {
    store.set_parent_in_children(node);
}

pub fn range_is_synthesized(loc: core::TextRange) -> bool {
    position_is_synthesized(loc.pos()) || position_is_synthesized(loc.end())
}

pub fn is_member_name(store: &AstStore, node: Node) -> bool {
    matches!(store.kind(node), Kind::Identifier | Kind::PrivateIdentifier)
}

pub fn get_source_file_of_node(store: &AstStore, node: Option<Node>) -> Option<Node> {
    get_source_file_node_of_node(store, node)
}

pub fn is_in_json_file(store: &AstStore, node: Node) -> bool {
    store.flags(node).intersects(NodeFlags::JSON_FILE)
}

pub fn is_parse_tree_node(store: &AstStore, node: Node) -> bool {
    !store.flags(node).intersects(NodeFlags::SYNTHESIZED)
}

pub fn is_node_descendant_of(store: &AstStore, node: Option<Node>, ancestor: Option<Node>) -> bool {
    let (Some(node), Some(ancestor)) = (node, ancestor) else {
        return false;
    };
    let mut current = Some(node);
    while let Some(node) = current {
        if node == ancestor {
            return true;
        }
        current = store.parent(node);
    }
    false
}

pub fn skip_partially_emitted_expressions(store: &AstStore, node: Node) -> Node {
    skip_outer_expressions(
        store,
        node,
        OuterExpressionKinds::PARTIALLY_EMITTED_EXPRESSIONS,
    )
}

pub fn is_unterminated_literal(store: &AstStore, node: Node) -> bool {
    match store.kind(node) {
        Kind::StringLiteral => store
            .as_string_literal(node)
            .token_flags
            .contains(TokenFlags::UNTERMINATED),
        Kind::NumericLiteral => store
            .as_numeric_literal(node)
            .token_flags
            .contains(TokenFlags::UNTERMINATED),
        Kind::BigIntLiteral => store
            .as_big_int_literal(node)
            .token_flags
            .contains(TokenFlags::UNTERMINATED),
        Kind::RegularExpressionLiteral => store
            .as_regular_expression_literal(node)
            .token_flags
            .contains(TokenFlags::UNTERMINATED),
        Kind::NoSubstitutionTemplateLiteral => store
            .as_no_substitution_template_literal(node)
            .template_flags
            .contains(TokenFlags::UNTERMINATED),
        Kind::TemplateHead => store
            .as_template_head(node)
            .template_flags
            .contains(TokenFlags::UNTERMINATED),
        Kind::TemplateMiddle => store
            .as_template_middle(node)
            .template_flags
            .contains(TokenFlags::UNTERMINATED),
        Kind::TemplateTail => store
            .as_template_tail(node)
            .template_flags
            .contains(TokenFlags::UNTERMINATED),
        _ => false,
    }
}

pub fn try_get_property_name_of_binding_or_assignment_element(
    store: &AstStore,
    element: Node,
) -> Option<Node> {
    match store.kind(element) {
        Kind::BindingElement => {
            if let Some(property_name) = store.property_name(element) {
                if is_computed_property_name(store, property_name)
                    && store.expression(property_name).is_some_and(|expression| {
                        is_string_or_numeric_literal_like(store, expression)
                    })
                {
                    return store.expression(property_name);
                }
                return Some(property_name);
            }
        }
        Kind::PropertyAssignment => {
            if let Some(property_name) = store.name(element) {
                if is_computed_property_name(store, property_name)
                    && store.expression(property_name).is_some_and(|expression| {
                        is_string_or_numeric_literal_like(store, expression)
                    })
                {
                    return store.expression(property_name);
                }
                return Some(property_name);
            }
        }
        Kind::SpreadAssignment => return store.name(element),
        _ => {}
    }

    let target = match store.kind(element) {
        Kind::BindingElement => store.name(element),
        Kind::PropertyAssignment => store.initializer(element),
        Kind::ShorthandPropertyAssignment | Kind::SpreadAssignment => store.name(element),
        _ => None,
    };
    target.filter(|target| is_property_name_literal(store, *target))
}

impl NodeFactory {
    pub fn new(hooks: NodeFactoryHooks) -> Self {
        Self::with_arena_capacity(hooks, 0)
    }

    pub fn with_arena_capacity(hooks: NodeFactoryHooks, capacity: usize) -> Self {
        Self {
            hooks,
            store: AstStore::with_capacity(capacity),
            node_count: 0,
            text_count: 0,
            clone_recorder: None,
        }
    }

    pub fn fresh_with_arena_capacity(&self, capacity: usize) -> Self {
        Self::with_arena_capacity(self.hooks.clone(), capacity)
    }

    pub(crate) fn new_node(
        &mut self,
        kind: Kind,
        flags: NodeFlags,
        payload: NodePayloadId,
    ) -> Node {
        self.node_count += 1;
        let loc = core::undefined_text_range();
        let node = self.store.alloc_header(kind, NodeFlags::NONE, loc, payload);
        if let Some(on_create) = &self.hooks.on_create {
            self.store.add_flags(node, on_create(node));
        }
        if flags != NodeFlags::NONE {
            self.store.add_flags(node, flags);
        }
        node
    }

    pub fn node_count(&self) -> i32 {
        self.node_count
    }

    pub fn text_count(&self) -> i32 {
        self.text_count
    }
}

impl Default for NodeFactory {
    fn default() -> Self {
        Self::new(NodeFactoryHooks::default())
    }
}

impl NodeFactory {
    pub fn store(&self) -> &AstStore {
        &self.store
    }

    pub(crate) fn store_mut(&mut self) -> &mut AstStore {
        &mut self.store
    }

    pub fn into_store(self) -> AstStore {
        self.store
    }

    pub fn set_source_file_declaration_metadata(
        &mut self,
        node: Node,
        referenced_files: Vec<FileReference>,
        type_reference_directives: Vec<FileReference>,
        lib_reference_directives: Vec<FileReference>,
    ) {
        let data = self.store.as_source_file_mut(node);
        data.is_declaration_file = true;
        data.referenced_files = referenced_files;
        data.type_reference_directives = type_reference_directives;
        data.lib_reference_directives = lib_reference_directives;
    }

    pub fn kind(&self, node: Node) -> Kind {
        self.store.kind(node)
    }

    pub fn flags(&self, node: Node) -> NodeFlags {
        self.store.flags(node)
    }

    pub fn loc(&self, node: Node) -> core::TextRange {
        self.store.loc(node)
    }

    pub(crate) fn set_loc(&mut self, node: Node, loc: core::TextRange) {
        self.store.set_loc(node, loc);
    }

    pub(crate) fn set_flags(&mut self, node: Node, flags: NodeFlags) {
        self.store.set_flags(node, flags);
    }

    pub(crate) fn add_flags(&mut self, node: Node, flags: NodeFlags) {
        self.store.add_flags(node, flags);
    }

    pub(crate) fn remove_flags(&mut self, node: Node, flags: NodeFlags) {
        self.store.remove_flags(node, flags);
    }

    pub fn clone_node(&mut self, node: Node) -> Node {
        let cloned = self.store.shallow_clone_node(node);
        if let Some(recorder) = &mut self.clone_recorder {
            recorder.insert(node, cloned);
        }
        if let Some(on_clone) = &self.hooks.on_clone {
            on_clone(&self.store, cloned, node);
        }
        cloned
    }

    pub(crate) fn set_parent(&mut self, node: Node, parent: Option<Node>) {
        self.store.set_parent(node, parent);
    }

    pub(crate) fn set_parent_in_children(&mut self, parent: Node) {
        self.store.set_parent_in_children(parent);
    }

    pub(crate) fn new_node_list_id(
        &mut self,
        loc: core::TextRange,
        range: core::TextRange,
        nodes: impl IntoIterator<Item = Node>,
    ) -> NodeListId {
        self.store.alloc_node_list(loc, range, nodes)
    }

    pub fn new_node_list(
        &mut self,
        loc: core::TextRange,
        range: core::TextRange,
        nodes: impl IntoIterator<Item = Node>,
    ) -> NodeList {
        NodeList::from_id(self.new_node_list_id(loc, range, nodes))
    }

    pub(crate) fn new_node_list_with_trailing_comma_id(
        &mut self,
        loc: core::TextRange,
        range: core::TextRange,
        nodes: impl IntoIterator<Item = Node>,
        has_trailing_comma: bool,
    ) -> NodeListId {
        self.store
            .alloc_node_list_with_trailing_comma(loc, range, nodes, has_trailing_comma)
    }

    pub fn new_node_list_with_trailing_comma(
        &mut self,
        loc: core::TextRange,
        range: core::TextRange,
        nodes: impl IntoIterator<Item = Node>,
        has_trailing_comma: bool,
    ) -> NodeList {
        NodeList::from_id(self.new_node_list_with_trailing_comma_id(
            loc,
            range,
            nodes,
            has_trailing_comma,
        ))
    }

    pub(crate) fn new_missing_node_list_id(
        &mut self,
        loc: core::TextRange,
        range: core::TextRange,
    ) -> NodeListId {
        self.store.alloc_missing_node_list(loc, range)
    }

    pub fn new_missing_node_list(
        &mut self,
        loc: core::TextRange,
        range: core::TextRange,
    ) -> NodeList {
        NodeList::from_id(self.new_missing_node_list_id(loc, range))
    }

    pub(crate) fn new_modifier_list_id(
        &mut self,
        loc: core::TextRange,
        range: core::TextRange,
        modifiers: impl IntoIterator<Item = Node>,
        _modifier_flags: ModifierFlags,
    ) -> ModifierListId {
        let modifiers = modifiers.into_iter().collect::<Vec<_>>();
        let modifier_flags = modifiers_to_flags(&self.store, &modifiers);
        self.store
            .alloc_modifier_list(loc, range, modifiers, modifier_flags)
    }

    pub fn new_modifier_list(
        &mut self,
        loc: core::TextRange,
        range: core::TextRange,
        modifiers: impl IntoIterator<Item = Node>,
        modifier_flags: ModifierFlags,
    ) -> ModifierList {
        ModifierList::from_id(self.new_modifier_list_id(loc, range, modifiers, modifier_flags))
    }

    fn new_preserved_modifier_list(
        &mut self,
        loc: core::TextRange,
        range: core::TextRange,
        modifiers: impl IntoIterator<Item = Node>,
        modifier_flags: ModifierFlags,
    ) -> ModifierListId {
        self.store
            .alloc_modifier_list(loc, range, modifiers, modifier_flags)
    }

    pub(crate) fn new_raw_node_slice_id(
        &mut self,
        nodes: impl IntoIterator<Item = Option<Node>>,
    ) -> RawNodeSliceId {
        self.store.alloc_raw_node_slice(nodes)
    }

    pub fn new_raw_node_slice(
        &mut self,
        nodes: impl IntoIterator<Item = Option<Node>>,
    ) -> RawNodeSlice {
        RawNodeSlice::from_id(self.new_raw_node_slice_id(nodes))
    }

    pub(crate) fn new_raw_string_slice_id(
        &mut self,
        strings: impl IntoIterator<Item = impl Into<String>>,
    ) -> RawStringSliceId {
        self.store.alloc_raw_string_slice(strings)
    }

    pub(crate) fn new_raw_string_slice(
        &mut self,
        strings: impl IntoIterator<Item = impl Into<String>>,
    ) -> RawStringSlice {
        RawStringSlice::from_id(self.new_raw_string_slice_id(strings))
    }
}

impl NamedExports {}

pub(crate) fn same_optional_node(left: Option<Node>, right: Option<Node>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left == right,
        (None, None) => true,
        _ => false,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AstChildSourceSpanKind {
    Node,
    NodeList,
    NodeListElement,
    ModifierList,
    ModifierListElement,
    RawNodeSliceElement,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AstChildSourceSpan {
    pub(crate) field_id: AstChildFieldId,
    pub(crate) field_name: &'static str,
    pub(crate) kind: AstChildSourceSpanKind,
    pub(crate) index: Option<usize>,
    pub(crate) node: Option<Node>,
    pub(crate) loc: Option<core::TextRange>,
    pub(crate) range: Option<core::TextRange>,
}

impl AstChildSourceSpan {
    pub fn field_name(self) -> &'static str {
        self.field_name
    }

    pub fn kind(self) -> AstChildSourceSpanKind {
        self.kind
    }

    pub fn index(self) -> Option<usize> {
        self.index
    }

    pub fn node(self) -> Option<Node> {
        self.node
    }

    pub fn loc(self) -> Option<core::TextRange> {
        self.loc
    }

    pub fn range(self) -> Option<core::TextRange> {
        self.range
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AstChildParentIssueKind {
    ForeignChildStore { actual_store_id: StoreId },
    MissingOriginalParent,
    WrongOriginalParent { actual_parent: Node },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AstChildParentIssue {
    parent: Node,
    child: Node,
    field_name: &'static str,
    child_span_kind: AstChildSourceSpanKind,
    index: Option<usize>,
    kind: AstChildParentIssueKind,
}

impl AstChildParentIssue {
    pub fn parent(self) -> Node {
        self.parent
    }

    pub fn child(self) -> Node {
        self.child
    }

    pub fn field_name(self) -> &'static str {
        self.field_name
    }

    pub fn child_span_kind(self) -> AstChildSourceSpanKind {
        self.child_span_kind
    }

    pub fn index(self) -> Option<usize> {
        self.index
    }

    pub fn kind(self) -> AstChildParentIssueKind {
        self.kind
    }
}

pub struct StableNodeIdMap {
    source_snapshot_id: SourceSnapshotId,
    root: Node,
    local_ids: NodeSideTable<LocalAstId>,
    nodes_by_local_id: HashMap<LocalAstId, Node>,
    len: usize,
}

impl StableNodeIdMap {
    pub fn source_id(&self) -> SourceId {
        self.source_snapshot_id.source_id()
    }

    pub fn source_hash(&self) -> xxh3::Uint128 {
        self.source_snapshot_id.source_hash()
    }

    pub fn source_snapshot_id(&self) -> SourceSnapshotId {
        self.source_snapshot_id
    }

    pub fn is_current_for_source_snapshot(&self, source_snapshot_id: SourceSnapshotId) -> bool {
        self.source_snapshot_id == source_snapshot_id
    }

    pub fn root(&self) -> Node {
        self.root
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn local_id(&self, node: Node) -> Option<LocalAstId> {
        self.local_ids.get_copied(node)
    }

    pub fn stable_id(&self, node: Node) -> Option<StableNodeId> {
        self.local_id(node)
            .map(|local_id| StableNodeId::new(self.source_id(), local_id))
    }

    pub fn contains_node(&self, node: Node) -> bool {
        self.local_id(node).is_some()
    }

    pub fn node_for_local_id(&self, local_id: LocalAstId) -> Option<Node> {
        self.nodes_by_local_id.get(&local_id).copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (Node, StableNodeId)> + '_ {
        let source_id = self.source_id();
        self.local_ids
            .iter()
            .map(move |(node, local_id)| (node, StableNodeId::new(source_id, *local_id)))
    }

    fn new(source_snapshot_id: SourceSnapshotId, root: Node) -> Self {
        Self {
            source_snapshot_id,
            root,
            local_ids: NodeSideTable::default(),
            nodes_by_local_id: HashMap::new(),
            len: 0,
        }
    }

    fn insert(&mut self, node: Node, local_id: LocalAstId) {
        if self.local_ids.insert(node, local_id).is_none() {
            self.nodes_by_local_id.insert(local_id, node);
            self.len += 1;
        }
    }

    fn next_available_local_id(&self, mut local_id: LocalAstId) -> LocalAstId {
        while self.nodes_by_local_id.contains_key(&local_id) {
            local_id = LocalAstId::from_u32(local_id.as_u32().wrapping_add(1));
        }
        local_id
    }
}

const STABLE_AST_ID_FNV_OFFSET: u64 = 0xcbf29ce484222325;
const STABLE_AST_ID_FNV_PRIME: u64 = 0x100000001b3;

fn stable_ast_id_hash_add_byte(hash: u64, byte: u8) -> u64 {
    (hash ^ u64::from(byte)).wrapping_mul(STABLE_AST_ID_FNV_PRIME)
}

fn stable_ast_id_hash_add_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash = stable_ast_id_hash_add_byte(hash, *byte);
    }
    stable_ast_id_hash_add_byte(hash, 0)
}

fn stable_ast_id_hash_add_u16(hash: u64, value: u16) -> u64 {
    stable_ast_id_hash_add_bytes(hash, &value.to_le_bytes())
}

fn stable_ast_id_hash_add_i32(hash: u64, value: i32) -> u64 {
    stable_ast_id_hash_add_bytes(hash, &value.to_le_bytes())
}

fn stable_ast_id_hash_add_u32(hash: u64, value: u32) -> u64 {
    stable_ast_id_hash_add_bytes(hash, &value.to_le_bytes())
}

fn stable_ast_id_hash_to_local_id(hash: u64) -> LocalAstId {
    LocalAstId::from_u32(((hash >> 32) as u32) ^ (hash as u32))
}

impl AstStore {
    pub fn build_stable_node_ids(&self, source_file: Node, source_id: SourceId) -> StableNodeIdMap {
        assert_eq!(self.kind(source_file), Kind::SourceFile);

        let mut stable_ids =
            StableNodeIdMap::new(self.source_snapshot_id(source_file, source_id), source_file);
        let mut visited = self.new_node_map::<()>();
        let mut stack = vec![(source_file, STABLE_AST_ID_FNV_OFFSET)];

        while let Some((node, container_hash)) = stack.pop() {
            if node.store_id() != self.store_id() {
                continue;
            }
            if visited.insert_same_store(node, ()).is_some() {
                continue;
            }

            let mut child_container_hash = container_hash;
            if self.participates_in_stable_node_ids(source_file, node) {
                let local_id =
                    self.stable_node_local_id_candidate(source_file, node, container_hash);
                let local_id = stable_ids.next_available_local_id(local_id);
                stable_ids.insert(node, local_id);
                child_container_hash = self.stable_node_child_container_hash(
                    source_file,
                    node,
                    container_hash,
                    local_id,
                );
            }

            let mut children = Vec::new();
            let result = self.for_each_present_child(node, |child| {
                if child.store_id() == self.store_id() {
                    children.push(child);
                }
                ControlFlow::Continue(())
            });
            debug_assert_eq!(result, ControlFlow::Continue(()));

            for child in children.into_iter().rev() {
                stack.push((child, child_container_hash));
            }
        }

        stable_ids
    }

    pub fn source_snapshot_id(&self, source_file: Node, source_id: SourceId) -> SourceSnapshotId {
        assert_eq!(self.kind(source_file), Kind::SourceFile);
        SourceSnapshotId::new(source_id, self.as_source_file(source_file).hash())
    }

    fn stable_node_local_id_candidate(
        &self,
        source_file: Node,
        node: Node,
        container_hash: u64,
    ) -> LocalAstId {
        if node == source_file {
            return LocalAstId::from_u32(0);
        }

        let mut hash = stable_ast_id_hash_add_bytes(container_hash, b"node");
        hash = stable_ast_id_hash_add_u16(hash, self.kind(node) as i16 as u16);

        if let Some(name) = self.name(node) {
            hash = stable_ast_id_hash_add_bytes(hash, b"name");
            hash = stable_ast_id_hash_add_bytes(hash, self.text(name).as_bytes());
        } else {
            let loc = self.loc(node);
            hash = stable_ast_id_hash_add_bytes(hash, b"range");
            hash = stable_ast_id_hash_add_i32(hash, loc.pos());
            hash = stable_ast_id_hash_add_i32(hash, loc.end());
        }

        stable_ast_id_hash_to_local_id(hash)
    }

    fn stable_node_child_container_hash(
        &self,
        source_file: Node,
        node: Node,
        container_hash: u64,
        local_id: LocalAstId,
    ) -> u64 {
        if !self.stable_node_establishes_container(source_file, node) {
            return container_hash;
        }

        let hash = stable_ast_id_hash_add_bytes(container_hash, b"container");
        stable_ast_id_hash_add_u32(hash, local_id.as_u32())
    }

    fn stable_node_establishes_container(&self, source_file: Node, node: Node) -> bool {
        node == source_file
            || self.view::<AstDeclarationView>(node).is_some() && self.name(node).is_some()
    }

    fn participates_in_stable_node_ids(&self, source_file: Node, node: Node) -> bool {
        if node == source_file {
            return true;
        }
        if node_is_synthesized(self, node) || node_is_missing(self, Some(node)) {
            return false;
        }

        self.view::<AstDeclarationView>(node).is_some()
            || self.view::<AstStatementView>(node).is_some()
            || self.view::<AstExpressionView>(node).is_some()
            || self.view::<AstTypeNodeView>(node).is_some()
            || self.view::<AstNameView>(node).is_some()
    }

    pub(crate) fn debug_tree(&self, node: Node) -> String {
        let mut output = String::new();
        self.debug_tree_node(node, 0, &mut output, None);
        output
    }

    pub(crate) fn debug_tree_with_stable_node_ids(
        &self,
        node: Node,
        stable_ids: &StableNodeIdMap,
    ) -> String {
        let mut output = String::new();
        self.debug_tree_node(node, 0, &mut output, Some(stable_ids));
        output
    }

    fn debug_tree_node(
        &self,
        node: Node,
        depth: usize,
        output: &mut String,
        stable_ids: Option<&StableNodeIdMap>,
    ) {
        debug_tree_push_line(output, depth, &self.debug_tree_node_label(node, stable_ids));
        for field in self.node_layout(node).child_fields {
            self.debug_tree_child_field(node, field, depth + 1, output, stable_ids);
        }
    }

    fn debug_tree_node_label(&self, node: Node, stable_ids: Option<&StableNodeIdMap>) -> String {
        let mut label = format!("{:?}", self.kind(node));
        if let Some(stable_id) = stable_ids.and_then(|stable_ids| stable_ids.stable_id(node)) {
            label.push_str(" stable=");
            label.push_str(&stable_id.to_string());
        }
        label
    }

    fn debug_tree_child_field(
        &self,
        node: Node,
        field: &AstChildFieldDescriptor,
        depth: usize,
        output: &mut String,
        stable_ids: Option<&StableNodeIdMap>,
    ) {
        match self.child_field_value(node, field) {
            AstChildFieldValue::Node(Some(child)) => {
                debug_tree_push_line(output, depth, &format!("{}:", field.name));
                self.debug_tree_node(child, depth + 1, output, stable_ids);
            }
            AstChildFieldValue::Node(None) => {
                debug_tree_push_line(output, depth, &format!("{}: <none>", field.name));
            }
            AstChildFieldValue::NodeList(Some(nodes)) => {
                self.debug_tree_node_sequence(field.name, nodes.iter(), depth, output, stable_ids);
            }
            AstChildFieldValue::NodeList(None) => {
                debug_tree_push_line(output, depth, &format!("{}: <none>", field.name));
            }
            AstChildFieldValue::ModifierList(Some(modifiers)) => {
                self.debug_tree_node_sequence(
                    field.name,
                    modifiers.iter(),
                    depth,
                    output,
                    stable_ids,
                );
            }
            AstChildFieldValue::ModifierList(None) => {
                debug_tree_push_line(output, depth, &format!("{}: <none>", field.name));
            }
            AstChildFieldValue::RawNodeSlice(Some(nodes)) => {
                self.debug_tree_optional_node_sequence(
                    field.name,
                    nodes.iter(),
                    depth,
                    output,
                    stable_ids,
                );
            }
            AstChildFieldValue::RawNodeSlice(None) => {
                debug_tree_push_line(output, depth, &format!("{}: <none>", field.name));
            }
        }
    }

    fn debug_tree_node_sequence<I>(
        &self,
        name: &str,
        children: I,
        depth: usize,
        output: &mut String,
        stable_ids: Option<&StableNodeIdMap>,
    ) where
        I: IntoIterator<Item = Node>,
    {
        let mut children = children.into_iter().peekable();
        if children.peek().is_none() {
            debug_tree_push_line(output, depth, &format!("{name}: []"));
            return;
        }
        debug_tree_push_line(output, depth, &format!("{name}:"));
        for (index, child) in children.enumerate() {
            debug_tree_push_line(output, depth + 1, &format!("[{index}]"));
            self.debug_tree_node(child, depth + 2, output, stable_ids);
        }
    }

    fn debug_tree_optional_node_sequence<I>(
        &self,
        name: &str,
        children: I,
        depth: usize,
        output: &mut String,
        stable_ids: Option<&StableNodeIdMap>,
    ) where
        I: IntoIterator<Item = Option<Node>>,
    {
        let mut children = children.into_iter().peekable();
        if children.peek().is_none() {
            debug_tree_push_line(output, depth, &format!("{name}: []"));
            return;
        }
        debug_tree_push_line(output, depth, &format!("{name}:"));
        for (index, child) in children.enumerate() {
            match child {
                Some(child) => {
                    debug_tree_push_line(output, depth + 1, &format!("[{index}]"));
                    self.debug_tree_node(child, depth + 2, output, stable_ids);
                }
                None => {
                    debug_tree_push_line(output, depth + 1, &format!("[{index}]: <none>"));
                }
            }
        }
    }

    pub fn child_source_spans(&self, node: Node) -> Vec<AstChildSourceSpan> {
        let mut spans = Vec::new();
        self.for_each_child_source_span(node, |span| spans.push(span));
        spans
    }

    pub fn for_each_child_source_span(
        &self,
        node: Node,
        mut visit: impl FnMut(AstChildSourceSpan),
    ) {
        for field in self.node_layout(node).child_fields {
            self.for_each_child_source_span_for_field(node, field, &mut visit);
        }
    }

    pub fn for_each_child_node_source_span<F>(&self, node: Node, mut visit: F) -> ControlFlow<()>
    where
        F: FnMut(AstChildSourceSpan) -> ControlFlow<()>,
    {
        for field in self.node_layout(node).child_fields {
            self.for_each_child_node_source_span_for_field(node, field, &mut visit)?;
        }
        ControlFlow::Continue(())
    }

    pub fn child_parent_issues(&self, root: Node) -> Vec<AstChildParentIssue> {
        let mut issues = Vec::new();
        let result = self.for_each_child_parent_issue(root, |issue| {
            issues.push(issue);
            ControlFlow::Continue(())
        });
        debug_assert_eq!(result, ControlFlow::Continue(()));
        issues
    }

    pub fn first_child_parent_issue(&self, root: Node) -> Option<AstChildParentIssue> {
        let mut first = None;
        let result = self.for_each_child_parent_issue(root, |issue| {
            first = Some(issue);
            ControlFlow::Break(())
        });
        debug_assert_eq!(
            result,
            if first.is_some() {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        );
        first
    }

    pub fn for_each_child_parent_issue<F>(&self, root: Node, mut visit: F) -> ControlFlow<()>
    where
        F: FnMut(AstChildParentIssue) -> ControlFlow<()>,
    {
        self.assert_same_store(root);
        let mut visited = self.new_node_map::<()>();
        self.walk_child_parent_issues(root, &mut visited, &mut visit)
    }

    fn walk_child_parent_issues<F>(
        &self,
        parent: Node,
        visited: &mut crate::arena::StoreNodeMap<()>,
        visit: &mut F,
    ) -> ControlFlow<()>
    where
        F: FnMut(AstChildParentIssue) -> ControlFlow<()>,
    {
        if visited.insert_same_store(parent, ()).is_some() {
            return ControlFlow::Continue(());
        }

        let mut same_store_children = Vec::new();
        self.for_each_child_node_source_span(parent, |span| {
            let Some(child) = span.node() else {
                return ControlFlow::Continue(());
            };

            if let Some(issue) = self.child_parent_issue(parent, span) {
                visit(issue)?;
            }
            if child.store_id() == self.store_id() {
                same_store_children.push(child);
            }
            ControlFlow::Continue(())
        })?;

        for child in same_store_children {
            self.walk_child_parent_issues(child, visited, visit)?;
        }
        ControlFlow::Continue(())
    }

    fn child_parent_issue(
        &self,
        parent: Node,
        span: AstChildSourceSpan,
    ) -> Option<AstChildParentIssue> {
        let child = span.node()?;
        let kind = if child.store_id() != self.store_id() {
            AstChildParentIssueKind::ForeignChildStore {
                actual_store_id: child.store_id(),
            }
        } else {
            match self.original_parent(child) {
                Some(actual_parent) if actual_parent == parent => return None,
                Some(actual_parent) => {
                    AstChildParentIssueKind::WrongOriginalParent { actual_parent }
                }
                None => AstChildParentIssueKind::MissingOriginalParent,
            }
        };

        Some(AstChildParentIssue {
            parent,
            child,
            field_name: span.field_name(),
            child_span_kind: span.kind(),
            index: span.index(),
            kind,
        })
    }

    fn for_each_child_source_span_for_field(
        &self,
        node: Node,
        field: &AstChildFieldDescriptor,
        visit: &mut impl FnMut(AstChildSourceSpan),
    ) {
        match self.child_field_value(node, field) {
            AstChildFieldValue::Node(Some(child)) => {
                self.visit_node_child_source_span(field, child, None, visit);
            }
            AstChildFieldValue::Node(None) | AstChildFieldValue::NodeList(None) => {}
            AstChildFieldValue::NodeList(Some(nodes)) => visit(AstChildSourceSpan {
                field_id: field.id,
                field_name: field.name,
                kind: AstChildSourceSpanKind::NodeList,
                index: None,
                node: None,
                loc: Some(nodes.loc()),
                range: Some(nodes.range()),
            }),
            AstChildFieldValue::ModifierList(Some(modifiers)) => visit(AstChildSourceSpan {
                field_id: field.id,
                field_name: field.name,
                kind: AstChildSourceSpanKind::ModifierList,
                index: None,
                node: None,
                loc: Some(modifiers.loc()),
                range: Some(modifiers.range()),
            }),
            AstChildFieldValue::ModifierList(None) | AstChildFieldValue::RawNodeSlice(None) => {}
            AstChildFieldValue::RawNodeSlice(Some(nodes)) => {
                for (index, child) in nodes.iter().enumerate() {
                    match child {
                        Some(child) => {
                            self.visit_node_child_source_span(field, child, Some(index), visit);
                        }
                        None => visit(AstChildSourceSpan {
                            field_id: field.id,
                            field_name: field.name,
                            kind: AstChildSourceSpanKind::RawNodeSliceElement,
                            index: Some(index),
                            node: None,
                            loc: None,
                            range: None,
                        }),
                    }
                }
            }
        }
    }

    fn for_each_child_node_source_span_for_field<F>(
        &self,
        node: Node,
        field: &AstChildFieldDescriptor,
        visit: &mut F,
    ) -> ControlFlow<()>
    where
        F: FnMut(AstChildSourceSpan) -> ControlFlow<()>,
    {
        match self.child_field_value(node, field) {
            AstChildFieldValue::Node(Some(child)) => {
                self.visit_node_child_source_span_with_kind(
                    field,
                    child,
                    AstChildSourceSpanKind::Node,
                    None,
                    visit,
                )?;
            }
            AstChildFieldValue::Node(None) | AstChildFieldValue::NodeList(None) => {}
            AstChildFieldValue::NodeList(Some(nodes)) => {
                for (index, child) in nodes.iter().enumerate() {
                    self.visit_node_child_source_span_with_kind(
                        field,
                        child,
                        AstChildSourceSpanKind::NodeListElement,
                        Some(index),
                        visit,
                    )?;
                }
            }
            AstChildFieldValue::ModifierList(Some(modifiers)) => {
                for (index, child) in modifiers.iter().enumerate() {
                    self.visit_node_child_source_span_with_kind(
                        field,
                        child,
                        AstChildSourceSpanKind::ModifierListElement,
                        Some(index),
                        visit,
                    )?;
                }
            }
            AstChildFieldValue::ModifierList(None) | AstChildFieldValue::RawNodeSlice(None) => {}
            AstChildFieldValue::RawNodeSlice(Some(nodes)) => {
                for (index, child) in nodes.iter().enumerate() {
                    let Some(child) = child else {
                        continue;
                    };
                    self.visit_node_child_source_span_with_kind(
                        field,
                        child,
                        AstChildSourceSpanKind::RawNodeSliceElement,
                        Some(index),
                        visit,
                    )?;
                }
            }
        }
        ControlFlow::Continue(())
    }

    fn visit_node_child_source_span(
        &self,
        field: &AstChildFieldDescriptor,
        child: Node,
        index: Option<usize>,
        visit: &mut impl FnMut(AstChildSourceSpan),
    ) {
        let kind = if index.is_some() {
            AstChildSourceSpanKind::RawNodeSliceElement
        } else {
            AstChildSourceSpanKind::Node
        };
        let (loc, range) = self.child_same_store_source_range(child);
        visit(AstChildSourceSpan {
            field_id: field.id,
            field_name: field.name,
            kind,
            index,
            node: Some(child),
            loc,
            range,
        });
    }

    fn visit_node_child_source_span_with_kind(
        &self,
        field: &AstChildFieldDescriptor,
        child: Node,
        kind: AstChildSourceSpanKind,
        index: Option<usize>,
        visit: &mut impl FnMut(AstChildSourceSpan) -> ControlFlow<()>,
    ) -> ControlFlow<()> {
        let (loc, range) = self.child_same_store_source_range(child);
        visit(AstChildSourceSpan {
            field_id: field.id,
            field_name: field.name,
            kind,
            index,
            node: Some(child),
            loc,
            range,
        })
    }

    fn child_same_store_source_range(
        &self,
        child: Node,
    ) -> (Option<core::TextRange>, Option<core::TextRange>) {
        if child.store_id() != self.store_id() {
            return (None, None);
        }
        let loc = self.loc(child);
        (Some(loc), Some(loc))
    }
}

fn debug_tree_push_line(output: &mut String, depth: usize, text: &str) {
    for _ in 0..depth {
        output.push_str("  ");
    }
    output.push_str(text);
    output.push('\n');
}

impl AsRef<Node> for Node {
    fn as_ref(&self) -> &Node {
        self
    }
}

pub fn is_declaration_node(store: &AstStore, node: Node) -> bool {
    store.declaration_data(node).is_some()
}

pub fn is_locals_container(store: &AstStore, node: Node) -> bool {
    store.kind(node) == Kind::SourceFile || store.locals_container_data(node).is_some()
}

pub fn has_syntactic_modifier(store: &AstStore, node: Node, flags: ModifierFlags) -> bool {
    store
        .modifiers(node)
        .is_some_and(|modifiers| modifiers.modifier_flags().intersects(flags))
}

pub fn modifier_to_flag(kind: Kind) -> ModifierFlags {
    match kind {
        Kind::PublicKeyword => ModifierFlags::PUBLIC,
        Kind::PrivateKeyword => ModifierFlags::PRIVATE,
        Kind::ProtectedKeyword => ModifierFlags::PROTECTED,
        Kind::ReadonlyKeyword => ModifierFlags::READONLY,
        Kind::OverrideKeyword => ModifierFlags::OVERRIDE,
        Kind::ExportKeyword => ModifierFlags::EXPORT,
        Kind::AbstractKeyword => ModifierFlags::ABSTRACT,
        Kind::DeclareKeyword => ModifierFlags::AMBIENT,
        Kind::StaticKeyword => ModifierFlags::STATIC,
        Kind::AccessorKeyword => ModifierFlags::ACCESSOR,
        Kind::AsyncKeyword => ModifierFlags::ASYNC,
        Kind::DefaultKeyword => ModifierFlags::DEFAULT,
        Kind::ConstKeyword => ModifierFlags::CONST,
        Kind::InKeyword => ModifierFlags::IN,
        Kind::OutKeyword => ModifierFlags::OUT,
        Kind::Decorator => ModifierFlags::DECORATOR,
        _ => ModifierFlags::NONE,
    }
}

pub fn is_parameter_property_modifier(kind: Kind) -> bool {
    modifier_to_flag(kind).intersects(ModifierFlags::PARAMETER_PROPERTY_MODIFIER)
}

pub fn create_modifiers_from_modifier_flags(
    flags: ModifierFlags,
    mut create_modifier: impl FnMut(Kind) -> Node,
) -> Vec<Node> {
    let ordered_flags = [
        (ModifierFlags::EXPORT, Kind::ExportKeyword),
        (ModifierFlags::AMBIENT, Kind::DeclareKeyword),
        (ModifierFlags::DEFAULT, Kind::DefaultKeyword),
        (ModifierFlags::CONST, Kind::ConstKeyword),
        (ModifierFlags::PUBLIC, Kind::PublicKeyword),
        (ModifierFlags::PRIVATE, Kind::PrivateKeyword),
        (ModifierFlags::PROTECTED, Kind::ProtectedKeyword),
        (ModifierFlags::ABSTRACT, Kind::AbstractKeyword),
        (ModifierFlags::STATIC, Kind::StaticKeyword),
        (ModifierFlags::OVERRIDE, Kind::OverrideKeyword),
        (ModifierFlags::READONLY, Kind::ReadonlyKeyword),
        (ModifierFlags::ACCESSOR, Kind::AccessorKeyword),
        (ModifierFlags::ASYNC, Kind::AsyncKeyword),
        (ModifierFlags::IN, Kind::InKeyword),
        (ModifierFlags::OUT, Kind::OutKeyword),
    ];
    ordered_flags
        .into_iter()
        .filter_map(|(flag, kind)| flags.intersects(flag).then(|| create_modifier(kind)))
        .collect()
}

pub fn modifiers_to_flags(store: &AstStore, modifiers: &[Node]) -> ModifierFlags {
    modifiers
        .iter()
        .fold(ModifierFlags::NONE, |flags, modifier| {
            flags | modifier_to_flag(store.kind(*modifier))
        })
}

pub fn is_modifier_like(store: &AstStore, node: Node) -> bool {
    is_modifier_kind(store.kind(node)) || is_decorator(store, node)
}

pub fn is_auto_accessor_property_declaration(store: &AstStore, node: Node) -> bool {
    is_property_declaration(store, node)
        && has_syntactic_modifier(store, node, ModifierFlags::ACCESSOR)
}

pub fn is_keyword(kind: Kind) -> bool {
    is_keyword_kind(kind)
}

pub fn is_contextual_keyword(token: Kind) -> bool {
    token >= Kind::FirstContextualKeyword && token <= Kind::LastContextualKeyword
}

pub fn is_non_contextual_keyword(token: Kind) -> bool {
    is_keyword(token) && !is_contextual_keyword(token)
}

pub fn node_has_kind(store: &AstStore, node: Option<Node>, kind: Kind) -> bool {
    node.is_some_and(|node| store.kind(node) == kind)
}

pub fn is_break_or_continue_statement(store: &AstStore, node: impl AsRef<Node>) -> bool {
    matches!(
        store.kind(*node.as_ref()),
        Kind::BreakStatement | Kind::ContinueStatement
    )
}

pub fn is_class_member_modifier(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::PublicKeyword
            | Kind::PrivateKeyword
            | Kind::ProtectedKeyword
            | Kind::ReadonlyKeyword
            | Kind::OverrideKeyword
            | Kind::StaticKeyword
            | Kind::AccessorKeyword
            | Kind::DeclareKeyword
            | Kind::AbstractKeyword
            | Kind::AsyncKeyword
    )
}

pub fn is_assignment_operator(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::EqualsToken
            | Kind::PlusEqualsToken
            | Kind::MinusEqualsToken
            | Kind::AsteriskAsteriskEqualsToken
            | Kind::AsteriskEqualsToken
            | Kind::SlashEqualsToken
            | Kind::PercentEqualsToken
            | Kind::AmpersandEqualsToken
            | Kind::BarEqualsToken
            | Kind::CaretEqualsToken
            | Kind::LessThanLessThanEqualsToken
            | Kind::GreaterThanGreaterThanGreaterThanEqualsToken
            | Kind::GreaterThanGreaterThanEqualsToken
            | Kind::BarBarEqualsToken
            | Kind::AmpersandAmpersandEqualsToken
            | Kind::QuestionQuestionEqualsToken
    )
}

pub fn is_logical_binary_operator(kind: Kind) -> bool {
    matches!(kind, Kind::BarBarToken | Kind::AmpersandAmpersandToken)
}

pub fn is_logical_or_coalescing_binary_operator(kind: Kind) -> bool {
    is_logical_binary_operator(kind) || kind == Kind::QuestionQuestionToken
}

pub fn is_logical_or_coalescing_assignment_operator(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::BarBarEqualsToken
            | Kind::AmpersandAmpersandEqualsToken
            | Kind::QuestionQuestionEqualsToken
    )
}

pub fn is_logical_or_coalescing_assignment_expression(store: &AstStore, node: Node) -> bool {
    is_binary_expression(store, node)
        && store.operator_token(node).is_some_and(|operator| {
            is_logical_or_coalescing_assignment_operator(store.kind(operator))
        })
}

pub fn is_logical_expression(store: &AstStore, node: Node) -> bool {
    let mut node = node;
    loop {
        match store.kind(node) {
            Kind::ParenthesizedExpression => {
                let Some(expression) = store.expression(node) else {
                    return false;
                };
                node = expression;
            }
            Kind::PrefixUnaryExpression
                if store.as_prefix_unary_expression(node).operator == Kind::ExclamationToken =>
            {
                let Some(operand) =
                    store.optional_node_from_id(store.as_prefix_unary_expression(node).operand)
                else {
                    return false;
                };
                node = operand;
            }
            _ => {
                return is_binary_expression(store, node)
                    && store.operator_token(node).is_some_and(|operator| {
                        is_logical_or_coalescing_binary_operator(store.kind(operator))
                    });
            }
        }
    }
}

pub fn is_optional_chain(store: &AstStore, node: Node) -> bool {
    store.flags(node).contains(NodeFlags::OPTIONAL_CHAIN)
        && matches!(
            store.kind(node),
            Kind::PropertyAccessExpression
                | Kind::ElementAccessExpression
                | Kind::CallExpression
                | Kind::NonNullExpression
        )
}

pub fn is_optional_chain_root(store: &AstStore, node: Node) -> bool {
    is_optional_chain(store, node)
        && !is_non_null_expression(store, node)
        && store.question_dot_token(node).is_some()
}

pub fn is_outermost_optional_chain(store: &AstStore, node: Node) -> bool {
    if !is_optional_chain(store, node) {
        return false;
    }
    let Some(parent) = store.parent(node) else {
        return true;
    };
    !is_optional_chain(store, parent)
        || is_optional_chain_root(store, parent)
        || store
            .expression(parent)
            .is_none_or(|expression| expression != node)
}

pub fn is_expression_of_optional_chain_root(store: &AstStore, node: Node) -> bool {
    store.parent(node).is_some_and(|parent| {
        is_optional_chain_root(store, parent) && store.expression(parent) == Some(node)
    })
}

pub fn is_left_hand_side_expression(store: &AstStore, node: Node) -> bool {
    let node = skip_outer_expressions(
        store,
        node,
        OuterExpressionKinds::PARTIALLY_EMITTED_EXPRESSIONS,
    );
    matches!(
        store.kind(node),
        Kind::PropertyAccessExpression
            | Kind::ElementAccessExpression
            | Kind::NewExpression
            | Kind::CallExpression
            | Kind::JsxElement
            | Kind::JsxSelfClosingElement
            | Kind::JsxFragment
            | Kind::TaggedTemplateExpression
            | Kind::ArrayLiteralExpression
            | Kind::ParenthesizedExpression
            | Kind::ObjectLiteralExpression
            | Kind::ClassExpression
            | Kind::FunctionExpression
            | Kind::Identifier
            | Kind::PrivateIdentifier
            | Kind::RegularExpressionLiteral
            | Kind::NumericLiteral
            | Kind::BigIntLiteral
            | Kind::StringLiteral
            | Kind::NoSubstitutionTemplateLiteral
            | Kind::TemplateExpression
            | Kind::FalseKeyword
            | Kind::NullKeyword
            | Kind::ThisKeyword
            | Kind::TrueKeyword
            | Kind::SuperKeyword
            | Kind::NonNullExpression
            | Kind::ExpressionWithTypeArguments
            | Kind::MetaProperty
            | Kind::ImportKeyword
            | Kind::MissingDeclaration
    )
}

pub fn node_is_present(store: &AstStore, node: Option<Node>) -> bool {
    !node_is_missing(store, node)
}

pub fn node_is_missing(store: &AstStore, node: Option<Node>) -> bool {
    node.is_none_or(|node| {
        let loc = store.loc(node);
        loc.pos() == loc.end() && loc.pos() >= 0 && store.kind(node) != Kind::EndOfFile
    })
}

pub fn position_is_synthesized(pos: i32) -> bool {
    pos < 0
}

pub fn node_is_synthesized(store: &AstStore, node: Node) -> bool {
    let loc = store.loc(node);
    position_is_synthesized(loc.pos()) || position_is_synthesized(loc.end())
}

pub fn is_question_token(store: &AstStore, node: Option<Node>) -> bool {
    node.is_some_and(|node| store.kind(node) == Kind::QuestionToken)
}

pub fn has_question_token(store: &AstStore, node: Node) -> bool {
    is_question_token(store, store.question_token(node))
}

pub fn has_decorators(store: &AstStore, node: Node) -> bool {
    has_syntactic_modifier(store, node, ModifierFlags::DECORATOR)
}

pub fn is_function_like_kind(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::MethodSignature
            | Kind::CallSignature
            | Kind::ConstructSignature
            | Kind::IndexSignature
            | Kind::FunctionType
            | Kind::ConstructorType
            | Kind::FunctionDeclaration
            | Kind::MethodDeclaration
            | Kind::Constructor
            | Kind::GetAccessor
            | Kind::SetAccessor
            | Kind::FunctionExpression
            | Kind::ArrowFunction
    )
}

pub fn is_function_like(store: &AstStore, node: Option<Node>) -> bool {
    node.is_some_and(|node| is_function_like_kind(store.kind(node)))
}

pub fn is_function_like_declaration(store: &AstStore, node: Option<Node>) -> bool {
    node.is_some_and(|node| {
        matches!(
            store.kind(node),
            Kind::FunctionDeclaration
                | Kind::MethodDeclaration
                | Kind::Constructor
                | Kind::GetAccessor
                | Kind::SetAccessor
                | Kind::FunctionExpression
                | Kind::ArrowFunction
        )
    })
}

pub fn is_function_block(store: &AstStore, node: Option<Node>) -> bool {
    node.is_some_and(|node| {
        store.kind(node) == Kind::Block
            && store
                .parent(node)
                .is_some_and(|parent| is_function_like(store, Some(parent)))
    })
}

pub fn is_declaration(store: &AstStore, node: Node) -> bool {
    if store.kind(node) == Kind::TypeParameter {
        store.parent(node).is_some()
    } else {
        is_declaration_node(store, node)
    }
}

pub fn is_statement_but_not_declaration_kind(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::BreakStatement
            | Kind::ContinueStatement
            | Kind::DebuggerStatement
            | Kind::DoStatement
            | Kind::ExpressionStatement
            | Kind::EmptyStatement
            | Kind::ForInStatement
            | Kind::ForOfStatement
            | Kind::ForStatement
            | Kind::IfStatement
            | Kind::LabeledStatement
            | Kind::ReturnStatement
            | Kind::SwitchStatement
            | Kind::ThrowStatement
            | Kind::TryStatement
            | Kind::VariableStatement
            | Kind::WhileStatement
            | Kind::WithStatement
            | Kind::NotEmittedStatement
    )
}

pub fn is_statement_but_not_declaration(store: &AstStore, node: Node) -> bool {
    is_statement_but_not_declaration_kind(store.kind(node))
}

pub fn is_declaration_statement_kind(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::FunctionDeclaration
            | Kind::MissingDeclaration
            | Kind::ClassDeclaration
            | Kind::InterfaceDeclaration
            | Kind::TypeAliasDeclaration
            | Kind::JSTypeAliasDeclaration
            | Kind::EnumDeclaration
            | Kind::ModuleDeclaration
            | Kind::ImportDeclaration
            | Kind::JSImportDeclaration
            | Kind::ImportEqualsDeclaration
            | Kind::ExportDeclaration
            | Kind::ExportAssignment
            | Kind::NamespaceExportDeclaration
    )
}

pub fn is_declaration_statement(store: &AstStore, node: Node) -> bool {
    is_declaration_statement_kind(store.kind(node))
}

pub fn is_declaration_or_variable_statement(store: &AstStore, node: Node) -> bool {
    is_declaration_statement(store, node) || is_variable_statement(store, node)
}

pub fn statement_container_statements(
    store: &AstStore,
    container: Node,
) -> Option<SourceNodeList<'_>> {
    let statements_container = if store.kind(container) == Kind::SourceFile {
        container
    } else {
        store.body(container).unwrap_or(container)
    };
    store.source_statements(statements_container)
}

pub fn is_block_statement(store: &AstStore, node: Node) -> bool {
    if store.kind(node) != Kind::Block {
        return false;
    }
    if store
        .parent(node)
        .is_some_and(|parent| matches!(store.kind(parent), Kind::TryStatement | Kind::CatchClause))
    {
        return false;
    }
    !is_function_block(store, Some(node))
}

pub fn is_statement(store: &AstStore, node: Node) -> bool {
    is_statement_but_not_declaration_kind(store.kind(node))
        || is_declaration_statement_kind(store.kind(node))
        || is_block_statement(store, node)
}

pub fn is_class_like(store: &AstStore, node: Node) -> bool {
    matches!(
        store.kind(node),
        Kind::ClassDeclaration | Kind::ClassExpression
    )
}

pub fn get_heritage_elements(store: &AstStore, node: Node, kind: Kind) -> Vec<Node> {
    let Some(clauses) = store.heritage_clauses(node) else {
        return Vec::new();
    };
    for clause in clauses.iter() {
        if store.as_heritage_clause(clause).token == kind {
            return store
                .node_list(store.as_heritage_clause(clause).types)
                .iter()
                .collect();
        }
    }
    Vec::new()
}

pub fn get_extends_heritage_clause_element(store: &AstStore, node: Node) -> Option<Node> {
    get_heritage_elements(store, node, Kind::ExtendsKeyword)
        .into_iter()
        .next()
}

pub fn get_class_extends_heritage_element(
    store: &AstStore,
    node: impl AsRef<Node>,
) -> Option<Node> {
    get_extends_heritage_clause_element(store, *node.as_ref())
}

pub fn get_implements_type_nodes(store: &AstStore, node: impl AsRef<Node>) -> Vec<Node> {
    get_heritage_elements(store, *node.as_ref(), Kind::ImplementsKeyword)
}

pub fn is_type_node_kind(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::AnyKeyword
            | Kind::UnknownKeyword
            | Kind::NumberKeyword
            | Kind::BigIntKeyword
            | Kind::StringKeyword
            | Kind::BooleanKeyword
            | Kind::SymbolKeyword
            | Kind::VoidKeyword
            | Kind::UndefinedKeyword
            | Kind::NullKeyword
            | Kind::NeverKeyword
            | Kind::IntrinsicKeyword
            | Kind::ThisKeyword
            | Kind::ObjectKeyword
    ) || (Kind::FirstTypeNode <= kind && kind <= Kind::LastTypeNode)
}

pub fn is_type_node(store: &AstStore, node: Node) -> bool {
    is_type_node_kind(store.kind(node))
}

pub fn find_ancestor(
    store: &AstStore,
    node: Option<Node>,
    callback: impl Fn(&AstStore, Node) -> bool,
) -> Option<Node> {
    let mut current = node;
    while let Some(node) = current {
        if callback(store, node) {
            return Some(node);
        }
        current = store.parent(node);
    }
    None
}

pub fn find_ancestor_kind(store: &AstStore, node: Option<Node>, kind: Kind) -> Option<Node> {
    find_ancestor(store, node, |store, node| store.kind(node) == kind)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FindAncestorResult {
    False,
    True,
    Quit,
}

pub fn to_find_ancestor_result(value: bool) -> FindAncestorResult {
    if value {
        FindAncestorResult::True
    } else {
        FindAncestorResult::False
    }
}

pub fn find_ancestor_or_quit(
    store: &AstStore,
    node: Option<Node>,
    mut callback: impl FnMut(&AstStore, Node) -> FindAncestorResult,
) -> Option<Node> {
    let mut current = node;
    while let Some(node) = current {
        match callback(store, node) {
            FindAncestorResult::Quit => return None,
            FindAncestorResult::True => return Some(node),
            FindAncestorResult::False => current = store.parent(node),
        }
    }
    None
}

pub fn is_trivia(kind: Kind) -> bool {
    kind >= Kind::FirstTriviaToken && kind <= Kind::LastTriviaToken
}

pub fn is_template_literal_kind(kind: Kind) -> bool {
    kind >= Kind::FirstTemplateToken && kind <= Kind::LastTemplateToken
}

pub fn is_string_text_containing_node(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let kind = store.kind(*node.as_ref());
    kind == Kind::StringLiteral || is_template_literal_kind(kind)
}

pub fn is_type_keyword_token(store: &AstStore, node: impl AsRef<Node>) -> bool {
    store.kind(*node.as_ref()) == Kind::TypeKeyword
}

pub fn is_template_literal_token(store: &AstStore, node: impl AsRef<Node>) -> bool {
    is_template_literal_kind(store.kind(*node.as_ref()))
}

pub fn is_label_name(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let node = *node.as_ref();
    is_identifier(store, node)
        && (store.parent(node).is_some_and(|parent| {
            is_labeled_statement(store, parent) && store.label(parent) == Some(node)
        }) || store.parent(node).is_some_and(|parent| {
            is_break_or_continue_statement(store, parent) && store.label(parent) == Some(node)
        }))
}

pub fn is_jump_statement_target(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let node = *node.as_ref();
    is_identifier(store, node)
        && store.parent(node).is_some_and(|parent| {
            is_break_or_continue_statement(store, parent) && store.label(parent) == Some(node)
        })
}

pub fn get_non_assigned_name_of_declaration(store: &AstStore, declaration: Node) -> Option<Node> {
    match store.kind(declaration) {
        Kind::BinaryExpression | Kind::CallExpression => {
            match get_assignment_declaration_kind(store, declaration)? {
                JSDeclarationKind::Property
                | JSDeclarationKind::ThisProperty
                | JSDeclarationKind::PrototypeProperty
                | JSDeclarationKind::ExportsProperty => {
                    let left = store.left(declaration)?;
                    get_element_or_property_access_name(store, left).or(Some(left))
                }
                JSDeclarationKind::ObjectDefinePropertyValue
                | JSDeclarationKind::ObjectDefinePropertyExports => store
                    .arguments(declaration)
                    .and_then(|arguments| arguments.iter().nth(1)),
                _ => None,
            }
        }
        Kind::ExportAssignment => {
            let expression = store.expression(declaration)?;
            is_identifier(store, expression).then_some(expression)
        }
        _ => store.name(declaration),
    }
}

pub fn get_assigned_name(store: &AstStore, node: Node) -> Option<Node> {
    let parent = store.parent(node)?;
    match store.kind(parent) {
        Kind::PropertyAssignment | Kind::BindingElement => store.name(parent),
        Kind::BinaryExpression => {
            if store.right(parent) != Some(node) {
                return None;
            }
            let left = store.left(parent)?;
            match store.kind(left) {
                Kind::Identifier => Some(left),
                Kind::PropertyAccessExpression => store.name(left),
                Kind::ElementAccessExpression => {
                    let argument = store.argument_expression(left)?;
                    let argument = skip_parentheses(store, argument);
                    is_string_or_numeric_literal_like(store, argument).then_some(argument)
                }
                _ => None,
            }
        }
        Kind::VariableDeclaration => {
            let name = store.name(parent)?;
            is_identifier(store, name).then_some(name)
        }
        _ => None,
    }
}

pub fn get_declaration_name(store: &AstStore, declaration: impl AsRef<Node>) -> String {
    let Some(name) = get_non_assigned_name_of_declaration(store, *declaration.as_ref()) else {
        return String::new();
    };
    if is_computed_property_name(store, name) {
        if let Some(expression) = store.expression(name) {
            if is_string_or_numeric_literal_like(store, expression) {
                return store.text(expression);
            }
            if is_property_access_expression(store, expression) {
                return store
                    .name(expression)
                    .map(|name| store.text(name))
                    .unwrap_or_default();
            }
        }
    } else if is_property_name(store, name) {
        return store.text(name);
    }
    String::new()
}

pub fn get_declaration_from_name(store: &AstStore, name: Option<Node>) -> Option<Node> {
    let name = name?;
    let parent = store.parent(name)?;
    match store.kind(name) {
        Kind::StringLiteral | Kind::NoSubstitutionTemplateLiteral | Kind::NumericLiteral => {
            if is_computed_property_name(store, parent) {
                return store.parent(parent);
            }
            if is_declaration(store, parent) && store.name(parent) == Some(name) {
                return Some(parent);
            }
        }
        Kind::Identifier => {
            if is_declaration(store, parent) {
                return (store.name(parent) == Some(name)).then_some(parent);
            }
            if is_qualified_name(store, parent) {
                return None;
            }
            if let Some(bin_exp) = store.parent(parent) {
                if is_binary_expression(store, bin_exp)
                    && get_assignment_declaration_kind(store, bin_exp).is_some()
                    && get_name_of_declaration(store, Some(bin_exp)) == Some(name)
                {
                    return Some(bin_exp);
                }
            }
        }
        Kind::PrivateIdentifier => {
            if is_declaration(store, parent) && store.name(parent) == Some(name) {
                return Some(parent);
            }
        }
        _ => {}
    }
    None
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JSDeclarationKind {
    ModuleExports,
    ExportsProperty,
    ThisProperty,
    Property,
    PrototypeProperty,
    ObjectDefinePropertyValue,
    ObjectDefinePropertyExports,
}

pub fn get_assignment_declaration_kind(store: &AstStore, node: Node) -> Option<JSDeclarationKind> {
    match store.kind(node) {
        Kind::BinaryExpression => {
            if !store
                .operator_token(node)
                .is_some_and(|operator| store.kind(operator) == Kind::EqualsToken)
            {
                return None;
            }
            let left = store.left(node)?;
            let right = store.right(node)?;
            if !is_access_expression(store, left) {
                return None;
            }
            if is_in_js_file(store, left) {
                if is_module_exports_access_expression(store, left)
                    && !is_exports_identifier(store, right)
                {
                    return Some(JSDeclarationKind::ModuleExports);
                }
                if store.expression(left).is_some_and(|expression| {
                    is_module_exports_access_expression(store, expression)
                        || is_exports_identifier(store, expression)
                }) && get_element_or_property_access_name(store, left).is_some()
                {
                    return Some(JSDeclarationKind::ExportsProperty);
                }
                if store
                    .expression(left)
                    .is_some_and(|expression| store.kind(expression) == Kind::ThisKeyword)
                {
                    return Some(JSDeclarationKind::ThisProperty);
                }
            }
            if store.kind(left) == Kind::PropertyAccessExpression
                && store.expression(left).is_some_and(|expression| {
                    is_entity_name_expression_ex(store, expression, is_in_js_file(store, left))
                })
                && store
                    .name(left)
                    .is_some_and(|name| is_identifier(store, name))
                || store.kind(left) == Kind::ElementAccessExpression
                    && store.expression(left).is_some_and(|expression| {
                        is_entity_name_expression_ex(store, expression, is_in_js_file(store, left))
                    })
            {
                return Some(JSDeclarationKind::Property);
            }
        }
        Kind::CallExpression => {
            if is_in_js_file(store, node) && is_bindable_object_define_property_call(store, node) {
                let entity_name = store
                    .arguments(node)
                    .and_then(|arguments| arguments.first())?;
                if is_exports_identifier(store, entity_name)
                    || is_module_exports_access_expression(store, entity_name)
                {
                    return Some(JSDeclarationKind::ObjectDefinePropertyExports);
                }
                return Some(JSDeclarationKind::ObjectDefinePropertyValue);
            }
        }
        _ => {}
    }
    None
}

pub fn get_assignment_declaration_property_access_kind(
    store: &AstStore,
    lhs: Node,
) -> Option<JSDeclarationKind> {
    let expression = store.expression(lhs)?;
    if store.kind(expression) == Kind::ThisKeyword {
        return Some(JSDeclarationKind::ThisProperty);
    }
    if is_module_exports_access_expression(store, lhs) {
        return Some(JSDeclarationKind::ModuleExports);
    }
    if is_bindable_static_name_expression(store, expression, true) {
        if is_prototype_access(store, expression) {
            return Some(JSDeclarationKind::PrototypeProperty);
        }

        let mut next_to_last = lhs;
        while store
            .expression(next_to_last)
            .is_some_and(|expression| !is_identifier(store, expression))
        {
            next_to_last = store.expression(next_to_last).unwrap();
        }
        let id = store.expression(next_to_last)?;
        if (is_exports_identifier(store, id)
            || is_module_exports_access_expression(store, next_to_last))
            && is_bindable_static_access_expression(store, lhs, true)
        {
            return Some(JSDeclarationKind::ExportsProperty);
        }
        if is_bindable_static_name_expression(store, lhs, true)
            || is_element_access_expression(store, lhs) && is_dynamic_name(store, lhs)
        {
            return Some(JSDeclarationKind::Property);
        }
    }
    None
}

pub fn assignment_declaration_target(store: &AstStore, node: Node) -> Option<Node> {
    match store.kind(node) {
        Kind::BinaryExpression => store.left(node),
        Kind::CallExpression => store
            .arguments(node)
            .and_then(|arguments| arguments.first()),
        _ => None,
    }
}

pub fn assignment_declaration_initializer(store: &AstStore, node: Node) -> Option<Node> {
    match store.kind(node) {
        Kind::BinaryExpression => store.right(node),
        Kind::CallExpression => store
            .arguments(node)
            .and_then(|arguments| arguments.iter().nth(2)),
        _ => None,
    }
}

pub fn is_write_access_for_reference(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let node = *node.as_ref();
    get_declaration_from_name(store, Some(node)).is_some_and(|decl| {
        store.flags(decl).intersects(NodeFlags::Ambient) || is_write_access(store, decl)
    }) || store.kind(node) == Kind::DefaultKeyword
        || is_write_access(store, node)
}

pub fn get_super_container(
    store: &AstStore,
    node: impl AsRef<Node>,
    stop_on_functions: bool,
) -> Option<Node> {
    let mut current = store.parent(*node.as_ref());
    while let Some(node) = current {
        match store.kind(node) {
            Kind::ComputedPropertyName => {
                current = store.parent(node).and_then(|parent| store.parent(parent));
                continue;
            }
            Kind::FunctionDeclaration | Kind::FunctionExpression | Kind::ArrowFunction => {
                if stop_on_functions {
                    return Some(node);
                }
            }
            Kind::PropertyDeclaration
            | Kind::PropertySignature
            | Kind::MethodDeclaration
            | Kind::MethodSignature
            | Kind::Constructor
            | Kind::GetAccessor
            | Kind::SetAccessor
            | Kind::ClassStaticBlockDeclaration => return Some(node),
            Kind::Decorator => {
                if let Some(parent) = store.parent(node) {
                    if store.kind(parent) == Kind::Parameter
                        && store
                            .parent(parent)
                            .is_some_and(|grandparent| is_class_element(store, grandparent))
                    {
                        current = store.parent(parent);
                        continue;
                    }
                    if is_class_element(store, parent) {
                        current = Some(parent);
                        continue;
                    }
                }
            }
            _ => {}
        }
        current = store.parent(node);
    }
    None
}

pub fn get_node_id(store: &AstStore, node: Node) -> NodeId {
    store.get_node_id(node)
}

pub fn is_string_literal_like(store: &AstStore, node: Node) -> bool {
    matches!(
        store.kind(node),
        Kind::StringLiteral | Kind::NoSubstitutionTemplateLiteral
    )
}

pub fn is_string_or_numeric_literal_like(store: &AstStore, node: Node) -> bool {
    is_string_literal_like(store, node) || is_numeric_literal(store, node)
}

pub fn is_binding_pattern(store: &AstStore, node: Node) -> bool {
    matches!(
        store.kind(node),
        Kind::ObjectBindingPattern | Kind::ArrayBindingPattern
    )
}

pub fn is_for_in_or_of_statement(store: &AstStore, node: Option<Node>) -> bool {
    node.is_some_and(|node| {
        matches!(
            store.kind(node),
            Kind::ForInStatement | Kind::ForOfStatement
        )
    })
}

pub fn is_function_expression_or_arrow_function(store: &AstStore, node: Node) -> bool {
    is_function_expression(store, node) || is_arrow_function(store, node)
}

pub fn is_property_name_literal(store: &AstStore, node: Node) -> bool {
    matches!(
        store.kind(node),
        Kind::Identifier
            | Kind::StringLiteral
            | Kind::NoSubstitutionTemplateLiteral
            | Kind::NumericLiteral
    )
}

pub fn is_boolean_literal(store: &AstStore, node: Node) -> bool {
    matches!(store.kind(node), Kind::TrueKeyword | Kind::FalseKeyword)
}

pub fn is_super_call(store: &AstStore, node: Node) -> bool {
    is_call_expression(store, node)
        && store
            .expression(node)
            .is_some_and(|expression| store.kind(expression) == Kind::SuperKeyword)
}

pub fn is_import_call(store: &AstStore, node: Node) -> bool {
    if !is_call_expression(store, node) {
        return false;
    }
    let Some(expression) = store.expression(node) else {
        return false;
    };
    store.kind(expression) == Kind::ImportKeyword
        || (is_meta_property(store, expression)
            && store.keyword_token(expression) == Some(Kind::ImportKeyword)
            && store.text(expression) == "defer")
}

pub fn require_call_argument(
    store: &AstStore,
    node: Node,
    require_string_literal_like_argument: bool,
) -> Option<Node> {
    if !is_call_expression(store, node) {
        return None;
    }
    let expression = store.expression(node)?;
    if !is_identifier(store, expression) || store.text(expression) != "require" {
        return None;
    }
    let arguments = store.arguments(node)?;
    if arguments.len() != 1 {
        return None;
    }
    let argument = arguments.first()?;
    if require_string_literal_like_argument && !is_string_literal_like(store, argument) {
        return None;
    }
    Some(argument)
}

pub fn is_require_call(
    store: &AstStore,
    node: Node,
    require_string_literal_like_argument: bool,
) -> bool {
    require_call_argument(store, node, require_string_literal_like_argument).is_some()
}

pub fn variable_declaration_for_binding_element(
    store: &AstStore,
    binding_element: Node,
) -> Option<Node> {
    if !is_binding_element(store, binding_element) {
        return None;
    }
    let binding_pattern = store.parent(binding_element)?;
    if !is_binding_pattern(store, binding_pattern) {
        return None;
    }
    let declaration = store.parent(binding_pattern)?;
    is_variable_declaration(store, declaration).then_some(declaration)
}

fn variable_declaration_for_require_check(store: &AstStore, node: Node) -> Option<Node> {
    match store.kind(node) {
        Kind::VariableDeclaration => Some(node),
        Kind::BindingElement => variable_declaration_for_binding_element(store, node),
        _ => None,
    }
}

fn is_variable_declaration_initialized_with_require(
    store: &AstStore,
    declaration: Node,
    allow_accessed_require: bool,
) -> bool {
    if !is_variable_declaration(store, declaration) || !is_in_js_file(store, declaration) {
        return false;
    }
    let Some(mut initializer) = store.initializer(declaration) else {
        return false;
    };
    if allow_accessed_require {
        initializer = get_leftmost_access_expression(store, initializer);
    }
    store
        .parent(declaration)
        .and_then(|parent| store.parent(parent))
        .is_some_and(|parent| {
            !get_combined_modifier_flags(store, parent).intersects(ModifierFlags::EXPORT)
        })
        && store.r#type(declaration).is_none()
        && is_require_call(store, initializer, true)
}

pub fn is_variable_declaration_initialized_to_require(store: &AstStore, node: Node) -> bool {
    variable_declaration_for_require_check(store, node).is_some_and(|declaration| {
        is_variable_declaration_initialized_with_require(store, declaration, false)
    })
}

pub fn is_variable_declaration_initialized_to_bare_or_accessed_require(
    store: &AstStore,
    node: Node,
) -> bool {
    is_variable_declaration_initialized_with_require(store, node, true)
}

pub fn get_module_specifier_of_bare_or_accessed_require(
    store: &AstStore,
    node: Node,
) -> Option<Node> {
    if is_variable_declaration_initialized_with_require(store, node, false) {
        return store
            .initializer(node)
            .and_then(|initializer| require_call_argument(store, initializer, true));
    }
    if is_variable_declaration_initialized_with_require(store, node, true) {
        let leftmost = get_leftmost_access_expression(store, store.initializer(node)?);
        return require_call_argument(store, leftmost, true);
    }
    None
}

pub fn is_require_variable_statement(store: &AstStore, statement: Node) -> bool {
    if !is_variable_statement(store, statement) {
        return false;
    }
    let Some(declaration_list) = store.declaration_list(statement) else {
        return false;
    };
    let Some(declarations) = store.declarations(declaration_list) else {
        return false;
    };
    let declarations: Vec<Node> = declarations.iter().collect();
    !declarations.is_empty()
        && declarations
            .into_iter()
            .all(|declaration| is_variable_declaration_initialized_to_require(store, declaration))
}

pub fn is_primitive_literal_value(store: &AstStore, node: Node, include_big_int: bool) -> bool {
    match store.kind(node) {
        Kind::TrueKeyword
        | Kind::FalseKeyword
        | Kind::NumericLiteral
        | Kind::StringLiteral
        | Kind::NoSubstitutionTemplateLiteral => true,
        Kind::BigIntLiteral => include_big_int,
        Kind::PrefixUnaryExpression => {
            let unary = store.as_prefix_unary_expression(node);
            let Some(operand) = store.optional_node_from_id(unary.operand) else {
                return false;
            };
            match unary.operator {
                Kind::MinusToken => {
                    is_numeric_literal(store, operand)
                        || (include_big_int && is_big_int_literal(store, operand))
                }
                Kind::PlusToken => is_numeric_literal(store, operand),
                _ => false,
            }
        }
        _ => false,
    }
}

pub fn module_export_name_is_default(store: &AstStore, node: Node) -> bool {
    matches!(store.kind(node), Kind::Identifier | Kind::StringLiteral)
        && store.text(node) == "default"
}

pub fn is_identifier_name(store: &AstStore, node: Node) -> bool {
    let Some(parent) = store.parent(node) else {
        return false;
    };
    match store.kind(parent) {
        Kind::PropertyDeclaration
        | Kind::PropertySignature
        | Kind::MethodDeclaration
        | Kind::MethodSignature
        | Kind::GetAccessor
        | Kind::SetAccessor
        | Kind::EnumMember
        | Kind::PropertyAssignment
        | Kind::PropertyAccessExpression => store.name(parent) == Some(node),
        Kind::QualifiedName => store.right(parent) == Some(node),
        Kind::BindingElement | Kind::ImportSpecifier => store.property_name(parent) == Some(node),
        Kind::ExportSpecifier
        | Kind::JsxAttribute
        | Kind::JsxSelfClosingElement
        | Kind::JsxOpeningElement
        | Kind::JsxClosingElement => true,
        _ => false,
    }
}

pub fn is_signed_numeric_literal(store: &AstStore, node: Node) -> bool {
    if store.kind(node) != Kind::PrefixUnaryExpression {
        return false;
    }
    let unary = store.as_prefix_unary_expression(node);
    matches!(unary.operator, Kind::PlusToken | Kind::MinusToken)
        && store
            .optional_node_from_id(unary.operand)
            .is_some_and(|operand| is_numeric_literal(store, operand))
}

pub fn is_dynamic_name(store: &AstStore, name: Node) -> bool {
    let expression = match store.kind(name) {
        Kind::ComputedPropertyName => store.expression(name),
        Kind::ElementAccessExpression => {
            let Some(argument) = store.argument_expression(name) else {
                return false;
            };
            Some(skip_outer_expressions(
                store,
                argument,
                OuterExpressionKinds::PARENTHESES,
            ))
        }
        _ => return false,
    };
    let Some(expression) = expression else {
        return false;
    };
    !is_string_or_numeric_literal_like(store, expression)
        && !is_signed_numeric_literal(store, expression)
}

pub fn is_import_meta(store: &AstStore, node: Node) -> bool {
    store.kind(node) == Kind::MetaProperty
        && store.keyword_token(node) == Some(Kind::ImportKeyword)
        && store
            .name(node)
            .is_some_and(|name| store.text(name) == "meta")
}

pub fn has_dynamic_name(store: &AstStore, declaration: Node) -> bool {
    get_name_of_declaration(store, Some(declaration))
        .is_some_and(|name| is_dynamic_name(store, name))
}

pub fn is_object_literal_method(store: &AstStore, node: Option<Node>) -> bool {
    node.is_some_and(|node| {
        store.kind(node) == Kind::MethodDeclaration
            && store
                .parent(node)
                .is_some_and(|parent| store.kind(parent) == Kind::ObjectLiteralExpression)
    })
}

pub fn is_parameter_property_declaration(store: &AstStore, node: Node, parent: Node) -> bool {
    is_parameter_declaration(store, node)
        && has_syntactic_modifier(store, node, ModifierFlags::PARAMETER_PROPERTY_MODIFIER)
        && store.kind(parent) == Kind::Constructor
}

pub fn is_this_identifier(store: &AstStore, node: Node) -> bool {
    is_identifier(store, node) && store.text(node) == "this"
}

pub fn is_this_parameter(store: &AstStore, node: Node) -> bool {
    is_parameter_declaration(store, node)
        && store
            .name(node)
            .is_some_and(|name| is_this_identifier(store, name))
}

pub fn is_in_js_file(store: &AstStore, node: Node) -> bool {
    store.flags(node).contains(NodeFlags::JAVA_SCRIPT_FILE)
}

pub fn is_part_of_type_query(store: &AstStore, node: Node) -> bool {
    let mut node = node;
    while matches!(store.kind(node), Kind::QualifiedName | Kind::Identifier) {
        let Some(parent) = store.parent(node) else {
            return false;
        };
        node = parent;
    }
    store.kind(node) == Kind::TypeQuery
}

pub fn is_part_of_parameter_declaration(store: &AstStore, node: Node) -> bool {
    store.kind(get_root_declaration(store, node)) == Kind::Parameter
}

pub fn is_enum_const(store: &AstStore, node: Node) -> bool {
    get_combined_modifier_flags(store, node).intersects(ModifierFlags::CONST)
}

pub fn is_async_function(store: &AstStore, node: Node) -> bool {
    matches!(
        store.kind(node),
        Kind::FunctionDeclaration
            | Kind::FunctionExpression
            | Kind::ArrowFunction
            | Kind::MethodDeclaration
    ) && has_syntactic_modifier(store, node, ModifierFlags::ASYNC)
}

pub fn get_immediately_invoked_function_expression(
    store: &AstStore,
    function: Node,
) -> Option<Node> {
    if !is_function_expression_or_arrow_function(store, function) {
        return None;
    }
    let mut previous = function;
    let mut parent = store.parent(function);
    while let Some(parent_node) = parent
        && is_parenthesized_expression(store, parent_node)
    {
        previous = parent_node;
        parent = store.parent(previous);
    }
    parent.filter(|parent| {
        is_call_expression(store, *parent) && store.expression(*parent) == Some(previous)
    })
}

pub fn is_dotted_name(store: &AstStore, node: Node) -> bool {
    is_identifier(store, node)
        || store.kind(node) == Kind::ThisKeyword
        || store.kind(node) == Kind::SuperKeyword
        || is_meta_property(store, node)
        || (is_property_access_expression(store, node)
            && store
                .expression(node)
                .is_some_and(|expression| is_dotted_name(store, expression)))
        || (is_parenthesized_expression(store, node)
            && store
                .expression(node)
                .is_some_and(|expression| is_dotted_name(store, expression)))
}

pub fn is_push_or_unshift_identifier(store: &AstStore, node: Node) -> bool {
    let text = store.text(node);
    text == "push" || text == "unshift"
}

pub fn is_const_assertion(store: &AstStore, node: Node) -> bool {
    matches!(
        store.kind(node),
        Kind::TypeAssertionExpression | Kind::AsExpression
    ) && store
        .type_node(node)
        .is_some_and(|type_node| is_const_type_reference(store, type_node))
}

pub fn is_nullish_coalesce(store: &AstStore, node: Node) -> bool {
    is_binary_expression(store, node)
        && store
            .operator_token(node)
            .is_some_and(|operator| store.kind(operator) == Kind::QuestionQuestionToken)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ModuleInstanceState {
    Unknown,
    NonInstantiated,
    Instantiated,
    ConstEnumOnly,
}

pub fn get_module_instance_state(store: &AstStore, node: Node) -> ModuleInstanceState {
    fn pop_ancestor(store: &AstStore, ancestors: &mut Vec<Node>, node: Node) -> Option<Node> {
        ancestors.pop().or_else(|| store.parent(node))
    }

    fn get(
        store: &AstStore,
        node: Node,
        ancestors: &mut Vec<Node>,
        visited: &mut HashMap<NodeId, ModuleInstanceState>,
    ) -> ModuleInstanceState {
        if store.kind(node) == Kind::ModuleDeclaration {
            if let Some(body) = store.body(node) {
                ancestors.push(node);
                let state = get_cached(store, body, ancestors, visited);
                ancestors.pop();
                state
            } else {
                ModuleInstanceState::Instantiated
            }
        } else {
            get_cached(store, node, ancestors, visited)
        }
    }

    fn get_cached(
        store: &AstStore,
        node: Node,
        ancestors: &mut Vec<Node>,
        visited: &mut HashMap<NodeId, ModuleInstanceState>,
    ) -> ModuleInstanceState {
        let node_id = get_node_id(store, node);
        if let Some(cached) = visited.get(&node_id).copied() {
            return if cached == ModuleInstanceState::Unknown {
                ModuleInstanceState::NonInstantiated
            } else {
                cached
            };
        }
        visited.insert(node_id, ModuleInstanceState::Unknown);
        let state = get_worker(store, node, ancestors, visited);
        visited.insert(node_id, state);
        state
    }

    fn get_worker(
        store: &AstStore,
        node: Node,
        ancestors: &mut Vec<Node>,
        visited: &mut HashMap<NodeId, ModuleInstanceState>,
    ) -> ModuleInstanceState {
        match store.kind(node) {
            Kind::InterfaceDeclaration
            | Kind::TypeAliasDeclaration
            | Kind::JSTypeAliasDeclaration => ModuleInstanceState::NonInstantiated,
            Kind::EnumDeclaration if is_enum_const(store, node) => {
                ModuleInstanceState::ConstEnumOnly
            }
            Kind::ImportDeclaration | Kind::JSImportDeclaration | Kind::ImportEqualsDeclaration
                if !has_syntactic_modifier(store, node, ModifierFlags::EXPORT) =>
            {
                ModuleInstanceState::NonInstantiated
            }
            Kind::ExportDeclaration => {
                if store.module_specifier(node).is_none()
                    && let Some(export_clause) = store.export_clause(node)
                    && store.kind(export_clause) == Kind::NamedExports
                {
                    let mut state = ModuleInstanceState::NonInstantiated;
                    ancestors.push(node);
                    ancestors.push(export_clause);
                    if let Some(elements) = store.elements(export_clause) {
                        for specifier in elements {
                            state = state
                                .max(get_alias_target_state(store, specifier, ancestors, visited));
                            if state == ModuleInstanceState::Instantiated {
                                break;
                            }
                        }
                    }
                    ancestors.pop();
                    ancestors.pop();
                    state
                } else {
                    ModuleInstanceState::Instantiated
                }
            }
            Kind::ModuleBlock => {
                let mut state = ModuleInstanceState::NonInstantiated;
                ancestors.push(node);
                let _ = store.for_each_present_child(node, |child| {
                    match get_cached(store, child, ancestors, visited) {
                        ModuleInstanceState::NonInstantiated => ControlFlow::Continue(()),
                        ModuleInstanceState::ConstEnumOnly => {
                            state = ModuleInstanceState::ConstEnumOnly;
                            ControlFlow::Continue(())
                        }
                        ModuleInstanceState::Instantiated => {
                            state = ModuleInstanceState::Instantiated;
                            ControlFlow::Break(())
                        }
                        ModuleInstanceState::Unknown => ControlFlow::Continue(()),
                    }
                });
                ancestors.pop();
                state
            }
            Kind::ModuleDeclaration => get(store, node, ancestors, visited),
            _ => ModuleInstanceState::Instantiated,
        }
    }

    fn get_alias_target_state(
        store: &AstStore,
        node: Node,
        ancestors: &mut Vec<Node>,
        visited: &mut HashMap<NodeId, ModuleInstanceState>,
    ) -> ModuleInstanceState {
        let Some(name) = store.property_name(node).or_else(|| store.name(node)) else {
            return ModuleInstanceState::Instantiated;
        };
        if store.kind(name) != Kind::Identifier {
            return ModuleInstanceState::Instantiated;
        }
        let mut ancestors = ancestors.clone();
        let mut current = node;
        while let Some(ancestor) = pop_ancestor(store, &mut ancestors, current) {
            if !(is_block(store, ancestor)
                || is_module_block(store, ancestor)
                || is_source_file(store, ancestor))
            {
                current = ancestor;
                continue;
            }
            let mut found = ModuleInstanceState::Unknown;
            let statements = if store.kind(ancestor) == Kind::SourceFile {
                Some(SourceNodeList::new(
                    store,
                    store.as_source_file(ancestor).statements,
                ))
            } else {
                store.statements(ancestor)
            };
            if let Some(statements) = statements {
                let mut statements_ancestors = ancestors.clone();
                statements_ancestors.push(ancestor);
                for statement in statements {
                    if node_has_name(store, statement, name) {
                        let state =
                            get_cached(store, statement, &mut statements_ancestors, visited);
                        if found == ModuleInstanceState::Unknown || state > found {
                            found = state;
                        }
                        if found == ModuleInstanceState::Instantiated {
                            break;
                        }
                        if store.kind(statement) == Kind::ImportEqualsDeclaration {
                            found = ModuleInstanceState::Instantiated;
                        }
                    }
                }
            }
            if found != ModuleInstanceState::Unknown {
                return found;
            }
            current = ancestor;
        }
        ModuleInstanceState::Instantiated
    }

    let mut ancestors = Vec::new();
    let mut visited = HashMap::new();
    get(store, node, &mut ancestors, &mut visited)
}

pub fn is_instantiated_module(store: &AstStore, node: Node, preserve_const_enums: bool) -> bool {
    let module_state = get_module_instance_state(store, node);
    module_state == ModuleInstanceState::Instantiated
        || preserve_const_enums && module_state == ModuleInstanceState::ConstEnumOnly
}

pub fn node_has_name(store: &AstStore, statement: Node, id: Node) -> bool {
    if store
        .name(statement)
        .is_some_and(|name| is_identifier(store, name) && store.text(name) == store.text(id))
    {
        return true;
    }
    if is_variable_statement(store, statement)
        && let Some(declaration_list) = store.declaration_list(statement)
    {
        let declarations = store.node_list(
            store
                .as_variable_declaration_list(declaration_list)
                .declarations,
        );
        return declarations
            .iter()
            .any(|declaration| node_has_name(store, declaration, id));
    }
    false
}

pub fn is_computed_non_literal_name(store: &AstStore, name: Node) -> bool {
    is_computed_property_name(store, name)
        && store
            .expression(name)
            .is_none_or(|expression| !is_string_or_numeric_literal_like(store, expression))
}

pub fn try_get_text_of_property_name(store: &AstStore, name: Node) -> (String, bool) {
    match store.kind(name) {
        Kind::Identifier
        | Kind::PrivateIdentifier
        | Kind::StringLiteral
        | Kind::NumericLiteral
        | Kind::BigIntLiteral
        | Kind::NoSubstitutionTemplateLiteral => (store.text(name), true),
        Kind::ComputedPropertyName => {
            if let Some(expression) = store.expression(name)
                && is_string_or_numeric_literal_like(store, expression)
            {
                return (store.text(expression), true);
            }
            (String::new(), false)
        }
        Kind::JsxNamespacedName => {
            let namespace = store.namespace(name).map(|node| store.text(node));
            let name_text = store.name(name).map(|node| store.text(node));
            match (namespace, name_text) {
                (Some(namespace), Some(name)) => (format!("{namespace}:{name}"), true),
                _ => (String::new(), false),
            }
        }
        _ => (String::new(), false),
    }
}

pub fn is_expression(store: &AstStore, node: Node) -> bool {
    matches!(
        store.kind(node),
        Kind::Identifier
            | Kind::PrivateIdentifier
            | Kind::NumericLiteral
            | Kind::BigIntLiteral
            | Kind::StringLiteral
            | Kind::RegularExpressionLiteral
            | Kind::NoSubstitutionTemplateLiteral
            | Kind::ThisKeyword
            | Kind::SuperKeyword
            | Kind::NullKeyword
            | Kind::TrueKeyword
            | Kind::FalseKeyword
            | Kind::ArrayLiteralExpression
            | Kind::ObjectLiteralExpression
            | Kind::PropertyAccessExpression
            | Kind::ElementAccessExpression
            | Kind::CallExpression
            | Kind::NewExpression
            | Kind::TaggedTemplateExpression
            | Kind::TypeAssertionExpression
            | Kind::ParenthesizedExpression
            | Kind::FunctionExpression
            | Kind::ArrowFunction
            | Kind::DeleteExpression
            | Kind::TypeOfExpression
            | Kind::VoidExpression
            | Kind::AwaitExpression
            | Kind::PrefixUnaryExpression
            | Kind::PostfixUnaryExpression
            | Kind::BinaryExpression
            | Kind::ConditionalExpression
            | Kind::TemplateExpression
            | Kind::YieldExpression
            | Kind::SpreadElement
            | Kind::ClassExpression
            | Kind::OmittedExpression
            | Kind::ExpressionWithTypeArguments
            | Kind::AsExpression
            | Kind::NonNullExpression
            | Kind::MetaProperty
            | Kind::SyntheticExpression
            | Kind::SatisfiesExpression
    )
}

pub fn can_have_modifiers(store: &AstStore, node: Node) -> bool {
    matches!(
        store.kind(node),
        Kind::TypeParameter
            | Kind::Parameter
            | Kind::PropertySignature
            | Kind::PropertyDeclaration
            | Kind::MethodSignature
            | Kind::MethodDeclaration
            | Kind::Constructor
            | Kind::GetAccessor
            | Kind::SetAccessor
            | Kind::IndexSignature
            | Kind::ConstructorType
            | Kind::FunctionExpression
            | Kind::ArrowFunction
            | Kind::ClassExpression
            | Kind::VariableStatement
            | Kind::FunctionDeclaration
            | Kind::ClassDeclaration
            | Kind::InterfaceDeclaration
            | Kind::TypeAliasDeclaration
            | Kind::EnumDeclaration
            | Kind::ModuleDeclaration
            | Kind::ImportEqualsDeclaration
            | Kind::ImportDeclaration
            | Kind::JSImportDeclaration
            | Kind::ExportAssignment
            | Kind::ExportDeclaration
    )
}

pub fn has_static_modifier(store: &AstStore, node: Node) -> bool {
    has_syntactic_modifier(store, node, ModifierFlags::STATIC)
}

pub fn is_static(store: &AstStore, node: Node) -> bool {
    is_class_element(store, node) && has_static_modifier(store, node)
        || is_class_static_block_declaration(store, node)
}

pub fn is_class_element(store: &AstStore, node: Node) -> bool {
    matches!(
        store.kind(node),
        Kind::Constructor
            | Kind::PropertyDeclaration
            | Kind::MethodDeclaration
            | Kind::GetAccessor
            | Kind::SetAccessor
            | Kind::IndexSignature
            | Kind::ClassStaticBlockDeclaration
            | Kind::SemicolonClassElement
    )
}

pub fn is_type_element(store: &AstStore, node: impl AsRef<Node>) -> bool {
    matches!(
        store.kind(*node.as_ref()),
        Kind::CallSignature
            | Kind::ConstructSignature
            | Kind::IndexSignature
            | Kind::MethodSignature
            | Kind::PropertySignature
            | Kind::GetAccessor
            | Kind::SetAccessor
            | Kind::NotEmittedTypeElement
    )
}

pub fn is_class_or_type_element(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let node = *node.as_ref();
    is_class_element(store, node) || is_type_element(store, node)
}

pub fn is_object_type_declaration(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let node = *node.as_ref();
    is_class_like(store, node)
        || is_interface_declaration(store, node)
        || is_type_literal_node(store, node)
}

pub fn node_kind_is(store: &AstStore, node: impl AsRef<Node>, kinds: &[Kind]) -> bool {
    let node = *node.as_ref();
    kinds.iter().any(|kind| store.kind(node) == *kind)
}

pub fn is_accessor(store: &AstStore, node: impl AsRef<Node>) -> bool {
    matches!(
        store.kind(*node.as_ref()),
        Kind::GetAccessor | Kind::SetAccessor
    )
}

pub fn is_entity_name(store: &AstStore, node: impl AsRef<Node>) -> bool {
    matches!(
        store.kind(*node.as_ref()),
        Kind::Identifier | Kind::QualifiedName
    )
}

pub fn is_type_or_js_type_alias_declaration(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let node = *node.as_ref();
    is_type_alias_declaration(store, node) || is_js_type_alias_declaration(store, node)
}

pub fn is_type_declaration(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let node = *node.as_ref();
    match store.kind(node) {
        Kind::TypeParameter
        | Kind::ClassDeclaration
        | Kind::InterfaceDeclaration
        | Kind::TypeAliasDeclaration
        | Kind::JSTypeAliasDeclaration
        | Kind::EnumDeclaration => true,
        Kind::ImportClause => store.is_type_only(node).unwrap_or(false),
        Kind::ImportSpecifier | Kind::ExportSpecifier => store
            .parent(node)
            .and_then(|parent| store.parent(parent))
            .is_some_and(|parent| store.is_type_only(parent).unwrap_or(false)),
        _ => false,
    }
}

pub fn has_modifier(store: &AstStore, node: impl AsRef<Node>, flags: ModifierFlags) -> bool {
    get_combined_modifier_flags(store, *node.as_ref()).intersects(flags)
}

pub fn has_accessor_modifier(store: &AstStore, node: impl AsRef<Node>) -> bool {
    has_syntactic_modifier(store, *node.as_ref(), ModifierFlags::ACCESSOR)
}

pub fn has_abstract_modifier(store: &AstStore, node: impl AsRef<Node>) -> bool {
    has_syntactic_modifier(store, *node.as_ref(), ModifierFlags::ABSTRACT)
}

pub fn has_ambient_modifier(store: &AstStore, node: impl AsRef<Node>) -> bool {
    has_syntactic_modifier(store, *node.as_ref(), ModifierFlags::AMBIENT)
}

pub fn get_containing_function(store: &AstStore, node: impl AsRef<Node>) -> Option<Node> {
    let parent = store.parent(*node.as_ref());
    find_ancestor(store, parent, |store, node| {
        is_function_like(store, Some(node))
    })
}

pub fn walk_up_parenthesized_expressions(store: &AstStore, node: Option<Node>) -> Option<Node> {
    let mut current = node;
    while let Some(node) = current {
        if store.kind(node) != Kind::ParenthesizedExpression {
            return Some(node);
        }
        current = store.parent(node);
    }
    None
}

pub fn is_logical_or_coalescing_binary_expression(
    store: &AstStore,
    node: impl AsRef<Node>,
) -> bool {
    let node = *node.as_ref();
    is_binary_expression(store, node)
        && store
            .operator_token(node)
            .is_some_and(|operator| is_logical_or_coalescing_binary_operator(store.kind(operator)))
}

pub fn get_extends_heritage_clause_elements(store: &AstStore, node: impl AsRef<Node>) -> Vec<Node> {
    get_heritage_elements(store, *node.as_ref(), Kind::ExtendsKeyword)
}

pub fn get_implements_heritage_clause_elements(
    store: &AstStore,
    node: impl AsRef<Node>,
) -> Vec<Node> {
    get_heritage_elements(store, *node.as_ref(), Kind::ImplementsKeyword)
}

pub fn is_function_or_module_block(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let node = *node.as_ref();
    is_source_file(store, node)
        || is_module_block(store, node)
        || is_function_block(store, Some(node))
}

pub fn is_for_of_statement(store: &AstStore, node: impl AsRef<Node>) -> bool {
    store.kind(*node.as_ref()) == Kind::ForOfStatement
}

pub fn is_literal_expression(store: &AstStore, node: impl AsRef<Node>) -> bool {
    is_literal_kind(store.kind(*node.as_ref()))
}

pub fn is_assertion_expression(store: &AstStore, node: impl AsRef<Node>) -> bool {
    matches!(
        store.kind(*node.as_ref()),
        Kind::TypeAssertionExpression | Kind::AsExpression
    )
}

pub fn get_combined_modifier_flags(store: &AstStore, node: Node) -> ModifierFlags {
    let mut node = get_root_declaration(store, node);
    let mut flags = modifiers_to_flags(store, &store.modifier_nodes(node));
    if store.kind(node) == Kind::VariableDeclaration
        && let Some(parent) = store.parent(node)
    {
        node = parent;
    }
    if store.kind(node) == Kind::VariableDeclarationList {
        flags |= modifiers_to_flags(store, &store.modifier_nodes(node));
        if let Some(parent) = store.parent(node) {
            node = parent;
        }
    }
    if store.kind(node) == Kind::VariableStatement {
        flags |= modifiers_to_flags(store, &store.modifier_nodes(node));
    }
    flags
}

pub fn get_combined_node_flags(store: &AstStore, node: Node) -> NodeFlags {
    let mut node = get_root_declaration(store, node);
    let mut flags = store.flags(node);
    if store.kind(node) == Kind::VariableDeclaration
        && let Some(parent) = store.parent(node)
    {
        node = parent;
    }
    if store.kind(node) == Kind::VariableDeclarationList {
        flags |= store.flags(node);
        if let Some(parent) = store.parent(node) {
            node = parent;
        }
    }
    if store.kind(node) == Kind::VariableStatement {
        flags |= store.flags(node);
    }
    flags
}

pub fn is_var_await_using(store: &AstStore, node: Node) -> bool {
    get_combined_node_flags(store, node) & NodeFlags::BLOCK_SCOPED == NodeFlags::AWAIT_USING
}

pub fn is_var_using(store: &AstStore, node: Node) -> bool {
    get_combined_node_flags(store, node) & NodeFlags::BLOCK_SCOPED == NodeFlags::USING
}

pub fn is_var_const(store: &AstStore, node: Node) -> bool {
    get_combined_node_flags(store, node) & NodeFlags::BLOCK_SCOPED == NodeFlags::CONST
}

pub fn get_root_declaration(store: &AstStore, mut node: Node) -> Node {
    while store.kind(node) == Kind::BindingElement {
        let Some(parent) = store.parent(node) else {
            return node;
        };
        let Some(grandparent) = store.parent(parent) else {
            return node;
        };
        node = grandparent;
    }
    node
}

pub fn get_containing_class(store: &AstStore, node: Node) -> Option<Node> {
    find_ancestor(store, store.parent(node), |store, ancestor| {
        matches!(
            store.kind(ancestor),
            Kind::ClassDeclaration | Kind::ClassExpression
        )
    })
}

pub fn get_name_of_declaration(store: &AstStore, node: Option<Node>) -> Option<Node> {
    let node = node?;
    get_non_assigned_name_of_declaration(store, node).or_else(|| {
        (is_function_expression(store, node)
            || is_arrow_function(store, node)
            || is_class_expression(store, node))
        .then(|| get_assigned_name(store, node))
        .flatten()
    })
}

pub fn is_private_identifier_class_element_declaration(store: &AstStore, node: Node) -> bool {
    let kind = store.kind(node);
    matches!(
        kind,
        Kind::PropertyDeclaration | Kind::MethodDeclaration | Kind::GetAccessor | Kind::SetAccessor
    ) && store
        .name(node)
        .is_some_and(|name| is_private_identifier(store, name))
}

pub fn is_access_expression(store: &AstStore, node: Node) -> bool {
    matches!(
        store.kind(node),
        Kind::PropertyAccessExpression | Kind::ElementAccessExpression
    )
}

pub fn is_object_literal_or_class_expression_method_or_accessor(
    store: &AstStore,
    node: Node,
) -> bool {
    matches!(
        store.kind(node),
        Kind::MethodDeclaration | Kind::GetAccessor | Kind::SetAccessor
    ) && store.parent(node).is_some_and(|parent| {
        matches!(
            store.kind(parent),
            Kind::ObjectLiteralExpression | Kind::ClassExpression
        )
    })
}

pub fn is_block_or_catch_scoped(store: &AstStore, declaration: Node) -> bool {
    get_combined_node_flags(store, declaration).intersects(NodeFlags::BLOCK_SCOPED)
        || is_catch_clause_variable_declaration_or_binding_element(store, declaration)
}

pub fn is_catch_clause_variable_declaration_or_binding_element(
    store: &AstStore,
    node: Node,
) -> bool {
    let mut node = node;
    if store.kind(node) == Kind::BindingElement {
        node = get_root_declaration(store, node);
    }
    store.kind(node) == Kind::VariableDeclaration
        && store
            .parent(node)
            .is_some_and(|parent| store.kind(parent) == Kind::CatchClause)
}

pub fn is_prologue_directive(store: &AstStore, node: Node) -> bool {
    store.kind(node) == Kind::ExpressionStatement
        && store
            .expression(node)
            .is_some_and(|expression| store.kind(expression) == Kind::StringLiteral)
}

pub fn skip_parentheses(store: &AstStore, node: Node) -> Node {
    skip_outer_expressions(store, node, OuterExpressionKinds::PARENTHESES)
}

pub fn get_source_file_node_of_node(store: &AstStore, node: Option<Node>) -> Option<Node> {
    let mut current = node?;
    loop {
        if store.kind(current) == Kind::SourceFile {
            return Some(current);
        }
        current = store.parent(current)?;
    }
}

pub fn is_in_top_level_context(store: &AstStore, node: Node) -> bool {
    let mut node = node;
    if store.kind(node) == Kind::Identifier
        && let Some(parent) = store.parent(node)
        && matches!(
            store.kind(parent),
            Kind::ClassDeclaration | Kind::FunctionDeclaration
        )
        && store.name(parent).is_some_and(|name| name == node)
    {
        node = parent;
    }
    get_this_container(store, node, true, false)
        .is_some_and(|container| store.kind(container) == Kind::SourceFile)
}

pub fn get_this_container(
    store: &AstStore,
    node: Node,
    include_arrow_functions: bool,
    include_class_computed_property_name: bool,
) -> Option<Node> {
    let mut node = node;
    loop {
        let parent = store.parent(node)?;
        match store.kind(parent) {
            Kind::ComputedPropertyName => {
                if include_class_computed_property_name
                    && store.parent(parent).is_some_and(|declaration| {
                        store
                            .parent(declaration)
                            .is_some_and(|grandparent| is_class_like(store, grandparent))
                    })
                {
                    return Some(parent);
                }
                node = store.parent(store.parent(parent)?)?;
                continue;
            }
            Kind::Decorator => {
                let decorator_parent = store.parent(parent)?;
                if store.kind(decorator_parent) == Kind::Parameter
                    && store
                        .parent(decorator_parent)
                        .is_some_and(|grandparent| is_class_element(store, grandparent))
                {
                    node = store.parent(decorator_parent)?;
                    continue;
                } else if is_class_element(store, decorator_parent) {
                    node = decorator_parent;
                    continue;
                }
            }
            Kind::ArrowFunction => {
                if include_arrow_functions {
                    return Some(parent);
                }
            }
            Kind::FunctionDeclaration
            | Kind::FunctionExpression
            | Kind::ModuleDeclaration
            | Kind::ClassStaticBlockDeclaration
            | Kind::PropertyDeclaration
            | Kind::PropertySignature
            | Kind::MethodDeclaration
            | Kind::MethodSignature
            | Kind::Constructor
            | Kind::GetAccessor
            | Kind::SetAccessor
            | Kind::CallSignature
            | Kind::ConstructSignature
            | Kind::IndexSignature
            | Kind::EnumDeclaration
            | Kind::SourceFile => return Some(parent),
            _ => {}
        }
        node = parent;
    }
}

pub fn get_external_module_name(store: &AstStore, node: Node) -> Option<Node> {
    match store.kind(node) {
        Kind::ImportDeclaration | Kind::JSImportDeclaration | Kind::ExportDeclaration => {
            store.module_specifier(node)
        }
        Kind::ImportEqualsDeclaration => {
            let module_reference = store.module_reference(node)?;
            (store.kind(module_reference) == Kind::ExternalModuleReference)
                .then(|| store.expression(module_reference))
                .flatten()
        }
        Kind::ImportType => get_import_type_node_literal(store, node),
        Kind::CallExpression => store
            .arguments(node)
            .and_then(|arguments| arguments.first()),
        Kind::ModuleDeclaration => {
            let name = store.name(node)?;
            is_string_literal(store, name).then_some(name)
        }
        _ => panic!("Unhandled case in get_external_module_name"),
    }
}

pub fn is_module_with_string_literal_name(store: &AstStore, node: Node) -> bool {
    is_module_declaration(store, node)
        && store
            .name(node)
            .is_some_and(|name| is_string_literal(store, name))
}

pub fn is_global_scope_augmentation(store: &AstStore, node: Node) -> bool {
    is_module_declaration(store, node)
        && store
            .keyword(node)
            .is_some_and(|keyword| keyword == Kind::GlobalKeyword)
}

pub fn is_ambient_module(store: &AstStore, node: Node) -> bool {
    is_module_declaration(store, node)
        && (is_module_with_string_literal_name(store, node)
            || is_global_scope_augmentation(store, node))
}

pub fn is_module_augmentation_external(store: &AstStore, node: Node) -> bool {
    let Some(parent) = store.parent(node) else {
        return false;
    };
    match store.kind(parent) {
        Kind::SourceFile => store
            .as_source_file(parent)
            .external_module_indicator()
            .is_some(),
        Kind::ModuleBlock => {
            let Some(grandparent) = store.parent(parent) else {
                return false;
            };
            is_ambient_module(store, grandparent)
                && store
                    .parent(grandparent)
                    .is_some_and(|source| is_source_file(store, source))
                && !store.parent(grandparent).is_some_and(|source| {
                    store
                        .as_source_file(source)
                        .external_module_indicator()
                        .is_some()
                })
        }
        _ => false,
    }
}

pub fn is_external_module_augmentation(store: &AstStore, node: Node) -> bool {
    is_ambient_module(store, node) && is_module_augmentation_external(store, node)
}

pub fn module_string_literal_name(store: &AstStore, node: Node) -> Option<Node> {
    is_module_with_string_literal_name(store, node)
        .then(|| store.name(node))
        .flatten()
}

pub fn is_any_import_syntax(store: &AstStore, node: Node) -> bool {
    matches!(
        store.kind(node),
        Kind::ImportDeclaration | Kind::ImportEqualsDeclaration
    )
}

pub fn is_import_node(store: &AstStore, node: Node) -> bool {
    is_any_import_syntax(store, node) || store.kind(node) == Kind::JSImportDeclaration
}

pub fn is_any_import_or_re_export(store: &AstStore, node: Node) -> bool {
    is_import_node(store, node) || is_export_declaration(store, node)
}

pub fn is_possible_import_or_export_statement(store: &AstStore, node: Node) -> bool {
    is_import_node(store, node)
        || is_export_declaration(store, node)
        || is_export_assignment(store, node)
        || is_namespace_export_declaration(store, node)
        || is_module_declaration(store, node)
}

pub fn is_external_module_indicator(store: &AstStore, node: Node) -> bool {
    is_any_import_or_re_export(store, node)
        || is_export_assignment(store, node)
        || has_syntactic_modifier(store, node, ModifierFlags::EXPORT)
}

pub fn is_import_export_syntax_kind(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::ImportDeclaration
            | Kind::JSImportDeclaration
            | Kind::ImportEqualsDeclaration
            | Kind::ImportClause
            | Kind::ImportSpecifier
            | Kind::NamespaceImport
            | Kind::ExportDeclaration
            | Kind::ExportSpecifier
            | Kind::NamespaceExport
    )
}

pub fn is_import_export_syntax(store: &AstStore, node: Node) -> bool {
    is_import_export_syntax_kind(store.kind(node)) || is_import_call(store, node)
}

pub fn is_import_declaration_like(store: &AstStore, node: Node) -> bool {
    matches!(
        store.kind(node),
        Kind::ImportDeclaration | Kind::JSImportDeclaration
    )
}

pub fn is_import_or_export_specifier(store: &AstStore, node: Node) -> bool {
    is_import_specifier(store, node) || is_export_specifier(store, node)
}

pub fn is_type_only_import_declaration(store: &AstStore, node: Node) -> bool {
    match store.kind(node) {
        Kind::ImportSpecifier => {
            store.is_type_only(node).unwrap_or(false)
                || store
                    .parent(node)
                    .and_then(|parent| store.parent(parent))
                    .is_some_and(|parent| store.is_type_only(parent).unwrap_or(false))
        }
        Kind::NamespaceImport => store
            .parent(node)
            .is_some_and(|parent| store.is_type_only(parent).unwrap_or(false)),
        Kind::ImportClause | Kind::ImportEqualsDeclaration => {
            store.is_type_only(node).unwrap_or(false)
        }
        _ => false,
    }
}

fn is_type_only_export_declaration(store: &AstStore, node: Node) -> bool {
    match store.kind(node) {
        Kind::ExportSpecifier => {
            store.is_type_only(node).unwrap_or(false)
                || store
                    .parent(node)
                    .and_then(|parent| store.parent(parent))
                    .is_some_and(|parent| store.is_type_only(parent).unwrap_or(false))
        }
        Kind::ExportDeclaration => {
            store.is_type_only(node).unwrap_or(false)
                && store.module_specifier(node).is_some()
                && store.export_clause(node).is_none()
        }
        Kind::NamespaceExport => store
            .parent(node)
            .is_some_and(|parent| store.is_type_only(parent).unwrap_or(false)),
        _ => false,
    }
}

pub fn is_type_only_import_or_export_declaration(store: &AstStore, node: Node) -> bool {
    is_type_only_import_declaration(store, node) || is_type_only_export_declaration(store, node)
}

pub fn is_exclusively_type_only_import_or_export(store: &AstStore, node: Node) -> bool {
    match store.kind(node) {
        Kind::ExportDeclaration => store.is_type_only(node).unwrap_or(false),
        Kind::ImportDeclaration | Kind::JSImportDeclaration => store
            .import_clause(node)
            .is_some_and(|import_clause| store.is_type_only(import_clause).unwrap_or(false)),
        _ => false,
    }
}

pub fn is_part_of_type_only_import_or_export_declaration(store: &AstStore, node: Node) -> bool {
    containing_type_only_import_or_export_declaration(store, node).is_some()
}

pub fn containing_type_only_import_or_export_declaration(
    store: &AstStore,
    node: Node,
) -> Option<Node> {
    find_ancestor(store, Some(node), |store, node| {
        is_type_only_import_or_export_declaration(store, node)
    })
}

pub fn is_emittable_import(store: &AstStore, node: Node) -> bool {
    match store.kind(node) {
        Kind::ImportDeclaration => store
            .import_clause(node)
            .is_some_and(|import_clause| !store.is_type_only(import_clause).unwrap_or(false)),
        Kind::ExportDeclaration | Kind::ImportEqualsDeclaration => {
            !store.is_type_only(node).unwrap_or(false)
        }
        Kind::CallExpression => is_import_call(store, node),
        _ => false,
    }
}

pub fn import_equals_module_reference(store: &AstStore, node: Node) -> Option<Node> {
    is_import_equals_declaration(store, node)
        .then(|| store.module_reference(node))
        .flatten()
}

pub fn is_external_module_import_equals_declaration(store: &AstStore, node: Node) -> bool {
    import_equals_module_reference(store, node)
        .is_some_and(|module_reference| is_external_module_reference(store, module_reference))
}

pub fn is_internal_module_import_equals_declaration(store: &AstStore, node: Node) -> bool {
    is_import_equals_declaration(store, node)
        && import_equals_module_reference(store, node).is_some()
        && !is_external_module_import_equals_declaration(store, node)
}

pub fn get_external_module_import_equals_declaration_expression(
    store: &AstStore,
    node: Node,
) -> Option<Node> {
    if !is_external_module_import_equals_declaration(store, node) {
        return None;
    }
    import_equals_module_reference(store, node)
        .and_then(|module_reference| store.expression(module_reference))
}

pub fn get_namespace_declaration_node(store: &AstStore, node: Node) -> Option<Node> {
    match store.kind(node) {
        Kind::ImportDeclaration | Kind::JSImportDeclaration => store
            .import_clause(node)
            .and_then(|import_clause| store.named_bindings(import_clause))
            .filter(|named_bindings| is_namespace_import(store, *named_bindings)),
        Kind::ImportEqualsDeclaration => Some(node),
        Kind::ExportDeclaration => store
            .export_clause(node)
            .filter(|export_clause| is_namespace_export(store, *export_clause)),
        _ => None,
    }
}

pub fn has_default_import(store: &AstStore, node: Node) -> bool {
    is_import_declaration_like(store, node)
        && store
            .import_clause(node)
            .is_some_and(|import_clause| store.name(import_clause).is_some())
}

pub fn get_import_attributes(store: &AstStore, node: Node) -> Option<Node> {
    match store.kind(node) {
        Kind::ImportDeclaration | Kind::JSImportDeclaration => {
            store.optional_node_from_id(store.as_import_declaration(node).attributes)
        }
        Kind::ExportDeclaration => {
            store.optional_node_from_id(store.as_export_declaration(node).attributes)
        }
        _ => None,
    }
}

pub fn try_get_import_from_module_specifier(
    store: &AstStore,
    node: impl AsRef<Node>,
) -> Option<Node> {
    let node = *node.as_ref();
    let parent = store.parent(node)?;
    match store.kind(parent) {
        Kind::ImportDeclaration | Kind::JSImportDeclaration | Kind::ExportDeclaration => {
            Some(parent)
        }
        Kind::ExternalModuleReference => store.parent(parent),
        Kind::CallExpression => {
            if is_import_call(store, parent) || is_require_call(store, parent, false) {
                Some(parent)
            } else {
                None
            }
        }
        Kind::LiteralType => {
            if !is_string_literal(store, node) {
                return None;
            }
            let grandparent = store.parent(parent)?;
            is_import_type_node(store, grandparent).then_some(grandparent)
        }
        _ => None,
    }
}

pub fn is_right_side_of_property_access(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let node = *node.as_ref();
    store.parent(node).is_some_and(|parent| {
        store.kind(parent) == Kind::PropertyAccessExpression && store.name(parent) == Some(node)
    })
}

pub fn is_argument_expression_of_element_access(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let node = *node.as_ref();
    store.parent(node).is_some_and(|parent| {
        store.kind(parent) == Kind::ElementAccessExpression
            && store.argument_expression(parent) == Some(node)
    })
}

fn climb_past_property_access(store: &AstStore, node: Node) -> Node {
    if is_right_side_of_property_access(store, node) {
        store.parent(node).unwrap_or(node)
    } else {
        node
    }
}

fn climb_past_property_or_element_access(store: &AstStore, node: Node) -> Node {
    if is_right_side_of_property_access(store, node)
        || is_argument_expression_of_element_access(store, node)
    {
        store.parent(node).unwrap_or(node)
    } else {
        node
    }
}

fn select_expression_of_call_or_new_expression_or_decorator(
    store: &AstStore,
    node: Node,
) -> Option<Node> {
    if is_call_expression(store, node)
        || is_new_expression(store, node)
        || is_decorator(store, node)
    {
        store.expression(node)
    } else {
        None
    }
}

fn select_tag_of_tagged_template_expression(store: &AstStore, node: Node) -> Option<Node> {
    is_tagged_template_expression(store, node)
        .then(|| store.tag(node))
        .flatten()
}

fn select_tag_name_of_jsx_opening_like_element(store: &AstStore, node: Node) -> Option<Node> {
    if is_jsx_opening_like_element(store, node) {
        store.tag_name(node)
    } else {
        None
    }
}

fn is_callee_worker(
    store: &AstStore,
    node: impl AsRef<Node>,
    pred: impl Fn(&AstStore, Node) -> bool,
    callee_selector: impl Fn(&AstStore, Node) -> Option<Node>,
    include_element_access: bool,
    skip_past_outer_expressions: bool,
) -> bool {
    let mut target = if include_element_access {
        climb_past_property_or_element_access(store, *node.as_ref())
    } else {
        climb_past_property_access(store, *node.as_ref())
    };
    if skip_past_outer_expressions && is_expression(store, target) {
        target = skip_outer_expressions(store, target, OuterExpressionKinds::ALL);
    }
    let Some(parent) = store.parent(target) else {
        return false;
    };
    pred(store, parent) && callee_selector(store, parent) == Some(target)
}

pub fn is_call_or_new_expression_target(
    store: &AstStore,
    node: impl AsRef<Node>,
    include_element_access: bool,
    skip_past_outer_expressions: bool,
) -> bool {
    is_callee_worker(
        store,
        node,
        is_call_or_new_expression,
        select_expression_of_call_or_new_expression_or_decorator,
        include_element_access,
        skip_past_outer_expressions,
    )
}

pub fn is_tagged_template_tag(
    store: &AstStore,
    node: impl AsRef<Node>,
    include_element_access: bool,
    skip_past_outer_expressions: bool,
) -> bool {
    is_callee_worker(
        store,
        node,
        is_tagged_template_expression,
        select_tag_of_tagged_template_expression,
        include_element_access,
        skip_past_outer_expressions,
    )
}

pub fn is_decorator_target(
    store: &AstStore,
    node: impl AsRef<Node>,
    include_element_access: bool,
    skip_past_outer_expressions: bool,
) -> bool {
    is_callee_worker(
        store,
        node,
        is_decorator,
        select_expression_of_call_or_new_expression_or_decorator,
        include_element_access,
        skip_past_outer_expressions,
    )
}

pub fn is_jsx_opening_like_element_tag_name(
    store: &AstStore,
    node: impl AsRef<Node>,
    include_element_access: bool,
    skip_past_outer_expressions: bool,
) -> bool {
    is_callee_worker(
        store,
        node,
        is_jsx_opening_like_element,
        select_tag_name_of_jsx_opening_like_element,
        include_element_access,
        skip_past_outer_expressions,
    )
}

fn get_import_type_node_literal(store: &AstStore, node: Node) -> Option<Node> {
    let argument = store.as_import_type_node(node).argument;
    let argument = store.node_from_id(argument);
    if is_literal_type_node(store, argument) {
        let literal = store.as_literal_type_node(argument).literal;
        return Some(store.node_from_id(literal));
    }
    None
}

pub fn get_external_module_indicator(
    store: &AstStore,
    root: Node,
    source_flags: NodeFlags,
    script_kind: core::ScriptKind,
    is_declaration_file: bool,
    options: ExternalModuleIndicatorOptions,
) -> Option<Node> {
    if script_kind == core::ScriptKind::JSON {
        return None;
    }

    if let Some(node) = is_file_probably_external_module(store, root, source_flags) {
        return Some(node);
    }

    if is_declaration_file {
        return None;
    }

    if options.jsx
        && let Some(node) = file_module_from_using_jsx_tag(store, root)
    {
        return Some(node);
    }

    if options.force {
        return Some(root);
    }

    None
}

fn is_file_probably_external_module(
    store: &AstStore,
    root: Node,
    source_flags: NodeFlags,
) -> Option<Node> {
    let source_file = store.as_source_file(root);
    for statement in store.node_list(source_file.statements).iter() {
        if is_external_module_indicator(store, statement) {
            return Some(statement);
        }
    }
    import_meta_if_necessary(store, root, source_flags)
}

fn import_meta_if_necessary(store: &AstStore, root: Node, source_flags: NodeFlags) -> Option<Node> {
    if !(store.flags(root) | source_flags).contains(NodeFlags::POSSIBLY_CONTAINS_IMPORT_META) {
        return None;
    }
    find_child_node(store, root, is_import_meta)
}

fn file_module_from_using_jsx_tag(store: &AstStore, root: Node) -> Option<Node> {
    find_child_node(store, root, |store, node| {
        matches!(
            store.kind(node),
            Kind::JsxOpeningElement | Kind::JsxFragment
        )
    })
}

fn find_child_node(
    store: &AstStore,
    root: Node,
    check: impl Fn(&AstStore, Node) -> bool,
) -> Option<Node> {
    let mut found = None;
    fn visit(
        store: &AstStore,
        node: Node,
        check: &impl Fn(&AstStore, Node) -> bool,
        found: &mut Option<Node>,
    ) -> ControlFlow<()> {
        if found.is_some() {
            return ControlFlow::Break(());
        }
        if check(store, node) {
            *found = Some(node);
            return ControlFlow::Break(());
        }
        store.for_each_present_child(node, |child| visit(store, child, check, found))
    }

    let _ = visit(store, root, &check, &mut found);
    found
}

pub fn for_each_dynamic_import_or_require_call(
    store: &AstStore,
    root: Node,
    include_type_space_imports: bool,
    require_string_literal_like_argument: bool,
    mut cb: impl FnMut(Node, Node) -> bool,
) -> bool {
    let file = store.as_source_file(root);
    let is_javascript_file = is_in_js_file(store, root);
    let mut positions = Vec::new();
    for (start, text) in file.text().match_indices("import") {
        positions.push((start + text.len()) as i32);
    }
    for (start, text) in file.text().match_indices("require") {
        positions.push((start + text.len()) as i32);
    }
    positions.sort_unstable();

    for position in positions {
        let node = get_node_at_position(store, root, position);
        if is_in_js_file(store, node)
            && is_javascript_file
            && is_require_call(store, node, require_string_literal_like_argument)
            && let Some(argument) = store
                .arguments(node)
                .and_then(|arguments| arguments.first())
            && cb(node, argument)
        {
            return true;
        }
        if is_import_call(store, node)
            && let Some(argument) = store
                .arguments(node)
                .and_then(|arguments| arguments.first())
            && (!require_string_literal_like_argument || is_string_literal_like(store, argument))
            && cb(node, argument)
        {
            return true;
        }
        if include_type_space_imports
            && is_literal_import_type_node(store, node)
            && let Some(argument) = get_import_type_node_literal(store, node)
            && cb(node, argument)
        {
            return true;
        }
    }
    false
}

pub fn is_literal_import_type_node(store: &AstStore, node: Node) -> bool {
    if store.kind(node) != Kind::ImportType {
        return false;
    }
    get_import_type_node_literal(store, node).is_some()
}

// Does not handle signed numeric names like `a[+0]` - handling those would require handling prefix unary expressions
// throughout late binding handling as well, which is awkward (but ultimately probably doable if there is demand)
pub fn get_element_or_property_access_name(store: &AstStore, node: Node) -> Option<Node> {
    match store.kind(node) {
        Kind::PropertyAccessExpression => {
            store.name(node).filter(|name| is_identifier(store, *name))
        }
        Kind::ElementAccessExpression => store.argument_expression(node).and_then(|argument| {
            let argument = skip_parentheses(store, argument);
            is_string_or_numeric_literal_like(store, argument).then_some(argument)
        }),
        _ => panic!("Unhandled case in GetElementOrPropertyAccessName"),
    }
}

pub fn is_destructuring_assignment(store: &AstStore, node: Node) -> bool {
    if is_assignment_expression(store, node, true) {
        let left = store.left(node);
        return left.is_some_and(|left| {
            matches!(
                store.kind(left),
                Kind::ObjectLiteralExpression | Kind::ArrayLiteralExpression
            )
        });
    }
    false
}

pub fn is_assignment_target(store: &AstStore, node: Node) -> bool {
    get_assignment_target(store, node).is_some()
}

pub fn get_assignment_target(store: &AstStore, node: Node) -> Option<Node> {
    let mut node = node;
    loop {
        let parent = store.parent(node)?;
        match store.kind(parent) {
            Kind::BinaryExpression => {
                if store
                    .operator_token(parent)
                    .is_some_and(|operator| is_assignment_operator(store.kind(operator)))
                    && store.left(parent).is_some_and(|left| left == node)
                {
                    return Some(parent);
                }
                return None;
            }
            Kind::PrefixUnaryExpression | Kind::PostfixUnaryExpression => {
                if matches!(
                    store.operator(parent),
                    Some(Kind::PlusPlusToken | Kind::MinusMinusToken)
                ) {
                    return Some(parent);
                }
                return None;
            }
            Kind::ForInStatement | Kind::ForOfStatement => {
                if store
                    .initializer(parent)
                    .is_some_and(|initializer| initializer == node)
                {
                    return Some(parent);
                }
                return None;
            }
            Kind::ParenthesizedExpression
            | Kind::ArrayLiteralExpression
            | Kind::SpreadElement
            | Kind::NonNullExpression => node = parent,
            Kind::SpreadAssignment => node = store.parent(parent)?,
            Kind::ShorthandPropertyAssignment => {
                if !store.name(parent).is_some_and(|name| name == node) {
                    return None;
                }
                node = store.parent(parent)?;
            }
            Kind::PropertyAssignment => {
                if store.name(parent).is_some_and(|name| name == node) {
                    return None;
                }
                node = store.parent(parent)?;
            }
            _ => return None,
        }
    }
}

pub fn find_constructor_declaration(store: &AstStore, node: Node) -> Option<Node> {
    store.members(node).into_iter().flatten().find(|member| {
        is_constructor_declaration(store, *member) && node_is_present(store, store.body(*member))
    })
}

pub fn get_declaration_container(store: &AstStore, node: Node) -> Option<Node> {
    let root = get_root_declaration(store, node);
    find_ancestor(store, Some(root), |store, node| {
        !matches!(
            store.kind(node),
            Kind::VariableDeclaration
                | Kind::VariableDeclarationList
                | Kind::ImportSpecifier
                | Kind::NamedImports
                | Kind::NamespaceImport
                | Kind::ImportClause
        )
    })
    .and_then(|ancestor| store.parent(ancestor))
}

pub fn is_potentially_executable_node(store: &AstStore, node: Node) -> bool {
    let kind = store.kind(node);
    if Kind::FirstStatement <= kind && kind <= Kind::LastStatement {
        if kind == Kind::VariableStatement {
            let Some(declaration_list) = store.declaration_list(node) else {
                return false;
            };
            if get_combined_node_flags(store, declaration_list).intersects(NodeFlags::BLOCK_SCOPED)
            {
                return true;
            }
            return store
                .declarations(declaration_list)
                .into_iter()
                .flatten()
                .any(|declaration| store.initializer(declaration).is_some());
        }
        return true;
    }
    matches!(
        kind,
        Kind::ClassDeclaration | Kind::EnumDeclaration | Kind::ModuleDeclaration
    )
}

pub fn has_same_property_access_name(store: &AstStore, node1: Node, node2: Node) -> bool {
    if is_identifier(store, node1) && is_identifier(store, node2) {
        return store.text(node1) == store.text(node2);
    }
    if is_property_access_expression(store, node1) && is_property_access_expression(store, node2) {
        return store
            .name(node1)
            .zip(store.name(node2))
            .is_some_and(|(name1, name2)| store.text(name1) == store.text(name2))
            && store
                .expression(node1)
                .zip(store.expression(node2))
                .is_some_and(|(expr1, expr2)| has_same_property_access_name(store, expr1, expr2));
    }
    false
}

pub fn get_right_most_assigned_expression(store: &AstStore, node: Node) -> Node {
    let mut node = node;
    while is_assignment_expression(store, node, true) {
        node = store
            .right(node)
            .expect("assignment expression should have a right operand");
    }
    node
}

pub fn is_assignment_expression(
    store: &AstStore,
    node: Node,
    exclude_compound_assignment: bool,
) -> bool {
    if store.kind(node) != Kind::BinaryExpression {
        return false;
    }
    let binary = store.as_binary_expression(node);
    let operator = store.node_from_id(binary.operator_token);
    (store.kind(operator) == Kind::EqualsToken
        || !exclude_compound_assignment && is_assignment_operator(store.kind(operator)))
        && is_left_hand_side_expression(store, store.node_from_id(binary.left))
}

pub fn is_compound_assignment(kind: Kind) -> bool {
    kind >= Kind::FirstCompoundAssignment && kind <= Kind::LastCompoundAssignment
}

pub fn is_ambient_module_symbol_name(name: &str) -> bool {
    name.starts_with('"') && name.ends_with('"')
}

pub fn is_instance_of_expression(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let node = *node.as_ref();
    is_binary_expression(store, node)
        && store
            .operator_token(node)
            .is_some_and(|operator| store.kind(operator) == Kind::InstanceOfKeyword)
}

pub fn get_enclosing_block_scope_container(
    store: &AstStore,
    node: impl AsRef<Node>,
) -> Option<Node> {
    let parent = store.parent(*node.as_ref());
    find_ancestor(store, parent, |store, current| {
        let parent = store.parent(current);
        is_block_scope(store, current, parent)
    })
}

pub fn is_block_scope(store: &AstStore, node: Node, parent: Option<Node>) -> bool {
    match store.kind(node) {
        Kind::SourceFile
        | Kind::CaseBlock
        | Kind::CatchClause
        | Kind::ModuleDeclaration
        | Kind::ForStatement
        | Kind::ForInStatement
        | Kind::ForOfStatement
        | Kind::Constructor
        | Kind::MethodDeclaration
        | Kind::GetAccessor
        | Kind::SetAccessor
        | Kind::FunctionDeclaration
        | Kind::FunctionExpression
        | Kind::ArrowFunction
        | Kind::PropertyDeclaration
        | Kind::ClassStaticBlockDeclaration => true,
        Kind::Block => !parent.is_some_and(|parent| {
            is_function_like(store, Some(parent))
                || is_class_static_block_declaration(store, parent)
        }),
        _ => false,
    }
}

pub fn get_first_identifier(store: &AstStore, node: impl AsRef<Node>) -> Option<Node> {
    let node = *node.as_ref();
    match store.kind(node) {
        Kind::Identifier => Some(node),
        Kind::QualifiedName => store
            .left(node)
            .and_then(|left| get_first_identifier(store, left)),
        Kind::PropertyAccessExpression => store
            .expression(node)
            .and_then(|expression| get_first_identifier(store, expression)),
        _ => None,
    }
}

pub fn is_call_like_expression(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let node = *node.as_ref();
    match store.kind(node) {
        Kind::JsxOpeningElement
        | Kind::JsxSelfClosingElement
        | Kind::JsxOpeningFragment
        | Kind::CallExpression
        | Kind::NewExpression
        | Kind::TaggedTemplateExpression
        | Kind::Decorator => true,
        Kind::BinaryExpression => store
            .operator_token(node)
            .is_some_and(|operator| store.kind(operator) == Kind::InstanceOfKeyword),
        _ => false,
    }
}

pub fn is_call_or_new_expression(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let node = *node.as_ref();
    is_call_expression(store, node) || is_new_expression(store, node)
}

pub fn is_jsx_opening_like_element(store: &AstStore, node: impl AsRef<Node>) -> bool {
    matches!(
        store.kind(*node.as_ref()),
        Kind::JsxOpeningElement | Kind::JsxSelfClosingElement
    )
}

pub fn is_jsx_call_like(store: &AstStore, node: impl AsRef<Node>) -> bool {
    matches!(
        store.kind(*node.as_ref()),
        Kind::JsxOpeningElement | Kind::JsxSelfClosingElement | Kind::JsxOpeningFragment
    )
}

pub fn is_super_property(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let node = *node.as_ref();
    is_access_expression(store, node)
        && store
            .expression(node)
            .is_some_and(|expression| store.kind(expression) == Kind::SuperKeyword)
}

pub fn is_method_or_accessor(store: &AstStore, node: impl AsRef<Node>) -> bool {
    matches!(
        store.kind(*node.as_ref()),
        Kind::MethodDeclaration | Kind::GetAccessor | Kind::SetAccessor
    )
}

pub fn skip_type_parentheses(store: &AstStore, node: impl AsRef<Node>) -> Node {
    let mut node = *node.as_ref();
    while is_parenthesized_type_node(store, node) {
        let Some(next) = store.type_node(node) else {
            break;
        };
        node = next;
    }
    node
}

pub fn walk_up_parenthesized_types(store: &AstStore, node: Option<Node>) -> Option<Node> {
    let mut current = node;
    while let Some(node) = current {
        if store.kind(node) != Kind::ParenthesizedType {
            return Some(node);
        }
        current = store.parent(node);
    }
    None
}

pub fn walk_up_binding_elements_and_patterns(
    store: &AstStore,
    binding: impl AsRef<Node>,
) -> Option<Node> {
    let mut node = store.parent(*binding.as_ref())?;
    while store
        .parent(node)
        .is_some_and(|parent| is_binding_element(store, parent))
    {
        node = store.parent(store.parent(node)?)?;
    }
    store.parent(node)
}

pub fn is_expression_node(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let node = *node.as_ref();
    match store.kind(node) {
        Kind::SuperKeyword
        | Kind::NullKeyword
        | Kind::TrueKeyword
        | Kind::FalseKeyword
        | Kind::RegularExpressionLiteral
        | Kind::ArrayLiteralExpression
        | Kind::ObjectLiteralExpression
        | Kind::PropertyAccessExpression
        | Kind::ElementAccessExpression
        | Kind::CallExpression
        | Kind::NewExpression
        | Kind::TaggedTemplateExpression
        | Kind::AsExpression
        | Kind::TypeAssertionExpression
        | Kind::SatisfiesExpression
        | Kind::NonNullExpression
        | Kind::ParenthesizedExpression
        | Kind::FunctionExpression
        | Kind::ClassExpression
        | Kind::ArrowFunction
        | Kind::VoidExpression
        | Kind::DeleteExpression
        | Kind::TypeOfExpression
        | Kind::PrefixUnaryExpression
        | Kind::PostfixUnaryExpression
        | Kind::BinaryExpression
        | Kind::ConditionalExpression
        | Kind::SpreadElement
        | Kind::TemplateExpression
        | Kind::OmittedExpression
        | Kind::JsxElement
        | Kind::JsxSelfClosingElement
        | Kind::JsxFragment
        | Kind::YieldExpression
        | Kind::AwaitExpression => true,
        Kind::MetaProperty => !store.parent(node).is_some_and(|parent| {
            is_import_call(store, parent)
                && store
                    .expression(parent)
                    .is_some_and(|expression| expression == node)
        }),
        Kind::ExpressionWithTypeArguments => !store
            .parent(node)
            .is_some_and(|parent| is_heritage_clause(store, parent)),
        Kind::QualifiedName => {
            let mut node = node;
            while store
                .parent(node)
                .is_some_and(|parent| store.kind(parent) == Kind::QualifiedName)
            {
                node = store.parent(node).unwrap();
            }
            store.parent(node).is_some_and(|parent| {
                is_type_query_node(store, parent) || is_jsx_tag_name(store, node)
            })
        }
        Kind::PrivateIdentifier => store.parent(node).is_some_and(|parent| {
            is_binary_expression(store, parent)
                && store.left(parent) == Some(node)
                && store
                    .operator_token(parent)
                    .is_some_and(|operator| store.kind(operator) == Kind::InKeyword)
        }),
        Kind::Identifier => {
            if store.parent(node).is_some_and(|parent| {
                is_type_query_node(store, parent) || is_jsx_tag_name(store, node)
            }) {
                return true;
            }
            is_in_expression_context(store, node)
        }
        Kind::NumericLiteral
        | Kind::BigIntLiteral
        | Kind::StringLiteral
        | Kind::NoSubstitutionTemplateLiteral
        | Kind::ThisKeyword => is_in_expression_context(store, node),
        _ => false,
    }
}

pub fn is_property_access_or_qualified_name(store: &AstStore, node: impl AsRef<Node>) -> bool {
    matches!(
        store.kind(*node.as_ref()),
        Kind::PropertyAccessExpression | Kind::QualifiedName
    )
}

pub fn is_property_name(store: &AstStore, node: impl AsRef<Node>) -> bool {
    matches!(
        store.kind(*node.as_ref()),
        Kind::Identifier
            | Kind::PrivateIdentifier
            | Kind::StringLiteral
            | Kind::NumericLiteral
            | Kind::ComputedPropertyName
    )
}

pub fn get_this_parameter(store: &AstStore, signature: impl AsRef<Node>) -> Option<Node> {
    store
        .parameters(*signature.as_ref())
        .and_then(|parameters| parameters.first())
        .filter(|parameter| is_this_parameter(store, *parameter))
}

pub fn index_of_node(store: &AstStore, nodes: &[Node], node: impl AsRef<Node>) -> isize {
    let node = *node.as_ref();
    nodes
        .binary_search_by_key(&store.loc(node).pos(), |candidate| {
            store.loc(*candidate).pos()
        })
        .map(|index| index as isize)
        .unwrap_or(-1)
}

pub fn is_infinity_or_nan_string(name: &str) -> bool {
    matches!(name, "Infinity" | "-Infinity" | "NaN")
}

pub fn is_plain_js_file(file: &SourceFile, check_js: core::Tristate) -> bool {
    matches!(
        file.data().script_kind,
        core::ScriptKind::JS | core::ScriptKind::JSX
    ) && file.data().check_js_directive.is_none()
        && check_js == core::Tristate::Unknown
}

pub fn is_source_file_js(file: &SourceFile) -> bool {
    matches!(
        file.data().script_kind,
        core::ScriptKind::JS | core::ScriptKind::JSX
    )
}

pub fn is_check_jsenabled_for_file(
    source_file: &SourceFile,
    compiler_options: &core::CompilerOptions,
) -> bool {
    source_file.data().check_js_directive.as_ref().map_or(
        compiler_options.check_js == core::Tristate::True,
        |directive| directive.enabled,
    )
}

pub fn is_plain_jsfile(file: Option<&SourceFile>, check_js: core::Tristate) -> bool {
    file.is_some_and(|file| {
        matches!(
            file.data().script_kind,
            core::ScriptKind::JS | core::ScriptKind::JSX
        ) && file.data().check_js_directive.is_none()
            && check_js == core::Tristate::Unknown
    })
}

pub fn should_transform_import_call(
    _file_name: &str,
    options: &core::CompilerOptions,
    implied_node_format_for_emit: core::ModuleKind,
) -> bool {
    let module_kind = options.get_emit_module_kind();
    if (core::ModuleKind::Node16 <= module_kind && module_kind <= core::ModuleKind::NodeNext)
        || module_kind == core::ModuleKind::Preserve
    {
        return false;
    }
    implied_node_format_for_emit < core::ModuleKind::ES2015
}

pub fn can_have_symbol(store: &AstStore, node: impl AsRef<Node>) -> bool {
    matches!(
        store.kind(*node.as_ref()),
        Kind::ArrowFunction
            | Kind::BinaryExpression
            | Kind::BindingElement
            | Kind::CallExpression
            | Kind::CallSignature
            | Kind::ClassDeclaration
            | Kind::ClassExpression
            | Kind::ClassStaticBlockDeclaration
            | Kind::Constructor
            | Kind::ConstructorType
            | Kind::ConstructSignature
            | Kind::ElementAccessExpression
            | Kind::EnumDeclaration
            | Kind::EnumMember
            | Kind::ExportAssignment
            | Kind::ExportDeclaration
            | Kind::ExportSpecifier
            | Kind::FunctionDeclaration
            | Kind::FunctionExpression
            | Kind::FunctionType
            | Kind::GetAccessor
            | Kind::ImportClause
            | Kind::ImportEqualsDeclaration
            | Kind::ImportSpecifier
            | Kind::IndexSignature
            | Kind::InterfaceDeclaration
            | Kind::JSTypeAliasDeclaration
            | Kind::JsxAttribute
            | Kind::JsxAttributes
            | Kind::JsxSpreadAttribute
            | Kind::MappedType
            | Kind::MethodDeclaration
            | Kind::MethodSignature
            | Kind::ModuleDeclaration
            | Kind::NamedTupleMember
            | Kind::NamespaceExport
            | Kind::NamespaceExportDeclaration
            | Kind::NamespaceImport
            | Kind::NewExpression
            | Kind::NoSubstitutionTemplateLiteral
            | Kind::NumericLiteral
            | Kind::ObjectLiteralExpression
            | Kind::Parameter
            | Kind::PropertyAccessExpression
            | Kind::PropertyAssignment
            | Kind::PropertyDeclaration
            | Kind::PropertySignature
            | Kind::SetAccessor
            | Kind::ShorthandPropertyAssignment
            | Kind::SourceFile
            | Kind::SpreadAssignment
            | Kind::StringLiteral
            | Kind::TypeAliasDeclaration
            | Kind::TypeLiteral
            | Kind::TypeParameter
            | Kind::VariableDeclaration
    )
}

pub fn is_declaration_name(store: &AstStore, name: impl AsRef<Node>) -> bool {
    let name = *name.as_ref();
    !is_source_file(store, name)
        && !is_binding_pattern(store, name)
        && store.parent(name).is_some_and(|parent| {
            is_declaration(store, parent)
                && store
                    .name(parent)
                    .is_some_and(|parent_name| parent_name == name)
        })
}

pub fn is_declaration_name_or_import_property_name(
    store: &AstStore,
    name: impl AsRef<Node>,
) -> bool {
    let name = *name.as_ref();
    match store.parent(name).map(|parent| store.kind(parent)) {
        Some(Kind::ImportSpecifier | Kind::ExportSpecifier) => {
            is_identifier(store, name) || is_string_literal(store, name)
        }
        _ => is_declaration_name(store, name),
    }
}

pub fn is_literal_computed_property_declaration_name(
    store: &AstStore,
    node: impl AsRef<Node>,
) -> bool {
    let node = *node.as_ref();
    is_string_or_numeric_literal_like(store, node)
        && store.parent(node).is_some_and(|parent| {
            is_computed_property_name(store, parent)
                && store
                    .parent(parent)
                    .is_some_and(|declaration| is_declaration(store, declaration))
        })
}

pub fn is_type_reference_type(store: &AstStore, node: impl AsRef<Node>) -> bool {
    matches!(
        store.kind(*node.as_ref()),
        Kind::TypeReference | Kind::ExpressionWithTypeArguments
    )
}

pub fn is_variable_like(store: &AstStore, node: impl AsRef<Node>) -> bool {
    matches!(
        store.kind(*node.as_ref()),
        Kind::BindingElement
            | Kind::EnumMember
            | Kind::Parameter
            | Kind::PropertyAssignment
            | Kind::PropertyDeclaration
            | Kind::PropertySignature
            | Kind::ShorthandPropertyAssignment
            | Kind::VariableDeclaration
    )
}

pub fn is_let(store: &AstStore, node: impl AsRef<Node>) -> bool {
    get_combined_node_flags(store, *node.as_ref()) & NodeFlags::BLOCK_SCOPED == NodeFlags::LET
}

pub fn has_initializer(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let node = *node.as_ref();
    matches!(
        store.kind(node),
        Kind::VariableDeclaration
            | Kind::Parameter
            | Kind::BindingElement
            | Kind::PropertyDeclaration
            | Kind::PropertyAssignment
            | Kind::EnumMember
            | Kind::ForStatement
            | Kind::ForInStatement
            | Kind::ForOfStatement
            | Kind::JsxAttribute
    ) && store.initializer(node).is_some()
}

pub fn get_type_annotation_node(store: &AstStore, node: impl AsRef<Node>) -> Option<Node> {
    let node = *node.as_ref();
    if matches!(
        store.kind(node),
        Kind::VariableDeclaration
            | Kind::Parameter
            | Kind::PropertySignature
            | Kind::PropertyDeclaration
            | Kind::TypePredicate
            | Kind::ParenthesizedType
            | Kind::TypeOperator
            | Kind::MappedType
            | Kind::TypeAssertionExpression
            | Kind::AsExpression
            | Kind::SatisfiesExpression
            | Kind::TypeAliasDeclaration
            | Kind::JSTypeAliasDeclaration
            | Kind::NamedTupleMember
            | Kind::OptionalType
            | Kind::RestType
            | Kind::TemplateLiteralTypeSpan
    ) {
        return store.r#type(node);
    }
    store
        .function_like_data(node)
        .and_then(|data| data.r#type.get())
}

pub fn get_invoked_expression(store: &AstStore, node: impl AsRef<Node>) -> Option<Node> {
    let node = *node.as_ref();
    match store.kind(node) {
        Kind::TaggedTemplateExpression => store.tag(node),
        Kind::JsxOpeningElement | Kind::JsxSelfClosingElement => store.tag_name(node),
        Kind::BinaryExpression => store.right(node),
        Kind::JsxOpeningFragment => Some(node),
        _ => store.expression(node),
    }
}

pub fn is_right_side_of_qualified_name_or_property_access(
    store: &AstStore,
    node: impl AsRef<Node>,
) -> bool {
    let node = *node.as_ref();
    let Some(parent) = store.parent(node) else {
        return false;
    };
    match store.kind(parent) {
        Kind::QualifiedName => store.right(parent).is_some_and(|right| right == node),
        Kind::PropertyAccessExpression => store.name(parent).is_some_and(|name| name == node),
        Kind::MetaProperty => store.name(parent).is_some_and(|name| name == node),
        _ => false,
    }
}

pub fn is_class_or_interface_like(store: &AstStore, node: impl AsRef<Node>) -> bool {
    matches!(
        store.kind(*node.as_ref()),
        Kind::ClassDeclaration | Kind::ClassExpression | Kind::InterfaceDeclaration
    )
}

pub fn is_any_export_assignment(store: &AstStore, node: impl AsRef<Node>) -> bool {
    is_export_assignment(store, *node.as_ref())
}

pub fn node_can_be_decorated(
    store: &AstStore,
    use_legacy_decorators: bool,
    node: impl AsRef<Node>,
    parent: Option<Node>,
    grandparent: Option<Node>,
) -> bool {
    let node = *node.as_ref();
    if use_legacy_decorators
        && store
            .name(node)
            .is_some_and(|name| is_private_identifier(store, name))
    {
        return false;
    }
    match store.kind(node) {
        Kind::ClassDeclaration => true,
        Kind::ClassExpression => !use_legacy_decorators,
        Kind::PropertyDeclaration => parent.is_some_and(|parent| {
            (use_legacy_decorators && is_class_declaration(store, parent))
                || (!use_legacy_decorators
                    && is_class_like(store, parent)
                    && !has_abstract_modifier(store, node)
                    && !has_ambient_modifier(store, node))
        }),
        Kind::GetAccessor | Kind::SetAccessor | Kind::MethodDeclaration => {
            store.body(node).is_some()
                && parent.is_some_and(|parent| {
                    (use_legacy_decorators && is_class_declaration(store, parent))
                        || (!use_legacy_decorators && is_class_like(store, parent))
                })
        }
        Kind::Parameter => {
            use_legacy_decorators
                && parent.is_some_and(|parent| {
                    store.body(parent).is_some()
                        && matches!(
                            store.kind(parent),
                            Kind::Constructor | Kind::MethodDeclaration | Kind::SetAccessor
                        )
                        && get_this_parameter(store, parent)
                            .is_none_or(|this_parameter| this_parameter != node)
                })
                && grandparent.is_some_and(|grandparent| is_class_declaration(store, grandparent))
        }
        _ => false,
    }
}

pub fn class_or_constructor_parameter_is_decorated(
    store: &AstStore,
    use_legacy_decorators: bool,
    node: impl AsRef<Node>,
) -> bool {
    let node = *node.as_ref();
    if node_is_decorated(store, use_legacy_decorators, node, None, None) {
        return true;
    }
    get_first_constructor_with_body(store, node).is_some_and(|constructor| {
        child_is_decorated(store, use_legacy_decorators, constructor, Some(node))
    })
}

pub fn class_element_or_class_element_parameter_is_decorated(
    store: &AstStore,
    use_legacy_decorators: bool,
    node: impl AsRef<Node>,
    parent: impl AsRef<Node>,
) -> bool {
    let node = *node.as_ref();
    let parent = *parent.as_ref();
    if node_is_decorated(store, use_legacy_decorators, node, Some(parent), None) {
        return true;
    }
    if (is_method_declaration(store, node)
        || is_constructor_declaration(store, node)
        || is_set_accessor_declaration(store, node))
        && store.parameters(node).is_some_and(|parameters| {
            parameters.iter().any(|parameter| {
                !is_this_parameter(store, parameter)
                    && node_is_decorated(
                        store,
                        use_legacy_decorators,
                        parameter,
                        Some(node),
                        Some(parent),
                    )
            })
        })
    {
        return true;
    }
    false
}

pub fn node_is_decorated(
    store: &AstStore,
    use_legacy_decorators: bool,
    node: Node,
    parent: Option<Node>,
    grandparent: Option<Node>,
) -> bool {
    has_decorators(store, node)
        && node_can_be_decorated(store, use_legacy_decorators, node, parent, grandparent)
}

fn node_or_child_is_decorated(
    store: &AstStore,
    use_legacy_decorators: bool,
    node: Node,
    parent: Option<Node>,
    grandparent: Option<Node>,
) -> bool {
    node_is_decorated(store, use_legacy_decorators, node, parent, grandparent)
        || child_is_decorated(store, use_legacy_decorators, node, parent)
}

fn child_is_decorated(
    store: &AstStore,
    use_legacy_decorators: bool,
    node: Node,
    parent: Option<Node>,
) -> bool {
    match store.kind(node) {
        Kind::ClassDeclaration | Kind::ClassExpression => {
            store.members(node).is_some_and(|members| {
                members.iter().any(|member| {
                    node_or_child_is_decorated(
                        store,
                        use_legacy_decorators,
                        member,
                        Some(node),
                        parent,
                    )
                })
            })
        }
        Kind::MethodDeclaration | Kind::SetAccessor | Kind::Constructor => {
            store.parameters(node).is_some_and(|parameters| {
                parameters.iter().any(|parameter| {
                    node_is_decorated(store, use_legacy_decorators, parameter, Some(node), parent)
                })
            })
        }
        _ => false,
    }
}

pub fn is_valid_type_only_alias_use_site(store: &AstStore, use_site: impl AsRef<Node>) -> bool {
    let use_site = *use_site.as_ref();
    store.flags(use_site).intersects(NodeFlags::AMBIENT)
        || is_part_of_type_query(store, use_site)
        || is_identifier_in_non_emitting_heritage_clause(store, use_site)
        || is_part_of_possibly_valid_type_or_abstract_computed_property_name(store, use_site)
        || !(is_expression_node(store, use_site)
            || is_shorthand_property_name_use_site(store, use_site))
}

fn is_identifier_in_non_emitting_heritage_clause(store: &AstStore, node: Node) -> bool {
    if !is_identifier(store, node) {
        return false;
    }
    let mut parent = store.parent(node);
    while parent.is_some_and(|parent| {
        is_property_access_expression(store, parent)
            || is_expression_with_type_arguments(store, parent)
    }) {
        parent = parent.and_then(|parent| store.parent(parent));
    }
    parent.is_some_and(|parent| {
        is_heritage_clause(store, parent)
            && (store.as_heritage_clause(parent).token == Kind::ImplementsKeyword
                || store
                    .parent(parent)
                    .is_some_and(|parent| is_interface_declaration(store, parent)))
    })
}

fn is_part_of_possibly_valid_type_or_abstract_computed_property_name(
    store: &AstStore,
    node: Node,
) -> bool {
    let mut node = node;
    while node_kind_is(
        store,
        node,
        &[Kind::Identifier, Kind::PropertyAccessExpression],
    ) {
        let Some(parent) = store.parent(node) else {
            return false;
        };
        node = parent;
    }
    if store.kind(node) != Kind::ComputedPropertyName {
        return false;
    }
    if store
        .parent(node)
        .is_some_and(|parent| has_syntactic_modifier(store, parent, ModifierFlags::ABSTRACT))
    {
        return true;
    }
    store
        .parent(node)
        .and_then(|parent| store.parent(parent))
        .is_some_and(|parent| {
            matches!(
                store.kind(parent),
                Kind::InterfaceDeclaration | Kind::TypeLiteral
            )
        })
}

fn is_shorthand_property_name_use_site(store: &AstStore, use_site: Node) -> bool {
    is_identifier(store, use_site)
        && store.parent(use_site).is_some_and(|parent| {
            is_shorthand_property_assignment(store, parent)
                && store.name(parent).is_some_and(|name| name == use_site)
        })
}

pub fn get_text_of_property_name(store: &AstStore, name: impl AsRef<Node>) -> String {
    let (text, _) = try_get_text_of_property_name(store, *name.as_ref());
    text
}

pub fn get_property_name_for_property_name_node(
    store: &AstStore,
    name: impl AsRef<Node>,
) -> String {
    get_text_of_property_name(store, name)
}

pub fn get_new_target_container(store: &AstStore, node: impl AsRef<Node>) -> Option<Node> {
    get_this_container(store, *node.as_ref(), false, false).filter(|container| {
        matches!(
            store.kind(*container),
            Kind::Constructor | Kind::FunctionDeclaration | Kind::FunctionExpression
        )
    })
}

pub fn is_this_in_type_query(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let mut node = *node.as_ref();
    if store.kind(node) != Kind::ThisKeyword && !is_this_identifier(store, node) {
        return false;
    }
    while store.parent(node).is_some_and(|parent| {
        is_qualified_name(store, parent) && store.left(parent).is_some_and(|left| left == node)
    }) {
        node = store.parent(node).unwrap();
    }
    store
        .parent(node)
        .is_some_and(|parent| store.kind(parent) == Kind::TypeQuery)
}

pub fn is_initialized_property(store: &AstStore, member: impl AsRef<Node>) -> bool {
    let member = *member.as_ref();
    is_property_declaration(store, member) && store.initializer(member).is_some()
}

pub fn has_context_sensitive_parameters(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let node = *node.as_ref();
    if store
        .type_parameters(node)
        .is_none_or(|type_parameters| type_parameters.is_empty())
    {
        if store
            .parameters(node)
            .is_some_and(|parameters| parameters.iter().any(|p| store.type_node(p).is_none()))
        {
            return true;
        }
        if !is_arrow_function(store, node) {
            let first_parameter = store
                .parameters(node)
                .and_then(|parameters| parameters.first());
            if first_parameter.is_none_or(|parameter| !is_this_parameter(store, parameter)) {
                return store.flags(node).intersects(NodeFlags::CONTAINS_THIS);
            }
        }
    }
    false
}

pub fn get_first_constructor_with_body(store: &AstStore, node: impl AsRef<Node>) -> Option<Node> {
    store.members(*node.as_ref()).and_then(|members| {
        members.iter().find(|member| {
            is_constructor_declaration(store, *member) && store.body(*member).is_some()
        })
    })
}

pub fn is_jsx_tag_name(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let node = *node.as_ref();
    let Some(parent) = store.parent(node) else {
        return false;
    };
    matches!(
        store.kind(parent),
        Kind::JsxOpeningElement | Kind::JsxClosingElement | Kind::JsxSelfClosingElement
    ) && store
        .tag_name(parent)
        .is_some_and(|tag_name| tag_name == node)
}

pub fn is_check_js_enabled_for_file(file: &SourceFile, check_js: core::Tristate) -> bool {
    check_js == core::Tristate::True
        || file
            .data()
            .check_js_directive
            .as_ref()
            .is_some_and(|directive| directive.enabled)
}

pub fn is_part_of_type_node(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let node = *node.as_ref();
    let kind = store.kind(node);
    if kind >= Kind::FirstTypeNode && kind <= Kind::LastTypeNode {
        return true;
    }
    match kind {
        Kind::AnyKeyword
        | Kind::UnknownKeyword
        | Kind::NumberKeyword
        | Kind::BigIntKeyword
        | Kind::StringKeyword
        | Kind::BooleanKeyword
        | Kind::SymbolKeyword
        | Kind::ObjectKeyword
        | Kind::UndefinedKeyword
        | Kind::NullKeyword
        | Kind::NeverKeyword => true,
        Kind::VoidKeyword => store
            .parent(node)
            .is_none_or(|parent| store.kind(parent) != Kind::VoidExpression),
        Kind::ExpressionWithTypeArguments => {
            is_part_of_type_expression_with_type_arguments(store, node)
        }
        Kind::TypeParameter => store
            .parent(node)
            .is_some_and(|parent| matches!(store.kind(parent), Kind::MappedType | Kind::InferType)),
        Kind::Identifier => {
            let parent = store.parent(node);
            if parent.is_some_and(|parent| {
                is_qualified_name(store, parent)
                    && store.right(parent).is_some_and(|right| right == node)
            }) {
                return is_part_of_type_node_in_parent(store, parent.unwrap());
            }
            if parent.is_some_and(|parent| {
                is_property_access_expression(store, parent)
                    && store.name(parent).is_some_and(|name| name == node)
            }) {
                return is_part_of_type_node_in_parent(store, parent.unwrap());
            }
            is_part_of_type_node_in_parent(store, node)
        }
        Kind::QualifiedName | Kind::PropertyAccessExpression | Kind::ThisKeyword => {
            is_part_of_type_node_in_parent(store, node)
        }
        _ => false,
    }
}

fn is_part_of_type_node_in_parent(store: &AstStore, node: Node) -> bool {
    let Some(parent) = store.parent(node) else {
        return false;
    };
    if store.kind(parent) == Kind::TypeQuery {
        return false;
    }
    if store.kind(parent) == Kind::ImportType {
        return !store.as_import_type_node(parent).is_type_of;
    }
    let parent_kind = store.kind(parent);
    if parent_kind >= Kind::FirstTypeNode && parent_kind <= Kind::LastTypeNode {
        return true;
    }
    match parent_kind {
        Kind::ExpressionWithTypeArguments => {
            is_part_of_type_expression_with_type_arguments(store, parent)
        }
        Kind::TypeParameter => store
            .constraint(parent)
            .is_some_and(|constraint| constraint == node),
        Kind::VariableDeclaration
        | Kind::Parameter
        | Kind::PropertyDeclaration
        | Kind::PropertySignature
        | Kind::FunctionDeclaration
        | Kind::FunctionExpression
        | Kind::ArrowFunction
        | Kind::Constructor
        | Kind::MethodDeclaration
        | Kind::MethodSignature
        | Kind::GetAccessor
        | Kind::SetAccessor
        | Kind::CallSignature
        | Kind::ConstructSignature
        | Kind::IndexSignature
        | Kind::TypeAssertionExpression => store
            .type_node(parent)
            .is_some_and(|type_node| type_node == node),
        Kind::CallExpression | Kind::NewExpression | Kind::TaggedTemplateExpression => {
            store.type_arguments(parent).is_some_and(|type_arguments| {
                type_arguments
                    .iter()
                    .any(|type_argument| type_argument == node)
            })
        }
        _ => false,
    }
}

fn is_part_of_type_expression_with_type_arguments(store: &AstStore, node: Node) -> bool {
    let Some(parent) = store.parent(node) else {
        return false;
    };
    is_heritage_clause(store, parent)
        && store.parent(parent).is_some_and(|parent_parent| {
            !is_class_like(store, parent_parent)
                || store.as_heritage_clause(parent).token == Kind::ImplementsKeyword
        })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessKind {
    Read,
    Write,
    ReadWrite,
}

pub fn is_write_only_access(store: &AstStore, node: impl AsRef<Node>) -> bool {
    access_kind(store, *node.as_ref()) == AccessKind::Write
}

pub fn is_write_access(store: &AstStore, node: impl AsRef<Node>) -> bool {
    access_kind(store, *node.as_ref()) != AccessKind::Read
}

fn access_kind(store: &AstStore, node: Node) -> AccessKind {
    let Some(parent) = store.parent(node) else {
        return AccessKind::Read;
    };
    match store.kind(parent) {
        Kind::ParenthesizedExpression => access_kind(store, parent),
        Kind::PrefixUnaryExpression => match store.as_prefix_unary_expression(parent).operator {
            Kind::PlusPlusToken | Kind::MinusMinusToken => AccessKind::ReadWrite,
            _ => AccessKind::Read,
        },
        Kind::PostfixUnaryExpression => match store.as_postfix_unary_expression(parent).operator {
            Kind::PlusPlusToken | Kind::MinusMinusToken => AccessKind::ReadWrite,
            _ => AccessKind::Read,
        },
        Kind::BinaryExpression => {
            if store.left(parent).is_some_and(|left| left == node)
                && store.operator_token(parent).is_some_and(|operator| {
                    let kind = store.kind(operator);
                    is_assignment_operator(kind)
                })
            {
                if store
                    .operator_token(parent)
                    .is_some_and(|operator| store.kind(operator) == Kind::EqualsToken)
                {
                    AccessKind::Write
                } else {
                    AccessKind::ReadWrite
                }
            } else {
                AccessKind::Read
            }
        }
        Kind::PropertyAccessExpression => {
            if store.name(parent).is_none_or(|name| name != node) {
                AccessKind::Read
            } else {
                access_kind(store, parent)
            }
        }
        Kind::PropertyAssignment => {
            let parent_access = store
                .parent(parent)
                .map_or(AccessKind::Read, |parent| access_kind(store, parent));
            if store.name(parent).is_some_and(|name| name == node) {
                reverse_access_kind(parent_access)
            } else {
                parent_access
            }
        }
        Kind::ShorthandPropertyAssignment => {
            if store
                .object_assignment_initializer(parent)
                .is_some_and(|initializer| initializer == node)
            {
                AccessKind::Read
            } else {
                store
                    .parent(parent)
                    .map_or(AccessKind::Read, |parent| access_kind(store, parent))
            }
        }
        Kind::ArrayLiteralExpression => access_kind(store, parent),
        Kind::ForInStatement | Kind::ForOfStatement => {
            if store
                .initializer(parent)
                .is_some_and(|initializer| initializer == node)
            {
                AccessKind::Write
            } else {
                AccessKind::Read
            }
        }
        _ => AccessKind::Read,
    }
}

fn reverse_access_kind(access_kind: AccessKind) -> AccessKind {
    match access_kind {
        AccessKind::Read => AccessKind::Write,
        AccessKind::Write => AccessKind::Read,
        AccessKind::ReadWrite => AccessKind::ReadWrite,
    }
}

pub fn tag_names_are_equivalent(store: &AstStore, lhs: Node, rhs: Node) -> bool {
    if store.kind(lhs) != store.kind(rhs) {
        return false;
    }
    match store.kind(lhs) {
        Kind::Identifier => store.text(lhs) == store.text(rhs),
        Kind::ThisKeyword => true,
        Kind::JsxNamespacedName => {
            let lhs_data = store.as_jsx_namespaced_name(lhs);
            let rhs_data = store.as_jsx_namespaced_name(rhs);
            store.text(store.node_from_id(lhs_data.namespace))
                == store.text(store.node_from_id(rhs_data.namespace))
                && store.text(store.node_from_id(lhs_data.name))
                    == store.text(store.node_from_id(rhs_data.name))
        }
        Kind::PropertyAccessExpression => {
            store
                .name(lhs)
                .zip(store.name(rhs))
                .is_some_and(|(lhs_name, rhs_name)| store.text(lhs_name) == store.text(rhs_name))
                && store
                    .expression(lhs)
                    .zip(store.expression(rhs))
                    .is_some_and(|(lhs_expr, rhs_expr)| {
                        tag_names_are_equivalent(store, lhs_expr, rhs_expr)
                    })
        }
        _ => panic!("Unhandled case in TagNamesAreEquivalent"),
    }
}

pub fn is_modifier(store: &AstStore, node: Node) -> bool {
    is_modifier_kind(store.kind(node))
}

pub fn can_have_illegal_decorators(store: &AstStore, node: Node) -> bool {
    matches!(
        store.kind(node),
        Kind::PropertyAssignment
            | Kind::ShorthandPropertyAssignment
            | Kind::FunctionDeclaration
            | Kind::Constructor
            | Kind::IndexSignature
            | Kind::ClassStaticBlockDeclaration
            | Kind::MissingDeclaration
            | Kind::VariableStatement
            | Kind::InterfaceDeclaration
            | Kind::TypeAliasDeclaration
            | Kind::EnumDeclaration
            | Kind::ModuleDeclaration
            | Kind::ImportEqualsDeclaration
            | Kind::ImportDeclaration
            | Kind::JSImportDeclaration
            | Kind::NamespaceExportDeclaration
            | Kind::ExportDeclaration
            | Kind::ExportAssignment
    )
}

pub fn can_have_decorators(store: &AstStore, node: Node) -> bool {
    matches!(
        store.kind(node),
        Kind::Parameter
            | Kind::PropertyDeclaration
            | Kind::MethodDeclaration
            | Kind::GetAccessor
            | Kind::SetAccessor
            | Kind::ClassExpression
            | Kind::ClassDeclaration
    )
}

impl AstStore {
    pub fn type_node(&self, node: Node) -> Option<Node> {
        self.r#type(node)
    }

    pub fn property_name_or_name(&self, node: Node) -> Option<Node> {
        self.property_name(node).or_else(|| self.name(node))
    }

    pub fn modifier_nodes(&self, node: Node) -> Vec<Node> {
        self.modifiers(node)
            .map(|modifiers| modifiers.nodes().iter().collect())
            .unwrap_or_default()
    }

    pub fn jsx_children(&self, node: Node) -> SourceNodeList<'_> {
        match self.kind(node) {
            Kind::JsxElement => SourceNodeList::new(self, self.as_jsx_element(node).children),
            Kind::JsxFragment => SourceNodeList::new(self, self.as_jsx_fragment(node).children),
            _ => panic!("node kind {} has no JSX children", self.kind(node)),
        }
    }

    pub(crate) fn jsx_children_id(&self, node: Node) -> NodeListId {
        match self.kind(node) {
            Kind::JsxElement => self.as_jsx_element(node).children,
            Kind::JsxFragment => self.as_jsx_fragment(node).children,
            _ => panic!("node kind {} has no JSX children", self.kind(node)),
        }
    }

    pub fn jsx_opening_element(&self, node: Node) -> Node {
        self.node_from_id(self.as_jsx_element(node).opening_element)
    }

    pub fn jsx_closing_element(&self, node: Node) -> Node {
        self.node_from_id(self.as_jsx_element(node).closing_element)
    }

    pub fn statement_list(&self, node: Node) -> SourceNodeList<'_> {
        let id = match self.kind(node) {
            Kind::SourceFile => self.as_source_file(node).statements,
            Kind::Block => self.as_block(node).statements,
            Kind::ModuleBlock => self.as_module_block(node).statements,
            Kind::CaseClause | Kind::DefaultClause => {
                self.as_case_or_default_clause(node).statements
            }
            _ => panic!("node kind {} has no statement list", self.kind(node)),
        };
        SourceNodeList::new(self, id)
    }

    pub fn text(&self, node: Node) -> String {
        match self.header(node).payload.tag() {
            NodePayloadTag::Identifier => self.as_identifier(node).text.clone(),
            NodePayloadTag::PrivateIdentifier => self.as_private_identifier(node).text.clone(),
            NodePayloadTag::StringLiteral => self.as_string_literal(node).text.clone(),
            NodePayloadTag::NumericLiteral => self.as_numeric_literal(node).text.clone(),
            NodePayloadTag::BigIntLiteral => self.as_big_int_literal(node).text.clone(),
            NodePayloadTag::TemplateExpression => String::new(),
            NodePayloadTag::TemplateHead => self.as_template_head(node).text.clone(),
            NodePayloadTag::TemplateMiddle => self.as_template_middle(node).text.clone(),
            NodePayloadTag::TemplateTail => self.as_template_tail(node).text.clone(),
            NodePayloadTag::NoSubstitutionTemplateLiteral => {
                self.as_no_substitution_template_literal(node).text.clone()
            }
            NodePayloadTag::MetaProperty => self
                .name(node)
                .map(|name| self.text(name))
                .unwrap_or_default(),
            NodePayloadTag::JsxNamespacedName => {
                let jsx = self.as_jsx_namespaced_name(node);
                format!(
                    "{}:{}",
                    self.text(
                        self.optional_node_from_id(jsx.namespace)
                            .expect("jsx namespaced name should have a namespace"),
                    ),
                    self.text(
                        self.optional_node_from_id(jsx.name)
                            .expect("jsx namespaced name should have a name"),
                    )
                )
            }
            NodePayloadTag::JsxText => self.as_jsx_text(node).text.clone(),
            _ => String::new(),
        }
    }

    pub fn token_flags(&self, node: Node) -> Option<TokenFlags> {
        match self.header(node).payload.tag() {
            NodePayloadTag::StringLiteral => Some(self.as_string_literal(node).token_flags),
            NodePayloadTag::NumericLiteral => Some(self.as_numeric_literal(node).token_flags),
            NodePayloadTag::BigIntLiteral => Some(self.as_big_int_literal(node).token_flags),
            NodePayloadTag::RegularExpressionLiteral => {
                Some(self.as_regular_expression_literal(node).token_flags)
            }
            NodePayloadTag::NoSubstitutionTemplateLiteral => {
                Some(self.as_no_substitution_template_literal(node).token_flags)
            }
            NodePayloadTag::TemplateHead => Some(self.as_template_head(node).token_flags),
            NodePayloadTag::TemplateMiddle => Some(self.as_template_middle(node).token_flags),
            NodePayloadTag::TemplateTail => Some(self.as_template_tail(node).token_flags),
            NodePayloadTag::JsxText => Some(self.as_jsx_text(node).token_flags),
            _ => None,
        }
    }

    pub fn template_flags(&self, node: Node) -> Option<TokenFlags> {
        match self.header(node).payload.tag() {
            NodePayloadTag::NoSubstitutionTemplateLiteral => Some(
                self.as_no_substitution_template_literal(node)
                    .template_flags,
            ),
            NodePayloadTag::TemplateHead => Some(self.as_template_head(node).template_flags),
            NodePayloadTag::TemplateMiddle => Some(self.as_template_middle(node).template_flags),
            NodePayloadTag::TemplateTail => Some(self.as_template_tail(node).template_flags),
            _ => None,
        }
    }

    pub fn end_of_file_token(&self, node: Node) -> Option<Node> {
        (self.kind(node) == Kind::SourceFile)
            .then(|| self.as_source_file(node).end_of_file_token())
            .flatten()
    }

    pub fn text_eq(&self, node: Node, text: &str) -> bool {
        self.text(node) == text
    }
}

impl NodeFactory {
    pub fn new_modifier(&mut self, kind: Kind) -> Node {
        self.new_token(kind)
    }
}
// PatternAmbientModule

pub struct PatternAmbientModule {
    pub pattern: core::Pattern,
    pub symbol: Option<SymbolHandle>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum CommentDirectiveKind {
    Unknown = 0,
    ExpectError,
    Ignore,
}

#[derive(Clone, Debug)]
pub struct CommentDirective {
    pub loc: core::TextRange,
    pub kind: CommentDirectiveKind,
}

// SourceFile

#[derive(Clone, Default)]
pub struct SourceFileMetaData {
    pub package_json_type: String,
    pub package_json_directory: String,
    pub implied_node_format: core::ResolutionMode,
}

pub struct CheckJsDirective {
    pub enabled: bool,
    pub range: CommentRange,
}

pub trait HasFileName {
    fn file_name(&self) -> String;
    fn path(&self) -> tspath::Path;
}

pub struct HasFileNameImpl {
    file_name: String,
    path: tspath::Path,
}

pub fn new_has_file_name(file_name: String, path: tspath::Path) -> HasFileNameImpl {
    HasFileNameImpl { file_name, path }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct TokenCacheKey {
    parent: Option<Node>,
    loc: core::TextRange,
}

type NodeCacheKey = Node;

fn node_cache_key(node: Node) -> NodeCacheKey {
    node
}

pub struct SourceFileData {
    pub(crate) composite_base: CompositeBase,

    // Fields set by NewSourceFile
    pub(crate) file_name: String, // For debugging convenience
    pub(crate) parse_options: SourceFileParseOptions,
    pub(crate) text: Arc<str>,
    pub(crate) statements: NodeListId,
    pub(crate) end_of_file_token: Option<Node>,

    // Fields set by parser
    diagnostics: Vec<Diagnostic>,
    js_diagnostics: Vec<Diagnostic>,
    language_variant: core::LanguageVariant,
    script_kind: core::ScriptKind,
    is_declaration_file: bool,
    contains_non_ascii: bool,
    uses_uri_style_node_core_modules: core::Tristate,
    identifiers: Arc<HashMap<String, String>>,
    identifier_count: i32,
    pub(crate) imports: Vec<Node>,
    pub(crate) module_augmentations: Vec<Node>,
    ambient_module_names: Vec<String>,
    comment_directives: Vec<CommentDirective>,
    pub(crate) reparsed_clones: Vec<Node>,
    pragmas: Vec<Pragma>,
    referenced_files: Vec<FileReference>,
    type_reference_directives: Vec<FileReference>,
    lib_reference_directives: Vec<FileReference>,
    check_js_directive: Option<CheckJsDirective>,
    node_count: i32,
    text_count: i32,
    pub(crate) common_js_module_indicator: Option<Node>,
    // If this is the SourceFile itself, then this module was "forced"
    // to be an external module (previously "true").
    pub(crate) external_module_indicator: Option<Node>,

    // Fields set by ECMALineMap
    ecma_line_map: OnceLock<Arc<[core::TextPos]>>,

    // Fields set by language service
    hash: xxh3::Uint128,
    token_cache_mu: Mutex<()>,
    token_cache: HashMap<TokenCacheKey, Node>,
    declaration_map_mu: Mutex<()>,
    declaration_map: HashMap<String, Vec<Node>>,
    name_table_once: OnceLock<()>,
    name_table: HashMap<String, i32>,

    // Fields for UTF-8 to UTF-16 position mapping
    position_map: OnceLock<PositionMap>,

    // Fields set by binder
    bind_once_state: OnceLock<Arc<dyn Any + Send + Sync>>,
}

pub struct ParsedSourceFileMetadata {
    pub diagnostics: Vec<Diagnostic>,
    pub js_diagnostics: Vec<Diagnostic>,
    pub comment_directives: Vec<CommentDirective>,
    pub pragmas: Vec<Pragma>,
    pub referenced_files: Vec<FileReference>,
    pub type_reference_directives: Vec<FileReference>,
    pub lib_reference_directives: Vec<FileReference>,
    pub check_js_directive: Option<CheckJsDirective>,
    pub common_js_module_indicator: Option<Node>,
    pub is_declaration_file: bool,
    pub contains_non_ascii: bool,
    pub language_variant: core::LanguageVariant,
    pub script_kind: core::ScriptKind,
    pub source_flags: NodeFlags,
    pub identifiers: Arc<HashMap<String, String>>,
    pub node_count: i32,
    pub text_count: i32,
    pub identifier_count: i32,
    pub reparsed_clones: Vec<Node>,
    pub imports: Vec<Node>,
    pub module_augmentations: Vec<Node>,
    pub ambient_module_names: Vec<String>,
    pub uses_uri_style_node_core_modules: core::Tristate,
    pub external_module_indicator: Option<Node>,
    pub hash: xxh3::Uint128,
}

impl Default for ParsedSourceFileMetadata {
    fn default() -> Self {
        Self {
            diagnostics: Vec::new(),
            js_diagnostics: Vec::new(),
            comment_directives: Vec::new(),
            pragmas: Vec::new(),
            referenced_files: Vec::new(),
            type_reference_directives: Vec::new(),
            lib_reference_directives: Vec::new(),
            check_js_directive: None,
            common_js_module_indicator: None,
            is_declaration_file: false,
            contains_non_ascii: false,
            language_variant: core::LanguageVariant::Standard,
            script_kind: core::ScriptKind::Unknown,
            source_flags: NodeFlags::NONE,
            identifiers: Arc::new(HashMap::new()),
            node_count: 0,
            text_count: 0,
            identifier_count: 0,
            reparsed_clones: Vec::new(),
            imports: Vec::new(),
            module_augmentations: Vec::new(),
            ambient_module_names: Vec::new(),
            uses_uri_style_node_core_modules: core::Tristate::Unknown,
            external_module_indicator: None,
            hash: 0,
        }
    }
}

impl SourceFileData {
    pub(crate) fn new(
        opts: SourceFileParseOptions,
        text: Arc<str>,
        statements: NodeListId,
        end_of_file_token: Option<Node>,
    ) -> Self {
        Self {
            composite_base: CompositeBase::default(),
            file_name: opts.file_name.clone(),
            parse_options: opts,
            text,
            statements,
            end_of_file_token,
            diagnostics: Vec::new(),
            js_diagnostics: Vec::new(),
            language_variant: core::LanguageVariant::Standard,
            script_kind: core::ScriptKind::Unknown,
            is_declaration_file: false,
            contains_non_ascii: false,
            uses_uri_style_node_core_modules: core::Tristate::Unknown,
            identifiers: Arc::new(HashMap::new()),
            identifier_count: 0,
            imports: Vec::new(),
            module_augmentations: Vec::new(),
            ambient_module_names: Vec::new(),
            comment_directives: Vec::new(),
            reparsed_clones: Vec::new(),
            pragmas: Vec::new(),
            referenced_files: Vec::new(),
            type_reference_directives: Vec::new(),
            lib_reference_directives: Vec::new(),
            check_js_directive: None,
            node_count: 0,
            text_count: 0,
            common_js_module_indicator: None,
            external_module_indicator: None,
            ecma_line_map: OnceLock::new(),
            hash: 0,
            token_cache_mu: Mutex::new(()),
            token_cache: HashMap::new(),
            declaration_map_mu: Mutex::new(()),
            declaration_map: HashMap::new(),
            name_table_once: OnceLock::new(),
            name_table: HashMap::new(),
            position_map: OnceLock::new(),
            bind_once_state: OnceLock::new(),
        }
    }
}

pub(crate) struct SourceFileCopyMetadata {
    pub(crate) language_variant: core::LanguageVariant,
    pub(crate) script_kind: core::ScriptKind,
    pub(crate) is_declaration_file: bool,
    pub(crate) contains_non_ascii: bool,
    pub(crate) uses_uri_style_node_core_modules: core::Tristate,
    pub(crate) identifiers: Arc<HashMap<String, String>>,
    pub(crate) imports: Vec<Node>,
    pub(crate) module_augmentations: Vec<Node>,
    pub(crate) ambient_module_names: Vec<String>,
    pub(crate) comment_directives: Vec<CommentDirective>,
    pub(crate) pragmas: Vec<Pragma>,
    pub(crate) referenced_files: Vec<FileReference>,
    pub(crate) type_reference_directives: Vec<FileReference>,
    pub(crate) lib_reference_directives: Vec<FileReference>,
    pub(crate) common_js_module_indicator: Option<Node>,
    pub(crate) external_module_indicator: Option<Node>,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SourceFileSelfReferences {
    common_js_module_indicator: bool,
    external_module_indicator: bool,
}

pub(crate) struct MappedSourceFileMetadata {
    pub(crate) metadata: SourceFileCopyMetadata,
    pub(crate) self_references: SourceFileSelfReferences,
}

impl SourceFileCopyMetadata {
    pub(crate) fn from_source(source: &SourceFileData) -> Self {
        Self {
            language_variant: source.language_variant,
            script_kind: source.script_kind,
            is_declaration_file: source.is_declaration_file,
            contains_non_ascii: source.contains_non_ascii,
            uses_uri_style_node_core_modules: source.uses_uri_style_node_core_modules,
            identifiers: source.identifiers.clone(),
            imports: source.imports.clone(),
            module_augmentations: source.module_augmentations.clone(),
            ambient_module_names: source.ambient_module_names.clone(),
            comment_directives: source.comment_directives.clone(),
            pragmas: source.pragmas.clone(),
            referenced_files: source.referenced_files.clone(),
            type_reference_directives: source.type_reference_directives.clone(),
            lib_reference_directives: source.lib_reference_directives.clone(),
            common_js_module_indicator: source.common_js_module_indicator,
            external_module_indicator: source.external_module_indicator,
        }
    }

    pub(crate) fn map_nodes(
        mut self,
        source_file: Node,
        mut preserve_node: impl FnMut(Node) -> Node,
    ) -> MappedSourceFileMetadata {
        let mut self_references = SourceFileSelfReferences::default();
        self.imports = self.imports.into_iter().map(&mut preserve_node).collect();
        self.module_augmentations = self
            .module_augmentations
            .into_iter()
            .map(&mut preserve_node)
            .collect();
        self.common_js_module_indicator = self.common_js_module_indicator.and_then(|node| {
            if node == source_file {
                self_references.common_js_module_indicator = true;
                None
            } else {
                Some(preserve_node(node))
            }
        });
        self.external_module_indicator = self.external_module_indicator.and_then(|node| {
            if node == source_file {
                self_references.external_module_indicator = true;
                None
            } else {
                Some(preserve_node(node))
            }
        });
        MappedSourceFileMetadata {
            metadata: self,
            self_references,
        }
    }
}

impl NodeFactory {
    pub fn new_source_file(
        &mut self,
        opts: SourceFileParseOptions,
        text: impl Into<Arc<str>>,
        statements: impl IntoNodeList,
        end_of_file_token: impl Into<Option<Node>>,
    ) -> Node {
        let statements = statements.into_node_list();
        statements.assert_store(self.store.store_id());
        let statements = statements.id();
        if tspath::get_encoded_root_length(&opts.file_name) == 0
            || opts.file_name != tspath::normalize_path(&opts.file_name)
        {
            panic!(
                "fileName should be normalized and absolute: {:?}",
                opts.file_name
            );
        }
        let data = SourceFileData::new(opts, text.into(), statements, end_of_file_token.into());
        let payload_idx = self.store.payloads_mut().source_file.alloc(data);
        let payload = NodePayloadId::new(NodePayloadTag::SourceFile, payload_idx.into_raw());
        self.new_node(Kind::SourceFile, NodeFlags::NONE, payload)
    }

    pub fn update_source_file(
        &mut self,
        node: Node,
        source_data: &SourceFileData,
        statements: impl IntoOptionalNodeList,
        end_of_file_token: impl Into<Option<Node>>,
    ) -> Node {
        let original_metadata = self.store.update_metadata(node);
        self.update_source_file_with_metadata(
            node,
            source_data,
            original_metadata,
            statements,
            end_of_file_token,
        )
    }

    pub fn update_source_file_in_current_store(
        &mut self,
        node: Node,
        statements: impl IntoOptionalNodeList,
        end_of_file_token: impl Into<Option<Node>>,
    ) -> Node {
        let original_metadata = self.store.update_metadata(node);
        let statements = statements.into_optional_node_list().map(|statements| {
            statements.assert_store(self.store.store_id());
            statements.id()
        });
        let end_of_file_token = end_of_file_token.into();
        let (parse_options, text, old_statements, old_end_of_file_token, metadata) = {
            let source_data = self.store.as_source_file(node);
            (
                source_data.parse_options.clone(),
                source_data.text.clone(),
                source_data.statements,
                source_data.end_of_file_token,
                SourceFileCopyMetadata::from_source(source_data),
            )
        };
        let statements = statements.unwrap_or(old_statements);
        statements.assert_store(self.store.store_id());
        if statements == old_statements && end_of_file_token == old_end_of_file_token {
            return node;
        }

        let mut data = SourceFileData::new(parse_options, text, statements, end_of_file_token);
        data.copy_metadata_from(metadata);
        let payload_idx = self.store.payloads_mut().source_file.alloc(data);
        let payload = NodePayloadId::new(NodePayloadTag::SourceFile, payload_idx.into_raw());
        let updated = self.new_node(Kind::SourceFile, NodeFlags::NONE, payload);
        self.finish_update_node(updated, node, original_metadata)
    }

    pub fn update_source_file_from_store(
        &mut self,
        source: &AstStore,
        node: Node,
        source_data: &SourceFileData,
        statements: impl IntoOptionalNodeList,
        end_of_file_token: impl Into<Option<Node>>,
    ) -> Node {
        let original_metadata = source.update_metadata(node);
        let statements = match statements.into_optional_node_list().map(|list| {
            list.assert_store(self.store.store_id());
            list.id()
        }) {
            Some(statements) => statements,
            None if source.store_id() == self.store.store_id() => source_data.statements,
            None => self.deep_clone_node_list_from_store(source, source_data.statements),
        };
        let end_of_file_token = end_of_file_token.into();
        if source.store_id() == self.store.store_id()
            && statements == source_data.statements
            && end_of_file_token == source_data.end_of_file_token
        {
            return node;
        }

        let mut metadata = SourceFileCopyMetadata::from_source(source_data);
        let mut self_references = SourceFileSelfReferences::default();
        if source.store_id() != self.store.store_id() {
            let mapped_metadata = metadata.map_nodes(node, |node| {
                self.deep_clone_node_from_store_preserve_location(source, node)
            });
            metadata = mapped_metadata.metadata;
            self_references = mapped_metadata.self_references;
        }

        let mut data = SourceFileData::new(
            source_data.parse_options.clone(),
            source_data.text.clone(),
            statements,
            end_of_file_token,
        );
        data.copy_metadata_from(metadata);
        let payload_idx = self.store.payloads_mut().source_file.alloc(data);
        let payload = NodePayloadId::new(NodePayloadTag::SourceFile, payload_idx.into_raw());
        let updated = self.new_node(Kind::SourceFile, NodeFlags::NONE, payload);
        self.restore_source_file_self_references(updated, self_references);
        self.finish_update_node(updated, node, original_metadata)
    }

    pub fn import_foreign_aggregate_nodes_from_store(
        &mut self,
        source: &AstStore,
    ) -> crate::arena::NodeSideTable<Node> {
        let foreign_nodes = self.store.foreign_nodes_in_aggregate_storage();
        if foreign_nodes.is_empty() {
            return crate::arena::NodeSideTable::default();
        }

        let mut replacements = crate::arena::NodeSideTable::default();
        self.import_foreign_nodes_from_store_into(source, foreign_nodes, &mut replacements);
        self.store.replace_aggregate_nodes(&replacements);
        replacements
    }

    pub fn import_foreign_nodes_from_store_into(
        &mut self,
        source: &AstStore,
        nodes: impl IntoIterator<Item = Node>,
        replacements: &mut crate::arena::NodeSideTable<Node>,
    ) {
        assert!(
            self.clone_recorder.is_none(),
            "nested clone recording is not supported"
        );
        let nodes = nodes.into_iter();
        let root_count_hint = match nodes.size_hint() {
            (lower, Some(upper)) if lower == upper => Some(upper),
            _ => None,
        };
        self.clone_recorder = Some(
            source
                .new_node_map_with_capacity(clone_recorder_capacity(source.len(), root_count_hint)),
        );
        let replacements_for_source = replacements.store(source.store_id());
        for node in nodes {
            assert_eq!(
                node.store_id(),
                source.store_id(),
                "transform output contains an aggregate node from an unrelated AST store"
            );
            if replacements_for_source.contains_key_same_store(node)
                || self.recorded_clone(node).is_some()
            {
                continue;
            }
            self.deep_clone_node_from_store_preserve_location(source, node);
        }
        let mut recorded = self
            .clone_recorder
            .take()
            .expect("clone recorder should be active");
        replacements.append_store_map(&mut recorded);
    }

    pub(crate) fn recorded_clone(&self, node: Node) -> Option<Node> {
        self.clone_recorder
            .as_ref()
            .and_then(|recorder| recorder.get_copied_same_store(node))
    }

    pub(crate) fn restore_source_file_self_references(
        &mut self,
        node: Node,
        self_references: SourceFileSelfReferences,
    ) {
        let data = self.store.as_source_file_mut(node);
        if self_references.common_js_module_indicator {
            data.common_js_module_indicator = Some(node);
        }
        if self_references.external_module_indicator {
            data.external_module_indicator = Some(node);
        }
    }

    pub(crate) fn update_source_file_from_store_with_mapped_metadata(
        &mut self,
        source: &AstStore,
        node: Node,
        source_data: &SourceFileData,
        metadata: SourceFileCopyMetadata,
        statements: impl IntoOptionalNodeList,
        end_of_file_token: impl Into<Option<Node>>,
    ) -> Node {
        let original_metadata = source.update_metadata(node);
        let statements = match statements.into_optional_node_list().map(|list| {
            list.assert_store(self.store.store_id());
            list.id()
        }) {
            Some(statements) => statements,
            None if source.store_id() == self.store.store_id() => source_data.statements,
            None => self.deep_clone_node_list_from_store(source, source_data.statements),
        };
        let end_of_file_token = end_of_file_token.into();
        if source.store_id() == self.store.store_id()
            && statements == source_data.statements
            && end_of_file_token == source_data.end_of_file_token
        {
            return node;
        }

        let mut data = SourceFileData::new(
            source_data.parse_options.clone(),
            source_data.text.clone(),
            statements,
            end_of_file_token,
        );
        data.copy_metadata_from(metadata);
        let payload_idx = self.store.payloads_mut().source_file.alloc(data);
        let payload = NodePayloadId::new(NodePayloadTag::SourceFile, payload_idx.into_raw());
        let updated = self.new_node(Kind::SourceFile, NodeFlags::NONE, payload);
        self.finish_update_node(updated, node, original_metadata)
    }

    fn update_source_file_with_metadata(
        &mut self,
        node: Node,
        source_data: &SourceFileData,
        original_metadata: NodeUpdateMetadata,
        statements: impl IntoOptionalNodeList,
        end_of_file_token: impl Into<Option<Node>>,
    ) -> Node {
        let statements = statements
            .into_optional_node_list()
            .map(|list| {
                list.assert_store(self.store.store_id());
                list.id()
            })
            .unwrap_or(source_data.statements);
        let end_of_file_token = end_of_file_token.into();
        if statements == source_data.statements
            && end_of_file_token == source_data.end_of_file_token
        {
            return node;
        }
        let data = SourceFileData::new(
            source_data.parse_options.clone(),
            source_data.text.clone(),
            statements,
            end_of_file_token,
        );
        let payload_idx = self.store.payloads_mut().source_file.alloc(data);
        let payload = NodePayloadId::new(NodePayloadTag::SourceFile, payload_idx.into_raw());
        let updated = self.new_node(Kind::SourceFile, NodeFlags::NONE, payload);
        self.store
            .as_source_file_mut(updated)
            .copy_from(source_data);
        self.finish_update_node(updated, node, original_metadata)
    }

    pub fn new_comment_range(
        &self,
        kind: Kind,
        pos: i32,
        end: i32,
        has_trailing_new_line: bool,
    ) -> CommentRange {
        CommentRange {
            text_range: core::new_text_range(pos, end),
            kind,
            has_trailing_new_line,
        }
    }
}

pub(crate) fn clone_recorder_capacity(source_len: usize, root_count_hint: Option<usize>) -> usize {
    const DENSE_SOURCE_LIMIT: usize = 8 * 1024;
    const NODES_PER_ROOT_HINT: usize = 64;

    let dense_capacity = source_len.saturating_add(1);
    if source_len <= DENSE_SOURCE_LIMIT {
        return dense_capacity;
    }

    let Some(root_count) = root_count_hint else {
        return 0;
    };

    root_count
        .saturating_mul(NODES_PER_ROOT_HINT)
        .saturating_add(1)
        .min(dense_capacity)
}

impl SourceFileData {
    pub fn parse_options(&self) -> SourceFileParseOptions {
        self.parse_options.clone()
    }

    pub fn language_variant(&self) -> core::LanguageVariant {
        self.language_variant
    }

    pub fn script_kind(&self) -> core::ScriptKind {
        self.script_kind
    }

    pub fn is_declaration_file(&self) -> bool {
        self.is_declaration_file
    }

    pub fn contains_non_ascii(&self) -> bool {
        self.contains_non_ascii
    }

    pub fn uses_uri_style_node_core_modules(&self) -> core::Tristate {
        self.uses_uri_style_node_core_modules
    }

    pub fn identifiers(&self) -> &HashMap<String, String> {
        &self.identifiers
    }

    pub fn identifier_count(&self) -> i32 {
        self.identifier_count
    }

    pub fn node_count(&self) -> i32 {
        self.node_count
    }

    pub fn text_count(&self) -> i32 {
        self.text_count
    }

    pub fn hash(&self) -> xxh3::Uint128 {
        self.hash
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn shared_text(&self) -> Arc<str> {
        Arc::clone(&self.text)
    }

    pub fn pos(&self) -> i32 {
        core::undefined_text_range().pos()
    }

    pub fn end(&self) -> i32 {
        core::undefined_text_range().end()
    }

    pub fn file_name(&self) -> String {
        self.parse_options.file_name.clone()
    }

    pub fn file_name_ref(&self) -> &str {
        &self.parse_options.file_name
    }

    pub fn get_or_init_bind_once_state<T>(&self, bind: impl FnOnce() -> T) -> Arc<T>
    where
        T: Send + Sync + 'static,
    {
        let state = self.bind_once_state.get_or_init(|| {
            let state: Arc<T> = Arc::new(bind());
            state
        });
        Arc::clone(state).downcast::<T>().unwrap_or_else(|_| {
            panic!(
                "cached bind-once state for `{}` has an unexpected type",
                self.file_name_ref()
            )
        })
    }

    pub fn end_of_file_token(&self) -> Option<Node> {
        self.end_of_file_token
    }

    pub fn path(&self) -> tspath::Path {
        self.parse_options.path.clone()
    }

    pub fn path_ref(&self) -> &str {
        &self.parse_options.path
    }

    pub fn imports(&self) -> &[Node] {
        &self.imports
    }

    pub fn reparsed_clones(&self) -> &[Node] {
        &self.reparsed_clones
    }

    pub fn module_augmentations(&self) -> &[Node] {
        &self.module_augmentations
    }

    pub fn ambient_module_names(&self) -> &[String] {
        &self.ambient_module_names
    }

    pub fn comment_directives(&self) -> &[CommentDirective] {
        &self.comment_directives
    }

    pub fn pragmas(&self) -> &[Pragma] {
        &self.pragmas
    }

    pub fn referenced_files(&self) -> &[FileReference] {
        &self.referenced_files
    }

    pub fn type_reference_directives(&self) -> &[FileReference] {
        &self.type_reference_directives
    }

    pub fn lib_reference_directives(&self) -> &[FileReference] {
        &self.lib_reference_directives
    }

    pub fn check_js_directive(&self) -> Option<&CheckJsDirective> {
        self.check_js_directive.as_ref()
    }

    pub fn common_js_module_indicator(&self) -> Option<Node> {
        self.common_js_module_indicator
    }

    pub fn external_module_indicator(&self) -> Option<Node> {
        self.external_module_indicator
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn js_diagnostics(&self) -> &[Diagnostic] {
        &self.js_diagnostics
    }

    pub fn is_js(&self) -> bool {
        self.script_kind == core::ScriptKind::JS || self.script_kind == core::ScriptKind::JSX
    }

    pub(crate) fn copy_from(&mut self, other: &SourceFileData) {
        self.copy_metadata_from(SourceFileCopyMetadata::from_source(other));
    }

    pub(crate) fn assert_node_metadata_in_store(&self, store_id: crate::arena::StoreId) {
        for node in self
            .imports
            .iter()
            .chain(self.module_augmentations.iter())
            .copied()
            .chain(self.common_js_module_indicator)
            .chain(self.external_module_indicator)
            .chain(self.reparsed_clones.iter().copied())
        {
            assert_eq!(
                node.store_id(),
                store_id,
                "source-file node metadata must belong to the target store"
            );
        }
    }

    pub(crate) fn assert_optional_node_metadata_in_store(
        store_id: crate::arena::StoreId,
        node: Option<Node>,
    ) {
        if let Some(node) = node {
            assert_eq!(
                node.store_id(),
                store_id,
                "source-file node metadata must belong to the target store"
            );
        }
    }

    pub(crate) fn copy_metadata_from(&mut self, metadata: SourceFileCopyMetadata) {
        // Do not copy fields set by NewSourceFile (Text, FileName, Path, or Statements).
        self.language_variant = metadata.language_variant;
        self.script_kind = metadata.script_kind;
        self.is_declaration_file = metadata.is_declaration_file;
        self.contains_non_ascii = metadata.contains_non_ascii;
        self.uses_uri_style_node_core_modules = metadata.uses_uri_style_node_core_modules;
        self.identifiers = metadata.identifiers;
        self.imports = metadata.imports;
        self.module_augmentations = metadata.module_augmentations;
        self.ambient_module_names = metadata.ambient_module_names;
        self.comment_directives = metadata.comment_directives;
        self.pragmas = metadata.pragmas;
        self.referenced_files = metadata.referenced_files;
        self.type_reference_directives = metadata.type_reference_directives;
        self.lib_reference_directives = metadata.lib_reference_directives;
        self.common_js_module_indicator = metadata.common_js_module_indicator;
        self.external_module_indicator = metadata.external_module_indicator;
    }

    pub fn ecma_line_map(&self) -> Arc<[core::TextPos]> {
        Arc::clone(
            self.ecma_line_map
                .get_or_init(|| Arc::from(core::compute_ecma_line_starts(self.text()))),
        )
    }

    // GetPositionMap returns the PositionMap for this source file, computing it lazily.
    pub fn get_position_map(&self) -> PositionMap {
        self.position_map
            .get_or_init(|| {
                if !self.contains_non_ascii {
                    let mut position_map = PositionMap::default();
                    position_map.ascii_only = true;
                    position_map
                } else {
                    compute_position_map(self.text())
                }
            })
            .clone()
    }
}

/// Immutable parse output: syntax tree storage plus the SourceFile root node.
#[derive(Clone)]
pub struct ParsedSourceFile {
    store: Arc<AstStore>,
    root: Node,
}

pub struct SourceFile {
    store: Arc<AstStore>,
    root: Node,
}

#[derive(Clone)]
pub struct DiagnosticFile {
    store: Weak<AstStore>,
    root: Node,
    file_name: String,
    path: tspath::Path,
    text: Arc<str>,
    ecma_line_map: OnceLock<Arc<[core::TextPos]>>,
}

pub struct DiagnosticFileView {
    store: Arc<AstStore>,
    root: Node,
}

#[derive(Clone, Copy)]
pub struct SourceFileView<'a> {
    store: &'a AstStore,
    root: Node,
}

impl DiagnosticFile {
    pub fn from_parsed_source_file(source_file: &ParsedSourceFile) -> Self {
        Self {
            store: Arc::downgrade(source_file.store_arc()),
            root: source_file.root,
            file_name: source_file.file_name(),
            path: source_file.path(),
            text: source_file.shared_text(),
            ecma_line_map: OnceLock::new(),
        }
    }

    pub fn from_source_file(source_file: &SourceFile) -> Self {
        Self {
            store: Arc::downgrade(source_file.store_arc()),
            root: source_file.root,
            file_name: source_file.file_name(),
            path: source_file.path(),
            text: source_file.shared_text(),
            ecma_line_map: OnceLock::new(),
        }
    }

    pub fn from_source_file_view(source_file: &SourceFileView<'_>) -> Self {
        Self {
            store: source_file.store.self_weak(),
            root: source_file.root,
            file_name: source_file.file_name(),
            path: source_file.path(),
            text: source_file.shared_text(),
            ecma_line_map: OnceLock::new(),
        }
    }

    pub(crate) fn from_store_weak(
        store: Weak<AstStore>,
        source_store: &AstStore,
        root: Node,
    ) -> Self {
        let source_file = source_store.source_file_view(root);
        Self {
            store,
            root,
            file_name: source_file.file_name(),
            path: source_file.path(),
            text: source_file.shared_text(),
            ecma_line_map: OnceLock::new(),
        }
    }

    pub fn root(&self) -> Node {
        self.root
    }

    pub fn as_node(&self) -> Node {
        self.root
    }

    pub fn file_name(&self) -> &str {
        &self.file_name
    }

    pub fn path(&self) -> &tspath::Path {
        &self.path
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn shared_text(&self) -> Arc<str> {
        Arc::clone(&self.text)
    }

    pub fn ecma_line_map(&self) -> Arc<[core::TextPos]> {
        Arc::clone(
            self.ecma_line_map
                .get_or_init(|| Arc::from(core::compute_ecma_line_starts(self.text()))),
        )
    }

    pub fn upgrade(&self) -> Option<DiagnosticFileView> {
        self.store.upgrade().map(|store| DiagnosticFileView {
            store,
            root: self.root,
        })
    }
}

impl std::fmt::Debug for DiagnosticFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiagnosticFile")
            .field("root", &self.root)
            .field("file_name", &self.file_name)
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl PartialEq for DiagnosticFile {
    fn eq(&self, other: &Self) -> bool {
        self.root == other.root
    }
}

impl Eq for DiagnosticFile {}

impl Hash for DiagnosticFile {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.root.hash(state);
    }
}

impl DiagnosticFileView {
    pub fn root(&self) -> Node {
        self.root
    }

    pub fn as_node(&self) -> Node {
        self.root
    }

    pub fn store(&self) -> &AstStore {
        self.store.as_ref()
    }

    pub fn file_name(&self) -> &str {
        self.store.as_source_file(self.root).file_name_ref()
    }

    pub fn text(&self) -> &str {
        self.store.as_source_file(self.root).text()
    }

    pub fn source_file_view(&self) -> SourceFileView<'_> {
        SourceFileView::new(self.store(), self.root)
    }
}

impl<'a> SourceFileView<'a> {
    pub(crate) fn new(store: &'a AstStore, root: Node) -> Self {
        assert_eq!(store.kind(root), Kind::SourceFile);
        Self { store, root }
    }

    pub fn from_parsed_source_file(source_file: &'a ParsedSourceFile) -> Self {
        Self {
            store: source_file.store(),
            root: source_file.root(),
        }
    }

    pub fn from_source_file(source_file: &'a SourceFile) -> Self {
        Self {
            store: source_file.store(),
            root: source_file.root(),
        }
    }

    pub fn root(&self) -> Node {
        self.root
    }

    pub fn as_node(&self) -> Node {
        self.root
    }

    pub fn diagnostic_file(&self) -> DiagnosticFile {
        DiagnosticFile::from_source_file_view(self)
    }

    pub fn store(&self) -> &'a AstStore {
        self.store
    }

    pub(crate) fn data(&self) -> &'a SourceFileData {
        self.store.as_source_file(self.root)
    }

    pub fn statements_view(&self) -> SourceNodeList<'a> {
        self.store.source_node_list(self.data().statements)
    }

    pub fn statements<T: AstViewNode>(&self) -> impl Iterator<Item = AstRef<'a, T>> + 'a {
        let store = self.store;
        self.statements_view()
            .iter()
            .filter_map(move |node| store.ast_ref::<T>(node))
    }

    pub fn find_statements<T: AstViewNode>(
        &self,
        mut predicate: impl FnMut(AstRef<'a, T>) -> bool,
    ) -> Option<AstRef<'a, T>> {
        self.statements::<T>().find(|child| predicate(*child))
    }

    pub fn end_of_file_token_node(&self) -> Option<Node> {
        self.data().end_of_file_token()
    }

    pub fn end_of_file_token_view<T: AstViewNode>(&self) -> Option<AstRef<'a, T>> {
        self.end_of_file_token_node()
            .and_then(|node| self.store.ast_ref::<T>(node))
    }

    pub fn text(&self) -> &str {
        self.data().text()
    }

    pub fn shared_text(&self) -> Arc<str> {
        self.data().shared_text()
    }

    pub fn file_name(&self) -> String {
        self.data().file_name()
    }

    pub fn file_name_ref(&self) -> &'a str {
        self.data().file_name_ref()
    }

    pub fn path(&self) -> tspath::Path {
        self.data().path()
    }

    pub fn hash(&self) -> xxh3::Uint128 {
        self.data().hash()
    }

    pub fn source_snapshot_id(&self, source_id: SourceId) -> SourceSnapshotId {
        self.store.source_snapshot_id(self.root, source_id)
    }

    pub fn build_stable_node_ids(&self, source_id: SourceId) -> StableNodeIdMap {
        self.store.build_stable_node_ids(self.root, source_id)
    }
}

impl AstStore {
    fn source_node_list_from_view<'a>(&'a self, view: NodeListView<'a>) -> SourceNodeList<'a> {
        SourceNodeList::new(self, view.id())
    }

    fn source_modifier_list_from_view<'a>(
        &'a self,
        view: ModifierListView<'a>,
    ) -> SourceModifierList<'a> {
        SourceModifierList::new(self, view.id())
    }

    fn source_raw_string_slice_from_id(&self, id: RawStringSliceId) -> SourceRawStringSlice<'_> {
        SourceRawStringSlice::new(self, id)
    }

    pub(crate) fn source_node_list(&self, id: NodeListId) -> SourceNodeList<'_> {
        SourceNodeList::new(self, id)
    }

    pub(crate) fn source_node_list_from_handle(&self, list: NodeList) -> SourceNodeList<'_> {
        SourceNodeList::new(self, list.id())
    }

    pub(crate) fn optional_source_node_list(
        &self,
        id: impl Into<Option<NodeListId>>,
    ) -> Option<SourceNodeList<'_>> {
        id.into().map(|id| self.source_node_list(id))
    }

    pub(crate) fn source_modifier_list(&self, id: ModifierListId) -> SourceModifierList<'_> {
        SourceModifierList::new(self, id)
    }

    pub fn source_modifier_list_from_handle(
        &self,
        modifiers: ModifierList,
    ) -> SourceModifierList<'_> {
        SourceModifierList::new(self, modifiers.id())
    }

    pub(crate) fn optional_source_modifier_list(
        &self,
        id: impl Into<Option<ModifierListId>>,
    ) -> Option<SourceModifierList<'_>> {
        id.into().map(|id| self.source_modifier_list(id))
    }

    pub(crate) fn source_raw_node_slice(&self, id: RawNodeSliceId) -> SourceRawNodeSlice<'_> {
        SourceRawNodeSlice::new(self, id)
    }

    pub(crate) fn optional_source_raw_node_slice(
        &self,
        id: impl Into<Option<RawNodeSliceId>>,
    ) -> Option<SourceRawNodeSlice<'_>> {
        id.into().map(|id| self.source_raw_node_slice(id))
    }

    pub(crate) fn source_raw_string_slice(&self, id: RawStringSliceId) -> SourceRawStringSlice<'_> {
        SourceRawStringSlice::new(self, id)
    }

    pub(crate) fn optional_source_raw_string_slice(
        &self,
        id: impl Into<Option<RawStringSliceId>>,
    ) -> Option<SourceRawStringSlice<'_>> {
        id.into().map(|id| self.source_raw_string_slice(id))
    }

    pub fn source_arguments(&self, node: Node) -> Option<SourceNodeList<'_>> {
        self.arguments(node)
    }

    pub fn source_attributes(&self, node: Node) -> Option<SourceNodeList<'_>> {
        self.attributes_id(node)
            .map(|id| SourceNodeList::new(self, id))
    }

    pub fn source_clauses(&self, node: Node) -> Option<SourceNodeList<'_>> {
        self.clauses(node)
    }

    pub fn source_comment(&self, _node: Node) -> Option<SourceNodeList<'_>> {
        None
    }

    pub fn source_declarations(&self, node: Node) -> Option<SourceNodeList<'_>> {
        self.declarations(node)
    }

    pub fn source_elements(&self, node: Node) -> Option<SourceNodeList<'_>> {
        self.elements(node)
    }

    pub fn source_heritage_clauses(&self, node: Node) -> Option<SourceNodeList<'_>> {
        self.heritage_clauses(node)
    }

    pub fn source_members(&self, node: Node) -> Option<SourceNodeList<'_>> {
        self.members(node)
    }

    pub fn source_modifiers(&self, node: Node) -> Option<SourceModifierList<'_>> {
        self.modifiers(node)
    }

    pub fn source_parameters(&self, node: Node) -> Option<SourceNodeList<'_>> {
        self.parameters(node)
    }

    pub fn source_properties(&self, node: Node) -> Option<SourceNodeList<'_>> {
        self.properties(node)
    }

    pub fn source_statements(&self, node: Node) -> Option<SourceNodeList<'_>> {
        if self.kind(node) == Kind::SourceFile {
            return Some(SourceNodeList::new(
                self,
                self.as_source_file(node).statements,
            ));
        }
        self.statements(node)
    }

    pub fn source_tags(&self, _node: Node) -> Option<SourceNodeList<'_>> {
        None
    }

    pub fn source_template_spans(&self, node: Node) -> Option<SourceNodeList<'_>> {
        self.template_spans(node)
    }

    pub(crate) fn source_text_parts(&self, _node: Node) -> Option<SourceRawStringSlice<'_>> {
        None
    }

    pub fn source_type_arguments(&self, node: Node) -> Option<SourceNodeList<'_>> {
        self.type_arguments(node)
    }

    pub fn source_type_parameters(&self, node: Node) -> Option<SourceNodeList<'_>> {
        self.type_parameters(node)
    }

    pub fn source_types(&self, node: Node) -> Option<SourceNodeList<'_>> {
        self.types(node)
    }

    pub fn source_jsx_children(&self, node: Node) -> SourceNodeList<'_> {
        self.jsx_children(node)
    }

    pub fn source_file_view(&self, root: Node) -> SourceFileView<'_> {
        SourceFileView::new(self, root)
    }

    fn attach_diagnostic_file(&mut self, root: Node, diagnostic_file: DiagnosticFile) {
        assert_eq!(self.kind(root), Kind::SourceFile);
        let data = self.as_source_file_mut(root);
        for diagnostic in data
            .diagnostics
            .iter_mut()
            .chain(data.js_diagnostics.iter_mut())
        {
            diagnostic.set_diagnostic_file_recursively(Some(diagnostic_file.clone()));
        }
    }

    fn into_source_file_arc(mut store: AstStore, root: Node) -> Arc<AstStore> {
        Arc::new_cyclic(|weak| {
            store.set_self_weak(weak.clone());
            let diagnostic_file = DiagnosticFile::from_store_weak(weak.clone(), &store, root);
            store.attach_diagnostic_file(root, diagnostic_file);
            store
        })
    }
}

impl NodeFactory {
    pub fn finish_parsed_source_file_as_parsed(
        mut self,
        root: Node,
        metadata: ParsedSourceFileMetadata,
    ) -> ParsedSourceFile {
        assert_eq!(self.store.kind(root), Kind::SourceFile);
        self.apply_parsed_source_file_metadata(root, metadata);
        let store = AstStore::into_source_file_arc(self.store, root);
        ParsedSourceFile::from_parts(store, root)
    }

    pub fn finish_parsed_source_file(
        self,
        root: Node,
        metadata: ParsedSourceFileMetadata,
    ) -> SourceFile {
        self.finish_parsed_source_file_as_parsed(root, metadata)
            .into_source_file()
    }

    pub fn finish_transformed_source_file(mut self, root: Node) -> SourceFile {
        assert_eq!(
            root.store_id(),
            self.store.store_id(),
            "transformed source files must be rooted in the output factory store"
        );
        assert_eq!(self.store.kind(root), Kind::SourceFile);
        self.store.set_parent_recursive(root);
        self.store
            .as_source_file(root)
            .assert_node_metadata_in_store(self.store.store_id());
        SourceFile {
            store: AstStore::into_source_file_arc(self.store, root),
            root,
        }
    }

    fn source_file_data_mut(&mut self, root: Node) -> &mut SourceFileData {
        self.store.as_source_file_mut(root)
    }

    fn apply_parsed_source_file_metadata(
        &mut self,
        root: Node,
        metadata: ParsedSourceFileMetadata,
    ) {
        SourceFileData::assert_optional_node_metadata_in_store(
            self.store.store_id(),
            metadata.common_js_module_indicator,
        );
        SourceFileData::assert_optional_node_metadata_in_store(
            self.store.store_id(),
            metadata.external_module_indicator,
        );
        for node in metadata
            .imports
            .iter()
            .chain(metadata.module_augmentations.iter())
            .chain(metadata.reparsed_clones.iter())
        {
            assert_eq!(
                node.store_id(),
                self.store.store_id(),
                "source-file node metadata must belong to the target store"
            );
        }
        {
            let data = self.source_file_data_mut(root);
            data.diagnostics = metadata.diagnostics;
            data.js_diagnostics = metadata.js_diagnostics;
            data.comment_directives = metadata.comment_directives;
            data.pragmas = metadata.pragmas;
            data.referenced_files = metadata.referenced_files;
            data.type_reference_directives = metadata.type_reference_directives;
            data.lib_reference_directives = metadata.lib_reference_directives;
            data.check_js_directive = metadata.check_js_directive;
            data.common_js_module_indicator = metadata.common_js_module_indicator;
            data.is_declaration_file = metadata.is_declaration_file;
            data.contains_non_ascii = metadata.contains_non_ascii;
            data.language_variant = metadata.language_variant;
            data.script_kind = metadata.script_kind;
            data.identifiers = metadata.identifiers;
            data.node_count = metadata.node_count;
            data.text_count = metadata.text_count;
            data.identifier_count = metadata.identifier_count;
            data.imports = metadata.imports;
            data.module_augmentations = metadata.module_augmentations;
            data.ambient_module_names = metadata.ambient_module_names;
            data.uses_uri_style_node_core_modules = metadata.uses_uri_style_node_core_modules;
            data.external_module_indicator = metadata.external_module_indicator;
            data.hash = metadata.hash;
            data.reparsed_clones = metadata.reparsed_clones;
        }
        self.store.add_flags(root, metadata.source_flags);
        let statements = self
            .store
            .node_list(self.store.as_source_file(root).statements)
            .iter()
            .collect::<Vec<_>>();
        for statement in statements {
            self.store.set_parent(statement, Some(root));
        }
    }
}

impl ParsedSourceFile {
    pub fn from_parts(store: Arc<AstStore>, root: Node) -> Self {
        assert_eq!(store.kind(root), Kind::SourceFile);
        Self { store, root }
    }

    fn store_arc(&self) -> &Arc<AstStore> {
        &self.store
    }

    pub fn root(&self) -> Node {
        self.root
    }

    pub fn as_node(&self) -> Node {
        self.root
    }

    pub fn store(&self) -> &AstStore {
        self.store_arc()
    }

    pub fn diagnostic_file(&self) -> DiagnosticFile {
        DiagnosticFile::from_parsed_source_file(self)
    }

    pub fn share_readonly(&self) -> ParsedSourceFile {
        self.clone()
    }

    pub fn source_file_view(&self) -> SourceFileView<'_> {
        SourceFileView::new(self.store(), self.root)
    }

    pub fn data(&self) -> &SourceFileData {
        self.store().as_source_file(self.root)
    }

    pub fn statements_view(&self) -> SourceNodeList<'_> {
        self.store().source_node_list(self.data().statements)
    }

    pub fn parse_options(&self) -> SourceFileParseOptions {
        self.data().parse_options()
    }

    pub fn hash(&self) -> xxh3::Uint128 {
        self.data().hash()
    }

    pub fn script_kind(&self) -> core::ScriptKind {
        self.data().script_kind()
    }

    pub fn into_source_file(self) -> SourceFile {
        SourceFile {
            store: self.store,
            root: self.root,
        }
    }

    pub fn text(&self) -> &str {
        self.data().text()
    }

    pub fn file_name(&self) -> String {
        self.data().file_name()
    }

    pub fn file_name_ref(&self) -> &str {
        self.data().file_name_ref()
    }

    pub fn path(&self) -> tspath::Path {
        self.data().path()
    }

    pub fn build_stable_node_ids(&self, source_id: SourceId) -> StableNodeIdMap {
        self.store().build_stable_node_ids(self.root, source_id)
    }

    pub fn source_snapshot_id(&self, source_id: SourceId) -> SourceSnapshotId {
        self.store().source_snapshot_id(self.root, source_id)
    }
}

impl SourceFile {
    pub fn from_parsed(parsed: ParsedSourceFile) -> Self {
        parsed.into_source_file()
    }

    fn store_arc(&self) -> &Arc<AstStore> {
        &self.store
    }

    pub fn root(&self) -> Node {
        self.root
    }

    pub fn as_node(&self) -> Node {
        self.root
    }

    pub fn store(&self) -> &AstStore {
        self.store_arc()
    }

    pub fn diagnostic_file(&self) -> DiagnosticFile {
        DiagnosticFile::from_source_file(self)
    }

    pub fn share_readonly(&self) -> SourceFile {
        SourceFile {
            store: Arc::clone(self.store_arc()),
            root: self.root,
        }
    }

    pub fn share_readonly_slice(files: &[SourceFile]) -> Vec<SourceFile> {
        files.iter().map(SourceFile::share_readonly).collect()
    }

    pub fn source_file_view(&self) -> SourceFileView<'_> {
        SourceFileView::new(self.store(), self.root)
    }

    pub fn data(&self) -> &SourceFileData {
        self.store().as_source_file(self.root)
    }

    pub fn statements_view(&self) -> SourceNodeList<'_> {
        self.store().source_node_list(self.data().statements)
    }

    pub fn text(&self) -> &str {
        self.data().text()
    }

    pub fn file_name(&self) -> String {
        self.data().file_name()
    }

    pub fn path(&self) -> tspath::Path {
        self.data().path()
    }

    pub fn hash(&self) -> xxh3::Uint128 {
        self.data().hash()
    }

    pub fn source_snapshot_id(&self, source_id: SourceId) -> SourceSnapshotId {
        self.store().source_snapshot_id(self.root, source_id)
    }

    pub fn build_stable_node_ids(&self, source_id: SourceId) -> StableNodeIdMap {
        self.store().build_stable_node_ids(self.root, source_id)
    }
}

impl Deref for ParsedSourceFile {
    type Target = SourceFileData;

    fn deref(&self) -> &Self::Target {
        self.data()
    }
}

impl Deref for SourceFileView<'_> {
    type Target = SourceFileData;

    fn deref(&self) -> &Self::Target {
        self.data()
    }
}

impl Deref for SourceFile {
    type Target = SourceFileData;

    fn deref(&self) -> &Self::Target {
        self.data()
    }
}

impl PartialEq for ParsedSourceFile {
    fn eq(&self, other: &Self) -> bool {
        self.root == other.root
    }
}

impl Eq for ParsedSourceFile {}

impl Hash for ParsedSourceFile {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.root.hash(state);
    }
}

impl PartialEq for SourceFile {
    fn eq(&self, other: &Self) -> bool {
        self.root == other.root
    }
}

impl Eq for SourceFile {}

impl Hash for SourceFile {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.root.hash(state);
    }
}

impl HasFileName for SourceFileData {
    fn file_name(&self) -> String {
        SourceFileData::file_name(self)
    }

    fn path(&self) -> tspath::Path {
        SourceFileData::path(self)
    }
}

impl HasFileName for ParsedSourceFile {
    fn file_name(&self) -> String {
        ParsedSourceFile::file_name(self)
    }

    fn path(&self) -> tspath::Path {
        ParsedSourceFile::path(self)
    }
}

impl HasFileName for SourceFile {
    fn file_name(&self) -> String {
        SourceFile::file_name(self)
    }

    fn path(&self) -> tspath::Path {
        SourceFile::path(self)
    }
}

impl HasFileName for SourceFileView<'_> {
    fn file_name(&self) -> String {
        SourceFileView::file_name(self)
    }

    fn path(&self) -> tspath::Path {
        SourceFileView::path(self)
    }
}

impl HasFileName for HasFileNameImpl {
    fn file_name(&self) -> String {
        self.file_name.clone()
    }

    fn path(&self) -> tspath::Path {
        self.path.clone()
    }
}

pub trait SourceFileLike {
    fn text(&self) -> String;
    fn ecma_line_map(&self) -> Arc<[core::TextPos]>;
}

pub trait SourceFileStoreLike: SourceFileLike {
    fn store(&self) -> &AstStore;
    fn as_node(&self) -> Node;

    fn data(&self) -> &SourceFileData {
        SourceFileStoreLike::store(self).as_source_file(self.as_node())
    }
}

impl SourceFileLike for SourceFileData {
    fn text(&self) -> String {
        self.text().to_owned()
    }

    fn ecma_line_map(&self) -> Arc<[core::TextPos]> {
        SourceFileData::ecma_line_map(self)
    }
}

impl SourceFileLike for &SourceFileData {
    fn text(&self) -> String {
        SourceFileData::text(self).to_owned()
    }

    fn ecma_line_map(&self) -> Arc<[core::TextPos]> {
        SourceFileData::ecma_line_map(self)
    }
}

impl SourceFileLike for ParsedSourceFile {
    fn text(&self) -> String {
        self.text().to_owned()
    }

    fn ecma_line_map(&self) -> Arc<[core::TextPos]> {
        self.data().ecma_line_map()
    }
}

impl SourceFileLike for SourceFile {
    fn text(&self) -> String {
        self.text().to_owned()
    }

    fn ecma_line_map(&self) -> Arc<[core::TextPos]> {
        self.data().ecma_line_map()
    }
}

impl SourceFileLike for SourceFileView<'_> {
    fn text(&self) -> String {
        self.text().to_owned()
    }

    fn ecma_line_map(&self) -> Arc<[core::TextPos]> {
        self.data().ecma_line_map()
    }
}

impl SourceFileLike for &SourceFileView<'_> {
    fn text(&self) -> String {
        SourceFileView::text(self).to_owned()
    }

    fn ecma_line_map(&self) -> Arc<[core::TextPos]> {
        self.data().ecma_line_map()
    }
}

impl SourceFileStoreLike for ParsedSourceFile {
    fn store(&self) -> &AstStore {
        ParsedSourceFile::store(self)
    }

    fn as_node(&self) -> Node {
        ParsedSourceFile::as_node(self)
    }
}

impl SourceFileStoreLike for SourceFile {
    fn store(&self) -> &AstStore {
        SourceFile::store(self)
    }

    fn as_node(&self) -> Node {
        SourceFile::as_node(self)
    }
}

impl SourceFileStoreLike for SourceFileView<'_> {
    fn store(&self) -> &AstStore {
        SourceFileView::store(self)
    }

    fn as_node(&self) -> Node {
        SourceFileView::as_node(self)
    }
}

impl SourceFileStoreLike for &SourceFile {
    fn store(&self) -> &AstStore {
        SourceFile::store(self)
    }

    fn as_node(&self) -> Node {
        SourceFile::as_node(self)
    }
}

impl SourceFileStoreLike for &SourceFileView<'_> {
    fn store(&self) -> &AstStore {
        SourceFileView::store(self)
    }

    fn as_node(&self) -> Node {
        SourceFileView::as_node(self)
    }
}

impl SourceFileLike for Arc<ParsedSourceFile> {
    fn text(&self) -> String {
        ParsedSourceFile::text(self).to_owned()
    }

    fn ecma_line_map(&self) -> Arc<[core::TextPos]> {
        self.data().ecma_line_map()
    }
}

impl SourceFileStoreLike for Arc<ParsedSourceFile> {
    fn store(&self) -> &AstStore {
        ParsedSourceFile::store(self)
    }

    fn as_node(&self) -> Node {
        ParsedSourceFile::as_node(self)
    }
}

impl SourceFileLike for Arc<SourceFile> {
    fn text(&self) -> String {
        SourceFile::text(self).to_owned()
    }

    fn ecma_line_map(&self) -> Arc<[core::TextPos]> {
        self.data().ecma_line_map()
    }
}

impl SourceFileStoreLike for Arc<SourceFile> {
    fn store(&self) -> &AstStore {
        SourceFile::store(self)
    }

    fn as_node(&self) -> Node {
        SourceFile::as_node(self)
    }
}

impl SourceFileLike for &SourceFile {
    fn text(&self) -> String {
        SourceFile::text(self).to_owned()
    }

    fn ecma_line_map(&self) -> Arc<[core::TextPos]> {
        self.data().ecma_line_map()
    }
}

impl SourceFileLike for &ParsedSourceFile {
    fn text(&self) -> String {
        ParsedSourceFile::text(self).to_owned()
    }

    fn ecma_line_map(&self) -> Arc<[core::TextPos]> {
        self.data().ecma_line_map()
    }
}

impl SourceFileStoreLike for &ParsedSourceFile {
    fn store(&self) -> &AstStore {
        ParsedSourceFile::store(self)
    }

    fn as_node(&self) -> Node {
        ParsedSourceFile::as_node(self)
    }
}

pub fn is_json_source_file(file: &impl SourceFileStoreLike) -> bool {
    file.data().script_kind == core::ScriptKind::JSON
}

impl AstStore {
    pub fn subtree_facts(&self, node: Node) -> SubtreeFacts {
        crate::subtreefacts::compute_subtree_facts(self, node)
    }

    pub fn children(&self, node: Node) -> Option<SourceNodeList<'_>> {
        match self.kind(node) {
            Kind::JsxElement => Some(SourceNodeList::new(
                self,
                self.as_jsx_element(node).children,
            )),
            Kind::JsxFragment => Some(SourceNodeList::new(
                self,
                self.as_jsx_fragment(node).children,
            )),
            _ => None,
        }
    }

    pub(crate) fn children_id(&self, node: Node) -> Option<NodeListId> {
        match self.kind(node) {
            Kind::JsxElement => Some(self.as_jsx_element(node).children),
            Kind::JsxFragment => Some(self.as_jsx_fragment(node).children),
            _ => None,
        }
    }

    pub fn syntax_list_children(&self, node: Node) -> Option<SourceRawNodeSlice<'_>> {
        (self.kind(node) == Kind::SyntaxList)
            .then(|| SourceRawNodeSlice::new(self, self.as_syntax_list(node).children))
    }

    pub fn import_attributes_elements(&self, node: Node) -> Option<SourceNodeList<'_>> {
        (self.kind(node) == Kind::ImportAttributes)
            .then(|| SourceNodeList::new(self, self.as_import_attributes(node).attributes))
    }
}

pub fn is_external_module(file: &impl SourceFileStoreLike) -> bool {
    file.data().external_module_indicator.is_some()
}

pub fn is_effective_external_module(
    file: &impl SourceFileStoreLike,
    options: &core::CompilerOptions,
) -> bool {
    is_external_module(file)
        || (is_common_js_containing_module_kind(options.get_emit_module_kind())
            && file.data().common_js_module_indicator.is_some())
}

pub fn is_external_or_common_js_module(file: &impl SourceFileStoreLike) -> bool {
    file.data().external_module_indicator.is_some()
        || file.data().common_js_module_indicator.is_some()
}

fn is_common_js_containing_module_kind(kind: core::ModuleKind) -> bool {
    kind == core::ModuleKind::CommonJS
        || core::ModuleKind::Node16 <= kind && kind <= core::ModuleKind::NodeNext
}

pub fn is_global_source_file(file: &impl SourceFileStoreLike) -> bool {
    !is_external_or_common_js_module(file)
}

pub fn is_outer_expression(store: &AstStore, node: Node, kinds: OuterExpressionKinds) -> bool {
    match store.kind(node) {
        Kind::ParenthesizedExpression => kinds.contains(OuterExpressionKinds::PARENTHESES),
        Kind::TypeAssertionExpression | Kind::AsExpression => {
            kinds.contains(OuterExpressionKinds::TYPE_ASSERTIONS)
        }
        Kind::SatisfiesExpression => kinds.contains(
            OuterExpressionKinds::EXPRESSIONS_WITH_TYPE_ARGUMENTS | OuterExpressionKinds::SATISFIES,
        ),
        Kind::ExpressionWithTypeArguments => {
            kinds.contains(OuterExpressionKinds::EXPRESSIONS_WITH_TYPE_ARGUMENTS)
        }
        Kind::NonNullExpression => kinds.contains(OuterExpressionKinds::NON_NULL_ASSERTIONS),
        Kind::PartiallyEmittedExpression => {
            kinds.contains(OuterExpressionKinds::PARTIALLY_EMITTED_EXPRESSIONS)
        }
        _ => false,
    }
}

pub fn skip_outer_expressions(store: &AstStore, node: Node, kinds: OuterExpressionKinds) -> Node {
    let mut node = node;
    while is_outer_expression(store, node, kinds) {
        node = store
            .expression(node)
            .expect("outer expression should have an expression child");
    }
    node
}

pub fn is_entity_name_expression(store: &AstStore, node: Node) -> bool {
    is_entity_name_expression_ex(store, node, false)
}

pub fn is_entity_name_expression_ex(store: &AstStore, node: Node, allow_js: bool) -> bool {
    is_identifier(store, node)
        || is_property_access_expression(store, node)
            && store
                .name(node)
                .is_some_and(|name| is_identifier(store, name))
            && store
                .expression(node)
                .is_some_and(|expression| is_entity_name_expression_ex(store, expression, allow_js))
        || allow_js
            && (store.kind(node) == Kind::ThisKeyword
                || is_element_access_expression(store, node)
                    && store.expression(node).is_some_and(|expression| {
                        is_entity_name_expression_ex(store, expression, allow_js)
                    })
                    && store.argument_expression(node).is_some_and(|argument| {
                        is_string_literal(store, argument) || is_numeric_literal(store, argument)
                    }))
}

#[derive(Clone, Copy)]
pub struct CommentRange {
    pub text_range: core::TextRange,
    pub kind: Kind,
    pub has_trailing_new_line: bool,
}

impl CommentRange {
    pub fn pos(&self) -> i32 {
        self.text_range.pos()
    }

    pub fn end(&self) -> i32 {
        self.text_range.end()
    }

    pub fn len(&self) -> i32 {
        self.text_range.len()
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }
}

#[derive(Clone)]
pub struct FileReference {
    pub text_range: core::TextRange,
    pub file_name: String,
    pub resolution_mode: core::ResolutionMode,
    pub preserve: bool,
}

impl FileReference {
    pub fn pos(&self) -> core::TextPos {
        self.text_range.pos()
    }

    pub fn end(&self) -> core::TextPos {
        self.text_range.end()
    }
}

#[derive(Clone)]
pub struct PragmaArgument {
    pub text_range: core::TextRange,
    pub name: String,
    pub value: String,
}

#[derive(Clone)]
pub struct Pragma {
    pub comment_range: CommentRange,
    pub name: String,
    pub args: HashMap<String, PragmaArgument>,
}

pub fn get_pragma_from_source_file<'a>(
    file: Option<&'a SourceFile>,
    name: &str,
) -> Option<&'a Pragma> {
    file.and_then(|file| file.pragmas.iter().rev().find(|pragma| pragma.name == name))
}

pub fn get_pragma_argument<'a>(pragma: Option<&'a Pragma>, name: &str) -> Option<&'a str> {
    pragma.and_then(|pragma| {
        pragma
            .args
            .get(name)
            .map(|argument| argument.value.as_str())
    })
}

pub fn get_jsx_implicit_import_base(
    compiler_options: &core::CompilerOptions,
    file: &SourceFile,
) -> String {
    let jsx_import_source_pragma = get_pragma_from_source_file(Some(file), "jsximportsource");
    let jsx_runtime_pragma = get_pragma_from_source_file(Some(file), "jsxruntime");
    if get_pragma_argument(jsx_runtime_pragma, "factory") == Some("classic") {
        return String::new();
    }
    if compiler_options.jsx == core::JsxEmit::ReactJSX
        || compiler_options.jsx == core::JsxEmit::ReactJSXDev
        || !compiler_options.jsx_import_source.is_empty()
        || jsx_import_source_pragma.is_some()
        || get_pragma_argument(jsx_runtime_pragma, "factory") == Some("automatic")
    {
        let result = get_pragma_argument(jsx_import_source_pragma, "factory")
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| {
                if compiler_options.jsx_import_source.is_empty() {
                    "react"
                } else {
                    &compiler_options.jsx_import_source
                }
            });
        return result.to_owned();
    }
    String::new()
}

pub fn get_jsx_runtime_import(base: &str, options: &core::CompilerOptions) -> String {
    if base.is_empty() {
        return String::new();
    }
    let runtime = if options.jsx == core::JsxEmit::ReactJSXDev {
        "jsx-dev-runtime"
    } else {
        "jsx-runtime"
    };
    format!("{base}/{runtime}")
}

pub fn for_each_return_statement(
    store: &AstStore,
    body: impl AsRef<Node>,
    mut visitor: impl FnMut(Node) -> bool,
) -> bool {
    fn traverse(store: &AstStore, node: Node, visitor: &mut impl FnMut(Node) -> bool) -> bool {
        match store.kind(node) {
            Kind::ReturnStatement => visitor(node),
            Kind::CaseBlock
            | Kind::Block
            | Kind::IfStatement
            | Kind::DoStatement
            | Kind::WhileStatement
            | Kind::ForStatement
            | Kind::ForInStatement
            | Kind::ForOfStatement
            | Kind::WithStatement
            | Kind::SwitchStatement
            | Kind::CaseClause
            | Kind::DefaultClause
            | Kind::LabeledStatement
            | Kind::TryStatement
            | Kind::CatchClause => store
                .for_each_present_child(node, |child| {
                    if traverse(store, child, visitor) {
                        ControlFlow::Break(())
                    } else {
                        ControlFlow::Continue(())
                    }
                })
                .is_break(),
            _ => false,
        }
    }

    traverse(store, *body.as_ref(), &mut visitor)
}

pub fn is_deprecated_declaration_with_cached_flags(
    _store: &AstStore,
    _declaration: impl AsRef<Node>,
    combined_flags: NodeFlags,
) -> bool {
    let _ = combined_flags;
    false
}

pub fn is_deprecated_declaration(store: &AstStore, declaration: impl AsRef<Node>) -> bool {
    is_deprecated_declaration_with_cached_flags(
        store,
        *declaration.as_ref(),
        get_combined_node_flags(store, *declaration.as_ref()),
    )
}

fn get_source_of_assignment(store: &AstStore, node: Node) -> Option<Node> {
    if !is_expression_statement(store, node) {
        return None;
    }
    let expression = store.expression(node)?;
    if !is_binary_expression(store, expression) {
        return None;
    }
    let operator = store.operator_token(expression)?;
    (store.kind(operator) == Kind::EqualsToken)
        .then(|| get_right_most_assigned_expression(store, expression))
}

fn get_source_of_defaulted_assignment(store: &AstStore, node: Node) -> Option<Node> {
    if !is_expression_statement(store, node) {
        return None;
    }
    let expression = store.expression(node)?;
    if !is_binary_expression(store, expression)
        || get_assignment_declaration_kind(store, expression).is_none()
    {
        return None;
    }
    let right = store.right(expression)?;
    if !is_binary_expression(store, right) {
        return None;
    }
    let operator = store.operator_token(right)?;
    matches!(
        store.kind(operator),
        Kind::BarBarToken | Kind::QuestionQuestionToken
    )
    .then(|| store.right(right))
    .flatten()
}

fn get_single_initializer_of_variable_statement_or_property_declaration(
    store: &AstStore,
    node: Node,
) -> Option<Node> {
    match store.kind(node) {
        Kind::VariableStatement => {
            let variable = get_single_variable_of_variable_statement(store, node)?;
            store.initializer(variable)
        }
        Kind::PropertyDeclaration | Kind::PropertyAssignment => store.initializer(node),
        _ => None,
    }
}

pub fn get_single_variable_of_variable_statement(store: &AstStore, node: Node) -> Option<Node> {
    if !is_variable_statement(store, node) {
        return None;
    }
    let declaration_list = store.declaration_list(node)?;
    let declarations = store.declarations(declaration_list)?;
    declarations.first()
}

fn get_nested_module_declaration(store: &AstStore, node: Node) -> Option<Node> {
    if !is_module_declaration(store, node) {
        return None;
    }
    let body = store.body(node)?;
    (store.kind(body) == Kind::ModuleDeclaration).then_some(body)
}

pub fn get_reparsed_node_for_node(store: &AstStore, node: impl AsRef<Node>) -> Node {
    let node = *node.as_ref();
    if store.flags(node).intersects(NodeFlags::REPARSED) {
        return node;
    }
    let Some(source_file) = get_source_file_node_of_node(store, Some(node)) else {
        return node;
    };
    let clones = &store.as_source_file(source_file).reparsed_clones;
    if clones.is_empty() {
        return node;
    }
    let node_loc = store.loc(node);
    let mut candidate = None;
    for clone in clones {
        let clone_loc = store.loc(*clone);
        if node_loc.contained_by(clone_loc) {
            candidate = Some(*clone);
            break;
        }
    }
    candidate
        .and_then(|clone| find_clone_in_node(store, clone, node))
        .unwrap_or(node)
}

fn find_clone_in_node(store: &AstStore, mut node: Node, original: Node) -> Option<Node> {
    loop {
        if store.kind(node) == store.kind(original) && store.loc(node) == store.loc(original) {
            return Some(node);
        }
        let original_loc = store.loc(original);
        let mut next = None;
        let _ = store.for_each_present_child(node, |child| {
            if original_loc.contained_by(store.loc(child)) {
                next = Some(child);
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        });
        node = next?;
    }
}

pub fn is_named_evaluation_source(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let node = *node.as_ref();
    match store.kind(node) {
        Kind::PropertyAssignment => store
            .name(node)
            .is_some_and(|name| !is_proto_setter(store, name)),
        Kind::ShorthandPropertyAssignment => store
            .as_shorthand_property_assignment(node)
            .object_assignment_initializer
            .get()
            .is_some(),
        Kind::VariableDeclaration => {
            store
                .name(node)
                .is_some_and(|name| is_identifier(store, name))
                && store.initializer(node).is_some()
        }
        Kind::Parameter => {
            let parameter = store.as_parameter_declaration(node);
            store
                .optional_node_from_id(parameter.name)
                .is_some_and(|name| is_identifier(store, name))
                && parameter.initializer.get().is_some()
                && parameter.dot_dot_dot_token.get().is_none()
        }
        Kind::BindingElement => {
            let binding = store.as_binding_element(node);
            store
                .optional_node_from_id(binding.name)
                .is_some_and(|name| is_identifier(store, name))
                && binding.initializer.get().is_some()
                && binding.dot_dot_dot_token.get().is_none()
        }
        Kind::PropertyDeclaration => store.initializer(node).is_some(),
        Kind::BinaryExpression => {
            let Some(operator) = store.operator_token(node) else {
                return false;
            };
            matches!(
                store.kind(operator),
                Kind::EqualsToken
                    | Kind::AmpersandAmpersandEqualsToken
                    | Kind::BarBarEqualsToken
                    | Kind::QuestionQuestionEqualsToken
            ) && store
                .left(node)
                .is_some_and(|left| is_identifier(store, left))
        }
        Kind::ExportAssignment => true,
        _ => false,
    }
}

pub fn is_proto_setter(store: &AstStore, node: Node) -> bool {
    (is_identifier(store, node) || is_string_literal(store, node))
        && store.text(node) == "__proto__"
}

pub fn get_leftmost_access_expression(store: &AstStore, mut node: Node) -> Node {
    while is_access_expression(store, node) {
        let Some(expression) = store.expression(node) else {
            break;
        };
        node = expression;
    }
    node
}

pub fn is_resolution_mode_override_host(store: &AstStore, node: Option<Node>) -> bool {
    node.is_some_and(|node| {
        matches!(
            store.kind(node),
            Kind::ImportType
                | Kind::ExportDeclaration
                | Kind::ImportDeclaration
                | Kind::JSImportDeclaration
        )
    })
}

pub fn has_resolution_mode_override(store: &AstStore, node: Option<Node>) -> bool {
    let Some(node) = node else {
        return false;
    };
    let attributes = match store.kind(node) {
        Kind::ImportType => store.as_import_type_node(node).attributes.get(),
        Kind::ImportDeclaration | Kind::JSImportDeclaration => {
            store.as_import_declaration(node).attributes.get()
        }
        Kind::ExportDeclaration => store.as_export_declaration(node).attributes.get(),
        _ => None,
    };
    let Some(attributes) = attributes.map(|id| store.node_from_id(id)) else {
        return false;
    };
    let attrs = store.as_import_attributes(attributes);
    let attrs = store.node_list(attrs.attributes);
    if attrs.len() != 1 {
        return false;
    }
    let Some(attribute) = attrs.first() else {
        return false;
    };
    let attribute = store.as_import_attribute(attribute);
    let Some(name) = store.optional_node_from_id(attribute.name) else {
        return false;
    };
    if !is_string_literal_like(store, name) || store.text(name) != "resolution-mode" {
        return false;
    }
    let Some(value) = store.optional_node_from_id(attribute.value) else {
        return false;
    };
    is_string_literal_like(store, value)
        && matches!(store.text(value).as_str(), "import" | "require")
}

pub fn get_resolution_mode_override(
    store: &AstStore,
    attributes: Node,
) -> (core::ResolutionMode, bool) {
    let attrs = store.as_import_attributes(attributes);
    let attrs = store.node_list(attrs.attributes);
    if attrs.len() != 1 {
        return (core::ResolutionMode::None, false);
    }
    let Some(attribute) = attrs.first() else {
        return (core::ResolutionMode::None, false);
    };
    let attribute = store.as_import_attribute(attribute);
    let Some(name) = store.optional_node_from_id(attribute.name) else {
        return (core::ResolutionMode::None, false);
    };
    if !is_string_literal_like(store, name) || store.text(name) != "resolution-mode" {
        return (core::ResolutionMode::None, false);
    }
    let Some(value) = store.optional_node_from_id(attribute.value) else {
        return (core::ResolutionMode::None, false);
    };
    if !is_string_literal_like(store, value) {
        return (core::ResolutionMode::None, false);
    }
    match store.text(value).as_str() {
        "import" => (core::RESOLUTION_MODE_ESM, true),
        "require" => (core::ModuleKind::CommonJS, true),
        _ => (core::ResolutionMode::None, false),
    }
}

pub fn get_rest_parameter_element_type(store: &AstStore, node: Option<Node>) -> Option<Node> {
    let node = node?;
    match store.kind(node) {
        Kind::ArrayType => store.element_type(node),
        Kind::TypeReference => store.type_arguments(node).and_then(|args| args.first()),
        _ => None,
    }
}

pub fn is_in_expression_context(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let node = *node.as_ref();
    let Some(parent) = store.parent(node) else {
        return false;
    };
    match store.kind(parent) {
        Kind::VariableDeclaration
        | Kind::Parameter
        | Kind::PropertyDeclaration
        | Kind::PropertySignature
        | Kind::EnumMember
        | Kind::PropertyAssignment
        | Kind::BindingElement => store.initializer(parent) == Some(node),
        Kind::ExpressionStatement
        | Kind::IfStatement
        | Kind::DoStatement
        | Kind::WhileStatement
        | Kind::ReturnStatement
        | Kind::WithStatement
        | Kind::SwitchStatement
        | Kind::CaseClause
        | Kind::DefaultClause
        | Kind::ThrowStatement
        | Kind::TypeAssertionExpression
        | Kind::AsExpression
        | Kind::TemplateSpan
        | Kind::ComputedPropertyName
        | Kind::SatisfiesExpression => store.expression(parent) == Some(node),
        Kind::ForStatement => {
            (store.initializer(parent) == Some(node)
                && store.kind(node) != Kind::VariableDeclarationList)
                || store.condition(parent) == Some(node)
                || store.incrementor(parent) == Some(node)
        }
        Kind::ForInStatement | Kind::ForOfStatement => {
            (store.initializer(parent) == Some(node)
                && store.kind(node) != Kind::VariableDeclarationList)
                || store.expression(parent) == Some(node)
        }
        Kind::Decorator
        | Kind::JsxExpression
        | Kind::JsxSpreadAttribute
        | Kind::SpreadAssignment => true,
        Kind::ExpressionWithTypeArguments => {
            store.expression(parent) == Some(node) && !is_part_of_type_node(store, parent)
        }
        Kind::ShorthandPropertyAssignment => {
            store
                .as_shorthand_property_assignment(parent)
                .object_assignment_initializer
                .get()
                .map(|id| store.node_from_id(id))
                == Some(node)
        }
        _ => is_expression_node(store, parent),
    }
}

pub fn is_expression_with_type_arguments_in_class_extends_clause(
    store: &AstStore,
    node: impl AsRef<Node>,
) -> bool {
    try_get_class_implementing_or_extending_expression_with_type_arguments(store, node)
        .is_some_and(|(_, is_implements)| !is_implements)
}

pub fn try_get_class_implementing_or_extending_expression_with_type_arguments(
    store: &AstStore,
    node: impl AsRef<Node>,
) -> Option<(Node, bool)> {
    let node = *node.as_ref();
    if !is_expression_with_type_arguments(store, node) {
        return None;
    }
    let parent = store.parent(node)?;
    if !is_heritage_clause(store, parent) {
        return None;
    }
    let class = store.parent(parent)?;
    if !is_class_like(store, class) {
        return None;
    }
    Some((
        class,
        store.as_heritage_clause(parent).token == Kind::ImplementsKeyword,
    ))
}

pub fn is_type_declaration_name(store: &AstStore, name: impl AsRef<Node>) -> bool {
    let name = *name.as_ref();
    is_identifier(store, name)
        && store.parent(name).is_some_and(|parent| {
            is_type_declaration(store, parent) && store.name(parent) == Some(name)
        })
}

pub fn get_node_at_position(store: &AstStore, source_file: Node, position: i32) -> Node {
    fn contains_position(store: &AstStore, node: Node, position: i32) -> bool {
        let loc = store.loc(node);
        loc.pos() <= position
            && (position < loc.end()
                || (position == loc.end() && store.kind(node) == Kind::EndOfFile))
    }

    let mut current = source_file;
    loop {
        let mut next = None;
        if next.is_none() {
            let _ = store.for_each_present_child(current, |child| {
                if contains_position(store, child, position) {
                    next = Some(child);
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                }
            });
        }
        match next {
            Some(child) if !is_meta_property(store, child) => current = child,
            _ => return current,
        }
    }
}

pub fn is_late_visibility_painted_statement(store: &AstStore, node: impl AsRef<Node>) -> bool {
    matches!(
        store.kind(*node.as_ref()),
        Kind::ImportDeclaration
            | Kind::JSImportDeclaration
            | Kind::ImportEqualsDeclaration
            | Kind::VariableStatement
            | Kind::ClassDeclaration
            | Kind::FunctionDeclaration
            | Kind::ModuleDeclaration
            | Kind::TypeAliasDeclaration
            | Kind::JSTypeAliasDeclaration
            | Kind::InterfaceDeclaration
            | Kind::EnumDeclaration
    )
}

pub fn is_function_or_source_file(store: &AstStore, node: Node) -> bool {
    is_function_like(store, Some(node)) || is_source_file(store, node)
}

#[derive(Clone, Copy)]
pub struct AllAccessorDeclarations {
    pub get_accessor: Option<Node>,
    pub set_accessor: Option<Node>,
    pub first_accessor: Node,
    pub second_accessor: Option<Node>,
}

pub fn get_all_accessor_declarations_for_declaration(
    store: &AstStore,
    accessor: Node,
    declarations: &[Node],
) -> AllAccessorDeclarations {
    let other_kind = match store.kind(accessor) {
        Kind::SetAccessor => Kind::GetAccessor,
        Kind::GetAccessor => Kind::SetAccessor,
        kind => panic!("Unexpected node kind {kind:?}"),
    };
    let other_accessor = declarations
        .iter()
        .copied()
        .find(|declaration| store.kind(*declaration) == other_kind);
    let (first_accessor, second_accessor) = if other_accessor
        .is_some_and(|other_accessor| store.loc(other_accessor).pos() < store.loc(accessor).pos())
    {
        (other_accessor.unwrap(), Some(accessor))
    } else {
        (accessor, other_accessor)
    };

    AllAccessorDeclarations {
        get_accessor: if store.kind(accessor) == Kind::GetAccessor {
            Some(accessor)
        } else {
            other_accessor
        },
        set_accessor: if store.kind(accessor) == Kind::SetAccessor {
            Some(accessor)
        } else {
            other_accessor
        },
        first_accessor,
        second_accessor,
    }
}

pub fn get_all_accessor_declarations(
    store: &AstStore,
    declarations: &[Node],
    accessor: Node,
) -> AllAccessorDeclarations {
    if has_dynamic_name(store, accessor) {
        return get_all_accessor_declarations_for_declaration(store, accessor, &[accessor]);
    }

    let Some(accessor_name) = store.name(accessor) else {
        return get_all_accessor_declarations_for_declaration(store, accessor, &[accessor]);
    };
    let accessor_name = get_property_name_for_property_name_node(store, accessor_name);
    let accessor_static = is_static(store, accessor);
    let matches = declarations
        .iter()
        .copied()
        .filter(|member| {
            is_accessor(store, member)
                && is_static(store, *member) == accessor_static
                && store.name(*member).is_some_and(|member_name| {
                    get_property_name_for_property_name_node(store, member_name) == accessor_name
                })
        })
        .collect::<Vec<_>>();
    get_all_accessor_declarations_for_declaration(store, accessor, &matches)
}

pub fn can_have_illegal_modifiers(store: &AstStore, node: impl AsRef<Node>) -> bool {
    matches!(
        store.kind(*node.as_ref()),
        Kind::ClassStaticBlockDeclaration
            | Kind::PropertyAssignment
            | Kind::ShorthandPropertyAssignment
            | Kind::MissingDeclaration
            | Kind::NamespaceExportDeclaration
    )
}

pub fn is_comma_sequence(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let node = *node.as_ref();
    is_binary_expression(store, node)
        && store
            .operator_token(node)
            .is_some_and(|operator| store.kind(operator) == Kind::CommaToken)
}

pub fn is_function_like_or_class_static_block_declaration(
    store: &AstStore,
    node: Option<Node>,
) -> bool {
    node.is_some_and(|node| {
        is_function_like(store, Some(node)) || is_class_static_block_declaration(store, node)
    })
}

pub fn is_iteration_statement(
    store: &AstStore,
    node: impl AsRef<Node>,
    look_in_labeled_statements: bool,
) -> bool {
    let node = *node.as_ref();
    match store.kind(node) {
        Kind::ForStatement
        | Kind::ForInStatement
        | Kind::ForOfStatement
        | Kind::DoStatement
        | Kind::WhileStatement => true,
        Kind::LabeledStatement => {
            look_in_labeled_statements
                && store.statement(node).is_some_and(|statement| {
                    is_iteration_statement(store, statement, look_in_labeled_statements)
                })
        }
        _ => false,
    }
}

pub fn is_jsx_attribute_like(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let node = *node.as_ref();
    is_jsx_attribute(store, node) || is_jsx_spread_attribute(store, node)
}

pub fn get_semantic_jsx_children(store: &AstStore, children: &[Node]) -> Vec<Node> {
    children
        .iter()
        .copied()
        .filter(|child| match store.kind(*child) {
            Kind::JsxExpression => store.expression(*child).is_some(),
            Kind::JsxText => !store
                .contains_only_trivia_white_spaces(*child)
                .unwrap_or(false),
            _ => true,
        })
        .collect()
}

pub fn is_property_access_entity_name_expression(
    store: &AstStore,
    node: impl AsRef<Node>,
    allow_js: bool,
) -> bool {
    let node = *node.as_ref();
    is_property_access_expression(store, node)
        && store
            .name(node)
            .is_some_and(|name| is_identifier(store, name))
        && store
            .expression(node)
            .is_some_and(|expression| is_entity_name_expression_ex(store, expression, allow_js))
}

pub fn has_inferred_type(store: &AstStore, node: impl AsRef<Node>) -> bool {
    matches!(
        store.kind(*node.as_ref()),
        Kind::Parameter
            | Kind::PropertySignature
            | Kind::PropertyDeclaration
            | Kind::BindingElement
            | Kind::PropertyAccessExpression
            | Kind::ElementAccessExpression
            | Kind::BinaryExpression
            | Kind::CallExpression
            | Kind::VariableDeclaration
            | Kind::ExportAssignment
            | Kind::PropertyAssignment
            | Kind::ShorthandPropertyAssignment
    )
}

pub fn is_module_identifier(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let node = *node.as_ref();
    is_identifier(store, node) && store.text(node) == "module"
}

pub fn is_exports_identifier(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let node = *node.as_ref();
    is_identifier(store, node) && store.text(node) == "exports"
}

pub fn is_literal_like_element_access(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let node = *node.as_ref();
    is_element_access_expression(store, node)
        && store
            .argument_expression(node)
            .is_some_and(|argument| is_string_or_numeric_literal_like(store, argument))
}

pub fn is_bindable_static_element_access_expression(
    store: &AstStore,
    node: impl AsRef<Node>,
    exclude_this_keyword: bool,
) -> bool {
    let node = *node.as_ref();
    is_literal_like_element_access(store, node)
        && store.expression(node).is_some_and(|expression| {
            (!exclude_this_keyword && store.kind(expression) == Kind::ThisKeyword)
                || is_entity_name_expression(store, expression)
                || is_bindable_static_access_expression(store, expression, true)
        })
}

pub fn is_bindable_static_access_expression(
    store: &AstStore,
    node: impl AsRef<Node>,
    exclude_this_keyword: bool,
) -> bool {
    let node = *node.as_ref();
    (is_property_access_expression(store, node)
        && store.expression(node).is_some_and(|expression| {
            (!exclude_this_keyword && store.kind(expression) == Kind::ThisKeyword)
                || store
                    .name(node)
                    .is_some_and(|name| is_identifier(store, name))
                    && is_bindable_static_name_expression(store, expression, true)
        }))
        || is_bindable_static_element_access_expression(store, node, exclude_this_keyword)
}

pub fn is_prototype_access(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let node = *node.as_ref();
    is_bindable_static_access_expression(store, node, false)
        && get_element_or_property_access_name(store, node)
            .is_some_and(|name| store.text(name) == "prototype")
}

pub fn is_bindable_static_name_expression(
    store: &AstStore,
    node: impl AsRef<Node>,
    exclude_this_keyword: bool,
) -> bool {
    let node = *node.as_ref();
    is_entity_name_expression(store, node)
        || is_bindable_static_access_expression(store, node, exclude_this_keyword)
}

pub fn is_module_exports_access_expression(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let node = *node.as_ref();
    is_access_expression(store, node)
        && store
            .expression(node)
            .is_some_and(|expression| is_module_identifier(store, expression))
        && get_element_or_property_access_name(store, node)
            .is_some_and(|name| store.text(name) == "exports")
}

pub fn is_bindable_object_define_property_call(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let node = *node.as_ref();
    let Some(arguments) = store.arguments(node) else {
        return false;
    };
    if arguments.len() != 3 {
        return false;
    }
    let Some(expression) = store.expression(node) else {
        return false;
    };
    if !is_property_access_expression(store, expression) {
        return false;
    }
    store
        .expression(expression)
        .is_some_and(|object| is_identifier(store, object) && store.text(object) == "Object")
        && store
            .name(expression)
            .is_some_and(|name| store.text(name) == "defineProperty")
        && arguments
            .iter()
            .nth(1)
            .is_some_and(|argument| is_string_or_numeric_literal_like(store, argument))
        && arguments
            .first()
            .is_some_and(|argument| is_bindable_static_name_expression(store, argument, true))
}

pub fn object_define_property_call_property_name_argument(
    store: &AstStore,
    node: Node,
) -> Option<Node> {
    if !is_call_expression(store, node) {
        return None;
    }
    let arguments = store.arguments(node)?;
    if arguments.len() != 3 {
        return None;
    }
    let expression = store.expression(node)?;
    if !is_property_access_expression(store, expression) {
        return None;
    }
    let object = store.expression(expression)?;
    let name = store.name(expression)?;
    if !is_identifier(store, object) || store.text(object) != "Object" {
        return None;
    }
    if store.text(name) != "defineProperty" {
        return None;
    }
    let property_name_argument = arguments.iter().nth(1)?;
    (is_string_or_numeric_literal_like(store, property_name_argument))
        .then_some(property_name_argument)
}

pub fn bindable_object_define_property_call_property_name_argument(
    store: &AstStore,
    node: Node,
) -> Option<Node> {
    if !is_bindable_object_define_property_call(store, node) {
        return None;
    }
    store
        .arguments(node)
        .and_then(|arguments| arguments.iter().nth(1))
}

pub fn is_object_define_property_call(store: &AstStore, node: Node) -> bool {
    object_define_property_call_property_name_argument(store, node).is_some()
}

pub fn expression_is_alias(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let node = *node.as_ref();
    is_entity_name_expression(store, node) || is_class_expression(store, node)
}

pub fn is_alias_declaration(store: &AstStore, node: Node) -> bool {
    match store.kind(node) {
        Kind::ImportEqualsDeclaration
        | Kind::NamespaceImport
        | Kind::ImportSpecifier
        | Kind::ExportSpecifier => true,
        Kind::ImportClause => store.name(node).is_some(),
        _ => false,
    }
}

pub fn is_alias_symbol_declaration(store: &AstStore, node: Node) -> bool {
    match store.kind(node) {
        Kind::ImportEqualsDeclaration
        | Kind::NamespaceExportDeclaration
        | Kind::NamespaceImport
        | Kind::NamespaceExport
        | Kind::ImportSpecifier
        | Kind::ExportSpecifier => true,
        Kind::ImportClause => store.name(node).is_some(),
        Kind::ExportAssignment => store
            .expression(node)
            .is_some_and(|expression| expression_is_alias(store, expression)),
        Kind::VariableDeclaration | Kind::BindingElement => {
            is_variable_declaration_initialized_to_require(store, node)
        }
        Kind::BinaryExpression => {
            matches!(
                get_assignment_declaration_kind(store, node),
                Some(JSDeclarationKind::ModuleExports | JSDeclarationKind::ExportsProperty)
            ) && store
                .right(node)
                .is_some_and(|right| expression_is_alias(store, right))
        }
        _ => false,
    }
}

pub fn is_expando_property_declaration(store: &AstStore, node: Node) -> bool {
    is_binary_expression(store, node)
}

pub fn is_implicitly_exported_js_type_alias(store: &AstStore, node: Node) -> bool {
    is_js_type_alias_declaration(store, node)
        && store
            .parent(node)
            .is_some_and(|parent| is_source_file(store, parent))
}

pub fn is_const_type_reference(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let node = *node.as_ref();
    is_type_reference_node(store, node)
        && store
            .type_arguments(node)
            .is_none_or(|arguments| arguments.is_empty())
        && store.type_name(node).is_some_and(|type_name| {
            is_identifier(store, type_name) && store.text(type_name) == "const"
        })
}

pub fn is_object_literal_element(store: &AstStore, node: impl AsRef<Node>) -> bool {
    matches!(
        store.kind(*node.as_ref()),
        Kind::PropertyAssignment
            | Kind::ShorthandPropertyAssignment
            | Kind::SpreadAssignment
            | Kind::MethodDeclaration
            | Kind::GetAccessor
            | Kind::SetAccessor
    )
}

pub fn has_type_arguments(store: &AstStore, node: impl AsRef<Node>) -> bool {
    matches!(
        store.kind(*node.as_ref()),
        Kind::CallExpression
            | Kind::NewExpression
            | Kind::TaggedTemplateExpression
            | Kind::TypeReference
            | Kind::ExpressionWithTypeArguments
            | Kind::ImportType
            | Kind::TypeQuery
            | Kind::JsxOpeningElement
            | Kind::JsxSelfClosingElement
    )
}

pub fn is_call_like_or_function_like_expression(store: &AstStore, node: impl AsRef<Node>) -> bool {
    let node = *node.as_ref();
    is_call_like_expression(store, node) || is_function_expression_or_arrow_function(store, node)
}

pub fn is_array_literal_or_object_literal_destructuring_pattern(
    store: &AstStore,
    node: Option<Node>,
) -> bool {
    let Some(node) = node else {
        return false;
    };
    if !is_array_literal_expression(store, node) && !is_object_literal_expression(store, node) {
        return false;
    }
    let Some(parent) = store.parent(node) else {
        return false;
    };
    if is_binary_expression(store, parent)
        && store.left(parent) == Some(node)
        && store
            .operator_token(parent)
            .is_some_and(|operator| store.kind(operator) == Kind::EqualsToken)
    {
        return true;
    }
    if is_for_of_statement(store, parent) && store.initializer(parent) == Some(node) {
        return true;
    }
    if is_property_assignment(store, parent) {
        return is_array_literal_or_object_literal_destructuring_pattern(store, Some(parent));
    }
    is_array_literal_or_object_literal_destructuring_pattern(store, Some(parent))
}

pub fn entity_name_to_string(
    store: &AstStore,
    name: impl AsRef<Node>,
    get_text_of_node: Option<fn(&AstStore, Node) -> String>,
) -> String {
    let name = *name.as_ref();
    match store.kind(name) {
        Kind::ThisKeyword => "this".to_owned(),
        Kind::Identifier | Kind::PrivateIdentifier => {
            if node_is_synthesized(store, name) {
                store.text(name)
            } else if let Some(get_text_of_node) = get_text_of_node {
                get_text_of_node(store, name)
            } else {
                store.text(name)
            }
        }
        Kind::QualifiedName => {
            let qualified = store.as_qualified_name(name);
            format!(
                "{}.{}",
                entity_name_to_string(
                    store,
                    store
                        .optional_node_from_id(qualified.left)
                        .expect("qualified name should have a left node"),
                    get_text_of_node,
                ),
                entity_name_to_string(
                    store,
                    store
                        .optional_node_from_id(qualified.right)
                        .expect("qualified name should have a right node"),
                    get_text_of_node,
                )
            )
        }
        Kind::PropertyAccessExpression => format!(
            "{}.{}",
            entity_name_to_string(store, store.expression(name).unwrap(), get_text_of_node),
            entity_name_to_string(store, store.name(name).unwrap(), get_text_of_node)
        ),
        Kind::JsxNamespacedName => {
            let jsx = store.as_jsx_namespaced_name(name);
            format!(
                "{}:{}",
                entity_name_to_string(
                    store,
                    store
                        .optional_node_from_id(jsx.namespace)
                        .expect("jsx namespaced name should have a namespace"),
                    get_text_of_node,
                ),
                entity_name_to_string(
                    store,
                    store
                        .optional_node_from_id(jsx.name)
                        .expect("jsx namespaced name should have a name"),
                    get_text_of_node,
                )
            )
        }
        _ => panic!("Unhandled case in entity_name_to_string"),
    }
}

pub fn replace_modifiers(
    factory: &mut NodeFactory,
    node: impl AsRef<Node>,
    modifiers: impl IntoOptionalModifierList,
) -> Node {
    let node = *node.as_ref();
    assert_eq!(
        node.store_id(),
        factory.store().store_id(),
        "replace_modifiers expects the node to belong to the target factory store"
    );
    let modifiers = modifiers.into_optional_modifier_list().map(|modifiers| {
        modifiers.assert_store(factory.store().store_id());
        modifiers.id()
    });
    let modifiers = OptionalModifierListId::from_option(modifiers);
    match factory.store().kind(node) {
        Kind::FunctionDeclaration => {
            factory
                .store_mut()
                .as_function_declaration_mut(node)
                .modifiers = modifiers;
        }
        Kind::ClassDeclaration => {
            factory.store_mut().as_class_declaration_mut(node).modifiers = modifiers;
        }
        Kind::InterfaceDeclaration => {
            factory
                .store_mut()
                .as_interface_declaration_mut(node)
                .modifiers = modifiers;
        }
        Kind::TypeAliasDeclaration | Kind::JSTypeAliasDeclaration => {
            factory
                .store_mut()
                .as_type_alias_declaration_mut(node)
                .modifiers = modifiers;
        }
        Kind::EnumDeclaration => {
            factory.store_mut().as_enum_declaration_mut(node).modifiers = modifiers;
        }
        Kind::ModuleDeclaration => {
            factory
                .store_mut()
                .as_module_declaration_mut(node)
                .modifiers = modifiers;
        }
        Kind::ImportDeclaration | Kind::JSImportDeclaration => {
            factory
                .store_mut()
                .as_import_declaration_mut(node)
                .modifiers = modifiers;
        }
        Kind::ImportEqualsDeclaration => {
            factory
                .store_mut()
                .as_import_equals_declaration_mut(node)
                .modifiers = modifiers;
        }
        Kind::ExportDeclaration => {
            factory
                .store_mut()
                .as_export_declaration_mut(node)
                .modifiers = modifiers;
        }
        Kind::ExportAssignment => {
            factory.store_mut().as_export_assignment_mut(node).modifiers = modifiers;
        }
        Kind::PropertyDeclaration => {
            factory
                .store_mut()
                .as_property_declaration_mut(node)
                .modifiers = modifiers;
        }
        Kind::MethodDeclaration => {
            factory
                .store_mut()
                .as_method_declaration_mut(node)
                .modifiers = modifiers;
        }
        Kind::GetAccessor => {
            factory
                .store_mut()
                .as_get_accessor_declaration_mut(node)
                .modifiers = modifiers;
        }
        Kind::SetAccessor => {
            factory
                .store_mut()
                .as_set_accessor_declaration_mut(node)
                .modifiers = modifiers;
        }
        Kind::Constructor => {
            factory
                .store_mut()
                .as_constructor_declaration_mut(node)
                .modifiers = modifiers;
        }
        Kind::Parameter => {
            factory
                .store_mut()
                .as_parameter_declaration_mut(node)
                .modifiers = modifiers;
        }
        Kind::PropertyAssignment => {
            factory
                .store_mut()
                .as_property_assignment_mut(node)
                .modifiers = modifiers;
        }
        Kind::ShorthandPropertyAssignment => {
            factory
                .store_mut()
                .as_shorthand_property_assignment_mut(node)
                .modifiers = modifiers;
        }
        Kind::VariableStatement => {
            factory
                .store_mut()
                .as_variable_statement_mut(node)
                .modifiers = modifiers;
        }
        _ => panic!(
            "Node that does not have modifiers tried to have modifier replaced: {:?}",
            factory.store().kind(node)
        ),
    }
    node
}

pub type PragmaKindFlags = u8;

pub const PRAGMA_KIND_TRIPLE_SLASH_XML: PragmaKindFlags = 1 << 0;
pub const PRAGMA_KIND_SINGLE_LINE: PragmaKindFlags = 1 << 1;
pub const PRAGMA_KIND_MULTI_LINE: PragmaKindFlags = 1 << 2;
pub const PRAGMA_KIND_FLAGS_NONE: PragmaKindFlags = 0;
pub const PRAGMA_KIND_ALL: PragmaKindFlags =
    PRAGMA_KIND_TRIPLE_SLASH_XML | PRAGMA_KIND_SINGLE_LINE | PRAGMA_KIND_MULTI_LINE;
pub const PRAGMA_KIND_DEFAULT: PragmaKindFlags = PRAGMA_KIND_ALL;

pub struct PragmaArgumentSpecification {
    pub name: String,
    pub optional: bool,
    pub capture_span: bool,
}

pub struct PragmaSpecification {
    pub args: Vec<PragmaArgumentSpecification>,
    pub kind: PragmaKindFlags,
}

impl PragmaSpecification {
    pub fn is_triple_slash(&self) -> bool {
        (self.kind & PRAGMA_KIND_TRIPLE_SLASH_XML) > 0
    }
}

// Hand-written visitor implementations for nodes with runtime-dependent
// child ordering. Generated code in ast_generated.go delegates to these.

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_options(path: &str) -> SourceFileParseOptions {
        SourceFileParseOptions {
            file_name: path.to_string(),
            path: path.to_string(),
            ..Default::default()
        }
    }

    fn empty_node_list(factory: &mut NodeFactory) -> NodeList {
        factory.new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            std::iter::empty::<Node>(),
        )
    }

    fn new_named_function(factory: &mut NodeFactory, name: &str, loc: core::TextRange) -> Node {
        let name_node = factory.new_identifier(name);
        let parameters = empty_node_list(factory);
        let function = factory.new_function_declaration(
            None::<ModifierList>,
            None::<Node>,
            Some(name_node),
            None::<NodeList>,
            parameters,
            None::<Node>,
            None::<Node>,
            None::<Node>,
        );
        factory.set_loc(function, loc);
        factory.set_loc(
            name_node,
            core::TextRange::new(loc.pos() + 9, loc.pos() + 9 + name.len() as i32),
        );
        function
    }

    fn new_named_function_with_parameter(
        factory: &mut NodeFactory,
        name: &str,
        parameter_name: &str,
        loc: core::TextRange,
    ) -> (Node, Node) {
        let name_node = factory.new_identifier(name);
        let parameter_name_node = factory.new_identifier(parameter_name);
        let parameter = factory.new_parameter_declaration(
            None::<ModifierList>,
            None::<Node>,
            Some(parameter_name_node),
            None::<Node>,
            None::<Node>,
            None::<Node>,
        );
        let parameters = factory.new_node_list(
            core::TextRange::new(loc.pos() + 10, loc.pos() + 20),
            core::TextRange::new(loc.pos() + 10, loc.pos() + 20),
            [parameter],
        );
        let function = factory.new_function_declaration(
            None::<ModifierList>,
            None::<Node>,
            Some(name_node),
            None::<NodeList>,
            parameters,
            None::<Node>,
            None::<Node>,
            None::<Node>,
        );
        factory.set_loc(function, loc);
        factory.set_loc(
            name_node,
            core::TextRange::new(loc.pos() + 9, loc.pos() + 9 + name.len() as i32),
        );
        factory.set_loc(
            parameter,
            core::TextRange::new(
                loc.pos() + 10 + name.len() as i32,
                loc.pos() + 10 + name.len() as i32 + parameter_name.len() as i32,
            ),
        );
        factory.set_loc(parameter_name_node, factory.store().loc(parameter));
        (function, parameter)
    }

    fn new_source_file_with_functions(
        factory: &mut NodeFactory,
        path: &str,
        functions: impl IntoIterator<Item = Node>,
    ) -> Node {
        let statements = factory.new_node_list(
            core::TextRange::new(0, 100),
            core::TextRange::new(0, 100),
            functions,
        );
        let end_of_file = factory.new_token(Kind::EndOfFile);
        let source_file =
            factory.new_source_file(parse_options(path), "", statements, Some(end_of_file));
        factory.set_loc(end_of_file, core::TextRange::new(100, 100));
        source_file
    }

    #[test]
    fn clone_recorder_capacity_should_stay_conservative_for_large_sparse_sources() {
        assert_eq!(clone_recorder_capacity(32, None), 33);
        assert_eq!(clone_recorder_capacity(32, Some(1)), 33);
        assert_eq!(clone_recorder_capacity(20_000, None), 0);
        assert_eq!(clone_recorder_capacity(20_000, Some(2)), 129);
        assert_eq!(clone_recorder_capacity(20_000, Some(1_000)), 20_001);
    }

    #[test]
    fn source_file_rejects_nodes_from_foreign_store() {
        let mut source_factory = NodeFactory::default();
        let expression = source_factory.new_identifier("value");
        let statement = source_factory.new_expression_statement(expression);
        let source_statements = source_factory.new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![statement],
        );
        let source_root = source_factory.new_source_file(
            parse_options("/resolver.ts"),
            "",
            source_statements,
            None,
        );
        let source_file = source_factory
            .finish_parsed_source_file(source_root, ParsedSourceFileMetadata::default());

        let mut output_factory = NodeFactory::default();
        let mut importer = AstImportState::new();
        let imported_statement =
            importer.preserve_node(source_file.store(), &mut output_factory, statement);
        let output_statements = output_factory.new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![imported_statement],
        );
        let output_root = output_factory.new_source_file(
            parse_options("/resolver.ts"),
            "",
            output_statements,
            None,
        );
        let output_file = output_factory.finish_transformed_source_file(output_root);

        assert_eq!(output_root.store_id(), output_file.store().store_id());
        assert_eq!(
            imported_statement.store_id(),
            output_file.store().store_id()
        );

        let view = SourceFileView::from_source_file(&output_file);
        assert_eq!(view.store().store_id(), output_file.store().store_id());
        let unrelated = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            output_file.store().kind(statement)
        }));
        assert!(unrelated.is_err());
    }

    #[test]
    fn source_file_view_typed_child_helpers_cast_statements_and_eof() {
        let mut factory = NodeFactory::default();
        let expression = factory.new_identifier("value");
        let statement = factory.new_expression_statement(expression);
        let statements = factory.new_node_list(
            core::TextRange::new(0, 12),
            core::TextRange::new(0, 12),
            [statement],
        );
        let end_of_file = factory.new_token(Kind::EndOfFile);
        let root = factory.new_source_file(
            parse_options("/source.ts"),
            "value;",
            statements,
            Some(end_of_file),
        );
        let file = factory.finish_parsed_source_file(root, ParsedSourceFileMetadata::default());
        let view = file.source_file_view();

        let statement_nodes = view
            .statements::<AstStatementView>()
            .map(|child| child.node())
            .collect::<Vec<_>>();
        assert_eq!(statement_nodes, vec![statement]);

        let found_statement = view
            .find_statements::<ExpressionStatementView>(|child| {
                child.kind() == Kind::ExpressionStatement
            })
            .expect("expected expression statement");
        assert_eq!(found_statement.node(), statement);

        assert_eq!(view.end_of_file_token_node(), Some(end_of_file));
        let eof = view
            .end_of_file_token_view::<AstTokenView>()
            .expect("expected EOF token");
        assert_eq!(eof.node(), end_of_file);
    }

    #[test]
    fn traversal_preserved_lists_match_cross_store_outputs() {
        let mut source_factory = NodeFactory::default();
        let expression = source_factory.new_identifier("value");
        let statement = source_factory.new_expression_statement(expression);
        let source_list = source_factory.new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![statement],
        );

        let mut output_factory = NodeFactory::default();
        let mut traversal = AstTraversalState::new();
        let output_list = traversal.preserve_node_list(
            source_factory.store(),
            &mut output_factory,
            source_list.id(),
        );

        assert!(traversal.preserved_source_node_list_matches(
            source_factory.store(),
            &output_factory,
            Some(source_list.id()),
            Some(output_list)
        ));
    }

    fn child_descriptor<'a>(layout: &'a AstNodeLayout, name: &str) -> &'a AstChildFieldDescriptor {
        layout
            .child_fields
            .iter()
            .find(|field| field.name == name)
            .expect("generated layout should contain child field")
    }

    #[test]
    fn generated_layout_descriptors_capture_source_file_traversal_roles() {
        let layout = ast_node_layout_for_payload_tag(NodePayloadTag::SourceFile)
            .expect("SourceFile should have a generated layout");

        assert_eq!(layout.kinds, &[Kind::SourceFile]);
        assert_eq!(layout.child_fields.len(), 2);
        assert_eq!(
            child_descriptor(layout, "statements"),
            &AstChildFieldDescriptor {
                id: AstChildFieldId::Statements,
                name: "statements",
                kind: AstChildFieldKind::NodeList,
                visitor_role: AstVisitorRole::Nodes,
                visit_route: AstVisitRoute::VisitTopLevelStatements,
            }
        );
        assert_eq!(
            child_descriptor(layout, "end_of_file_token"),
            &AstChildFieldDescriptor {
                id: AstChildFieldId::EndOfFileToken,
                name: "end_of_file_token",
                kind: AstChildFieldKind::OptionalNode,
                visitor_role: AstVisitorRole::Node,
                visit_route: AstVisitRoute::VisitToken,
            }
        );
    }

    #[test]
    fn generated_layout_descriptors_capture_special_traversal_roles() {
        let function_layout = ast_node_layout_for_payload_tag(NodePayloadTag::FunctionDeclaration)
            .expect("FunctionDeclaration should have a generated layout");
        assert_eq!(
            child_descriptor(function_layout, "parameters").visit_route,
            AstVisitRoute::VisitParameters
        );
        assert_eq!(
            child_descriptor(function_layout, "body").visit_route,
            AstVisitRoute::VisitFunctionBody
        );

        let labeled_layout = ast_node_layout_for_payload_tag(NodePayloadTag::LabeledStatement)
            .expect("LabeledStatement should have a generated layout");
        assert_eq!(
            child_descriptor(labeled_layout, "statement").visit_route,
            AstVisitRoute::VisitEmbeddedStatement
        );

        let syntax_list_layout = ast_node_layout_for_payload_tag(NodePayloadTag::SyntaxList)
            .expect("SyntaxList should have a generated layout");
        assert_eq!(
            child_descriptor(syntax_list_layout, "children"),
            &AstChildFieldDescriptor {
                id: AstChildFieldId::Children,
                name: "children",
                kind: AstChildFieldKind::RawNodeSlice,
                visitor_role: AstVisitorRole::Node,
                visit_route: AstVisitRoute::VisitNode,
            }
        );
    }

    #[test]
    fn stable_node_ids_should_include_source_backed_nodes() {
        let mut factory = NodeFactory::default();
        let function_name = factory.new_identifier("f");
        let parameter_name = factory.new_identifier("arg");
        let parameter = factory.new_parameter_declaration(
            None::<ModifierList>,
            None::<Node>,
            Some(parameter_name),
            None::<Node>,
            None::<Node>,
            None::<Node>,
        );
        let parameters = factory.new_node_list(
            core::TextRange::new(11, 14),
            core::TextRange::new(11, 14),
            [parameter],
        );
        let expression = factory.new_identifier("value");
        let statement = factory.new_expression_statement(expression);
        let body_statements = factory.new_node_list(
            core::TextRange::new(17, 22),
            core::TextRange::new(17, 22),
            [statement],
        );
        let body = factory.new_block(body_statements, false);
        let function = factory.new_function_declaration(
            None::<ModifierList>,
            None::<Node>,
            Some(function_name),
            None::<NodeList>,
            parameters,
            None::<Node>,
            None::<Node>,
            Some(body),
        );
        let statements = factory.new_node_list(
            core::TextRange::new(0, 24),
            core::TextRange::new(0, 24),
            [function],
        );
        let end_of_file = factory.new_token(Kind::EndOfFile);
        let source_file = factory.new_source_file(
            parse_options("/stable.ts"),
            "function f(arg) { value }",
            statements,
            Some(end_of_file),
        );

        factory.set_loc(function, core::TextRange::new(0, 24));
        factory.set_loc(function_name, core::TextRange::new(9, 10));
        factory.set_loc(parameter, core::TextRange::new(11, 14));
        factory.set_loc(parameter_name, core::TextRange::new(11, 14));
        factory.set_loc(body, core::TextRange::new(16, 24));
        factory.set_loc(statement, core::TextRange::new(18, 23));
        factory.set_loc(expression, core::TextRange::new(18, 23));
        factory.set_loc(end_of_file, core::TextRange::new(24, 24));

        let source_id = SourceId::from_u32(7);
        let stable_ids = factory
            .store()
            .build_stable_node_ids(source_file, source_id);
        let local = |node| {
            stable_ids
                .local_id(node)
                .expect("node should have a stable local id")
                .as_u32()
        };

        assert_eq!(stable_ids.source_id(), source_id);
        assert_eq!(
            stable_ids.source_snapshot_id(),
            SourceSnapshotId::new(source_id, 0)
        );
        assert_eq!(stable_ids.source_hash(), 0);
        assert!(stable_ids.is_current_for_source_snapshot(SourceSnapshotId::new(source_id, 0)));
        assert_eq!(stable_ids.root(), source_file);
        assert_eq!(stable_ids.iter().count(), stable_ids.len());
        assert_eq!(
            stable_ids.stable_id(source_file),
            Some(StableNodeId::new(source_id, LocalAstId::from_u32(0)))
        );
        assert_eq!(
            stable_ids.node_for_local_id(LocalAstId::from_u32(0)),
            Some(source_file)
        );
        assert!(stable_ids.contains_node(function));
        assert!(stable_ids.contains_node(function_name));
        assert!(stable_ids.contains_node(parameter));
        assert!(stable_ids.contains_node(parameter_name));
        assert!(stable_ids.contains_node(body));
        assert!(stable_ids.contains_node(statement));
        assert!(stable_ids.contains_node(expression));
        assert_ne!(local(function), local(function_name));
        assert_eq!(stable_ids.stable_id(end_of_file), None);
    }

    #[test]
    fn stable_node_ids_should_keep_named_declaration_id_when_node_is_inserted_before_it() {
        let source_id = SourceId::from_u32(11);

        let mut old_factory = NodeFactory::default();
        let old_keep = new_named_function(&mut old_factory, "keep", core::TextRange::new(20, 35));
        let old_source_file =
            new_source_file_with_functions(&mut old_factory, "/stable.ts", [old_keep]);
        let old_stable_ids = old_factory
            .store()
            .build_stable_node_ids(old_source_file, source_id);

        let mut new_factory = NodeFactory::default();
        let new_added = new_named_function(&mut new_factory, "added", core::TextRange::new(0, 18));
        let new_keep = new_named_function(&mut new_factory, "keep", core::TextRange::new(40, 55));
        let new_source_file =
            new_source_file_with_functions(&mut new_factory, "/stable.ts", [new_added, new_keep]);
        let new_stable_ids = new_factory
            .store()
            .build_stable_node_ids(new_source_file, source_id);

        assert_eq!(
            old_stable_ids.local_id(old_keep),
            new_stable_ids.local_id(new_keep)
        );
    }

    #[test]
    fn stable_node_ids_should_keep_same_named_child_id_when_same_name_is_inserted_in_other_container()
     {
        let source_id = SourceId::from_u32(12);

        let mut old_factory = NodeFactory::default();
        let (old_left, old_left_parameter) = new_named_function_with_parameter(
            &mut old_factory,
            "left",
            "value",
            core::TextRange::new(0, 20),
        );
        let (old_right, old_right_parameter) = new_named_function_with_parameter(
            &mut old_factory,
            "right",
            "value",
            core::TextRange::new(25, 50),
        );
        let old_source_file =
            new_source_file_with_functions(&mut old_factory, "/stable.ts", [old_left, old_right]);
        let old_stable_ids = old_factory
            .store()
            .build_stable_node_ids(old_source_file, source_id);

        let mut new_factory = NodeFactory::default();
        let (new_added, _new_added_parameter) = new_named_function_with_parameter(
            &mut new_factory,
            "added",
            "value",
            core::TextRange::new(0, 20),
        );
        let (new_left, new_left_parameter) = new_named_function_with_parameter(
            &mut new_factory,
            "left",
            "value",
            core::TextRange::new(25, 45),
        );
        let (new_right, new_right_parameter) = new_named_function_with_parameter(
            &mut new_factory,
            "right",
            "value",
            core::TextRange::new(50, 75),
        );
        let new_source_file = new_source_file_with_functions(
            &mut new_factory,
            "/stable.ts",
            [new_added, new_left, new_right],
        );
        let new_stable_ids = new_factory
            .store()
            .build_stable_node_ids(new_source_file, source_id);

        assert_ne!(
            old_stable_ids.local_id(old_left_parameter),
            old_stable_ids.local_id(old_right_parameter)
        );
        assert_eq!(
            old_stable_ids.local_id(old_left_parameter),
            new_stable_ids.local_id(new_left_parameter)
        );
        assert_eq!(
            old_stable_ids.local_id(old_right_parameter),
            new_stable_ids.local_id(new_right_parameter)
        );
    }

    #[test]
    fn stable_node_ids_should_skip_synthetic_source_children() {
        let mut factory = NodeFactory::default();
        let synthetic_expression = factory.new_identifier("synthetic");
        let statement = factory.new_expression_statement(synthetic_expression);
        let statements = factory.new_node_list(
            core::TextRange::new(0, 9),
            core::TextRange::new(0, 9),
            [statement],
        );
        let end_of_file = factory.new_token(Kind::EndOfFile);
        let source_file = factory.new_source_file(
            parse_options("/synthetic.ts"),
            "synthetic",
            statements,
            Some(end_of_file),
        );

        factory.set_loc(statement, core::TextRange::new(0, 9));
        factory.set_loc(end_of_file, core::TextRange::new(9, 9));

        let stable_ids = factory
            .store()
            .build_stable_node_ids(source_file, SourceId::from_u32(8));

        assert!(stable_ids.contains_node(source_file));
        assert!(stable_ids.contains_node(statement));
        assert!(!stable_ids.contains_node(synthetic_expression));
        assert!(!stable_ids.contains_node(end_of_file));
    }

    #[test]
    fn set_parent_in_children_should_use_descriptor_child_fields() {
        let mut factory = NodeFactory::default();
        let function_name = factory.new_identifier("f");
        let parameter_name = factory.new_identifier("arg");
        let parameter = factory.new_parameter_declaration(
            None::<ModifierList>,
            None::<Node>,
            Some(parameter_name),
            None::<Node>,
            None::<Node>,
            None::<Node>,
        );
        let parameters = factory.new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            [parameter],
        );
        let body_statements = empty_node_list(&mut factory);
        let body = factory.new_block(body_statements, false);
        let function = factory.new_function_declaration(
            None::<ModifierList>,
            None::<Node>,
            Some(function_name),
            None::<NodeList>,
            parameters,
            None::<Node>,
            None::<Node>,
            Some(body),
        );

        factory.store_mut().set_parent_in_children(function);

        assert_eq!(factory.store().parent(function_name), Some(function));
        assert_eq!(factory.store().parent(parameter), Some(function));
        assert_eq!(factory.store().parent(body), Some(function));
        assert_eq!(factory.store().parent(parameter_name), None);
    }

    #[test]
    fn set_parent_recursive_should_use_descriptor_child_fields() {
        let mut factory = NodeFactory::default();
        let function_name = factory.new_identifier("f");
        let parameter_name = factory.new_identifier("arg");
        let parameter = factory.new_parameter_declaration(
            None::<ModifierList>,
            None::<Node>,
            Some(parameter_name),
            None::<Node>,
            None::<Node>,
            None::<Node>,
        );
        let parameters = factory.new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            [parameter],
        );
        let expression = factory.new_identifier("value");
        let statement = factory.new_expression_statement(Some(expression));
        let body_statements = factory.new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            [statement],
        );
        let body = factory.new_block(body_statements, false);
        let function = factory.new_function_declaration(
            None::<ModifierList>,
            None::<Node>,
            Some(function_name),
            None::<NodeList>,
            parameters,
            None::<Node>,
            None::<Node>,
            Some(body),
        );

        factory.store_mut().set_parent_recursive(function);

        assert_eq!(factory.store().parent(function_name), Some(function));
        assert_eq!(factory.store().parent(parameter), Some(function));
        assert_eq!(factory.store().parent(parameter_name), Some(parameter));
        assert_eq!(factory.store().parent(body), Some(function));
        assert_eq!(factory.store().parent(statement), Some(body));
        assert_eq!(factory.store().parent(expression), Some(statement));
    }

    #[test]
    fn child_parent_issues_should_be_empty_for_descriptor_wired_tree() {
        let mut factory = NodeFactory::default();
        let function_name = factory.new_identifier("f");
        let parameter_name = factory.new_identifier("arg");
        let parameter = factory.new_parameter_declaration(
            None::<ModifierList>,
            None::<Node>,
            Some(parameter_name),
            None::<Node>,
            None::<Node>,
            None::<Node>,
        );
        let parameters = factory.new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            [parameter],
        );
        let body_statements = empty_node_list(&mut factory);
        let body = factory.new_block(body_statements, false);
        let function = factory.new_function_declaration(
            None::<ModifierList>,
            None::<Node>,
            Some(function_name),
            None::<NodeList>,
            parameters,
            None::<Node>,
            None::<Node>,
            Some(body),
        );

        factory.store_mut().set_parent_recursive(function);

        assert!(factory.store().child_parent_issues(function).is_empty());
    }

    #[test]
    fn child_parent_issues_should_report_missing_original_parent() {
        let mut factory = NodeFactory::default();
        let function_name = factory.new_identifier("f");
        let body_statements = empty_node_list(&mut factory);
        let body = factory.new_block(body_statements, false);
        let parameters = empty_node_list(&mut factory);
        let function = factory.new_function_declaration(
            None::<ModifierList>,
            None::<Node>,
            Some(function_name),
            None::<NodeList>,
            parameters,
            None::<Node>,
            None::<Node>,
            Some(body),
        );

        let issue = factory
            .store()
            .first_child_parent_issue(function)
            .expect("function name should be missing its original parent");

        assert_eq!(issue.parent(), function);
        assert_eq!(issue.child(), function_name);
        assert_eq!(issue.field_name(), "name");
        assert_eq!(issue.child_span_kind(), AstChildSourceSpanKind::Node);
        assert_eq!(issue.index(), None);
        assert_eq!(issue.kind(), AstChildParentIssueKind::MissingOriginalParent);
    }

    #[test]
    fn child_parent_issues_should_report_wrong_original_parent() {
        let mut factory = NodeFactory::default();
        let function_name = factory.new_identifier("f");
        let parameter_name = factory.new_identifier("arg");
        let parameter = factory.new_parameter_declaration(
            None::<ModifierList>,
            None::<Node>,
            Some(parameter_name),
            None::<Node>,
            None::<Node>,
            None::<Node>,
        );
        let parameters = factory.new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            [parameter],
        );
        let body_statements = empty_node_list(&mut factory);
        let body = factory.new_block(body_statements, false);
        let function = factory.new_function_declaration(
            None::<ModifierList>,
            None::<Node>,
            Some(function_name),
            None::<NodeList>,
            parameters,
            None::<Node>,
            None::<Node>,
            Some(body),
        );

        factory.store_mut().set_parent_recursive(function);
        factory.store_mut().set_parent(parameter, Some(body));

        let issue = factory
            .store()
            .first_child_parent_issue(function)
            .expect("parameter should have the wrong original parent");

        assert_eq!(issue.parent(), function);
        assert_eq!(issue.child(), parameter);
        assert_eq!(issue.field_name(), "parameters");
        assert_eq!(
            issue.child_span_kind(),
            AstChildSourceSpanKind::NodeListElement
        );
        assert_eq!(issue.index(), Some(0));
        assert_eq!(
            issue.kind(),
            AstChildParentIssueKind::WrongOriginalParent {
                actual_parent: body
            }
        );
    }

    #[test]
    fn deep_clone_should_use_descriptor_child_fields() {
        let mut source_factory = NodeFactory::default();
        let function_name = source_factory.new_identifier("f");
        let parameter_name = source_factory.new_identifier("arg");
        let parameter = source_factory.new_parameter_declaration(
            None::<ModifierList>,
            None::<Node>,
            Some(parameter_name),
            None::<Node>,
            None::<Node>,
            None::<Node>,
        );
        let parameters = source_factory.new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            [parameter],
        );
        let body_statements = empty_node_list(&mut source_factory);
        let body = source_factory.new_block(body_statements, false);
        let function = source_factory.new_function_declaration(
            None::<ModifierList>,
            None::<Node>,
            Some(function_name),
            None::<NodeList>,
            parameters,
            None::<Node>,
            None::<Node>,
            Some(body),
        );

        let mut target_factory = NodeFactory::default();
        let cloned = target_factory
            .deep_clone_node_from_store_preserve_location(source_factory.store(), function);
        let target = target_factory.store();
        let cloned_name = target.name(cloned).expect("function name should be cloned");
        let cloned_parameters = target
            .parameters(cloned)
            .expect("function parameters should be cloned");
        let cloned_parameter = cloned_parameters
            .iter()
            .next()
            .expect("parameter should be cloned");
        let cloned_parameter_name = target
            .name(cloned_parameter)
            .expect("parameter name should be cloned");
        let cloned_body = target.body(cloned).expect("function body should be cloned");

        assert_ne!(cloned, function);
        assert_eq!(cloned.store_id(), target.store_id());
        assert_eq!(cloned_name.store_id(), target.store_id());
        assert_eq!(cloned_parameter.store_id(), target.store_id());
        assert_eq!(cloned_parameter_name.store_id(), target.store_id());
        assert_eq!(cloned_body.store_id(), target.store_id());
        assert_eq!(target.text(cloned_name), "f");
        assert_eq!(target.text(cloned_parameter_name), "arg");
    }

    #[test]
    fn update_for_statement_should_use_descriptor_child_fields_for_noop_comparison() {
        let mut factory = NodeFactory::default();
        let initializer = factory.new_identifier("i");
        let condition = factory.new_identifier("condition");
        let incrementor = factory.new_identifier("next");
        let statement_expression = factory.new_identifier("body");
        let statement = factory.new_expression_statement(statement_expression);
        let for_statement = factory.new_for_statement(
            Some(initializer),
            Some(condition),
            Some(incrementor),
            Some(statement),
        );

        let unchanged = factory.update_for_statement(
            for_statement,
            Some(initializer),
            Some(condition),
            Some(incrementor),
            Some(statement),
        );

        assert_eq!(unchanged, for_statement);

        let replacement_initializer = factory.new_identifier("replacement");
        let changed = factory.update_for_statement(
            for_statement,
            Some(replacement_initializer),
            Some(condition),
            Some(incrementor),
            Some(statement),
        );

        assert_ne!(changed, for_statement);
        assert_eq!(
            factory.store().initializer(changed),
            Some(replacement_initializer)
        );
    }

    #[test]
    fn debug_tree_should_use_descriptor_child_fields() {
        let mut factory = NodeFactory::default();
        let initializer = factory.new_identifier("i");
        let condition = factory.new_identifier("condition");
        let incrementor = factory.new_identifier("next");
        let statement_expression = factory.new_identifier("body");
        let statement = factory.new_expression_statement(statement_expression);
        let for_statement = factory.new_for_statement(
            Some(initializer),
            Some(condition),
            Some(incrementor),
            Some(statement),
        );

        assert_eq!(
            factory.store().debug_tree(for_statement),
            "\
ForStatement
  initializer:
    Identifier
  condition:
    Identifier
  incrementor:
    Identifier
  statement:
    ExpressionStatement
      expression:
        Identifier
"
        );
    }

    #[test]
    fn debug_tree_should_preserve_raw_node_slice_empty_slots() {
        let mut factory = NodeFactory::default();
        let first = factory.new_identifier("first");
        let second = factory.new_identifier("second");
        let children = factory.new_raw_node_slice([Some(first), None, Some(second)]);
        let syntax_list = factory.new_syntax_list_from_raw(children);

        assert_eq!(
            factory.store().debug_tree(syntax_list),
            "\
SyntaxList
  children:
    [0]
      Identifier
    [1]: <none>
    [2]
      Identifier
"
        );
    }

    #[test]
    fn debug_tree_with_stable_node_ids_should_label_source_backed_nodes() {
        let mut factory = NodeFactory::default();
        let expression = factory.new_identifier("value");
        let statement = factory.new_expression_statement(expression);
        let statements = factory.new_node_list(
            core::TextRange::new(0, 5),
            core::TextRange::new(0, 5),
            [statement],
        );
        let end_of_file = factory.new_token(Kind::EndOfFile);
        let source_file = factory.new_source_file(
            parse_options("/stable.ts"),
            "value",
            statements,
            Some(end_of_file),
        );
        factory.set_loc(statement, core::TextRange::new(0, 5));
        factory.set_loc(expression, core::TextRange::new(0, 5));
        factory.set_loc(end_of_file, core::TextRange::new(5, 5));

        let stable_ids = factory
            .store()
            .build_stable_node_ids(source_file, SourceId::from_u32(9));
        let statement_id = stable_ids
            .stable_id(statement)
            .expect("statement should have a stable id");
        let expression_id = stable_ids
            .stable_id(expression)
            .expect("expression should have a stable id");

        assert_eq!(
            factory
                .store()
                .debug_tree_with_stable_node_ids(source_file, &stable_ids),
            format!(
                "\
SourceFile stable=9:0
  statements:
    [0]
      ExpressionStatement stable={statement_id}
        expression:
          Identifier stable={expression_id}
  end_of_file_token:
    EndOfFile
"
            )
        );
    }

    #[test]
    fn child_source_spans_should_use_descriptor_child_fields() {
        let mut factory = NodeFactory::default();
        let initializer = factory.new_identifier("i");
        let condition = factory.new_identifier("condition");
        let incrementor = factory.new_identifier("next");
        let statement_expression = factory.new_identifier("body");
        let statement = factory.new_expression_statement(statement_expression);
        let for_statement = factory.new_for_statement(
            Some(initializer),
            Some(condition),
            Some(incrementor),
            Some(statement),
        );
        factory.set_loc(initializer, core::TextRange::new(1, 2));
        factory.set_loc(condition, core::TextRange::new(3, 4));
        factory.set_loc(incrementor, core::TextRange::new(5, 6));
        factory.set_loc(statement, core::TextRange::new(7, 10));

        assert_eq!(
            factory.store().child_source_spans(for_statement),
            vec![
                AstChildSourceSpan {
                    field_id: AstChildFieldId::Initializer,
                    field_name: "initializer",
                    kind: AstChildSourceSpanKind::Node,
                    index: None,
                    node: Some(initializer),
                    loc: Some(core::TextRange::new(1, 2)),
                    range: Some(core::TextRange::new(1, 2)),
                },
                AstChildSourceSpan {
                    field_id: AstChildFieldId::Condition,
                    field_name: "condition",
                    kind: AstChildSourceSpanKind::Node,
                    index: None,
                    node: Some(condition),
                    loc: Some(core::TextRange::new(3, 4)),
                    range: Some(core::TextRange::new(3, 4)),
                },
                AstChildSourceSpan {
                    field_id: AstChildFieldId::Incrementor,
                    field_name: "incrementor",
                    kind: AstChildSourceSpanKind::Node,
                    index: None,
                    node: Some(incrementor),
                    loc: Some(core::TextRange::new(5, 6)),
                    range: Some(core::TextRange::new(5, 6)),
                },
                AstChildSourceSpan {
                    field_id: AstChildFieldId::Statement,
                    field_name: "statement",
                    kind: AstChildSourceSpanKind::Node,
                    index: None,
                    node: Some(statement),
                    loc: Some(core::TextRange::new(7, 10)),
                    range: Some(core::TextRange::new(7, 10)),
                },
            ],
        );
    }

    #[test]
    fn child_source_spans_should_report_list_ranges() {
        let mut factory = NodeFactory::default();
        let function_name = factory.new_identifier("f");
        let parameter_name = factory.new_identifier("arg");
        let parameter = factory.new_parameter_declaration(
            None::<ModifierList>,
            None::<Node>,
            Some(parameter_name),
            None::<Node>,
            None::<Node>,
            None::<Node>,
        );
        let parameters = factory.new_node_list(
            core::TextRange::new(10, 20),
            core::TextRange::new(11, 19),
            [parameter],
        );
        let body_statements = empty_node_list(&mut factory);
        let body = factory.new_block(body_statements, false);
        let function = factory.new_function_declaration(
            None::<ModifierList>,
            None::<Node>,
            Some(function_name),
            None::<NodeList>,
            parameters,
            None::<Node>,
            None::<Node>,
            Some(body),
        );
        factory.set_loc(function_name, core::TextRange::new(1, 2));
        factory.set_loc(body, core::TextRange::new(21, 30));

        assert_eq!(
            factory.store().child_source_spans(function),
            vec![
                AstChildSourceSpan {
                    field_id: AstChildFieldId::Name,
                    field_name: "name",
                    kind: AstChildSourceSpanKind::Node,
                    index: None,
                    node: Some(function_name),
                    loc: Some(core::TextRange::new(1, 2)),
                    range: Some(core::TextRange::new(1, 2)),
                },
                AstChildSourceSpan {
                    field_id: AstChildFieldId::Parameters,
                    field_name: "parameters",
                    kind: AstChildSourceSpanKind::NodeList,
                    index: None,
                    node: None,
                    loc: Some(core::TextRange::new(10, 20)),
                    range: Some(core::TextRange::new(11, 19)),
                },
                AstChildSourceSpan {
                    field_id: AstChildFieldId::Body,
                    field_name: "body",
                    kind: AstChildSourceSpanKind::Node,
                    index: None,
                    node: Some(body),
                    loc: Some(core::TextRange::new(21, 30)),
                    range: Some(core::TextRange::new(21, 30)),
                },
            ],
        );
    }

    #[test]
    fn child_node_source_spans_should_report_list_elements() {
        let mut factory = NodeFactory::default();
        let function_name = factory.new_identifier("f");
        let parameter_name = factory.new_identifier("arg");
        let parameter = factory.new_parameter_declaration(
            None::<ModifierList>,
            None::<Node>,
            Some(parameter_name),
            None::<Node>,
            None::<Node>,
            None::<Node>,
        );
        let parameters = factory.new_node_list(
            core::TextRange::new(10, 20),
            core::TextRange::new(11, 19),
            [parameter],
        );
        let body_statements = empty_node_list(&mut factory);
        let body = factory.new_block(body_statements, false);
        let function = factory.new_function_declaration(
            None::<ModifierList>,
            None::<Node>,
            Some(function_name),
            None::<NodeList>,
            parameters,
            None::<Node>,
            None::<Node>,
            Some(body),
        );
        factory.set_loc(function_name, core::TextRange::new(1, 2));
        factory.set_loc(parameter, core::TextRange::new(12, 18));
        factory.set_loc(body, core::TextRange::new(21, 30));

        let mut spans = Vec::new();
        let result = factory
            .store()
            .for_each_child_node_source_span(function, |span| {
                spans.push(span);
                ControlFlow::Continue(())
            });

        assert_eq!(result, ControlFlow::Continue(()));
        assert_eq!(
            spans,
            vec![
                AstChildSourceSpan {
                    field_id: AstChildFieldId::Name,
                    field_name: "name",
                    kind: AstChildSourceSpanKind::Node,
                    index: None,
                    node: Some(function_name),
                    loc: Some(core::TextRange::new(1, 2)),
                    range: Some(core::TextRange::new(1, 2)),
                },
                AstChildSourceSpan {
                    field_id: AstChildFieldId::Parameters,
                    field_name: "parameters",
                    kind: AstChildSourceSpanKind::NodeListElement,
                    index: Some(0),
                    node: Some(parameter),
                    loc: Some(core::TextRange::new(12, 18)),
                    range: Some(core::TextRange::new(12, 18)),
                },
                AstChildSourceSpan {
                    field_id: AstChildFieldId::Body,
                    field_name: "body",
                    kind: AstChildSourceSpanKind::Node,
                    index: None,
                    node: Some(body),
                    loc: Some(core::TextRange::new(21, 30)),
                    range: Some(core::TextRange::new(21, 30)),
                },
            ],
        );
    }

    #[test]
    fn for_each_present_child_should_match_child_node_source_spans() {
        let mut factory = NodeFactory::default();
        let first = factory.new_identifier("first");
        let second = factory.new_identifier("second");
        let children = factory.new_raw_node_slice([Some(first), None, Some(second)]);
        let syntax_list = factory.new_syntax_list_from_raw(children);

        let mut present_children = Vec::new();
        let result = factory
            .store()
            .for_each_present_child(syntax_list, |child| {
                present_children.push(child);
                ControlFlow::Continue(())
            });

        let mut span_children = Vec::new();
        let span_result = factory
            .store()
            .for_each_child_node_source_span(syntax_list, |span| {
                if let Some(child) = span.node() {
                    span_children.push(child);
                }
                ControlFlow::Continue(())
            });

        assert_eq!(result, ControlFlow::Continue(()));
        assert_eq!(span_result, ControlFlow::Continue(()));
        assert_eq!(present_children, vec![first, second]);
        assert_eq!(present_children, span_children);
    }

    #[test]
    fn child_source_spans_should_preserve_raw_node_slice_empty_slots() {
        let mut factory = NodeFactory::default();
        let first = factory.new_identifier("first");
        let second = factory.new_identifier("second");
        let children = factory.new_raw_node_slice([Some(first), None, Some(second)]);
        let syntax_list = factory.new_syntax_list_from_raw(children);
        factory.set_loc(first, core::TextRange::new(1, 2));
        factory.set_loc(second, core::TextRange::new(3, 4));

        assert_eq!(
            factory.store().child_source_spans(syntax_list),
            vec![
                AstChildSourceSpan {
                    field_id: AstChildFieldId::Children,
                    field_name: "children",
                    kind: AstChildSourceSpanKind::RawNodeSliceElement,
                    index: Some(0),
                    node: Some(first),
                    loc: Some(core::TextRange::new(1, 2)),
                    range: Some(core::TextRange::new(1, 2)),
                },
                AstChildSourceSpan {
                    field_id: AstChildFieldId::Children,
                    field_name: "children",
                    kind: AstChildSourceSpanKind::RawNodeSliceElement,
                    index: Some(1),
                    node: None,
                    loc: None,
                    range: None,
                },
                AstChildSourceSpan {
                    field_id: AstChildFieldId::Children,
                    field_name: "children",
                    kind: AstChildSourceSpanKind::RawNodeSliceElement,
                    index: Some(2),
                    node: Some(second),
                    loc: Some(core::TextRange::new(3, 4)),
                    range: Some(core::TextRange::new(3, 4)),
                },
            ],
        );
    }

    #[test]
    fn set_parent_in_children_should_ignore_empty_raw_slice_slots() {
        let mut factory = NodeFactory::default();
        let first = factory.new_identifier("first");
        let second = factory.new_identifier("second");
        let children = factory.new_raw_node_slice([Some(first), None, Some(second)]);
        let syntax_list = factory.new_syntax_list_from_raw(children);

        factory.store_mut().set_parent_in_children(syntax_list);

        assert_eq!(factory.store().parent(first), Some(syntax_list));
        assert_eq!(factory.store().parent(second), Some(syntax_list));
    }

    struct TestTraversal<'a> {
        source: &'a AstStore,
        factory: NodeFactory,
        state: AstTraversalState,
        mode: TestTraversalMode,
        visits: usize,
    }

    #[derive(Clone, Copy)]
    enum TestTraversalMode {
        Identity,
        DeleteFirstNode,
        DeleteEmbeddedStatement,
        DeleteNodeAndLiftEmbeddedStatement,
        ReplaceFirstNode,
        ReplaceWithSyntaxList,
        ReentrantVisitEachChild,
    }

    impl<'a> TestTraversal<'a> {
        fn new(source: &'a AstStore, mode: TestTraversalMode) -> Self {
            Self {
                source,
                factory: NodeFactory::default(),
                state: AstTraversalState::new(),
                mode,
                visits: 0,
            }
        }

        fn store_for(&self, node: Node) -> &AstStore {
            if node.store_id() == self.factory.store().store_id() {
                self.factory.store()
            } else {
                assert_eq!(node.store_id(), self.source.store_id());
                self.source
            }
        }

        fn preserve_source_node(&mut self, node: Node) -> Node {
            if node.store_id() == self.factory.store().store_id() {
                node
            } else {
                self.state
                    .preserve_node(self.source, &mut self.factory, node)
            }
        }

        fn visit_each_child(&mut self, node: Node) -> Node {
            self.generated_visit_each_child(&node)
        }

        fn lift_to_block_or_empty(&mut self, node: Option<Node>) -> Option<Node> {
            let node = match node {
                Some(node) => node,
                None => {
                    let statements = self.factory.new_node_list(
                        core::undefined_text_range(),
                        core::undefined_text_range(),
                        Vec::<Node>::new(),
                    );
                    return Some(self.factory.new_block(statements, true));
                }
            };
            Some(self.lift_to_block(node))
        }

        fn lift_to_block(&mut self, node: Node) -> Node {
            let store = self.store_for(node);
            let nodes = if store.kind(node) == Kind::SyntaxList {
                store
                    .raw_node_slice(store.as_syntax_list(node).children)
                    .iter()
                    .flatten()
                    .collect::<Vec<_>>()
            } else {
                vec![node]
            };
            let nodes = nodes
                .into_iter()
                .map(|node| self.preserve_source_node(node))
                .collect::<Vec<_>>();
            if nodes.len() == 1 {
                nodes[0]
            } else {
                let statements = self.factory.new_node_list(
                    core::undefined_text_range(),
                    core::undefined_text_range(),
                    nodes,
                );
                self.factory.new_block(statements, true)
            }
        }
    }

    impl<'source> AstVisitEachChildRuntime<'source> for TestTraversal<'source> {
        fn source_store(&self) -> &AstStore {
            self.source
        }

        fn factory(&self) -> &NodeFactory {
            &self.factory
        }

        fn factory_mut(&mut self) -> &mut NodeFactory {
            &mut self.factory
        }

        fn preserved_node(&self, source: Node) -> Option<Node> {
            self.state.preserved_node(&self.factory, source)
        }

        fn preserve_node(&mut self, node: Node) -> Node {
            self.preserve_source_node(node)
        }

        fn record_preserved_node(&mut self, source: Node, imported: Node) -> Node {
            let imported = self.preserve_node(imported);
            self.state
                .record_preserved_node(source.store_id(), &self.factory, source, imported)
        }

        fn preserved_source_node_matches(
            &self,
            source: Option<Node>,
            output: Option<Node>,
        ) -> bool {
            self.state
                .preserved_source_node_matches(&self.factory, source, output)
        }

        fn update_source_file_from_visited(
            &mut self,
            node: Node,
            statements: Option<NodeList>,
            end_of_file_token: Option<Node>,
            source_unchanged: bool,
        ) -> Node {
            assert_eq!(node.store_id(), self.source.store_id());
            let source = self.source;
            if source_unchanged {
                let imported = self.preserve_source_node(node);
                return self.record_preserved_node(node, imported);
            }
            let source_data = source.as_source_file(node);
            let source_metadata = SourceFileCopyMetadata::from_source(source_data)
                .map_nodes(node, |node| self.preserve_source_node(node));
            let updated = self
                .factory
                .update_source_file_from_store_with_mapped_metadata(
                    source,
                    node,
                    source_data,
                    source_metadata.metadata,
                    statements,
                    end_of_file_token,
                );
            self.factory
                .restore_source_file_self_references(updated, source_metadata.self_references);
            updated
        }

        fn visit_node(&mut self, node: Option<Node>) -> Option<Node> {
            let node = node?;
            self.visits += 1;
            match self.mode {
                TestTraversalMode::Identity => Some(node),
                TestTraversalMode::DeleteFirstNode if self.visits == 1 => None,
                TestTraversalMode::DeleteNodeAndLiftEmbeddedStatement => None,
                TestTraversalMode::ReplaceFirstNode if self.visits == 1 => {
                    Some(self.factory.new_identifier("replacement"))
                }
                TestTraversalMode::ReplaceWithSyntaxList => {
                    let imported = self.preserve_source_node(node);
                    let default_modifier = self.factory.new_modifier(Kind::DefaultKeyword);
                    Some(
                        self.factory
                            .new_syntax_list(vec![imported, default_modifier]),
                    )
                }
                TestTraversalMode::ReentrantVisitEachChild
                    if self.source.kind(node) == Kind::ExpressionStatement =>
                {
                    Some(self.visit_each_child(node))
                }
                _ => Some(self.preserve_source_node(node)),
            }
        }

        fn visit_token(&mut self, node: Option<Node>) -> Option<Node> {
            self.visit_node(node)
        }

        fn visit_function_body(&mut self, node: Option<Node>) -> Option<Node> {
            self.visit_node(node)
        }

        fn visit_iteration_body(&mut self, node: Option<Node>) -> Option<Node> {
            self.visit_embedded_statement(node)
        }

        fn visit_embedded_statement(&mut self, node: Option<Node>) -> Option<Node> {
            match self.mode {
                TestTraversalMode::Identity
                | TestTraversalMode::DeleteFirstNode
                | TestTraversalMode::ReplaceFirstNode
                | TestTraversalMode::ReplaceWithSyntaxList => self.visit_node(node),
                TestTraversalMode::DeleteEmbeddedStatement => None,
                TestTraversalMode::DeleteNodeAndLiftEmbeddedStatement => {
                    let visited = self.visit_node(node);
                    self.lift_to_block_or_empty(visited)
                }
                TestTraversalMode::ReentrantVisitEachChild => {
                    self.visit_node(node).map(|node| self.lift_to_block(node))
                }
            }
        }
    }

    impl<'source> AstGeneratedVisitEachChild<'source> for TestTraversal<'source> {}

    #[test]
    fn visit_nodes_input_preserves_foreign_list_identity_when_unchanged() {
        let mut source_factory = NodeFactory::default();
        let expression = source_factory.new_identifier("value");
        let statement = source_factory.new_expression_statement(expression);
        let source_list = source_factory.new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![statement],
        );
        let mut traversal = TestTraversal::new(source_factory.store(), TestTraversalMode::Identity);

        let visited = traversal
            .visit_nodes_input(Some(SourceNodeListInput::from_source(SourceNodeList::new(
                source_factory.store(),
                source_list.id(),
            ))))
            .expect("identity visitor should preserve required list");

        assert_eq!(visited, source_list);
        assert_eq!(visited.id().store_id(), source_factory.store().store_id());
    }

    #[test]
    fn visit_nodes_input_imports_unchanged_siblings_when_deletion_changes_foreign_list() {
        let mut source_factory = NodeFactory::default();
        let first = source_factory.new_identifier("drop");
        let second = source_factory.new_identifier("keep");
        let source_list = source_factory.new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![first, second],
        );
        let mut traversal =
            TestTraversal::new(source_factory.store(), TestTraversalMode::DeleteFirstNode);

        let visited = traversal
            .visit_nodes_input(Some(SourceNodeListInput::from_source(SourceNodeList::new(
                source_factory.store(),
                source_list.id(),
            ))))
            .expect("changed required list should be materialized");
        let nodes = SourceNodeList::new(traversal.factory.store(), visited.id())
            .iter()
            .collect::<Vec<_>>();

        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].store_id(), traversal.factory.store().store_id());
        assert_eq!(traversal.factory.store().kind(nodes[0]), Kind::Identifier);
    }

    #[test]
    fn visit_nodes_input_imports_unchanged_siblings_when_replacement_changes_foreign_list() {
        let mut source_factory = NodeFactory::default();
        let first = source_factory.new_identifier("old");
        let second = source_factory.new_identifier("keep");
        let source_list = source_factory.new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![first, second],
        );
        let mut traversal =
            TestTraversal::new(source_factory.store(), TestTraversalMode::ReplaceFirstNode);

        let visited = traversal
            .visit_nodes_input(Some(SourceNodeListInput::from_source(SourceNodeList::new(
                source_factory.store(),
                source_list.id(),
            ))))
            .expect("changed required list should be materialized");
        let nodes = SourceNodeList::new(traversal.factory.store(), visited.id())
            .iter()
            .collect::<Vec<_>>();

        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].store_id(), traversal.factory.store().store_id());
        assert_eq!(nodes[1].store_id(), traversal.factory.store().store_id());
        assert_eq!(traversal.factory.store().kind(nodes[0]), Kind::Identifier);
        assert_eq!(traversal.factory.store().kind(nodes[1]), Kind::Identifier);
    }

    #[test]
    fn visit_raw_node_slice_input_imports_unchanged_slots_when_changed() {
        let mut source_factory = NodeFactory::default();
        let first = source_factory.new_identifier("drop");
        let second = source_factory.new_identifier("keep");
        let source_slice = source_factory.new_raw_node_slice([Some(first), Some(second)]);
        let mut traversal =
            TestTraversal::new(source_factory.store(), TestTraversalMode::DeleteFirstNode);

        let visited = traversal
            .visit_raw_node_slice_input(Some(SourceRawNodeSliceInput::from_source(
                SourceRawNodeSlice::new(source_factory.store(), source_slice.id()),
            )))
            .expect("changed raw node slice should be materialized");
        let nodes = SourceRawNodeSlice::new(traversal.factory.store(), visited.id())
            .iter()
            .collect::<Vec<_>>();

        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0], None);
        let kept = nodes[1].expect("unchanged sibling should be preserved");
        assert_eq!(kept.store_id(), traversal.factory.store().store_id());
        assert_eq!(traversal.factory.store().kind(kept), Kind::Identifier);
    }

    #[test]
    fn generated_visit_each_child_preserves_foreign_node_when_children_unchanged() {
        let mut source_factory = NodeFactory::default();
        let expression = source_factory.new_identifier("value");
        let statement = source_factory.new_expression_statement(expression);
        let mut traversal = TestTraversal::new(source_factory.store(), TestTraversalMode::Identity);

        let visited = traversal.visit_each_child(statement);

        assert_eq!(visited, statement);
        assert_eq!(visited.store_id(), source_factory.store().store_id());
    }

    #[test]
    fn visit_modifiers_input_expands_syntax_list_replacement() {
        let mut source_factory = NodeFactory::default();
        let export_modifier = source_factory.new_modifier(Kind::ExportKeyword);
        let source_modifiers = source_factory.new_modifier_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            vec![export_modifier],
            ModifierFlags::EXPORT,
        );
        let mut traversal = TestTraversal::new(
            source_factory.store(),
            TestTraversalMode::ReplaceWithSyntaxList,
        );

        let visited = traversal
            .visit_modifiers_input(Some(SourceModifierListInput::from_source(
                SourceModifierList::new(source_factory.store(), source_modifiers.id()),
            )))
            .expect("modifier replacement should produce a modifier list");
        let nodes = SourceModifierList::new(traversal.factory.store(), visited.id())
            .iter()
            .collect::<Vec<_>>();

        assert_eq!(nodes.len(), 2);
        assert_eq!(
            traversal.factory.store().kind(nodes[0]),
            Kind::ExportKeyword
        );
        assert_eq!(
            traversal.factory.store().kind(nodes[1]),
            Kind::DefaultKeyword
        );
    }

    #[test]
    fn typed_embedded_statement_delete_returns_none() {
        let mut source_factory = NodeFactory::default();
        let expression = source_factory.new_identifier("value");
        let statement = source_factory.new_expression_statement(expression);
        let mut traversal = TestTraversal::new(
            source_factory.store(),
            TestTraversalMode::DeleteEmbeddedStatement,
        );

        assert_eq!(traversal.visit_embedded_statement(Some(statement)), None);
    }

    #[test]
    fn typed_embedded_statement_delete_lifts_to_empty_block() {
        let mut source_factory = NodeFactory::default();
        let expression = source_factory.new_identifier("value");
        let statement = source_factory.new_expression_statement(expression);
        let mut traversal = TestTraversal::new(
            source_factory.store(),
            TestTraversalMode::DeleteNodeAndLiftEmbeddedStatement,
        );

        let visited = traversal
            .visit_embedded_statement(Some(statement))
            .expect("node deletion should lift to an empty block");
        assert_eq!(traversal.factory.store().kind(visited), Kind::Block);
        let statements = traversal.factory.store().statements(visited).unwrap();
        assert_eq!(statements.len(), 0);
    }

    #[test]
    fn typed_generated_visit_each_child_is_reentrant() {
        let mut source_factory = NodeFactory::default();
        let expression = source_factory.new_identifier("value");
        let statement = source_factory.new_expression_statement(expression);
        let mut traversal = TestTraversal::new(
            source_factory.store(),
            TestTraversalMode::ReentrantVisitEachChild,
        );

        let _ = traversal.visit_node(Some(statement));

        assert_eq!(traversal.visits, 2);
    }
}
