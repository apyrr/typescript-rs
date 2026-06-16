use std::sync::Arc;

use ts_ast as ast;
use ts_collections as collections;

pub struct ProgramBindingState {
    root: ast::Node,
    symbol_store: ast::ProgramSymbolStore,
    symbol_count: i32,
    classifiable_names: collections::Set<String>,
    global_exports: ast::SymbolHandleTable,
    js_global_augmentations: ast::SymbolHandleTable,
    pattern_ambient_modules: Vec<ast::PatternAmbientModule>,
    symbol: Option<ast::SymbolHandle>,
    end_flow_node: Option<ast::FlowRef>,
    flow_graph: Arc<ast::FlowGraph>,
    common_js_module_indicator: Option<ast::Node>,
    external_module_indicator: Option<ast::Node>,
    nested_cjs_exports: Vec<ast::Node>,
    bind_suggestion_diagnostics: Vec<ast::Diagnostic>,
    bind_diagnostics: Vec<ast::Diagnostic>,
    flags: ast::NodeFlags,
    locals_by_container: ast::StoreNodeMap<ast::SymbolHandleTable>,
    next_container_by_node: ast::StoreNodeMap<ast::Node>,
    declaration_symbols_by_node: ast::StoreNodeMap<ast::SymbolHandle>,
    exportable_local_symbols_by_node: ast::StoreNodeMap<ast::SymbolHandle>,
    flow_nodes_by_node: ast::StoreNodeMap<ast::FlowRef>,
    return_flow_nodes_by_node: ast::StoreNodeMap<ast::FlowRef>,
    body_end_flow_nodes_by_node: ast::StoreNodeMap<ast::FlowRef>,
    fallthrough_flow_nodes_by_node: ast::StoreNodeMap<ast::FlowRef>,
    binder_flags_by_node: ast::StoreNodeMap<BinderFlagUpdate>,
}

#[derive(Clone, Copy, Default)]
pub struct BinderFlagUpdate {
    added: ast::NodeFlags,
    removed: ast::NodeFlags,
}

impl ProgramBindingState {
    pub(crate) fn new(root: ast::Node, store: &ast::AstStore) -> Self {
        Self {
            root,
            symbol_store: ast::ProgramSymbolStore::new(),
            symbol_count: 0,
            classifiable_names: collections::Set::new(),
            global_exports: ast::SymbolHandleTable::default(),
            js_global_augmentations: ast::SymbolHandleTable::default(),
            pattern_ambient_modules: Vec::new(),
            symbol: None,
            end_flow_node: None,
            flow_graph: Arc::new(ast::FlowGraph::default()),
            common_js_module_indicator: None,
            external_module_indicator: None,
            nested_cjs_exports: Vec::new(),
            bind_suggestion_diagnostics: Vec::new(),
            bind_diagnostics: Vec::new(),
            flags: ast::NodeFlags::empty(),
            locals_by_container: store.new_node_map(),
            next_container_by_node: store.new_node_map(),
            declaration_symbols_by_node: store.new_node_map(),
            exportable_local_symbols_by_node: store.new_node_map(),
            flow_nodes_by_node: store.new_node_map(),
            return_flow_nodes_by_node: store.new_node_map(),
            body_end_flow_nodes_by_node: store.new_node_map(),
            fallthrough_flow_nodes_by_node: store.new_node_map(),
            binder_flags_by_node: store.new_node_map(),
        }
    }

    pub fn root(&self) -> ast::Node {
        self.root
    }

    pub fn symbol_owner_key(&self) -> ast::ProgramSymbolOwnerKey {
        self.symbol_store.owner_key()
    }

    pub fn symbol_id(&self, symbol: ast::SymbolHandle) -> ast::SymbolId {
        self.symbol_store.symbol_id(symbol)
    }

    pub fn private_identifier_symbol_name(
        &self,
        symbol: ast::SymbolHandle,
        description: &str,
    ) -> String {
        self.symbol_store
            .private_identifier_symbol_name(symbol, description)
    }

    pub fn unique_es_symbol_type_name(
        &self,
        symbol: ast::SymbolHandle,
        symbol_name: &str,
    ) -> String {
        self.symbol_store
            .unique_es_symbol_type_name(symbol, symbol_name)
    }

    pub fn symbol_flags(&self, symbol: ast::SymbolHandle) -> ast::SymbolFlags {
        self.symbol_store.flags(symbol)
    }

