use crate::ast;
use crate::ast::{NodeSliceTraversal, RawNodeSliceTraversal};
use crate::checker::*;
use crate::nodebuilderscopes::NodeBuilderScopeCleanup;
use crate::{core, evaluator, jsnum, nodebuilder, scanner};
use std::{cell::RefCell, rc::Rc};
use ts_printer as printer;

struct RecoveryBoundaryState {
    had_error: bool,
    tracked_symbols: Vec<RecoveryTrackedSymbol>,
    deferred_reports: Vec<DeferredReport>,
}

struct RecoveryTrackedSymbol {
    symbol: SymbolIdentity,
    enclosing_declaration: Option<ast::Node>,
    meaning: ast::SymbolFlags,
}

struct RecoveryBoundary<'a> {
    state: Rc<RefCell<RecoveryBoundaryState>>,
    wrapped_tracker: Rc<RefCell<Option<Box<dyn nodebuilder::SymbolTracker + 'a>>>>,
    old_tracked_symbols: Vec<TrackedSymbolArgs>,
    old_encountered_error: bool,
    old_approximate_length: usize,
}

enum DeferredReport {
    CyclicStructure,
    InaccessibleThis,
    InaccessibleUniqueSymbol,
    LikelyUnsafeImportRequired {
        specifier: String,
        symbol_name: String,
    },
    NonSerializableProperty {
        property_name: String,
    },
    PrivateInBaseOfClassExpression {
        property_name: String,
    },
}

struct SourceNodeListSnapshot {
    source_store_id: ast::StoreId,
    loc: core::TextRange,
    range: core::TextRange,
    missing: bool,
    has_trailing_comma: bool,
    nodes: Vec<ast::Node>,
}

impl SourceNodeListSnapshot {
    fn from_source(list: ast::SourceNodeList<'_>) -> Self {
        Self {
            source_store_id: list.store().store_id(),
            loc: list.loc(),
            range: list.range(),
            missing: list.is_missing(),
            has_trailing_comma: list.has_trailing_comma(),
            nodes: list.iter().collect(),
        }
    }
}

struct SourceModifierListSnapshot {
    source_store_id: ast::StoreId,
    loc: core::TextRange,
    range: core::TextRange,
    modifier_flags: ast::ModifierFlags,
    nodes: Vec<ast::Node>,
}

impl SourceModifierListSnapshot {
    fn from_source(modifiers: ast::SourceModifierList<'_>) -> Self {
        Self {
            source_store_id: modifiers.store().store_id(),
            loc: modifiers.loc(),
            range: modifiers.range(),
            modifier_flags: modifiers.modifier_flags(),
            nodes: modifiers.iter().collect(),
        }
    }
}

#[derive(Clone, Copy)]
struct RecoveryScopeState {
    tracked_symbols_top: usize,
    deferred_reports_top: usize,
    had_error: bool,
}

impl RecoveryBoundaryState {
    fn mark_error(&mut self) {
        self.had_error = true;
    }

    fn mark_error_with_report(&mut self, report: DeferredReport) {
        self.had_error = true;
        self.deferred_reports.push(report);
    }
}

impl RecoveryBoundary<'_> {
    fn mark_error(&mut self) {
        self.state.borrow_mut().mark_error();
    }

    fn mark_error_with_report(&mut self, report: DeferredReport) {
        self.state.borrow_mut().mark_error_with_report(report);
    }

    fn has_error(&self) -> bool {
        self.state.borrow().had_error
    }

    fn track_symbol_identity(
        &mut self,
        symbol: SymbolIdentity,
        enclosing_declaration: Option<ast::Node>,
        meaning: ast::SymbolFlags,
    ) {
        self.state
            .borrow_mut()
            .tracked_symbols
            .push(RecoveryTrackedSymbol {
                symbol,
                enclosing_declaration,
                meaning,
            });
    }

    fn start_recovery_scope(&self) -> RecoveryScopeState {
        let state = self.state.borrow();
        RecoveryScopeState {
            tracked_symbols_top: state.tracked_symbols.len(),
            deferred_reports_top: state.deferred_reports.len(),
            had_error: state.had_error,
        }
    }

    fn end_recovery_scope(&mut self, state: RecoveryScopeState) {
        let mut current = self.state.borrow_mut();
        current.had_error = state.had_error;
        current.tracked_symbols.truncate(state.tracked_symbols_top);
        current
            .deferred_reports
            .truncate(state.deferred_reports_top);
    }
}

struct RecoveryTracker<'a> {
    wrapped: Rc<RefCell<Option<Box<dyn nodebuilder::SymbolTracker + 'a>>>>,
    state: Rc<RefCell<RecoveryBoundaryState>>,
}

impl nodebuilder::SymbolTracker for RecoveryTracker<'_> {
    fn track_symbol(
        &mut self,
        symbol: ast::SymbolIdentity,
        _symbol_flags: ast::SymbolFlags,
        enclosing_declaration: Option<ast::Node>,
        meaning: ast::SymbolFlags,
    ) -> bool {
        let symbol = SymbolIdentity::from_symbol_handle(symbol.symbol_handle());
        self.state
            .borrow_mut()
            .tracked_symbols
            .push(RecoveryTrackedSymbol {
                symbol,
                enclosing_declaration,
                meaning,
            });
        false
    }

    fn report_inaccessible_this_error(&mut self) {
        self.state
            .borrow_mut()
            .mark_error_with_report(DeferredReport::InaccessibleThis);
    }

    fn report_private_in_base_of_class_expression(&mut self, property_name: &str) {
        self.state.borrow_mut().mark_error_with_report(
            DeferredReport::PrivateInBaseOfClassExpression {
                property_name: property_name.to_string(),
            },
        );
    }

    fn report_inaccessible_unique_symbol_error(&mut self) {
        self.state
            .borrow_mut()
            .mark_error_with_report(DeferredReport::InaccessibleUniqueSymbol);
    }

    fn report_cyclic_structure_error(&mut self) {
        self.state
            .borrow_mut()
            .mark_error_with_report(DeferredReport::CyclicStructure);
    }

    fn report_likely_unsafe_import_required_error(&mut self, specifier: &str, symbol_name: &str) {
        self.state.borrow_mut().mark_error_with_report(
            DeferredReport::LikelyUnsafeImportRequired {
                specifier: specifier.to_string(),
                symbol_name: symbol_name.to_string(),
            },
        );
    }

    fn report_truncation_error(&mut self) {
        if let Some(wrapped) = self.wrapped.borrow_mut().as_mut() {
            wrapped.report_truncation_error();
        }
    }

    fn report_nonlocal_augmentation(
        &mut self,
        containing_file: &ast::SourceFile,
        parent_symbol: ast::SymbolIdentity,
        augmenting_symbol: ast::SymbolIdentity,
    ) {
        if let Some(wrapped) = self.wrapped.borrow_mut().as_mut() {
            wrapped.report_nonlocal_augmentation(containing_file, parent_symbol, augmenting_symbol);
        }
    }

    fn report_non_serializable_property(&mut self, property_name: &str) {
        self.state
            .borrow_mut()
            .mark_error_with_report(DeferredReport::NonSerializableProperty {
                property_name: property_name.to_string(),
            });
    }

    fn report_inference_fallback(&mut self, node: ast::Node) {
        if let Some(wrapped) = self.wrapped.borrow_mut().as_mut() {
            wrapped.report_inference_fallback(node);
        }
    }

    fn push_error_fallback_node(&mut self, node: ast::Node) {
        if let Some(wrapped) = self.wrapped.borrow_mut().as_mut() {
            wrapped.push_error_fallback_node(node);
        }
    }

    fn pop_error_fallback_node(&mut self) {
        if let Some(wrapped) = self.wrapped.borrow_mut().as_mut() {
            wrapped.pop_error_fallback_node();
        }
    }
}

struct NodeCopyTraversal<'b, 'a, 'state, 'c, 'e> {
    builder: &'b mut NodeBuilderImpl<'a, 'state, 'c, 'e>,
    source: &'a ast::AstStore,
    bound: RecoveryBoundary<'a>,
    non_local_node: bool,
    import_state: ast::AstImportState,
}

impl<'b, 'a, 'state, 'c, 'e> NodeCopyTraversal<'b, 'a, 'state, 'c, 'e> {
    fn new(
        builder: &'b mut NodeBuilderImpl<'a, 'state, 'c, 'e>,
        source: &'a ast::AstStore,
        bound: RecoveryBoundary<'a>,
    ) -> Self {
        Self {
            builder,
            source,
            bound,
            non_local_node: true,
            import_state: ast::AstImportState::new(),
        }
    }

    fn node_value(node: ast::Node) -> ast::Node {
        NodeBuilderImpl::node_value(node)
    }

    fn symbol_identity_flags(&self, symbol: SymbolIdentity) -> ast::SymbolFlags {
        self.builder.ch.symbol_handle_flags(symbol.symbol_handle())
    }

    fn symbol_identity_value_declaration(&self, symbol: SymbolIdentity) -> Option<ast::Node> {
        self.builder
            .ch
            .symbol_handle_value_declaration(symbol.symbol_handle())
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
            self.builder
                .ch
                .symbol_handle_export_symbol(symbol.symbol_handle())
                .map(SymbolIdentity::from_symbol_handle)
        } else {
            None
        };
        self.builder
            .ch
            .get_merged_symbol_identity(export_symbol.or(Some(symbol)))
    }

    fn is_symbol_accessible_in_builder_scope_by_identity(
        &mut self,
        symbol: Option<SymbolIdentity>,
        enclosing_declaration: Option<ast::Node>,
        meaning: ast::SymbolFlags,
        should_compute_aliases_to_make_visible: bool,
    ) -> printer::SymbolAccessibilityResult {
        self.builder
            .is_symbol_accessible_in_builder_scope_by_identity(
                symbol,
                enclosing_declaration,
                meaning,
                should_compute_aliases_to_make_visible,
            )
    }

    fn output_node_list_value(list: ast::NodeList) -> ast::NodeList {
        NodeBuilderImpl::output_node_list_value(list)
    }
}

