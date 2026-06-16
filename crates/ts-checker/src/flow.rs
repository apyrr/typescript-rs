// package checker

use std::collections::HashMap;
use std::sync::Arc;

use ts_ast as ast;
use ts_binder as binder;
use ts_core as core;
use ts_diagnostics as diagnostics;
use ts_scanner as scanner;
use ts_tracing as tracing;
use xxhash_rust::xxh3;

use crate::checker::*;
use crate::semantic::MarkedAssignmentSymbolLinksStoreExt;

#[derive(Clone, Copy)]
pub struct FlowType {
    t: Option<TypeHandle>,
    incomplete: bool,
}

impl FlowType {
    fn is_nil(&self) -> bool {
        self.t.is_none()
    }
}

impl<'a, 'state> Checker<'a, 'state> {
    fn new_flow_type(&mut self, mut t: TypeHandle, incomplete: bool) -> FlowType {
        if incomplete && self.type_flags(t) & TYPE_FLAGS_NEVER != 0 {
            t = self.semantic_state.semantic_handles().silent_never_type;
        }
        FlowType {
            t: Some(t),
            incomplete,
        }
    }
}

pub struct SharedFlow {
    flow: ast::FlowRef,
    flow_type: FlowType,
}

pub struct FlowState {
    pub reference: Option<ast::Node>,
    pub declared_type: Option<TypeHandle>,
    pub initial_type: Option<TypeHandle>,
    pub flow_container: Option<ast::Node>,
    ref_key: CacheHashKey,
    depth: usize,
    shared_flow_start: usize,
    reduce_labels: Vec<ast::FlowReduceLabelData>,
}

impl FlowState {
    fn new() -> Self {
        Self {
            reference: None,
            declared_type: None,
            initial_type: None,
            flow_container: None,
            ref_key: CacheHashKey::default(),
            depth: 0,
            shared_flow_start: 0,
            reduce_labels: Vec::new(),
        }
    }

    fn reset(&mut self) {
        self.reference = None;
        self.declared_type = None;
        self.initial_type = None;
        self.flow_container = None;
        self.ref_key = CacheHashKey::default();
        self.depth = 0;
        self.shared_flow_start = 0;
        self.reduce_labels.clear();
    }
}

fn flow_node_reference_node(reference: &ast::FlowNodeReference) -> ast::Node {
    match reference {
        ast::FlowNodeReference::Node(node) => *node,
        _ => panic!("expected flow node reference to contain an AST node"),
    }
}

fn flow_node_reference_switch_clause_data(
    reference: &ast::FlowNodeReference,
) -> ast::FlowSwitchClauseData {
    match reference {
        ast::FlowNodeReference::SwitchClause(data) => data.clone(),
        _ => panic!("expected flow node reference to contain switch clause data"),
    }
}

fn flow_node_reference_reduce_label_data(
    reference: &ast::FlowNodeReference,
) -> ast::FlowReduceLabelData {
    match reference {
        ast::FlowNodeReference::ReduceLabel(data) => data.clone(),
        _ => panic!("expected flow node reference to contain reduce label data"),
    }
}

fn flow_node_reference_handle(reference: &ast::FlowNodeReference) -> ast::Node {
    flow_node_reference_node(reference)
}

fn flow_node_handle(flow: &ast::FlowNode) -> ast::Node {
    flow_node_reference_handle(flow.node.as_ref().unwrap())
}

impl<'a, 'state> Checker<'a, 'state> {
    fn is_readonly_symbol_identity_in_flow(&mut self, symbol: SymbolIdentity) -> bool {
        let handle = symbol.symbol_handle();
        let check_flags = self.symbol_handle_check_flags(handle);
        let flags = self.symbol_handle_flags(handle);
        let value_declaration = self.symbol_handle_value_declaration(handle);
        let modifier_flags = self.declaration_modifier_flags_from_symbol_handle_for_flow(handle);
        check_flags.intersects(ast::CHECK_FLAGS_READONLY)
            || flags.intersects(ast::SYMBOL_FLAGS_PROPERTY)
                && modifier_flags.intersects(ast::ModifierFlags::Readonly)
            || flags.intersects(ast::SYMBOL_FLAGS_VARIABLE)
                && value_declaration.is_some_and(|declaration| {
                    self.get_combined_node_flags_cached(declaration)
                        .intersects(ast::NODE_FLAGS_CONSTANT)
                })
            || flags.intersects(ast::SYMBOL_FLAGS_ACCESSOR)
                && !flags.intersects(ast::SYMBOL_FLAGS_SET_ACCESSOR)
            || flags.intersects(ast::SYMBOL_FLAGS_ENUM_MEMBER)
            || self.any_symbol_handle_declaration(handle, |checker, declaration| {
                checker.is_readonly_assignment_declaration(declaration)
            })
    }

    fn declaration_modifier_flags_from_symbol_handle_for_flow(
        &mut self,
        symbol: ast::SymbolHandle,
    ) -> ast::ModifierFlags {
        let flags = self.symbol_handle_flags(symbol);
        let check_flags = self.symbol_handle_check_flags(symbol);
        let Some(value_declaration) = self.symbol_handle_value_declaration(symbol) else {
            if check_flags.intersects(ast::CHECK_FLAGS_SYNTHETIC) {
                let access_modifier = if check_flags.intersects(ast::CHECK_FLAGS_CONTAINS_PRIVATE) {
                    ast::ModifierFlags::Private
                } else if check_flags.intersects(ast::CHECK_FLAGS_CONTAINS_PUBLIC) {
                    ast::ModifierFlags::Public
                } else {
                    ast::ModifierFlags::Protected
                };
                let static_modifier = if check_flags.intersects(ast::CHECK_FLAGS_CONTAINS_STATIC) {
                    ast::ModifierFlags::Static
                } else {
                    ast::ModifierFlags::None
                };
                return access_modifier | static_modifier;
            }
            if flags.intersects(ast::SYMBOL_FLAGS_PROTOTYPE) {
                return ast::ModifierFlags::Public | ast::ModifierFlags::Static;
            }
            return ast::ModifierFlags::None;
        };
        let declaration = if flags.intersects(ast::SYMBOL_FLAGS_GET_ACCESSOR) {
            self.with_symbol_handle_declarations(symbol, |declarations| {
                declarations.iter().copied().find(|declaration| {
                    ast::is_get_accessor_declaration(
                        self.store_for_node(*declaration),
                        *declaration,
                    )
                })
            })
            .unwrap_or(value_declaration)
        } else {
            value_declaration
        };
        let modifier_flags = self.get_combined_modifier_flags_cached(declaration);
        if self.symbol_handle_parent(symbol).is_some_and(|parent| {
            self.symbol_handle_flags(parent)
                .intersects(ast::SYMBOL_FLAGS_CLASS)
        }) {
            return modifier_flags;
        }
        modifier_flags & !ast::ModifierFlags::AccessibilityModifier
    }

    pub(crate) fn is_constant_variable_identity(&mut self, symbol: SymbolIdentity) -> bool {
        let handle = symbol.symbol_handle();
        self.symbol_handle_flags(handle)
            .intersects(ast::SYMBOL_FLAGS_VARIABLE)
            && self
                .symbol_handle_value_declaration(handle)
                .is_some_and(|declaration| {
                    self.get_combined_node_flags_cached(declaration)
                        .intersects(ast::NODE_FLAGS_CONSTANT)
                })
    }

    pub(crate) fn is_parameter_or_mutable_local_variable_identity(
        &mut self,
        symbol: SymbolIdentity,
    ) -> bool {
        let handle = symbol.symbol_handle();
        let Some(value_declaration) = self.symbol_handle_value_declaration(handle) else {
            return false;
        };
        let store = self.store_for_node(value_declaration);
        let declaration = ast::get_root_declaration(store, value_declaration);
        ast::is_parameter_declaration(store, declaration)
            || ast::is_variable_declaration(store, declaration)
                && (store
                    .parent(declaration)
                    .is_some_and(|parent| ast::is_catch_clause(store, parent))
                    || self.is_mutable_local_variable_declaration(declaration))
    }

    fn is_symbol_assigned_identity(&mut self, symbol: SymbolIdentity) -> bool {
        let Some(value_declaration) = self.symbol_handle_value_declaration(symbol.symbol_handle())
        else {
            return false;
        };
        self.ensure_assignments_marked_for_declaration(value_declaration, symbol);
        self.semantic_state
            .marked_assignment_last_assignment_pos(symbol)
            != 0
    }

    pub(crate) fn is_past_last_assignment_identity(
        &mut self,
        symbol: SymbolIdentity,
        location: Option<ast::Node>,
    ) -> bool {
        let Some(value_declaration) = self.symbol_handle_value_declaration(symbol.symbol_handle())
        else {
            return true;
        };
        self.ensure_assignments_marked_for_declaration(value_declaration, symbol);
        let last_assignment_pos = self
            .semantic_state
            .marked_assignment_last_assignment_pos(symbol);
        last_assignment_pos == 0
            || location.is_some_and(|location| {
                last_assignment_pos < self.store_for_node(location).loc(location).pos()
            })
    }

    fn ensure_assignments_marked_for_declaration(
        &mut self,
        value_declaration: ast::Node,
        symbol: SymbolIdentity,
    ) {
        if self
            .semantic_state
            .marked_assignment_last_assignment_pos(symbol)
            != 0
        {
            return;
        }
        let parent = ast::find_ancestor(
            self.store_for_node(value_declaration),
            Some(value_declaration),
            ast::is_function_or_source_file,
        );
        let Some(parent) = parent else {
            return;
        };
        let newly_marked = !self.has_node_link_flags(parent, NODE_CHECK_FLAGS_ASSIGNMENTS_MARKED);
        if newly_marked {
            self.add_node_link_flags(parent, NODE_CHECK_FLAGS_ASSIGNMENTS_MARKED);
        }
        if newly_marked && !self.has_parent_with_assignments_marked(parent) {
            self.mark_node_assignments(parent);
        }
    }

    fn is_stable_cached_element_access_symbol(&mut self, symbol: SymbolIdentity) -> bool {
        self.is_constant_variable_identity(symbol)
            || self.is_parameter_or_mutable_local_variable_identity(symbol)
                && !self.is_symbol_assigned_identity(symbol)
    }

    fn get_export_symbol_of_value_symbol_identity_if_exported(
        &mut self,
        symbol: Option<SymbolIdentity>,
    ) -> Option<SymbolIdentity> {
        let symbol = symbol?;
        let export_symbol = if self
            .symbol_identity_flags(symbol)
            .intersects(ast::SYMBOL_FLAGS_EXPORT_VALUE)
        {
            self.symbol_identity_export_symbol(symbol)
        } else {
            None
        };
        self.get_merged_symbol_identity(export_symbol.or(Some(symbol)))
    }

    fn symbol_identity_value_declaration(&self, symbol: SymbolIdentity) -> Option<ast::Node> {
        self.symbol_handle_value_declaration(symbol.symbol_handle())
    }

    fn get_type_of_symbol_identity_for_flow(
        &mut self,
        symbol: SymbolIdentity,
        location: Option<ast::Node>,
    ) -> TypeHandle {
        self.get_type_of_symbol_identity_at_location(symbol, location)
    }

    fn get_explicit_type_of_symbol_identity(
        &mut self,
        symbol: Option<SymbolIdentity>,
        diagnostic: Option<&mut ast::Diagnostic>,
    ) -> Option<TypeHandle> {
        let symbol = self.resolve_symbol_identity(symbol?, false);
        let flags = self.symbol_identity_flags(symbol);
        if flags
            & (ast::SYMBOL_FLAGS_FUNCTION
                | ast::SYMBOL_FLAGS_METHOD
                | ast::SYMBOL_FLAGS_CLASS
                | ast::SYMBOL_FLAGS_VALUE_MODULE)
            != 0
        {
            return Some(self.get_type_of_symbol_identity_for_flow(symbol, None));
        }
        if flags & (ast::SYMBOL_FLAGS_VARIABLE | ast::SYMBOL_FLAGS_PROPERTY) != 0 {
            let check_flags = self.symbol_identity_check_flags(symbol);
            if check_flags & ast::CHECK_FLAGS_MAPPED != 0 {
                if let Some(origin) = self.mapped_synthetic_origin_symbol_identity(symbol) {
                    if self
                        .get_explicit_type_of_symbol_identity(Some(origin), None)
                        .is_some()
                    {
                        return Some(self.get_type_of_symbol_identity_for_flow(symbol, None));
                    }
                }
            }
            if let Some(declaration) = self.symbol_identity_value_declaration(symbol) {
                if self.is_declaration_with_explicit_type_annotation(declaration) {
                    return Some(self.get_type_of_symbol_identity_for_flow(symbol, None));
                }
                let declaration_store = self.store_for_node(declaration);
                if ast::is_variable_declaration(declaration_store, declaration) && {
                    let declaration_parent = self
                        .store_for_node(declaration)
                        .parent(declaration)
                        .unwrap();
                    self.store_for_node(declaration_parent)
                        .parent(declaration_parent)
                        .is_some_and(|parent| ast::is_for_of_statement(declaration_store, parent))
                } {
                    let declaration_parent = self
                        .store_for_node(declaration)
                        .parent(declaration)
                        .unwrap();
                    let statement = self
                        .store_for_node(declaration_parent)
                        .parent(declaration_parent)
                        .unwrap();
                    let expression_type = self.get_type_of_dotted_name(
                        self.store_for_node(statement)
                            .expression(statement)
                            .unwrap(),
                        None,
                    );
                    if let Some(expression_type) = expression_type {
                        let use_ = if self
                            .store_for_node(statement)
                            .await_modifier(statement)
                            .is_some()
                        {
                            ITERATION_USE_FOR_AWAIT_OF
                        } else {
                            ITERATION_USE_FOR_OF
                        };
                        return Some(self.check_iterated_type_or_element_type(
                            use_,
                            expression_type,
                            self.semantic_state.semantic_handles().undefined_type,
                            None,
                        ));
                    }
                }
                if let Some(diagnostic) = diagnostic {
                    let name = self.symbol_identity_name(symbol);
                    diagnostic.add_related_info(create_diagnostic_for_node_with_args(
                        self.store_for_node(declaration),
                        declaration,
                        &diagnostics::X_0_needs_an_explicit_type_annotation,
                        vec![DiagnosticArg::from(name.to_string())],
                    ));
                }
            }
        }
        None
    }

    fn get_private_identifier_property_name_for_type_symbol(
        &self,
        t: TypeHandle,
        description: &str,
    ) -> Option<String> {
        let symbol = self.type_symbol_identity(t)?;
        let handle = symbol.symbol_handle();
        Some(self.private_identifier_symbol_name_for_symbol_handle(handle, description))
    }

    fn get_flow_type_of_property_identity(
        &mut self,
        reference: ast::Node,
        prop: Option<SymbolIdentity>,
    ) -> TypeHandle {
        let Some(prop) = prop else {
            return self.get_flow_type_of_property(reference, None);
        };
        let handle = prop.symbol_handle();
        self.get_flow_type_of_property_handle(reference, Some(handle))
    }

    pub(crate) fn get_flow_state(&mut self) -> FlowState {
        self.semantic_state
            .free_flow_states
            .pop()
            .unwrap_or_else(FlowState::new)
    }

    pub(crate) fn put_flow_state(&mut self, mut f: FlowState) {
        f.reset();
        self.semantic_state.free_flow_states.push(f);
    }
}

impl<'a, 'state> Checker<'a, 'state> {
    fn flow_store_for_node(&self, node: ast::Node) -> &ast::AstStore {
        if node.store_id() == self.factory().store().store_id() {
            self.factory().store()
        } else {
            self.store_for_node(node)
        }
    }

    fn flow_kind(&self, node: ast::Node) -> ast::Kind {
        self.flow_store_for_node(node).kind(node)
    }

    fn flow_flags(&self, node: ast::Node) -> ast::NodeFlags {
        self.flow_store_for_node(node).flags(node)
    }

    fn flow_pos(&self, node: ast::Node) -> i32 {
        self.flow_store_for_node(node).loc(node).pos()
    }

    fn flow_end(&self, node: ast::Node) -> i32 {
        self.flow_store_for_node(node).loc(node).end()
    }

    fn try_source_file_for_flow_node(&self, node: ast::Node) -> Option<&'a ast::SourceFile> {
        let mut current = Some(node);
        while let Some(node) = current {
            if let Some(source_file) = self.try_source_file_for_node(node) {
                return Some(source_file);
            }
            current = self.flow_store_for_node(node).parent(node);
        }
        None
    }

    pub(crate) fn get_flow_ref_of_node(
        &self,
        node: ast::Node,
    ) -> Option<(Arc<binder::ProgramBindingState>, ast::FlowRef)> {
        let source_file = self.try_source_file_for_flow_node(node)?;
        let binding_state = self.source_file_binding_state_arc(source_file);
        let flow_node = self.output_node_flow_node(node)?;
        Some((binding_state, flow_node))
    }

    pub(crate) fn get_flow_type_of_reference(
        &mut self,
        reference: ast::Node,
        declared_type: TypeHandle,
    ) -> TypeHandle {
        self.get_flow_type_of_reference_ex(reference, declared_type, declared_type, None, None)
    }

    pub(crate) fn get_flow_type_of_reference_ex(
        &mut self,
        reference: ast::Node,
        declared_type: TypeHandle,
        initial_type: TypeHandle,
        flow_container: Option<ast::Node>,
        flow_node: Option<(Arc<binder::ProgramBindingState>, ast::FlowRef)>,
    ) -> TypeHandle {
        if self.flow_analysis_disabled() {
            return self.semantic_state.semantic_handles().error_type;
        }
        let (flow_graph, flow_node) = if let Some(flow_node) = flow_node {
            flow_node
        } else {
            let Some(flow_node) = self.get_flow_ref_of_node(reference) else {
                return declared_type;
            };
            flow_node
        };
        let mut f = self.get_flow_state();
        f.reference = Some(reference);
        f.declared_type = Some(declared_type);
        f.initial_type = Some(core::coalesce(Some(initial_type), Some(declared_type)).unwrap());
        f.flow_container = flow_container;
        f.shared_flow_start = self.semantic_state.shared_flows.len();
        self.enter_flow_invocation();
        let evolved_type = self
            .get_type_at_flow_node(&mut f, flow_graph.flow_graph(), flow_node)
            .t
            .unwrap();
        self.semantic_state
            .shared_flows
            .truncate(f.shared_flow_start);
        self.put_flow_state(f);
        // When the reference is 'x' in an 'x.length', 'x.push(value)', 'x.unshift(value)' or x[n] = value' operation,
        // we give type 'any[]' to 'x' instead of using the type determined by control flow analysis such that operations
        // on empty arrays are possible without implicit any errors and new element types can be inferred without
        // type mismatch errors.
        let result_type = if self.object_flags(evolved_type) & OBJECT_FLAGS_EVOLVING_ARRAY != 0
            && self.is_evolving_array_operation_target(reference)
        {
            self.semantic_state.semantic_handles().auto_array_type
        } else {
            self.finalize_evolving_array_type(evolved_type)
        };
        let reference_parent = self.flow_store_for_node(reference).parent(reference);
        if result_type
            == self
                .semantic_state
                .semantic_handles()
                .unreachable_never_type
            || reference_parent.is_some()
                && ast::is_non_null_expression(
                    self.flow_store_for_node(reference_parent.unwrap()),
                    reference_parent.unwrap(),
                )
                && self.type_flags(result_type) & TYPE_FLAGS_NEVER == 0
                && {
                    let fact_type =
                        self.get_type_with_facts(result_type, TYPE_FACTS_NE_UNDEFINED_OR_NULL);
                    self.type_flags(fact_type) & TYPE_FLAGS_NEVER != 0
                }
        {
            return declared_type;
        }
        result_type
    }

    fn get_type_at_flow_node(
        &mut self,
        f: &mut FlowState,
        graph: &ast::FlowGraph,
        mut flow: ast::FlowRef,
    ) -> FlowType {
        if f.depth == 2000 {
            // We have made 2000 recursive invocations. To avoid overflowing the call stack we report an error
            // and disable further control flow analysis in the containing function or module body.
            if let Some(tr) = self.tracer.as_ref() {
                tr.instant(
                    tracing::Phase::CheckTypes,
                    "getTypeAtFlowNode_DepthLimit",
                    HashMap::from([("depth".to_string(), f.depth.into())]),
                );
            }
            self.set_flow_analysis_disabled(true);
            self.report_flow_control_error(*f.reference.as_ref().unwrap());
            return FlowType {
                t: Some(self.semantic_state.semantic_handles().error_type),
                incomplete: false,
            };
        }
        f.depth += 1;
        let mut shared_flow: Option<ast::FlowRef> = None;
        loop {
            let (flags, node, antecedent, antecedents) = {
                let flow_node = graph.node(flow);
                (
                    flow_node.flags,
                    flow_node.node.clone(),
                    flow_node.antecedent,
                    flow_node.antecedents,
                )
            };
            if flags & ast::FlowFlags::Shared != 0 {
                // We cache results of flow type resolution for shared nodes that were previously visited in
                // the same getFlowTypeOfReference invocation. A node is considered shared when it is the
                // antecedent of more than one node.
                for shared in self.semantic_state.shared_flows[f.shared_flow_start..].iter() {
                    if shared.flow == flow {
                        f.depth -= 1;
                        return shared.flow_type.clone();
                    }
                }
                shared_flow = Some(flow);
            }
            let t = if flags & ast::FlowFlags::Assignment != 0 {
                let t = {
                    let flow_node = graph.node(flow);
                    self.get_type_at_flow_assignment(f, graph, &flow_node)
                };
                if t.is_nil() {
                    flow = antecedent.unwrap();
                    continue;
                }
                t
            } else if flags & ast::FlowFlags::Call != 0 {
                let t = {
                    let flow_node = graph.node(flow);
                    self.get_type_at_flow_call(f, graph, &flow_node)
                };
                if t.is_nil() {
                    flow = antecedent.unwrap();
                    continue;
                }
                t
            } else if flags & ast::FlowFlags::Condition != 0 {
                let flow_node = graph.node(flow);
                self.get_type_at_flow_condition(f, graph, &flow_node)
            } else if flags & ast::FlowFlags::SwitchClause != 0 {
                let flow_node = graph.node(flow);
                self.get_type_at_switch_clause(f, graph, &flow_node)
            } else if flags & ast::FlowFlags::BranchLabel != 0 {
                let antecedents = get_branch_label_antecedents_ref(graph, flow, &f.reduce_labels);
                if antecedents.next.is_none() {
                    flow = antecedents.flow.unwrap();
                    continue;
                }
                self.get_type_at_flow_branch_label(f, graph, &antecedents)
            } else if flags & ast::FlowFlags::LoopLabel != 0 {
                let antecedents = graph.list(antecedents.unwrap());
                if antecedents.next.is_none() {
                    flow = antecedents.flow.unwrap();
                    continue;
                }
                let flow_node = graph.node(flow);
                self.get_type_at_flow_loop_label(f, graph, &flow_node)
            } else if flags & ast::FlowFlags::ArrayMutation != 0 {
                let t = {
                    let flow_node = graph.node(flow);
                    self.get_type_at_flow_array_mutation(f, graph, &flow_node)
                };
                if t.is_nil() {
                    flow = antecedent.unwrap();
                    continue;
                }
                t
            } else if flags & ast::FlowFlags::ReduceLabel != 0 {
                let data = flow_node_reference_reduce_label_data(&node.unwrap());
                f.reduce_labels.push(data);
                let t = self.get_type_at_flow_node(f, graph, antecedent.unwrap());
                f.reduce_labels.truncate(f.reduce_labels.len() - 1);
                t
            } else if flags & ast::FlowFlags::Start != 0 {
                // Check if we should continue with the control flow of the containing function.
                if let Some(ast::FlowNodeReference::Node(container)) = node.as_ref() {
                    let container = *container;
                    let reference = f.reference.unwrap();
                    let reference_store = self.flow_store_for_node(reference);
                    let container_store = self.flow_store_for_node(container);
                    if Some(container) != f.flow_container
                        && !ast::is_property_access_expression(reference_store, reference)
                        && !ast::is_element_access_expression(reference_store, reference)
                        && !(reference_store.kind(reference) == ast::Kind::ThisKeyword
                            && !ast::is_arrow_function(container_store, container))
                    {
                        flow = self.output_node_flow_node(container).unwrap();
                        continue;
                    }
                }
                // At the top of the flow we have the initial type.
                FlowType {
                    t: f.initial_type.clone(),
                    incomplete: false,
                }
            } else {
                // Unreachable code errors are reported in the binding phase. Here we
                // simply return the non-auto declared type to reduce follow-on errors.
                FlowType {
                    t: Some(self.convert_auto_to_any(*f.declared_type.as_ref().unwrap())),
                    incomplete: false,
                }
            };
            if let Some(shared_flow) = shared_flow.take() {
                self.semantic_state.shared_flows.push(SharedFlow {
                    flow: shared_flow,
                    flow_type: t.clone(),
                });
            }
            f.depth -= 1;
            return t;
        }
    }
}