    #[inline]
    pub fn symbol_flags_for_owned_handle(&self, symbol: ast::SymbolHandle) -> ast::SymbolFlags {
        self.symbol_store.flags_for_owned_handle(symbol)
    }

    pub fn symbol_check_flags(&self, symbol: ast::SymbolHandle) -> ast::CheckFlags {
        self.symbol_store.check_flags(symbol)
    }

    #[inline]
    pub fn symbol_check_flags_for_owned_handle(
        &self,
        symbol: ast::SymbolHandle,
    ) -> ast::CheckFlags {
        self.symbol_store.check_flags_for_owned_handle(symbol)
    }

    pub fn symbol_name(&self, symbol: ast::SymbolHandle) -> &ast::SymbolName {
        self.symbol_store.name(symbol)
    }

    #[inline]
    pub fn symbol_name_for_owned_handle(&self, symbol: ast::SymbolHandle) -> &ast::SymbolName {
        self.symbol_store.name_for_owned_handle(symbol)
    }

    pub fn with_symbol_declarations<R>(
        &self,
        symbol: ast::SymbolHandle,
        f: impl FnOnce(&[ast::Node]) -> R,
    ) -> R {
        self.symbol_store.with_declarations(symbol, f)
    }

    #[inline]
    pub fn with_symbol_declarations_for_owned_handle<R>(
        &self,
        symbol: ast::SymbolHandle,
        f: impl FnOnce(&[ast::Node]) -> R,
    ) -> R {
        self.symbol_store
            .with_declarations_for_owned_handle(symbol, f)
    }

    #[inline]
    pub fn share_symbol_declarations_for_owned_handle(
        &self,
        symbol: ast::SymbolHandle,
    ) -> ast::SymbolDeclarations {
        self.symbol_store
            .share_declarations_for_owned_handle(symbol)
    }

    #[inline]
    pub fn first_symbol_declaration_for_owned_handle(
        &self,
        symbol: ast::SymbolHandle,
    ) -> Option<ast::Node> {
        self.symbol_store.first_declaration_for_owned_handle(symbol)
    }

    pub fn symbol_value_declaration(&self, symbol: ast::SymbolHandle) -> Option<ast::Node> {
        self.symbol_store.value_declaration(symbol)
    }

    #[inline]
    pub fn symbol_value_declaration_for_owned_handle(
        &self,
        symbol: ast::SymbolHandle,
    ) -> Option<ast::Node> {
        self.symbol_store.value_declaration_for_owned_handle(symbol)
    }

    #[inline]
    pub fn symbol_value_declaration_snapshot_for_owned_handle(
        &self,
        symbol: ast::SymbolHandle,
    ) -> ast::SymbolValueDeclarationSnapshot {
        self.symbol_store
            .value_declaration_snapshot_for_owned_handle(symbol)
    }

    pub fn symbol_parent(&self, symbol: ast::SymbolHandle) -> Option<ast::SymbolHandle> {
        self.symbol_store.parent(symbol)
    }

    #[inline]
    pub fn symbol_parent_for_owned_handle(
        &self,
        symbol: ast::SymbolHandle,
    ) -> Option<ast::SymbolHandle> {
        self.symbol_store.parent_for_owned_handle(symbol)
    }

    #[inline]
    pub fn symbol_instantiation_header_for_owned_handle(
        &self,
        symbol: ast::SymbolHandle,
    ) -> ast::SymbolInstantiationHeader {
        self.symbol_store
            .instantiation_header_for_owned_handle(symbol)
    }

    #[inline]
    pub fn symbol_instantiation_snapshot_for_owned_handle(
        &self,
        symbol: ast::SymbolHandle,
    ) -> ast::SymbolInstantiationSnapshot {
        self.symbol_store
            .instantiation_snapshot_for_owned_handle(symbol)
    }

    pub fn symbol_export_symbol(&self, symbol: ast::SymbolHandle) -> Option<ast::SymbolHandle> {
        self.symbol_store.export_symbol(symbol)
    }

    #[inline]
    pub fn symbol_export_symbol_for_owned_handle(
        &self,
        symbol: ast::SymbolHandle,
    ) -> Option<ast::SymbolHandle> {
        self.symbol_store.export_symbol_for_owned_handle(symbol)
    }

    pub fn with_symbol_members<R>(
        &self,
        symbol: ast::SymbolHandle,
        f: impl FnOnce(Option<&ast::SymbolHandleTable>) -> R,
    ) -> R {
        self.symbol_store.with_members(symbol, f)
    }