impl<'a, 'state, 'c, 'e> NodeBuilderImpl<'a, 'state, 'c, 'e> {
    pub(crate) fn reuse_node(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        node.and_then(|node| self.try_reuse_existing_node_helper(node))
    }

    pub(crate) fn try_js_type_node_to_type_node(
        &mut self,
        node: Option<ast::Node>,
    ) -> Option<ast::Node> {
        self.reuse_node(node)
    }

    pub(crate) fn reuse_name(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        let node = node?;
        let res = self.reuse_node(Some(node))?;
        let res_store = self.store_for_node(res);
        let node_store = self.store_for_node(node);
        if res_store.kind(res) == ast::Kind::Identifier
            && node_store.kind(node) == ast::Kind::Identifier
            && node_store.text(node) == "new"
        {
            let str_ = Self::node_value(
                self.e
                    .factory
                    .node_factory
                    .new_string_literal("new", ast::TOKEN_FLAGS_NONE),
            );
            self.set_original(&str_, &res);
            return self.set_text_range(Some(str_), Some(res));
        }
        Some(res)
    }

    pub(crate) fn reuse_type_node(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        let node = node?;
        let reused = self.try_reuse_existing_node_helper(node);
        if let Some(reused) = reused {
            if self.ctx.max_expansion_depth >= 0 && !self.ctx.can_increase_expansion_depth {
                self.walk_node_for_expandability(node);
            }
            return Some(reused);
        }
        self.report_inference_fallback(node);
        let t = self.get_type_from_type_node(node, false)?;
        self.type_to_type_node(t)
    }

    fn walk_node_for_expandability(&mut self, node: ast::Node) {
        if self.ctx.can_increase_expansion_depth {
            return;
        }

        let store = self.store_for_node(node);
        if ast::is_type_reference_node(store, node)
            || ast::is_expression_with_type_arguments(store, node)
            || ast::is_type_predicate_node(store, node)
            || ast::is_import_type_node(store, node)
        {
            if let Some(t) = self.get_type_from_type_node(node, false) {
                self.check_type_expandability(Some(t));
                if self.ctx.can_increase_expansion_depth {
                    return;
                }
            }
        }

        let children = {
            let mut children = Vec::new();
            let store = self.store_for_node(node);
            let _ = store.for_each_present_child(node, |child| {
                children.push(child);
                std::ops::ControlFlow::Continue(())
            });
            children
        };
        for child in children {
            let child = Self::node_value(child);
            self.walk_node_for_expandability(child);
            if self.ctx.can_increase_expansion_depth {
                break;
            }
        }
    }

    fn create_recovery_boundary(&mut self) -> RecoveryBoundary<'a> {
        self.ch.check_not_canceled();
        let state = Rc::new(RefCell::new(RecoveryBoundaryState {
            had_error: false,
            tracked_symbols: Vec::new(),
            deferred_reports: Vec::new(),
        }));
        let wrapped_tracker = Rc::new(RefCell::new(self.tracker.take()));
        self.tracker = Some(Box::new(RecoveryTracker {
            wrapped: Rc::clone(&wrapped_tracker),
            state: Rc::clone(&state),
        }));
        RecoveryBoundary {
            state,
            wrapped_tracker,
            old_tracked_symbols: std::mem::take(&mut self.ctx.tracked_symbols),
            old_encountered_error: self.ctx.encountered_error,
            old_approximate_length: self.ctx.approximate_length,
        }
    }

    fn finalize_boundary(&mut self, bound: RecoveryBoundary<'a>) -> bool {
        self.tracker = bound.wrapped_tracker.borrow_mut().take();
        self.ctx.encountered_error = bound.old_encountered_error;
        self.ctx.approximate_length = bound.old_approximate_length;
        self.ctx.tracked_symbols = bound.old_tracked_symbols;
        let (had_error, tracked_symbols, deferred_reports) = {
            let mut state = bound.state.borrow_mut();
            (
                state.had_error,
                std::mem::take(&mut state.tracked_symbols),
                std::mem::take(&mut state.deferred_reports),
            )
        };
        for report in deferred_reports {
            match report {
                DeferredReport::CyclicStructure => self.report_cyclic_structure_error(),
                DeferredReport::InaccessibleThis => self.report_inaccessible_this_error(),
                DeferredReport::InaccessibleUniqueSymbol => {
                    self.report_inaccessible_unique_symbol_error()
                }
                DeferredReport::LikelyUnsafeImportRequired {
                    specifier,
                    symbol_name,
                } => self.report_likely_unsafe_import_required_error(&specifier, &symbol_name),
                DeferredReport::NonSerializableProperty { property_name } => {
                    self.report_non_serializable_property(&property_name)
                }
                DeferredReport::PrivateInBaseOfClassExpression { property_name } => {
                    self.report_private_in_base_of_class_expression(&property_name)
                }
            }
        }
        if had_error {
            return false;
        }
        for tracked in tracked_symbols {
            self.track_symbol_identity(
                tracked.symbol,
                tracked.enclosing_declaration,
                tracked.meaning,
            );
        }
        true
    }

    pub(crate) fn try_reuse_existing_node_helper(
        &mut self,
        existing: ast::Node,
    ) -> Option<ast::Node> {
        let bound = self.create_recovery_boundary();
        let source = self.source_store_for_node(existing);
        let (transformed, bound) = {
            let mut traversal = NodeCopyTraversal::new(self, source, bound);
            let transformed = traversal.visit_node(Some(existing));
            (transformed, traversal.bound)
        };
        if !self.finalize_boundary(bound) {
            return None;
        }
        let store = self.store_for_node(existing);
        self.ctx.approximate_length +=
            (store.loc(existing).end() - store.loc(existing).pos()).max(0) as usize;
        transformed.map(Self::node_value)
    }
}

