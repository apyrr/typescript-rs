#![allow(non_upper_case_globals)]

use crate::*;
use std::ops::{BitAnd, BitOr, BitOrAssign};
use std::sync::{
    RwLock, RwLockReadGuard, RwLockWriteGuard,
    atomic::{AtomicU64, Ordering},
};
use ts_collections::FastHashMap;

// FlowFlags

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FlowFlags(pub u32);

impl FlowFlags {
    pub const Unreachable: FlowFlags = FlowFlags(1 << 0); // Unreachable code
    pub const Start: FlowFlags = FlowFlags(1 << 1); // Start of flow graph
    pub const BranchLabel: FlowFlags = FlowFlags(1 << 2); // Non-looping junction
    pub const LoopLabel: FlowFlags = FlowFlags(1 << 3); // Looping junction
    pub const Assignment: FlowFlags = FlowFlags(1 << 4); // Assignment
    pub const TrueCondition: FlowFlags = FlowFlags(1 << 5); // Condition known to be true
    pub const FalseCondition: FlowFlags = FlowFlags(1 << 6); // Condition known to be false
    pub const SwitchClause: FlowFlags = FlowFlags(1 << 7); // Switch statement clause
    pub const ArrayMutation: FlowFlags = FlowFlags(1 << 8); // Potential array mutation
    pub const Call: FlowFlags = FlowFlags(1 << 9); // Potential assertion call
    pub const ReduceLabel: FlowFlags = FlowFlags(1 << 10); // Temporarily reduce antecedents of label
    pub const Referenced: FlowFlags = FlowFlags(1 << 11); // Referenced as antecedent once
    pub const Shared: FlowFlags = FlowFlags(1 << 12); // Referenced as antecedent more than once
    pub const Label: FlowFlags = FlowFlags(Self::BranchLabel.0 | Self::LoopLabel.0);
    pub const Condition: FlowFlags = FlowFlags(Self::TrueCondition.0 | Self::FalseCondition.0);

    pub fn intersects(self, other: FlowFlags) -> bool {
        self.0 & other.0 != 0
    }
}

impl BitOr for FlowFlags {
    type Output = FlowFlags;

    fn bitor(self, rhs: FlowFlags) -> FlowFlags {
        FlowFlags(self.0 | rhs.0)
    }
}

impl BitOrAssign for FlowFlags {
    fn bitor_assign(&mut self, rhs: FlowFlags) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for FlowFlags {
    type Output = u32;

    fn bitand(self, rhs: FlowFlags) -> u32 {
        self.0 & rhs.0
    }
}

// FlowNode

static NEXT_FLOW_GRAPH_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct FlowGraphId(u64);

impl FlowGraphId {
    const INVALID: Self = Self(0);