    #[inline]
    pub fn with_symbol_members_for_owned_handle<R>(
        &self,
        symbol: ast::SymbolHandle,
        f: impl FnOnce(Option<&ast::SymbolHandleTable>) -> R,
    ) -> R {
        self.symbol_store.with_members_for_owned_handle(symbol, f)
    }

    pub fn with_symbol_exports<R>(
        &self,
        symbol: ast::SymbolHandle,
        f: impl FnOnce(Option<&ast::SymbolHandleTable>) -> R,
    ) -> R {
        self.symbol_store.with_exports(symbol, f)
    }

    #[inline]
    pub fn with_symbol_exports_for_owned_handle<R>(
        &self,
        symbol: ast::SymbolHandle,
        f: impl FnOnce(Option<&ast::SymbolHandleTable>) -> R,
    ) -> R {
        self.symbol_store.with_exports_for_owned_handle(symbol, f)
    }

    pub fn lookup_symbol_member(
        &self,
        symbol: ast::SymbolHandle,
        name: &str,
    ) -> Option<ast::SymbolHandle> {
        self.symbol_store.lookup_member(symbol, name)
    }

    pub fn lookup_symbol_export(
        &self,
        symbol: ast::SymbolHandle,
        name: &str,
    ) -> Option<ast::SymbolHandle> {
        self.symbol_store.lookup_export(symbol, name)
    }

    pub fn symbol_declarations_are_empty(&self, symbol: ast::SymbolHandle) -> bool {
        self.with_symbol_declarations(symbol, |declarations| declarations.is_empty())
    }

    pub fn symbol_count(&self) -> i32 {
        self.symbol_count
    }

    pub fn classifiable_names(&self) -> &collections::Set<String> {
        &self.classifiable_names
    }

    fn global_exports(&self) -> &ast::SymbolHandleTable {
        &self.global_exports
    }

    fn js_global_augmentations(&self) -> &ast::SymbolHandleTable {
        &self.js_global_augmentations
    }

    pub fn with_global_exports<R>(&self, f: impl FnOnce(&ast::SymbolHandleTable) -> R) -> R {
        f(self.global_exports())
    }

    pub fn global_exports_len(&self) -> usize {
        self.global_exports().len()
    }

    pub fn global_exports_is_empty(&self) -> bool {
        self.global_exports().is_empty()
    }

    pub fn global_export_at(&self, index: usize) -> Option<(ast::SymbolName, ast::SymbolHandle)> {
        self.global_exports()
            .get_index(index)
            .map(|(name, &symbol)| (name.clone(), symbol))
    }

    pub fn js_global_augmentations_len(&self) -> usize {
        self.js_global_augmentations().len()
    }

    pub fn js_global_augmentation_at(
        &self,
        index: usize,
    ) -> Option<(ast::SymbolName, ast::SymbolHandle)> {
        self.js_global_augmentations()
            .get_index(index)
            .map(|(name, &symbol)| (name.clone(), symbol))
    }

    pub fn pattern_ambient_modules(&self) -> &[ast::PatternAmbientModule] {
        &self.pattern_ambient_modules
    }

    pub fn source_symbol(&self) -> Option<ast::SymbolHandle> {
        self.symbol
    }

    pub fn end_flow_node(&self) -> Option<ast::FlowRef> {
        self.end_flow_node
    }

    pub fn flow_graph(&self) -> &ast::FlowGraph {
        &self.flow_graph
    }

    pub fn common_js_module_indicator(&self) -> Option<ast::Node> {
        self.common_js_module_indicator
    }

    pub fn external_module_indicator(&self) -> Option<ast::Node> {
        self.external_module_indicator
    }

    pub fn nested_cjs_exports(&self) -> &[ast::Node] {
        &self.nested_cjs_exports
    }

    pub fn bind_suggestion_diagnostics(&self) -> &[ast::Diagnostic] {
        &self.bind_suggestion_diagnostics
    }

    pub fn bind_diagnostics(&self) -> &[ast::Diagnostic] {
        &self.bind_diagnostics
    }

    pub fn flags(&self) -> ast::NodeFlags {
        self.flags
    }

    pub fn flags_for_node(&self, node: ast::Node, mut flags: ast::NodeFlags) -> ast::NodeFlags {
        if let Some(update) = self.binder_flag_update(node) {
            flags = update.apply(flags);
        }
        flags
    }