impl<'a, 'state, 'c, 'e> NodeCopyTraversal<'_, 'a, 'state, 'c, 'e> {
    fn store_for(&self, node: ast::Node) -> &ast::AstStore {
        if node.store_id() == self.source.store_id()
            || node.store_id() == self.builder.e.factory.node_factory.store().store_id()
        {
            ast::AstImportState::store_for(self.source, &self.builder.e.factory.node_factory, node)
        } else {
            self.builder.store_for_node(node)
        }
    }

    fn preserved_node(&self, source: ast::Node) -> Option<ast::Node> {
        self.import_state
            .preserved_node(&self.builder.e.factory.node_factory, source)
    }

    fn record_preserved_node(&mut self, source: ast::Node, imported: ast::Node) -> ast::Node {
        let imported = if source.store_id()
            == self.builder.e.factory.node_factory.store().store_id()
            || imported.store_id() == self.builder.e.factory.node_factory.store().store_id()
        {
            imported
        } else {
            self.preserve_node(source)
        };
        self.import_state.record_preserved_node(
            self.source.store_id(),
            &mut self.builder.e.factory.node_factory,
            source,
            imported,
        )
    }

    fn preserve_node(&mut self, node: ast::Node) -> ast::Node {
        self.builder.register_source_file_for_node(node);
        self.import_state.clone_node_from_store(
            self.source,
            &mut self.builder.e.factory.node_factory,
            node,
        )
    }

    fn clone_source_node_list_to_output(&mut self, list: ast::SourceNodeList<'_>) -> ast::NodeList {
        self.import_state
            .clone_source_node_list(&mut self.builder.e.factory.node_factory, list)
    }

    fn clone_source_modifier_list_to_output(
        &mut self,
        modifiers: ast::SourceModifierList<'_>,
    ) -> ast::ModifierList {
        self.import_state
            .clone_source_modifier_list(&mut self.builder.e.factory.node_factory, modifiers)
    }

    fn clone_source_raw_node_slice_to_output(
        &mut self,
        nodes: ast::SourceRawNodeSlice<'_>,
    ) -> ast::RawNodeSlice {
        self.import_state
            .clone_source_raw_node_slice(&mut self.builder.e.factory.node_factory, nodes)
    }

    fn clone_source_node_list_input_to_output(
        &mut self,
        list: &ast::SourceNodeListInput,
    ) -> ast::NodeList {
        self.import_state.clone_source_node_list_input(
            self.source,
            &mut self.builder.e.factory.node_factory,
            list,
        )
    }

    fn clone_source_modifier_list_input_to_output(
        &mut self,
        modifiers: &ast::SourceModifierListInput,
    ) -> ast::ModifierList {
        self.import_state.clone_source_modifier_list_input(
            self.source,
            &mut self.builder.e.factory.node_factory,
            modifiers,
        )
    }

    fn clone_source_raw_node_slice_input_to_output(
        &mut self,
        nodes: &ast::SourceRawNodeSliceInput,
    ) -> ast::RawNodeSlice {
        self.import_state.clone_source_raw_node_slice_input(
            self.source,
            &mut self.builder.e.factory.node_factory,
            nodes,
        )
    }

    fn output_node_list_from_source_view(
        &mut self,
        list: ast::SourceNodeList<'_>,
    ) -> ast::NodeList {
        self.output_node_list_from_source_snapshot(SourceNodeListSnapshot::from_source(list))
    }

    fn output_node_list_from_source_snapshot(
        &mut self,
        list: SourceNodeListSnapshot,
    ) -> ast::NodeList {
        let nodes =
            if list.source_store_id == self.builder.e.factory.node_factory.store().store_id() {
                list.nodes
            } else {
                list.nodes
                    .into_iter()
                    .map(|node| self.preserve_node(node))
                    .collect::<Vec<_>>()
            };
        if list.missing {
            self.builder
                .e
                .factory
                .node_factory
                .new_missing_node_list(list.loc, list.range)
        } else if list.has_trailing_comma {
            self.builder
                .e
                .factory
                .node_factory
                .new_node_list_with_trailing_comma(list.loc, list.range, nodes, true)
        } else {
            self.builder
                .e
                .factory
                .node_factory
                .new_node_list(list.loc, list.range, nodes)
        }
    }

    fn optional_output_node_list_from_source_view(
        &mut self,
        list: Option<ast::SourceNodeList<'_>>,
    ) -> Option<ast::NodeList> {
        list.map(|list| self.output_node_list_from_source_view(list))
    }

    fn optional_output_node_list_from_source_snapshot(
        &mut self,
        list: Option<SourceNodeListSnapshot>,
    ) -> Option<ast::NodeList> {
        list.map(|list| self.output_node_list_from_source_snapshot(list))
    }

    fn output_modifier_list_from_source_view(
        &mut self,
        modifiers: ast::SourceModifierList<'_>,
    ) -> ast::ModifierList {
        self.output_modifier_list_from_source_snapshot(SourceModifierListSnapshot::from_source(
            modifiers,
        ))
    }

    fn output_modifier_list_from_source_snapshot(
        &mut self,
        modifiers: SourceModifierListSnapshot,
    ) -> ast::ModifierList {
        let nodes = if modifiers.source_store_id
            == self.builder.e.factory.node_factory.store().store_id()
        {
            modifiers.nodes
        } else {
            modifiers
                .nodes
                .into_iter()
                .map(|node| self.preserve_node(node))
                .collect::<Vec<_>>()
        };
        self.builder.e.factory.node_factory.new_modifier_list(
            modifiers.loc,
            modifiers.range,
            nodes,
            modifiers.modifier_flags,
        )
    }

    fn optional_output_modifier_list_from_source_view(
        &mut self,
        modifiers: Option<ast::SourceModifierList<'_>>,
    ) -> Option<ast::ModifierList> {
        modifiers.map(|modifiers| self.output_modifier_list_from_source_view(modifiers))
    }

    fn optional_output_modifier_list_from_source_snapshot(
        &mut self,
        modifiers: Option<SourceModifierListSnapshot>,
    ) -> Option<ast::ModifierList> {
        modifiers.map(|modifiers| self.output_modifier_list_from_source_snapshot(modifiers))
    }

    fn preserved_source_node_matches(
        &self,
        source: Option<ast::Node>,
        output: Option<ast::Node>,
    ) -> bool {
        self.import_state.preserved_source_node_matches(
            &self.builder.e.factory.node_factory,
            source,
            output,
        )
    }

    fn preserved_source_node_list_view_matches(
        &self,
        source: Option<ast::SourceNodeList<'_>>,
        output: Option<ast::NodeList>,
    ) -> bool {
        self.import_state.preserved_source_node_list_view_matches(
            &self.builder.e.factory.node_factory,
            source,
            output,
        )
    }

    fn preserved_source_modifier_list_view_matches(
        &self,
        source: Option<ast::SourceModifierList<'_>>,
        output: Option<ast::ModifierList>,
    ) -> bool {
        self.import_state
            .preserved_source_modifier_list_view_matches(
                &self.builder.e.factory.node_factory,
                source,
                output,
            )
    }

    fn preserved_source_raw_node_slice_view_matches(
        &self,
        source: Option<ast::SourceRawNodeSlice<'_>>,
        output: Option<ast::RawNodeSlice>,
    ) -> bool {
        self.import_state
            .preserved_source_raw_node_slice_view_matches(
                &self.builder.e.factory.node_factory,
                source,
                output,
            )
    }

    fn preserved_source_raw_string_slice_view_matches(
        &self,
        source: Option<ast::SourceRawStringSlice<'_>>,
        output: Option<ast::RawStringSlice>,
    ) -> bool {
        self.import_state
            .preserved_source_raw_string_slice_view_matches(
                &self.builder.e.factory.node_factory,
                source,
                output,
            )
    }

    fn flatten_visited_node(&mut self, visited: ast::Node, out: &mut Vec<ast::Node>) {
        self.import_state.flatten_visited_node(
            self.source,
            &mut self.builder.e.factory.node_factory,
            visited,
            out,
        );
    }

    fn append_visit_slice_result(
        &mut self,
        original: ast::Node,
        visited: Option<ast::Node>,
        out: &mut Vec<ast::Node>,
    ) {
        self.import_state.append_visit_slice_result(
            self.source,
            &mut self.builder.e.factory.node_factory,
            original,
            visited,
            out,
        );
    }

    fn visit_slice(&mut self, nodes: ast::SourceNodeList<'_>) -> Option<Vec<ast::Node>> {
        ast::visit_slice_with(self, nodes)
    }

    fn visit_slice_input(&mut self, nodes: &ast::SourceNodeListInput) -> Option<Vec<ast::Node>> {
        for (index, node) in nodes.iter().enumerate() {
            let visited = self.visit_slice_node(node);
            if visited == Some(node) {
                continue;
            }

            let mut result = Vec::with_capacity(nodes.len());
            result.extend(
                nodes
                    .iter()
                    .take(index)
                    .map(|node| self.import_slice_node(node)),
            );
            self.append_visit_slice_result(node, visited, &mut result);

            for node in nodes.iter().skip(index + 1) {
                let visited = self.visit_slice_node(node);
                self.append_visit_slice_result(node, visited, &mut result);
            }

            return Some(result);
        }

        None
    }

    fn visit_modifier_slice_input(
        &mut self,
        modifiers: &ast::SourceModifierListInput,
    ) -> Option<Vec<ast::Node>> {
        for (index, node) in modifiers.iter().enumerate() {
            let visited = self.visit_slice_node(node);
            if visited == Some(node) {
                continue;
            }

            let modifier_nodes = modifiers.nodes();
            let mut result = Vec::with_capacity(modifier_nodes.len());
            result.extend(
                modifier_nodes
                    .iter()
                    .take(index)
                    .map(|node| self.import_slice_node(*node)),
            );
            self.append_visit_slice_result(node, visited, &mut result);

            for node in modifier_nodes.iter().skip(index + 1).copied() {
                let visited = self.visit_slice_node(node);
                self.append_visit_slice_result(node, visited, &mut result);
            }

            return Some(result);
        }

        None
    }

    fn visit_node(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        let node = node?;
        if self.bound.has_error() {
            return Some(self.clone_node_to_output_factory(node));
        }

        let old_non_local_node = self.non_local_node;
        self.non_local_node = self.node_is_non_local(node);
        let recover = self.bound.start_recovery_scope();
        let scope_cleanup = self.enter_nodecopy_scope(node);
        let result = self.visit_existing_node_tree_symbols(node);
        if let Some(scope_cleanup) = scope_cleanup {
            self.builder.exit_scope(scope_cleanup);
        }
        self.non_local_node = old_non_local_node;

        if self.bound.has_error() {
            let store = self.store_for(node);
            if ast::is_type_node(store, node) && !ast::is_type_predicate_node(store, node) {
                self.bound.end_recovery_scope(recover);
                let node_value = Self::node_value(node);
                return self
                    .builder
                    .get_type_from_type_node(node_value, false)
                    .and_then(|t| self.builder.type_to_type_node(t));
            }
            return Some(self.clone_node_to_output_factory(node));
        }

        result
    }

    fn visit_source_nodes_to_output_list(
        &mut self,
        nodes: Option<ast::SourceNodeListInput>,
    ) -> Option<ast::NodeList> {
        let nodes = nodes?;
        let visited = self.visit_slice_input(&nodes);
        let result = if let Some(visited) = visited.as_ref() {
            self.builder.e.factory.node_factory.new_node_list(
                nodes.loc(),
                nodes.range(),
                visited.iter().copied(),
            )
        } else {
            self.clone_source_node_list_input_to_output(&nodes)
        };
        if self.non_local_node {
            let cloned_nodes = if let Some(visited) = visited {
                visited
            } else {
                nodes
                    .iter()
                    .map(|node| self.preserve_node(node))
                    .collect::<Vec<_>>()
            };
            Some(self.builder.e.factory.node_factory.new_node_list(
                core::new_text_range(-1, -1),
                core::new_text_range(-1, -1),
                cloned_nodes,
            ))
        } else {
            Some(result)
        }
    }

    fn visit_source_modifiers_to_output_list(
        &mut self,
        modifiers: Option<ast::SourceModifierListInput>,
    ) -> Option<ast::ModifierList> {
        let modifiers = modifiers?;
        if let Some(visited) = self.visit_modifier_slice_input(&modifiers) {
            Some(self.builder.e.factory.node_factory.new_modifier_list(
                modifiers.loc(),
                modifiers.range(),
                visited,
                ast::ModifierFlags::NONE,
            ))
        } else {
            Some(self.clone_source_modifier_list_input_to_output(&modifiers))
        }
    }

    fn append_source_raw_node_slice_result_to_output(
        &mut self,
        original: Option<ast::Node>,
        result: Option<ast::Node>,
        out: &mut Vec<Option<ast::Node>>,
    ) {
        self.import_state.append_raw_node_slice_result(
            self.source,
            &mut self.builder.e.factory.node_factory,
            original,
            result,
            out,
        );
    }

    fn visit_source_raw_node_slice_to_output(
        &mut self,
        nodes: Option<ast::SourceRawNodeSliceInput>,
    ) -> Option<ast::RawNodeSlice> {
        let nodes = nodes?;
        if let Some(visited) = self.visit_raw_node_slice_input_to_vec(&nodes) {
            return Some(
                self.builder
                    .e
                    .factory
                    .node_factory
                    .new_raw_node_slice(visited),
            );
        }

        Some(self.clone_source_raw_node_slice_input_to_output(&nodes))
    }

    fn visit_raw_node_slice_input_to_vec(
        &mut self,
        nodes: &ast::SourceRawNodeSliceInput,
    ) -> Option<Vec<Option<ast::Node>>> {
        for (index, node) in nodes.iter().enumerate() {
            let visited = self.visit_raw_slice_node(node);
            if visited == node {
                continue;
            }

            let mut result = Vec::with_capacity(nodes.iter().len());
            result.extend(
                nodes
                    .iter()
                    .take(index)
                    .map(|node| node.map(|node| self.import_raw_slice_node(node))),
            );
            self.append_visited_raw_slice_node(node, visited, &mut result);

            for node in nodes.iter().skip(index + 1) {
                let visited = self.visit_raw_slice_node(node);
                self.append_visited_raw_slice_node(node, visited, &mut result);
            }

            return Some(result);
        }

        None
    }

    fn visit_each_child(&mut self, node: ast::Node) -> ast::Node {
        ast::AstGeneratedVisitEachChild::generated_visit_each_child(self, &node)
    }

    fn lift_to_block(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        self.import_state
            .lift_to_block(self.source, &mut self.builder.e.factory.node_factory, node)
    }

    fn node_is_non_local(&mut self, node: ast::Node) -> bool {
        let Some(enclosing_file) = self.builder.enclosing_file() else {
            return true;
        };
        let original = self.builder.e.most_original(&node);
        let store = self.store_for(original);
        let Some(source_file) = ast::get_source_file_of_node(store, Some(original)) else {
            return true;
        };
        store.as_source_file(source_file).file_name_ref() != enclosing_file.file_name_ref()
    }

    fn clone_node_to_output_factory(&mut self, node: ast::Node) -> ast::Node {
        if node.store_id() == self.builder.e.factory.node_factory.store().store_id() {
            return node;
        }
        if node.store_id() == self.source.store_id() {
            let source = self.source;
            self.builder.register_source_file_for_node(node);
            self.builder
                .e
                .factory
                .node_factory
                .deep_clone_node_from_store(source, node)
        } else {
            panic!("nodecopy cannot resolve node from unrelated AST store")
        }
    }

    fn set_text_range_in_output(&mut self, node: ast::Node, original: ast::Node) -> ast::Node {
        let node_value = Self::node_value(node);
        let original = Self::node_value(original);
        self.builder
            .set_text_range(Some(node_value), Some(original))
            .unwrap_or(node)
    }

    fn visit_each_child_for_nodecopy(&mut self, node: ast::Node) -> ast::Node {
        let visited = self.visit_each_child(node);
        if visited == node && !ast::node_is_synthesized(self.store_for(node), node) {
            self.clone_node_to_output_factory(node)
        } else {
            visited
        }
    }

    fn computed_property_name_literal(&mut self, expression: ast::Node) -> Option<ast::Node> {
        let computed_property_name_type = self.builder.serialize_type_for_expression(expression);
        let computed_property_name_type_store = self.store_for(computed_property_name_type);
        if ast::is_literal_type_node(
            computed_property_name_type_store,
            computed_property_name_type,
        ) {
            return computed_property_name_type_store.literal(computed_property_name_type);
        }

        let evaluated = self.builder.ch.evaluate_entity(expression, None);
        match evaluated.value {
            evaluator::Value::String(value) => Some(
                self.builder
                    .e
                    .factory
                    .node_factory
                    .new_string_literal(value, ast::TOKEN_FLAGS_NONE),
            ),
            evaluator::Value::Number(value) => {
                if jsnum::compare(value, jsnum::Number(0.0)) < 0 {
                    let numeric = self
                        .builder
                        .e
                        .factory
                        .node_factory
                        .new_numeric_literal(&value.to_string()[1..], ast::TOKEN_FLAGS_NONE);
                    Some(
                        self.builder
                            .e
                            .factory
                            .node_factory
                            .new_prefix_unary_expression(ast::KIND_MINUS_TOKEN, numeric),
                    )
                } else {
                    Some(
                        self.builder
                            .e
                            .factory
                            .node_factory
                            .new_numeric_literal(value.to_string(), ast::TOKEN_FLAGS_NONE),
                    )
                }
            }
            _ => {
                let store = self.store_for(computed_property_name_type);
                if ast::is_import_type_node(store, computed_property_name_type) {
                    self.builder
                        .track_computed_name(expression, self.builder.ctx.enclosing_declaration);
                }
                None
            }
        }
    }

    fn update_computed_property_name_with_literal(
        &mut self,
        node: ast::Node,
        literal: ast::Node,
    ) -> ast::Node {
        let literal_store = self.store_for(literal);
        let literal_kind = literal_store.kind(literal);
        let literal_text = literal_store.text(literal).to_string();
        if literal_kind == ast::Kind::StringLiteral
            && scanner::is_identifier_text(&literal_text, core::LANGUAGE_VARIANT_STANDARD)
        {
            return self
                .builder
                .e
                .factory
                .node_factory
                .new_identifier(literal_text);
        }
        if literal_kind == ast::Kind::NumericLiteral && !literal_text.starts_with('-') {
            return literal;
        }
        self.builder
            .e
            .factory
            .node_factory
            .update_computed_property_name_from_store(self.source, node, literal)
    }

    fn visit_existing_node_tree_symbols(&mut self, node: ast::Node) -> Option<ast::Node> {
        let kind = self.store_for(node).kind(node);

        if let Some(name) = self.store_for(node).name(node)
            && self.store_for(name).kind(name) == ast::Kind::ComputedPropertyName
            && !self.builder.ch.has_late_bindable_name(node)
        {
            if !ast::has_dynamic_name(self.store_for(node), node) {
                return Some(self.visit_each_child(node));
            }
            let expression = self.store_for(name).expression(name).unwrap();
            let computed_property_name_type = self.builder.ch.check_computed_property_name(name);
            let computed_property_name_type_is_any =
                self.builder.ch.type_flags(computed_property_name_type) & TYPE_FLAGS_ANY != 0;
            let should_remove_declaration = !((self.builder.ctx.internal_flags
                & nodebuilder::INTERNAL_FLAGS_ALLOW_UNRESOLVED_NAMES
                != 0)
                && ast::is_entity_name_expression(self.store_for(expression), expression)
                && computed_property_name_type_is_any);
            if should_remove_declaration {
                return None;
            }
        }

        let result = match kind {
            ast::Kind::TypeReference
                if self
                    .store_for(node)
                    .type_name(node)
                    .is_some_and(|type_name| {
                        ast::is_identifier(self.store_for(type_name), type_name)
                            && self.store_for(type_name).text(type_name).is_empty()
                    }) =>
            {
                let replacement = self
                    .builder
                    .e
                    .factory
                    .node_factory
                    .new_keyword_type_node(ast::Kind::AnyKeyword);
                self.builder.set_original(&replacement, &node);
                Some(replacement)
            }
            ast::Kind::TypeReference => self.visit_type_reference_or_mark_error(node),
            ast::Kind::TypeQuery => self.visit_type_query_or_mark_error(node),
            ast::Kind::IndexedAccessType => self.visit_indexed_access_or_mark_error(node),
            ast::Kind::TypeOperator => {
                if self.store_for(node).operator(node) == Some(ast::Kind::KeyOfKeyword) {
                    if let Some(result) = self.try_visit_key_of(node) {
                        Some(result)
                    } else {
                        self.bound.mark_error();
                        Some(node)
                    }
                } else if self.store_for(node).operator(node) == Some(ast::Kind::UniqueKeyword)
                    && self
                        .store_for(node)
                        .r#type(node)
                        .is_some_and(|ty| self.store_for(ty).kind(ty) == ast::Kind::SymbolKeyword)
                    && !self.unique_symbol_type_is_in_scope(node)
                {
                    self.bound.mark_error();
                    Some(node)
                } else {
                    Some(self.visit_each_child_for_nodecopy(node))
                }
            }
            ast::Kind::ImportType
                if ast::is_literal_import_type_node(self.store_for(node), node) =>
            {
                self.visit_import_type_node(node)
            }
            ast::Kind::ThisType => Some(node),
            ast::Kind::TypeParameter => {
                let (name, modifiers, expression, constraint_id, default_type_id) = {
                    let store = self.store_for(node);
                    (
                        store.name(node)?,
                        store
                            .source_modifiers(node)
                            .map(SourceModifierListSnapshot::from_source),
                        store.expression(node),
                        store.constraint(node),
                        store.default_type(node),
                    )
                };
                let (_, new_name, _) = self.track_existing_entity_name(name, None);
                let modifiers = self.optional_output_modifier_list_from_source_snapshot(modifiers);
                let constraint = constraint_id.and_then(|n| self.visit_node(Some(n)));
                let default_type = default_type_id.and_then(|n| self.visit_node(Some(n)));
                let expression = expression.and_then(|n| self.visit_node(Some(n)));
                Some(
                    self.builder
                        .e
                        .factory
                        .node_factory
                        .update_type_parameter_declaration_from_store(
                            self.source,
                            node,
                            modifiers,
                            new_name,
                            constraint,
                            expression,
                            default_type,
                        ),
                )
            }
            ast::Kind::ComputedPropertyName
                if self.store_for(node).expression(node).is_some_and(|expr| {
                    ast::is_entity_name_expression(self.store_for(expr), expr)
                }) =>
            {
                let expr = self.store_for(node).expression(node).unwrap();
                let recover = self.bound.start_recovery_scope();
                let (introduces_error, result, _) = self.track_existing_entity_name(expr, None);
                if !introduces_error {
                    Some(
                        self.builder
                            .e
                            .factory
                            .node_factory
                            .update_computed_property_name_from_store(self.source, node, result),
                    )
                } else {
                    self.bound.end_recovery_scope(recover);
                    if let Some(literal) = self.computed_property_name_literal(expr) {
                        Some(self.update_computed_property_name_with_literal(node, literal))
                    } else {
                        Some(self.clone_node_to_output_factory(node))
                    }
                }
            }
            ast::Kind::TypePredicate => self.visit_type_predicate_node(node),
            ast::Kind::ConditionalType => self.visit_conditional_type_node(node),
            ast::Kind::TupleType | ast::Kind::MappedType => {
                let res = self.visit_each_child_for_nodecopy(node);
                self.builder
                    .e
                    .factory
                    .node_factory
                    .mark_checker_synthesized(res);
                self.builder.e.mark_emit_node(&res, printer::EF_SINGLE_LINE);
                Some(res)
            }
            ast::Kind::TypeLiteral
                if self.builder.ctx.flags & nodebuilder::FLAGS_MULTILINE_OBJECT_LITERALS == 0 =>
            {
                let res = self.visit_each_child_for_nodecopy(node);
                self.builder.e.mark_emit_node(&res, printer::EF_SINGLE_LINE);
                Some(res)
            }
            ast::Kind::StringLiteral
                if self.builder.ctx.flags
                    & nodebuilder::FLAGS_USE_SINGLE_QUOTES_FOR_STRING_LITERAL_TYPE
                    != 0 =>
            {
                let cloned = self.clone_node_to_output_factory(node);
                let source_token_flags = self
                    .store_for(node)
                    .token_flags(node)
                    .expect("string literal should have token flags");
                self.builder
                    .e
                    .factory
                    .node_factory
                    .force_cloned_string_literal_single_quote_from(cloned, source_token_flags);
                Some(cloned)
            }
            _ if self.should_add_any_annotation(node) => self.visit_node_with_any_annotation(node),
            _ => Some(self.visit_each_child_for_nodecopy(node)),
        }?;

        let result = self.set_text_range_in_output(result, node);
        if self.bound.has_error() {
            return Some(self.clone_node_to_output_factory(node));
        }
        Some(result)
    }

    fn enter_nodecopy_scope(&mut self, node: ast::Node) -> Option<NodeBuilderScopeCleanup> {
        let store = self.store_for(node);
        if ast::is_function_like(store, Some(node)) {
            let signature = self
                .builder
                .ch
                .get_signature_from_declaration(Self::node_value(node));
            let signature_record = self.builder.ch.signature_record(signature).clone();
            return Some(self.builder.enter_new_scope(
                Some(Self::node_value(node)),
                Some(signature_record.parameters.to_vec()),
                signature_record.type_parameters,
                None,
                None,
            ));
        }
        if ast::is_mapped_type_node(store, node) {
            let type_parameter = store.type_parameter(node)?;
            let symbol = self
                .builder
                .ch
                .get_symbol_of_declaration(Self::node_value(type_parameter))?;
            let type_parameter_type = self.builder.ch.get_declared_type_of_symbol_handle(symbol);
            return Some(self.builder.enter_new_scope(
                Some(Self::node_value(node)),
                None,
                vec![type_parameter_type],
                None,
                None,
            ));
        }
        None
    }

    fn unique_symbol_type_is_in_scope(&mut self, node: ast::Node) -> bool {
        let Some(non_fake_enclosing) = self.get_enclosing_declaration_ignoring_fake_scope() else {
            return false;
        };
        ast::find_ancestor(self.store_for(node), Some(node), |_, ancestor| {
            ancestor == non_fake_enclosing
        })
        .is_some()
    }

    fn get_enclosing_declaration_ignoring_fake_scope(&self) -> Option<ast::Node> {
        let mut enc = self.builder.ctx.enclosing_declaration;
        while let Some(current) = enc {
            if self
                .builder
                .fake_scope_for_signature_declaration(current)
                .is_none()
            {
                break;
            }
            let store = self.builder.store_for_node(current);
            enc = store.parent(current).map(Self::node_value);
        }
        enc
    }

    fn try_visit_simple_type_node(&mut self, node: ast::Node) -> Option<ast::Node> {
        let inner = {
            let store = self.store_for(node);
            ast::skip_parentheses(store, node)
        };
        let inner = Self::node_value(inner);
        match self.store_for(inner).kind(inner) {
            ast::Kind::TypeReference => self.try_visit_type_reference(inner),
            ast::Kind::TypeQuery => self.try_visit_type_query(inner),
            ast::Kind::IndexedAccessType => self.try_visit_indexed_access(inner),
            ast::Kind::TypeOperator
                if self.store_for(inner).operator(inner) == Some(ast::Kind::KeyOfKeyword) =>
            {
                self.try_visit_key_of(inner)
            }
            _ => self.visit_node(Some(node)),
        }
    }

    fn try_visit_indexed_access(&mut self, node: ast::Node) -> Option<ast::Node> {
        let (object_type, index_type) = {
            let store = self.store_for(node);
            (store.object_type(node)?, store.index_type(node)?)
        };
        let result_object_type = self.try_visit_simple_type_node(Self::node_value(object_type))?;
        let index_type = self.visit_node(Some(index_type));
        Some(
            self.builder
                .e
                .factory
                .node_factory
                .update_indexed_access_type_node_from_store(
                    self.source,
                    node,
                    result_object_type,
                    index_type,
                ),
        )
    }

    fn visit_indexed_access_or_mark_error(&mut self, node: ast::Node) -> Option<ast::Node> {
        if let Some(result) = self.try_visit_indexed_access(node) {
            return Some(result);
        }
        self.bound.mark_error();
        Some(node)
    }

    fn try_visit_key_of(&mut self, node: ast::Node) -> Option<ast::Node> {
        let (operator, ty) = {
            let store = self.store_for(node);
            (store.operator(node).unwrap(), store.r#type(node)?)
        };
        let ty = self.try_visit_simple_type_node(Self::node_value(ty))?;
        Some(
            self.builder
                .e
                .factory
                .node_factory
                .update_type_operator_node_from_store(self.source, node, operator, ty),
        )
    }

    fn try_visit_type_query(&mut self, node: ast::Node) -> Option<ast::Node> {
        let source_expr_name = {
            let store = self.store_for(node);
            store.expr_name(node)?
        };
        let type_arguments = self.source.source_type_arguments(node);
        let (introduces_error, expr_name, _) =
            self.track_existing_entity_name(source_expr_name, None);
        let type_arguments = type_arguments.and_then(|nodes| {
            self.visit_source_nodes_to_output_list(Some(ast::SourceNodeListInput::from_source(
                nodes,
            )))
        });
        if !introduces_error {
            return Some(
                self.builder
                    .e
                    .factory
                    .node_factory
                    .update_type_query_node_from_store(
                        self.source,
                        node,
                        expr_name,
                        type_arguments,
                    ),
            );
        }
        let type_args = type_arguments.map(Self::output_node_list_value);
        self.builder
            .serialize_type_name(Self::node_value(source_expr_name), true, type_args)
            .map(|serialized_name| self.set_text_range_in_output(serialized_name, source_expr_name))
    }

    fn visit_type_query_or_mark_error(&mut self, node: ast::Node) -> Option<ast::Node> {
        if let Some(result) = self.try_visit_type_query(node) {
            return Some(result);
        }
        self.bound.mark_error();
        Some(node)
    }

    fn try_visit_type_reference(&mut self, node: ast::Node) -> Option<ast::Node> {
        if ast::is_const_type_reference(self.store_for(node), node) {
            return None;
        }
        let symbol = self.builder.ch.node_resolved_symbol(node)?;
        if self.builder.ch.missing_name_symbol_identity_flags(symbol)
            & ast::SYMBOL_FLAGS_TYPE_PARAMETER
            != 0
        {
            let declared_type = self
                .builder
                .ch
                .get_declared_type_of_symbol_identity_or_error(symbol);
            if let Some(mapper) = self.builder.ctx.mapper
                && self
                    .builder
                    .ch
                    .map_type_mapper_handle(mapper, declared_type)
                    != declared_type
            {
                return None;
            }
        }
        let type_name = self.store_for(node).type_name(node)?;
        let type_arguments = self.source.source_type_arguments(node);
        let (introduces_error, new_name, _) = self.track_existing_entity_name(type_name, None);
        let type_arguments = type_arguments.and_then(|nodes| {
            self.visit_source_nodes_to_output_list(Some(ast::SourceNodeListInput::from_source(
                nodes,
            )))
        });
        if !introduces_error {
            return Some(
                self.builder
                    .e
                    .factory
                    .node_factory
                    .update_type_reference_node_from_store(
                        self.source,
                        node,
                        new_name,
                        type_arguments,
                    ),
            );
        }
        let type_args = type_arguments.map(Self::output_node_list_value);
        self.builder
            .serialize_type_name(Self::node_value(type_name), false, type_args)
    }

    fn visit_type_reference_or_mark_error(&mut self, node: ast::Node) -> Option<ast::Node> {
        if let Some(result) = self.try_visit_type_reference(node) {
            return Some(result);
        }
        self.bound.mark_error();
        Some(node)
    }

    fn visit_import_type_node(&mut self, node: ast::Node) -> Option<ast::Node> {
        let (has_assert_attributes, argument, attributes, qualifier, is_type_of) = {
            let store = self.store_for(node);
            let attributes = store.attributes(node);
            (
                attributes.is_some_and(|attributes| {
                    self.store_for(attributes).token(attributes) == Some(ast::Kind::AssertKeyword)
                }),
                store.argument(node)?,
                attributes,
                store.qualifier(node),
                store.is_type_of(node).unwrap_or(false),
            )
        };
        let type_arguments = self.source.source_type_arguments(node);
        if has_assert_attributes {
            self.bound.mark_error();
            return Some(node);
        }
        let node_value = Self::node_value(node);
        if self
            .builder
            .get_type_from_type_node(node_value, true)
            .is_none()
        {
            self.bound.mark_error();
            return Some(node);
        }
        let literal = self.store_for(argument).literal(argument)?;
        let specifier = self.rewrite_module_specifier(node, literal);
        let visited_specifier = if specifier == literal {
            self.visit_node(Some(specifier))?
        } else {
            specifier
        };
        let visited_specifier = self.record_preserved_node(specifier, visited_specifier);
        let argument = if visited_specifier != literal {
            self.builder
                .e
                .factory
                .node_factory
                .new_literal_type_node(visited_specifier)
        } else if specifier != literal {
            self.builder
                .e
                .factory
                .node_factory
                .new_literal_type_node(specifier)
        } else {
            argument
        };
        let attributes = attributes.and_then(|n| {
            self.visit_node(Some(n))
                .map(|v| self.record_preserved_node(n, v))
        });
        let qualifier = qualifier.and_then(|n| {
            self.visit_node(Some(n))
                .map(|v| self.record_preserved_node(n, v))
        });
        let type_arguments = type_arguments.and_then(|nodes| {
            self.visit_source_nodes_to_output_list(Some(ast::SourceNodeListInput::from_source(
                nodes,
            )))
        });
        let updated = if node.store_id() == self.builder.e.factory.node_factory.store().store_id() {
            self.builder.e.factory.node_factory.update_import_type_node(
                node,
                is_type_of,
                argument,
                attributes,
                qualifier,
                type_arguments,
            )
        } else {
            self.builder
                .e
                .factory
                .node_factory
                .update_import_type_node_from_store(
                    self.source,
                    node,
                    is_type_of,
                    argument,
                    attributes,
                    qualifier,
                    type_arguments,
                )
        };
        Some(updated)
    }

    fn visit_type_predicate_node(&mut self, node: ast::Node) -> Option<ast::Node> {
        let (parameter_name, asserts_modifier, type_node) = {
            let store = self.store_for(node);
            (
                store.parameter_name(node)?,
                store.asserts_modifier(node),
                store.r#type(node),
            )
        };
        let parameter_name = if ast::is_identifier(self.store_for(parameter_name), parameter_name) {
            let (introduces_error, result, _) =
                self.track_existing_entity_name(parameter_name, None);
            if introduces_error {
                self.bound.mark_error();
            }
            self.clone_node_to_output_factory(result)
        } else {
            self.clone_node_to_output_factory(parameter_name)
        };
        let asserts_modifier = asserts_modifier
            .and_then(|n| self.visit_node(Some(n)))
            .map(|n| self.clone_node_to_output_factory(n));
        let type_node = type_node
            .and_then(|n| self.visit_node(Some(n)))
            .map(|n| self.clone_node_to_output_factory(n));
        let source = self.source;
        Some(
            self.builder
                .e
                .factory
                .node_factory
                .update_type_predicate_node_from_store(
                    source,
                    node,
                    asserts_modifier,
                    parameter_name,
                    type_node,
                ),
        )
    }

    fn visit_conditional_type_node(&mut self, node: ast::Node) -> Option<ast::Node> {
        let (check_type, extends_type, true_type, false_type) = {
            let store = self.store_for(node);
            (
                store.check_type(node),
                store.extends_type(node),
                store.true_type(node),
                store.false_type(node),
            )
        };
        let check_type = check_type
            .and_then(|n| self.visit_node(Some(n)))
            .map(|n| self.clone_node_to_output_factory(n));
        let infer_type_parameters = self
            .builder
            .ch
            .get_infer_type_parameters(Self::node_value(node));
        let scope_cleanup = self.builder.enter_new_scope(
            Some(Self::node_value(node)),
            None,
            infer_type_parameters,
            None,
            None,
        );
        let extends_type = extends_type
            .and_then(|n| self.visit_node(Some(n)))
            .map(|n| self.clone_node_to_output_factory(n));
        let true_type = true_type
            .and_then(|n| self.visit_node(Some(n)))
            .map(|n| self.clone_node_to_output_factory(n));
        self.builder.exit_scope(scope_cleanup);
        let false_type = false_type
            .and_then(|n| self.visit_node(Some(n)))
            .map(|n| self.clone_node_to_output_factory(n));
        let source = self.source;
        Some(
            self.builder
                .e
                .factory
                .node_factory
                .update_conditional_type_node_from_store(
                    source,
                    node,
                    check_type,
                    extends_type,
                    true_type,
                    false_type,
                ),
        )
    }

    fn should_add_any_annotation(&mut self, node: ast::Node) -> bool {
        let store = self.store_for(node);
        (ast::is_function_like(store, Some(node)) && store.r#type(node).is_none())
            || (ast::is_property_declaration(store, node)
                && store.r#type(node).is_none()
                && store.initializer(node).is_none())
            || (ast::is_property_signature_declaration(store, node)
                && store.r#type(node).is_none()
                && store.initializer(node).is_none())
            || (ast::is_parameter_declaration(store, node)
                && store.r#type(node).is_none()
                && store.initializer(node).is_none())
    }

    fn visit_node_with_any_annotation(&mut self, node: ast::Node) -> Option<ast::Node> {
        let visited = self.visit_each_child_for_nodecopy(node);
        let kind = self.store_for(visited).kind(visited);
        let new_type = self
            .builder
            .e
            .factory
            .node_factory
            .new_keyword_type_node(ast::Kind::AnyKeyword);
        match kind {
            ast::Kind::PropertyDeclaration => {
                let (modifiers, name, postfix_token) = {
                    let store = self.store_for(visited);
                    (
                        store
                            .source_modifiers(visited)
                            .map(SourceModifierListSnapshot::from_source),
                        store.name(visited),
                        store.postfix_token(visited),
                    )
                };
                let modifiers = self.optional_output_modifier_list_from_source_snapshot(modifiers);
                Some(
                    self.builder
                        .e
                        .factory
                        .node_factory
                        .update_property_declaration(
                            visited,
                            modifiers,
                            name,
                            postfix_token,
                            new_type,
                            None,
                        ),
                )
            }
            ast::Kind::PropertySignature => {
                let (modifiers, name, postfix_token) = {
                    let store = self.store_for(visited);
                    (
                        store
                            .source_modifiers(visited)
                            .map(SourceModifierListSnapshot::from_source),
                        store.name(visited),
                        store.postfix_token(visited),
                    )
                };
                let modifiers = self.optional_output_modifier_list_from_source_snapshot(modifiers);
                Some(
                    self.builder
                        .e
                        .factory
                        .node_factory
                        .update_property_signature_declaration(
                            visited,
                            modifiers,
                            name,
                            postfix_token,
                            new_type,
                            None,
                        ),
                )
            }
            ast::Kind::Parameter => {
                let (dot_dot_dot_token, name, question_token) = {
                    let store = self.store_for(visited);
                    (
                        store.dot_dot_dot_token(visited),
                        store.name(visited),
                        store.question_token(visited),
                    )
                };
                Some(
                    self.builder
                        .e
                        .factory
                        .node_factory
                        .update_parameter_declaration(
                            visited,
                            None,
                            dot_dot_dot_token,
                            name,
                            question_token,
                            new_type,
                            None,
                        ),
                )
            }
            ast::Kind::MethodSignature => {
                let (modifiers, name, postfix_token, type_parameters, parameters) = {
                    let store = self.store_for(visited);
                    (
                        store
                            .source_modifiers(visited)
                            .map(SourceModifierListSnapshot::from_source),
                        store.name(visited),
                        store.postfix_token(visited),
                        store
                            .source_type_parameters(visited)
                            .map(SourceNodeListSnapshot::from_source),
                        store
                            .source_parameters(visited)
                            .map(SourceNodeListSnapshot::from_source)
                            .expect("method signature parameters"),
                    )
                };
                let modifiers = self.optional_output_modifier_list_from_source_snapshot(modifiers);
                let type_parameters =
                    self.optional_output_node_list_from_source_snapshot(type_parameters);
                let parameters = self.output_node_list_from_source_snapshot(parameters);
                Some(
                    self.builder
                        .e
                        .factory
                        .node_factory
                        .update_method_signature_declaration(
                            visited,
                            modifiers,
                            name,
                            postfix_token,
                            type_parameters,
                            parameters,
                            new_type,
                        ),
                )
            }
            ast::Kind::CallSignature => {
                let (type_parameters, parameters) = {
                    let store = self.store_for(visited);
                    (
                        store
                            .source_type_parameters(visited)
                            .map(SourceNodeListSnapshot::from_source),
                        store
                            .source_parameters(visited)
                            .map(SourceNodeListSnapshot::from_source)
                            .expect("call signature parameters"),
                    )
                };
                let type_parameters =
                    self.optional_output_node_list_from_source_snapshot(type_parameters);
                let parameters = self.output_node_list_from_source_snapshot(parameters);
                Some(
                    self.builder
                        .e
                        .factory
                        .node_factory
                        .update_call_signature_declaration(
                            visited,
                            type_parameters,
                            parameters,
                            new_type,
                        ),
                )
            }
            ast::Kind::ConstructSignature => {
                let (type_parameters, parameters) = {
                    let store = self.store_for(visited);
                    (
                        store
                            .source_type_parameters(visited)
                            .map(SourceNodeListSnapshot::from_source),
                        store
                            .source_parameters(visited)
                            .map(SourceNodeListSnapshot::from_source)
                            .expect("construct signature parameters"),
                    )
                };
                let type_parameters =
                    self.optional_output_node_list_from_source_snapshot(type_parameters);
                let parameters = self.output_node_list_from_source_snapshot(parameters);
                Some(
                    self.builder
                        .e
                        .factory
                        .node_factory
                        .update_construct_signature_declaration(
                            visited,
                            type_parameters,
                            parameters,
                            new_type,
                        ),
                )
            }
            ast::Kind::IndexSignature => {
                let (modifiers, parameters) = {
                    let store = self.store_for(visited);
                    (
                        store
                            .source_modifiers(visited)
                            .map(SourceModifierListSnapshot::from_source),
                        store
                            .source_parameters(visited)
                            .map(SourceNodeListSnapshot::from_source)
                            .expect("index signature parameters"),
                    )
                };
                let modifiers = self.optional_output_modifier_list_from_source_snapshot(modifiers);
                let parameters = self.output_node_list_from_source_snapshot(parameters);
                Some(
                    self.builder
                        .e
                        .factory
                        .node_factory
                        .update_index_signature_declaration(
                            visited, modifiers, parameters, new_type,
                        ),
                )
            }
            ast::Kind::FunctionType => {
                let (type_parameters, parameters) = {
                    let store = self.store_for(visited);
                    (
                        store
                            .source_type_parameters(visited)
                            .map(SourceNodeListSnapshot::from_source),
                        store
                            .source_parameters(visited)
                            .map(SourceNodeListSnapshot::from_source)
                            .expect("function type parameters"),
                    )
                };
                let type_parameters =
                    self.optional_output_node_list_from_source_snapshot(type_parameters);
                let parameters = self.output_node_list_from_source_snapshot(parameters);
                Some(
                    self.builder
                        .e
                        .factory
                        .node_factory
                        .update_function_type_node(visited, type_parameters, parameters, new_type),
                )
            }
            ast::Kind::ConstructorType => {
                let (modifiers, type_parameters, parameters) = {
                    let store = self.store_for(visited);
                    (
                        store
                            .source_modifiers(visited)
                            .map(SourceModifierListSnapshot::from_source),
                        store
                            .source_type_parameters(visited)
                            .map(SourceNodeListSnapshot::from_source),
                        store
                            .source_parameters(visited)
                            .map(SourceNodeListSnapshot::from_source)
                            .expect("constructor type parameters"),
                    )
                };
                let modifiers = self.optional_output_modifier_list_from_source_snapshot(modifiers);
                let type_parameters =
                    self.optional_output_node_list_from_source_snapshot(type_parameters);
                let parameters = self.output_node_list_from_source_snapshot(parameters);
                Some(
                    self.builder
                        .e
                        .factory
                        .node_factory
                        .update_constructor_type_node(
                            visited,
                            modifiers,
                            type_parameters,
                            parameters,
                            new_type,
                        ),
                )
            }
            _ => Some(visited),
        }
    }

    fn track_existing_entity_name(
        &mut self,
        node: ast::Node,
        override_enclosing: Option<ast::Node>,
    ) -> (bool, ast::Node, Option<SymbolIdentity>) {
        let enclosing_declaration = override_enclosing.or(self.builder.ctx.enclosing_declaration);
        let Some(leftmost) = ast::get_first_identifier(self.store_for(node), node) else {
            let cloned = self.clone_node_to_output_factory(node);
            return (false, self.set_text_range_in_output(cloned, node), None);
        };

        {
            let store = self.store_for(node);
            if ast::is_in_js_file(store, node) {
                let leftmost_parent = store.parent(leftmost);
                let is_common_js_export = ast::is_exports_identifier(store, leftmost)
                    || leftmost_parent.is_some_and(|parent| {
                        ast::is_module_exports_access_expression(store, parent)
                            || (ast::is_qualified_name(store, parent)
                                && store
                                    .left(parent)
                                    .is_some_and(|left| ast::is_module_identifier(store, left))
                                && store
                                    .right(parent)
                                    .is_some_and(|right| ast::is_exports_identifier(store, right)))
                    });
                if is_common_js_export {
                    let cloned = self.clone_node_to_output_factory(node);
                    return (true, self.set_text_range_in_output(cloned, node), None);
                }
            }
        }

        let (meaning, node_is_declaration_name) = {
            let store = self.store_for(node);
            (
                crate::emitresolver::get_meaning_of_entity_name_reference(store, node),
                ast::is_declaration_name(store, node),
            )
        };
        let leftmost_ref = Self::node_value(leftmost);
        let mut introduces_error = false;
        if ast::is_this_identifier(self.store_for(leftmost), leftmost) {
            let container = self
                .builder
                .ch
                .get_this_container(leftmost_ref, false, false);
            let symbol = self.builder.ch.get_symbol_of_declaration(container);
            if self
                .builder
                .ch
                .is_symbol_accessible_by_identity(
                    symbol.map(SymbolIdentity::from_symbol_handle),
                    Some(leftmost_ref),
                    meaning,
                    false,
                )
                .accessibility
                != printer::SYMBOL_ACCESSIBILITY_ACCESSIBLE
            {
                introduces_error = true;
                self.bound
                    .mark_error_with_report(DeferredReport::InaccessibleThis);
            }
            let attached = self.attach_symbol_to_leftmost_identifier(
                leftmost,
                node,
                symbol.map(SymbolIdentity::from_symbol_handle),
            );
            return (introduces_error, attached, None);
        }

        let mut symbol =
            self.builder
                .ch
                .resolve_entity_name(leftmost_ref, meaning, true, true, None);

        if self.builder.ctx.enclosing_declaration.is_some()
            && !symbol.as_ref().is_some_and(|symbol| {
                self.symbol_identity_flags(*symbol)
                    .intersects(ast::SYMBOL_FLAGS_TYPE_PARAMETER)
            })
        {
            symbol = self.get_export_symbol_of_value_symbol_identity_if_exported(symbol);
            let enclosing_declaration = self.builder.ctx.enclosing_declaration;
            let symbol_at_location = self.builder.resolve_entity_name_in_builder_scope(
                leftmost_ref,
                meaning,
                true,
                true,
                enclosing_declaration,
            );
            // Some declarations may be transplanted to a new location.
            // When this happens we need to make sure that the name has the same meaning at both locations
            // We also check for the unknownSymbol because when we create a fake scope some parameters may actually not be usable
            // either because they are the expanded rest parameter,
            // or because they are the newly added parameters from the tuple, which might have different meanings in the original context
            let unusable_parameter_symbol = symbol_at_location
                .is_some_and(|symbol| self.builder.ch.is_unknown_symbol_identity(symbol));
            let mismatched_symbol_at_location = symbol_at_location.is_some_and(|at_location| {
                symbol.is_some_and(|symbol| {
                    let exported_at_location = self
                        .get_export_symbol_of_value_symbol_identity_if_exported(Some(at_location))
                        .unwrap_or(at_location);
                    !self
                        .builder
                        .ch
                        .same_symbol_identity(exported_at_location, symbol)
                })
            });
            if unusable_parameter_symbol
                || symbol_at_location.is_none() && symbol.is_some()
                || mismatched_symbol_at_location
            {
                // In isolated declaration we will not do rest parameter expansion so there is no need to report on these.
                if !unusable_parameter_symbol {
                    self.builder.report_inference_fallback(node);
                }
                let cloned = self.clone_node_to_output_factory(node);
                return (true, self.set_text_range_in_output(cloned, node), symbol);
            }
            symbol = symbol_at_location;
        }

        if let Some(symbol) = symbol {
            let symbol_flags = self.symbol_identity_flags(symbol);
            let value_declaration = self.symbol_identity_value_declaration(symbol);
            if symbol_flags.intersects(ast::SYMBOL_FLAGS_FUNCTION_SCOPED_VARIABLE) {
                if let Some(value_declaration) = value_declaration {
                    let declaration_store = self.store_for(value_declaration);
                    if ast::is_part_of_parameter_declaration(declaration_store, value_declaration) {
                        let attached =
                            self.attach_symbol_to_leftmost_identifier(leftmost, node, Some(symbol));
                        return (introduces_error, attached, None);
                    }
                }
            }
            if !symbol_flags.intersects(ast::SYMBOL_FLAGS_TYPE_PARAMETER)
                && !node_is_declaration_name
                && self
                    .builder
                    .ch
                    .is_symbol_accessible_by_identity(
                        Some(symbol),
                        enclosing_declaration.and_then(|enclosing_declaration| {
                            self.builder
                                .checker_accessible_enclosing_declaration(enclosing_declaration)
                        }),
                        meaning,
                        false,
                    )
                    .accessibility
                    != printer::SYMBOL_ACCESSIBILITY_ACCESSIBLE
            {
                self.builder.report_inference_fallback(node);
                introduces_error = true;
            } else {
                self.bound
                    .track_symbol_identity(symbol, enclosing_declaration, meaning);
            }
            let attached = self.attach_symbol_to_leftmost_identifier(leftmost, node, Some(symbol));
            return (introduces_error, attached, None);
        }

        let cloned = self.clone_node_to_output_factory(node);
        (
            introduces_error,
            self.set_text_range_in_output(cloned, node),
            None,
        )
    }

    fn attach_symbol_to_leftmost_identifier(
        &mut self,
        leftmost: ast::Node,
        node: ast::Node,
        symbol: Option<SymbolIdentity>,
    ) -> ast::Node {
        if node == leftmost {
            let store = self.store_for(node);
            let text = store.text(node).to_string();
            let name = if let Some(symbol) = symbol {
                if self
                    .builder
                    .ch
                    .symbol_identity_flags(symbol)
                    .intersects(ast::SYMBOL_FLAGS_TYPE_PARAMETER)
                {
                    let declared = self
                        .builder
                        .ch
                        .get_declared_type_of_symbol_identity_or_error(symbol);
                    Some(self.builder.type_parameter_to_name(declared))
                } else {
                    None
                }
                .unwrap_or_else(|| {
                    let id = self
                        .builder
                        .e
                        .factory
                        .node_factory
                        .new_identifier(text.clone());
                    let id_value = Self::node_value(id);
                    self.builder.id_to_symbol.insert(id_value, symbol);
                    id
                })
            } else {
                self.builder.e.factory.node_factory.new_identifier(text)
            };
            self.builder
                .e
                .mark_emit_node(&name, printer::EF_NO_ASCII_ESCAPING);
            return self.set_text_range_in_output(name, node);
        }
        let visited = self.visit_each_child(node);
        self.set_text_range_in_output(visited, node)
    }

    fn rewrite_module_specifier(&mut self, parent: ast::Node, lit: ast::Node) -> ast::Node {
        let new_name = self.get_module_specifier_override(parent, lit);
        if new_name.is_empty() {
            return lit;
        }
        let res = self
            .builder
            .e
            .factory
            .node_factory
            .new_string_literal(new_name, ast::TOKEN_FLAGS_NONE);
        self.builder.set_original(&res, &lit);
        res
    }

    fn get_module_specifier_override(&mut self, parent: ast::Node, lit: ast::Node) -> String {
        let Some(enclosing_file) = self.builder.enclosing_file() else {
            return String::new();
        };
        let lit_store = self.store_for(lit);
        let Some(source_file) = ast::get_source_file_of_node(lit_store, Some(lit)) else {
            return String::new();
        };
        if lit_store.as_source_file(source_file).file_name_ref() == enclosing_file.file_name_ref() {
            return String::new();
        }
        let original_name = lit_store.text(lit).to_string();
        let attributes = self.store_for(parent).attributes(parent);
        let mode = attributes
            .map(|attributes| {
                self.builder
                    .ch
                    .get_resolution_mode_override(Self::node_value(attributes), false)
            })
            .unwrap_or(core::ResolutionMode::None);
        let node_symbol = self.builder.ch.node_resolved_symbol(parent);
        let meaning = if self.store_for(parent).is_type_of(parent).unwrap_or(false) {
            ast::SYMBOL_FLAGS_VALUE
        } else {
            ast::SYMBOL_FLAGS_TYPE
        };
        let enclosing_declaration = self.builder.ctx.enclosing_declaration;
        let parent_symbol = node_symbol.and_then(|node_symbol| {
            if self
                .builder
                .is_symbol_accessible_in_builder_scope_by_identity(
                    Some(node_symbol),
                    enclosing_declaration,
                    meaning,
                    false,
                )
                .accessibility
                == printer::SYMBOL_ACCESSIBILITY_ACCESSIBLE
            {
                self.builder
                    .lookup_symbol_identity_chain(node_symbol, meaning, true)
                    .into_iter()
                    .next()
            } else {
                None
            }
        });
        let mut name = if parent_symbol.as_ref().is_some_and(|symbol| {
            self.builder
                .ch
                .symbol_identity_flags(*symbol)
                .intersects(ast::SYMBOL_FLAGS_MODULE)
                && self
                    .builder
                    .ch
                    .symbol_identity_name(*symbol)
                    .starts_with('"')
        }) {
            self.builder
                .get_specifier_for_module_symbol_identity(parent_symbol.unwrap(), mode)
        } else {
            let target_file = self
                .builder
                .ch
                .get_external_module_file_from_declaration(Self::node_value(parent));
            target_file
                .and_then(|target_file| {
                    let symbol = self.builder.ch.source_file_symbol(target_file)?;
                    Some(
                        self.builder
                            .get_specifier_for_module_symbol_handle(symbol, mode),
                    )
                })
                .unwrap_or_default()
        };
        if !name.is_empty() && name.contains("/node_modules/") {
            self.builder.ctx.encountered_error = true;
            self.bound
                .mark_error_with_report(DeferredReport::LikelyUnsafeImportRequired {
                    specifier: name.clone(),
                    symbol_name: String::new(),
                });
        }
        if name == original_name {
            name.clear();
        }
        name
    }
}

impl ast::NodeSliceTraversal for NodeCopyTraversal<'_, '_, '_, '_, '_> {
    fn visit_slice_node(&mut self, node: ast::Node) -> Option<ast::Node> {
        self.visit_node(Some(node))
    }

    fn import_slice_node(&mut self, node: ast::Node) -> ast::Node {
        self.preserve_node(node)
    }

    fn append_visited_slice_node(
        &mut self,
        original: ast::Node,
        visited: Option<ast::Node>,
        out: &mut Vec<ast::Node>,
    ) {
        self.append_visit_slice_result(original, visited, out);
    }
}