    fn fresh() -> Self {
        let id = NEXT_FLOW_GRAPH_ID.fetch_add(1, Ordering::Relaxed);
        assert_ne!(id, 0, "flow graph id overflow");
        Self(id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FlowRef {
    graph: FlowGraphId,
    index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FlowListRef {
    graph: FlowGraphId,
    index: usize,
}

impl FlowRef {
    const fn index(self) -> usize {
        self.index
    }
}

impl Default for FlowRef {
    fn default() -> Self {
        Self {
            graph: FlowGraphId::INVALID,
            index: usize::MAX,
        }
    }
}

#[derive(Clone, Default)]
pub struct FlowRefSideTable<T> {
    graphs: FastHashMap<FlowGraphId, Vec<Option<T>>>,
}

impl<T> FlowRefSideTable<T> {
    pub fn get(&self, flow: FlowRef) -> Option<&T> {
        self.graphs
            .get(&flow.graph)
            .and_then(|slots| slots.get(flow.index()))
            .and_then(Option::as_ref)
    }

    pub fn insert(&mut self, flow: FlowRef, value: T) {
        let index = flow.index();
        let slots = self.graphs.entry(flow.graph).or_default();
        if slots.len() <= index {
            slots.resize_with(index + 1, || None);
        }
        slots[index] = Some(value);
    }

    pub fn clear(&mut self) {
        self.graphs.clear();
    }
}

pub struct FlowGraph {
    id: FlowGraphId,
    nodes: Vec<RwLock<FlowNode>>,
    lists: Vec<RwLock<FlowList>>,
}

impl Default for FlowGraph {
    fn default() -> Self {
        Self {
            id: FlowGraphId::fresh(),
            nodes: Vec::new(),
            lists: Vec::new(),
        }
    }
}

impl FlowGraph {
    pub fn new_node(&mut self, mut node: FlowNode) -> FlowRef {
        let id = FlowRef {
            graph: self.id,
            index: self.nodes.len(),
        };
        node.id = id;
        self.nodes.push(RwLock::new(node));
        id
    }

    pub fn new_list(&mut self, list: FlowList) -> FlowListRef {
        let id = FlowListRef {
            graph: self.id,
            index: self.lists.len(),
        };
        self.lists.push(RwLock::new(list));
        id
    }

    pub fn node(&self, id: FlowRef) -> RwLockReadGuard<'_, FlowNode> {
        assert_eq!(id.graph, self.id, "FlowRef used with the wrong FlowGraph");
        self.nodes[id.index]
            .read()
            .unwrap_or_else(|err| err.into_inner())
    }

    pub fn node_mut(&self, id: FlowRef) -> RwLockWriteGuard<'_, FlowNode> {
        assert_eq!(id.graph, self.id, "FlowRef used with the wrong FlowGraph");
        self.nodes[id.index]
            .write()
            .unwrap_or_else(|err| err.into_inner())
    }

    pub fn list(&self, id: FlowListRef) -> RwLockReadGuard<'_, FlowList> {
        assert_eq!(
            id.graph, self.id,
            "FlowListRef used with the wrong FlowGraph"
        );
        self.lists[id.index]
            .read()
            .unwrap_or_else(|err| err.into_inner())
    }

    pub fn list_mut(&self, id: FlowListRef) -> RwLockWriteGuard<'_, FlowList> {
        assert_eq!(
            id.graph, self.id,
            "FlowListRef used with the wrong FlowGraph"
        );
        self.lists[id.index]
            .write()
            .unwrap_or_else(|err| err.into_inner())
    }
}

#[derive(Clone, Default)]
pub struct FlowNode {
    pub id: FlowRef,
    pub flags: FlowFlags,
    pub node: Option<FlowNodeReference>, // Associated AST or synthetic flow node
    pub antecedent: Option<FlowRef>,     // Antecedent for all but FlowLabel
    pub antecedents: Option<FlowListRef>, // Linked list of antecedents for FlowLabel
}

#[derive(Clone)]
pub enum FlowNodeReference {
    Node(Node),
    SwitchClause(FlowSwitchClauseData),
    ReduceLabel(FlowReduceLabelData),
}

impl From<Node> for FlowNodeReference {
    fn from(node: Node) -> Self {
        Self::Node(node)
    }
}

impl From<FlowSwitchClauseData> for FlowNodeReference {
    fn from(data: FlowSwitchClauseData) -> Self {
        Self::SwitchClause(data)
    }
}

impl From<FlowReduceLabelData> for FlowNodeReference {
    fn from(data: FlowReduceLabelData) -> Self {
        Self::ReduceLabel(data)
    }
}

#[derive(Clone, Default)]
pub struct FlowList {
    pub flow: Option<FlowRef>,
    pub next: Option<FlowListRef>,
}

pub type FlowLabel = FlowNode;

impl FlowNode {
    pub fn as_flow_node(&self) -> &FlowNode {
        self
    }

    pub fn as_flow_node_mut(&mut self) -> &mut FlowNode {
        self
    }

    pub fn as_flow_label_mut(&mut self) -> &mut FlowLabel {
        self
    }
}

// FlowSwitchClauseData (synthetic AST node for FlowFlagsSwitchClause)

#[derive(Clone, Default)]
pub struct FlowSwitchClauseData {
    pub switch_statement: Option<Node>,
    pub clause_start: i32, // Start index of case/default clause range
    pub clause_end: i32,   // End index of case/default clause range
}

pub fn new_flow_switch_clause_data(
    switch_statement: Option<Node>,
    clause_start: i32,
    clause_end: i32,
) -> FlowNodeReference {
    FlowSwitchClauseData {
        switch_statement,
        clause_start,
        clause_end,
        ..Default::default()
    }
    .into()
}

impl FlowSwitchClauseData {
    pub fn is_empty(&self) -> bool {
        self.clause_start == self.clause_end
    }

    pub fn switch_statement(&self) -> &Node {
        self.switch_statement.as_ref().unwrap()
    }
}

// FlowReduceLabelData (synthetic AST node for FlowFlagsReduceLabel)

#[derive(Clone, Default)]
pub struct FlowReduceLabelData {
    pub target: Option<FlowRef>,          // Target label
    pub antecedents: Option<FlowListRef>, // Temporary antecedent list
}

pub fn new_flow_reduce_label_data(
    target: Option<FlowRef>,
    antecedents: Option<FlowListRef>,
) -> FlowNodeReference {
    FlowReduceLabelData {
        target,
        antecedents,
        ..Default::default()
    }
    .into()
}