    pub(crate) fn binder_flag_update(&self, node: ast::Node) -> Option<BinderFlagUpdate> {
        self.binder_flags_by_node.get_copied_same_store(node)
    }

    pub(crate) fn add_flags(&mut self, node: ast::Node, flags: ast::NodeFlags) {
        let update = self
            .binder_flags_by_node
            .get_or_insert_with_same_store(node, Default::default);
        update.removed &= !flags;
        update.added |= flags;
    }

    pub(crate) fn remove_flags(&mut self, node: ast::Node, flags: ast::NodeFlags) {
        let update = self
            .binder_flags_by_node
            .get_or_insert_with_same_store(node, Default::default);
        update.added &= !flags;
        update.removed |= flags;
    }

    pub(crate) fn take_locals(&mut self, node: ast::Node) -> Option<ast::SymbolHandleTable> {
        self.locals_by_container.remove_same_store(node)
    }

    pub(crate) fn insert_locals(&mut self, node: ast::Node, locals: ast::SymbolHandleTable) {
        self.locals_by_container.insert_same_store(node, locals);
    }

    pub(crate) fn locals_mut(&mut self, node: ast::Node) -> &mut ast::SymbolHandleTable {
        self.locals_by_container
            .get_or_insert_with_same_store(node, Default::default)
    }

    pub(crate) fn get_or_insert_local_symbol(
        &mut self,
        node: ast::Node,
        name: ast::SymbolName,
        initial_flags: ast::SymbolFlags,
    ) -> (ast::SymbolHandle, bool) {
        let symbol_name = name.clone();
        let locals = self
            .locals_by_container
            .get_or_insert_with_same_store(node, Default::default);
        match locals.entry(name) {
            indexmap::map::Entry::Occupied(entry) => (*entry.get(), false),
            indexmap::map::Entry::Vacant(entry) => {
                let symbol = self
                    .symbol_store
                    .create_binding_symbol(initial_flags, symbol_name);
                entry.insert(symbol);
                (symbol, true)
            }
        }
    }

    pub(crate) fn record_declaration_symbol(&mut self, node: ast::Node, symbol: ast::SymbolHandle) {
        self.declaration_symbols_by_node
            .insert_same_store(node, symbol);
    }

    pub(crate) fn record_exportable_local_symbol(
        &mut self,
        node: ast::Node,
        symbol: ast::SymbolHandle,
    ) {
        self.exportable_local_symbols_by_node
            .insert_same_store(node, symbol);
    }

    pub(crate) fn set_flow_node(&mut self, node: ast::Node, flow_node: Option<ast::FlowRef>) {
        Self::set_optional_flow_ref(&mut self.flow_nodes_by_node, node, flow_node);
    }

    pub(crate) fn set_return_flow_node(
        &mut self,
        node: ast::Node,
        flow_node: Option<ast::FlowRef>,
    ) {
        Self::set_optional_flow_ref(&mut self.return_flow_nodes_by_node, node, flow_node);
    }

    pub(crate) fn set_body_end_flow_node(
        &mut self,
        node: ast::Node,
        flow_node: Option<ast::FlowRef>,
    ) {
        Self::set_optional_flow_ref(&mut self.body_end_flow_nodes_by_node, node, flow_node);
    }

    pub(crate) fn set_fallthrough_flow_node(
        &mut self,
        node: ast::Node,
        flow_node: Option<ast::FlowRef>,
    ) {
        Self::set_optional_flow_ref(&mut self.fallthrough_flow_nodes_by_node, node, flow_node);
    }

    fn set_optional_flow_ref(
        flows: &mut ast::StoreNodeMap<ast::FlowRef>,
        node: ast::Node,
        flow_node: Option<ast::FlowRef>,
    ) {
        if let Some(flow_node) = flow_node {
            flows.insert_same_store(node, flow_node);
        } else {
            flows.remove_same_store(node);
        }
    }

    pub(crate) fn set_next_container(&mut self, current: ast::Node, next: ast::Node) {
        self.next_container_by_node.insert_same_store(current, next);
    }

    pub(crate) fn create_symbol(
        &mut self,
        flags: ast::SymbolFlags,
        name: impl Into<ast::SymbolName>,
    ) -> ast::SymbolHandle {
        self.symbol_store.create_binding_symbol(flags, name)
    }