pub fn get_branch_label_antecedents_ref(
    graph: &ast::FlowGraph,
    flow: ast::FlowRef,
    reduce_labels: &[ast::FlowReduceLabelData],
) -> ast::FlowList {
    for data in reduce_labels.iter().rev() {
        if data.target == Some(flow) {
            return graph.list(data.antecedents.unwrap()).clone();
        }
    }
    let antecedents = graph.node(flow).antecedents.unwrap();
    graph.list(antecedents).clone()
}

impl<'a, 'state> Checker<'a, 'state> {
    fn get_type_at_flow_assignment(
        &mut self,
        f: &mut FlowState,
        graph: &ast::FlowGraph,
        flow: &ast::FlowNode,
    ) -> FlowType {
        let node = flow_node_handle(flow);
        // Assignments only narrow the computed type if the declared type is a union type. Thus, we
        // only need to evaluate the assigned type if the declared type is a union type.
        if self.is_matching_reference(*f.reference.as_ref().unwrap(), node) {
            if !self.is_reachable_flow_ref(graph, flow.id) {
                return FlowType {
                    t: Some(
                        self.semantic_state
                            .semantic_handles()
                            .unreachable_never_type,
                    ),
                    incomplete: false,
                };
            }
            let store = self.store_for_node(node);
            if get_assignment_target_kind(store, node) == ASSIGNMENT_KIND_COMPOUND {
                let flow_type = self.get_type_at_flow_node(f, graph, flow.antecedent.unwrap());
                let base_type = self.get_base_type_of_literal_type(*flow_type.t.as_ref().unwrap());
                return self.new_flow_type(base_type, flow_type.incomplete);
            }
            if f.declared_type.as_ref().unwrap()
                == &self.semantic_state.semantic_handles().auto_type
                || f.declared_type.as_ref().unwrap()
                    == &self.semantic_state.semantic_handles().auto_array_type
            {
                if self.is_empty_array_assignment(node) {
                    let never_type = self.semantic_state.semantic_handles().never_type;
                    return FlowType {
                        t: Some(self.get_evolving_array_type(never_type)),
                        incomplete: false,
                    };
                }
                let initial_or_assigned_type = self.get_initial_or_assigned_type(f, flow);
                let assigned_type = self.get_widened_literal_type(initial_or_assigned_type);
                if self.is_type_assignable_to(assigned_type, *f.declared_type.as_ref().unwrap()) {
                    return FlowType {
                        t: Some(assigned_type),
                        incomplete: false,
                    };
                }
                return FlowType {
                    t: Some(self.semantic_state.semantic_handles().any_array_type),
                    incomplete: false,
                };
            }
            let mut t = f.declared_type.unwrap();
            let store = self.store_for_node(node);
            if is_in_compound_like_assignment(store, node) {
                t = self.get_base_type_of_literal_type(t);
            }
            if self.type_flags(t) & TYPE_FLAGS_UNION != 0 {
                let assigned_type = self.get_initial_or_assigned_type(f, flow);
                let reduced_type = self.get_assignment_reduced_type(t, assigned_type);
                return FlowType {
                    t: Some(reduced_type),
                    incomplete: false,
                };
            }
            return FlowType {
                t: Some(t),
                incomplete: false,
            };
        }
        // We didn't have a direct match. However, if the reference is a dotted name, this
        // may be an assignment to a left hand part of the reference. For example, for a
        // reference 'x.y.z', we may be at an assignment to 'x.y' or 'x'. In that case,
        // return the declared type.
        if self.contains_matching_reference(*f.reference.as_ref().unwrap(), node) {
            if !self.is_reachable_flow_ref(graph, flow.id) {
                return FlowType {
                    t: Some(
                        self.semantic_state
                            .semantic_handles()
                            .unreachable_never_type,
                    ),
                    incomplete: false,
                };
            }
            // Matching dotted names can be expandos on a function expression. In that
            // case TypeScript continues flow analysis before the variable declaration.
            let node_store = self.store_for_node(node);
            if ast::is_variable_declaration(node_store, node)
                && (ast::is_in_js_file(node_store, node) || self.is_var_const_like(node))
                && node_store.initializer(node).is_some_and(|initializer| {
                    self.get_expando_initializer(initializer, false)
                        .is_some_and(|init| {
                            ast::is_function_expression(self.store_for_node(init), init)
                                || ast::is_arrow_function(self.store_for_node(init), init)
                        })
                })
            {
                return self.get_type_at_flow_node(f, graph, flow.antecedent.unwrap());
            }
            return FlowType {
                t: f.declared_type.clone(),
                incomplete: false,
            };
        }
        // for (const _ in ref) acts as a nonnull on ref
        let store = self.store_for_node(node);
        let node_parent = store.parent(node);
        let node_grandparent = node_parent
            .as_ref()
            .and_then(|parent| self.store_for_node(*parent).parent(*parent));
        if ast::is_variable_declaration(store, node)
            && node_grandparent.as_ref().is_some_and(|grandparent| {
                ast::is_for_in_statement(self.store_for_node(*grandparent), *grandparent)
            })
            && {
                let grandparent = node_grandparent.as_ref().unwrap();
                let expression = self
                    .store_for_node(*grandparent)
                    .expression(*grandparent)
                    .unwrap();
                let expression = expression;
                self.is_matching_reference(*f.reference.as_ref().unwrap(), expression)
                    || self.optional_chain_contains_reference(
                        expression,
                        *f.reference.as_ref().unwrap(),
                    )
            }
        {
            let antecedent_type = self
                .get_type_at_flow_node(f, graph, flow.antecedent.unwrap())
                .t
                .unwrap();
            let finalized = self.finalize_evolving_array_type(antecedent_type);
            let non_nullable = self.get_non_nullable_type_if_needed(finalized);
            return FlowType {
                t: Some(non_nullable),
                incomplete: false,
            };
        }
        // Assignment doesn't affect reference
        FlowType {
            t: None,
            incomplete: false,
        }
    }

    fn get_initial_or_assigned_type(&mut self, f: &FlowState, flow: &ast::FlowNode) -> TypeHandle {
        let node = flow_node_handle(flow);
        let store = self.store_for_node(node);
        if ast::is_variable_declaration(store, node) || ast::is_binding_element(store, node) {
            let initial_type = self.get_initial_type(node);
            return self.get_narrowable_type_for_reference(
                initial_type,
                *f.reference.as_ref().unwrap(),
                CHECK_MODE_NORMAL,
            );
        }
        let assigned_type = self.get_assigned_type(node);
        self.get_narrowable_type_for_reference(
            assigned_type,
            *f.reference.as_ref().unwrap(),
            CHECK_MODE_NORMAL,
        )
    }

    fn is_empty_array_assignment(&mut self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        let initializer = if ast::is_variable_declaration(store, node) {
            store.initializer(node)
        } else {
            None
        };
        if let Some(initializer) = initializer {
            return is_empty_array_literal(self.store_for_node(initializer), initializer);
        }
        if ast::is_binding_element(store, node) {
            return false;
        }
        let parent = store.parent(node);
        let Some(parent) = parent else {
            return false;
        };
        if !ast::is_binary_expression(self.store_for_node(parent), parent) {
            return false;
        }
        let parent_store = self.store_for_node(parent);
        let right = parent_store.right(parent).unwrap();
        is_empty_array_literal(parent_store, right)
    }

    fn get_type_at_flow_call(
        &mut self,
        f: &mut FlowState,
        graph: &ast::FlowGraph,
        flow: &ast::FlowNode,
    ) -> FlowType {
        let node = flow_node_handle(flow);
        let signature = self.get_effects_signature(node);
        if let Some(signature) = signature {
            let predicate = self.get_type_predicate_of_signature(signature);
            if let Some(predicate) = predicate {
                let predicate_record = self.type_predicate_record(predicate).clone();
                if predicate_record.kind == TYPE_PREDICATE_KIND_ASSERTS_THIS
                    || predicate_record.kind == TYPE_PREDICATE_KIND_ASSERTS_IDENTIFIER
                {
                    let flow_type = self.get_type_at_flow_node(f, graph, flow.antecedent.unwrap());
                    let t = self.finalize_evolving_array_type(*flow_type.t.as_ref().unwrap());
                    let narrowed_type = if predicate_record.t.is_some() {
                        self.narrow_type_by_type_predicate(
                            f, t, predicate, node, true, /*assumeTrue*/
                        )
                    } else if predicate_record.kind == TYPE_PREDICATE_KIND_ASSERTS_IDENTIFIER
                        && predicate_record.parameter_index >= 0
                        && self
                            .store_for_node(node)
                            .arguments(node)
                            .is_some_and(|arguments| {
                                (predicate_record.parameter_index as usize) < arguments.len()
                            })
                    {
                        let arguments = self.store_for_node(node).arguments(node).unwrap();
                        let argument = arguments
                            .iter()
                            .nth(predicate_record.parameter_index as usize)
                            .unwrap();
                        self.narrow_type_by_assertion(f, t, argument)
                    } else {
                        t
                    };
                    if narrowed_type == t {
                        return flow_type;
                    }
                    return self.new_flow_type(narrowed_type, flow_type.incomplete);
                }
            }
            let return_type = self.get_return_type_of_signature(signature);
            if self.type_flags(return_type) & TYPE_FLAGS_NEVER != 0 {
                return FlowType {
                    t: Some(
                        self.semantic_state
                            .semantic_handles()
                            .unreachable_never_type,
                    ),
                    incomplete: false,
                };
            }
        }
        FlowType {
            t: None,
            incomplete: false,
        }
    }

    fn narrow_type_by_type_predicate(
        &mut self,
        f: &FlowState,
        mut t: TypeHandle,
        predicate: TypePredicateHandle,
        call_expression: ast::Node,
        assume_true: bool,
    ) -> TypeHandle {
        let predicate_record = self.type_predicate_record(predicate).clone();
        // Don't narrow from 'any' if the predicate type is exactly 'Object' or 'Function'
        if predicate_record.t.is_some()
            && !(is_type_any(self, Some(t))
                && (predicate_record.t.as_ref().unwrap()
                    == &self.semantic_state.semantic_handles().global_object_type
                    || predicate_record.t.as_ref().unwrap()
                        == &self.semantic_state.semantic_handles().global_function_type))
        {
            let predicate_argument = self.get_type_predicate_argument(predicate, call_expression);
            if let Some(predicate_argument) = predicate_argument {
                let predicate_argument = predicate_argument;
                if self.is_matching_reference(*f.reference.as_ref().unwrap(), predicate_argument) {
                    return self.get_narrowed_type(
                        t,
                        *predicate_record.t.as_ref().unwrap(),
                        assume_true,
                        false, /*checkDerived*/
                    );
                }
                if self.strict_null_checks()
                    && self.optional_chain_contains_reference(
                        predicate_argument,
                        *f.reference.as_ref().unwrap(),
                    )
                    && (assume_true
                        && !self.has_type_facts(
                            *predicate_record.t.as_ref().unwrap(),
                            TYPE_FACTS_EQ_UNDEFINED,
                        )
                        || !assume_true
                            && every_type(
                                self,
                                *predicate_record.t.as_ref().unwrap(),
                                |checker, tt| checker.is_nullable_type(tt),
                            ))
                {
                    t = self.get_adjusted_type_with_facts(t, TYPE_FACTS_NE_UNDEFINED_OR_NULL);
                }
                let access = self.get_discriminant_property_access(f, predicate_argument, t);
                if let Some(access) = access {
                    return self.narrow_type_by_discriminant(t, access, |checker, tt| {
                        checker.get_narrowed_type(
                            tt,
                            *predicate_record.t.as_ref().unwrap(),
                            assume_true,
                            false, /*checkDerived*/
                        )
                    });
                }
            }
        }
        t
    }

    fn narrow_type_by_assertion(
        &mut self,
        f: &FlowState,
        t: TypeHandle,
        expr: ast::Node,
    ) -> TypeHandle {
        let node = ast::skip_parentheses(self.store_for_node(expr), expr);
        let store = self.store_for_node(node);
        if store.kind(node) == ast::Kind::FalseKeyword {
            return self
                .semantic_state
                .semantic_handles()
                .unreachable_never_type;
        }
        if store.kind(node) == ast::Kind::BinaryExpression {
            let operator = store.kind(store.operator_token(node).unwrap());
            if operator == ast::Kind::AmpersandAmpersandToken {
                let left_node = store.left(node).unwrap();
                let right_node = store.right(node).unwrap();
                let left = self.narrow_type_by_assertion(f, t, left_node);
                return self.narrow_type_by_assertion(f, left, right_node);
            }
            if operator == ast::Kind::BarBarToken {
                let left_node = store.left(node).unwrap();
                let right_node = store.right(node).unwrap();
                let left = self.narrow_type_by_assertion(f, t, left_node);
                let right = self.narrow_type_by_assertion(f, t, right_node);
                return self.get_union_type(vec![left, right]);
            }
        }
        self.narrow_type(f, t, node, true /*assumeTrue*/)
    }

    fn get_type_at_flow_condition(
        &mut self,
        f: &mut FlowState,
        graph: &ast::FlowGraph,
        flow: &ast::FlowNode,
    ) -> FlowType {
        let flow_type = self.get_type_at_flow_node(f, graph, flow.antecedent.unwrap());
        if self.type_flags(*flow_type.t.as_ref().unwrap()) & TYPE_FLAGS_NEVER != 0 {
            return flow_type;
        }
        // If we have an antecedent type (meaning we're reachable in some way), we first
        // attempt to narrow the antecedent type. If that produces the never type, and if
        // the antecedent type is incomplete (i.e. a transient type in a loop), then we
        // take the type guard as an indication that control *could* reach here once we
        // have the complete type. We proceed by switching to the silent never type which
        // doesn't report errors when operators are applied to it. Note that this is the
        // *only* place a silent never type is ever generated.
        let assume_true = flow.flags & ast::FlowFlags::TrueCondition != 0;
        let non_evolving_type = self.finalize_evolving_array_type(*flow_type.t.as_ref().unwrap());
        let narrowed_type =
            self.narrow_type(f, non_evolving_type, flow_node_handle(flow), assume_true);
        if narrowed_type == non_evolving_type {
            return flow_type;
        }
        self.new_flow_type(narrowed_type, flow_type.incomplete)
    }

    // Narrow the given type based on the given expression having the assumed boolean value. The returned type
    // will be a subtype or the same type as the argument.
    pub(crate) fn narrow_type(
        &mut self,
        f: &FlowState,
        t: TypeHandle,
        expr: ast::Node,
        assume_true: bool,
    ) -> TypeHandle {
        // for `a?.b`, we emulate a synthetic `a !== null && a !== undefined` condition for `a`
        let store = self.store_for_node(expr);
        let parent = store.parent(expr);
        let is_nullish_left = parent.as_ref().is_some_and(|parent| {
            let parent_store = self.store_for_node(*parent);
            if !ast::is_binary_expression(parent_store, *parent) {
                return false;
            }
            let operator = parent_store.kind(parent_store.operator_token(*parent).unwrap());
            (operator == ast::Kind::QuestionQuestionToken
                || operator == ast::Kind::QuestionQuestionEqualsToken)
                && parent_store.left(*parent).as_ref() == Some(&expr)
        });
        if ast::is_expression_of_optional_chain_root(store, expr) || is_nullish_left {
            return self.narrow_type_by_optionality(f, t, expr, assume_true);
        }
        match store.kind(expr) {
            ast::Kind::Identifier => {
                // When narrowing a reference to a const variable, non-assigned parameter, or readonly property, we inline
                // up to five levels of aliased conditional expressions that are themselves declared as const variables.
                if !self.is_matching_reference(*f.reference.as_ref().unwrap(), expr)
                    && self.inline_level() < 5
                {
                    let symbol = self.get_resolved_symbol(expr);
                    if self.is_constant_variable_identity(symbol) {
                        let declaration = self.symbol_identity_value_declaration(symbol);
                        if let Some(declaration) = declaration {
                            let declaration_store = self.store_for_node(declaration);
                            let declaration_type_node = declaration_store.type_node(declaration);
                            let declaration_initializer =
                                declaration_store.initializer(declaration);
                            if ast::is_variable_declaration(declaration_store, declaration)
                                && declaration_type_node.is_none()
                                && declaration_initializer.is_some()
                                && self.is_constant_reference(*f.reference.as_ref().unwrap())
                            {
                                self.enter_inline();
                                let initializer = declaration_initializer.unwrap();
                                let result = self.narrow_type(f, t, initializer, assume_true);
                                self.exit_inline();
                                return result;
                            }
                        }
                    }
                }
                self.narrow_type_by_truthiness(f, t, expr, assume_true)
            }
            ast::Kind::ThisKeyword
            | ast::Kind::SuperKeyword
            | ast::Kind::PropertyAccessExpression
            | ast::Kind::ElementAccessExpression => {
                self.narrow_type_by_truthiness(f, t, expr, assume_true)
            }
            ast::Kind::CallExpression => {
                self.narrow_type_by_call_expression(f, t, expr, assume_true)
            }
            ast::Kind::ParenthesizedExpression
            | ast::Kind::NonNullExpression
            | ast::Kind::SatisfiesExpression => {
                let expression = self.store_for_node(expr).expression(expr).unwrap();
                self.narrow_type(f, t, expression, assume_true)
            }
            ast::Kind::BinaryExpression => {
                self.narrow_type_by_binary_expression(f, t, expr, assume_true)
            }
            ast::Kind::PrefixUnaryExpression => {
                if self.store_for_node(expr).operator(expr) == Some(ast::Kind::ExclamationToken) {
                    return self.narrow_type(
                        f,
                        t,
                        self.store_for_node(expr).operand(expr).unwrap(),
                        !assume_true,
                    );
                }
                t
            }
            _ => t,
        }
    }

    fn narrow_type_by_optionality(
        &mut self,
        f: &FlowState,
        t: TypeHandle,
        expr: ast::Node,
        assume_present: bool,
    ) -> TypeHandle {
        if self.is_matching_reference(*f.reference.as_ref().unwrap(), expr) {
            return self.get_adjusted_type_with_facts(
                t,
                core::if_else(
                    assume_present,
                    TYPE_FACTS_NE_UNDEFINED_OR_NULL,
                    TYPE_FACTS_EQ_UNDEFINED_OR_NULL,
                ),
            );
        }
        let access = self.get_discriminant_property_access(f, expr, t);
        if let Some(access) = access {
            return self.narrow_type_by_discriminant(t, access, |checker, tt| {
                checker.get_type_with_facts(
                    tt,
                    core::if_else(
                        assume_present,
                        TYPE_FACTS_NE_UNDEFINED_OR_NULL,
                        TYPE_FACTS_EQ_UNDEFINED_OR_NULL,
                    ),
                )
            });
        }
        t
    }

    fn narrow_type_by_truthiness(
        &mut self,
        f: &FlowState,
        mut t: TypeHandle,
        expr: ast::Node,
        assume_true: bool,
    ) -> TypeHandle {
        if self.is_matching_reference(*f.reference.as_ref().unwrap(), expr) {
            return self.get_adjusted_type_with_facts(
                t,
                core::if_else(assume_true, TYPE_FACTS_TRUTHY, TYPE_FACTS_FALSY),
            );
        }
        if self.strict_null_checks()
            && assume_true
            && self.optional_chain_contains_reference(expr, *f.reference.as_ref().unwrap())
        {
            t = self.get_adjusted_type_with_facts(t, TYPE_FACTS_NE_UNDEFINED_OR_NULL);
        }
        let access = self.get_discriminant_property_access(f, expr, t);
        if let Some(access) = access {
            return self.narrow_type_by_discriminant(t, access, |checker, tt| {
                checker.get_type_with_facts(
                    tt,
                    core::if_else(assume_true, TYPE_FACTS_TRUTHY, TYPE_FACTS_FALSY),
                )
            });
        }
        t
    }

    fn narrow_type_by_call_expression(
        &mut self,
        f: &FlowState,
        t: TypeHandle,
        call_expression: ast::Node,
        assume_true: bool,
    ) -> TypeHandle {
        if self.has_matching_argument(call_expression, *f.reference.as_ref().unwrap()) {
            let mut predicate: Option<TypePredicateHandle> = None;
            if assume_true || !is_call_chain(self.store_for_node(call_expression), call_expression)
            {
                let signature = self.get_effects_signature(call_expression);
                if let Some(signature) = signature {
                    predicate = self.get_type_predicate_of_signature(signature);
                }
            }
            if let Some(predicate) = predicate {
                let predicate_record = self.type_predicate_record(predicate);
                if predicate_record.kind == TYPE_PREDICATE_KIND_THIS
                    || predicate_record.kind == TYPE_PREDICATE_KIND_IDENTIFIER
                {
                    return self.narrow_type_by_type_predicate(
                        f,
                        t,
                        predicate,
                        call_expression,
                        assume_true,
                    );
                }
            }
        }
        if self.contains_missing_type(t)
            && ast::is_access_expression(
                self.store_for_node(*f.reference.as_ref().unwrap()),
                *f.reference.as_ref().unwrap(),
            )
            && self
                .store_for_node(call_expression)
                .expression(call_expression)
                .is_some_and(|expression| {
                    ast::is_property_access_expression(self.store_for_node(expression), expression)
                })
        {
            let call_expression_store = self.store_for_node(call_expression);
            let call_access = call_expression_store.expression(call_expression).unwrap();
            let reference = f.reference.as_ref().unwrap();
            let reference_expression = self
                .store_for_node(*reference)
                .expression(*reference)
                .unwrap();
            let call_access_expression = self
                .store_for_node(call_access)
                .expression(call_access)
                .unwrap();
            let reference_expression = reference_expression;
            let call_access_expression = call_access_expression;
            let call_access_store = self.store_for_node(call_access);
            let call_access_name = call_access_store.name(call_access);
            let call_arguments = call_expression_store.arguments(call_expression).unwrap();
            let is_has_own_property_access = call_access_name.as_ref().is_some_and(|name| {
                ast::is_identifier(call_access_store, *name)
                    && call_access_store.text(*name) == "hasOwnProperty"
            });
            let reference_candidate = self.get_reference_candidate(call_access_expression);
            if self.is_matching_reference(reference_expression, reference_candidate)
                && is_has_own_property_access
                && call_arguments.len() == 1
            {
                let argument = call_arguments.first().unwrap();
                let (accessed_name, ok) =
                    self.get_accessed_property_name(*f.reference.as_ref().unwrap());
                if ok
                    && ast::is_string_literal_like(self.store_for_node(argument), argument)
                    && accessed_name == self.store_for_node(argument).text(argument)
                {
                    return self.get_type_with_facts(
                        t,
                        core::if_else(
                            assume_true,
                            TYPE_FACTS_NE_UNDEFINED,
                            TYPE_FACTS_EQ_UNDEFINED,
                        ),
                    );
                }
            }
        }
        t
    }