impl ast::RawNodeSliceTraversal for NodeCopyTraversal<'_, '_, '_, '_, '_> {
    fn visit_raw_slice_node(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        match node {
            Some(node) => self.visit_node(Some(node)),
            None => self.visit_node(None),
        }
    }

    fn import_raw_slice_node(&mut self, node: ast::Node) -> ast::Node {
        self.preserve_node(node)
    }

    fn append_visited_raw_slice_node(
        &mut self,
        original: Option<ast::Node>,
        visited: Option<ast::Node>,
        out: &mut Vec<Option<ast::Node>>,
    ) {
        self.append_source_raw_node_slice_result_to_output(original, visited, out);
    }
}

impl<'source, 'a, 'state, 'c, 'e> ast::AstVisitEachChildRuntime<'source>
    for NodeCopyTraversal<'_, 'a, 'state, 'c, 'e>
where
    'a: 'source,
{
    fn source_store(&self) -> &ast::AstStore {
        self.source
    }

    fn factory(&self) -> &ts_ast::NodeFactory {
        &self.builder.e.factory.node_factory
    }

    fn factory_mut(&mut self) -> &mut ast::NodeFactory {
        &mut self.builder.e.factory.node_factory
    }

    fn preserved_node(&self, source: ast::Node) -> Option<ast::Node> {
        NodeCopyTraversal::preserved_node(self, source)
    }

    fn preserve_node(&mut self, node: ast::Node) -> ast::Node {
        NodeCopyTraversal::preserve_node(self, node)
    }

    fn record_preserved_node(&mut self, source: ast::Node, imported: ast::Node) -> ast::Node {
        NodeCopyTraversal::record_preserved_node(self, source, imported)
    }

    fn preserved_source_node_matches(
        &self,
        source: Option<ast::Node>,
        output: Option<ast::Node>,
    ) -> bool {
        NodeCopyTraversal::preserved_source_node_matches(self, source, output)
    }

    fn update_source_file_from_visited(
        &mut self,
        node: ast::Node,
        statements: Option<ast::NodeList>,
        end_of_file_token: Option<ast::Node>,
        source_unchanged: bool,
    ) -> ast::Node {
        let source = self.source;
        if source_unchanged {
            return self.record_preserved_node(node, node);
        }
        let source_data = source.as_source_file(node).clone();
        let updated = self
            .builder
            .e
            .factory
            .node_factory
            .update_source_file_from_store(
                source,
                node,
                &source_data,
                statements,
                end_of_file_token,
            );
        updated
    }

    fn visit_node(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        NodeCopyTraversal::visit_node(self, node)
    }

    fn visit_token(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        NodeCopyTraversal::visit_node(self, node)
    }

    fn visit_nodes_input(
        &mut self,
        nodes: Option<ast::SourceNodeListInput>,
    ) -> Option<ast::NodeList> {
        NodeCopyTraversal::visit_source_nodes_to_output_list(self, nodes)
    }

    fn visit_modifiers_input(
        &mut self,
        modifiers: Option<ast::SourceModifierListInput>,
    ) -> Option<ast::ModifierList> {
        NodeCopyTraversal::visit_source_modifiers_to_output_list(self, modifiers)
    }

    fn visit_parameters_input(
        &mut self,
        nodes: Option<ast::SourceNodeListInput>,
    ) -> Option<ast::NodeList> {
        NodeCopyTraversal::visit_source_nodes_to_output_list(self, nodes)
    }

    fn visit_function_body(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        NodeCopyTraversal::visit_node(self, node)
    }

    fn visit_iteration_body(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        let visited = NodeCopyTraversal::visit_node(self, node);
        self.lift_to_block(visited)
    }

    fn visit_top_level_statements_input(
        &mut self,
        nodes: Option<ast::SourceNodeListInput>,
    ) -> Option<ast::NodeList> {
        NodeCopyTraversal::visit_source_nodes_to_output_list(self, nodes)
    }

    fn visit_embedded_statement(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        let visited = NodeCopyTraversal::visit_node(self, node);
        self.lift_to_block(visited)
    }

    fn visit_raw_node_slice_input(
        &mut self,
        nodes: Option<ast::SourceRawNodeSliceInput>,
    ) -> Option<ast::RawNodeSlice> {
        NodeCopyTraversal::visit_source_raw_node_slice_to_output(self, nodes)
    }
}

impl<'source, 'a, 'state, 'c, 'e> ast::AstGeneratedVisitEachChild<'source>
    for NodeCopyTraversal<'_, 'a, 'state, 'c, 'e>
where
    'a: 'source,
{
}