    pub(crate) fn add_symbol_flags(&mut self, symbol: ast::SymbolHandle, flags: ast::SymbolFlags) {
        self.symbol_store.add_binding_flags(symbol, flags);
    }

    pub(crate) fn remove_symbol_flags(
        &mut self,
        symbol: ast::SymbolHandle,
        flags: ast::SymbolFlags,
    ) {
        self.symbol_store.remove_binding_flags(symbol, flags);
    }

    pub(crate) fn set_symbol_declarations(
        &mut self,
        symbol: ast::SymbolHandle,
        declarations: Vec<ast::Node>,
    ) {
        self.symbol_store
            .set_binding_declarations(symbol, declarations);
    }

    pub(crate) fn add_symbol_declaration(
        &mut self,
        symbol: ast::SymbolHandle,
        declaration: ast::Node,
    ) {
        self.symbol_store
            .add_binding_declaration(symbol, declaration);
    }

    pub(crate) fn add_symbol_declaration_if_unique(
        &mut self,
        symbol: ast::SymbolHandle,
        declaration: ast::Node,
    ) {
        self.symbol_store
            .add_binding_declaration_if_unique(symbol, declaration);
    }

    pub(crate) fn set_symbol_value_declaration(
        &mut self,
        symbol: ast::SymbolHandle,
        value_declaration: Option<ast::Node>,
    ) {
        self.symbol_store
            .set_binding_value_declaration(symbol, value_declaration);
    }

    pub(crate) fn set_symbol_parent(
        &mut self,
        symbol: ast::SymbolHandle,
        parent: Option<ast::SymbolHandle>,
    ) {
        self.symbol_store.set_binding_parent(symbol, parent);
    }

    pub(crate) fn set_symbol_export_symbol(
        &mut self,
        symbol: ast::SymbolHandle,
        export_symbol: Option<ast::SymbolHandle>,
    ) {
        self.symbol_store
            .set_binding_export_symbol(symbol, export_symbol);
    }

    pub(crate) fn ensure_symbol_members(&mut self, symbol: ast::SymbolHandle) {
        self.symbol_store.ensure_binding_members(symbol);
    }

    pub(crate) fn ensure_symbol_exports(&mut self, symbol: ast::SymbolHandle) {
        self.symbol_store.ensure_binding_exports(symbol);
    }

    pub(crate) fn insert_symbol_export(
        &mut self,
        symbol: ast::SymbolHandle,
        name: impl Into<ast::SymbolName>,
        export: ast::SymbolHandle,
    ) -> Option<ast::SymbolHandle> {
        self.symbol_store.insert_export(symbol, name, export)
    }

    pub(crate) fn insert_symbol_member(
        &mut self,
        symbol: ast::SymbolHandle,
        name: impl Into<ast::SymbolName>,
        member: ast::SymbolHandle,
    ) -> Option<ast::SymbolHandle> {
        self.symbol_store.insert_member(symbol, name, member)
    }

    pub(crate) fn finish_file_binding(&mut self, file_binding: FinishedFileBinding) {
        self.symbol_count = file_binding.symbol_count;
        self.classifiable_names = file_binding.classifiable_names;
        self.global_exports = file_binding.global_exports;
        self.js_global_augmentations = file_binding.js_global_augmentations;
        self.pattern_ambient_modules = file_binding.pattern_ambient_modules;
        self.symbol = file_binding.symbol;
        self.end_flow_node = file_binding.end_flow_node;
        self.flow_graph = Arc::new(file_binding.flow_graph);
        self.common_js_module_indicator = file_binding.common_js_module_indicator;
        self.external_module_indicator = file_binding.external_module_indicator;
        self.nested_cjs_exports = file_binding.nested_cjs_exports;
        self.bind_suggestion_diagnostics = file_binding.bind_suggestion_diagnostics;
        self.bind_diagnostics = file_binding.bind_diagnostics;
        self.flags = file_binding.flags;
    }

    fn locals(&self, node: ast::Node) -> Option<&ast::SymbolHandleTable> {
        self.locals_by_container.get_same_store(node)
    }

    pub fn with_locals<R>(
        &self,
        node: ast::Node,
        f: impl FnOnce(Option<&ast::SymbolHandleTable>) -> R,
    ) -> R {
        f(self.locals(node))
    }

    pub fn has_locals(&self, node: ast::Node) -> bool {
        self.locals(node).is_some()
    }