    fn narrow_type_by_binary_expression(
        &mut self,
        f: &FlowState,
        t: TypeHandle,
        expr: ast::Node,
        assume_true: bool,
    ) -> TypeHandle {
        let store = self.store_for_node(expr);
        let operator = store.kind(store.operator_token(expr).unwrap());
        let left_node = store.left(expr).unwrap();
        let right_node = store.right(expr).unwrap();
        let left_expr = left_node;
        let right_expr = right_node;
        match operator {
            ast::Kind::EqualsToken
            | ast::Kind::BarBarEqualsToken
            | ast::Kind::AmpersandAmpersandEqualsToken
            | ast::Kind::QuestionQuestionEqualsToken => {
                let narrowed = self.narrow_type(f, t, right_expr, assume_true);
                self.narrow_type_by_truthiness(f, narrowed, left_expr, assume_true)
            }
            ast::Kind::EqualsEqualsToken
            | ast::Kind::ExclamationEqualsToken
            | ast::Kind::EqualsEqualsEqualsToken
            | ast::Kind::ExclamationEqualsEqualsToken => {
                let left = self.get_reference_candidate(left_expr);
                let right = self.get_reference_candidate(right_expr);
                if self.store_for_node(left).kind(left) == ast::Kind::TypeOfExpression
                    && ast::is_string_literal_like(self.store_for_node(right), right)
                {
                    return self.narrow_type_by_typeof(f, t, left, operator, right, assume_true);
                }
                if self.store_for_node(right).kind(right) == ast::Kind::TypeOfExpression
                    && ast::is_string_literal_like(self.store_for_node(left), left)
                {
                    return self.narrow_type_by_typeof(f, t, right, operator, left, assume_true);
                }
                if self.is_matching_reference(*f.reference.as_ref().unwrap(), left) {
                    return self.narrow_type_by_equality(t, operator, right, assume_true);
                }
                if self.is_matching_reference(*f.reference.as_ref().unwrap(), right) {
                    return self.narrow_type_by_equality(t, operator, left, assume_true);
                }
                let mut current = t;
                if self.strict_null_checks() {
                    if self.optional_chain_contains_reference(left, *f.reference.as_ref().unwrap())
                    {
                        current = self.narrow_type_by_optional_chain_containment(
                            f,
                            current,
                            operator,
                            right,
                            assume_true,
                        );
                    } else if self
                        .optional_chain_contains_reference(right, *f.reference.as_ref().unwrap())
                    {
                        current = self.narrow_type_by_optional_chain_containment(
                            f,
                            current,
                            operator,
                            left,
                            assume_true,
                        );
                    }
                }
                let left_access = self.get_discriminant_property_access(f, left, current);
                if let Some(left_access) = left_access {
                    return self.narrow_type_by_discriminant_property(
                        current,
                        left_access,
                        operator,
                        right,
                        assume_true,
                    );
                }
                let right_access = self.get_discriminant_property_access(f, right, current);
                if let Some(right_access) = right_access {
                    return self.narrow_type_by_discriminant_property(
                        current,
                        right_access,
                        operator,
                        left,
                        assume_true,
                    );
                }
                if self.is_matching_constructor_reference(f, left) {
                    return self.narrow_type_by_constructor(current, operator, right, assume_true);
                }
                if self.is_matching_constructor_reference(f, right) {
                    return self.narrow_type_by_constructor(current, operator, left, assume_true);
                }
                if ast::is_boolean_literal(self.store_for_node(right), right)
                    && !ast::is_access_expression(self.store_for_node(left), left)
                {
                    return self.narrow_type_by_boolean_comparison(
                        f,
                        current,
                        left,
                        right,
                        operator,
                        assume_true,
                    );
                }
                if ast::is_boolean_literal(self.store_for_node(left), left)
                    && !ast::is_access_expression(self.store_for_node(right), right)
                {
                    return self.narrow_type_by_boolean_comparison(
                        f,
                        current,
                        right,
                        left,
                        operator,
                        assume_true,
                    );
                }
                current
            }
            ast::Kind::InstanceOfKeyword => self.narrow_type_by_instanceof(f, t, expr, assume_true),
            ast::Kind::InKeyword => {
                if ast::is_private_identifier(self.store_for_node(left_expr), left_expr) {
                    return self.narrow_type_by_private_identifier_in_in_expression(
                        f,
                        t,
                        expr,
                        assume_true,
                    );
                }
                let target = self.get_reference_candidate(right_expr);
                if self.contains_missing_type(t)
                    && ast::is_access_expression(
                        self.store_for_node(*f.reference.as_ref().unwrap()),
                        *f.reference.as_ref().unwrap(),
                    )
                    && self.is_matching_reference(
                        {
                            let reference = f.reference.unwrap();
                            self.store_for_node(reference)
                                .expression(reference)
                                .unwrap()
                        },
                        target,
                    )
                {
                    let left_type = self.get_type_of_expression(left_expr);
                    if self.is_type_usable_as_property_name(left_type) {
                        let (accessed_name, ok) =
                            self.get_accessed_property_name(*f.reference.as_ref().unwrap());
                        if ok && accessed_name == self.get_property_name_from_type(left_type) {
                            return self.get_type_with_facts(
                                t,
                                core::if_else(
                                    assume_true,
                                    TYPE_FACTS_NE_UNDEFINED,
                                    TYPE_FACTS_EQ_UNDEFINED,
                                ),
                            );
                        }
                    }
                }
                if self.is_matching_reference(*f.reference.as_ref().unwrap(), target) {
                    let left_type = self.get_type_of_expression(left_expr);
                    if self.is_type_usable_as_property_name(left_type) {
                        return self.narrow_type_by_in_keyword(f, t, left_type, assume_true);
                    }
                }
                t
            }
            ast::Kind::CommaToken => self.narrow_type(f, t, right_expr, assume_true),
            ast::Kind::AmpersandAmpersandToken => {
                // Ordinarily we won't see && and || expressions in control flow analysis because the Binder breaks those
                // expressions down to individual conditional control flows. However, we may encounter them when analyzing
                // aliased conditional expressions.
                if assume_true {
                    let left = self.narrow_type(f, t, left_expr, true /*assumeTrue*/);
                    return self.narrow_type(f, left, right_expr, true /*assumeTrue*/);
                }
                let left = self.narrow_type(f, t, left_expr, false /*assumeTrue*/);
                let right = self.narrow_type(f, t, right_expr, false /*assumeTrue*/);
                self.get_union_type(vec![left, right])
            }
            ast::Kind::BarBarToken => {
                if assume_true {
                    let left = self.narrow_type(f, t, left_expr, true /*assumeTrue*/);
                    let right = self.narrow_type(f, t, right_expr, true /*assumeTrue*/);
                    return self.get_union_type(vec![left, right]);
                }
                let left = self.narrow_type(f, t, left_expr, false /*assumeTrue*/);
                self.narrow_type(f, left, right_expr, false /*assumeTrue*/)
            }
            _ => t,
        }
    }

    fn narrow_type_by_equality(
        &mut self,
        t: TypeHandle,
        operator: ast::Kind,
        value: ast::Node,
        mut assume_true: bool,
    ) -> TypeHandle {
        if self.type_flags(t) & TYPE_FLAGS_ANY != 0 {
            return t;
        }
        if operator == ast::Kind::ExclamationEqualsToken
            || operator == ast::Kind::ExclamationEqualsEqualsToken
        {
            assume_true = !assume_true;
        }
        let value_type = self.get_type_of_expression(value);
        let double_equals = operator == ast::Kind::EqualsEqualsToken
            || operator == ast::Kind::ExclamationEqualsToken;
        if self.type_flags(value_type) & TYPE_FLAGS_NULLABLE != 0 {
            if !self.strict_null_checks() {
                return t;
            }
            let facts = if double_equals {
                core::if_else(
                    assume_true,
                    TYPE_FACTS_EQ_UNDEFINED_OR_NULL,
                    TYPE_FACTS_NE_UNDEFINED_OR_NULL,
                )
            } else if self.type_flags(value_type) & TYPE_FLAGS_NULL != 0 {
                core::if_else(assume_true, TYPE_FACTS_EQ_NULL, TYPE_FACTS_NE_NULL)
            } else {
                core::if_else(
                    assume_true,
                    TYPE_FACTS_EQ_UNDEFINED,
                    TYPE_FACTS_NE_UNDEFINED,
                )
            };
            return self.get_adjusted_type_with_facts(t, facts);
        }
        if assume_true {
            if !double_equals
                && (self.type_flags(t) & TYPE_FLAGS_UNKNOWN != 0
                    || some_type(self, t, |checker, tt| {
                        checker.is_empty_anonymous_object_type(tt)
                    }))
            {
                if self.type_flags(value_type) & (TYPE_FLAGS_PRIMITIVE | TYPE_FLAGS_NON_PRIMITIVE)
                    != 0
                    || self.is_empty_anonymous_object_type(value_type)
                {
                    return value_type;
                }
                if self.type_flags(value_type) & TYPE_FLAGS_OBJECT != 0 {
                    return self.semantic_state.semantic_handles().non_primitive_type;
                }
            }
            let filtered_type = self.filter_type_with_checker(t, |checker, tt| {
                checker.are_types_comparable(tt, value_type)
                    || double_equals && checker.is_coercible_under_double_equals(tt, value_type)
            });
            return self.replace_primitives_with_literals(filtered_type, value_type);
        }
        if self.is_unit_type(value_type) {
            return self.filter_type_with_checker(t, |checker, tt| {
                !(checker.is_unit_like_type(tt) && checker.are_types_comparable(tt, value_type))
            });
        }
        t
    }

    fn narrow_type_by_typeof(
        &mut self,
        f: &FlowState,
        mut t: TypeHandle,
        type_of_expr: ast::Node,
        operator: ast::Kind,
        literal: ast::Node,
        mut assume_true: bool,
    ) -> TypeHandle {
        // We have '==', '!=', '===', or !==' operator with 'typeof xxx' and string literal operands
        if operator == ast::Kind::ExclamationEqualsToken
            || operator == ast::Kind::ExclamationEqualsEqualsToken
        {
            assume_true = !assume_true;
        }
        let type_of_expression = self
            .store_for_node(type_of_expr)
            .expression(type_of_expr)
            .unwrap();
        let target = self.get_reference_candidate(type_of_expression);
        if !self.is_matching_reference(*f.reference.as_ref().unwrap(), target) {
            let adjusted;
            if self.strict_null_checks()
                && self.optional_chain_contains_reference(target, *f.reference.as_ref().unwrap())
                && assume_true == (self.store_for_node(literal).text(literal) != "undefined")
            {
                adjusted = self.get_adjusted_type_with_facts(t, TYPE_FACTS_NE_UNDEFINED_OR_NULL);
                t = adjusted;
            }
            let property_access = self.get_discriminant_property_access(f, target, t);
            if let Some(property_access) = property_access {
                return self.narrow_type_by_discriminant(t, property_access, |checker, tt| {
                    checker.narrow_type_by_literal_expression(tt, literal, assume_true)
                });
            }
            return t;
        }
        self.narrow_type_by_literal_expression(t, literal, assume_true)
    }

    fn narrow_type_by_literal_expression(
        &mut self,
        t: TypeHandle,
        literal: ast::Node,
        assume_true: bool,
    ) -> TypeHandle {
        if assume_true {
            let text = self.store_for_node(literal).text(literal);
            return self.narrow_type_by_type_name(t, &text);
        }
        let text = self.store_for_node(literal).text(literal);
        let facts = typeof_ne_facts(&text).unwrap_or(TYPE_FACTS_TYPEOF_NE_HOST_OBJECT);
        self.get_adjusted_type_with_facts(t, facts)
    }

    fn narrow_type_by_type_name(&mut self, t: TypeHandle, type_name: &str) -> TypeHandle {
        match type_name {
            "string" => self.narrow_type_by_type_facts(
                t,
                self.semantic_state.semantic_handles().string_type,
                TYPE_FACTS_TYPEOF_EQ_STRING,
            ),
            "number" => self.narrow_type_by_type_facts(
                t,
                self.semantic_state.semantic_handles().number_type,
                TYPE_FACTS_TYPEOF_EQ_NUMBER,
            ),
            "bigint" => self.narrow_type_by_type_facts(
                t,
                self.semantic_state.semantic_handles().bigint_type,
                TYPE_FACTS_TYPEOF_EQ_BIG_INT,
            ),
            "boolean" => self.narrow_type_by_type_facts(
                t,
                self.semantic_state.semantic_handles().boolean_type,
                TYPE_FACTS_TYPEOF_EQ_BOOLEAN,
            ),
            "symbol" => self.narrow_type_by_type_facts(
                t,
                self.semantic_state.semantic_handles().es_symbol_type,
                TYPE_FACTS_TYPEOF_EQ_SYMBOL,
            ),
            "object" => {
                if self.type_flags(t) & TYPE_FLAGS_ANY != 0 {
                    return t;
                }
                let object = self.narrow_type_by_type_facts(
                    t,
                    self.semantic_state.semantic_handles().non_primitive_type,
                    TYPE_FACTS_TYPEOF_EQ_OBJECT,
                );
                let null = self.narrow_type_by_type_facts(
                    t,
                    self.semantic_state.semantic_handles().null_type,
                    TYPE_FACTS_EQ_NULL,
                );
                self.get_union_type(vec![object, null])
            }
            "function" => {
                if self.type_flags(t) & TYPE_FLAGS_ANY != 0 {
                    return t;
                }
                self.narrow_type_by_type_facts(
                    t,
                    self.semantic_state.semantic_handles().global_function_type,
                    TYPE_FACTS_TYPEOF_EQ_FUNCTION,
                )
            }
            "undefined" => self.narrow_type_by_type_facts(
                t,
                self.semantic_state.semantic_handles().undefined_type,
                TYPE_FACTS_EQ_UNDEFINED,
            ),
            _ => self.narrow_type_by_type_facts(
                t,
                self.semantic_state.semantic_handles().non_primitive_type,
                TYPE_FACTS_TYPEOF_EQ_HOST_OBJECT,
            ),
        }
    }

    fn narrow_type_by_type_facts(
        &mut self,
        t: TypeHandle,
        implied_type: TypeHandle,
        facts: TypeFacts,
    ) -> TypeHandle {
        self.map_type(t, |checker, tt| {
            if checker.is_type_related_to(
                tt,
                implied_type,
                checker.semantic_state.strict_subtype_relation,
            ) {
                if checker.has_type_facts(tt, facts) {
                    return tt;
                }
                return checker.semantic_state.semantic_handles().never_type;
            }
            if checker.is_type_subtype_of(implied_type, tt) {
                return implied_type;
            }
            if checker.has_type_facts(tt, facts) {
                return checker.get_intersection_type(vec![tt, implied_type]);
            }
            checker.semantic_state.semantic_handles().never_type
        })
    }

    fn narrow_type_by_discriminant_property(
        &mut self,
        t: TypeHandle,
        access: ast::Node,
        operator: ast::Kind,
        value: ast::Node,
        assume_true: bool,
    ) -> TypeHandle {
        if (operator == ast::Kind::EqualsEqualsEqualsToken
            || operator == ast::Kind::ExclamationEqualsEqualsToken)
            && self.type_flags(t) & TYPE_FLAGS_UNION != 0
        {
            let key_property_name = self.get_key_property_name(t);
            if !key_property_name.is_empty() {
                let (accessed_name, ok) = self.get_accessed_property_name(access);
                if ok && key_property_name == accessed_name {
                    let value_type = self.get_type_of_expression(value);
                    let candidate = self.get_constituent_type_for_key_type(t, value_type);
                    if let Some(candidate) = candidate {
                        if assume_true && operator == ast::Kind::EqualsEqualsEqualsToken
                            || !assume_true && operator == ast::Kind::ExclamationEqualsEqualsToken
                        {
                            return candidate;
                        }
                        if let Some(prop_type) =
                            self.get_type_of_property_of_type(candidate, &key_property_name)
                        {
                            if self.is_unit_type(prop_type) {
                                return self.remove_type(t, candidate);
                            }
                        }
                        return t;
                    }
                }
            }
        }
        self.narrow_type_by_discriminant(t, access, |checker, tt| {
            checker.narrow_type_by_equality(tt, operator, value, assume_true)
        })
    }

    fn narrow_type_by_discriminant<F>(
        &mut self,
        t: TypeHandle,
        access: ast::Node,
        mut narrow_type: F,
    ) -> TypeHandle
    where
        F: FnMut(&mut Checker<'a, '_>, TypeHandle) -> TypeHandle,
    {
        let (prop_name, ok) = self.get_accessed_property_name(access);
        if !ok {
            return t;
        }
        let optional_chain = ast::is_optional_chain(self.store_for_node(access), access);
        let remove_nullable = self.strict_null_checks()
            && (optional_chain || is_non_null_access(self.store_for_node(access), access))
            && self.maybe_type_of_kind(t, TYPE_FLAGS_NULLABLE);
        let mut non_null_type = t;
        if remove_nullable {
            non_null_type = self.get_type_with_facts(t, TYPE_FACTS_NE_UNDEFINED_OR_NULL);
        }
        let prop_type = self.get_type_of_property_of_type(non_null_type, &prop_name);
        let Some(mut prop_type) = prop_type else {
            return t;
        };
        if remove_nullable && optional_chain {
            prop_type = self.get_optional_type(prop_type, false);
        }
        let narrowed_prop_type = narrow_type(self, prop_type);
        self.filter_type_with_checker(t, |checker, tt| {
            let discriminant_type = checker
                .get_type_of_property_or_index_signature_of_type(tt, &prop_name)
                .unwrap_or(checker.semantic_state.semantic_handles().unknown_type);
            checker.type_flags(discriminant_type) & TYPE_FLAGS_NEVER == 0
                && checker.type_flags(narrowed_prop_type) & TYPE_FLAGS_NEVER == 0
                && checker.are_types_comparable(narrowed_prop_type, discriminant_type)
        })
    }

    fn is_matching_constructor_reference(&mut self, f: &FlowState, expr: ast::Node) -> bool {
        if ast::is_access_expression(self.store_for_node(expr), expr) {
            let (accessed_name, ok) = self.get_accessed_property_name(expr);
            let expression = self.store_for_node(expr).expression(expr);
            let expression_matches = expression.is_some_and(|expression| {
                self.is_matching_reference(*f.reference.as_ref().unwrap(), expression)
            });
            if ok && accessed_name == "constructor" && expression_matches {
                return true;
            }
        }
        false
    }

    fn narrow_type_by_constructor(
        &mut self,
        t: TypeHandle,
        operator: ast::Kind,
        identifier: ast::Node,
        assume_true: bool,
    ) -> TypeHandle {
        // Do not narrow when checking inequality.
        if assume_true
            && operator != ast::Kind::EqualsEqualsToken
            && operator != ast::Kind::EqualsEqualsEqualsToken
            || !assume_true
                && operator != ast::Kind::ExclamationEqualsToken
                && operator != ast::Kind::ExclamationEqualsEqualsToken
        {
            return t;
        }
        // Get the type of the constructor identifier expression, if it is not a function then do not narrow.
        let identifier_type = self.get_type_of_expression(identifier);
        if !self.is_function_type(identifier_type) && !self.is_constructor_type(identifier_type) {
            return t;
        }
        // Get the prototype property of the type identifier so we can find out its type.
        let prototype_property = self.get_property_of_type(identifier_type, "prototype");
        let Some(prototype_property) = prototype_property else {
            return t;
        };
        // Get the type of the prototype, if it is undefined, or the global `Object` or `Function` types then do not narrow.
        let prototype_type = self.get_type_of_symbol_at_location(prototype_property, None);
        let mut candidate: Option<TypeHandle> = None;
        if !is_type_any(self, Some(prototype_type)) {
            candidate = Some(prototype_type);
        }
        let Some(candidate) = candidate else {
            return t;
        };
        if candidate == self.semantic_state.semantic_handles().global_object_type
            || candidate == self.semantic_state.semantic_handles().global_function_type
        {
            return t;
        }
        // If the type that is being narrowed is `any` then just return the `candidate` type since every type is a subtype of `any`.
        if is_type_any(self, Some(t)) {
            return candidate;
        }
        // Filter out types that are not considered to be "constructed by" the `candidate` type.
        self.filter_type_with_checker(t, |checker, tt| checker.is_constructed_by(tt, candidate))
    }

    fn is_constructed_by(&mut self, source: TypeHandle, target: TypeHandle) -> bool {
        // If either the source or target type are a class type then we need to check that they are the same exact type.
        // This is because you may have a class `A` that defines some set of properties, and another class `B`
        // that defines the same set of properties as class `A`, in that case they are structurally the same
        // type, but when you do something like `instanceOfA.constructor === B` it will return false.
        if self.type_flags(source) & TYPE_FLAGS_OBJECT != 0
            && self.object_flags(source) & OBJECT_FLAGS_CLASS != 0
            || self.type_flags(target) & TYPE_FLAGS_OBJECT != 0
                && self.object_flags(target) & OBJECT_FLAGS_CLASS != 0
        {
            return self.same_optional_symbol_identity(
                self.type_symbol_identity(source),
                self.type_symbol_identity(target),
            );
        }
        // For all other types just check that the `source` type is a subtype of the `target` type.
        self.is_type_subtype_of(source, target)
    }

    fn narrow_type_by_boolean_comparison(
        &mut self,
        f: &FlowState,
        t: TypeHandle,
        expr: ast::Node,
        bool_value: ast::Node,
        operator: ast::Kind,
        mut assume_true: bool,
    ) -> TypeHandle {
        assume_true = (assume_true
            != (self.store_for_node(bool_value).kind(bool_value) == ast::Kind::TrueKeyword))
            != (operator != ast::Kind::ExclamationEqualsEqualsToken
                && operator != ast::Kind::ExclamationEqualsToken);
        self.narrow_type(f, t, expr, assume_true)
    }

    fn narrow_type_by_instanceof(
        &mut self,
        f: &FlowState,
        t: TypeHandle,
        expr: ast::Node,
        assume_true: bool,
    ) -> TypeHandle {
        let store = self.store_for_node(expr);
        let left_node = store.left(expr).unwrap();
        let right_node = store.right(expr).unwrap();
        let left = self.get_reference_candidate(left_node);
        if !self.is_matching_reference(*f.reference.as_ref().unwrap(), left) {
            if assume_true
                && self.strict_null_checks()
                && self.optional_chain_contains_reference(left, *f.reference.as_ref().unwrap())
            {
                return self.get_adjusted_type_with_facts(t, TYPE_FACTS_NE_UNDEFINED_OR_NULL);
            }
            return t;
        }
        let right = right_node;
        let right_type = self.get_type_of_expression(right);
        let global_object_type = self.semantic_state.semantic_handles().global_object_type;
        let global_function_type = self.semantic_state.semantic_handles().global_function_type;
        if global_object_type == self.semantic_state.semantic_handles().empty_object_type
            || global_function_type == self.semantic_state.semantic_handles().empty_object_type
        {
            return t;
        }
        if !self.is_type_derived_from(right_type, global_object_type) {
            return t;
        }
        // if the right-hand side has an object type with a custom `[Symbol.hasInstance]` method, and that method
        // has a type predicate, use the type predicate to perform narrowing. This allows normal `object` types to
        // participate in `instanceof`, as per Step 2 of https://tc39.es/ecma262/#sec-instanceofoperator.
        let mut predicate: Option<TypePredicateHandle> = None;
        if let Some(signature) = self.get_effects_signature(expr) {
            predicate = self.get_type_predicate_of_signature(signature);
        }
        if let Some(predicate) = predicate {
            let predicate_record = self.type_predicate_record(predicate);
            if predicate_record.kind == TYPE_PREDICATE_KIND_IDENTIFIER
                && predicate_record.parameter_index == 0
            {
                return self.get_narrowed_type(
                    t,
                    *predicate_record.t.as_ref().unwrap(),
                    assume_true,
                    true, /*checkDerived*/
                );
            }
        }
        if !self.is_type_derived_from(right_type, global_function_type) {
            return t;
        }
        let instance_type = self.map_type(right_type, |checker, constructor_type| {
            checker.get_instance_type(constructor_type)
        });
        // Don't narrow from `any` if the target type is exactly `Object` or `Function`, and narrow
        // in the false branch only if the target is a non-empty object type.
        if is_type_any(self, Some(t))
            && (instance_type == global_object_type || instance_type == global_function_type)
            || !assume_true
                && !(self.type_flags(instance_type) & TYPE_FLAGS_OBJECT != 0
                    && !self.is_empty_anonymous_object_type(instance_type))
        {
            return t;
        }
        self.get_narrowed_type(t, instance_type, assume_true, true /*checkDerived*/)
    }

    fn get_narrowed_type(
        &mut self,
        t: TypeHandle,
        candidate: TypeHandle,
        assume_true: bool,
        check_derived: bool,
    ) -> TypeHandle {
        if self.type_flags(t) & TYPE_FLAGS_UNION == 0 {
            return self.get_narrowed_type_worker(t, candidate, assume_true, check_derived);
        }
        let key = NarrowedTypeKey {
            t: t,
            candidate: candidate,
            assume_true,
            check_derived,
        };
        if let Some(narrowed_type) = self.semantic_state.narrowed_type(key) {
            return narrowed_type;
        }
        let narrowed_type = self.get_narrowed_type_worker(t, candidate, assume_true, check_derived);
        self.semantic_state.set_narrowed_type(key, narrowed_type);
        narrowed_type
    }