    pub fn locals_len(&self, node: ast::Node) -> usize {
        self.locals(node).map_or(0, ast::SymbolHandleTable::len)
    }

    pub fn local_at(&self, node: ast::Node, index: usize) -> Option<ast::SymbolHandle> {
        self.locals(node)
            .and_then(|locals| locals.get_index(index).map(|(_, &symbol)| symbol))
    }

    pub fn local_entry_at(
        &self,
        node: ast::Node,
        index: usize,
    ) -> Option<(ast::SymbolName, ast::SymbolHandle)> {
        self.locals(node).and_then(|locals| {
            locals
                .get_index(index)
                .map(|(name, &symbol)| (name.clone(), symbol))
        })
    }

    pub fn lookup_local(&self, node: ast::Node, name: &str) -> Option<ast::SymbolHandle> {
        self.locals(node)
            .and_then(|locals| locals.get(name).copied())
    }

    pub fn next_container(&self, node: ast::Node) -> Option<ast::Node> {
        self.next_container_by_node.get_copied_same_store(node)
    }

    pub fn symbol(&self, node: ast::Node) -> Option<ast::SymbolHandle> {
        self.declaration_symbols_by_node.get_copied_same_store(node)
    }

    pub fn exportable_local_symbol(&self, node: ast::Node) -> Option<ast::SymbolHandle> {
        self.exportable_local_symbols_by_node
            .get_copied_same_store(node)
    }

    pub fn flow_node(&self, node: ast::Node) -> Option<ast::FlowRef> {
        self.flow_nodes_by_node.get_copied_same_store(node)
    }

    pub fn return_flow_node(&self, node: ast::Node) -> Option<ast::FlowRef> {
        self.return_flow_nodes_by_node.get_copied_same_store(node)
    }

    pub fn body_end_flow_node(&self, node: ast::Node) -> Option<ast::FlowRef> {
        self.body_end_flow_nodes_by_node.get_copied_same_store(node)
    }

    pub fn fallthrough_flow_node(&self, node: ast::Node) -> Option<ast::FlowRef> {
        self.fallthrough_flow_nodes_by_node
            .get_copied_same_store(node)
    }
}

impl BinderFlagUpdate {
    pub(crate) fn apply(self, mut flags: ast::NodeFlags) -> ast::NodeFlags {
        flags |= self.added;
        flags &= !self.removed;
        flags
    }
}

pub(crate) struct FinishedFileBinding {
    symbol_count: i32,
    classifiable_names: collections::Set<String>,
    global_exports: ast::SymbolHandleTable,
    js_global_augmentations: ast::SymbolHandleTable,
    pattern_ambient_modules: Vec<ast::PatternAmbientModule>,
    symbol: Option<ast::SymbolHandle>,
    end_flow_node: Option<ast::FlowRef>,
    flow_graph: ast::FlowGraph,
    common_js_module_indicator: Option<ast::Node>,
    external_module_indicator: Option<ast::Node>,
    nested_cjs_exports: Vec<ast::Node>,
    bind_suggestion_diagnostics: Vec<ast::Diagnostic>,
    bind_diagnostics: Vec<ast::Diagnostic>,
    flags: ast::NodeFlags,
}

impl FinishedFileBinding {
    pub(crate) fn new(
        symbol_count: i32,
        classifiable_names: collections::Set<String>,
        global_exports: ast::SymbolHandleTable,
        js_global_augmentations: ast::SymbolHandleTable,
        pattern_ambient_modules: Vec<ast::PatternAmbientModule>,
        symbol: Option<ast::SymbolHandle>,
        end_flow_node: Option<ast::FlowRef>,
        flow_graph: ast::FlowGraph,
        common_js_module_indicator: Option<ast::Node>,
        external_module_indicator: Option<ast::Node>,
        nested_cjs_exports: Vec<ast::Node>,
        bind_suggestion_diagnostics: Vec<ast::Diagnostic>,
        bind_diagnostics: Vec<ast::Diagnostic>,
        flags: ast::NodeFlags,
    ) -> Self {
        Self {
            symbol_count,
            classifiable_names,
            global_exports,
            js_global_augmentations,
            pattern_ambient_modules,
            symbol,
            end_flow_node,
            flow_graph,
            common_js_module_indicator,
            external_module_indicator,
            nested_cjs_exports,
            bind_suggestion_diagnostics,
            bind_diagnostics,
            flags,
        }
    }
}