    fn get_narrowed_type_worker(
        &mut self,
        mut t: TypeHandle,
        candidate: TypeHandle,
        assume_true: bool,
        check_derived: bool,
    ) -> TypeHandle {
        if !assume_true {
            if t == candidate {
                return self.semantic_state.semantic_handles().never_type;
            }
            if check_derived {
                return self.filter_type_with_checker(t, |checker, tt| {
                    !checker.is_type_derived_from(tt, candidate)
                });
            }
            if self.type_flags(t) & TYPE_FLAGS_UNKNOWN != 0 {
                t = self.semantic_state.semantic_handles().unknown_union_type;
            }
            let true_type = self.get_narrowed_type(
                t, candidate, true,  /*assumeTrue*/
                false, /*checkDerived*/
            );
            let filtered = self.filter_type_with_checker(t, |checker, tt| {
                !checker.is_type_subset_of(tt, true_type)
            });
            return self.recombine_unknown_type(filtered);
        }
        if self.type_flags(t) & TYPE_FLAGS_ANY_OR_UNKNOWN != 0 {
            return candidate;
        }
        if t == candidate {
            return candidate;
        }
        // We first attempt to filter the current type, narrowing constituents as appropriate and removing
        // constituents that are unrelated to the candidate.
        let mut key_property_name = String::new();
        if self.type_flags(t) & TYPE_FLAGS_UNION != 0 {
            key_property_name = self.get_key_property_name(t);
        }
        let narrowed_type = self.map_type(candidate, |checker, n| {
            // If a discriminant property is available, use that to reduce the type.
            let mut matching = t;
            if !key_property_name.is_empty() {
                if let Some(discriminant) =
                    checker.get_type_of_property_of_type(n, &key_property_name)
                {
                    if let Some(constituent) =
                        checker.get_constituent_type_for_key_type(t, discriminant)
                    {
                        matching = constituent;
                    }
                }
            }
            // For each constituent t in the current type, if t and c are directly related, pick the most
            // specific of the two. When t and c are related in both directions, we prefer c for type predicates
            // because that is the asserted type, but t for `instanceof` because generics aren't reflected in
            // prototype object types.
            let directly_related = if check_derived {
                checker.map_type(matching, |checker, tt| {
                    if checker.is_type_derived_from(tt, n) {
                        return tt;
                    }
                    if checker.is_type_derived_from(n, tt) {
                        return n;
                    }
                    checker.semantic_state.semantic_handles().never_type
                })
            } else {
                checker.map_type(matching, |checker, tt| {
                    if checker.is_type_strict_subtype_of(tt, n) {
                        return tt;
                    }
                    if checker.is_type_strict_subtype_of(n, tt) {
                        return n;
                    }
                    if checker.is_type_subtype_of(tt, n) {
                        return tt;
                    }
                    if checker.is_type_subtype_of(n, tt) {
                        return n;
                    }
                    checker.semantic_state.semantic_handles().never_type
                })
            };
            if checker.type_flags(directly_related) & TYPE_FLAGS_NEVER == 0 {
                return directly_related;
            }
            // If no constituents are directly related, create intersections for any generic constituents that
            // are related by constraint.
            checker.map_type(t, |checker, tt| {
                if checker.maybe_type_of_kind(tt, TYPE_FLAGS_INSTANTIABLE) {
                    let constraint = checker.get_base_constraint_of_type(tt);
                    let related = if check_derived {
                        constraint
                            .as_ref()
                            .is_none_or(|constraint| checker.is_type_derived_from(n, *constraint))
                    } else {
                        constraint
                            .as_ref()
                            .is_none_or(|constraint| checker.is_type_subtype_of(n, *constraint))
                    };
                    if related {
                        return checker.get_intersection_type(vec![tt, n]);
                    }
                }
                checker.semantic_state.semantic_handles().never_type
            })
        });
        // If filtering produced a non-empty type, return that. Otherwise, pick the most specific of the two
        // based on assignability, or as a last resort produce an intersection.
        if self.type_flags(narrowed_type) & TYPE_FLAGS_NEVER == 0 {
            narrowed_type
        } else if self.is_type_subtype_of(candidate, t) {
            candidate
        } else if self.is_type_assignable_to(t, candidate) {
            t
        } else if self.is_type_assignable_to(candidate, t) {
            candidate
        } else {
            self.get_intersection_type(vec![t, candidate])
        }
    }

    fn get_instance_type(&mut self, constructor_type: TypeHandle) -> TypeHandle {
        let prototype_property_type =
            self.get_type_of_property_of_type(constructor_type, "prototype");
        if let Some(prototype_property_type) = prototype_property_type {
            if !is_type_any(self, Some(prototype_property_type)) {
                return prototype_property_type;
            }
        }
        let construct_signatures =
            self.get_signatures_of_type(constructor_type, SIGNATURE_KIND_CONSTRUCT);
        if !construct_signatures.is_empty() {
            let mut returns = Vec::with_capacity(construct_signatures.len());
            for signature in construct_signatures {
                let erased = self.get_erased_signature(signature);
                let return_type = self.get_return_type_of_signature(erased);
                returns.push(return_type);
            }
            return self.get_union_type(returns);
        }
        // We use the empty object type to indicate we don't know the type of objects created by
        // this constructor function.
        self.semantic_state.semantic_handles().empty_object_type
    }

    fn narrow_type_by_private_identifier_in_in_expression(
        &mut self,
        f: &FlowState,
        t: TypeHandle,
        expr: ast::Node,
        assume_true: bool,
    ) -> TypeHandle {
        let store = self.store_for_node(expr);
        let left = store.left(expr).unwrap();
        let right = store.right(expr).unwrap();
        let target = self.get_reference_candidate(right);
        if !self.is_matching_reference(*f.reference.as_ref().unwrap(), target) {
            return t;
        }
        let symbol = self.get_symbol_for_private_identifier_expression(left);
        let Some(symbol) = symbol else {
            return t;
        };
        let class_symbol = self.symbol_identity_parent(symbol).unwrap();
        let value_declaration = self.symbol_identity_value_declaration(symbol).unwrap();
        let target_type = if ast::has_static_modifier(
            self.store_for_node(value_declaration),
            value_declaration,
        ) {
            self.get_type_of_symbol_identity_for_flow(class_symbol, None)
        } else {
            let constructor_type = self.get_type_of_symbol_identity_for_flow(class_symbol, None);
            self.get_instance_type(constructor_type)
        };
        self.get_narrowed_type(t, target_type, assume_true, true /*checkDerived*/)
    }

    fn narrow_type_by_in_keyword(
        &mut self,
        _f: &FlowState,
        t: TypeHandle,
        name_type: TypeHandle,
        assume_true: bool,
    ) -> TypeHandle {
        let name = self.get_property_name_from_type(name_type);
        let is_known_property = some_type(self, t, |checker, tt| {
            checker.is_type_presence_possible(tt, &name, true /*assumeTrue*/)
        });
        if is_known_property {
            // If the check is for a known property (i.e. a property declared in some constituent of
            // the target type), we filter the target type by presence of absence of the property.
            return self.filter_type_with_checker(t, |checker, tt| {
                checker.is_type_presence_possible(tt, &name, assume_true)
            });
        }
        if assume_true {
            // If the check is for an unknown property, we intersect the target type with `Record<X, unknown>`,
            // where X is the name of the property.
            let record_symbol = {
                let resolver = (self.semantic_state.get_global_record_symbol).clone();
                self.resolve_global_symbol(resolver)
            };
            if let Some(record_symbol) = record_symbol {
                let record = self.get_type_alias_instantiation(
                    record_symbol,
                    vec![
                        name_type,
                        self.semantic_state.semantic_handles().unknown_type,
                    ],
                    None,
                );
                return self.get_intersection_type(vec![t, record]);
            }
        }
        t
    }

    fn is_type_presence_possible(
        &mut self,
        t: TypeHandle,
        prop_name: &str,
        assume_true: bool,
    ) -> bool {
        let prop = self.get_property_of_type(t, prop_name);
        if let Some(prop) = prop {
            return self.missing_name_symbol_identity_flags(prop) & ast::SYMBOL_FLAGS_OPTIONAL != 0
                || self.missing_name_symbol_identity_check_flags(prop) & ast::CHECK_FLAGS_PARTIAL
                    != 0
                || assume_true;
        }
        self.get_applicable_index_info_for_name(t, prop_name)
            .is_some()
            || !assume_true
    }

    fn narrow_type_by_optional_chain_containment(
        &mut self,
        _f: &FlowState,
        t: TypeHandle,
        operator: ast::Kind,
        value: ast::Node,
        assume_true: bool,
    ) -> TypeHandle {
        // We are in a branch of obj?.foo === value (or any one of the other equality operators). We narrow obj as follows:
        // When operator is === and type of value excludes undefined, null and undefined is removed from type of obj in true branch.
        // When operator is !== and type of value excludes undefined, null and undefined is removed from type of obj in false branch.
        // When operator is == and type of value excludes null and undefined, null and undefined is removed from type of obj in true branch.
        // When operator is != and type of value excludes null and undefined, null and undefined is removed from type of obj in false branch.
        // When operator is === and type of value is undefined, null and undefined is removed from type of obj in false branch.
        // When operator is !== and type of value is undefined, null and undefined is removed from type of obj in true branch.
        // When operator is == and type of value is null or undefined, null and undefined is removed from type of obj in false branch.
        // When operator is != and type of value is null or undefined, null and undefined is removed from type of obj in true branch.
        let equals_operator = operator == ast::Kind::EqualsEqualsToken
            || operator == ast::Kind::EqualsEqualsEqualsToken;
        let nullable_flags = if operator == ast::Kind::EqualsEqualsToken
            || operator == ast::Kind::ExclamationEqualsToken
        {
            TYPE_FLAGS_NULLABLE
        } else {
            TYPE_FLAGS_UNDEFINED
        };
        let value_type = self.get_type_of_expression(value);
        // Note that we include any and unknown in the exclusion test because their domain includes null and undefined.
        let remove_nullable = equals_operator != assume_true
            && every_type(self, value_type, |checker, tt| {
                checker.type_flags(tt) & nullable_flags != 0
            })
            || equals_operator == assume_true
                && every_type(self, value_type, |checker, tt| {
                    checker.type_flags(tt) & (TYPE_FLAGS_ANY_OR_UNKNOWN | nullable_flags) == 0
                });
        if remove_nullable {
            return self.get_adjusted_type_with_facts(t, TYPE_FACTS_NE_UNDEFINED_OR_NULL);
        }
        t
    }

    fn get_type_at_switch_clause(
        &mut self,
        f: &mut FlowState,
        graph: &ast::FlowGraph,
        flow: &ast::FlowNode,
    ) -> FlowType {
        let data = flow_node_reference_switch_clause_data(flow.node.as_ref().unwrap());
        let switch_statement = data.switch_statement();
        let switch_expression = self
            .store_for_node(*switch_statement)
            .expression(*switch_statement)
            .unwrap();
        let switch_expression = switch_expression;
        let expr = ast::skip_parentheses(self.store_for_node(switch_expression), switch_expression);
        let flow_type = self.get_type_at_flow_node(f, graph, flow.antecedent.unwrap());
        let mut t = flow_type.t.unwrap();
        if self.is_matching_reference(*f.reference.as_ref().unwrap(), expr) {
            t = self.narrow_type_by_switch_on_discriminant(t, &data);
        } else if self.store_for_node(expr).kind(expr) == ast::Kind::TypeOfExpression
            && self.is_matching_reference(
                *f.reference.as_ref().unwrap(),
                self.store_for_node(expr).expression(expr).unwrap(),
            )
        {
            t = self.narrow_type_by_switch_on_type_of(t, &data);
        } else if self.store_for_node(expr).kind(expr) == ast::Kind::TrueKeyword {
            t = self.narrow_type_by_switch_on_true(f, t, &data);
        } else {
            if self.strict_null_checks() {
                if self.optional_chain_contains_reference(expr, *f.reference.as_ref().unwrap()) {
                    t = self.narrow_type_by_switch_optional_chain_containment(
                        t,
                        &data,
                        |checker, tt| {
                            checker.type_flags(tt) & (TYPE_FLAGS_UNDEFINED | TYPE_FLAGS_NEVER) == 0
                        },
                    );
                } else if ast::is_type_of_expression(self.store_for_node(expr), expr)
                    && self.optional_chain_contains_reference(
                        self.store_for_node(expr).expression(expr).unwrap(),
                        *f.reference.as_ref().unwrap(),
                    )
                {
                    t = self.narrow_type_by_switch_optional_chain_containment(
                        t,
                        &data,
                        |checker, tt| {
                            !(checker.type_flags(tt) & TYPE_FLAGS_NEVER != 0
                                || checker.type_flags(tt) & TYPE_FLAGS_STRING_LITERAL != 0
                                    && checker.get_string_literal_value(tt) == "undefined")
                        },
                    );
                }
            }
            let access = self.get_discriminant_property_access(f, expr, t);
            if let Some(access) = access {
                let access = access;
                t = self.narrow_type_by_switch_on_discriminant_property(t, access, &data);
            }
        }
        self.new_flow_type(t, flow_type.incomplete)
    }

    fn narrow_type_by_switch_on_discriminant(
        &mut self,
        t: TypeHandle,
        data: &ast::FlowSwitchClauseData,
    ) -> TypeHandle {
        // We only narrow if all case expressions specify
        // values with unit types, except for the case where
        // `type` is unknown. In this instance we map object
        // types to the nonPrimitive type and narrow with that.
        let switch_types = self.get_switch_clause_types(*data.switch_statement());
        if switch_types.is_empty() {
            return t;
        }
        let clause_types = &switch_types[data.clause_start as usize..data.clause_end as usize];
        let has_default_clause = data.clause_start == data.clause_end
            || clause_types.contains(&self.semantic_state.semantic_handles().never_type);
        if self.type_flags(t) & TYPE_FLAGS_UNKNOWN != 0 && !has_default_clause {
            let mut ground_clause_types: Option<Vec<TypeHandle>> = None;
            for (i, s) in clause_types.iter().enumerate() {
                if self.type_flags(*s) & (TYPE_FLAGS_PRIMITIVE | TYPE_FLAGS_NON_PRIMITIVE) != 0 {
                    if let Some(ground_clause_types) = ground_clause_types.as_mut() {
                        ground_clause_types.push(*s);
                    }
                } else if self.type_flags(*s) & TYPE_FLAGS_OBJECT != 0 {
                    if ground_clause_types.is_none() {
                        ground_clause_types = Some(clause_types[..i].to_vec());
                    }
                    ground_clause_types
                        .as_mut()
                        .unwrap()
                        .push(self.semantic_state.semantic_handles().non_primitive_type);
                } else {
                    return t;
                }
            }
            return self
                .get_union_type(ground_clause_types.unwrap_or_else(|| clause_types.to_vec()));
        }
        let discriminant_type = self.get_union_type(clause_types.to_vec());
        let case_type = if self.type_flags(discriminant_type) & TYPE_FLAGS_NEVER != 0 {
            self.semantic_state.semantic_handles().never_type
        } else {
            let filtered = self.filter_type_with_checker(t, |checker, tt| {
                checker.are_types_comparable(discriminant_type, tt)
            });
            self.replace_primitives_with_literals(filtered, discriminant_type)
        };
        if !has_default_clause {
            return case_type;
        }
        let default_type = self.filter_type_with_checker(t, |checker, tt| {
            if !checker.is_unit_like_type(tt) {
                return true;
            }
            let u = if checker.type_flags(tt) & TYPE_FLAGS_UNDEFINED == 0 {
                let unit_type = checker.extract_unit_type(tt);
                checker.get_regular_type_of_literal_type(unit_type)
            } else {
                checker.semantic_state.semantic_handles().undefined_type
            };
            !switch_types
                .iter()
                .any(|st| checker.is_unit_type(*st) && checker.are_types_comparable(*st, u))
        });
        if self.type_flags(case_type) & TYPE_FLAGS_NEVER != 0 {
            return default_type;
        }
        self.get_union_type(vec![case_type, default_type])
    }

    fn narrow_type_by_switch_on_type_of(
        &mut self,
        t: TypeHandle,
        data: &ast::FlowSwitchClauseData,
    ) -> TypeHandle {
        let witnesses = self.get_switch_clause_type_of_witnesses(*data.switch_statement());
        let Some(witnesses) = witnesses else {
            return t;
        };
        let switch_statement = data.switch_statement();
        let switch_store = self.store_for_node(*switch_statement);
        let case_block = switch_store.case_block(*switch_statement).unwrap();
        let clauses: Vec<_> = switch_store.clauses(case_block).unwrap().iter().collect();
        // Equal start and end denotes implicit fallthrough; undefined marks explicit default clause.
        let default_index = core::find_index(&clauses, |clause| {
            switch_store.kind(*clause) == ast::Kind::DefaultClause
        });
        let clause_start = data.clause_start as usize;
        let clause_end = data.clause_end as usize;
        let has_default_clause = clause_start == clause_end
            || (default_index >= clause_start as isize && default_index < clause_end as isize);
        if has_default_clause {
            // In the default clause we filter constituents down to those that are not-equal to all handled cases.
            let not_equal_facts =
                self.get_not_equal_facts_from_typeof_switch(clause_start, clause_end, &witnesses);
            return self.filter_type_with_checker(t, |checker, tt| {
                checker.get_type_facts(tt, not_equal_facts) == not_equal_facts
            });
        }
        // In the non-default cause we create a union of the type narrowed by each of the listed cases.
        let clause_witnesses = &witnesses[clause_start..clause_end];
        let mut narrowed = Vec::with_capacity(clause_witnesses.len());
        for text in clause_witnesses {
            if !text.is_empty() {
                narrowed.push(self.narrow_type_by_type_name(t, text));
            } else {
                narrowed.push(self.semantic_state.semantic_handles().never_type);
            }
        }
        self.get_union_type(narrowed)
    }

    fn narrow_type_by_switch_on_true(
        &mut self,
        f: &FlowState,
        t: TypeHandle,
        data: &ast::FlowSwitchClauseData,
    ) -> TypeHandle {
        let switch_statement = data.switch_statement();
        let switch_store = self.store_for_node(*switch_statement);
        let case_block = switch_store.case_block(*switch_statement).unwrap();
        let clauses: Vec<_> = switch_store.clauses(case_block).unwrap().iter().collect();
        let default_index = core::find_index(&clauses, |clause| {
            switch_store.kind(*clause) == ast::Kind::DefaultClause
        });
        let clause_start = data.clause_start as usize;
        let clause_end = data.clause_end as usize;
        let has_default_clause = clause_start == clause_end
            || (default_index >= clause_start as isize && default_index < clause_end as isize);
        // First, narrow away all of the cases that preceded this set of cases.
        let mut current = t;
        for i in 0..clause_start {
            let clause = &clauses[i];
            if switch_store.kind(*clause) == ast::Kind::CaseClause {
                let expression = self.store_for_node(*clause).expression(*clause).unwrap();
                current = self.narrow_type(f, current, expression, false /*assumeTrue*/);
            }
        }
        // If our current set has a default, then none the other cases were hit either.
        // There's no point in narrowing by the other cases in the set, since we can
        // get here through other paths.
        if has_default_clause {
            for i in clause_end..clauses.len() {
                let clause = &clauses[i];
                if switch_store.kind(*clause) == ast::Kind::CaseClause {
                    let expression = self.store_for_node(*clause).expression(*clause).unwrap();
                    current = self.narrow_type(f, current, expression, false /*assumeTrue*/);
                }
            }
            return current;
        }
        // Now, narrow based on the cases in this set.
        let mut types = Vec::with_capacity(clause_end - clause_start);
        for clause in &clauses[clause_start..clause_end] {
            if switch_store.kind(*clause) == ast::Kind::CaseClause {
                let expression = self.store_for_node(*clause).expression(*clause).unwrap();
                types.push(self.narrow_type(f, current, expression, true /*assumeTrue*/));
            } else {
                types.push(self.semantic_state.semantic_handles().never_type);
            }
        }
        self.get_union_type(types)
    }

    fn narrow_type_by_switch_optional_chain_containment<F>(
        &mut self,
        t: TypeHandle,
        data: &ast::FlowSwitchClauseData,
        clause_check: F,
    ) -> TypeHandle
    where
        F: Fn(&mut Checker<'a, '_>, TypeHandle) -> bool,
    {
        let switch_types = self.get_switch_clause_types(*data.switch_statement());
        let every_clause_checks = data.clause_start != data.clause_end
            && switch_types[data.clause_start as usize..data.clause_end as usize]
                .iter()
                .all(|tt| clause_check(self, *tt));
        if every_clause_checks {
            return self.get_type_with_facts(t, TYPE_FACTS_NE_UNDEFINED_OR_NULL);
        }
        t
    }

    fn narrow_type_by_switch_on_discriminant_property(
        &mut self,
        t: TypeHandle,
        access: ast::Node,
        data: &ast::FlowSwitchClauseData,
    ) -> TypeHandle {
        if data.clause_start < data.clause_end && self.type_flags(t) & TYPE_FLAGS_UNION != 0 {
            let (accessed_name, _) = self.get_accessed_property_name(access);
            if !accessed_name.is_empty() && self.get_key_property_name(t) == accessed_name {
                let switch_types = self.get_switch_clause_types(*data.switch_statement());
                let clause_types =
                    &switch_types[data.clause_start as usize..data.clause_end as usize];
                let mut candidates = Vec::with_capacity(clause_types.len());
                for s in clause_types {
                    candidates.push(
                        self.get_constituent_type_for_key_type(t, *s)
                            .unwrap_or(self.semantic_state.semantic_handles().unknown_type),
                    );
                }
                let candidate = self.get_union_type(candidates);
                if candidate != self.semantic_state.semantic_handles().unknown_type {
                    return candidate;
                }
            }
        }
        self.narrow_type_by_discriminant(t, access, |checker, tt| {
            checker.narrow_type_by_switch_on_discriminant(tt, data)
        })
    }

    fn get_type_at_flow_branch_label(
        &mut self,
        f: &mut FlowState,
        graph: &ast::FlowGraph,
        antecedents: &ast::FlowList,
    ) -> FlowType {
        let antecedent_start = self.semantic_state.antecedent_type_checkpoint();
        let mut subtype_reduction = false;
        let mut seen_incomplete = false;
        let mut bypass_flow: Option<ast::FlowRef> = None;
        let mut list = Some(antecedents.clone());
        while let Some(current_list) = list {
            let antecedent = current_list.flow.unwrap();
            let antecedent_node = graph.node(antecedent);
            if bypass_flow.is_none()
                && antecedent_node.flags & ast::FlowFlags::SwitchClause != 0
                && flow_node_reference_switch_clause_data(antecedent_node.node.as_ref().unwrap())
                    .is_empty()
            {
                // The antecedent is the bypass branch of a potentially exhaustive switch statement.
                bypass_flow = Some(antecedent);
                list = current_list.next.map(|next| graph.list(next).clone());
                continue;
            }
            drop(antecedent_node);
            let flow_type = self.get_type_at_flow_node(f, graph, antecedent);
            // If the type at a particular antecedent path is the declared type and the
            // reference is known to always be assigned (i.e. when declared and initial types
            // are the same), there is no reason to process more antecedents since the only
            // possible outcome is subtypes that will be removed in the final union type anyway.
            if flow_type.t.as_ref() == f.declared_type.as_ref() && f.declared_type == f.initial_type
            {
                self.semantic_state
                    .truncate_antecedent_types(antecedent_start);
                return FlowType {
                    t: flow_type.t,
                    incomplete: false,
                };
            }
            let flow_type_handle = *flow_type.t.as_ref().unwrap();
            if !self
                .semantic_state
                .has_antecedent_type_since(antecedent_start, flow_type_handle)
            {
                self.semantic_state.push_antecedent_type(flow_type_handle);
            }
            // If an antecedent type is not a subset of the declared type, we need to perform
            // subtype reduction. This happens when a "foreign" type is injected into the control
            // flow using the instanceof operator or a user defined type predicate.
            if !self.is_type_subset_of(
                *flow_type.t.as_ref().unwrap(),
                *f.initial_type.as_ref().unwrap(),
            ) {
                subtype_reduction = true;
            }
            if flow_type.incomplete {
                seen_incomplete = true;
            }
            list = current_list.next.map(|next| graph.list(next).clone());
        }
        if let Some(bypass_flow) = bypass_flow {
            let flow_type = self.get_type_at_flow_node(f, graph, bypass_flow);
            let bypass_switch_statement = {
                let bypass_flow = graph.node(bypass_flow);
                let data =
                    flow_node_reference_switch_clause_data(bypass_flow.node.as_ref().unwrap());
                *data.switch_statement()
            };
            // If the bypass flow contributes a type we haven't seen yet and the switch statement
            // isn't exhaustive, process the bypass flow type. Since exhaustiveness checks increase
            // the risk of circularities, we only want to perform them when they make a difference.
            if self.type_flags(*flow_type.t.as_ref().unwrap()) & TYPE_FLAGS_NEVER == 0
                && !self
                    .semantic_state
                    .has_antecedent_type_since(antecedent_start, *flow_type.t.as_ref().unwrap())
                && !self.is_exhaustive_switch_statement(bypass_switch_statement)
            {
                if flow_type.t.as_ref() == f.declared_type.as_ref()
                    && f.declared_type == f.initial_type
                {
                    self.semantic_state
                        .truncate_antecedent_types(antecedent_start);
                    return FlowType {
                        t: flow_type.t,
                        incomplete: false,
                    };
                }
                self.semantic_state
                    .push_antecedent_type(*flow_type.t.as_ref().unwrap());
                if !self.is_type_subset_of(
                    *flow_type.t.as_ref().unwrap(),
                    *f.initial_type.as_ref().unwrap(),
                ) {
                    subtype_reduction = true;
                }
                if flow_type.incomplete {
                    seen_incomplete = true;
                }
            }
        }
        let types = self.semantic_state.antecedent_types_since(antecedent_start);
        let union_or_evolving = self.get_union_or_evolving_array_type(
            f,
            &types,
            core::if_else(
                subtype_reduction,
                UNION_REDUCTION_SUBTYPE,
                UNION_REDUCTION_LITERAL,
            ),
        );
        let result = self.new_flow_type(union_or_evolving, seen_incomplete);
        self.semantic_state
            .truncate_antecedent_types(antecedent_start);
        result
    }

    // At flow control branch or loop junctions, if the type along every antecedent code path
    // is an evolving array type, we construct a combined evolving array type. Otherwise we
    // finalize all evolving array types.
    fn get_union_or_evolving_array_type(
        &mut self,
        f: &FlowState,
        types: &[TypeHandle],
        subtype_reduction: UnionReduction,
    ) -> TypeHandle {
        if self.is_evolving_array_type_list(types) {
            let mut element_types = Vec::with_capacity(types.len());
            for t in types {
                element_types.push(self.get_element_type_of_evolving_array_type(*t));
            }
            let union = self.get_union_type(element_types);
            return self.get_evolving_array_type(union);
        }
        let mut finalized_refs = Vec::with_capacity(types.len());
        let mut same = true;
        for t in types {
            let finalized = self.finalize_evolving_array_type(*t);
            if finalized != *t {
                same = false;
            }
            finalized_refs.push(finalized);
        }
        let union = if same {
            self.get_union_type_ex(types.to_vec(), subtype_reduction, None, None)
        } else {
            self.get_union_type_ex(finalized_refs, subtype_reduction, None, None)
        };
        let result = self.recombine_unknown_type(union);
        if result != *f.declared_type.as_ref().unwrap()
            && self.type_flags(result)
                & self.type_flags(*f.declared_type.as_ref().unwrap())
                & TYPE_FLAGS_UNION
                != 0
            && self.type_types(result) == self.type_types(*f.declared_type.as_ref().unwrap())
        {
            return *f.declared_type.as_ref().unwrap();
        }
        result
    }

    fn get_type_at_flow_loop_label(
        &mut self,
        f: &mut FlowState,
        graph: &ast::FlowGraph,
        flow: &ast::FlowNode,
    ) -> FlowType {
        if f.ref_key == 0 {
            f.ref_key = self.get_flow_reference_key(f);
        }
        if f.ref_key == non_dotted_name_cache_key() {
            // No cache key is generated when binding patterns are in unnarrowable situations
            return FlowType {
                t: f.declared_type.clone(),
                incomplete: false,
            };
        }
        let key = FlowLoopKey {
            flow_node: flow.id,
            ref_key: f.ref_key,
        };
        // If we have previously computed the control flow type for the reference at
        // this flow loop junction, return the cached type.
        if let Some(cached) = self.semantic_state.flow_loop_type(&key) {
            return FlowType {
                t: Some(cached),
                incomplete: false,
            };
        }
        // If this flow loop junction and reference are already being processed, return
        // the union of the types computed for each branch so far, marked as incomplete.
        // It is possible to see an empty array in cases where loops are nested and the
        // back edge of the outer loop reaches an inner loop that is already being analyzed.
        // In such cases we restart the analysis of the inner loop, which will then see
        // a non-empty in-process array for the outer loop and eventually terminate because
        // the first antecedent of a loop junction is always the non-looping control flow
        // path that leads to the top.
        let mut in_process_types = None;
        for loop_info in &self.semantic_state.flow_loop_stack {
            if loop_info.key == key && !loop_info.types.is_empty() {
                in_process_types = Some(loop_info.types.clone());
                break;
            }
        }
        if let Some(types) = in_process_types {
            let union_or_evolving =
                self.get_union_or_evolving_array_type(f, &types, UNION_REDUCTION_LITERAL);
            return self.new_flow_type(union_or_evolving, true /*incomplete*/);
        }
        // Add the flow loop junction and reference to the in-process stack and analyze
        // each antecedent code path.
        let mut antecedent_types: Vec<TypeHandle> = Vec::with_capacity(4);
        let mut subtype_reduction = false;
        let mut first_antecedent_type: Option<FlowType> = None;
        let mut list = flow
            .antecedents
            .map(|antecedents| graph.list(antecedents).clone());
        while let Some(current_list) = list {
            let flow_type = if first_antecedent_type.is_none() {
                // The first antecedent of a loop junction is always the non-looping control
                // flow path that leads to the top.
                let first = self.get_type_at_flow_node(f, graph, current_list.flow.unwrap());
                first_antecedent_type = Some(first.clone());
                first
            } else {
                // All but the first antecedent are the looping control flow paths that lead
                // back to the loop junction. We track these on the flow loop stack.
                self.semantic_state.flow_loop_stack.push(FlowLoopInfo {
                    key: key.clone(),
                    types: antecedent_types.clone(),
                });
                let save_flow_type_cache = self.semantic_state.suspend_flow_type_cache();
                let flow_type = self.get_type_at_flow_node(f, graph, current_list.flow.unwrap());
                self.semantic_state
                    .restore_flow_type_cache(save_flow_type_cache);
                let flow_loop_stack_len = self.semantic_state.flow_loop_stack.len();
                self.semantic_state
                    .flow_loop_stack
                    .truncate(flow_loop_stack_len - 1);
                // If we see a value appear in the cache it is a sign that control flow analysis
                // was restarted and completed by checkExpressionCached. We can simply pick up
                // the resulting type and bail out.
                if let Some(cached) = self.semantic_state.flow_loop_type(&key) {
                    return FlowType {
                        t: Some(cached),
                        incomplete: false,
                    };
                }
                flow_type
            };
            antecedent_types =
                core::append_if_unique(&antecedent_types, *flow_type.t.as_ref().unwrap());
            // If an antecedent type is not a subset of the declared type, we need to perform
            // subtype reduction. This happens when a "foreign" type is injected into the control
            // flow using the instanceof operator or a user defined type predicate.
            if !self.is_type_subset_of(
                *flow_type.t.as_ref().unwrap(),
                *f.initial_type.as_ref().unwrap(),
            ) {
                subtype_reduction = true;
            }
            // If the type at a particular antecedent path is the declared type there is no
            // reason to process more antecedents since the only possible outcome is subtypes
            // that will be removed in the final union type anyway.
            if flow_type.t.as_ref() == f.declared_type.as_ref() {
                break;
            }
            list = current_list.next.map(|next| graph.list(next).clone());
        }
        // The result is incomplete if the first antecedent (the non-looping control flow path)
        // is incomplete.
        let result = self.get_union_or_evolving_array_type(
            f,
            &antecedent_types,
            core::if_else(
                subtype_reduction,
                UNION_REDUCTION_SUBTYPE,
                UNION_REDUCTION_LITERAL,
            ),
        );
        if first_antecedent_type.unwrap().incomplete {
            return self.new_flow_type(result, true /*incomplete*/);
        }
        self.semantic_state.set_flow_loop_type(key, result);
        FlowType {
            t: Some(result),
            incomplete: false,
        }
    }

    fn get_type_at_flow_array_mutation(
        &mut self,
        f: &mut FlowState,
        graph: &ast::FlowGraph,
        flow: &ast::FlowNode,
    ) -> FlowType {
        if f.declared_type.as_ref().unwrap() == &self.semantic_state.semantic_handles().auto_type
            || f.declared_type.as_ref().unwrap()
                == &self.semantic_state.semantic_handles().auto_array_type
        {
            let node = flow_node_handle(flow);
            let store = self.store_for_node(node);
            let expr = if ast::is_call_expression(store, node) {
                let callee = store.expression(node).unwrap();
                self.store_for_node(callee).expression(callee).unwrap()
            } else {
                let left = store.left(node).unwrap();
                self.store_for_node(left).expression(left).unwrap()
            };
            let expr = expr;
            let reference_candidate = self.get_reference_candidate(expr);
            if self.is_matching_reference(*f.reference.as_ref().unwrap(), reference_candidate) {
                let flow_type = self.get_type_at_flow_node(f, graph, flow.antecedent.unwrap());
                if self.object_flags(*flow_type.t.as_ref().unwrap()) & OBJECT_FLAGS_EVOLVING_ARRAY
                    != 0
                {
                    let mut evolved_type = *flow_type.t.as_ref().unwrap();
                    if ast::is_call_expression(store, node) {
                        for arg in self.store_for_node(node).arguments(node).unwrap() {
                            let arg = arg;
                            evolved_type = self.add_evolving_array_element_type(evolved_type, arg);
                        }
                    } else {
                        let node_store = self.store_for_node(node);
                        let left = node_store.left(node).unwrap();
                        let right = node_store.right(node).unwrap();
                        let left_store = self.store_for_node(left);
                        let argument_expression = left_store.argument_expression(left).unwrap();
                        // We must get the context free expression type so as to not recur in an uncached fashion on the LHS (which causes exponential blowup in compile time)
                        let index_type =
                            self.get_context_free_type_of_expression(argument_expression);
                        if self.is_type_assignable_to_kind(index_type, TYPE_FLAGS_NUMBER_LIKE) {
                            evolved_type =
                                self.add_evolving_array_element_type(evolved_type, right);
                        }
                    }
                    return self.new_flow_type(evolved_type, flow_type.incomplete);
                }
                return flow_type;
            }
        }
        FlowType {
            t: None,
            incomplete: false,
        }
    }

    fn get_discriminant_property_access(
        &mut self,
        f: &FlowState,
        expr: ast::Node,
        computed_type: TypeHandle,
    ) -> Option<ast::Node> {
        // As long as the computed type is a subset of the declared type, we use the full declared type to detect
        // a discriminant property. In cases where the computed type isn't a subset, e.g because of a preceding type
        // predicate narrowing, we use the actual computed type.
        if self.type_flags(*f.declared_type.as_ref().unwrap()) & TYPE_FLAGS_UNION != 0
            || self.type_flags(computed_type) & TYPE_FLAGS_UNION != 0
        {
            let access = self.get_candidate_discriminant_property_access(f, expr);
            if let Some(access) = access {
                let access_ref = access;
                let (name, ok) = self.get_accessed_property_name(access_ref);
                if ok {
                    let mut t = computed_type;
                    if self.type_flags(*f.declared_type.as_ref().unwrap()) & TYPE_FLAGS_UNION != 0
                        && self.is_type_subset_of(computed_type, *f.declared_type.as_ref().unwrap())
                    {
                        t = f.declared_type.unwrap();
                    }
                    if self.is_discriminant_property(t, &name) {
                        return Some(access_ref.clone());
                    }
                }
            }
        }
        None
    }

    fn get_candidate_discriminant_property_access(
        &mut self,
        f: &FlowState,
        expr: ast::Node,
    ) -> Option<ast::Node> {
        let reference = f.reference.as_ref().unwrap();
        let reference_store = self.flow_store_for_node(*reference);
        if ast::is_binding_pattern(reference_store, *reference)
            || ast::is_function_expression_or_arrow_function(reference_store, *reference)
            || f.reference.is_some_and(|reference| {
                ast::is_object_literal_method(self.flow_store_for_node(reference), Some(reference))
            })
        {
            // When the reference is a binding pattern or function or arrow expression, we are narrowing a pseudo-reference in
            // getNarrowedTypeOfSymbol. An identifier for a destructuring variable declared in the same binding pattern or
            // parameter declared in the same parameter list is a candidate.
            if ast::is_identifier(self.flow_store_for_node(expr), expr) {
                let symbol = self.get_resolved_symbol(expr);
                let declaration = self
                    .get_export_symbol_of_value_symbol_identity_if_exported(Some(symbol))
                    .and_then(|symbol| self.symbol_identity_value_declaration(symbol));
                if let Some(declaration) = declaration {
                    let declaration_store = self.flow_store_for_node(declaration);
                    if (ast::is_binding_element(declaration_store, declaration)
                        || ast::is_parameter_declaration(declaration_store, declaration))
                        && f.reference.unwrap() == declaration_store.parent(declaration).unwrap()
                        && self
                            .flow_store_for_node(declaration)
                            .initializer(declaration)
                            .is_none()
                        && !has_dot_dot_dot_token(
                            self.flow_store_for_node(declaration),
                            declaration,
                        )
                    {
                        return Some(declaration.clone());
                    }
                }
            }
        } else if ast::is_access_expression(self.flow_store_for_node(expr), expr) {
            // An access expression is a candidate if the reference matches the left hand expression.
            let expression = self.flow_store_for_node(expr).expression(expr).unwrap();
            if self.is_matching_reference(*f.reference.as_ref().unwrap(), expression) {
                return Some(expr.clone());
            }
        } else if ast::is_identifier(self.flow_store_for_node(expr), expr) {
            let symbol = self.get_resolved_symbol(expr);
            if self.is_constant_variable_identity(symbol) {
                let declaration = self.symbol_identity_value_declaration(symbol);
                let initializer = declaration.and_then(|declaration| {
                    get_candidate_variable_declaration_initializer(
                        self.flow_store_for_node(declaration),
                        declaration,
                    )
                });
                // Given 'const x = obj.kind', allow 'x' as an alias for 'obj.kind'
                if let Some(initializer) = initializer.as_ref() {
                    if ast::is_access_expression(
                        self.flow_store_for_node(*initializer),
                        *initializer,
                    ) && self.is_matching_reference(
                        *f.reference.as_ref().unwrap(),
                        self.flow_store_for_node(*initializer)
                            .expression(*initializer)
                            .unwrap(),
                    ) {
                        return Some(initializer.clone());
                    }
                }
                // Given 'const { kind: x } = obj', allow 'x' as an alias for 'obj.kind'
                if let Some(declaration) = declaration {
                    if ast::is_binding_element(self.flow_store_for_node(declaration), declaration)
                        && self
                            .flow_store_for_node(declaration)
                            .initializer(declaration)
                            .is_none()
                    {
                        let declaration_parent = self
                            .flow_store_for_node(declaration)
                            .parent(declaration)
                            .unwrap();
                        let declaration_grandparent = self
                            .flow_store_for_node(declaration_parent)
                            .parent(declaration_parent)
                            .unwrap();
                        let initializer = get_candidate_variable_declaration_initializer(
                            self.flow_store_for_node(declaration_grandparent),
                            declaration_grandparent,
                        );
                        if let Some(initializer) = initializer {
                            let initializer = initializer;
                            if (ast::is_identifier(
                                self.flow_store_for_node(initializer),
                                initializer,
                            ) || ast::is_access_expression(
                                self.flow_store_for_node(initializer),
                                initializer,
                            )) && self
                                .is_matching_reference(*f.reference.as_ref().unwrap(), initializer)
                            {
                                return Some(declaration.clone());
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn get_evolving_array_type(&mut self, element_type: TypeHandle) -> TypeHandle {
        let key = CachedTypeKey {
            kind: CachedTypeKindEvolvingArrayType,
            type_id: self.type_id(element_type),
        };
        if let Some(result) = self.semantic_state.cached_type(key) {
            return result;
        }
        let result = self.new_object_type_from_identity(OBJECT_FLAGS_EVOLVING_ARRAY, None);
        self.semantic_state
            .type_record_mut(result)
            .as_evolving_array_type_mut()
            .element_type = Some(element_type);
        self.semantic_state.set_cached_type(key, result);
        result
    }

    fn get_element_type_of_evolving_array_type(&mut self, t: TypeHandle) -> TypeHandle {
        if self.object_flags(t) & OBJECT_FLAGS_EVOLVING_ARRAY != 0 {
            return self
                .type_record(t)
                .as_evolving_array_type()
                .element_type
                .unwrap();
        }
        self.semantic_state.semantic_handles().never_type
    }
}

fn typeof_ne_facts(text: &str) -> Option<TypeFacts> {
    match text {
        "string" => Some(TYPE_FACTS_TYPEOF_NE_STRING),
        "number" => Some(TYPE_FACTS_TYPEOF_NE_NUMBER),
        "bigint" => Some(TYPE_FACTS_TYPEOF_NE_BIG_INT),
        "boolean" => Some(TYPE_FACTS_TYPEOF_NE_BOOLEAN),
        "symbol" => Some(TYPE_FACTS_TYPEOF_NE_SYMBOL),
        "undefined" => Some(TYPE_FACTS_NE_UNDEFINED),
        "object" => Some(TYPE_FACTS_TYPEOF_NE_OBJECT),
        "function" => Some(TYPE_FACTS_TYPEOF_NE_FUNCTION),
        _ => None,
    }
}

fn get_candidate_variable_declaration_initializer(
    store: &ast::AstStore,
    node: ast::Node,
) -> Option<ast::Node> {
    if ast::is_variable_declaration(store, node) && store.type_node(node).is_none() {
        if let Some(initializer) = store.initializer(node) {
            return Some(ast::skip_parentheses(store, initializer));
        }
    }
    None
}

impl<'a, 'state> Checker<'a, 'state> {
    fn is_evolving_array_type_list(&self, types: &[TypeHandle]) -> bool {
        let mut has_evolving_array_type = false;
        for t in types {
            if self.type_flags(*t) & TYPE_FLAGS_NEVER == 0 {
                if self.object_flags(*t) & OBJECT_FLAGS_EVOLVING_ARRAY == 0 {
                    return false;
                }
                has_evolving_array_type = true;
            }
        }
        has_evolving_array_type
    }

    // Return true if the given node is 'x' in an 'x.length', x.push(value)', 'x.unshift(value)' or
    // 'x[n] = value' operation, where 'n' is an expression of type any, undefined, or a number-like type.
    pub(crate) fn is_evolving_array_operation_target(&mut self, node: ast::Node) -> bool {
        let root = self.get_reference_root(node);
        let parent = self.flow_store_for_node(root).parent(root).unwrap();
        let parent_store = self.flow_store_for_node(parent);
        let parent_name = parent_store.name(parent);
        let is_length_push_or_unshift = ast::is_property_access_expression(parent_store, parent)
            && parent_name.as_ref().is_some_and(|name| {
                parent_store.text(*name) == "length"
                    || parent_store.parent(parent).as_ref().is_some_and(|parent| {
                        ast::is_call_expression(self.flow_store_for_node(*parent), *parent)
                    }) && ast::is_identifier(parent_store, *name)
                        && ast::is_push_or_unshift_identifier(parent_store, *name)
            });
        let parent_parent = parent_store.parent(parent);
        let is_element_assignment = if ast::is_element_access_expression(parent_store, parent)
            && parent_store.expression(parent) == Some(root)
            && parent_parent.as_ref().is_some_and(|parent_parent| {
                ast::is_binary_expression(self.flow_store_for_node(*parent_parent), *parent_parent)
            })
            && {
                let parent_parent = parent_parent.as_ref().unwrap();
                let parent_parent_store = self.flow_store_for_node(*parent_parent);
                parent_parent_store
                    .kind(parent_parent_store.operator_token(*parent_parent).unwrap())
                    == ast::Kind::EqualsToken
                    && parent_parent_store.left(*parent_parent).as_ref() == Some(&parent)
                    && !ast::is_assignment_target(parent_parent_store, *parent_parent)
            } {
            let argument_expression = parent_store.argument_expression(parent).unwrap();
            let index_type = self.get_type_of_expression(argument_expression);
            self.is_type_assignable_to_kind(index_type, TYPE_FLAGS_NUMBER_LIKE)
        } else {
            false
        };
        is_length_push_or_unshift || is_element_assignment
    }

    // When adding evolving array element types we do not perform subtype reduction. Instead,
    // we defer subtype reduction until the evolving array type is finalized into a manifest
    // array type.
    fn add_evolving_array_element_type(
        &mut self,
        evolving_array_type: TypeHandle,
        node: ast::Node,
    ) -> TypeHandle {
        let context_free = self.get_context_free_type_of_expression(node);
        let base = self.get_base_type_of_literal_type(context_free);
        let new_element_type = self.get_regular_type_of_object_literal(base);
        let element_type = self
            .type_record(evolving_array_type)
            .as_evolving_array_type()
            .element_type
            .unwrap();
        if self.is_type_subset_of(new_element_type, element_type) {
            return evolving_array_type;
        }
        let union = self.get_union_type(vec![element_type, new_element_type]);
        self.get_evolving_array_type(union)
    }

    fn finalize_evolving_array_type(&mut self, t: TypeHandle) -> TypeHandle {
        if self.object_flags(t) & OBJECT_FLAGS_EVOLVING_ARRAY != 0 {
            return self.get_final_array_type(t);
        }
        t
    }

    fn get_final_array_type(&mut self, t: TypeHandle) -> TypeHandle {
        if let Some(final_array_type) = self
            .type_record(t)
            .as_evolving_array_type()
            .final_array_type
        {
            return final_array_type;
        }
        let element_type = self
            .type_record(t)
            .as_evolving_array_type()
            .element_type
            .unwrap();
        let final_array_type = self.create_final_array_type(element_type);
        self.semantic_state
            .type_record_mut(t)
            .as_evolving_array_type_mut()
            .final_array_type = Some(final_array_type);
        final_array_type
    }

    fn create_final_array_type(&mut self, element_type: TypeHandle) -> TypeHandle {
        if self.type_flags(element_type) & TYPE_FLAGS_NEVER != 0 {
            return self.semantic_state.semantic_handles().auto_array_type;
        }
        if self.type_flags(element_type) & TYPE_FLAGS_UNION != 0 {
            let reduced = self.get_union_type_ex(
                self.type_types(element_type),
                UNION_REDUCTION_SUBTYPE,
                None,
                None,
            );
            return self.create_array_type(reduced);
        }
        self.create_array_type(element_type)
    }

    fn report_flow_control_error(&mut self, node: ast::Node) {
        let mut current = Some(node);
        let block = loop {
            let Some(current_node) = current else {
                panic!("flow control error node should be in a function or module block");
            };
            let store = self.flow_store_for_node(current_node);
            if ast::is_function_or_module_block(store, current_node) {
                break current_node;
            }
            current = store.parent(current_node);
        };
        let source_file = self
            .try_source_file_for_flow_node(node)
            .expect("flow control error node should belong to a source file");
        let statement_list = self.flow_store_for_node(block).statement_list(block);
        let span =
            scanner::get_range_of_token_at_position(source_file, statement_list.pos() as usize);
        self.diagnostics().add(ast::new_diagnostic_with_file(
            Some(source_file.diagnostic_file()),
            span,
            &diagnostics::The_containing_function_or_module_body_is_too_large_for_control_flow_analysis,
            &[],
        ));
    }
}

fn non_dotted_name_cache_key() -> CacheHashKey {
    xxh3::xxh3_128("?".as_bytes())
}

impl<'a, 'state> Checker<'a, 'state> {
    pub(crate) fn is_matching_reference(&mut self, source: ast::Node, target: ast::Node) -> bool {
        match self.flow_kind(target) {
            ast::Kind::ParenthesizedExpression | ast::Kind::NonNullExpression => {
                let expression = self.flow_store_for_node(target).expression(target).unwrap();
                return self.is_matching_reference(source, expression);
            }
            ast::Kind::BinaryExpression => {
                let (left, right, operator, is_assignment, is_binary) = {
                    let store = self.flow_store_for_node(target);
                    (
                        store.left(target).unwrap(),
                        store.right(target).unwrap(),
                        store.kind(store.operator_token(target).unwrap()),
                        ast::is_assignment_expression(store, target, false),
                        ast::is_binary_expression(store, target),
                    )
                };
                return is_assignment && self.is_matching_reference(source, left)
                    || is_binary
                        && operator == ast::Kind::CommaToken
                        && self.is_matching_reference(source, right);
            }
            _ => {}
        }
        match self.flow_kind(source) {
            ast::Kind::MetaProperty => {
                let source_store = self.flow_store_for_node(source);
                let target_store = self.flow_store_for_node(target);
                ast::is_meta_property(target_store, target)
                    && source_store.keyword_token(source) == target_store.keyword_token(target)
                    && source_store.text(source_store.name(source).unwrap())
                        == target_store.text(target_store.name(target).unwrap())
            }
            ast::Kind::Identifier | ast::Kind::PrivateIdentifier => {
                if ast::is_this_in_type_query(self.flow_store_for_node(source), source) {
                    return self.flow_kind(target) == ast::Kind::ThisKeyword;
                }
                let source_symbol = self.get_resolved_symbol(source);
                let target_symbol = self.get_resolved_symbol(target);
                let exported_source_symbol = self
                    .get_export_symbol_of_value_symbol_identity_if_exported(Some(source_symbol));
                let target_declaration_symbol = self
                    .get_symbol_of_declaration(target)
                    .map(SymbolIdentity::from_symbol_handle);
                ast::is_identifier(self.flow_store_for_node(target), target)
                    && self.same_symbol_identity(source_symbol, target_symbol)
                    || (ast::is_variable_declaration(self.flow_store_for_node(target), target)
                        || ast::is_binding_element(self.flow_store_for_node(target), target))
                        && self.same_optional_symbol_identity(
                            exported_source_symbol,
                            target_declaration_symbol,
                        )
            }
            ast::Kind::ThisKeyword => self.flow_kind(target) == ast::Kind::ThisKeyword,
            ast::Kind::SuperKeyword => self.flow_kind(target) == ast::Kind::SuperKeyword,
            ast::Kind::NonNullExpression
            | ast::Kind::ParenthesizedExpression
            | ast::Kind::SatisfiesExpression => {
                let expression = self.flow_store_for_node(source).expression(source).unwrap();
                self.is_matching_reference(expression, target)
            }
            ast::Kind::PropertyAccessExpression | ast::Kind::ElementAccessExpression => {
                let (source_property_name, ok) = self.get_accessed_property_name(source);
                let target_is_access =
                    ast::is_access_expression(self.flow_store_for_node(target), target);
                if ok && target_is_access {
                    let (target_property_name, ok) = self.get_accessed_property_name(target);
                    if ok {
                        let source_expression =
                            self.flow_store_for_node(source).expression(source).unwrap();
                        let target_expression =
                            self.flow_store_for_node(target).expression(target).unwrap();
                        return target_property_name == source_property_name
                            && self.is_matching_reference(source_expression, target_expression);
                    }
                }
                let source_store = self.flow_store_for_node(source);
                let target_store = self.flow_store_for_node(target);
                if ast::is_element_access_expression(source_store, source)
                    && ast::is_element_access_expression(target_store, target)
                {
                    let source_arg = source_store.argument_expression(source).unwrap();
                    let target_arg = target_store.argument_expression(target).unwrap();
                    let source_arg = source_arg;
                    let target_arg = target_arg;
                    if ast::is_identifier(source_store, source_arg)
                        && ast::is_identifier(target_store, target_arg)
                    {
                        let symbol = self.get_resolved_symbol(source_arg);
                        let target_symbol = self.get_resolved_symbol(target_arg);
                        if self.same_symbol_identity(symbol, target_symbol)
                            && self.is_stable_cached_element_access_symbol(symbol)
                        {
                            let source_expression =
                                self.flow_store_for_node(source).expression(source).unwrap();
                            let target_expression =
                                self.flow_store_for_node(target).expression(target).unwrap();
                            return self
                                .is_matching_reference(source_expression, target_expression);
                        }
                    }
                }
                false
            }
            ast::Kind::QualifiedName => {
                if ast::is_access_expression(self.flow_store_for_node(target), target) {
                    let (target_property_name, ok) = self.get_accessed_property_name(target);
                    if ok {
                        let source_store = self.flow_store_for_node(source);
                        let right = source_store.right(source).unwrap();
                        let left = source_store.left(source).unwrap();
                        let target_expression =
                            self.flow_store_for_node(target).expression(target).unwrap();
                        return source_store.text(right) == target_property_name
                            && self.is_matching_reference(left, target_expression);
                    }
                }
                false
            }
            ast::Kind::BinaryExpression => {
                let store = self.flow_store_for_node(source);
                let operator = store.kind(store.operator_token(source).unwrap());
                let right = store.right(source).unwrap();
                ast::is_binary_expression(store, source)
                    && operator == ast::Kind::CommaToken
                    && self.is_matching_reference(right, target)
            }
            _ => false,
        }
    }

    // Return the flow cache key for a "dotted name" (i.e. a sequence of identifiers
    // separated by dots). The key consists of the id of the symbol referenced by the
    // leftmost identifier followed by zero or more property names separated by dots.
    // The result is nonDottedNameCacheKey if the reference isn't a dotted name.
    fn get_flow_reference_key(&mut self, f: &FlowState) -> CacheHashKey {
        let mut b = KeyBuilder::default();
        if self.write_flow_cache_key(
            &mut b,
            *f.reference.as_ref().unwrap(),
            *f.declared_type.as_ref().unwrap(),
            *f.initial_type.as_ref().unwrap(),
            f.flow_container,
        ) {
            return b.hash();
        }
        non_dotted_name_cache_key() // Reference isn't a dotted name
    }

    fn write_symbol_identity_to_flow_cache_key(&self, b: &mut KeyBuilder, symbol: SymbolIdentity) {
        let handle = symbol.symbol_handle();
        b.write_byte(0xff);
        b.write_byte(match handle.domain() {
            ast::SymbolDomain::Program => 0,
            ast::SymbolDomain::CheckerTransient => 1,
        });
        b.write_symbol_handle(self, handle);
    }

    fn write_flow_cache_key(
        &mut self,
        b: &mut KeyBuilder,
        node: ast::Node,
        declared_type: TypeHandle,
        initial_type: TypeHandle,
        flow_container: Option<ast::Node>,
    ) -> bool {
        match self.flow_kind(node) {
            ast::Kind::Identifier => {
                if !ast::is_this_in_type_query(self.flow_store_for_node(node), node) {
                    let symbol = self.get_resolved_symbol(node);
                    if self.is_unknown_symbol_identity(symbol) {
                        return false;
                    }
                    self.write_symbol_identity_to_flow_cache_key(b, symbol);
                }
                b.write_byte(b':');
                b.write_type(self, declared_type);
                if initial_type != declared_type {
                    b.write_byte(b'=');
                    b.write_type(self, initial_type);
                }
                if let Some(flow_container) = flow_container {
                    b.write_byte(b'@');
                    b.write_node(
                        self.flow_store_for_node(flow_container),
                        Some(flow_container),
                    );
                }
                true
            }
            ast::Kind::ThisKeyword => {
                b.write_byte(b':');
                b.write_type(self, declared_type);
                if initial_type != declared_type {
                    b.write_byte(b'=');
                    b.write_type(self, initial_type);
                }
                if let Some(flow_container) = flow_container {
                    b.write_byte(b'@');
                    b.write_node(
                        self.flow_store_for_node(flow_container),
                        Some(flow_container),
                    );
                }
                true
            }
            ast::Kind::NonNullExpression | ast::Kind::ParenthesizedExpression => self
                .write_flow_cache_key(
                    b,
                    self.flow_store_for_node(node).expression(node).unwrap(),
                    declared_type,
                    initial_type,
                    flow_container,
                ),
            ast::Kind::QualifiedName => {
                let (left, right_text) = {
                    let store = self.flow_store_for_node(node);
                    let left = store.left(node).unwrap();
                    let right = store.right(node).unwrap();
                    (left, store.text(right).to_string())
                };
                if !self.write_flow_cache_key(b, left, declared_type, initial_type, flow_container)
                {
                    return false;
                }
                b.write_byte(b'.');
                b.write_string(&right_text);
                true
            }
            ast::Kind::PropertyAccessExpression | ast::Kind::ElementAccessExpression => {
                let (prop_name, ok) = self.get_accessed_property_name(node);
                if ok {
                    let expression = self.flow_store_for_node(node).expression(node).unwrap();
                    if !self.write_flow_cache_key(
                        b,
                        expression,
                        declared_type,
                        initial_type,
                        flow_container,
                    ) {
                        return false;
                    }
                    b.write_byte(b'.');
                    b.write_string(&prop_name);
                    return true;
                }
                if ast::is_element_access_expression(self.flow_store_for_node(node), node)
                    && self
                        .flow_store_for_node(node)
                        .argument_expression(node)
                        .is_some_and(|argument| {
                            ast::is_identifier(self.flow_store_for_node(argument), argument)
                        })
                {
                    let argument_expression = self
                        .flow_store_for_node(node)
                        .argument_expression(node)
                        .unwrap();
                    let symbol = self.get_resolved_symbol(argument_expression);
                    if self.is_stable_cached_element_access_symbol(symbol) {
                        let expression = self.flow_store_for_node(node).expression(node).unwrap();
                        if !self.write_flow_cache_key(
                            b,
                            expression,
                            declared_type,
                            initial_type,
                            flow_container,
                        ) {
                            return false;
                        }
                        b.write_string(".@");
                        self.write_symbol_identity_to_flow_cache_key(b, symbol);
                        return true;
                    }
                }
                false
            }
            ast::Kind::ObjectBindingPattern
            | ast::Kind::ArrayBindingPattern
            | ast::Kind::FunctionDeclaration
            | ast::Kind::FunctionExpression
            | ast::Kind::ArrowFunction
            | ast::Kind::MethodDeclaration => {
                b.write_node(self.flow_store_for_node(node), Some(node));
                b.write_byte(b'#');
                b.write_type(self, declared_type);
                true
            }
            _ => false,
        }
    }

    fn get_accessed_property_name(&mut self, access: ast::Node) -> (String, bool) {
        let store = self.flow_store_for_node(access);
        if ast::is_property_access_expression(store, access) {
            let name = store.name(access).unwrap();
            return (store.text(name).to_string(), true);
        }
        if ast::is_element_access_expression(store, access) {
            return self.try_get_element_access_expression_name(access);
        }
        if ast::is_binding_element(store, access) {
            return self.get_destructuring_property_name(access);
        }
        if ast::is_parameter_declaration(store, access) {
            let parent = store.parent(access).unwrap();
            return (
                self.store_for_node(parent)
                    .parameters(parent)
                    .unwrap()
                    .iter()
                    .position(|p| p == access)
                    .unwrap()
                    .to_string(),
                true,
            );
        }
        (String::new(), false)
    }

    pub(crate) fn try_get_element_access_expression_name(
        &mut self,
        node: ast::Node,
    ) -> (String, bool) {
        let store = self.flow_store_for_node(node);
        let argument = store.argument_expression(node).unwrap();
        let argument = argument;
        let argument_store = self.flow_store_for_node(argument);
        if ast::is_string_or_numeric_literal_like(argument_store, argument) {
            return (argument_store.text(argument).to_string(), true);
        }
        if ast::is_entity_name_expression(argument_store, argument) {
            return self.try_get_name_from_entity_name_expression(argument);
        }
        (String::new(), false)
    }

    fn flow_resolution_location_for_synthetic_node(&self, node: ast::Node) -> Option<ast::Node> {
        let factory_store = self.factory().store();
        if node.store_id() != factory_store.store_id() {
            return None;
        }

        let mut current = node;
        loop {
            let parent = factory_store.parent(current)?;
            if parent.store_id() != factory_store.store_id() {
                return Some(parent);
            }
            current = parent;
        }
    }

    fn try_get_name_from_entity_name_expression(&mut self, node: ast::Node) -> (String, bool) {
        let resolution_location = self.flow_resolution_location_for_synthetic_node(node);
        let symbol = self.resolve_entity_name(
            node,
            ast::SYMBOL_FLAGS_VALUE,
            true, /*ignoreErrors*/
            false,
            resolution_location,
        );
        let Some(symbol) = symbol else {
            return (String::new(), false);
        };
        if !(self.is_constant_variable_identity(symbol)
            || self.symbol_identity_flags(symbol) & ast::SYMBOL_FLAGS_ENUM_MEMBER != 0)
        {
            return (String::new(), false);
        }
        let declaration = self.symbol_identity_value_declaration(symbol);
        let Some(declaration) = declaration else {
            return (String::new(), false);
        };
        let declaration = declaration;
        let t = self.try_get_type_from_type_node(declaration);
        if let Some(t) = t {
            if let Some(name) = self.try_get_name_from_type(t) {
                return (name, true);
            }
        }
        if has_only_expression_initializer(self.store_for_node(declaration), declaration)
            && self.is_block_scoped_name_declared_before_use(
                declaration,
                resolution_location.unwrap_or(node),
            )
        {
            let declaration_store = self.store_for_node(declaration);
            let initializer = declaration_store.initializer(declaration);
            if let Some(initializer) = initializer {
                let declaration_parent = declaration_store.parent(declaration).unwrap();
                let initializer_type = if ast::is_binding_pattern(
                    self.store_for_node(declaration_parent),
                    declaration_parent,
                ) {
                    self.get_type_for_binding_element(declaration)
                } else {
                    self.get_type_of_expression(initializer)
                };
                return self
                    .try_get_name_from_type(initializer_type)
                    .map(|name| (name, true))
                    .unwrap_or_else(|| (String::new(), false));
            } else if ast::is_enum_member(self.store_for_node(declaration), declaration) {
                let store = self.store_for_node(declaration);
                let name = store.name(declaration).unwrap();
                return ast::try_get_text_of_property_name(store, name);
            }
        }
        (String::new(), false)
    }

    pub(crate) fn get_destructuring_property_name(&mut self, node: ast::Node) -> (String, bool) {
        let parent = self.store_for_node(node).parent(node).unwrap();
        if ast::is_binding_element(self.store_for_node(node), node)
            && ast::is_object_binding_pattern(self.store_for_node(parent), parent)
        {
            return self.get_literal_property_name_text(
                get_binding_element_property_name(self.store_for_node(node), node).unwrap(),
            );
        }
        if ast::is_property_assignment(self.store_for_node(node), node)
            || ast::is_shorthand_property_assignment(self.store_for_node(node), node)
        {
            return self
                .get_literal_property_name_text(self.store_for_node(node).name(node).unwrap());
        }
        if ast::is_array_literal_expression(self.store_for_node(parent), parent)
            || ast::is_array_binding_pattern(self.store_for_node(parent), parent)
        {
            return (
                self.store_for_node(parent)
                    .elements(parent)
                    .unwrap()
                    .iter()
                    .position(|e| e == node)
                    .unwrap()
                    .to_string(),
                true,
            );
        }
        (String::new(), false)
    }

    fn get_literal_property_name_text(&mut self, name: ast::Node) -> (String, bool) {
        let t = self.get_literal_type_from_property_name(name);
        if self.type_flags(t) & (TYPE_FLAGS_STRING_LITERAL | TYPE_FLAGS_NUMBER_LITERAL) != 0 {
            return (
                literal_value_to_string(&self.type_record(t).as_literal_type().value),
                true,
            );
        }
        (String::new(), false)
    }
}

fn literal_value_to_string(value: &LiteralValue) -> String {
    match value {
        LiteralValue::String(value) => value.clone(),
        LiteralValue::Number(value) => value.to_string(),
        LiteralValue::Bool(value) => core::if_else(*value, "true".to_string(), "false".to_string()),
        LiteralValue::BigInt(value) | LiteralValue::PseudoBigInt(value) => value.to_string(),
        LiteralValue::None
        | LiteralValue::Symbol(_)
        | LiteralValue::Node(_)
        | LiteralValue::Type(_)
        | LiteralValue::Signature(_) => panic!("Unhandled case in anyToString"),
    }
}

impl<'a, 'state> Checker<'a, 'state> {
    pub(crate) fn is_constant_reference(&mut self, node: ast::Node) -> bool {
        let store = self.flow_store_for_node(node);
        match store.kind(node) {
            ast::Kind::ThisKeyword => true,
            ast::Kind::Identifier => {
                if !ast::is_this_in_type_query(store, node) {
                    let symbol = self.get_resolved_symbol(node);
                    return {
                        self.is_constant_variable_identity(symbol)
                            || self.is_parameter_or_mutable_local_variable_identity(symbol)
                                && !self.is_symbol_assigned_identity(symbol)
                            || self.symbol_identity_value_declaration(symbol).is_some_and(
                                |declaration| {
                                    ast::is_function_expression(
                                        self.flow_store_for_node(declaration),
                                        declaration,
                                    )
                                },
                            )
                    };
                }
                false
            }
            ast::Kind::PropertyAccessExpression | ast::Kind::ElementAccessExpression => {
                // The resolvedSymbol property is initialized by checkPropertyAccess or checkElementAccess before we get here.
                let expression = store.expression(node).unwrap();
                if self.is_constant_reference(expression) {
                    let symbol = self.get_resolved_symbol_or_nil(node);
                    if let Some(symbol) = symbol {
                        return self.is_readonly_symbol_identity_in_flow(symbol);
                    }
                }
                false
            }
            ast::Kind::ObjectBindingPattern | ast::Kind::ArrayBindingPattern => {
                let node_parent = store.parent(node).unwrap();
                let root_declaration =
                    ast::get_root_declaration(self.flow_store_for_node(node_parent), node_parent);
                let root_store = self.flow_store_for_node(root_declaration);
                if ast::is_parameter_declaration(root_store, root_declaration)
                    || ast::is_variable_declaration(root_store, root_declaration)
                        && root_store
                            .parent(root_declaration)
                            .is_some_and(|parent| ast::is_catch_clause(root_store, parent))
                {
                    return !self.is_some_symbol_assigned(root_declaration);
                }
                ast::is_variable_declaration(root_store, root_declaration)
                    && self.is_var_const_like(root_declaration)
            }
            _ => false,
        }
    }

    fn contains_matching_reference(&mut self, mut source: ast::Node, target: ast::Node) -> bool {
        while ast::is_access_expression(self.flow_store_for_node(source), source) {
            source = self.flow_store_for_node(source).expression(source).unwrap();
            if self.is_matching_reference(source, target) {
                return true;
            }
        }
        false
    }

    fn optional_chain_contains_reference(
        &mut self,
        mut source: ast::Node,
        target: ast::Node,
    ) -> bool {
        while ast::is_optional_chain(self.flow_store_for_node(source), source) {
            source = self.flow_store_for_node(source).expression(source).unwrap();
            if self.is_matching_reference(source, target) {
                return true;
            }
        }
        false
    }

    fn get_reference_candidate(&mut self, node: ast::Node) -> ast::Node {
        let store = self.flow_store_for_node(node);
        match store.kind(node) {
            ast::Kind::ParenthesizedExpression => {
                let expression = store.expression(node).unwrap();
                self.get_reference_candidate(expression)
            }
            ast::Kind::BinaryExpression => {
                let operator = store.operator_token(node).unwrap();
                let operator = store.kind(operator);
                match operator {
                    ast::Kind::EqualsToken
                    | ast::Kind::BarBarEqualsToken
                    | ast::Kind::AmpersandAmpersandEqualsToken
                    | ast::Kind::QuestionQuestionEqualsToken => {
                        self.get_reference_candidate(store.left(node).unwrap())
                    }
                    ast::Kind::CommaToken => {
                        self.get_reference_candidate(store.right(node).unwrap())
                    }
                    _ => node,
                }
            }
            _ => node,
        }
    }

    fn get_reference_root(&mut self, node: ast::Node) -> ast::Node {
        let parent = self.flow_store_for_node(node).parent(node).unwrap();
        let parent_store = self.flow_store_for_node(parent);
        if ast::is_parenthesized_expression(parent_store, parent)
            || ast::is_binary_expression(parent_store, parent)
                && parent_store
                    .operator_token(parent)
                    .is_some_and(|operator| parent_store.kind(operator) == ast::Kind::EqualsToken)
                && parent_store.left(parent) == Some(node)
            || ast::is_binary_expression(parent_store, parent)
                && parent_store
                    .operator_token(parent)
                    .is_some_and(|operator| parent_store.kind(operator) == ast::Kind::CommaToken)
                && parent_store.right(parent) == Some(node)
        {
            return self.get_reference_root(parent);
        }
        node
    }

    pub(crate) fn has_matching_argument(
        &mut self,
        expression: ast::Node,
        reference: ast::Node,
    ) -> bool {
        let (arguments, expression_expression) = {
            let store = self.flow_store_for_node(expression);
            (
                store
                    .arguments(expression)
                    .map(|arguments| arguments.iter().collect::<Vec<_>>())
                    .unwrap_or_default(),
                store.expression(expression),
            )
        };
        for argument in arguments {
            if self.is_or_contains_matching_reference(reference, argument)
                || self.optional_chain_contains_reference(argument, reference)
            {
                return true;
            }
        }
        if let Some(expression_expression) = expression_expression {
            let expression_store = self.flow_store_for_node(expression_expression);
            if ast::is_property_access_expression(expression_store, expression_expression)
                && self.is_or_contains_matching_reference(
                    reference,
                    expression_store.expression(expression_expression).unwrap(),
                )
            {
                return true;
            }
        }
        false
    }

    fn is_or_contains_matching_reference(&mut self, source: ast::Node, target: ast::Node) -> bool {
        self.is_matching_reference(source, target)
            || self.contains_matching_reference(source, target)
    }

    // Return a new type in which occurrences of the string, number and bigint primitives and placeholder template
    // literal types in typeWithPrimitives have been replaced with occurrences of compatible and more specific types
    // from typeWithLiterals. This is essentially a limited form of intersection between the two types. We avoid a
    // true intersection because it is more costly and, when applied to union types, generates a large number of
    // types we don't actually care about.
    fn replace_primitives_with_literals(
        &mut self,
        type_with_primitives: TypeHandle,
        type_with_literals: TypeHandle,
    ) -> TypeHandle {
        if self.maybe_type_of_kind(
            type_with_primitives,
            TYPE_FLAGS_STRING
                | TYPE_FLAGS_TEMPLATE_LITERAL
                | TYPE_FLAGS_NUMBER
                | TYPE_FLAGS_BIG_INT,
        ) && self.maybe_type_of_kind(
            type_with_literals,
            TYPE_FLAGS_STRING_LITERAL
                | TYPE_FLAGS_TEMPLATE_LITERAL
                | TYPE_FLAGS_STRING_MAPPING
                | TYPE_FLAGS_NUMBER_LITERAL
                | TYPE_FLAGS_BIG_INT_LITERAL,
        ) {
            return self.map_type(type_with_primitives, |checker, tt| {
                if checker.type_flags(tt) & TYPE_FLAGS_STRING != 0 {
                    checker.extract_types_of_kind(
                        type_with_literals,
                        TYPE_FLAGS_STRING
                            | TYPE_FLAGS_STRING_LITERAL
                            | TYPE_FLAGS_TEMPLATE_LITERAL
                            | TYPE_FLAGS_STRING_MAPPING,
                    )
                } else if checker.is_pattern_literal_type(tt)
                    && !checker.maybe_type_of_kind(
                        type_with_literals,
                        TYPE_FLAGS_STRING | TYPE_FLAGS_TEMPLATE_LITERAL | TYPE_FLAGS_STRING_MAPPING,
                    )
                {
                    checker.extract_types_of_kind(type_with_literals, TYPE_FLAGS_STRING_LITERAL)
                } else if checker.type_flags(tt) & TYPE_FLAGS_NUMBER != 0 {
                    checker.extract_types_of_kind(
                        type_with_literals,
                        TYPE_FLAGS_NUMBER | TYPE_FLAGS_NUMBER_LITERAL,
                    )
                } else if checker.type_flags(tt) & TYPE_FLAGS_BIG_INT != 0 {
                    checker.extract_types_of_kind(
                        type_with_literals,
                        TYPE_FLAGS_BIG_INT | TYPE_FLAGS_BIG_INT_LITERAL,
                    )
                } else {
                    tt
                }
            });
        }
        type_with_primitives
    }

    fn is_exhaustive_switch_statement(&mut self, node: ast::Node) -> bool {
        let links_handle = { self.semantic_state.switch_statement_link_handle(node) };
        let exhaustive_state = self.semantic_state.exhaustive_state_by_handle(links_handle);
        if exhaustive_state == EXHAUSTIVE_STATE_UNKNOWN {
            // Indicate resolution is in process
            self.semantic_state
                .mark_exhaustive_computing_by_handle(links_handle);
            let is_exhaustive = self.compute_exhaustive_switch_statement(node);
            self.semantic_state
                .set_exhaustive_result_if_computing_by_handle(links_handle, is_exhaustive);
        } else if exhaustive_state == EXHAUSTIVE_STATE_COMPUTING {
            // Resolve circularity to false
            self.semantic_state
                .mark_exhaustive_false_by_handle(links_handle);
        }
        self.semantic_state.exhaustive_state_by_handle(links_handle) == EXHAUSTIVE_STATE_TRUE
    }

    fn compute_exhaustive_switch_statement(&mut self, node: ast::Node) -> bool {
        let expression = self.store_for_node(node).expression(node).unwrap();
        if ast::is_type_of_expression(self.store_for_node(expression), expression) {
            let witnesses = self.get_switch_clause_type_of_witnesses(node);
            let Some(witnesses) = witnesses else {
                return false;
            };
            let type_of_expression = self
                .store_for_node(expression)
                .expression(expression)
                .unwrap();
            let checked_operand = self.check_expression_cached(type_of_expression);
            let operand_constraint = self.get_base_constraint_or_type(checked_operand);
            // Get the not-equal flags for all handled cases.
            let not_equal_facts = self.get_not_equal_facts_from_typeof_switch(0, 0, &witnesses);
            if self.type_flags(operand_constraint) & TYPE_FLAGS_ANY_OR_UNKNOWN != 0 {
                // We special case the top types to be exhaustive when all cases are handled.
                return TYPE_FACTS_ALL_TYPEOF_NE & not_equal_facts == TYPE_FACTS_ALL_TYPEOF_NE;
            }
            // A missing not-equal flag indicates that the type wasn't handled by some case.
            return !some_type(self, operand_constraint, |checker, tt| {
                checker.get_type_facts(tt, not_equal_facts) == not_equal_facts
            });
        }
        let checked_expression = self.check_expression_cached(expression);
        let t = self.get_base_constraint_or_type(checked_expression);
        if !self.is_literal_type(t) {
            return false;
        }
        let switch_types = self.get_switch_clause_types(node);
        if switch_types.is_empty()
            || core::some(&switch_types, |tt| self.is_neither_unit_type_nor_never(*tt))
        {
            return false;
        }
        let regular = self.map_type(t, |checker, tt| {
            checker.get_regular_type_of_literal_type(tt)
        });
        self.each_type_contained_in(regular, &switch_types)
    }

    fn each_type_contained_in(&mut self, source: TypeHandle, types: &[TypeHandle]) -> bool {
        if self.type_flags(source) & TYPE_FLAGS_UNION != 0 {
            return !core::some(&self.type_types(source), |tt| !types.contains(tt));
        }
        types.contains(&source)
    }

    fn is_coercible_under_double_equals(&self, source: TypeHandle, target: TypeHandle) -> bool {
        self.type_flags(source)
            & (TYPE_FLAGS_NUMBER | TYPE_FLAGS_STRING | TYPE_FLAGS_BOOLEAN_LITERAL)
            != 0
            && self.type_flags(target)
                & (TYPE_FLAGS_NUMBER | TYPE_FLAGS_STRING | TYPE_FLAGS_BOOLEAN)
                != 0
    }
}

impl<'a, 'state> Checker<'a, 'state> {
    // Get the type names from all cases in a switch on `typeof`. The default clause and/or duplicate type names are
    // represented as empty strings. Return nil if one or more case clause expressions are not string literals.
    fn get_switch_clause_type_of_witnesses(&mut self, node: ast::Node) -> Option<Vec<String>> {
        let links_handle = { self.semantic_state.switch_statement_link_handle(node) };
        let (witnesses_computed, cached_witnesses) =
            self.semantic_state.witnesses_state_by_handle(links_handle);
        if witnesses_computed {
            return cached_witnesses;
        }

        let store = self.store_for_node(node);
        let case_block = store.case_block(node).unwrap();
        let clauses = store.clauses(case_block).unwrap();
        let mut witnesses = vec![String::new(); clauses.len()];
        let mut failed = false;
        for (i, clause) in clauses.iter().enumerate() {
            if store.kind(clause) == ast::Kind::CaseClause {
                let mut text = String::new();
                let expression = store.expression(clause).unwrap();
                if ast::is_string_literal_like(store, expression) {
                    text = store.text(expression).to_string();
                }
                if text.is_empty() {
                    failed = true;
                    break;
                }
                if !witnesses.contains(&text) {
                    witnesses[i] = text;
                }
            }
        }
        let witnesses = if failed { None } else { Some(witnesses) };
        self.semantic_state
            .set_witnesses_by_handle(links_handle, witnesses.clone());
        witnesses
    }

    // Return the combined not-equal type facts for all cases except those between the start and end indices.
    fn get_not_equal_facts_from_typeof_switch(
        &mut self,
        start: usize,
        end: usize,
        witnesses: &[String],
    ) -> TypeFacts {
        let mut facts = TYPE_FACTS_NONE;
        for (i, witness) in witnesses.iter().enumerate() {
            if (i < start || i >= end) && !witness.is_empty() {
                facts |= typeof_ne_facts(witness).unwrap_or(TYPE_FACTS_TYPEOF_NE_HOST_OBJECT);
            }
        }
        facts
    }

    fn get_switch_clause_types(&mut self, node: ast::Node) -> Vec<TypeHandle> {
        let links_handle = { self.semantic_state.switch_statement_link_handle(node) };
        let (switch_types_computed, cached_switch_types) = self
            .semantic_state
            .switch_types_state_by_handle(links_handle);
        if switch_types_computed {
            return cached_switch_types;
        }

        let store = self.store_for_node(node);
        let case_block = store.case_block(node).unwrap();
        let clauses = store.clauses(case_block).unwrap();
        let mut switch_types = Vec::with_capacity(clauses.len());
        for clause in clauses.iter() {
            switch_types.push(self.get_type_of_switch_clause(clause));
        }
        self.semantic_state
            .set_switch_types_by_handle(links_handle, switch_types.clone());
        switch_types
    }

    fn get_type_of_switch_clause(&mut self, clause: ast::Node) -> TypeHandle {
        if self.store_for_node(clause).kind(clause) == ast::Kind::CaseClause {
            let expression = self.store_for_node(clause).expression(clause).unwrap();
            let expr_type = self.get_type_of_expression(expression);
            let regular = self.get_regular_type_of_literal_type(expr_type);
            return regular;
        }
        self.semantic_state.semantic_handles().never_type
    }

    pub(crate) fn get_effects_signature(&mut self, node: ast::Node) -> Option<SignatureHandle> {
        let mut signature = self.semantic_state.effects_signature(node);
        if signature.is_none() {
            // A call expression parented by an expression statement is a potential assertion. Other call
            // expressions are potential type predicate function calls. In order to avoid triggering
            // circularities in control flow analysis, we use getTypeOfDottedName when resolving the call
            // target expression of an assertion.
            let mut func_type: Option<TypeHandle> = None;
            let store = self.store_for_node(node);
            if ast::is_binary_expression(store, node) {
                let right = store.right(node).unwrap();
                let right_type = self.check_non_null_expression(right);
                func_type = self.get_symbol_has_instance_method_of_object_type(right_type);
            } else if store
                .parent(node)
                .is_some_and(|parent| ast::is_expression_statement(store, parent))
            {
                let expression = store.expression(node).unwrap();
                func_type = self.get_type_of_dotted_name(expression, None /*diagnostic*/);
            } else if store
                .expression(node)
                .is_some_and(|expression| store.kind(expression) != ast::Kind::SuperKeyword)
            {
                let expression = store.expression(node).unwrap();
                if ast::is_optional_chain(store, node) {
                    let checked = self.check_expression(expression);
                    let optional = self.get_optional_expression_type(checked, expression);
                    func_type = Some(self.check_non_null_type(optional, expression));
                } else {
                    func_type = Some(self.check_non_null_expression(expression));
                }
            }
            let mut apparent_type: Option<TypeHandle> = None;
            if let Some(func_type) = func_type {
                apparent_type = Some(self.get_apparent_type(func_type));
            }
            let signatures = self.get_signatures_of_type(
                apparent_type.unwrap_or(self.semantic_state.semantic_handles().unknown_type),
                SIGNATURE_KIND_CALL,
            );
            if signatures.len() == 1
                && self
                    .signature_record(signatures[0])
                    .type_parameters
                    .is_empty()
            {
                signature = Some(signatures[0]);
            } else if signatures
                .iter()
                .any(|sig| self.has_type_predicate_or_never_return_type(*sig))
            {
                signature = Some(self.get_resolved_signature(node, None, CHECK_MODE_NORMAL));
            }
            if !(signature
                .as_ref()
                .is_some_and(|sig| self.has_type_predicate_or_never_return_type(*sig)))
            {
                signature = Some(self.semantic_state.semantic_handles().unknown_signature);
            }
            self.semantic_state.set_effects_signature(node, signature);
        }
        if signature
            .as_ref()
            .is_some_and(|sig| sig == &self.semantic_state.semantic_handles().unknown_signature)
        {
            return None;
        }
        signature
    }

    /**
     * Get the type of the `[Symbol.hasInstance]` method of an object type.
     */
    pub(crate) fn get_symbol_has_instance_method_of_object_type(
        &mut self,
        t: TypeHandle,
    ) -> Option<TypeHandle> {
        let has_instance_property_name =
            self.get_property_name_for_known_symbol_name("hasInstance");
        if self.all_types_assignable_to_kind(t, TYPE_FLAGS_NON_PRIMITIVE) {
            let has_instance_property = self.get_property_of_type(t, &has_instance_property_name);
            if let Some(has_instance_property) = has_instance_property {
                let has_instance_property_type =
                    self.get_type_of_symbol_at_location(has_instance_property, None);
                if !self
                    .get_signatures_of_type(has_instance_property_type, SIGNATURE_KIND_CALL)
                    .is_empty()
                {
                    return Some(has_instance_property_type);
                }
            }
        }
        None
    }

    pub(crate) fn get_property_name_for_known_symbol_name(&mut self, symbol_name: &str) -> String {
        let ctor_type = {
            let resolver = (self
                .semantic_state
                .get_global_es_symbol_constructor_symbol_or_nil)
                .clone();
            self.resolve_global_symbol(resolver)
        };
        if let Some(ctor_type) = ctor_type {
            let ctor_symbol_type = self.get_type_of_symbol_at_location(ctor_type, None);
            let unique_type = self.get_type_of_property_of_type(ctor_symbol_type, symbol_name);
            if let Some(unique_type) = unique_type {
                if self.is_type_usable_as_property_name(unique_type) {
                    return self.get_property_name_from_type(unique_type);
                }
            }
        }
        format!("{}@{}", ast::INTERNAL_SYMBOL_NAME_PREFIX, symbol_name)
    }

    // We require the dotted function name in an assertion expression to be comprised of identifiers
    // that reference function, method, class or value module symbols; or variable, property or
    // parameter symbols with declarations that have explicit type annotations. Such references are
    // resolvable with no possibility of triggering circularities in control flow analysis.
    pub(crate) fn get_type_of_dotted_name(
        &mut self,
        node: ast::Node,
        mut diagnostic: Option<&mut ast::Diagnostic>,
    ) -> Option<TypeHandle> {
        let store = self.store_for_node(node);
        if store.flags(node) & ast::NodeFlags::IN_WITH_STATEMENT == 0 {
            match store.kind(node) {
                ast::Kind::Identifier => {
                    let resolved = self.get_resolved_symbol(node);
                    let symbol =
                        self.get_export_symbol_of_value_symbol_identity_if_exported(Some(resolved));
                    return self.get_explicit_type_of_symbol_identity(symbol, diagnostic);
                }
                ast::Kind::ThisKeyword => return self.get_explicit_this_type(node),
                ast::Kind::SuperKeyword => return Some(self.check_super_expression(node)),
                ast::Kind::PropertyAccessExpression => {
                    let expression = store.expression(node).unwrap();
                    let t = self.get_type_of_dotted_name(expression, diagnostic.as_deref_mut());
                    if let Some(t) = t {
                        let name = store.name(node).unwrap();
                        let mut prop: Option<SymbolIdentity> = None;
                        if ast::is_private_identifier(store, name) {
                            if let Some(symbol_name) = self
                                .get_private_identifier_property_name_for_type_symbol(
                                    t,
                                    &store.text(name),
                                )
                            {
                                prop = self.get_property_of_type(t, &symbol_name);
                            }
                        } else {
                            prop = self.get_property_of_type(t, &store.text(name));
                        }
                        if let Some(prop) = prop {
                            return self
                                .get_explicit_type_of_symbol_identity(Some(prop), diagnostic);
                        }
                    }
                }
                ast::Kind::ParenthesizedExpression => {
                    let expression = store.expression(node).unwrap();
                    return self.get_type_of_dotted_name(expression, diagnostic);
                }
                _ => {}
            }
        }
        None
    }

    fn is_declaration_with_explicit_type_annotation(&mut self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        (ast::is_variable_declaration(store, node)
            || ast::is_property_declaration(store, node)
            || ast::is_property_signature_declaration(store, node)
            || ast::is_parameter_declaration(store, node))
            && self.store_for_node(node).type_node(node).is_some()
            || self.is_expando_property_function_with_return_type_annotation(node)
    }

    fn is_expando_property_function_with_return_type_annotation(
        &mut self,
        node: ast::Node,
    ) -> bool {
        let store = self.store_for_node(node);
        if ast::is_binary_expression(store, node) {
            let expr = store.right(node).unwrap();
            if ast::is_function_like(self.store_for_node(expr), Some(expr))
                && self.store_for_node(expr).type_node(expr).is_some()
            {
                return true;
            }
        }
        false
    }

    fn has_type_predicate_or_never_return_type(&mut self, sig: SignatureHandle) -> bool {
        if self.get_type_predicate_of_signature(sig).is_some() {
            return true;
        }
        if let Some(declaration) = self.signature_record(sig).declaration {
            let unknown_type = self.semantic_state.semantic_handles().unknown_type;
            let return_type = self
                .get_return_type_from_annotation(declaration)
                .unwrap_or(unknown_type);
            return self.type_flags(return_type) & TYPE_FLAGS_NEVER != 0;
        }
        false
    }

    fn get_explicit_this_type(&mut self, node: ast::Node) -> Option<TypeHandle> {
        let container = ast::get_this_container(
            self.store_for_node(node),
            node,
            false, /*includeArrowFunctions*/
            false, /*includeClassComputedPropertyName*/
        );
        let container = container.unwrap();
        if ast::is_function_like(self.store_for_node(container), Some(container)) {
            let signature = self.get_signature_from_declaration(container);
            if let Some(this_parameter) = self.signature_this_parameter(signature) {
                return self.get_explicit_type_of_symbol_identity(Some(this_parameter), None);
            }
        }
        let container_parent = self.store_for_node(container).parent(container);
        if container_parent
            .as_ref()
            .is_some_and(|parent| ast::is_class_like(self.store_for_node(*parent), *parent))
        {
            let container_parent = container_parent.unwrap();
            let symbol = self.get_symbol_of_declaration(container_parent).unwrap();
            if ast::is_static(self.store_for_node(container), container) {
                return Some(self.get_type_of_symbol_handle(symbol));
            } else {
                let constructor_type = self.get_type_of_symbol_handle(symbol);
                let instance_type = self.get_instance_type(constructor_type);
                return Some(
                    self.type_record(instance_type)
                        .as_interface_type()
                        .and_then(|t| t.this_type)
                        .unwrap_or(instance_type),
                );
            }
        }
        None
    }

    fn get_initial_type(&mut self, node: ast::Node) -> TypeHandle {
        match self.store_for_node(node).kind(node) {
            ast::Kind::VariableDeclaration => self.get_initial_type_of_variable_declaration(node),
            ast::Kind::BindingElement => self.get_initial_type_of_binding_element(node),
            _ => panic!("Unhandled case in getInitialType"),
        }
    }

    fn get_initial_type_of_variable_declaration(&mut self, node: ast::Node) -> TypeHandle {
        let store = self.store_for_node(node);
        if let Some(initializer) = store.initializer(node) {
            return self.get_type_of_initializer(initializer);
        }
        let parent = store.parent(node).unwrap();
        let declaration_parent = self.store_for_node(parent).parent(parent).unwrap();
        if ast::is_for_in_statement(self.store_for_node(declaration_parent), declaration_parent) {
            return self.semantic_state.semantic_handles().string_type;
        }
        if ast::is_for_of_statement(self.store_for_node(declaration_parent), declaration_parent) {
            let t = self.check_right_hand_side_of_for_of(declaration_parent);
            if let Some(t) = t {
                return t;
            }
        }
        self.semantic_state.semantic_handles().error_type
    }

    pub(crate) fn get_type_of_initializer(&mut self, node: ast::Node) -> TypeHandle {
        // Return the cached type if one is available. If the type of the variable was inferred
        // from its initializer, we'll already have cached the type. Otherwise we compute it now
        // without caching such that transient types are reflected.
        if let Some(t) = self.semantic_state.try_type_node_resolved_type(node) {
            return t;
        }
        self.get_type_of_expression(node)
    }

    fn get_initial_type_of_binding_element(&mut self, node: ast::Node) -> TypeHandle {
        let pattern = self.store_for_node(node).parent(node).unwrap();
        let parent_type =
            self.get_initial_type(self.store_for_node(pattern).parent(pattern).unwrap());
        let t = if ast::is_object_binding_pattern(self.store_for_node(pattern), pattern) {
            self.get_type_of_destructured_property(
                parent_type,
                get_binding_element_property_name(self.store_for_node(node), node).unwrap(),
            )
        } else if !has_dot_dot_dot_token(self.store_for_node(node), node) {
            self.get_type_of_destructured_array_element(
                parent_type,
                self.store_for_node(pattern)
                    .elements(pattern)
                    .unwrap()
                    .iter()
                    .position(|e| e == node)
                    .unwrap(),
            )
        } else {
            self.get_type_of_destructured_spread_expression(parent_type)
        };
        let default_expression = self.store_for_node(node).initializer(node);
        self.get_type_with_default(t, default_expression)
    }

    fn get_assigned_type(&mut self, node: ast::Node) -> TypeHandle {
        let parent = self.store_for_node(node).parent(node).unwrap();
        match self.store_for_node(parent).kind(parent) {
            ast::Kind::ForInStatement => self.semantic_state.semantic_handles().string_type,
            ast::Kind::ForOfStatement => self
                .check_right_hand_side_of_for_of(parent)
                .unwrap_or(self.semantic_state.semantic_handles().error_type),
            ast::Kind::BinaryExpression => self.get_assigned_type_of_binary_expression(parent),
            ast::Kind::DeleteExpression => self.semantic_state.semantic_handles().undefined_type,
            ast::Kind::ArrayLiteralExpression => {
                self.get_assigned_type_of_array_literal_element(parent, node)
            }
            ast::Kind::SpreadElement => self.get_assigned_type_of_spread_expression(parent),
            ast::Kind::PropertyAssignment => self.get_assigned_type_of_property_assignment(parent),
            ast::Kind::ShorthandPropertyAssignment => {
                self.get_assigned_type_of_shorthand_property_assignment(parent)
            }
            _ => self.semantic_state.semantic_handles().error_type,
        }
    }

    fn get_assigned_type_of_binary_expression(&mut self, node: ast::Node) -> TypeHandle {
        let parent = self.store_for_node(node).parent(node).unwrap();
        let parent_store = self.store_for_node(parent);
        let is_destructuring_default_assignment =
            ast::is_array_literal_expression(parent_store, parent)
                && self.is_destructuring_assignment_target(parent)
                || ast::is_property_assignment(parent_store, parent)
                    && self.is_destructuring_assignment_target(
                        self.store_for_node(parent).parent(parent).unwrap(),
                    );
        if is_destructuring_default_assignment {
            let assigned = self.get_assigned_type(node);
            let right = self.store_for_node(node).right(node).unwrap();
            return self.get_type_with_default(assigned, Some(right));
        }
        let right = self.store_for_node(node).right(node).unwrap();
        self.get_type_of_expression(right)
    }

    fn get_assigned_type_of_array_literal_element(
        &mut self,
        node: ast::Node,
        element: ast::Node,
    ) -> TypeHandle {
        let assigned = self.get_assigned_type(node);
        self.get_type_of_destructured_array_element(
            assigned,
            self.store_for_node(node)
                .elements(node)
                .unwrap()
                .iter()
                .position(|e| e == element)
                .unwrap(),
        )
    }

    fn get_type_of_destructured_array_element(
        &mut self,
        t: TypeHandle,
        index: usize,
    ) -> TypeHandle {
        if every_type(self, t, |checker, tt| checker.is_tuple_like_type(tt)) {
            if let Some(element_type) = self.get_tuple_element_type(t, index) {
                return element_type;
            }
        }
        let element_type = self.check_iterated_type_or_element_type(
            ITERATION_USE_DESTRUCTURING,
            t,
            self.semantic_state.semantic_handles().undefined_type,
            None, /*errorNode*/
        );
        if element_type != self.semantic_state.semantic_handles().any_type
            || !is_type_any(self, Some(t))
        {
            return self.include_undefined_in_index_signature(element_type);
        }
        self.semantic_state.semantic_handles().error_type
    }

    fn include_undefined_in_index_signature(&mut self, t: TypeHandle) -> TypeHandle {
        if self.compiler_options.no_unchecked_indexed_access == core::TSTrue {
            return self
                .get_union_type(vec![t, self.semantic_state.semantic_handles().missing_type]);
        }
        t
    }

    fn get_assigned_type_of_spread_expression(&mut self, node: ast::Node) -> TypeHandle {
        let assigned = self.get_assigned_type(self.store_for_node(node).parent(node).unwrap());
        self.get_type_of_destructured_spread_expression(assigned)
    }

    fn get_type_of_destructured_spread_expression(&mut self, t: TypeHandle) -> TypeHandle {
        let mut element_type = self.check_iterated_type_or_element_type(
            ITERATION_USE_DESTRUCTURING,
            t,
            self.semantic_state.semantic_handles().undefined_type,
            None, /*errorNode*/
        );
        if element_type == self.semantic_state.semantic_handles().any_type
            && !is_type_any(self, Some(t))
        {
            element_type = self.semantic_state.semantic_handles().error_type;
        }
        self.create_array_type(element_type)
    }

    fn get_assigned_type_of_property_assignment(&mut self, node: ast::Node) -> TypeHandle {
        let assigned = self.get_assigned_type(self.store_for_node(node).parent(node).unwrap());
        self.get_type_of_destructured_property(
            assigned,
            self.store_for_node(node).name(node).unwrap(),
        )
    }

    fn get_type_of_destructured_property(&mut self, t: TypeHandle, name: ast::Node) -> TypeHandle {
        let name_type = self.get_literal_type_from_property_name(name);
        if !self.is_type_usable_as_property_name(name_type) {
            return self.semantic_state.semantic_handles().error_type;
        }
        let text = self.get_property_name_from_type(name_type);
        if let Some(prop_type) = self.get_type_of_property_of_type(t, &text) {
            return prop_type;
        }
        if let Some(index_info) = self.get_applicable_index_info_for_name(t, &text) {
            return self.include_undefined_in_index_signature(
                self.index_info_record(index_info).value_type.unwrap(),
            );
        }
        self.semantic_state.semantic_handles().error_type
    }

    fn get_assigned_type_of_shorthand_property_assignment(
        &mut self,
        node: ast::Node,
    ) -> TypeHandle {
        let assigned = self.get_assigned_type_of_property_assignment(node);
        let default_expression = self
            .store_for_node(node)
            .object_assignment_initializer(node);
        self.get_type_with_default(assigned, default_expression)
    }

    pub(crate) fn is_destructuring_assignment_target(&mut self, parent: ast::Node) -> bool {
        let Some(grandparent) = self.store_for_node(parent).parent(parent) else {
            return false;
        };
        let grandparent_store = self.store_for_node(grandparent);
        ast::is_binary_expression(grandparent_store, grandparent)
            && grandparent_store.left(grandparent) == Some(parent)
            || ast::is_for_of_statement(grandparent_store, grandparent)
                && grandparent_store.initializer(grandparent) == Some(parent)
    }

    fn get_type_with_default(
        &mut self,
        t: TypeHandle,
        default_expression: Option<ast::Node>,
    ) -> TypeHandle {
        if let Some(default_expression) = default_expression {
            let non_undefined = self.get_non_undefined_type(t);
            let default_type = self.get_type_of_expression(default_expression);
            return self.get_union_type(vec![non_undefined, default_type]);
        }
        t
    }

    // Remove those constituent types of declaredType to which no constituent type of assignedType is assignable.
    // For example, when a variable of type number | string | boolean is assigned a value of type number | boolean,
    // we remove type string.
    fn get_assignment_reduced_type(
        &mut self,
        declared_type: TypeHandle,
        assigned_type: TypeHandle,
    ) -> TypeHandle {
        if declared_type == assigned_type {
            return declared_type;
        }
        if self.type_flags(assigned_type) & TYPE_FLAGS_NEVER != 0 {
            return assigned_type;
        }
        let key = AssignmentReducedKey {
            id1: self.type_id(declared_type),
            id2: self.type_id(assigned_type),
        };
        if let Some(result) = self.semantic_state.assignment_reduced_type(key) {
            return result;
        }
        let result = self.get_assignment_reduced_type_worker(declared_type, assigned_type);
        self.semantic_state.set_assignment_reduced_type(key, result);
        result
    }

    fn get_assignment_reduced_type_worker(
        &mut self,
        declared_type: TypeHandle,
        assigned_type: TypeHandle,
    ) -> TypeHandle {
        let filtered_type = self.filter_type_with_checker(declared_type, |checker, tt| {
            checker.type_maybe_assignable_to(assigned_type, tt)
        });
        // Ensure that we narrow to fresh types if the assignment is a fresh boolean literal type.
        let mut reduced_type = filtered_type;
        if self.type_flags(assigned_type) & TYPE_FLAGS_BOOLEAN_LITERAL != 0
            && self.is_fresh_literal_type(assigned_type)
        {
            reduced_type = self.map_type(filtered_type, |checker, tt| {
                checker.get_fresh_type_of_literal_type(tt)
            });
        }
        // Our crude heuristic produces an invalid result in some cases: see GH#26130.
        // For now, when that happens, we give up and don't narrow at all.  (This also
        // means we'll never narrow for erroneous assignments where the assigned type
        // is not assignable to the declared type.)
        if self.is_type_assignable_to(assigned_type, reduced_type) {
            return reduced_type;
        }
        declared_type
    }

    fn type_maybe_assignable_to(&mut self, source: TypeHandle, target: TypeHandle) -> bool {
        if self.type_flags(source) & TYPE_FLAGS_UNION == 0 {
            return self.is_type_assignable_to(source, target);
        }
        for t in self.type_types(source) {
            if self.is_type_assignable_to(t, target) {
                return true;
            }
        }
        false
    }

    fn get_type_predicate_argument(
        &mut self,
        predicate: TypePredicateHandle,
        call_expression: ast::Node,
    ) -> Option<ast::Node> {
        let predicate = self.type_predicate_record(predicate).clone();
        if predicate.kind == TYPE_PREDICATE_KIND_IDENTIFIER
            || predicate.kind == TYPE_PREDICATE_KIND_ASSERTS_IDENTIFIER
        {
            let arguments = self
                .store_for_node(call_expression)
                .arguments(call_expression)
                .unwrap();
            if predicate.parameter_index >= 0
                && (predicate.parameter_index as usize) < arguments.len()
            {
                return arguments.iter().nth(predicate.parameter_index as usize);
            }
        } else {
            let expression = self
                .store_for_node(call_expression)
                .expression(call_expression)
                .unwrap();
            let invoked_expression =
                ast::skip_parentheses(self.store_for_node(expression), expression);
            if ast::is_access_expression(
                self.store_for_node(invoked_expression),
                invoked_expression,
            ) {
                let invoked_expression_node = self
                    .store_for_node(invoked_expression)
                    .expression(invoked_expression)
                    .unwrap();
                return Some(ast::skip_parentheses(
                    self.store_for_node(invoked_expression_node),
                    invoked_expression_node,
                ));
            }
        }
        None
    }

    pub(crate) fn get_flow_type_in_constructor(
        &mut self,
        symbol: SymbolIdentity,
        constructor: ast::Node,
    ) -> Option<TypeHandle> {
        let symbol_name = self.missing_name_symbol_identity_name(symbol);
        let access_name =
            if symbol_name.starts_with(&(ast::INTERNAL_SYMBOL_NAME_PREFIX.to_string() + "#")) {
                self.factory_mut()
                    .new_private_identifier(&symbol_name[symbol_name.find('@').unwrap() + 1..])
            } else {
                self.factory_mut().new_identifier(symbol_name.as_str())
            };
        let this_expression = self
            .factory_mut()
            .new_keyword_expression(ast::Kind::ThisKeyword);
        let reference = self.factory_mut().new_property_access_expression(
            this_expression,
            None,
            access_name,
            ast::NodeFlags::None,
        );
        let return_flow_node = self.node_return_flow_node(constructor);
        self.factory_mut()
            .link_checker_synthetic_parent(this_expression, Some(reference));
        self.factory_mut()
            .link_checker_synthetic_parent(reference, Some(constructor));
        self.record_checker_synthetic_flow_node(reference, return_flow_node);
        let reference = reference;
        let flow_type = self.get_flow_type_of_property_identity(reference, Some(symbol));
        if self.no_implicit_any()
            && (flow_type == self.semantic_state.semantic_handles().auto_type
                || flow_type == self.semantic_state.semantic_handles().auto_array_type)
        {
            let type_string = self.type_to_string(flow_type, None);
            let symbol_string = self.symbol_identity_to_string(symbol);
            let value_declaration = self
                .missing_name_symbol_identity_value_declaration(symbol)
                .unwrap();
            self.error(
                value_declaration,
                &diagnostics::Member_0_implicitly_has_an_1_type,
                vec![
                    DiagnosticArg::from(symbol_string),
                    DiagnosticArg::from(type_string),
                ],
            );
        }
        // We don't infer a type if assignments are only null or undefined.
        if every_type(self, flow_type, |checker, tt| checker.is_nullable_type(tt)) {
            return None;
        }
        Some(self.convert_auto_to_any(flow_type))
    }

    pub(crate) fn get_flow_type_in_static_blocks(
        &mut self,
        symbol: SymbolIdentity,
        static_blocks: &[ast::Node],
    ) -> Option<TypeHandle> {
        let symbol_name = self.missing_name_symbol_identity_name(symbol);
        let access_name =
            if symbol_name.starts_with(&(ast::INTERNAL_SYMBOL_NAME_PREFIX.to_string() + "#")) {
                self.factory_mut()
                    .new_private_identifier(&symbol_name[symbol_name.find('@').unwrap() + 1..])
            } else {
                self.factory_mut().new_identifier(symbol_name.as_str())
            };
        for static_block in static_blocks {
            let this_expression = self
                .factory_mut()
                .new_keyword_expression(ast::Kind::ThisKeyword);
            let reference = self.factory_mut().new_property_access_expression(
                this_expression,
                None,
                access_name.clone(),
                ast::NodeFlags::None,
            );
            let return_flow_node = self.node_return_flow_node(*static_block);
            self.factory_mut()
                .link_checker_synthetic_parent(this_expression, Some(reference));
            self.factory_mut()
                .link_checker_synthetic_parent(reference, Some(*static_block));
            self.record_checker_synthetic_flow_node(reference, return_flow_node);
            let reference = reference;
            let flow_type = self.get_flow_type_of_property_identity(reference, Some(symbol));
            if self.no_implicit_any()
                && (flow_type == self.semantic_state.semantic_handles().auto_type
                    || flow_type == self.semantic_state.semantic_handles().auto_array_type)
            {
                let type_string = self.type_to_string(flow_type, None);
                let symbol_string = self.symbol_identity_to_string(symbol);
                let value_declaration = self
                    .missing_name_symbol_identity_value_declaration(symbol)
                    .unwrap();
                self.error(
                    value_declaration,
                    &diagnostics::Member_0_implicitly_has_an_1_type,
                    vec![
                        DiagnosticArg::from(symbol_string),
                        DiagnosticArg::from(type_string),
                    ],
                );
            }
            // We don't infer a type if assignments are only null or undefined.
            if every_type(self, flow_type, |checker, tt| checker.is_nullable_type(tt)) {
                continue;
            }
            return Some(self.convert_auto_to_any(flow_type));
        }
        None
    }

    pub(crate) fn is_reachable_flow_ref(
        &mut self,
        graph: &ast::FlowGraph,
        flow: ast::FlowRef,
    ) -> bool {
        let mut f = self.get_flow_state();
        let result = self.is_reachable_flow_ref_worker(&mut f, flow, graph, false);
        self.put_flow_state(f);
        self.record_last_flow_node(flow, result);
        result
    }

    fn is_reachable_flow_ref_worker(
        &mut self,
        f: &mut FlowState,
        mut flow: ast::FlowRef,
        graph: &ast::FlowGraph,
        mut no_cache_check: bool,
    ) -> bool {
        loop {
            if let Some(reachable) = self.last_flow_node_reachable(flow) {
                return reachable;
            }
            let (flags, node, antecedent, antecedents) = {
                let flow_node = graph.node(flow);
                (
                    flow_node.flags,
                    flow_node.node.clone(),
                    flow_node.antecedent,
                    flow_node.antecedents,
                )
            };
            if flags & ast::FlowFlags::Shared != 0 {
                if !no_cache_check {
                    if let Some(reachable) = self.flow_node_reachable(flow) {
                        return reachable;
                    }
                    let reachable = self
                        .is_reachable_flow_ref_worker(f, flow, graph, true /*noCacheCheck*/);
                    self.record_flow_node_reachable(flow, reachable);
                    return reachable;
                }
                no_cache_check = false;
            }
            if flags
                & (ast::FlowFlags::Assignment
                    | ast::FlowFlags::Condition
                    | ast::FlowFlags::ArrayMutation)
                != 0
            {
                flow = antecedent.unwrap();
            } else if flags & ast::FlowFlags::Call != 0 {
                let node = flow_node_reference_handle(&node.unwrap());
                if let Some(signature) = self.get_effects_signature(node) {
                    if let Some(predicate) = self.get_type_predicate_of_signature(signature) {
                        let predicate_record = self.type_predicate_record(predicate);
                        if predicate_record.kind == TYPE_PREDICATE_KIND_ASSERTS_IDENTIFIER
                            && predicate_record.t.is_none()
                        {
                            let arguments = self.store_for_node(node).arguments(node).unwrap();
                            if predicate_record.parameter_index >= 0
                                && (predicate_record.parameter_index as usize) < arguments.len()
                                && self.is_false_expression(
                                    arguments
                                        .iter()
                                        .nth(predicate_record.parameter_index as usize)
                                        .unwrap(),
                                )
                            {
                                return false;
                            }
                        }
                    }
                    let return_type = self.get_return_type_of_signature(signature);
                    if self.type_flags(return_type) & TYPE_FLAGS_NEVER != 0 {
                        return false;
                    }
                }
                flow = antecedent.unwrap();
            } else if flags & ast::FlowFlags::BranchLabel != 0 {
                let antecedents = get_branch_label_antecedents_ref(graph, flow, &f.reduce_labels);
                let mut list = Some(antecedents);
                while let Some(current_list) = list {
                    if self.is_reachable_flow_ref_worker(
                        f,
                        current_list.flow.unwrap(),
                        graph,
                        false, /*noCacheCheck*/
                    ) {
                        return true;
                    }
                    list = current_list.next.map(|next| graph.list(next).clone());
                }
                return false;
            } else if flags & ast::FlowFlags::LoopLabel != 0 {
                let Some(antecedents) = antecedents else {
                    return false;
                };
                flow = graph.list(antecedents).flow.unwrap();
            } else if flags & ast::FlowFlags::SwitchClause != 0 {
                let data = flow_node_reference_switch_clause_data(&node.unwrap());
                if data.clause_start == data.clause_end
                    && self.is_exhaustive_switch_statement(*data.switch_statement())
                {
                    return false;
                }
                flow = antecedent.unwrap();
            } else if flags & ast::FlowFlags::ReduceLabel != 0 {
                self.clear_last_flow_node();
                let data = flow_node_reference_reduce_label_data(&node.unwrap());
                f.reduce_labels.push(data);
                let result =
                    self.is_reachable_flow_ref_worker(f, antecedent.unwrap(), graph, false);
                f.reduce_labels.truncate(f.reduce_labels.len() - 1);
                return result;
            } else {
                return flags & ast::FlowFlags::Unreachable == 0;
            }
        }
    }

    fn is_false_expression(&mut self, expr: ast::Node) -> bool {
        let node = ast::skip_parentheses(self.store_for_node(expr), expr);
        let store = self.store_for_node(node);
        if store.kind(node) == ast::Kind::FalseKeyword {
            return true;
        }
        if ast::is_binary_expression(store, node) {
            let operator = store.operator_token(node).unwrap();
            let operator = store.kind(operator);
            let left = store.left(node).unwrap();
            let right = store.right(node).unwrap();
            return operator == ast::Kind::AmpersandAmpersandToken
                && (self.is_false_expression(left) || self.is_false_expression(right))
                || operator == ast::Kind::BarBarToken
                    && self.is_false_expression(left)
                    && self.is_false_expression(right);
        }
        false
    }

    pub(crate) fn is_post_super_flow_ref(
        &mut self,
        graph: &ast::FlowGraph,
        flow: ast::FlowRef,
        no_cache_check: bool,
    ) -> bool {
        let mut f = self.get_flow_state();
        let result = self.is_post_super_flow_ref_worker(&mut f, flow, graph, no_cache_check);
        self.put_flow_state(f);
        result
    }

    fn is_post_super_flow_ref_worker(
        &mut self,
        f: &mut FlowState,
        mut flow: ast::FlowRef,
        graph: &ast::FlowGraph,
        mut no_cache_check: bool,
    ) -> bool {
        loop {
            let (flags, node, antecedent, antecedents) = {
                let flow_node = graph.node(flow);
                (
                    flow_node.flags,
                    flow_node.node.clone(),
                    flow_node.antecedent,
                    flow_node.antecedents,
                )
            };
            if flags & ast::FlowFlags::Shared != 0 {
                if !no_cache_check {
                    if let Some(post_super) = self.flow_node_post_super(flow) {
                        return post_super;
                    }
                    let post_super = self
                        .is_post_super_flow_ref_worker(f, flow, graph, true /*noCacheCheck*/);
                    self.record_flow_node_post_super(flow, post_super);
                    return post_super;
                }
                no_cache_check = false;
            }
            if flags
                & (ast::FlowFlags::Assignment
                    | ast::FlowFlags::Condition
                    | ast::FlowFlags::ArrayMutation
                    | ast::FlowFlags::SwitchClause)
                != 0
            {
                flow = antecedent.unwrap();
            } else if flags & ast::FlowFlags::Call != 0 {
                let node = flow_node_reference_node(node.as_ref().unwrap());
                if self
                    .store_for_node(node)
                    .expression(node)
                    .is_some_and(|expression| {
                        self.store_for_node(node).kind(expression) == ast::Kind::SuperKeyword
                    })
                {
                    return true;
                }
                flow = antecedent.unwrap();
            } else if flags & ast::FlowFlags::BranchLabel != 0 {
                let antecedents = get_branch_label_antecedents_ref(graph, flow, &f.reduce_labels);
                let mut list = Some(antecedents);
                while let Some(current_list) = list {
                    if !self.is_post_super_flow_ref_worker(
                        f,
                        current_list.flow.unwrap(),
                        graph,
                        false, /*noCacheCheck*/
                    ) {
                        return false;
                    }
                    list = current_list.next.map(|next| graph.list(next).clone());
                }
                return true;
            } else if flags & ast::FlowFlags::LoopLabel != 0 {
                flow = graph.list(antecedents.unwrap()).flow.unwrap();
            } else if flags & ast::FlowFlags::ReduceLabel != 0 {
                let data = flow_node_reference_reduce_label_data(&node.unwrap());
                f.reduce_labels.push(data);
                let result =
                    self.is_post_super_flow_ref_worker(f, antecedent.unwrap(), graph, false);
                f.reduce_labels.truncate(f.reduce_labels.len() - 1);
                return result;
            } else {
                return flags & ast::FlowFlags::Unreachable != 0;
            }
        }
    }

    // Check if a parameter, catch variable, or mutable local variable is definitely assigned anywhere
    pub(crate) fn is_symbol_assigned_definitely(&mut self, symbol: SymbolIdentity) -> bool {
        let Some(value_declaration) = self.symbol_identity_value_declaration(symbol) else {
            return false;
        };
        self.ensure_assignments_marked_for_declaration(value_declaration, symbol);
        self.semantic_state
            .marked_assignment_has_definite_assignment(symbol)
    }

    // Check if a parameter, catch variable, or mutable local variable is assigned anywhere
    pub(crate) fn is_symbol_assigned(&mut self, symbol: SymbolIdentity) -> bool {
        let Some(value_declaration) = self.symbol_identity_value_declaration(symbol) else {
            return false;
        };
        self.ensure_assignments_marked_for_declaration(value_declaration, symbol);
        self.semantic_state
            .marked_assignment_last_assignment_pos(symbol)
            != 0
    }

    // Return true if there are no assignments to the given symbol or if the given location
    // is past the last assignment to the symbol.
    pub(crate) fn is_past_last_assignment(
        &mut self,
        symbol: SymbolIdentity,
        location: Option<ast::Node>,
    ) -> bool {
        let Some(value_declaration) = self.symbol_identity_value_declaration(symbol) else {
            return true;
        };
        self.ensure_assignments_marked_for_declaration(value_declaration, symbol);
        let last_assignment_pos = self
            .semantic_state
            .marked_assignment_last_assignment_pos(symbol);
        last_assignment_pos == 0
            || location.is_some_and(|location| {
                last_assignment_pos < self.store_for_node(location).loc(location).pos()
            })
    }

    fn has_parent_with_assignments_marked(&mut self, node: ast::Node) -> bool {
        let parent = self.store_for_node(node).parent(node);
        ast::find_ancestor(self.store_for_node(node), parent, |store, node| {
            ast::is_function_or_source_file(store, node)
                && self.has_node_link_flags(node, NODE_CHECK_FLAGS_ASSIGNMENTS_MARKED)
        })
        .is_some()
    }

    // For all assignments within the given root node, record the last assignment source position for all
    // referenced parameters and mutable local variables. When assignments occur in nested functions  or
    // references occur in export specifiers, record math.MaxInt32 as the assignment position. When
    // assignments occur in compound statements, record the ending source position of the compound statement
    // as the assignment position (this is more conservative than full control flow analysis, but requires
    // only a single walk over the AST).
    pub(crate) fn mark_node_assignments_worker(&mut self, node: ast::Node) -> bool {
        match self.store_for_node(node).kind(node) {
            ast::Kind::Identifier => {
                let assignment_kind = get_assignment_target_kind(self.store_for_node(node), node);
                if assignment_kind != ASSIGNMENT_KIND_NONE {
                    let symbol = self.get_resolved_symbol(node);
                    let is_parameter_or_mutable_local_variable =
                        self.is_parameter_or_mutable_local_variable_identity(symbol);
                    if is_parameter_or_mutable_local_variable {
                        let marked_assignment_handle = self
                            .semantic_state
                            .marked_assignment_symbol_link_handle(symbol);
                        let last_assignment_pos = self
                            .semantic_state
                            .marked_assignment_last_assignment_pos_by_handle(
                                marked_assignment_handle,
                            );
                        if last_assignment_pos == 0 || last_assignment_pos != i32::MAX {
                            let value_declaration =
                                self.symbol_identity_value_declaration(symbol).unwrap();
                            let referencing_function = ast::find_ancestor(
                                self.store_for_node(node),
                                Some(node),
                                ast::is_function_or_source_file,
                            );
                            let declaring_function = ast::find_ancestor(
                                self.store_for_node(value_declaration),
                                Some(value_declaration),
                                ast::is_function_or_source_file,
                            );
                            let assignment_pos = if referencing_function == declaring_function {
                                self.extend_assignment_position(node, value_declaration)
                            } else {
                                i32::MAX
                            };
                            self.semantic_state
                                .set_marked_assignment_last_assignment_pos_by_handle(
                                    marked_assignment_handle,
                                    assignment_pos,
                                );
                        }
                        if assignment_kind == ASSIGNMENT_KIND_DEFINITE {
                            self.semantic_state
                                .mark_marked_assignment_has_definite_assignment_by_handle(
                                    marked_assignment_handle,
                                );
                        }
                    }
                }
                false
            }
            ast::Kind::ExportSpecifier => {
                let node_parent = self.store_for_node(node).parent(node).unwrap();
                let _export_declaration_node = self
                    .store_for_node(node_parent)
                    .parent(node_parent)
                    .unwrap();
                let name = self
                    .store_for_node(node)
                    .property_name_or_name(node)
                    .unwrap();
                if !self
                    .store_for_node(node)
                    .is_type_only(node)
                    .unwrap_or(false)
                    && !self
                        .store_for_node(node)
                        .is_type_only(node)
                        .unwrap_or(false)
                    && self.store_for_node(node).module_specifier(node).is_none()
                    && !ast::is_string_literal(self.store_for_node(name), name)
                {
                    let name = name;
                    let symbol = self.resolve_entity_name(
                        name,
                        ast::SYMBOL_FLAGS_VALUE,
                        true, /*ignoreErrors*/
                        true, /*dontResolveAlias*/
                        None,
                    );
                    if let Some(symbol) = symbol {
                        let is_parameter_or_mutable_local_variable =
                            self.is_parameter_or_mutable_local_variable_identity(symbol);
                        if is_parameter_or_mutable_local_variable {
                            self.semantic_state
                                .set_marked_assignment_last_assignment_pos(symbol, i32::MAX);
                        }
                    }
                }
                false
            }
            ast::Kind::InterfaceDeclaration
            | ast::Kind::TypeAliasDeclaration
            | ast::Kind::JSTypeAliasDeclaration
            | ast::Kind::EnumDeclaration => false,
            _ => {
                if ast::is_type_node(self.store_for_node(node), node) {
                    return false;
                }
                let mut has_assignment = false;
                let _ = self
                    .store_for_node(node)
                    .for_each_present_child(node, |child| {
                        if self.mark_node_assignments(child) {
                            has_assignment = true;
                            std::ops::ControlFlow::Break(())
                        } else {
                            std::ops::ControlFlow::Continue(())
                        }
                    });
                has_assignment
            }
        }
    }

    // Extend the position of the given assignment target node to the end of any intervening variable statement,
    // expression statement, compound statement, or class declaration occurring between the node and the given
    // declaration node.
    fn extend_assignment_position(&mut self, mut node: ast::Node, declaration: ast::Node) -> i32 {
        let mut pos = self.store_for_node(node).loc(node).pos();
        while self.store_for_node(node).loc(node).pos()
            > self.store_for_node(declaration).loc(declaration).pos()
        {
            match self.store_for_node(node).kind(node) {
                ast::Kind::VariableStatement
                | ast::Kind::ExpressionStatement
                | ast::Kind::IfStatement
                | ast::Kind::DoStatement
                | ast::Kind::WhileStatement
                | ast::Kind::ForStatement
                | ast::Kind::ForInStatement
                | ast::Kind::ForOfStatement
                | ast::Kind::WithStatement
                | ast::Kind::SwitchStatement
                | ast::Kind::TryStatement
                | ast::Kind::ClassDeclaration => {
                    pos = self.store_for_node(node).loc(node).end();
                }
                _ => {}
            }
            let Some(parent) = self.store_for_node(node).parent(node) else {
                break;
            };
            node = parent;
        }
        pos
    }
}
