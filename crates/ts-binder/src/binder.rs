use std::{marker::PhantomData, sync::Arc};
use ts_ast as ast;
use ts_ast::SymbolFlagsExt;
use ts_collections as collections;
use ts_core as core;
use ts_debug as debug;
use ts_diagnostics as diagnostics;
use ts_scanner as scanner;
use ts_tspath as tspath;

use crate::ProgramBindingState;
use crate::program_binding_state::FinishedFileBinding;

pub type ContainerFlags = i32;

// The current node is not a container, and no container manipulation should happen before
// recursing into it.
pub const CONTAINER_FLAGS_NONE: ContainerFlags = 0;
// The current node is a container.  It should be set as the current container (and block-
// container) before recursing into it.  The current node does not have locals.  Examples:
//
//      Classes, ObjectLiterals, TypeLiterals, Interfaces...
pub const CONTAINER_FLAGS_IS_CONTAINER: ContainerFlags = 1 << 0;
// The current node is a block-scoped-container.  It should be set as the current block-
// container before recursing into it.  Examples:
//
//      Blocks (when not parented by functions), Catch clauses, For/For-in/For-of statements...
pub const CONTAINER_FLAGS_IS_BLOCK_SCOPED_CONTAINER: ContainerFlags = 1 << 1;
// The current node is the container of a control flow path. The current control flow should
// be saved and restored, and a new control flow initialized within the container.
pub const CONTAINER_FLAGS_IS_CONTROL_FLOW_CONTAINER: ContainerFlags = 1 << 2;
pub const CONTAINER_FLAGS_IS_FUNCTION_LIKE: ContainerFlags = 1 << 3;
pub const CONTAINER_FLAGS_IS_FUNCTION_EXPRESSION: ContainerFlags = 1 << 4;
pub const CONTAINER_FLAGS_HAS_LOCALS: ContainerFlags = 1 << 5;
pub const CONTAINER_FLAGS_IS_INTERFACE: ContainerFlags = 1 << 6;
pub const CONTAINER_FLAGS_IS_OBJECT_LITERAL_OR_CLASS_EXPRESSION_METHOD_OR_ACCESSOR: ContainerFlags =
    1 << 7;
pub const CONTAINER_FLAGS_IS_THIS_CONTAINER: ContainerFlags = 1 << 8;
pub const CONTAINER_FLAGS_PROPAGATES_THIS_KEYWORD: ContainerFlags = 1 << 9;

#[derive(Clone)]
pub struct ExpandoAssignmentInfo {
    pub node: ast::Node,
    pub container_state: BinderContainer,
    pub block_scope_container_state: BinderContainer,
}

#[derive(Clone)]
pub struct BinderContainer {
    pub kind: ast::Kind,
    pub flags: ast::NodeFlags,
    pub symbol: Option<ast::SymbolHandle>,
    pub parent_symbol: Option<ast::SymbolHandle>,
    pub locals_key: ast::Node,
    pub is_source_file: bool,
    pub is_locals_container: bool,
    pub is_static: bool,
}

pub struct Binder<'a> {
    pub store: &'a ast::AstStore,
    pub file: ast::Node,
    pub file_name: String,
    pub file_is_external_or_common_js_module: bool,
    pub file_is_json_source_file: bool,
    pub file_is_declaration_file: bool,
    pub file_has_common_js_module_indicator: bool,
    pub file_has_external_module_indicator: bool,
    pub file_common_js_module_indicator: Option<ast::Node>,
    pub file_external_module_indicator: Option<ast::Node>,
    pub file_diagnostics_empty: bool,
    pub file_symbol: Option<ast::SymbolHandle>,
    pub file_global_exports: ast::SymbolHandleTable,
    pub file_js_global_augmentations: ast::SymbolHandleTable,
    pub file_pattern_ambient_modules: Vec<ast::PatternAmbientModule>,
    pub file_bind_diagnostics: Vec<ast::Diagnostic>,
    pub file_bind_suggestion_diagnostics: Vec<ast::Diagnostic>,
    pub nested_cjs_exports: Vec<ast::Node>,
    pub file_flags: ast::NodeFlags,
    pub file_end_flow_node: Option<ast::FlowRef>,
    pub unreachable_flow: Option<ast::FlowRef>,

    pub container: Option<ast::Node>,
    pub this_container: Option<ast::Node>,
    pub block_scope_container: Option<ast::Node>,
    pub container_state: Option<BinderContainer>,
    pub this_container_state: Option<BinderContainer>,
    pub block_scope_container_state: Option<BinderContainer>,
    pub last_container: Option<ast::Node>,
    pub current_flow: Option<ast::FlowRef>,
    pub current_break_target: Option<ast::FlowRef>,
    pub current_continue_target: Option<ast::FlowRef>,
    pub current_return_target: Option<ast::FlowRef>,
    pub current_true_target: Option<ast::FlowRef>,
    pub current_false_target: Option<ast::FlowRef>,
    pub current_exception_target: Option<ast::FlowRef>,
    pub pre_switch_case_flow: Option<ast::FlowRef>,
    pub active_label_list: Option<Box<ActiveLabel<'a>>>,
    pub emit_flags: ast::NodeFlags,
    pub seen_this_keyword: bool,
    pub has_explicit_return: bool,
    pub has_flow_effects: bool,
    pub in_assignment_pattern: bool,
    pub seen_parse_error: bool,
    pub symbol_count: i32,
    pub classifiable_names: collections::Set<String>,
    pub not_const_enum_only_modules: collections::Set<ast::SymbolHandle>,
    pub flow_graph: ast::FlowGraph,
    pub binding_state: ProgramBindingState,
    pub expando_assignments: Vec<ExpandoAssignmentInfo>,
    _marker: PhantomData<&'a mut ()>,
}

pub struct ActiveLabel<'a> {
    pub next: Option<Box<ActiveLabel<'a>>>,
    pub break_target: Option<ast::FlowRef>,
    pub continue_target: Option<ast::FlowRef>,
    pub name: String,
    pub referenced: bool,
    _marker: PhantomData<&'a mut ()>,
}

struct SourceFileBindingState {
    file_name: String,
    is_external_or_common_js_module: bool,
    is_json_source_file: bool,
    is_declaration_file: bool,
    has_common_js_module_indicator: bool,
    has_external_module_indicator: bool,
    common_js_module_indicator: Option<ast::Node>,
    external_module_indicator: Option<ast::Node>,
    diagnostics_empty: bool,
    symbol: Option<ast::SymbolHandle>,
    global_exports: ast::SymbolHandleTable,
    js_global_augmentations: ast::SymbolHandleTable,
    pattern_ambient_modules: Vec<ast::PatternAmbientModule>,
    bind_diagnostics: Vec<ast::Diagnostic>,
    bind_suggestion_diagnostics: Vec<ast::Diagnostic>,
    flags: ast::NodeFlags,
}

enum BindingSymbolTable<'table> {
    Borrowed(&'table mut ast::SymbolHandleTable),
    Locals(ast::Node),
    SymbolExports(ast::SymbolHandle),
    SymbolMembers(ast::SymbolHandle),
}

impl<'a> ActiveLabel<'a> {
    pub fn break_target(&self) -> Option<ast::FlowRef> {
        self.break_target
    }

    pub fn continue_target(&self) -> Option<ast::FlowRef> {
        self.continue_target
    }
}

pub fn bind_parsed_source_file(file: &ast::ParsedSourceFile) -> Arc<ProgramBindingState> {
    bind_source_file_cached(file.data(), file.store(), file.root())
}

pub fn bind_source_file(file: &ast::SourceFile) -> Arc<ProgramBindingState> {
    bind_source_file_cached(file.data(), file.store(), file.root())
}

pub fn bind_source_file_view(file: &ast::SourceFileView<'_>) -> ProgramBindingState {
    bind_source_file_readonly(file.store(), file.root())
}

fn bind_source_file_cached(
    data: &ast::SourceFileData,
    store: &ast::AstStore,
    root: ast::Node,
) -> Arc<ProgramBindingState> {
    data.get_or_init_bind_once_state(|| bind_source_file_readonly(store, root))
}

fn get_binder<'a>(file: ast::Node, store: &'a ast::AstStore) -> Binder<'a> {
    Binder::new(file, store)
}

fn bind_source_file_readonly(store: &ast::AstStore, root: ast::Node) -> ProgramBindingState {
    let binding_state = read_source_file_binding_state(store, root);
    let mut b = get_binder(root, store);
    b.file_name = binding_state.file_name;
    b.file_is_external_or_common_js_module = binding_state.is_external_or_common_js_module;
    b.file_is_json_source_file = binding_state.is_json_source_file;
    b.file_is_declaration_file = binding_state.is_declaration_file;
    b.file_has_common_js_module_indicator = binding_state.has_common_js_module_indicator;
    b.file_has_external_module_indicator = binding_state.has_external_module_indicator;
    b.file_common_js_module_indicator = binding_state.common_js_module_indicator;
    b.file_external_module_indicator = binding_state.external_module_indicator;
    b.file_diagnostics_empty = binding_state.diagnostics_empty;
    b.file_flags = binding_state.flags;
    b.file_symbol = binding_state.symbol;
    b.file_global_exports = binding_state.global_exports;
    b.file_js_global_augmentations = binding_state.js_global_augmentations;
    b.file_pattern_ambient_modules = binding_state.pattern_ambient_modules;
    b.file_bind_diagnostics = binding_state.bind_diagnostics;
    b.file_bind_suggestion_diagnostics = binding_state.bind_suggestion_diagnostics;
    b.unreachable_flow = Some(b.new_flow_node(ast::FlowFlags::Unreachable));
    let mut file_node = b.file;
    b.bind(Some(&mut file_node));
    b.bind_deferred_expando_assignments();
    b.take_program_binding_state()
}

fn read_source_file_binding_state(
    store: &ast::AstStore,
    root: ast::Node,
) -> SourceFileBindingState {
    let data = store.as_source_file(root);
    SourceFileBindingState {
        file_name: data.file_name(),
        is_external_or_common_js_module: data.external_module_indicator().is_some()
            || data.common_js_module_indicator().is_some(),
        is_json_source_file: data.script_kind() == core::ScriptKind::JSON,
        is_declaration_file: data.is_declaration_file(),
        has_common_js_module_indicator: data.common_js_module_indicator().is_some(),
        has_external_module_indicator: data.external_module_indicator().is_some(),
        common_js_module_indicator: data.common_js_module_indicator(),
        external_module_indicator: data.external_module_indicator(),
        diagnostics_empty: data.diagnostics().is_empty(),
        symbol: None,
        global_exports: ast::SymbolHandleTable::default(),
        js_global_augmentations: ast::SymbolHandleTable::default(),
        pattern_ambient_modules: Vec::new(),
        bind_diagnostics: Vec::new(),
        bind_suggestion_diagnostics: Vec::new(),
        flags: store.flags(root),
    }
}

impl<'a> Binder<'a> {
    fn new(file: ast::Node, store: &'a ast::AstStore) -> Self {
        Self {
            file_name: String::new(),
            file_is_external_or_common_js_module: false,
            file_is_json_source_file: false,
            file_is_declaration_file: false,
            file_has_common_js_module_indicator: false,
            file_has_external_module_indicator: false,
            file_common_js_module_indicator: None,
            file_external_module_indicator: None,
            file_diagnostics_empty: true,
            file_symbol: None,
            file_global_exports: ast::SymbolHandleTable::default(),
            file_js_global_augmentations: ast::SymbolHandleTable::default(),
            file_pattern_ambient_modules: Vec::new(),
            file_bind_diagnostics: Vec::new(),
            file_bind_suggestion_diagnostics: Vec::new(),
            nested_cjs_exports: Vec::new(),
            file_flags: ast::NodeFlags::empty(),
            file_end_flow_node: None,
            unreachable_flow: None,
            container: None,
            this_container: None,
            block_scope_container: None,
            container_state: None,
            this_container_state: None,
            block_scope_container_state: None,
            last_container: None,
            current_flow: None,
            current_break_target: None,
            current_continue_target: None,
            current_return_target: None,
            current_true_target: None,
            current_false_target: None,
            current_exception_target: None,
            pre_switch_case_flow: None,
            active_label_list: None,
            emit_flags: ast::NodeFlags::empty(),
            seen_this_keyword: false,
            has_explicit_return: false,
            has_flow_effects: false,
            in_assignment_pattern: false,
            seen_parse_error: false,
            symbol_count: 0,
            classifiable_names: collections::Set::new(),
            not_const_enum_only_modules: collections::Set::new(),
            flow_graph: ast::FlowGraph::default(),
            binding_state: ProgramBindingState::new(file, store),
            expando_assignments: Vec::new(),
            _marker: PhantomData,
            store,
            file,
        }
    }

    fn source_file_view(&self) -> ast::SourceFileView<'_> {
        self.store.source_file_view(self.file)
    }

    fn declaration_name_to_string(&self, name: &ast::Node) -> String {
        scanner::declaration_name_to_string(&self.source_file_view(), Some(name))
    }

    fn flags(&self, node: ast::Node) -> ast::NodeFlags {
        self.binding_state
            .flags_for_node(node, self.store.flags(node))
    }

    fn add_flags(&mut self, node: ast::Node, flags: ast::NodeFlags) {
        self.binding_state.add_flags(node, flags);
        if node == self.file {
            self.file_flags = self.flags(self.file);
        }
    }

    fn remove_flags(&mut self, node: ast::Node, flags: ast::NodeFlags) {
        self.binding_state.remove_flags(node, flags);
        if node == self.file {
            self.file_flags = self.flags(self.file);
        }
    }

    fn set_export_context(&mut self, node: ast::Node, enabled: bool) {
        if enabled {
            self.add_flags(node, ast::NodeFlags::ExportContext);
        } else {
            self.remove_flags(node, ast::NodeFlags::ExportContext);
        }
    }

    fn reset_reachability_flags(&mut self, node: ast::Node) {
        self.remove_flags(
            node,
            ast::NodeFlags::ReachabilityAndEmitFlags | ast::NodeFlags::ContainsThis,
        );
    }

    fn mark_subtree_has_error(&mut self, node: ast::Node) {
        self.add_flags(node, ast::NodeFlags::ThisNodeOrAnySubNodesHasError);
    }

    fn mark_implicit_return(&mut self, node: ast::Node) {
        self.add_flags(node, ast::NodeFlags::HasImplicitReturn);
    }

    fn mark_explicit_return(&mut self, node: ast::Node) {
        self.add_flags(node, ast::NodeFlags::HasExplicitReturn);
    }

    fn mark_contains_this(&mut self, node: ast::Node) {
        self.add_flags(node, ast::NodeFlags::ContainsThis);
    }

    fn set_contains_this(&mut self, node: ast::Node, enabled: bool) {
        if enabled {
            self.mark_contains_this(node);
        } else {
            self.remove_flags(node, ast::NodeFlags::ContainsThis);
        }
    }

    fn mark_unreachable(&mut self, node: ast::Node) {
        self.add_flags(node, ast::NodeFlags::Unreachable);
    }

    fn symbol(&self, node: ast::Node) -> Option<ast::SymbolHandle> {
        if node == self.file {
            return self.file_symbol;
        }
        self.binding_state.symbol(node)
    }

    fn record_declaration_symbol(&mut self, node: ast::Node, symbol: ast::SymbolHandle) {
        self.binding_state.record_declaration_symbol(node, symbol);
    }

    fn record_exportable_local_symbol(&mut self, node: ast::Node, symbol: ast::SymbolHandle) {
        self.binding_state
            .record_exportable_local_symbol(node, symbol);
    }

    fn symbol_flags(&self, symbol: ast::SymbolHandle) -> ast::SymbolFlags {
        self.binding_state.symbol_flags(symbol)
    }

    fn symbol_name(&self, symbol: ast::SymbolHandle) -> String {
        self.binding_state.symbol_name(symbol).to_string()
    }

    fn symbol_display_name(&self, symbol: ast::SymbolHandle) -> String {
        if let Some(value_declaration) = self.symbol_value_declaration(symbol)
            && ast::is_private_identifier_class_element_declaration(self.store, value_declaration)
        {
            let name = self
                .store
                .name(value_declaration)
                .expect("private identifier class element should have a name");
            return self.store.text(name);
        }
        self.symbol_name(symbol)
    }

    fn symbol_value_declaration(&self, symbol: ast::SymbolHandle) -> Option<ast::Node> {
        self.binding_state.symbol_value_declaration(symbol)
    }

    fn with_symbol_declarations<R>(
        &self,
        symbol: ast::SymbolHandle,
        f: impl FnOnce(&[ast::Node]) -> R,
    ) -> R {
        self.binding_state.with_symbol_declarations(symbol, f)
    }

    fn collect_symbol_declarations(&self, symbol: ast::SymbolHandle) -> Vec<ast::Node> {
        self.with_symbol_declarations(symbol, |declarations| declarations.to_vec())
    }

    fn first_symbol_declaration(&self, symbol: ast::SymbolHandle) -> Option<ast::Node> {
        self.with_symbol_declarations(symbol, |declarations| declarations.first().copied())
    }

    fn symbol_declarations_are_empty(&self, symbol: ast::SymbolHandle) -> bool {
        self.with_symbol_declarations(symbol, |declarations| declarations.is_empty())
    }

    fn symbol_export(&self, symbol: ast::SymbolHandle, name: &str) -> Option<ast::SymbolHandle> {
        self.binding_state.lookup_symbol_export(symbol, name)
    }

    fn symbol_member(&self, symbol: ast::SymbolHandle, name: &str) -> Option<ast::SymbolHandle> {
        self.binding_state.lookup_symbol_member(symbol, name)
    }

    fn symbol_exports_is_empty(&self, symbol: ast::SymbolHandle) -> bool {
        self.binding_state.with_symbol_exports(symbol, |exports| {
            exports.is_none_or(|exports| exports.is_empty())
        })
    }

    fn add_symbol_flags(&mut self, symbol: ast::SymbolHandle, flags: ast::SymbolFlags) {
        self.binding_state.add_symbol_flags(symbol, flags);
    }

    fn remove_symbol_flags(&mut self, symbol: ast::SymbolHandle, flags: ast::SymbolFlags) {
        self.binding_state.remove_symbol_flags(symbol, flags);
    }

    fn add_symbol_declaration(&mut self, symbol: ast::SymbolHandle, declaration: ast::Node) {
        self.binding_state
            .add_symbol_declaration(symbol, declaration);
    }

    fn add_symbol_declaration_if_unique(
        &mut self,
        symbol: ast::SymbolHandle,
        declaration: ast::Node,
    ) {
        self.binding_state
            .add_symbol_declaration_if_unique(symbol, declaration);
    }

    fn set_symbol_value_declaration(
        &mut self,
        symbol: ast::SymbolHandle,
        value_declaration: Option<ast::Node>,
    ) {
        self.binding_state
            .set_symbol_value_declaration(symbol, value_declaration);
    }

    fn set_symbol_parent(&mut self, symbol: ast::SymbolHandle, parent: Option<ast::SymbolHandle>) {
        self.binding_state.set_symbol_parent(symbol, parent);
    }

    fn set_symbol_export_symbol(
        &mut self,
        symbol: ast::SymbolHandle,
        export_symbol: Option<ast::SymbolHandle>,
    ) {
        self.binding_state
            .set_symbol_export_symbol(symbol, export_symbol);
    }

    fn insert_symbol_export(
        &mut self,
        symbol: ast::SymbolHandle,
        name: impl Into<ast::SymbolName>,
        export: ast::SymbolHandle,
    ) {
        self.binding_state
            .insert_symbol_export(symbol, name, export);
    }

    fn set_value_declaration(&mut self, symbol: ast::SymbolHandle, node: &mut ast::Node) {
        let value_declaration = self.symbol_value_declaration(symbol);
        if value_declaration.is_none()
            || value_declaration.is_some_and(|value_declaration| {
                is_assignment_declaration(self.store, &value_declaration)
            }) && !is_assignment_declaration(self.store, node)
            || value_declaration.is_some_and(|value_declaration| value_declaration != *node)
                && value_declaration.is_some_and(|value_declaration| {
                    is_effective_module_declaration(self.store, &value_declaration)
                })
        {
            // Non-assignment declarations take precedence over assignment declarations and
            // non-namespace declarations take precedence over namespace declarations.
            self.set_symbol_value_declaration(symbol, Some(*node));
        }
    }

    fn add_declaration_to_symbol(
        &mut self,
        symbol: ast::SymbolHandle,
        node: &mut ast::Node,
        symbol_flags: ast::SymbolFlags,
    ) {
        self.add_symbol_flags(symbol, symbol_flags);
        self.record_declaration_symbol(*node, symbol);
        self.add_symbol_declaration_if_unique(symbol, *node);
        if symbol_flags.intersects(
            ast::SYMBOL_FLAGS_CLASS
                | ast::SYMBOL_FLAGS_ENUM
                | ast::SYMBOL_FLAGS_MODULE
                | ast::SYMBOL_FLAGS_VARIABLE,
        ) {
            self.binding_state.ensure_symbol_exports(symbol);
        }
        if symbol_flags.intersects(
            ast::SYMBOL_FLAGS_CLASS
                | ast::SYMBOL_FLAGS_INTERFACE
                | ast::SYMBOL_FLAGS_TYPE_LITERAL
                | ast::SYMBOL_FLAGS_OBJECT_LITERAL,
        ) {
            self.binding_state.ensure_symbol_members(symbol);
        }
        // On merge of const enum module with class or function, reset const enum only flag (namespaces will already recalculate)
        if self
            .symbol_flags(symbol)
            .intersects(ast::SYMBOL_FLAGS_CONST_ENUM_ONLY_MODULE)
            && self.symbol_flags(symbol).intersects(
                ast::SYMBOL_FLAGS_FUNCTION
                    | ast::SYMBOL_FLAGS_CLASS
                    | ast::SYMBOL_FLAGS_REGULAR_ENUM,
            )
        {
            self.remove_symbol_flags(symbol, ast::SYMBOL_FLAGS_CONST_ENUM_ONLY_MODULE);
            self.not_const_enum_only_modules.add(symbol);
        }
        if symbol_flags.intersects(ast::SYMBOL_FLAGS_VALUE) {
            self.set_value_declaration(symbol, node);
        }
    }

    fn container_from_node(&self, node: &ast::Node) -> BinderContainer {
        BinderContainer {
            kind: self.store.kind(*node),
            flags: self.flags(*node),
            symbol: self.symbol(*node),
            parent_symbol: self
                .store
                .parent(*node)
                .and_then(|parent| self.symbol(parent)),
            locals_key: *node,
            is_source_file: ast::is_source_file(&self.store, *node),
            is_locals_container: ast::is_locals_container(&self.store, *node),
            is_static: ast::is_static(&self.store, *node),
        }
    }

    fn container_from_source_file(&self, locals_key: ast::Node) -> BinderContainer {
        BinderContainer {
            kind: ast::Kind::SourceFile,
            flags: self.flags(self.file),
            symbol: self.file_symbol,
            parent_symbol: None,
            locals_key,
            is_source_file: true,
            is_locals_container: true,
            is_static: false,
        }
    }

    fn set_node_flow_node(&mut self, node: ast::Node, flow_node: Option<ast::FlowRef>) {
        self.binding_state.set_flow_node(node, flow_node);
    }

    fn set_return_flow_node(&mut self, node: ast::Node, return_flow_node: Option<ast::FlowRef>) {
        self.binding_state
            .set_return_flow_node(node, return_flow_node);
    }

    fn declare_symbol_in_locals(
        &mut self,
        locals_key: ast::Node,
        node: &mut ast::Node,
        includes: ast::SymbolFlags,
        excludes: ast::SymbolFlags,
    ) -> ast::SymbolHandle {
        self.declare_symbol(
            BindingSymbolTable::Locals(locals_key),
            None,
            node,
            includes,
            excludes,
        )
    }

    fn declare_symbol_in_file_global_exports(
        &mut self,
        parent: Option<ast::SymbolHandle>,
        node: &mut ast::Node,
        includes: ast::SymbolFlags,
        excludes: ast::SymbolFlags,
    ) -> ast::SymbolHandle {
        let mut global_exports = std::mem::take(&mut self.file_global_exports);
        let symbol = self.declare_symbol(
            BindingSymbolTable::Borrowed(&mut global_exports),
            parent,
            node,
            includes,
            excludes,
        );
        self.file_global_exports = global_exports;
        symbol
    }

    fn declare_symbol_in_file_js_global_augmentations(
        &mut self,
        node: &mut ast::Node,
        includes: ast::SymbolFlags,
        excludes: ast::SymbolFlags,
    ) -> ast::SymbolHandle {
        let mut js_global_augmentations = std::mem::take(&mut self.file_js_global_augmentations);
        let symbol = self.declare_symbol(
            BindingSymbolTable::Borrowed(&mut js_global_augmentations),
            None,
            node,
            includes,
            excludes,
        );
        self.file_js_global_augmentations = js_global_augmentations;
        symbol
    }

    fn take_program_binding_state(&mut self) -> ProgramBindingState {
        let mut state = std::mem::replace(
            &mut self.binding_state,
            ProgramBindingState::new(self.file, self.store),
        );
        state.finish_file_binding(FinishedFileBinding::new(
            self.symbol_count,
            std::mem::take(&mut self.classifiable_names),
            std::mem::take(&mut self.file_global_exports),
            std::mem::take(&mut self.file_js_global_augmentations),
            std::mem::take(&mut self.file_pattern_ambient_modules),
            self.file_symbol,
            self.file_end_flow_node,
            std::mem::take(&mut self.flow_graph),
            self.file_common_js_module_indicator,
            self.file_external_module_indicator,
            std::mem::take(&mut self.nested_cjs_exports),
            std::mem::take(&mut self.file_bind_suggestion_diagnostics),
            std::mem::take(&mut self.file_bind_diagnostics),
            self.flags(self.file),
        ));
        state
    }

    fn new_symbol(
        &mut self,
        flags: ast::SymbolFlags,
        name: impl Into<ast::SymbolName>,
    ) -> ast::SymbolHandle {
        self.symbol_count += 1;
        self.binding_state.create_symbol(flags, name)
    }

    fn new_empty_symbol(&mut self, name: impl Into<ast::SymbolName>) -> ast::SymbolHandle {
        self.new_symbol(ast::SYMBOL_FLAGS_NONE, name)
    }

    fn new_missing_symbol(&mut self) -> ast::SymbolHandle {
        self.new_empty_symbol(ast::INTERNAL_SYMBOL_NAME_MISSING.to_owned())
    }

    fn insert_new_symbol(
        &mut self,
        symbol_table: &mut BindingSymbolTable<'_>,
        name: &str,
        is_replaceable_by_method: bool,
    ) -> ast::SymbolHandle {
        let name = ast::SymbolName::new(name);
        let symbol = self.new_empty_symbol(name.clone());
        if is_replaceable_by_method {
            self.add_symbol_flags(symbol, ast::SYMBOL_FLAGS_REPLACEABLE_BY_METHOD);
        }
        self.insert_symbol_into_table(symbol_table, name, symbol);
        symbol
    }

    fn get_or_insert_symbol_in_table(
        &mut self,
        symbol_table: &mut BindingSymbolTable<'_>,
        name: &str,
        is_replaceable_by_method: bool,
    ) -> (ast::SymbolHandle, bool) {
        if let BindingSymbolTable::Locals(locals_key) = symbol_table {
            let initial_flags = if is_replaceable_by_method {
                ast::SYMBOL_FLAGS_REPLACEABLE_BY_METHOD
            } else {
                ast::SYMBOL_FLAGS_NONE
            };
            let (symbol, inserted) = self.binding_state.get_or_insert_local_symbol(
                *locals_key,
                ast::SymbolName::new(name),
                initial_flags,
            );
            if inserted {
                self.symbol_count += 1;
            }
            return (symbol, inserted);
        }

        if let Some(symbol) = self.lookup_symbol_in_table(symbol_table, name) {
            (symbol, false)
        } else {
            (
                self.insert_new_symbol(symbol_table, name, is_replaceable_by_method),
                true,
            )
        }
    }

    fn lookup_symbol_in_table(
        &self,
        symbol_table: &BindingSymbolTable<'_>,
        name: &str,
    ) -> Option<ast::SymbolHandle> {
        match symbol_table {
            BindingSymbolTable::Borrowed(symbol_table) => symbol_table.get(name).copied(),
            BindingSymbolTable::Locals(locals_key) => {
                self.binding_state.lookup_local(*locals_key, name)
            }
            BindingSymbolTable::SymbolExports(parent) => {
                self.binding_state.lookup_symbol_export(*parent, name)
            }
            BindingSymbolTable::SymbolMembers(parent) => {
                self.binding_state.lookup_symbol_member(*parent, name)
            }
        }
    }

    fn insert_symbol_into_table(
        &mut self,
        symbol_table: &mut BindingSymbolTable<'_>,
        name: impl Into<ast::SymbolName>,
        symbol: ast::SymbolHandle,
    ) {
        let name = name.into();
        match symbol_table {
            BindingSymbolTable::Borrowed(symbol_table) => {
                symbol_table.insert(name, symbol);
            }
            BindingSymbolTable::Locals(locals_key) => {
                self.binding_state
                    .locals_mut(*locals_key)
                    .insert(name, symbol);
            }
            BindingSymbolTable::SymbolExports(parent) => {
                self.binding_state
                    .insert_symbol_export(*parent, name, symbol);
            }
            BindingSymbolTable::SymbolMembers(parent) => {
                self.binding_state
                    .insert_symbol_member(*parent, name, symbol);
            }
        }
    }

    /**
     * Declares a Symbol for the node and adds it to symbols. Reports errors for conflicting identifier names.
     * @param symbolTable - The symbol table which node will be added to.
     * @param parent - node's parent declaration.
     * @param node - The declaration to be added to the symbol table
     * @param includes - The SymbolFlags that node has in addition to its declaration type (eg: export, ambient, etc.)
     * @param excludes - The flags which node cannot be declared alongside in a symbol table. Used to report forbidden declarations.
     */
    fn declare_symbol(
        &mut self,
        symbol_table: BindingSymbolTable<'_>,
        parent: Option<ast::SymbolHandle>,
        node: &mut ast::Node,
        includes: ast::SymbolFlags,
        excludes: ast::SymbolFlags,
    ) -> ast::SymbolHandle {
        self.declare_symbol_ex(
            symbol_table,
            parent,
            node,
            includes,
            excludes,
            false, /*isReplaceableByMethod*/
            false, /*isComputedName*/
        )
    }

    fn declare_symbol_ex(
        &mut self,
        symbol_table: BindingSymbolTable<'_>,
        parent: Option<ast::SymbolHandle>,
        node: &mut ast::Node,
        includes: ast::SymbolFlags,
        excludes: ast::SymbolFlags,
        is_replaceable_by_method: bool,
        is_computed_name: bool,
    ) -> ast::SymbolHandle {
        debug::assert(
            is_computed_name || !ast::has_dynamic_name(&self.store, *node),
            None,
        );
        let is_default_export =
            ast::has_syntactic_modifier(&self.store, *node, ast::ModifierFlags::Default)
                || ast::is_export_specifier(&self.store, *node)
                    && self
                        .store
                        .name(*node)
                        .is_some_and(|name| ast::module_export_name_is_default(&self.store, name));
        // The exported symbol for an export default function/class node is always named "default"
        let name = if is_computed_name {
            ast::INTERNAL_SYMBOL_NAME_COMPUTED.to_owned()
        } else if is_default_export && parent.is_some() {
            ast::INTERNAL_SYMBOL_NAME_DEFAULT.to_owned()
        } else {
            self.get_declaration_name(node)
        };
        self.declare_symbol_ex_with_name(
            symbol_table,
            parent,
            node,
            includes,
            excludes,
            is_replaceable_by_method,
            name,
            is_default_export,
        )
    }

    fn declare_symbol_ex_with_name(
        &mut self,
        mut symbol_table: BindingSymbolTable<'_>,
        parent: Option<ast::SymbolHandle>,
        node: &mut ast::Node,
        includes: ast::SymbolFlags,
        excludes: ast::SymbolFlags,
        is_replaceable_by_method: bool,
        name: String,
        is_default_export: bool,
    ) -> ast::SymbolHandle {
        let symbol = if name == ast::INTERNAL_SYMBOL_NAME_MISSING {
            self.new_missing_symbol()
        } else {
            // Check and see if the symbol table already has a symbol with this name.  If not,
            // create a new symbol with this name and add it to the table.  Note that we don't
            // give the new symbol any flags *yet*.  This ensures that it will not conflict
            // with the 'excludes' flags we pass in.
            //
            // If we do get an existing symbol, see if it conflicts with the new symbol we're
            // creating.  For example, a 'var' symbol and a 'class' symbol will conflict within
            // the same symbol table.  If we have a conflict, report the issue on each
            // declaration we have for this symbol, and then create a new symbol for this
            // declaration.
            //
            // Note that when properties declared in Javascript constructors
            // (marked by isReplaceableByMethod) conflict with another symbol, the property loses.
            // Always. This allows the common Javascript pattern of overwriting a prototype method
            // with an bound instance method of the same type: `this.method = this.method.bind(this)`
            //
            // If we created a new symbol, either because we didn't have a symbol with this name
            // in the symbol table, or we conflicted with an existing symbol, then just add this
            // node as the sole declaration of the new symbol.
            //
            // Otherwise, we'll be merging into a compatible existing symbol (for example when
            // you have multiple 'vars' with the same name in the same container).  In this case
            // just add this node into the declarations list of the symbol.
            if includes.intersects(ast::SYMBOL_FLAGS_CLASSIFIABLE) {
                self.classifiable_names.add(name.clone());
            }
            let (initial_symbol, inserted_symbol) = self.get_or_insert_symbol_in_table(
                &mut symbol_table,
                name.as_str(),
                is_replaceable_by_method,
            );
            let mut symbol = Some(initial_symbol);
            if !inserted_symbol
                && is_replaceable_by_method
                && !self
                    .symbol_flags(symbol.unwrap())
                    .intersects(ast::SYMBOL_FLAGS_REPLACEABLE_BY_METHOD)
            {
                // A symbol already exists, so don't add this as a declaration.
                return symbol.unwrap();
            } else if !inserted_symbol && self.symbol_flags(symbol.unwrap()).intersects(excludes) {
                if self
                    .symbol_flags(symbol.unwrap())
                    .intersects(ast::SYMBOL_FLAGS_REPLACEABLE_BY_METHOD)
                {
                    // Javascript constructor-declared symbols can be discarded in favor of
                    // prototype symbols like methods.
                    symbol = Some(self.insert_new_symbol(&mut symbol_table, name.as_str(), false));
                } else if !((includes.intersects(ast::SYMBOL_FLAGS_VARIABLE)
                    && self
                        .symbol_flags(symbol.unwrap())
                        .intersects(ast::SYMBOL_FLAGS_ASSIGNMENT))
                    || (includes.intersects(ast::SYMBOL_FLAGS_ASSIGNMENT)
                        && self
                            .symbol_flags(symbol.unwrap())
                            .intersects(ast::SYMBOL_FLAGS_VARIABLE)))
                {
                    // Assignment declarations are allowed to merge with variables, no matter what other flags they have.
                    // Report errors every position with duplicate declaration
                    // Report errors on previous encountered declarations
                    let block_scoped_var_in_same_statement_scope = includes
                        .intersects(ast::SYMBOL_FLAGS_FUNCTION_SCOPED_VARIABLE)
                        && self.with_symbol_declarations(symbol.unwrap(), |declarations| {
                            declarations.iter().any(|declaration| {
                                self.var_declaration_shares_scope_with_block_scoped_declaration(
                                    *node,
                                    *declaration,
                                ) && self
                                    .declaration_has_duplicate_identifier_diagnostic(*declaration)
                            })
                        });
                    let mut message: &'static diagnostics::Message = if self
                        .symbol_flags(symbol.unwrap())
                        .intersects(ast::SYMBOL_FLAGS_BLOCK_SCOPED_VARIABLE)
                        && !block_scoped_var_in_same_statement_scope
                    {
                        &diagnostics::CANNOT_REDECLARE_BLOCK_SCOPED_VARIABLE_0
                    } else {
                        &diagnostics::DUPLICATE_IDENTIFIER_0
                    };
                    let mut message_needs_name = true;
                    if self
                        .symbol_flags(symbol.unwrap())
                        .intersects(ast::SYMBOL_FLAGS_ENUM)
                        || includes.intersects(ast::SYMBOL_FLAGS_ENUM)
                    {
                        message = &diagnostics::ENUM_DECLARATIONS_CAN_ONLY_MERGE_WITH_NAMESPACE_OR_OTHER_ENUM_DECLARATIONS;
                        message_needs_name = false;
                    }
                    let mut multiple_default_exports = false;
                    if !self.symbol_declarations_are_empty(symbol.unwrap()) {
                        // If the current node is a default export of some sort, then check if
                        // there are any other default exports that we need to error on.
                        // We'll know whether we have other default exports depending on if `symbol` already has a declaration list set.
                        if is_default_export {
                            message = &diagnostics::A_MODULE_CANNOT_HAVE_MULTIPLE_DEFAULT_EXPORTS;
                            message_needs_name = false;
                            multiple_default_exports = true;
                        } else {
                            // This is to properly report an error in the case "export default { }" is after export default of class declaration or function declaration.
                            // Error on multiple export default in the following case:
                            // 1. multiple export default of class declaration or function declaration by checking NodeFlags.Default
                            // 2. multiple export default of export assignment. This one doesn't have NodeFlags.Default on (as export default doesn't considered as modifiers)
                            if !self.symbol_declarations_are_empty(symbol.unwrap())
                                && ast::is_export_assignment(&self.store, *node)
                                && !self.store.is_export_equals(*node).unwrap_or(false)
                            {
                                message =
                                    &diagnostics::A_MODULE_CANNOT_HAVE_MULTIPLE_DEFAULT_EXPORTS;
                                message_needs_name = false;
                                multiple_default_exports = true;
                            }
                        }
                    }
                    let declaration_name = ast::get_name_of_declaration(&self.store, Some(*node));
                    let declaration_name = declaration_name.as_ref().unwrap_or(node);
                    let mut diag = if message_needs_name {
                        self.create_diagnostic_for_node(
                            declaration_name,
                            message,
                            &[self.get_display_name(node)],
                        )
                    } else {
                        self.create_diagnostic_for_node(declaration_name, message, &[])
                    };
                    if ast::is_type_alias_declaration(&self.store, *node)
                        && ast::node_is_missing(&self.store, self.store.type_node(*node))
                        && ast::has_syntactic_modifier(
                            &self.store,
                            *node,
                            ast::ModifierFlags::Export,
                        )
                        && self.symbol_flags(symbol.unwrap()).intersects(
                            ast::SYMBOL_FLAGS_ALIAS
                                | ast::SYMBOL_FLAGS_TYPE
                                | ast::SYMBOL_FLAGS_NAMESPACE,
                        )
                    {
                        // export type T; - may have meant export type { T }?
                        diag.add_related_info(self.create_diagnostic_for_node(
                            node,
                            &diagnostics::DID_YOU_MEAN_0,
                            &[format!(
                                "export type {{ {} }}",
                                self.store
                                    .name(*node)
                                    .map_or_else(String::new, |name| self.store.text(name))
                            )],
                        ));
                    }
                    let declarations = self.collect_symbol_declarations(symbol.unwrap());
                    for (index, declaration) in declarations.iter().enumerate() {
                        let decl = ast::get_name_of_declaration(&self.store, Some(*declaration));
                        let decl = decl.as_ref().unwrap_or(declaration);
                        let mut d = if message_needs_name {
                            self.create_diagnostic_for_node(
                                decl,
                                message,
                                &[self.get_display_name(declaration)],
                            )
                        } else {
                            self.create_diagnostic_for_node(decl, message, &[])
                        };
                        if multiple_default_exports {
                            d.add_related_info(self.create_diagnostic_for_node(
                                declaration_name,
                                core::if_else(
                                    index == 0,
                                    &diagnostics::ANOTHER_EXPORT_DEFAULT_IS_HERE,
                                    &diagnostics::X_AND_HERE,
                                ),
                                &[],
                            ));
                        }
                        self.add_diagnostic(d);
                        if multiple_default_exports {
                            diag.add_related_info(self.create_diagnostic_for_node(
                                decl,
                                &diagnostics::THE_FIRST_EXPORT_DEFAULT_IS_HERE,
                                &[],
                            ));
                        }
                    }
                    self.add_diagnostic(diag);
                    // When get or set accessor conflicts with a non-accessor or an accessor of a different kind, we mark
                    // the symbol as a full accessor such that all subsequent declarations are considered conflicting. This
                    // for example ensures that a get accessor followed by a non-accessor followed by a set accessor with the
                    // same name are all marked as duplicates.
                    if self
                        .symbol_flags(symbol.unwrap())
                        .intersects(ast::SYMBOL_FLAGS_ACCESSOR)
                        && (self.symbol_flags(symbol.unwrap()) & ast::SYMBOL_FLAGS_ACCESSOR)
                            != (includes & ast::SYMBOL_FLAGS_ACCESSOR)
                    {
                        self.add_symbol_flags(symbol.unwrap(), ast::SYMBOL_FLAGS_ACCESSOR);
                    }
                    symbol = Some(self.new_empty_symbol(name.as_str()));
                }
            }
            symbol.unwrap()
        };
        self.add_declaration_to_symbol(symbol, node, includes);
        let symbol_parent = self.binding_state.symbol_parent(symbol);
        if symbol_parent.is_none() {
            self.set_symbol_parent(symbol, parent);
        } else if !same_symbol_parent(symbol_parent.as_ref(), parent.as_ref()) {
            panic!("Existing symbol parent should match new one");
        }
        symbol
    }

    fn declare_symbol_in_exports(
        &mut self,
        parent: ast::SymbolHandle,
        node: &mut ast::Node,
        includes: ast::SymbolFlags,
        excludes: ast::SymbolFlags,
    ) -> ast::SymbolHandle {
        self.declare_symbol(
            BindingSymbolTable::SymbolExports(parent),
            Some(parent),
            node,
            includes,
            excludes,
        )
    }

    fn declare_symbol_in_members(
        &mut self,
        parent: ast::SymbolHandle,
        node: &mut ast::Node,
        includes: ast::SymbolFlags,
        excludes: ast::SymbolFlags,
    ) -> ast::SymbolHandle {
        self.declare_symbol(
            BindingSymbolTable::SymbolMembers(parent),
            Some(parent),
            node,
            includes,
            excludes,
        )
    }

    fn declare_symbol_ex_in_exports(
        &mut self,
        parent: ast::SymbolHandle,
        node: &mut ast::Node,
        includes: ast::SymbolFlags,
        excludes: ast::SymbolFlags,
        is_replaceable_by_method: bool,
        is_computed_name: bool,
    ) -> ast::SymbolHandle {
        self.declare_symbol_ex(
            BindingSymbolTable::SymbolExports(parent),
            Some(parent),
            node,
            includes,
            excludes,
            is_replaceable_by_method,
            is_computed_name,
        )
    }

    fn declare_symbol_ex_in_members(
        &mut self,
        parent: ast::SymbolHandle,
        node: &mut ast::Node,
        includes: ast::SymbolFlags,
        excludes: ast::SymbolFlags,
        is_replaceable_by_method: bool,
        is_computed_name: bool,
    ) -> ast::SymbolHandle {
        self.declare_symbol_ex(
            BindingSymbolTable::SymbolMembers(parent),
            Some(parent),
            node,
            includes,
            excludes,
            is_replaceable_by_method,
            is_computed_name,
        )
    }

    fn declare_symbol_ex_with_name_in_exports(
        &mut self,
        parent: ast::SymbolHandle,
        node: &mut ast::Node,
        includes: ast::SymbolFlags,
        excludes: ast::SymbolFlags,
        is_replaceable_by_method: bool,
        name: String,
        is_computed_name: bool,
    ) -> ast::SymbolHandle {
        self.declare_symbol_ex_with_name(
            BindingSymbolTable::SymbolExports(parent),
            Some(parent),
            node,
            includes,
            excludes,
            is_replaceable_by_method,
            name,
            is_computed_name,
        )
    }

    fn declare_symbol_ex_with_name_in_members(
        &mut self,
        parent: ast::SymbolHandle,
        node: &mut ast::Node,
        includes: ast::SymbolFlags,
        excludes: ast::SymbolFlags,
        is_replaceable_by_method: bool,
        name: String,
        is_computed_name: bool,
    ) -> ast::SymbolHandle {
        self.declare_symbol_ex_with_name(
            BindingSymbolTable::SymbolMembers(parent),
            Some(parent),
            node,
            includes,
            excludes,
            is_replaceable_by_method,
            name,
            is_computed_name,
        )
    }

    // Should not be called on a declaration with a computed property name,
    // unless it is a well known Symbol.
    fn get_declaration_name(&self, node: &ast::Node) -> String {
        if ast::is_export_assignment(&self.store, *node) {
            return core::if_else(
                self.store.is_export_equals(*node).unwrap_or(false),
                ast::INTERNAL_SYMBOL_NAME_EXPORT_EQUALS,
                ast::INTERNAL_SYMBOL_NAME_DEFAULT,
            )
            .to_owned();
        }
        let name = ast::get_name_of_declaration(&self.store, Some(*node));
        if let Some(name) = name {
            if ast::is_ambient_module(&self.store, *node) {
                let module_name = self.store.text(name);
                if ast::is_global_scope_augmentation(&self.store, *node) {
                    return ast::INTERNAL_SYMBOL_NAME_GLOBAL.to_owned();
                }
                return format!("\"{}\"", module_name);
            }
            if ast::is_private_identifier(&self.store, name) {
                // containingClass exists because private names only allowed inside classes
                let containing_class = ast::get_containing_class(&self.store, *node);
                if containing_class.is_none() {
                    // we can get here in cases where there is already a parse error.
                    return ast::INTERNAL_SYMBOL_NAME_MISSING.to_owned();
                }
                let containing_class = containing_class.unwrap();
                let Some(containing_class_symbol) = self.symbol(containing_class) else {
                    return ast::INTERNAL_SYMBOL_NAME_MISSING.to_owned();
                };
                return get_symbol_name_for_private_identifier(
                    &self.binding_state,
                    containing_class_symbol,
                    &self.store.text(name),
                );
            }
            if ast::is_property_name_literal(&self.store, name)
                || ast::is_jsx_namespaced_name(&self.store, name)
            {
                return self.store.text(name);
            }
            if ast::is_computed_property_name(&self.store, name) {
                let name_expression = self.store.expression(name);
                // treat computed property names where expression is string/numeric literal as just string/numeric literal
                if name_expression.as_ref().is_some_and(|name_expression| {
                    ast::is_string_or_numeric_literal_like(&self.store, *name_expression)
                }) {
                    let name_expression = name_expression.as_ref().unwrap();
                    return self.store.text(*name_expression);
                }
                if name_expression.as_ref().is_some_and(|name_expression| {
                    ast::is_signed_numeric_literal(&self.store, *name_expression)
                }) {
                    let name_expression = name_expression.as_ref().unwrap();
                    return scanner::token_to_string(
                        self.store.operator(*name_expression).unwrap(),
                    )
                    .to_owned()
                        + &self
                            .store
                            .text(self.store.operand(*name_expression).unwrap());
                }
                panic!("Only computed properties with literal names have declaration names");
            }
            return ast::INTERNAL_SYMBOL_NAME_MISSING.to_owned();
        }
        match self.store.kind(*node) {
            ast::Kind::Constructor => ast::INTERNAL_SYMBOL_NAME_CONSTRUCTOR.to_owned(),
            ast::Kind::FunctionType | ast::Kind::CallSignature => {
                ast::INTERNAL_SYMBOL_NAME_CALL.to_owned()
            }
            ast::Kind::ConstructorType | ast::Kind::ConstructSignature => {
                ast::INTERNAL_SYMBOL_NAME_NEW.to_owned()
            }
            ast::Kind::IndexSignature => ast::INTERNAL_SYMBOL_NAME_INDEX.to_owned(),
            ast::Kind::ExportDeclaration => ast::INTERNAL_SYMBOL_NAME_EXPORT_STAR.to_owned(),
            ast::Kind::SourceFile | ast::Kind::BinaryExpression => {
                ast::INTERNAL_SYMBOL_NAME_EXPORT_EQUALS.to_owned()
            }
            _ => ast::INTERNAL_SYMBOL_NAME_MISSING.to_owned(),
        }
    }

    fn get_display_name(&self, node: &ast::Node) -> String {
        if let Some(name_node) = self.store.name(*node) {
            return self.declaration_name_to_string(&name_node);
        }
        let name = self.get_declaration_name(node);
        if name != ast::INTERNAL_SYMBOL_NAME_MISSING {
            return name;
        }
        "(Missing)".to_owned()
    }
}

pub fn get_symbol_name_for_private_identifier(
    binding_state: &ProgramBindingState,
    containing_class_symbol: ast::SymbolHandle,
    description: &str,
) -> String {
    binding_state.private_identifier_symbol_name(containing_class_symbol, description)
}

fn same_symbol_parent(left: Option<&ast::SymbolHandle>, right: Option<&ast::SymbolHandle>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left == right,
        (None, None) => true,
        _ => false,
    }
}

impl<'a> Binder<'a> {
    fn declare_module_member(
        &mut self,
        node: &mut ast::Node,
        symbol_flags: ast::SymbolFlags,
        symbol_excludes: ast::SymbolFlags,
    ) -> ast::SymbolHandle {
        let container = self
            .container_state
            .clone()
            .expect("container should be active");
        let has_export_modifier = ast::get_combined_modifier_flags(&self.store, *node)
            .intersects(ast::ModifierFlags::Export)
            || ast::is_implicitly_exported_js_type_alias(&self.store, *node);
        if symbol_flags.intersects(ast::SYMBOL_FLAGS_ALIAS) {
            let kind = self.store.kind(*node);
            if kind == ast::Kind::ExportSpecifier
                || (kind == ast::Kind::ImportEqualsDeclaration && has_export_modifier)
            {
                let parent = container.symbol.unwrap();
                return self.declare_symbol_in_exports(parent, node, symbol_flags, symbol_excludes);
            }
            return self.declare_symbol_in_locals(
                container.locals_key,
                node,
                symbol_flags,
                symbol_excludes,
            );
        }
        // Exported module members are given 2 symbols: A local symbol that is classified with an ExportValue flag,
        // and an associated export symbol with all the correct flags set on it. There are 2 main reasons:
        //
        //   1. We treat locals and exports of the same name as mutually exclusive within a container.
        //      That means the binder will issue a Duplicate Identifier error if you mix locals and exports
        //      with the same name in the same container.
        //      TODO: Make this a more specific error and decouple it from the exclusion logic.
        //   2. When we checkIdentifier in the checker, we set its resolved symbol to the local symbol,
        //      but return the export symbol (by calling getExportSymbolOfValueSymbolIfExported). That way
        //      when the emitter comes back to it, it knows not to qualify the name if it was found in a containing scope.
        //
        // NOTE: Nested ambient modules always should go to to 'locals' table to prevent their automatic merge
        //       during global merging in the checker. Why? The only case when ambient module is permitted inside another module is module augmentation
        //       and this case is specially handled. Module augmentations should only be merged with original module definition
        //       and should never be merged directly with other augmentation, and the latter case would be possible if automatic merge is allowed.
        let container_flags = self.flags(container.locals_key);
        if !ast::is_ambient_module(&self.store, *node)
            && (has_export_modifier || container_flags.intersects(ast::NodeFlags::ExportContext))
        {
            if !container.is_locals_container
                || (ast::has_syntactic_modifier(&self.store, *node, ast::ModifierFlags::Default)
                    && self.get_declaration_name(node) == ast::INTERNAL_SYMBOL_NAME_MISSING)
            {
                let parent = container.symbol.unwrap();
                return self.declare_symbol_in_exports(parent, node, symbol_flags, symbol_excludes);
                // No local symbol for an unnamed default!
            }
            let mut export_kind = ast::SYMBOL_FLAGS_NONE;
            if symbol_flags.intersects(ast::SYMBOL_FLAGS_VALUE) {
                export_kind = ast::SYMBOL_FLAGS_EXPORT_VALUE;
            }
            let local = self.declare_symbol_in_locals(
                container.locals_key,
                node,
                export_kind,
                symbol_excludes,
            );
            let export_symbol = {
                let parent = container.symbol.unwrap();
                self.declare_symbol_in_exports(parent, node, symbol_flags, symbol_excludes)
            };
            self.set_symbol_export_symbol(local, Some(export_symbol));
            self.record_exportable_local_symbol(*node, local);
            return local;
        }
        self.declare_symbol_in_locals(container.locals_key, node, symbol_flags, symbol_excludes)
    }

    fn declare_class_member(
        &mut self,
        node: &mut ast::Node,
        symbol_flags: ast::SymbolFlags,
        symbol_excludes: ast::SymbolFlags,
    ) -> ast::SymbolHandle {
        let container = self
            .container_state
            .clone()
            .expect("container should be active");
        if ast::is_static(&self.store, *node) {
            let parent = container.symbol.unwrap();
            let name = self.get_declaration_name(node);
            return self.declare_symbol_ex_with_name_in_exports(
                parent,
                node,
                symbol_flags,
                symbol_excludes,
                false,
                name,
                false,
            );
        }
        let parent = container.symbol.unwrap();
        let name = self.get_declaration_name(node);
        self.declare_symbol_ex_with_name_in_members(
            parent,
            node,
            symbol_flags,
            symbol_excludes,
            false,
            name,
            false,
        )
    }

    fn declare_source_file_member(
        &mut self,
        node: &mut ast::Node,
        symbol_flags: ast::SymbolFlags,
        symbol_excludes: ast::SymbolFlags,
    ) -> ast::SymbolHandle {
        if self.file_is_external_or_common_js_module {
            return self.declare_module_member(node, symbol_flags, symbol_excludes);
        }
        let locals_key = self
            .container_state
            .as_ref()
            .expect("source file container should be active")
            .locals_key;
        self.declare_symbol_in_locals(locals_key, node, symbol_flags, symbol_excludes)
    }

    fn variable_statement_scope_container(&self, node: ast::Node) -> Option<ast::Node> {
        let var_decl_list =
            ast::find_ancestor_kind(self.store, Some(node), ast::Kind::VariableDeclarationList)?;
        let var_statement = self.store.parent(var_decl_list)?;
        if ast::is_variable_statement(self.store, var_statement) {
            self.store.parent(var_statement)
        } else {
            None
        }
    }

    fn var_declaration_shares_scope_with_block_scoped_declaration(
        &self,
        node: ast::Node,
        declaration: ast::Node,
    ) -> bool {
        let Some(node_container) = self.variable_statement_scope_container(node) else {
            return false;
        };
        let Some(declaration_container) = self.variable_statement_scope_container(declaration)
        else {
            return false;
        };
        node_container == declaration_container
    }

    fn declaration_has_duplicate_identifier_diagnostic(&self, declaration: ast::Node) -> bool {
        let declaration_name =
            ast::get_name_of_declaration(self.store, Some(declaration)).unwrap_or(declaration);
        let source_file = self.source_file_view();
        let range = scanner::get_error_range_for_node(&source_file, &declaration_name);
        self.file_bind_diagnostics.iter().any(|diagnostic| {
            diagnostic.code() == diagnostics::DUPLICATE_IDENTIFIER_0.code()
                && diagnostic.pos() == range.pos()
                && diagnostic.end() == range.end()
        })
    }

    fn declare_symbol_and_add_to_symbol_table(
        &mut self,
        node: &mut ast::Node,
        symbol_flags: ast::SymbolFlags,
        symbol_excludes: ast::SymbolFlags,
    ) -> ast::SymbolHandle {
        let container = self
            .container_state
            .clone()
            .expect("container should be active");
        match container.kind {
            ast::Kind::ModuleDeclaration => {
                self.declare_module_member(node, symbol_flags, symbol_excludes)
            }
            ast::Kind::SourceFile => {
                self.declare_source_file_member(node, symbol_flags, symbol_excludes)
            }
            ast::Kind::ClassExpression | ast::Kind::ClassDeclaration => {
                self.declare_class_member(node, symbol_flags, symbol_excludes)
            }
            ast::Kind::EnumDeclaration => {
                let parent = container.symbol.unwrap();
                self.declare_symbol_in_exports(parent, node, symbol_flags, symbol_excludes)
            }
            ast::Kind::TypeLiteral
            | ast::Kind::ObjectLiteralExpression
            | ast::Kind::InterfaceDeclaration
            | ast::Kind::JsxAttributes => {
                let parent = container.symbol.unwrap();
                self.declare_symbol_in_members(parent, node, symbol_flags, symbol_excludes)
            }
            ast::Kind::FunctionType
            | ast::Kind::ConstructorType
            | ast::Kind::CallSignature
            | ast::Kind::ConstructSignature
            | ast::Kind::IndexSignature
            | ast::Kind::MethodDeclaration
            | ast::Kind::MethodSignature
            | ast::Kind::Constructor
            | ast::Kind::GetAccessor
            | ast::Kind::SetAccessor
            | ast::Kind::FunctionDeclaration
            | ast::Kind::FunctionExpression
            | ast::Kind::ArrowFunction
            | ast::Kind::ClassStaticBlockDeclaration
            | ast::Kind::TypeAliasDeclaration
            | ast::Kind::JSTypeAliasDeclaration
            | ast::Kind::MappedType => self.declare_symbol_in_locals(
                container.locals_key,
                node,
                symbol_flags,
                symbol_excludes,
            ),
            _ => panic!("Unhandled case in declareSymbolAndAddToSymbolTable"),
        }
    }

    fn new_flow_node(&mut self, flags: ast::FlowFlags) -> ast::FlowRef {
        self.flow_graph.new_node(ast::FlowNode {
            flags,
            ..Default::default()
        })
    }

    fn new_flow_node_ex(
        &mut self,
        flags: ast::FlowFlags,
        node: impl Into<ast::FlowNodeReference>,
        antecedent: ast::FlowRef,
    ) -> ast::FlowRef {
        let result = self.new_flow_node(flags);
        {
            let mut flow = self.flow_graph.node_mut(result);
            flow.node = Some(node.into());
            flow.antecedent = Some(antecedent);
        }
        result
    }

    fn create_loop_label(&mut self) -> ast::FlowRef {
        self.new_flow_node(ast::FlowFlags::LoopLabel)
    }

    fn create_branch_label(&mut self) -> ast::FlowRef {
        self.new_flow_node(ast::FlowFlags::BranchLabel)
    }

    fn create_reduce_label(
        &mut self,
        target: ast::FlowRef,
        antecedents: Option<ast::FlowListRef>,
        antecedent: ast::FlowRef,
    ) -> ast::FlowRef {
        let data = ast::new_flow_reduce_label_data(Some(target), antecedents);
        self.new_flow_node_ex(ast::FlowFlags::ReduceLabel, data, antecedent)
    }

    fn create_flow_condition(
        &mut self,
        flags: ast::FlowFlags,
        antecedent: ast::FlowRef,
        expression: Option<&mut ast::Node>,
    ) -> ast::FlowRef {
        if self
            .flow_graph
            .node(antecedent)
            .flags
            .intersects(ast::FlowFlags::Unreachable)
        {
            return antecedent;
        }
        let Some(expression) = expression else {
            if flags.intersects(ast::FlowFlags::TrueCondition) {
                return antecedent;
            }
            return self.unreachable_flow.unwrap();
        };
        let expression_kind = self.store.kind(*expression);
        if ((expression_kind == ast::Kind::TrueKeyword
            && flags.intersects(ast::FlowFlags::FalseCondition))
            || (expression_kind == ast::Kind::FalseKeyword
                && flags.intersects(ast::FlowFlags::TrueCondition)))
            && !ast::is_expression_of_optional_chain_root(&self.store, *expression)
            && !self
                .store
                .parent(*expression)
                .is_some_and(|parent| ast::is_nullish_coalesce(&self.store, parent))
        {
            return self.unreachable_flow.unwrap();
        }
        if !is_narrowing_expression(&self.store, expression) {
            return antecedent;
        }
        self.set_flow_node_referenced(antecedent);
        self.new_flow_node_ex(flags, *expression, antecedent)
    }

    fn create_flow_mutation(
        &mut self,
        flags: ast::FlowFlags,
        antecedent: ast::FlowRef,
        node: &mut ast::Node,
    ) -> ast::FlowRef {
        self.set_flow_node_referenced(antecedent);
        self.has_flow_effects = true;
        let result = self.new_flow_node_ex(flags, *node, antecedent);
        if let Some(current_exception_target) = self.current_exception_target {
            self.add_antecedent(current_exception_target, result);
        }
        result
    }

    fn create_flow_switch_clause(
        &mut self,
        antecedent: ast::FlowRef,
        switch_statement: &mut ast::Node,
        clause_start: i32,
        clause_end: i32,
    ) -> ast::FlowRef {
        self.set_flow_node_referenced(antecedent);
        let data = ast::new_flow_switch_clause_data(
            Some(switch_statement.clone()),
            clause_start,
            clause_end,
        );
        self.new_flow_node_ex(ast::FlowFlags::SwitchClause, data, antecedent)
    }

    fn create_flow_call(&mut self, antecedent: ast::FlowRef, node: &mut ast::Node) -> ast::FlowRef {
        self.set_flow_node_referenced(antecedent);
        self.has_flow_effects = true;
        self.new_flow_node_ex(ast::FlowFlags::Call, *node, antecedent)
    }

    fn new_flow_list(
        &mut self,
        head: ast::FlowRef,
        tail: Option<ast::FlowListRef>,
    ) -> ast::FlowListRef {
        self.flow_graph.new_list(ast::FlowList {
            flow: Some(head),
            next: tail,
        })
    }

    fn combine_flow_lists(
        &mut self,
        head: Option<ast::FlowListRef>,
        tail: Option<ast::FlowListRef>,
    ) -> Option<ast::FlowListRef> {
        let Some(head) = head else {
            return tail;
        };
        let (flow, next) = {
            let list = self.flow_graph.list(head);
            (list.flow.unwrap(), list.next)
        };
        let next = self.combine_flow_lists(next, tail);
        Some(self.new_flow_list(flow, next))
    }
}

impl<'a> Binder<'a> {
    fn set_flow_node_referenced(&self, flow: ast::FlowRef) {
        let mut flow = self.flow_graph.node_mut(flow);
        // On first reference we set the Referenced flag, thereafter we set the Shared flag
        if !flow.flags.intersects(ast::FlowFlags::Referenced) {
            flow.flags |= ast::FlowFlags::Referenced;
        } else {
            flow.flags |= ast::FlowFlags::Shared;
        }
    }

    fn add_antecedent(&mut self, label: ast::FlowRef, antecedent: ast::FlowRef) {
        if self
            .flow_graph
            .node(antecedent)
            .flags
            .intersects(ast::FlowFlags::Unreachable)
        {
            return;
        }
        // If antecedent isn't already on the Antecedents list, add it to the end of the list
        let mut last: Option<ast::FlowListRef> = None;
        let mut list = self.flow_graph.node(label).antecedents;
        while let Some(current) = list {
            let current_list = self.flow_graph.list(current);
            if current_list.flow == Some(antecedent) {
                return;
            }
            last = Some(current);
            list = current_list.next;
        }
        let new_list = self.new_flow_list(antecedent, None);
        if let Some(last) = last {
            self.flow_graph.list_mut(last).next = Some(new_list);
        } else {
            self.flow_graph.node_mut(label).antecedents = Some(new_list);
        }
        self.set_flow_node_referenced(antecedent);
    }

    fn finish_flow_label(&mut self, label: ast::FlowRef) -> ast::FlowRef {
        let Some(antecedents) = self.flow_graph.node(label).antecedents else {
            return self.unreachable_flow.unwrap();
        };
        let list = self.flow_graph.list(antecedents);
        if list.next.is_none() {
            return list.flow.unwrap();
        }
        label
    }

    fn bind<N>(&mut self, node: Option<N>) -> bool
    where
        N: std::ops::DerefMut<Target = ast::Node>,
    {
        let Some(mut node) = node else {
            return false;
        };
        let node = &mut *node;
        // First we bind declaration nodes to a symbol if possible. We'll both create a symbol
        // and then potentially add the symbol to an appropriate symbol table. Possible
        // destination symbol tables are:
        //
        //  1) The 'exports' table of the current container's symbol.
        //  2) The 'members' table of the current container's symbol.
        //  3) The 'locals' table of the current container.
        //
        // However, not all symbols will end up in any of these tables. 'Anonymous' symbols
        // (like TypeLiterals for example) will not be put in any table.
        match self.store.kind(*node) {
            ast::Kind::Identifier => {
                self.set_node_flow_node(*node, self.current_flow);
                self.check_contextual_identifier(node);
            }
            ast::Kind::ThisKeyword | ast::Kind::SuperKeyword => {
                if self.store.kind(*node) == ast::Kind::ThisKeyword {
                    self.seen_this_keyword = true;
                }
                self.set_node_flow_node(*node, self.current_flow);
            }
            ast::Kind::QualifiedName => {
                if self.current_flow.is_some() && ast::is_part_of_type_query(&self.store, *node) {
                    self.set_node_flow_node(*node, self.current_flow);
                }
            }
            ast::Kind::MetaProperty => {
                self.set_node_flow_node(*node, self.current_flow);
            }
            ast::Kind::PrivateIdentifier => {
                self.check_private_identifier(node);
            }
            ast::Kind::PropertyAccessExpression | ast::Kind::ElementAccessExpression => {
                if self.current_flow.is_some() && is_narrowable_reference(&self.store, node) {
                    self.set_node_flow_node(*node, self.current_flow);
                }
            }
            ast::Kind::BinaryExpression => {
                self.bind_binary_expression_worker(node);
            }
            ast::Kind::CatchClause => self.check_strict_mode_catch_clause(node),
            ast::Kind::DeleteExpression => self.check_strict_mode_delete_expression(node),
            ast::Kind::PostfixUnaryExpression => {
                self.check_strict_mode_postfix_unary_expression(node)
            }
            ast::Kind::PrefixUnaryExpression => {
                self.check_strict_mode_prefix_unary_expression(node)
            }
            ast::Kind::WithStatement => self.check_strict_mode_with_statement(node),
            ast::Kind::LabeledStatement => self.check_strict_mode_labeled_statement(node),
            ast::Kind::ThisType => self.seen_this_keyword = true,
            ast::Kind::TypeParameter => self.bind_type_parameter(node),
            ast::Kind::Parameter => self.bind_parameter(node),
            ast::Kind::VariableDeclaration => {
                self.bind_variable_declaration_or_binding_element(node)
            }
            ast::Kind::BindingElement => {
                self.set_node_flow_node(*node, self.current_flow);
                self.bind_variable_declaration_or_binding_element(node);
            }
            ast::Kind::PropertyDeclaration | ast::Kind::PropertySignature => {
                self.bind_property_worker(node)
            }
            ast::Kind::PropertyAssignment | ast::Kind::ShorthandPropertyAssignment => {
                self.bind_object_property_declaration(node)
            }
            ast::Kind::EnumMember => self.bind_enum_member_declaration(node),
            ast::Kind::CallSignature
            | ast::Kind::ConstructSignature
            | ast::Kind::IndexSignature => self.bind_signature_declaration(node),
            ast::Kind::MethodDeclaration | ast::Kind::MethodSignature => {
                self.bind_method_worker(node);
            }
            ast::Kind::FunctionDeclaration => self.bind_function_declaration(node),
            ast::Kind::Constructor => self.bind_constructor_declaration(node),
            ast::Kind::GetAccessor | ast::Kind::SetAccessor => self.bind_accessor_declaration(node),
            ast::Kind::FunctionType | ast::Kind::ConstructorType => {
                self.bind_function_or_constructor_type(node)
            }
            ast::Kind::TypeLiteral | ast::Kind::MappedType | ast::Kind::ObjectLiteralExpression => {
                self.bind_anonymous_object_or_type_declaration(node)
            }
            ast::Kind::FunctionExpression | ast::Kind::ArrowFunction => {
                self.bind_function_expression(node)
            }
            ast::Kind::ClassExpression | ast::Kind::ClassDeclaration => {
                self.bind_class_like_declaration(node)
            }
            ast::Kind::InterfaceDeclaration => self.bind_interface_declaration(node),
            ast::Kind::CallExpression => {
                if let Some(kind) = ast::get_assignment_declaration_kind(&self.store, *node) {
                    match kind {
                        ast::JSDeclarationKind::ObjectDefinePropertyValue => {
                            self.bind_expando_property_assignment(node)
                        }
                        ast::JSDeclarationKind::ObjectDefinePropertyExports => {
                            self.bind_object_define_property_export(node)
                        }
                        _ => {}
                    }
                }
                if ast::is_in_js_file(&self.store, *node) {
                    self.bind_call_expression(node);
                }
            }
            ast::Kind::TypeAliasDeclaration | ast::Kind::JSTypeAliasDeclaration => {
                self.bind_type_alias_declaration(node)
            }
            ast::Kind::EnumDeclaration => self.bind_enum_declaration(node),
            ast::Kind::ModuleDeclaration => self.bind_module_declaration(node),
            ast::Kind::ImportEqualsDeclaration
            | ast::Kind::NamespaceImport
            | ast::Kind::ImportSpecifier
            | ast::Kind::ExportSpecifier
            | ast::Kind::ImportClause => self.bind_alias_declaration(node),
            ast::Kind::NamespaceExportDeclaration => self.bind_namespace_export_declaration(node),
            ast::Kind::ExportDeclaration => self.bind_export_declaration(node),
            ast::Kind::ExportAssignment => self.bind_export_assignment(node),
            ast::Kind::SourceFile => self.bind_source_file_if_external_module(),
            ast::Kind::JsxAttributes => self.bind_jsx_attributes(node),
            ast::Kind::JsxAttribute => self.bind_jsx_attribute(
                node,
                ast::SYMBOL_FLAGS_PROPERTY,
                ast::SYMBOL_FLAGS_PROPERTY_EXCLUDES,
            ),
            _ => {}
        }
        // Then we recurse into the children of the node to bind them as well. For certain
        // symbols we do specialized work when we recurse. For example, we'll keep track of
        // the current 'container' node when it changes. This helps us know which symbol table
        // a local should go into for example. Since terminal nodes are known not to have
        // children, as an optimization we don't process those.
        let mut this_node_or_any_subnodes_has_error = self
            .store
            .flags(*node)
            .intersects(ast::NodeFlags::ThisNodeHasError);
        if self.store.kind(*node) > ast::Kind::LastToken {
            let save_seen_parse_error = self.seen_parse_error;
            self.seen_parse_error = false;
            let container_flags = get_container_flags(&self.store, *node);
            if container_flags == CONTAINER_FLAGS_NONE {
                self.bind_children(node);
            } else {
                self.bind_container(node, container_flags);
            }
            if self.seen_parse_error {
                this_node_or_any_subnodes_has_error = true;
            }
            self.seen_parse_error = save_seen_parse_error;
        }
        if this_node_or_any_subnodes_has_error {
            self.mark_subtree_has_error(*node);
            self.seen_parse_error = true;
        }
        false
    }

    fn bind_property_worker(&mut self, node: &mut ast::Node) {
        if !matches!(
            self.store.kind(*node),
            ast::Kind::PropertyDeclaration | ast::Kind::PropertySignature
        ) {
            return;
        }
        let is_auto_accessor = ast::is_auto_accessor_property_declaration(&self.store, *node);
        let includes = core::if_else(
            is_auto_accessor,
            ast::SYMBOL_FLAGS_ACCESSOR,
            ast::SYMBOL_FLAGS_PROPERTY,
        );
        let excludes = core::if_else(
            is_auto_accessor,
            ast::SYMBOL_FLAGS_ACCESSOR_EXCLUDES,
            ast::SYMBOL_FLAGS_PROPERTY_EXCLUDES,
        );
        self.bind_property_or_method_or_accessor(
            node,
            includes
                | core::if_else(
                    self.store
                        .postfix_token(*node)
                        .is_some_and(|postfix_token| {
                            self.store.kind(postfix_token) == ast::Kind::QuestionToken
                        }),
                    ast::SYMBOL_FLAGS_OPTIONAL,
                    ast::SYMBOL_FLAGS_NONE,
                ),
            excludes,
        );
    }

    fn bind_method_worker(&mut self, node: &mut ast::Node) {
        if !matches!(
            self.store.kind(*node),
            ast::Kind::MethodDeclaration | ast::Kind::MethodSignature
        ) {
            return;
        }
        self.bind_property_or_method_or_accessor(
            node,
            ast::SYMBOL_FLAGS_METHOD
                | core::if_else(
                    self.store
                        .postfix_token(*node)
                        .is_some_and(|postfix_token| {
                            self.store.kind(postfix_token) == ast::Kind::QuestionToken
                        }),
                    ast::SYMBOL_FLAGS_OPTIONAL,
                    ast::SYMBOL_FLAGS_NONE,
                ),
            core::if_else(
                ast::is_object_literal_method(&self.store, Some(*node)),
                ast::SYMBOL_FLAGS_VALUE,
                ast::SYMBOL_FLAGS_METHOD_EXCLUDES,
            ),
        );
    }
}

impl<'a> Binder<'a> {
    fn bind_source_file_if_external_module(&mut self) {
        let mut file_node = self.file;
        self.set_export_context_flag(&mut file_node);
        if self.file_has_external_module_indicator {
            self.bind_source_file_as_external_module();
        } else if self.file_is_json_source_file {
            self.bind_source_file_as_external_module();
            // Create symbol equivalent for the module.exports = {}
            let original_symbol = self.file_symbol.clone();
            let parent_symbol = self
                .file_symbol
                .clone()
                .expect("JSON source file should have a symbol");
            self.declare_symbol_in_exports(
                parent_symbol,
                &mut file_node,
                ast::SYMBOL_FLAGS_PROPERTY,
                ast::SYMBOL_FLAGS_ALL,
            );
            self.file_symbol = original_symbol;
        }
    }

    fn bind_source_file_as_external_module(&mut self) {
        let module_name = { format!("\"{}\"", tspath::remove_file_extension(&self.file_name)) };
        self.bind_anonymous_source_file_declaration(ast::SYMBOL_FLAGS_VALUE_MODULE, module_name);
    }

    fn bind_module_declaration(&mut self, node: &mut ast::Node) {
        self.set_export_context_flag(node);
        if !ast::is_module_declaration(&self.store, *node) {
            return;
        }

        if ast::is_ambient_module(&self.store, *node) {
            if ast::has_syntactic_modifier(&self.store, *node, ast::ModifierFlags::EXPORT) {
                self.error_on_first_token(
                    node,
                    &diagnostics::X_EXPORT_MODIFIER_CANNOT_BE_APPLIED_TO_AMBIENT_MODULES_AND_MODULE_AUGMENTATIONS_SINCE_THEY_ARE_ALWAYS_VISIBLE,
                    &[],
                );
            }
            if ast::is_module_augmentation_external(&self.store, *node) {
                self.declare_module_symbol(node);
            } else {
                let symbol = self.declare_symbol_and_add_to_symbol_table(
                    node,
                    ast::SYMBOL_FLAGS_VALUE_MODULE,
                    ast::SYMBOL_FLAGS_VALUE_MODULE_EXCLUDES,
                );
                if let Some(name) = ast::module_string_literal_name(&self.store, *node) {
                    let name_text = self.store.text(name);
                    let pattern = core::try_parse_pattern(&name_text);
                    if !pattern.is_valid() {
                        // An invalid pattern - must have multiple wildcards.
                        self.error_on_first_token(
                            &name,
                            &diagnostics::PATTERN_0_CAN_HAVE_AT_MOST_ONE_ASTERISK_CHARACTER,
                            &[name_text],
                        );
                    } else if pattern.star_index >= 0 {
                        self.file_pattern_ambient_modules
                            .push(ast::PatternAmbientModule {
                                pattern,
                                symbol: Some(symbol),
                            });
                    }
                }
            }
        } else {
            let state = self.declare_module_symbol(node);
            if state != ast::ModuleInstanceState::NonInstantiated {
                let symbol_handle = self
                    .symbol(*node)
                    .expect("module declaration should have a symbol after declaration");
                let const_enum_only_module = !self.symbol_flags(symbol_handle).intersects(
                    ast::SYMBOL_FLAGS_FUNCTION
                        | ast::SYMBOL_FLAGS_CLASS
                        | ast::SYMBOL_FLAGS_REGULAR_ENUM,
                ) && state == ast::ModuleInstanceState::ConstEnumOnly
                    && !self.not_const_enum_only_modules.has(&symbol_handle);
                if const_enum_only_module {
                    self.add_symbol_flags(symbol_handle, ast::SYMBOL_FLAGS_CONST_ENUM_ONLY_MODULE);
                } else {
                    self.remove_symbol_flags(
                        symbol_handle,
                        ast::SYMBOL_FLAGS_CONST_ENUM_ONLY_MODULE,
                    );
                    self.not_const_enum_only_modules.add(symbol_handle);
                }
            }
        }
    }

    fn declare_module_symbol(&mut self, node: &mut ast::Node) -> ast::ModuleInstanceState {
        let state = ast::get_module_instance_state(&self.store, *node);
        let instantiated = state != ast::ModuleInstanceState::NonInstantiated;
        self.declare_symbol_and_add_to_symbol_table(
            node,
            core::if_else(
                instantiated,
                ast::SYMBOL_FLAGS_VALUE_MODULE,
                ast::SYMBOL_FLAGS_NAMESPACE_MODULE,
            ),
            core::if_else(
                instantiated,
                ast::SYMBOL_FLAGS_VALUE_MODULE_EXCLUDES,
                ast::SYMBOL_FLAGS_NAMESPACE_MODULE_EXCLUDES,
            ),
        );
        state
    }

    fn bind_namespace_export_declaration(&mut self, node: &mut ast::Node) {
        if !ast::is_namespace_export_declaration(&self.store, *node) {
            return;
        }
        if self.store.modifiers(*node).is_some() {
            self.error_on_node(node, &diagnostics::MODIFIERS_CANNOT_APPEAR_HERE, &[]);
        }
        let Some(parent) = self.store.parent(*node) else {
            self.error_on_node(
                node,
                &diagnostics::GLOBAL_MODULE_EXPORTS_MAY_ONLY_APPEAR_AT_TOP_LEVEL,
                &[],
            );
            return;
        };
        if !ast::is_source_file(&self.store, parent) {
            self.error_on_node(
                node,
                &diagnostics::GLOBAL_MODULE_EXPORTS_MAY_ONLY_APPEAR_AT_TOP_LEVEL,
                &[],
            );
        } else if self
            .store
            .as_source_file(parent)
            .external_module_indicator()
            .is_none()
        {
            self.error_on_node(
                node,
                &diagnostics::GLOBAL_MODULE_EXPORTS_MAY_ONLY_APPEAR_IN_MODULE_FILES,
                &[],
            );
        } else if !self.store.as_source_file(parent).is_declaration_file() {
            self.error_on_node(
                node,
                &diagnostics::GLOBAL_MODULE_EXPORTS_MAY_ONLY_APPEAR_IN_DECLARATION_FILES,
                &[],
            );
        } else {
            let parent_symbol = self.file_symbol.clone();
            self.declare_symbol_in_file_global_exports(
                parent_symbol,
                node,
                ast::SYMBOL_FLAGS_ALIAS,
                ast::SYMBOL_FLAGS_ALIAS_EXCLUDES,
            );
        }
    }

    fn bind_alias_declaration(&mut self, node: &mut ast::Node) {
        if !ast::is_alias_declaration(&self.store, *node) {
            return;
        }
        self.declare_symbol_and_add_to_symbol_table(
            node,
            ast::SYMBOL_FLAGS_ALIAS,
            ast::SYMBOL_FLAGS_ALIAS_EXCLUDES,
        );
    }

    fn bind_export_declaration(&mut self, node: &mut ast::Node) {
        if !ast::is_export_declaration(&self.store, *node) {
            return;
        }
        let export_clause = self.store.export_clause(*node);
        let container = self
            .container_state
            .clone()
            .expect("container should be active");
        if container.symbol.is_none() {
            // Export * in some sort of block construct
            self.bind_anonymous_declaration(
                node,
                ast::SYMBOL_FLAGS_EXPORT_STAR,
                self.get_declaration_name(node),
            );
        } else if export_clause.is_none() {
            // All export * declarations are collected in an __export symbol
            let parent = container
                .symbol
                .expect("export container should have a symbol");
            self.declare_symbol_in_exports(
                parent,
                node,
                ast::SYMBOL_FLAGS_EXPORT_STAR,
                ast::SYMBOL_FLAGS_NONE,
            );
        } else if export_clause
            .is_some_and(|export_clause| ast::is_namespace_export(&self.store, export_clause))
        {
            let parent = container
                .symbol
                .expect("export container should have a symbol");
            let mut export_clause = export_clause.unwrap();
            self.declare_symbol_in_exports(
                parent,
                &mut export_clause,
                ast::SYMBOL_FLAGS_ALIAS,
                ast::SYMBOL_FLAGS_ALIAS_EXCLUDES,
            );
        }
    }

    fn bind_export_assignment(&mut self, node: &mut ast::Node) {
        if !ast::is_export_assignment(&self.store, *node) {
            return;
        }
        let container = self
            .container_state
            .clone()
            .expect("container should be active");
        if container.symbol.is_none() {
            // Incorrect export assignment in some sort of block construct
            self.bind_anonymous_declaration(
                node,
                ast::SYMBOL_FLAGS_VALUE,
                self.get_declaration_name(node),
            );
        } else {
            let parent = container
                .symbol
                .expect("export container should have a symbol");
            let symbol_flags = if self
                .store
                .expression(*node)
                .is_some_and(|expression| ast::expression_is_alias(&self.store, expression))
            {
                ast::SYMBOL_FLAGS_ALIAS
            } else {
                ast::SYMBOL_FLAGS_PROPERTY
            };
            let symbol =
                self.declare_symbol_in_exports(parent, node, symbol_flags, ast::SYMBOL_FLAGS_ALL);
            if self.store.is_export_equals(*node).unwrap_or(false) {
                // Ensure export assignments have a ValueDeclaration set.
                self.set_value_declaration(symbol, node);
            }
        }
    }

    fn bind_jsx_attributes(&mut self, node: &mut ast::Node) {
        self.bind_anonymous_declaration(
            node,
            ast::SYMBOL_FLAGS_OBJECT_LITERAL,
            ast::INTERNAL_SYMBOL_NAME_JSX_ATTRIBUTES.to_owned(),
        );
    }

    fn bind_jsx_attribute(
        &mut self,
        node: &mut ast::Node,
        symbol_flags: ast::SymbolFlags,
        symbol_excludes: ast::SymbolFlags,
    ) {
        self.declare_symbol_and_add_to_symbol_table(node, symbol_flags, symbol_excludes);
    }

    fn set_export_context_flag(&mut self, node: &mut ast::Node) {
        // A declaration source file or ambient module declaration that contains no export declarations (but possibly regular
        // declarations with export modifiers) is an export context in which declarations are implicitly exported.
        if self.flags(*node).intersects(ast::NodeFlags::Ambient)
            && !self.has_export_declarations(node)
        {
            self.set_export_context(*node, true);
        } else {
            self.set_export_context(*node, false);
        }
    }

    fn has_export_declarations(&self, node: &ast::Node) -> bool {
        if !matches!(
            self.store.kind(*node),
            ast::Kind::SourceFile | ast::Kind::ModuleDeclaration
        ) {
            return false;
        }
        ast::statement_container_statements(self.store, *node).is_some_and(|statements| {
            statements.iter().any(|statement| {
                ast::is_export_declaration(self.store, statement)
                    || ast::is_export_assignment(self.store, statement)
            })
        })
    }

    fn bind_function_expression(&mut self, node: &mut ast::Node) {
        if !self.file_is_declaration_file
            && !self.flags(*node).intersects(ast::NodeFlags::Ambient)
            && ast::is_async_function(&self.store, *node)
        {
            self.emit_flags |= ast::NodeFlags::HasAsyncFunctions;
        }
        self.set_node_flow_node(*node, self.current_flow);
        let mut binding_name = ast::INTERNAL_SYMBOL_NAME_FUNCTION.to_owned();
        if ast::is_function_expression(&self.store, *node) && self.store.name(*node).is_some() {
            self.check_strict_mode_function_name(node);
            binding_name = self
                .store
                .name(*node)
                .map(|name| self.store.text(name))
                .unwrap();
        }
        self.bind_anonymous_declaration(node, ast::SYMBOL_FLAGS_FUNCTION, binding_name);
    }

    fn bind_call_expression(&mut self, node: &mut ast::Node) {
        // We're only inspecting call expressions to detect CommonJS modules, so we can skip
        // this check if we've already seen the module indicator
        if !self.file_has_common_js_module_indicator
            && ast::is_require_call(&self.store, *node, false)
        {
            self.set_common_js_module_indicator(node);
        }
    }

    fn set_common_js_module_indicator(&mut self, node: &mut ast::Node) -> bool {
        let source_file_node = self.file;
        if self
            .file_external_module_indicator
            .as_ref()
            .is_some_and(|indicator| *indicator != source_file_node)
        {
            return false;
        }
        if self.file_common_js_module_indicator.is_none() {
            self.file_common_js_module_indicator = Some(node.clone());
            self.file_has_common_js_module_indicator = true;
            self.file_is_external_or_common_js_module = true;
            if self.file_external_module_indicator.is_none() {
                self.bind_source_file_as_external_module();
            }
        }
        true
    }

    fn track_nested_cjs_export(&mut self, node: &ast::Node) {
        let Some(parent) = self.store.parent(*node) else {
            return;
        };
        if !(ast::is_source_file(self.store, parent)
            || ast::is_expression_statement(self.store, parent)
                && self
                    .store
                    .parent(parent)
                    .is_some_and(|parent| ast::is_source_file(self.store, parent)))
        {
            self.nested_cjs_exports.push(*node);
        }
    }

    fn bind_type_alias_declaration(&mut self, node: &mut ast::Node) {
        if !ast::is_type_or_js_type_alias_declaration(&self.store, *node) {
            return;
        }
        let current_container_is_source_file = self
            .block_scope_container_state
            .as_ref()
            .map(|container| container.is_source_file);
        if self.store.kind(*node) == ast::Kind::TypeAliasDeclaration
            || current_container_is_source_file.is_some_and(|is_source_file| !is_source_file)
        {
            self.bind_block_scoped_declaration(
                node,
                ast::SYMBOL_FLAGS_TYPE_ALIAS,
                ast::SYMBOL_FLAGS_TYPE_ALIAS_EXCLUDES,
            );
        }
    }

    fn bind_class_like_declaration(&mut self, node: &mut ast::Node) {
        let kind = self.store.kind(*node);
        if !matches!(
            kind,
            ast::Kind::ClassDeclaration | ast::Kind::ClassExpression
        ) {
            return;
        }
        match kind {
            ast::Kind::ClassDeclaration => self.bind_block_scoped_declaration(
                node,
                ast::SYMBOL_FLAGS_CLASS,
                ast::SYMBOL_FLAGS_CLASS_EXCLUDES,
            ),
            ast::Kind::ClassExpression => {
                let name = self.store.name(*node);
                let name_text = name
                    .map(|name| self.store.text(name))
                    .unwrap_or_else(|| ast::INTERNAL_SYMBOL_NAME_CLASS.to_owned());
                if name.is_some() {
                    self.classifiable_names.add(name_text.clone());
                }
                self.bind_anonymous_declaration(node, ast::SYMBOL_FLAGS_CLASS, name_text);
            }
            _ => unreachable!("class-like binding only accepts class nodes"),
        }
        let symbol = self.symbol(*node).unwrap();
        // TypeScript 1.0 spec (April 2014): 8.4
        // Every class automatically contains a static property member named 'prototype', the
        // type of which is an instantiation of the class type with type Any supplied as a type
        // argument for each type parameter. It is an error to explicitly declare a static
        // property member with the name 'prototype'.
        //
        // Note: we check for this here because this class may be merging into a module.  The
        // module might have an exported variable called 'prototype'.  We can't allow that as
        // that would clash with the built-in 'prototype' for the class.
        let prototype_symbol = self.new_symbol(
            ast::SYMBOL_FLAGS_PROPERTY | ast::SYMBOL_FLAGS_PROTOTYPE,
            "prototype".to_owned(),
        );
        let prototype_name = self.symbol_name(prototype_symbol);
        let symbol_export = self.symbol_export(symbol, prototype_name.as_str());
        if let Some(symbol_export) = symbol_export {
            let declaration = self.first_symbol_declaration(symbol_export).unwrap();
            self.error_on_node(
                &declaration,
                &diagnostics::DUPLICATE_IDENTIFIER_0,
                &[self.symbol_display_name(prototype_symbol)],
            );
        }
        self.set_symbol_parent(prototype_symbol, Some(symbol));
        self.insert_symbol_export(symbol, prototype_name, prototype_symbol);
    }

    fn bind_interface_declaration(&mut self, node: &mut ast::Node) {
        if !ast::is_interface_declaration(&self.store, *node) {
            return;
        }
        self.bind_block_scoped_declaration(
            node,
            ast::SYMBOL_FLAGS_INTERFACE,
            ast::SYMBOL_FLAGS_INTERFACE_EXCLUDES,
        );
    }

    fn bind_anonymous_object_or_type_declaration(&mut self, node: &mut ast::Node) {
        match self.store.kind(*node) {
            ast::Kind::TypeLiteral | ast::Kind::MappedType => self.bind_anonymous_declaration(
                node,
                ast::SYMBOL_FLAGS_TYPE_LITERAL,
                ast::INTERNAL_SYMBOL_NAME_TYPE.to_owned(),
            ),
            ast::Kind::ObjectLiteralExpression => self.bind_anonymous_declaration(
                node,
                ast::SYMBOL_FLAGS_OBJECT_LITERAL,
                ast::INTERNAL_SYMBOL_NAME_OBJECT.to_owned(),
            ),
            _ => return,
        }
    }

    fn bind_signature_declaration(&mut self, node: &mut ast::Node) {
        if !matches!(
            self.store.kind(*node),
            ast::Kind::CallSignature | ast::Kind::ConstructSignature | ast::Kind::IndexSignature
        ) {
            return;
        }
        self.declare_symbol_and_add_to_symbol_table(
            node,
            ast::SYMBOL_FLAGS_SIGNATURE,
            ast::SYMBOL_FLAGS_NONE,
        );
    }

    fn bind_constructor_declaration(&mut self, node: &mut ast::Node) {
        if !ast::is_constructor_declaration(&self.store, *node) {
            return;
        }
        self.declare_symbol_and_add_to_symbol_table(
            node,
            ast::SYMBOL_FLAGS_CONSTRUCTOR,
            ast::SYMBOL_FLAGS_NONE,
        );
    }

    fn bind_accessor_declaration(&mut self, node: &mut ast::Node) {
        let (symbol_flags, symbol_excludes) = match self.store.kind(*node) {
            ast::Kind::GetAccessor => (
                ast::SYMBOL_FLAGS_GET_ACCESSOR,
                ast::SYMBOL_FLAGS_GET_ACCESSOR_EXCLUDES,
            ),
            ast::Kind::SetAccessor => (
                ast::SYMBOL_FLAGS_SET_ACCESSOR,
                ast::SYMBOL_FLAGS_SET_ACCESSOR_EXCLUDES,
            ),
            _ => return,
        };
        self.bind_property_or_method_or_accessor(node, symbol_flags, symbol_excludes);
    }

    fn bind_object_property_declaration(&mut self, node: &mut ast::Node) {
        if !matches!(
            self.store.kind(*node),
            ast::Kind::PropertyAssignment | ast::Kind::ShorthandPropertyAssignment
        ) {
            return;
        }
        self.bind_property_or_method_or_accessor(
            node,
            ast::SYMBOL_FLAGS_PROPERTY,
            ast::SYMBOL_FLAGS_PROPERTY_EXCLUDES,
        );
    }

    fn bind_enum_member_declaration(&mut self, node: &mut ast::Node) {
        if !ast::is_enum_member(&self.store, *node) {
            return;
        }
        self.bind_property_or_method_or_accessor(
            node,
            ast::SYMBOL_FLAGS_ENUM_MEMBER,
            ast::SYMBOL_FLAGS_ENUM_MEMBER_EXCLUDES,
        );
    }

    fn bind_property_or_method_or_accessor(
        &mut self,
        node: &mut ast::Node,
        symbol_flags: ast::SymbolFlags,
        symbol_excludes: ast::SymbolFlags,
    ) {
        if !self.file_is_declaration_file
            && !self.flags(*node).intersects(ast::NodeFlags::Ambient)
            && ast::is_async_function(&self.store, *node)
        {
            self.emit_flags |= ast::NodeFlags::HasAsyncFunctions;
        }
        if self.current_flow.is_some()
            && ast::is_object_literal_or_class_expression_method_or_accessor(&self.store, *node)
        {
            self.set_node_flow_node(*node, self.current_flow);
        }
        if ast::has_dynamic_name(&self.store, *node) {
            self.bind_anonymous_declaration(
                node,
                symbol_flags,
                ast::INTERNAL_SYMBOL_NAME_COMPUTED.to_owned(),
            );
        } else {
            self.declare_symbol_and_add_to_symbol_table(node, symbol_flags, symbol_excludes);
        }
    }

    fn bind_function_or_constructor_type(&mut self, node: &mut ast::Node) {
        let signature_internal_name = match self.store.kind(*node) {
            ast::Kind::FunctionType => ast::INTERNAL_SYMBOL_NAME_CALL,
            ast::Kind::ConstructorType => ast::INTERNAL_SYMBOL_NAME_NEW,
            _ => {
                return;
            }
        };
        let type_literal_internal_name = ast::INTERNAL_SYMBOL_NAME_TYPE;
        // For a given function symbol "<...>(...) => T" we want to generate a symbol identical
        // to the one we would get for: { <...>(...): T }
        //
        // We do that by making an anonymous type literal symbol, and then setting the function
        // symbol as its sole member. To the rest of the system, this symbol will be indistinguishable
        // from an actual type literal symbol you would have gotten had you used the long form.
        let symbol = self.new_symbol(
            ast::SYMBOL_FLAGS_SIGNATURE,
            signature_internal_name.to_owned(),
        );
        self.add_declaration_to_symbol(symbol, node, ast::SYMBOL_FLAGS_SIGNATURE);
        let type_literal_symbol = self.new_symbol(
            ast::SYMBOL_FLAGS_TYPE_LITERAL,
            type_literal_internal_name.to_owned(),
        );
        self.add_declaration_to_symbol(type_literal_symbol, node, ast::SYMBOL_FLAGS_TYPE_LITERAL);
        {
            let symbol_name = self.symbol_name(symbol).clone();
            self.binding_state
                .insert_symbol_member(type_literal_symbol, symbol_name, symbol);
        }
    }

    fn add_late_bound_assignment_declaration_to_symbol(
        &mut self,
        node: &mut ast::Node,
        symbol: ast::SymbolHandle,
    ) {
        let assignment_symbol = if let Some(assignment_symbol) =
            self.symbol_export(symbol, ast::INTERNAL_SYMBOL_NAME_ASSIGNMENT_DECLARATION)
        {
            assignment_symbol
        } else {
            let assignment_symbol = self.new_symbol(
                ast::SYMBOL_FLAGS_NONE,
                ast::INTERNAL_SYMBOL_NAME_ASSIGNMENT_DECLARATION,
            );
            self.insert_symbol_export(
                symbol,
                ast::INTERNAL_SYMBOL_NAME_ASSIGNMENT_DECLARATION,
                assignment_symbol,
            );
            assignment_symbol
        };
        self.add_symbol_declaration(assignment_symbol, *node);
    }
}

impl<'a> Binder<'a> {
    fn bind_module_exports_assignment(&mut self, node: &mut ast::Node) {
        if self.set_common_js_module_indicator(node) {
            self.track_nested_cjs_export(node);
            let parent = self
                .file_symbol
                .expect("external source file should have a symbol");
            let flags = if self
                .store
                .right(*node)
                .is_some_and(|right| ast::expression_is_alias(&self.store, right))
            {
                ast::SYMBOL_FLAGS_ALIAS
            } else {
                ast::SYMBOL_FLAGS_PROPERTY
            };
            let symbol = self.declare_symbol(
                BindingSymbolTable::SymbolExports(parent),
                Some(parent),
                node,
                flags,
                ast::SYMBOL_FLAGS_NONE,
            );
            self.set_value_declaration(symbol, node);
        }
    }

    fn is_exports_or_module_exports_or_alias(&self, node: ast::Node) -> bool {
        let mut queue = vec![node];
        let mut count = 0;
        while let Some(node) = queue.pop() {
            count += 1;
            if count >= 100 {
                return false;
            }
            if ast::is_exports_identifier(&self.store, node)
                || ast::is_module_exports_access_expression(&self.store, node)
            {
                return true;
            }
            if ast::is_identifier(&self.store, node) {
                let name = self.store.text(node);
                let file_key = self.file;
                let Some(symbol) = self.binding_state.lookup_local(file_key, name.as_str()) else {
                    continue;
                };
                let Some(value_declaration) = self.symbol_value_declaration(symbol) else {
                    continue;
                };
                if !ast::is_variable_declaration(&self.store, value_declaration) {
                    continue;
                }
                let Some(initializer) = self.store.initializer(value_declaration) else {
                    continue;
                };
                queue.push(initializer);
                if ast::is_binary_expression(&self.store, initializer)
                    && self
                        .store
                        .operator_token(initializer)
                        .is_some_and(|operator| self.store.kind(operator) == ast::Kind::EqualsToken)
                {
                    if let Some(left) = self.store.left(initializer) {
                        queue.push(left);
                    }
                    if let Some(right) = self.store.right(initializer) {
                        queue.push(right);
                    }
                }
            }
        }
        false
    }

    fn lookup_symbol_for_name_in_block_scope_container(&self, name: &str) -> bool {
        let Some(container) = self.block_scope_container_state.as_ref() else {
            return false;
        };
        if self
            .binding_state
            .lookup_local(container.locals_key, name)
            .is_some()
        {
            return true;
        }
        container.symbol.is_some_and(|symbol| {
            self.binding_state
                .lookup_symbol_export(symbol, name)
                .is_some()
        })
    }

    fn bind_expando_property_assignment(&mut self, node: &mut ast::Node) {
        self.expando_assignments.push(ExpandoAssignmentInfo {
            node: node.clone(),
            container_state: self
                .container_state
                .clone()
                .expect("container should be active"),
            block_scope_container_state: self
                .block_scope_container_state
                .clone()
                .expect("block scope container should be active"),
        });
    }

    fn bind_special_property_assignment(&mut self, node: &mut ast::Node) {
        // Class declarations in Typescript do not allow property declarations
        let Some(left) = self.store.left(*node) else {
            return;
        };
        let Some(expression) = self.store.expression(left) else {
            return;
        };
        let block_scope_container = self
            .block_scope_container_state
            .clone()
            .expect("block scope container should be active");
        let container = self
            .container_state
            .clone()
            .expect("container should be active");
        let parent_symbol = self
            .lookup_symbol_for_property_access(expression, Some(&block_scope_container))
            .or_else(|| self.lookup_symbol_for_property_access(expression, Some(&container)));
        if !node_is_in_js_file(&self.store, *node) && !self.is_function_symbol(parent_symbol) {
            return;
        }
        let root_expr = ast::get_leftmost_access_expression(&self.store, left);
        if ast::is_identifier(&self.store, root_expr) {
            let root_name = self.store.text(root_expr);
            if self
                .lookup_name(root_name.as_str(), &container)
                .is_some_and(|symbol| {
                    self.symbol_flags(symbol)
                        .intersects(ast::SYMBOL_FLAGS_ALIAS)
                })
            {
                return;
            }
        }
        self.bind_expando_property_assignment(node);
    }

    fn bind_deferred_expando_assignments(&mut self) {
        for info in self.expando_assignments.clone() {
            self.container_state = Some(info.container_state);
            self.block_scope_container_state = Some(info.block_scope_container_state);
            let mut node = info.node;
            self.bind_deferred_expando_assignment(&mut node);
        }
    }

    // If the given module symbol has an export= symbol, promote exports with a type or namespace meaning
    // from the module symbol onto the export= symbol and, if any such exports exist, mark the export=
    // symbol as a namespace module.
    fn bind_common_js_type_exports(&mut self, module_symbol: ast::SymbolHandle) {
        if let Some(export_equals) =
            self.symbol_export(module_symbol, ast::INTERNAL_SYMBOL_NAME_EXPORT_EQUALS)
        {
            let symbols: Vec<_> =
                self.binding_state
                    .with_symbol_exports(module_symbol, |exports| {
                        exports
                            .map(|exports| exports.values().copied().collect())
                            .unwrap_or_default()
                    });
            for symbol in symbols {
                if self.symbol_name(symbol) != ast::INTERNAL_SYMBOL_NAME_EXPORT_EQUALS
                    && self
                        .symbol_flags(symbol)
                        .intersects(ast::SYMBOL_FLAGS_TYPE | ast::SYMBOL_FLAGS_NAMESPACE)
                {
                    let name = self.symbol_name(symbol);
                    self.insert_symbol_export(export_equals, name, symbol);
                    self.add_symbol_flags(export_equals, ast::SYMBOL_FLAGS_NAMESPACE_MODULE);
                }
            }
        }
    }

    fn bind_potentially_missing_namespaces(
        &mut self,
        namespace_symbol: Option<ast::SymbolHandle>,
        entity_name: ast::Node,
        is_toplevel: bool,
        is_prototype_property: bool,
        container_is_class: bool,
    ) -> Option<ast::SymbolHandle> {
        if namespace_symbol.is_some_and(|symbol| {
            self.symbol_flags(symbol)
                .intersects(ast::SYMBOL_FLAGS_ALIAS)
        }) {
            return namespace_symbol;
        }
        let mut namespace_symbol = namespace_symbol;
        if is_toplevel && !is_prototype_property {
            namespace_symbol =
                self.bind_missing_namespaces_in_entity_name(entity_name, namespace_symbol);
        }
        if container_is_class
            && let Some(namespace_symbol) = namespace_symbol
            && let Some(mut value_declaration) = self.symbol_value_declaration(namespace_symbol)
        {
            self.add_declaration_to_symbol(
                namespace_symbol,
                &mut value_declaration,
                ast::SYMBOL_FLAGS_CLASS,
            );
        }
        namespace_symbol
    }

    fn bind_missing_namespaces_in_entity_name(
        &mut self,
        entity_name: ast::Node,
        parent: Option<ast::SymbolHandle>,
    ) -> Option<ast::SymbolHandle> {
        if self.is_exports_or_module_exports_or_alias(entity_name) {
            return self.file_symbol;
        }
        if ast::is_identifier(&self.store, entity_name) {
            let symbol = self
                .container_state
                .clone()
                .and_then(|container| self.lookup_entity(&entity_name, &container));
            return self.bind_missing_namespace_identifier(entity_name, symbol, parent);
        }
        let expression = self.store.expression(entity_name)?;
        let parent_symbol = self.bind_missing_namespaces_in_entity_name(expression, parent);
        let name = ast::get_element_or_property_access_name(&self.store, entity_name)?;
        let existing = parent_symbol.and_then(|parent_symbol| {
            let name_text = self.store.text(name);
            self.symbol_export(parent_symbol, name_text.as_str())
        });
        self.bind_missing_namespace_identifier(name, existing, parent_symbol)
    }

    fn bind_missing_namespace_identifier(
        &mut self,
        mut declaration: ast::Node,
        symbol: Option<ast::SymbolHandle>,
        parent: Option<ast::SymbolHandle>,
    ) -> Option<ast::SymbolHandle> {
        let flags = ast::SYMBOL_FLAGS_MODULE | ast::SYMBOL_FLAGS_ASSIGNMENT;
        let exclude_flags = ast::SYMBOL_FLAGS_VALUE_MODULE_EXCLUDES & !ast::SYMBOL_FLAGS_ASSIGNMENT;
        if let Some(symbol) = symbol {
            self.add_declaration_to_symbol(symbol, &mut declaration, flags);
            return Some(symbol);
        }
        if let Some(parent) = parent {
            return Some(self.declare_symbol_in_exports(
                parent,
                &mut declaration,
                flags,
                exclude_flags,
            ));
        }
        Some(self.declare_symbol_in_file_js_global_augmentations(
            &mut declaration,
            flags,
            exclude_flags,
        ))
    }

    fn get_namespace_expando_symbol(
        &self,
        namespace_symbol: ast::SymbolHandle,
    ) -> Option<ast::SymbolHandle> {
        if self
            .symbol_flags(namespace_symbol)
            .intersects(ast::SYMBOL_FLAGS_MODULE)
        {
            return Some(namespace_symbol);
        }
        self.get_initializer_symbol(Some(namespace_symbol))
    }

    fn bind_deferred_expando_assignment(&mut self, node: &mut ast::Node) {
        let parent = get_parent_of_property_assignment(&self.store, node);
        let block_scope_container = self
            .block_scope_container_state
            .clone()
            .expect("block scope container should be active");
        let container = self
            .container_state
            .clone()
            .expect("container should be active");
        let mut symbol = self.lookup_entity(&parent, &block_scope_container);
        if symbol.is_none() {
            symbol = self.lookup_entity(&parent, &container);
        }
        if let Some(symbol) = self.get_initializer_symbol(symbol) {
            if ast::has_dynamic_name(&self.store, *node) {
                self.bind_anonymous_declaration(
                    node,
                    ast::SYMBOL_FLAGS_PROPERTY | ast::SYMBOL_FLAGS_ASSIGNMENT,
                    ast::INTERNAL_SYMBOL_NAME_COMPUTED.to_owned(),
                );
                self.add_late_bound_assignment_declaration_to_symbol(node, symbol);
            } else {
                // We declare expandos only when there are no non-expando declarations for that name.
                let mut declaration = *node;
                let declaration_name = self.get_declaration_name(&declaration);
                if self
                    .symbol_export(symbol, declaration_name.as_str())
                    .is_none_or(|existing| {
                        self.symbol_flags(existing)
                            .intersects(ast::SYMBOL_FLAGS_ASSIGNMENT)
                    })
                {
                    self.declare_symbol_in_exports(
                        symbol,
                        &mut declaration,
                        ast::SYMBOL_FLAGS_PROPERTY | ast::SYMBOL_FLAGS_ASSIGNMENT,
                        ast::SYMBOL_FLAGS_PROPERTY_EXCLUDES,
                    );
                }
            }
        }
    }

    fn is_top_level_namespace_assignment(&self, node: &ast::Node) -> bool {
        let top = match self.store.kind(*node) {
            ast::Kind::BinaryExpression => {
                let mut expr = *node;
                while let Some(parent) = self.store.parent(expr)
                    && self.store.kind(parent) == ast::Kind::BinaryExpression
                {
                    expr = parent;
                }
                self.store.parent(expr)
            }
            ast::Kind::CallExpression => self.store.parent(*node),
            _ => None,
        };
        top.is_some_and(|top| {
            ast::is_source_file(&self.store, top)
                || self
                    .store
                    .parent(top)
                    .is_some_and(|parent| ast::is_source_file(&self.store, parent))
        })
    }

    fn bind_prototype_property_assignment(&mut self, node: &mut ast::Node) {
        let Some(lhs) = self.store.left(*node) else {
            return;
        };
        let Some(class_prototype) = self.store.expression(lhs) else {
            return;
        };
        let Some(constructor_function) = self.store.expression(class_prototype) else {
            return;
        };
        let block_scope_container = self
            .block_scope_container_state
            .clone()
            .expect("block scope container should be active");
        let container = self
            .container_state
            .clone()
            .expect("container should be active");
        let mut symbol = self.lookup_entity(&constructor_function, &block_scope_container);
        if symbol.is_none() {
            symbol = self.lookup_entity(&constructor_function, &container);
        }
        let Some(symbol) = symbol else {
            return;
        };
        let Some(mut value_declaration) = self.symbol_value_declaration(symbol) else {
            return;
        };
        self.add_declaration_to_symbol(symbol, &mut value_declaration, ast::SYMBOL_FLAGS_CLASS);
        let Some(initializer) = self.store.right(*node) else {
            return;
        };
        let (includes, excludes) =
            if ast::is_function_like_declaration(&self.store, Some(initializer)) {
                (
                    ast::SYMBOL_FLAGS_METHOD,
                    ast::SYMBOL_FLAGS_METHOD_EXCLUDES & !ast::SYMBOL_FLAGS_ASSIGNMENT,
                )
            } else {
                (
                    ast::SYMBOL_FLAGS_PROPERTY,
                    ast::SYMBOL_FLAGS_PROPERTY_EXCLUDES & !ast::SYMBOL_FLAGS_ASSIGNMENT,
                )
            };
        let mut declaration = lhs;
        self.declare_symbol_in_members(
            symbol,
            &mut declaration,
            includes | ast::SYMBOL_FLAGS_ASSIGNMENT,
            excludes,
        );
    }
}

fn get_parent_of_property_assignment(store: &ast::AstStore, node: &ast::Node) -> ast::Node {
    match store.kind(*node) {
        ast::Kind::BinaryExpression => store
            .left(*node)
            .and_then(|left| store.expression(left))
            .unwrap(),
        ast::Kind::CallExpression => store
            .arguments(*node)
            .and_then(|args| args.first())
            .unwrap(),
        _ => panic!("Unhandled case in getParentOfPropertyAssignment"),
    }
}

impl<'a> Binder<'a> {
    fn bind_object_define_property_export(&mut self, node: &mut ast::Node) {
        if self.set_common_js_module_indicator(node) {
            self.track_nested_cjs_export(node);
            let parent = self
                .file_symbol
                .clone()
                .expect("external source file should have a symbol");
            let flags = if ast::is_binary_expression(&self.store, *node)
                && self
                    .store
                    .right(*node)
                    .is_some_and(|right| ast::expression_is_alias(&self.store, right))
            {
                ast::SYMBOL_FLAGS_ALIAS
            } else {
                ast::SYMBOL_FLAGS_FUNCTION_SCOPED_VARIABLE
            };
            self.declare_symbol_in_exports(
                parent,
                node,
                flags,
                ast::SYMBOL_FLAGS_FUNCTION_SCOPED_VARIABLE_EXCLUDES,
            );
        }
    }

    fn bind_exports_property_assignment(&mut self, node: &mut ast::Node) {
        if !self.set_common_js_module_indicator(node) {
            return;
        }
        self.track_nested_cjs_export(node);
        let Some(left) = self.store.left(*node) else {
            return;
        };
        let Some(left_expression) = self.store.expression(left) else {
            return;
        };
        let is_alias = self
            .store
            .right(*node)
            .is_some_and(|right| ast::expression_is_alias(&self.store, right))
            && (ast::is_exports_identifier(&self.store, left_expression)
                || ast::is_module_exports_access_expression(&self.store, left_expression));
        let flags = if is_alias {
            ast::SYMBOL_FLAGS_ALIAS
        } else {
            ast::SYMBOL_FLAGS_FUNCTION_SCOPED_VARIABLE
        };
        let parent = self
            .file_symbol
            .clone()
            .expect("external source file should have a symbol");
        self.declare_symbol_in_exports(
            parent,
            node,
            flags,
            ast::SYMBOL_FLAGS_FUNCTION_SCOPED_VARIABLE_EXCLUDES,
        );
    }
}

impl<'a> Binder<'a> {
    fn get_initializer_symbol(
        &self,
        symbol: Option<ast::SymbolHandle>,
    ) -> Option<ast::SymbolHandle> {
        let symbol = symbol?;
        let declaration = self.symbol_value_declaration(symbol)?;
        // For an assignment 'fn.xxx = ...', where 'fn' is a previously declared function or a previously
        // declared const variable initialized with a function expression or arrow function, we add expando
        // property declarations to the function's symbol. This also applies to class expressions in JS files,
        // and empty object literals in JS files when the declaration doesn't have a type annotation.
        if ast::is_function_declaration(&self.store, declaration)
            || node_is_in_js_file(&self.store, declaration)
                && ast::is_class_declaration(&self.store, declaration)
        {
            return Some(symbol);
        }
        if ast::is_variable_declaration(&self.store, declaration) {
            let is_const_or_js = self
                .store
                .parent(declaration)
                .map(|parent| self.flags(parent))
                .unwrap_or_default()
                .intersects(ast::NodeFlags::Const)
                || node_is_in_js_file(&self.store, declaration);
            if is_const_or_js {
                if let Some(initializer) = self.store.initializer(declaration) {
                    if is_expando_initializer_in_store(&self.store, declaration, initializer) {
                        return self.symbol(initializer);
                    }
                }
            }
        }
        if ast::is_binary_expression(&self.store, declaration)
            && node_is_in_js_file(&self.store, declaration)
        {
            let initializer = self.store.right(declaration);
            if let Some(initializer) = initializer {
                if is_expando_initializer_in_store(&self.store, declaration, initializer) {
                    return self.symbol(initializer);
                }
            }
        }
        None
    }

    fn is_function_symbol(&self, symbol: Option<ast::SymbolHandle>) -> bool {
        let Some(symbol) = symbol else {
            return false;
        };
        let Some(declaration) = self.symbol_value_declaration(symbol) else {
            return false;
        };
        ast::is_function_declaration(&self.store, declaration)
            || ast::is_variable_declaration(&self.store, declaration)
                && self
                    .store
                    .initializer(declaration)
                    .is_some_and(|initializer| {
                        ast::is_function_like(&self.store, Some(initializer))
                    })
    }

    fn bind_this_property_assignment(&mut self, node: &mut ast::Node) {
        if !node_is_in_js_file(&self.store, *node) {
            return;
        }
        let Some(this_container) = ast::get_this_container(
            &self.store,
            *node,
            false, /*includeArrowFunctions*/
            false, /*includeClassComputedPropertyName*/
        ) else {
            return;
        };
        let left_is_private_property = {
            let left = self.store.left(*node);
            left.is_some_and(|left| ast::is_property_access_expression(&self.store, left))
                && left
                    .and_then(|left| self.store.name(left))
                    .is_some_and(|name| ast::is_private_identifier(&self.store, name))
        };
        if left_is_private_property {
            return;
        }
        match self.store.kind(this_container) {
            ast::Kind::FunctionDeclaration | ast::Kind::FunctionExpression => {
                // !!! constructor functions
            }
            ast::Kind::Constructor
            | ast::Kind::PropertyDeclaration
            | ast::Kind::MethodDeclaration
            | ast::Kind::GetAccessor
            | ast::Kind::SetAccessor
            | ast::Kind::ClassStaticBlockDeclaration => {
                let containing_class = self
                    .store
                    .parent(this_container)
                    .expect("this container should have a parent");
                let containing_symbol = self
                    .symbol(containing_class)
                    .expect("this container parent should have a symbol");
                self.bind_this_property_to_symbol(
                    node,
                    containing_symbol,
                    ast::is_static(&self.store, this_container),
                );
            }
            ast::Kind::SourceFile => {}
            ast::Kind::ModuleDeclaration => {}
            _ => panic!(
                "Unhandled case in bindThisPropertyAssignment: {:?}",
                self.store.kind(this_container)
            ),
        }
    }

    fn get_this_class_symbol(&mut self) -> Option<ast::SymbolHandle> {
        let this_container = self.this_container_state.as_ref()?;
        match this_container.kind {
            ast::Kind::FunctionDeclaration | ast::Kind::FunctionExpression => None,
            ast::Kind::Constructor
            | ast::Kind::PropertyDeclaration
            | ast::Kind::MethodDeclaration
            | ast::Kind::GetAccessor
            | ast::Kind::SetAccessor
            | ast::Kind::ClassStaticBlockDeclaration => this_container.parent_symbol.clone(),
            _ => None,
        }
    }

    fn bind_this_property_to_symbol(
        &mut self,
        node: &mut ast::Node,
        symbol: ast::SymbolHandle,
        is_static: bool,
    ) {
        if ast::has_dynamic_name(&self.store, *node) {
            if is_static {
                self.declare_symbol_ex_in_exports(
                    symbol.clone(),
                    node,
                    ast::SYMBOL_FLAGS_PROPERTY,
                    ast::SYMBOL_FLAGS_NONE,
                    true, /*isReplaceableByMethod*/
                    true, /*isComputedName*/
                );
            } else {
                self.declare_symbol_ex_in_members(
                    symbol.clone(),
                    node,
                    ast::SYMBOL_FLAGS_PROPERTY,
                    ast::SYMBOL_FLAGS_NONE,
                    true, /*isReplaceableByMethod*/
                    true, /*isComputedName*/
                );
            }
            self.add_late_bound_assignment_declaration_to_symbol(node, symbol);
        } else {
            if is_static {
                self.declare_symbol_ex_in_exports(
                    symbol,
                    node,
                    ast::SYMBOL_FLAGS_PROPERTY | ast::SYMBOL_FLAGS_ASSIGNMENT,
                    ast::SYMBOL_FLAGS_NONE,
                    true,  /*isReplaceableByMethod*/
                    false, /*isComputedName*/
                );
            } else {
                self.declare_symbol_ex_in_members(
                    symbol,
                    node,
                    ast::SYMBOL_FLAGS_PROPERTY | ast::SYMBOL_FLAGS_ASSIGNMENT,
                    ast::SYMBOL_FLAGS_NONE,
                    true,  /*isReplaceableByMethod*/
                    false, /*isComputedName*/
                );
            }
        }
    }

    fn bind_enum_declaration(&mut self, node: &mut ast::Node) {
        if !ast::is_enum_declaration(&self.store, *node) {
            return;
        }
        let (symbol_flags, symbol_excludes) = if ast::is_enum_const(&self.store, *node) {
            (
                ast::SYMBOL_FLAGS_CONST_ENUM,
                ast::SYMBOL_FLAGS_CONST_ENUM_EXCLUDES,
            )
        } else {
            (
                ast::SYMBOL_FLAGS_REGULAR_ENUM,
                ast::SYMBOL_FLAGS_REGULAR_ENUM_EXCLUDES,
            )
        };
        self.bind_block_scoped_declaration(node, symbol_flags, symbol_excludes);
    }

    fn bind_variable_declaration_or_binding_element(&mut self, node: &mut ast::Node) {
        if !matches!(
            self.store.kind(*node),
            ast::Kind::VariableDeclaration | ast::Kind::BindingElement
        ) {
            return;
        }
        let name = self.store.name(*node);
        self.check_strict_mode_eval_or_arguments(node, name.as_ref());
        if name.is_none() || name.is_some_and(|name| ast::is_binding_pattern(&self.store, name)) {
            return;
        }
        if ast::is_variable_declaration_initialized_to_require(&self.store, *node) {
            self.declare_symbol_and_add_to_symbol_table(
                node,
                ast::SYMBOL_FLAGS_ALIAS,
                ast::SYMBOL_FLAGS_ALIAS_EXCLUDES,
            );
        } else if ast::is_block_or_catch_scoped(&self.store, *node) {
            self.bind_block_scoped_declaration(
                node,
                ast::SYMBOL_FLAGS_BLOCK_SCOPED_VARIABLE,
                ast::SYMBOL_FLAGS_BLOCK_SCOPED_VARIABLE_EXCLUDES,
            );
        } else if ast::is_part_of_parameter_declaration(&self.store, *node) {
            // It is safe to walk up parent chain to find whether the node is a destructuring parameter declaration
            // because its parent chain has already been set up, since parents are set before descending into children.
            //
            // If node is a binding element in parameter declaration, we need to use ParameterExcludes.
            // Using ParameterExcludes flag allows the compiler to report an error on duplicate identifiers in Parameter Declaration
            // For example:
            //      function foo([a,a]) {} // Duplicate Identifier error
            //      function bar(a,a) {}   // Duplicate Identifier error, parameter declaration in this case is handled in bindParameter
            //                             // which correctly set excluded symbols
            self.declare_symbol_and_add_to_symbol_table(
                node,
                ast::SYMBOL_FLAGS_FUNCTION_SCOPED_VARIABLE,
                ast::SYMBOL_FLAGS_PARAMETER_EXCLUDES,
            );
        } else {
            self.declare_symbol_and_add_to_symbol_table(
                node,
                ast::SYMBOL_FLAGS_FUNCTION_SCOPED_VARIABLE,
                ast::SYMBOL_FLAGS_FUNCTION_SCOPED_VARIABLE_EXCLUDES,
            );
        }
    }

    fn bind_parameter(&mut self, node: &mut ast::Node) {
        if !ast::is_parameter_declaration(&self.store, *node) {
            return;
        }
        let parameter_name = self.store.name(*node);
        if !self.flags(*node).intersects(ast::NodeFlags::Ambient) {
            // It is a SyntaxError if the identifier eval or arguments appears within a FormalParameterList of a
            // strict mode FunctionLikeDeclaration or FunctionExpression(13.1)
            self.check_strict_mode_eval_or_arguments(node, parameter_name.as_ref());
        }
        if parameter_name.is_some_and(|name| ast::is_binding_pattern(&self.store, name)) {
            let parent = self
                .store
                .parent(*node)
                .expect("parameter should have a parent");
            let index = self
                .store
                .parameters(parent)
                .into_iter()
                .flatten()
                .position(|parameter| parameter == *node)
                .expect("parameter should be in parent parameter list");
            self.bind_anonymous_declaration(
                node,
                ast::SYMBOL_FLAGS_FUNCTION_SCOPED_VARIABLE,
                format!("__{}", index),
            );
        } else {
            self.declare_symbol_and_add_to_symbol_table(
                node,
                ast::SYMBOL_FLAGS_FUNCTION_SCOPED_VARIABLE,
                ast::SYMBOL_FLAGS_PARAMETER_EXCLUDES,
            );
        }
        // If this is a property-parameter, then also declare the property symbol into the
        // containing class.
        let parent = self
            .store
            .parent(*node)
            .expect("parameter should have a parent");
        if ast::is_parameter_property_declaration(&self.store, *node, parent) {
            let flags = ast::SYMBOL_FLAGS_PROPERTY
                | core::if_else(
                    self.store.question_token(*node).is_some(),
                    ast::SYMBOL_FLAGS_OPTIONAL,
                    ast::SYMBOL_FLAGS_NONE,
                );
            let class_symbol = {
                let class_declaration = self.store.parent(parent).unwrap();
                self.symbol(class_declaration).unwrap()
            };
            self.declare_symbol_in_members(
                class_symbol,
                node,
                flags,
                ast::SYMBOL_FLAGS_PROPERTY_EXCLUDES,
            );
        }
    }

    fn bind_function_declaration(&mut self, node: &mut ast::Node) {
        if !ast::is_function_declaration(&self.store, *node) {
            return;
        }
        if !self.file_is_declaration_file
            && !self.flags(*node).intersects(ast::NodeFlags::Ambient)
            && ast::is_async_function(&self.store, *node)
        {
            self.emit_flags |= ast::NodeFlags::HasAsyncFunctions;
        }
        self.check_strict_mode_function_name(node);
        self.bind_block_scoped_declaration(
            node,
            ast::SYMBOL_FLAGS_FUNCTION,
            ast::SYMBOL_FLAGS_FUNCTION_EXCLUDES,
        );
    }

    fn bind_anonymous_declaration(
        &mut self,
        node: &mut ast::Node,
        symbol_flags: ast::SymbolFlags,
        name: String,
    ) {
        let symbol = self.new_symbol(symbol_flags, name);
        if symbol_flags.intersects(ast::SYMBOL_FLAGS_ENUM_MEMBER | ast::SYMBOL_FLAGS_CLASS_MEMBER) {
            let parent = self
                .container_state
                .as_ref()
                .and_then(|container| container.symbol);
            self.set_symbol_parent(symbol, parent);
        }
        self.add_declaration_to_symbol(symbol, node, symbol_flags);
    }

    fn bind_anonymous_source_file_declaration(
        &mut self,
        symbol_flags: ast::SymbolFlags,
        name: String,
    ) {
        let symbol = self.new_symbol(symbol_flags, name);
        self.file_symbol = Some(symbol);
        let mut node = self.file;
        self.add_declaration_to_symbol(symbol, &mut node, symbol_flags);
    }
}

impl<'a> Binder<'a> {
    fn bind_block_scoped_declaration(
        &mut self,
        node: &mut ast::Node,
        symbol_flags: ast::SymbolFlags,
        symbol_excludes: ast::SymbolFlags,
    ) {
        let container = self
            .block_scope_container_state
            .clone()
            .expect("block scope container should be active");
        match container.kind {
            ast::Kind::ModuleDeclaration => {
                self.declare_module_member(node, symbol_flags, symbol_excludes);
            }
            ast::Kind::SourceFile => {
                // PORT NOTE: reshaped for borrowck. The current container is the
                // mutable SourceFile shell; use the active SourceFile payload for
                // source-file queries.
                if self.file_is_external_or_common_js_module {
                    self.declare_module_member(node, symbol_flags, symbol_excludes);
                    return;
                }
                self.declare_symbol_in_locals(
                    container.locals_key,
                    node,
                    symbol_flags,
                    symbol_excludes,
                );
            }
            _ => {
                self.declare_symbol_in_locals(
                    container.locals_key,
                    node,
                    symbol_flags,
                    symbol_excludes,
                );
            }
        }
    }

    fn bind_type_parameter(&mut self, node: &mut ast::Node) {
        if !ast::is_type_parameter_declaration(&self.store, *node) {
            return;
        }
        if self
            .store
            .parent(*node)
            .is_some_and(|parent| self.store.kind(parent) == ast::Kind::InferType)
        {
            let infer_type = self.store.parent(*node).unwrap();
            let container = ast::find_ancestor(&self.store, Some(infer_type), |store, node| {
                store.parent(node).is_some_and(|parent| {
                    ast::is_conditional_type_node(store, parent)
                        && store
                            .extends_type(parent)
                            .is_some_and(|extends_type| extends_type == node)
                })
            })
            .and_then(|extends_type| self.store.parent(extends_type));
            if let Some(container) = container {
                self.declare_symbol(
                    BindingSymbolTable::Locals(container),
                    None, /*parent*/
                    node,
                    ast::SYMBOL_FLAGS_TYPE_PARAMETER,
                    ast::SYMBOL_FLAGS_TYPE_PARAMETER_EXCLUDES,
                );
            } else {
                self.bind_anonymous_declaration(
                    node,
                    ast::SYMBOL_FLAGS_TYPE_PARAMETER,
                    self.get_declaration_name(node),
                );
            }
        } else {
            self.declare_symbol_and_add_to_symbol_table(
                node,
                ast::SYMBOL_FLAGS_TYPE_PARAMETER,
                ast::SYMBOL_FLAGS_TYPE_PARAMETER_EXCLUDES,
            );
        }
    }

    fn lookup_entity(
        &mut self,
        node: &ast::Node,
        container: &BinderContainer,
    ) -> Option<ast::SymbolHandle> {
        if ast::is_identifier(&self.store, *node) {
            return self.lookup_name(&self.store.text(*node), container);
        }
        let expression = self.store.expression(*node)?;
        if self.store.kind(expression) == ast::Kind::ThisKeyword {
            if let Some(class_symbol) = self.get_this_class_symbol() {
                if let Some(name) = ast::get_element_or_property_access_name(&self.store, *node) {
                    let is_static = self.this_container.is_some_and(|_| {
                        self.this_container_state
                            .as_ref()
                            .is_some_and(|container| container.is_static)
                    });
                    let name_text = self.store.text(name);
                    return if is_static {
                        self.symbol_export(class_symbol, name_text.as_str())
                    } else {
                        self.symbol_member(class_symbol, name_text.as_str())
                    };
                }
            }
            return None;
        }
        let initializer_symbol = self.lookup_entity(&expression, container);
        if let Some(symbol) = self.get_initializer_symbol(initializer_symbol) {
            if !self.symbol_exports_is_empty(symbol) {
                if let Some(name) = ast::get_element_or_property_access_name(&self.store, *node) {
                    let name_text = self.store.text(name);
                    return self.symbol_export(symbol, name_text.as_str());
                }
            }
        }
        None
    }

    fn lookup_symbol_for_property_access(
        &mut self,
        node: ast::Node,
        lookup_container: Option<&BinderContainer>,
    ) -> Option<ast::SymbolHandle> {
        if ast::is_identifier(&self.store, node) {
            let container = lookup_container
                .cloned()
                .or_else(|| self.container_state.clone())?;
            return self.lookup_name(&self.store.text(node), &container);
        }
        let symbol = self.lookup_symbol_for_property_access(self.store.expression(node)?, None)?;
        let name = ast::get_element_or_property_access_name(&self.store, node)?;
        self.symbol_export(symbol, &self.store.text(name))
    }

    fn lookup_name(
        &mut self,
        name: &str,
        container: &BinderContainer,
    ) -> Option<ast::SymbolHandle> {
        if let Some(local) = self.binding_state.lookup_local(container.locals_key, name) {
            return self
                .binding_state
                .symbol_export_symbol(local)
                .or(Some(local));
        }
        if container.is_source_file {
            if let Some(symbol) = self.file_js_global_augmentations.get(name).copied() {
                return Some(symbol);
            }
        }
        if let Some(symbol) = container.symbol {
            return self.symbol_export(symbol, name);
        }
        None
    }

    // The binder visits every node in the syntax tree so it is a convenient place to perform a single localized
    // check for reserved words used as identifiers in strict mode code, as well as `yield` or `await` in
    // [Yield] or [Await] contexts, respectively.
    fn check_contextual_identifier(&mut self, node: &mut ast::Node) {
        // Report error only if there are no parse errors in file
        if self.file_diagnostics_empty
            && !self.flags(*node).intersects(ast::NodeFlags::Ambient)
            && !ast::is_identifier_name(&self.store, *node)
        {
            // strict mode identifiers
            let original_keyword_kind = scanner::get_identifier_token(&self.store.text(*node));
            if original_keyword_kind == ast::Kind::Identifier {
                return;
            }
            if original_keyword_kind >= ast::Kind::FirstFutureReservedWord
                && original_keyword_kind <= ast::Kind::LastFutureReservedWord
            {
                self.error_on_node(
                    node,
                    self.get_strict_mode_identifier_message(node),
                    &[self.declaration_name_to_string(node)],
                );
            } else if original_keyword_kind == ast::Kind::AwaitKeyword {
                if self.file_has_external_module_indicator
                    && ast::is_in_top_level_context(&self.store, *node)
                {
                    self.error_on_node(
                        node,
                        &diagnostics::IDENTIFIER_EXPECTED_0_IS_A_RESERVED_WORD_AT_THE_TOP_LEVEL_OF_A_MODULE,
                        &[self.declaration_name_to_string(node)],
                    );
                } else if self
                    .store
                    .flags(*node)
                    .intersects(ast::NodeFlags::AwaitContext)
                {
                    self.error_on_node(
                        node,
                        &diagnostics::IDENTIFIER_EXPECTED_0_IS_A_RESERVED_WORD_THAT_CANNOT_BE_USED_HERE,
                        &[self.declaration_name_to_string(node)],
                    );
                }
            } else if original_keyword_kind == ast::Kind::YieldKeyword
                && self
                    .store
                    .flags(*node)
                    .intersects(ast::NodeFlags::YieldContext)
            {
                self.error_on_node(
                    node,
                    &diagnostics::IDENTIFIER_EXPECTED_0_IS_A_RESERVED_WORD_THAT_CANNOT_BE_USED_HERE,
                    &[self.declaration_name_to_string(node)],
                );
            }
        }
    }

    fn check_private_identifier(&mut self, node: &mut ast::Node) {
        if self.store.text(*node) == "#constructor" {
            // Report error only if there are no parse errors in file
            if self.file_diagnostics_empty {
                self.error_on_node(
                    node,
                    &diagnostics::X_CONSTRUCTOR_IS_A_RESERVED_WORD,
                    &[self.declaration_name_to_string(node)],
                );
            }
        }
    }

    fn get_strict_mode_identifier_message(
        &self,
        node: &ast::Node,
    ) -> &'static diagnostics::Message {
        // Provide specialized messages to help the user understand why we think they're in
        // strict mode.
        if ast::get_containing_class(&self.store, *node).is_some() {
            return &diagnostics::IDENTIFIER_EXPECTED_0_IS_A_RESERVED_WORD_IN_STRICT_MODE_CLASS_DEFINITIONS_ARE_AUTOMATICALLY_IN_STRICT_MODE;
        }
        if self.file_has_external_module_indicator {
            return &diagnostics::IDENTIFIER_EXPECTED_0_IS_A_RESERVED_WORD_IN_STRICT_MODE_MODULES_ARE_AUTOMATICALLY_IN_STRICT_MODE;
        }
        &diagnostics::IDENTIFIER_EXPECTED_0_IS_A_RESERVED_WORD_IN_STRICT_MODE
    }
}

// Should be called only on prologue directives (ast.IsPrologueDirective(node) should be true)
fn is_use_strict_prologue_directive(
    source_file: &impl ast::SourceFileStoreLike,
    node: ast::Node,
) -> bool {
    let expression = source_file.store().expression(node).unwrap();
    let node_text = scanner::get_source_text_of_node_from_source_file(
        source_file,
        &expression,
        false, /*includeTrivia*/
    );
    // Note: the node text must be exactly "use strict" or 'use strict'.  It is not ok for the
    // string to contain unicode escapes (as per ES5).
    node_text == "\"use strict\"" || node_text == "'use strict'"
}

pub fn find_use_strict_prologue(
    source_file: &impl ast::SourceFileStoreLike,
    statements: &[ast::Node],
) -> Option<ast::Node> {
    for statement in statements.iter().copied() {
        if ast::is_prologue_directive(source_file.store(), statement) {
            if is_use_strict_prologue_directive(source_file, statement) {
                return Some(statement);
            }
        } else {
            return None;
        }
    }

    None
}

impl<'a> Binder<'a> {
    fn check_strict_mode_function_name(&mut self, node: &mut ast::Node) {
        if !self.flags(*node).intersects(ast::NodeFlags::Ambient) {
            // It is a SyntaxError if the identifier eval or arguments appears within a FormalParameterList of a strict mode FunctionDeclaration or FunctionExpression (13.1))
            let name = self.store.name(*node);
            self.check_strict_mode_eval_or_arguments(node, name.as_ref());
        }
    }

    fn get_strict_mode_block_scope_function_declaration_message(
        &self,
        node: &ast::Node,
    ) -> &'static diagnostics::Message {
        // Provide specialized messages to help the user understand why we think they're in strict mode.
        if ast::get_containing_class(&self.store, *node).is_some() {
            return &diagnostics::FUNCTION_DECLARATIONS_ARE_NOT_ALLOWED_INSIDE_BLOCKS_IN_STRICT_MODE_WHEN_TARGETING_ES5_CLASS_DEFINITIONS_ARE_AUTOMATICALLY_IN_STRICT_MODE;
        }
        if self.file_has_external_module_indicator {
            return &diagnostics::FUNCTION_DECLARATIONS_ARE_NOT_ALLOWED_INSIDE_BLOCKS_IN_STRICT_MODE_WHEN_TARGETING_ES5_MODULES_ARE_AUTOMATICALLY_IN_STRICT_MODE;
        }
        &diagnostics::FUNCTION_DECLARATIONS_ARE_NOT_ALLOWED_INSIDE_BLOCKS_IN_STRICT_MODE_WHEN_TARGETING_ES5
    }

    fn check_strict_mode_binary_expression(&mut self, node: &mut ast::Node) {
        let left = self.store.left(*node);
        let operator_token = self.store.operator_token(*node);
        if left
            .as_ref()
            .is_some_and(|left| ast::is_left_hand_side_expression(&self.store, *left))
            && operator_token
                .as_ref()
                .is_some_and(|token| ast::is_assignment_operator(self.store.kind(*token)))
        {
            // ECMA 262 (Annex C) The identifier eval or arguments may not appear as the LeftHandSideExpression of an
            // Assignment operator(11.13) or of a PostfixExpression(11.3)
            self.check_strict_mode_eval_or_arguments(node, left.as_ref());
        }
    }

    fn check_strict_mode_catch_clause(&mut self, node: &mut ast::Node) {
        // It is a SyntaxError if a TryStatement with a Catch occurs within strict code and the Identifier of the
        // Catch production is eval or arguments
        if let Some(variable_declaration) = self.store.variable_declaration(*node) {
            let name = self.store.name(variable_declaration);
            self.check_strict_mode_eval_or_arguments(node, name.as_ref());
        }
    }

    fn check_strict_mode_delete_expression(&mut self, node: &mut ast::Node) {
        // Grammar checking
        let expression = self.store.expression(*node);
        if expression
            .as_ref()
            .is_some_and(|expression| self.store.kind(*expression) == ast::Kind::Identifier)
        {
            // When a delete operator occurs within strict mode code, a SyntaxError is thrown if its
            // UnaryExpression is a direct reference to a variable, function argument, or function name
            if let Some(expression) = expression.as_ref() {
                self.error_on_node(
                    expression,
                    &diagnostics::X_DELETE_CANNOT_BE_CALLED_ON_AN_IDENTIFIER_IN_STRICT_MODE,
                    &[],
                );
            }
        }
    }

    fn check_strict_mode_postfix_unary_expression(&mut self, node: &mut ast::Node) {
        // Grammar checking
        // The identifier eval or arguments may not appear as the LeftHandSideExpression of an
        // Assignment operator(11.13) or of a PostfixExpression(11.3) or as the UnaryExpression
        // operated upon by a Prefix Increment(11.4.4) or a Prefix Decrement(11.4.5) operator.
        let operand = self.store.operand(*node);
        self.check_strict_mode_eval_or_arguments(node, operand.as_ref());
    }

    fn check_strict_mode_prefix_unary_expression(&mut self, node: &mut ast::Node) {
        // Grammar checking
        let operator = self.store.operator(*node).unwrap();
        let operand = self.store.operand(*node);
        if operator == ast::Kind::PlusPlusToken || operator == ast::Kind::MinusMinusToken {
            self.check_strict_mode_eval_or_arguments(node, operand.as_ref());
        }
    }

    fn check_strict_mode_with_statement(&mut self, node: &mut ast::Node) {
        // Grammar checking for withStatement
        self.error_on_first_token(
            node,
            &diagnostics::X_WITH_STATEMENTS_ARE_NOT_ALLOWED_IN_STRICT_MODE,
            &[],
        );
    }

    fn check_strict_mode_labeled_statement(&mut self, node: &mut ast::Node) {
        // Grammar checking for labeledStatement
        let statement = self.store.statement(*node);
        let label = self.store.label(*node);
        if statement
            .as_ref()
            .is_some_and(|statement| ast::is_declaration_statement(self.store, *statement))
            || statement
                .as_ref()
                .is_some_and(|statement| ast::is_variable_statement(&self.store, *statement))
        {
            if let Some(label) = label.as_ref() {
                self.error_on_first_token(label, &diagnostics::A_LABEL_IS_NOT_ALLOWED_HERE, &[]);
            }
        }
    }
}

fn is_eval_or_arguments_identifier(node: &ast::Node) -> bool {
    // The caller immediately checks text through its active store.
    let _ = node;
    true
}

impl<'a> Binder<'a> {
    fn check_strict_mode_eval_or_arguments(
        &mut self,
        context_node: &ast::Node,
        name: Option<&ast::Node>,
    ) {
        if let Some(name) = name {
            let name_text = self.store.text(*name);
            if ast::is_identifier(&self.store, *name)
                && (name_text == "eval" || name_text == "arguments")
            {
                // We check first if the name is inside class declaration or class expression; if so give explicit message
                // otherwise report generic error message.
                self.error_on_node(
                    name,
                    self.get_strict_mode_eval_or_arguments_message(context_node),
                    &[name_text],
                );
            }
        }
    }

    fn get_strict_mode_eval_or_arguments_message(
        &self,
        node: &ast::Node,
    ) -> &'static diagnostics::Message {
        // Provide specialized messages to help the user understand why we think they're in strict mode
        if ast::get_containing_class(&self.store, *node).is_some() {
            return &diagnostics::CODE_CONTAINED_IN_A_CLASS_IS_EVALUATED_IN_JAVASCRIPT_S_STRICT_MODE_WHICH_DOES_NOT_ALLOW_THIS_USE_OF_0_FOR_MORE_INFORMATION_SEE_HTTPS_COLON_SLASH_SLASHDEVELOPER_MOZILLA_ORG_SLASHEN_US_SLASHDOCS_SLASHWEB_SLASHJAVASCRIPT_SLASHREFERENCE_SLASHSTRICT_MODE;
        }
        if self.file_has_external_module_indicator {
            return &diagnostics::INVALID_USE_OF_0_MODULES_ARE_AUTOMATICALLY_IN_STRICT_MODE;
        }
        &diagnostics::INVALID_USE_OF_0_IN_STRICT_MODE
    }
}

impl<'a> Binder<'a> {
    // All container nodes are kept on a linked list in declaration order. This list is used by
    // the getLocalNameOfContainer function in the type checker to validate that the local name
    // used for a container is unique.
    fn bind_container(&mut self, node: &mut ast::Node, container_flags: ContainerFlags) {
        // Before we recurse into a node's children, we first save the existing parent, container
        // and block-container.  Then after we pop out of processing the children, we restore
        // these saved values.
        let save_container = self.container;
        let save_this_container = self.this_container;
        let saved_block_scope_container = self.block_scope_container;
        let save_container_state = self.container_state.clone();
        let save_this_container_state = self.this_container_state.clone();
        let saved_block_scope_container_state = self.block_scope_container_state.clone();
        // Depending on what kind of node this is, we may have to adjust the current container
        // and block-container.   If the current node is a container, then it is automatically
        // considered the current block-container as well.  Also, for containers that we know
        // may contain locals, we eagerly initialize the .locals field. We do this because
        // it's highly likely that the .locals will be needed to place some child in (for example,
        // a parameter, or variable declaration).
        //
        // However, we do not proactively create the .locals for block-containers because it's
        // totally normal and common for block-containers to never actually have a block-scoped
        // variable in them.  We don't want to end up allocating an object for every 'block' we
        // run into when most of them won't be necessary.
        //
        // Finally, if this is a block-container, then we clear out any existing .locals object
        // it may contain within it.  This happens in incremental scenarios.  Because we can be
        // reusing a node from a previous compilation, that node may have had 'locals' created
        // for it.  We must clear this so we don't accidentally move any stale data forward from
        // a previous compilation.
        let node_kind = self.store.kind(*node);
        if container_flags & CONTAINER_FLAGS_IS_CONTAINER != 0 {
            self.container = Some(*node);
            self.block_scope_container = Some(*node);
            let state = if node_kind == ast::Kind::SourceFile {
                let locals_key = *node;
                self.container_from_source_file(locals_key)
            } else {
                self.container_from_node(node)
            };
            self.container_state = Some(state.clone());
            self.block_scope_container_state = Some(state);
            if container_flags & CONTAINER_FLAGS_HAS_LOCALS != 0 {
                // localsContainer := node
                // localsContainer.LocalsContainerData().locals = make(SymbolTable)
                self.add_to_container_chain(node);
            }
        } else if container_flags & CONTAINER_FLAGS_IS_BLOCK_SCOPED_CONTAINER != 0 {
            self.block_scope_container = Some(*node);
            self.block_scope_container_state = Some(self.container_from_node(node));
            self.add_to_container_chain(node);
        }
        if container_flags & CONTAINER_FLAGS_IS_THIS_CONTAINER != 0 {
            self.this_container = Some(*node);
            self.this_container_state = Some(self.container_from_node(node));
        }
        if container_flags & CONTAINER_FLAGS_IS_CONTROL_FLOW_CONTAINER != 0 {
            let save_current_flow = self.current_flow;
            let save_break_target = self.current_break_target.take();
            let save_continue_target = self.current_continue_target.take();
            let save_return_target = self.current_return_target.take();
            let save_exception_target = self.current_exception_target.take();
            let save_active_label_list = self.active_label_list.take();
            let save_has_explicit_return = self.has_explicit_return;
            let save_seen_this_keyword = self.seen_this_keyword;
            let is_immediately_invoked = (container_flags & CONTAINER_FLAGS_IS_FUNCTION_EXPRESSION
                != 0
                && !ast::has_syntactic_modifier(&self.store, *node, ast::ModifierFlags::Async)
                && !is_generator_function_expression(&self.store, node)
                && ast::get_immediately_invoked_function_expression(&self.store, *node).is_some())
                || node_kind == ast::Kind::ClassStaticBlockDeclaration;
            // A non-async, non-generator IIFE is considered part of the containing control flow. Return statements behave
            // similarly to break statements that exit to a label just past the statement body.
            if !is_immediately_invoked {
                let flow_start = self.new_flow_node(ast::FlowFlags::Start);
                self.current_flow = Some(flow_start);
                if container_flags
                    & (CONTAINER_FLAGS_IS_FUNCTION_EXPRESSION
                        | CONTAINER_FLAGS_IS_OBJECT_LITERAL_OR_CLASS_EXPRESSION_METHOD_OR_ACCESSOR)
                    != 0
                {
                    self.flow_graph.node_mut(flow_start).node = Some((*node).into());
                }
            }
            // We create a return control flow graph for IIFEs and constructors. For constructors
            // we use the return control flow graph in strict property initialization checks.
            if is_immediately_invoked || node_kind == ast::Kind::Constructor {
                let return_target = self.new_flow_node(ast::FlowFlags::BranchLabel);
                self.current_return_target = Some(return_target);
            } else {
                self.current_return_target = None;
            }
            self.current_exception_target = None;
            self.current_break_target = None;
            self.current_continue_target = None;
            self.active_label_list = None;
            self.has_explicit_return = false;
            self.seen_this_keyword = false;
            self.bind_children(node);
            // Reset flags (for incremental scenarios)
            self.reset_reachability_flags(*node);
            let current_flow = self.current_flow.unwrap();
            if !self
                .flow_graph
                .node(current_flow)
                .flags
                .intersects(ast::FlowFlags::Unreachable)
                && container_flags & CONTAINER_FLAGS_IS_FUNCTION_LIKE != 0
            {
                let body_is_present = self
                    .store
                    .body(*node)
                    .is_some_and(|body| ast::node_is_present(&self.store, Some(body)));
                if body_is_present {
                    self.mark_implicit_return(*node);
                    if self.has_explicit_return {
                        self.mark_explicit_return(*node);
                    }
                    self.binding_state
                        .set_body_end_flow_node(*node, Some(current_flow));
                }
            }
            if self.seen_this_keyword {
                self.mark_contains_this(*node);
            }
            if node_kind == ast::Kind::SourceFile {
                self.file_flags |= self.emit_flags;
                self.file_end_flow_node = self.current_flow;
            }
            if let Some(current_return_target) = self.current_return_target {
                self.add_antecedent(current_return_target, self.current_flow.unwrap());
                self.current_flow = Some(self.finish_flow_label(current_return_target));
                if node_kind == ast::Kind::Constructor
                    || node_kind == ast::Kind::ClassStaticBlockDeclaration
                {
                    self.set_return_flow_node(*node, self.current_flow);
                }
            }
            if !is_immediately_invoked {
                self.current_flow = save_current_flow;
            }
            self.current_break_target = save_break_target;
            self.current_continue_target = save_continue_target;
            self.current_return_target = save_return_target;
            self.current_exception_target = save_exception_target;
            self.active_label_list = save_active_label_list;
            self.has_explicit_return = save_has_explicit_return;
            if container_flags & CONTAINER_FLAGS_PROPAGATES_THIS_KEYWORD != 0 {
                self.seen_this_keyword = save_seen_this_keyword || self.seen_this_keyword;
            } else {
                self.seen_this_keyword = save_seen_this_keyword;
            }
        } else if container_flags & CONTAINER_FLAGS_IS_INTERFACE != 0 {
            let save_seen_this_keyword = self.seen_this_keyword;
            self.seen_this_keyword = false;
            self.bind_children(node);
            // ContainsThis cannot overlap with HasExtendedUnicodeEscape on Identifier
            if self.seen_this_keyword {
                self.mark_contains_this(*node);
            } else {
                self.set_contains_this(*node, false);
            }
            self.seen_this_keyword = save_seen_this_keyword;
        } else {
            self.bind_children(node);
        }
        if ast::is_source_file(&self.store, *node) && node_is_in_js_file(&self.store, *node) {
            // Binding of top-level JSTypeAliasDeclaration nodes is deferred to ensure CommonJS module
            // indicators, if any, are processed first.
            let statements = self
                .store
                .source_statements(self.file)
                .expect("source file should have statements");
            for mut statement in statements
                .iter()
                .filter(|statement| ast::is_js_type_alias_declaration(self.store, *statement))
            {
                self.bind_block_scoped_declaration(
                    &mut statement,
                    ast::SYMBOL_FLAGS_TYPE_ALIAS,
                    ast::SYMBOL_FLAGS_TYPE_ALIAS_EXCLUDES,
                );
            }
            if self.file_has_common_js_module_indicator {
                self.declare_common_js_variable("module");
                self.declare_common_js_variable("exports");
            }
        }
        let is_external_source_file =
            ast::is_source_file(&self.store, *node) && self.file_is_external_or_common_js_module;
        if is_external_source_file || ast::is_ambient_module(&self.store, *node) {
            if is_external_source_file {
                let symbol = self
                    .file_symbol
                    .clone()
                    .expect("external source file should have a symbol");
                self.bind_common_js_type_exports(symbol);
            } else {
                let symbol = self
                    .binding_state
                    .symbol(*node)
                    .expect("ambient module should have symbol");
                self.bind_common_js_type_exports(symbol);
            }
        }
        self.container = save_container;
        self.this_container = save_this_container;
        self.block_scope_container = saved_block_scope_container;
        self.container_state = save_container_state;
        self.this_container_state = save_this_container_state;
        self.block_scope_container_state = saved_block_scope_container_state;
    }

    fn declare_common_js_variable(&mut self, name: &str) {
        let file_node = self.file;
        let locals_key = file_node;
        let has_local = self.binding_state.lookup_local(locals_key, name).is_some();
        if !has_local {
            let declaration = file_node.clone();
            let symbol = self.new_symbol(
                ast::SYMBOL_FLAGS_FUNCTION_SCOPED_VARIABLE | ast::SYMBOL_FLAGS_MODULE_EXPORTS,
                name.to_owned(),
            );
            self.binding_state
                .set_symbol_declarations(symbol, vec![declaration]);
            self.set_symbol_value_declaration(symbol, Some(declaration));
            if name == "module" {
                let exports_property = self.new_symbol(
                    ast::SYMBOL_FLAGS_MODULE_EXPORTS | ast::SYMBOL_FLAGS_PROPERTY,
                    "exports".to_owned(),
                );
                let declarations = self.collect_symbol_declarations(symbol);
                let value_declaration = self.symbol_value_declaration(symbol);
                self.binding_state
                    .set_symbol_declarations(exports_property, declarations);
                self.set_symbol_value_declaration(exports_property, value_declaration);
                self.set_symbol_parent(exports_property, Some(symbol));
                self.binding_state
                    .insert_symbol_member(symbol, "exports", exports_property);
            }
            self.binding_state
                .locals_mut(locals_key)
                .insert(name.into(), symbol);
        }
    }

    fn bind_children(&mut self, node: &mut ast::Node) {
        let save_in_assignment_pattern = self.in_assignment_pattern;
        // Most nodes aren't valid in an assignment pattern, so we clear the value here
        // and set it before we descend into nodes that could actually be part of an assignment pattern.
        self.in_assignment_pattern = false;

        if same_flow_node(self.current_flow, self.unreachable_flow) {
            self.set_node_flow_node(*node, None);
            if ast::is_potentially_executable_node(&self.store, *node) {
                self.mark_unreachable(*node);
            }
            self.bind_each_child(node);
            self.in_assignment_pattern = save_in_assignment_pattern;
            return;
        }

        let node_kind = self.store.kind(*node);
        if ast::Kind::FirstStatement <= node_kind && node_kind <= ast::Kind::LastStatement {
            self.set_node_flow_node(*node, self.current_flow);
        }

        match node_kind {
            ast::Kind::WhileStatement => self.bind_while_statement(node),
            ast::Kind::DoStatement => self.bind_do_statement(node),
            ast::Kind::ForStatement => self.bind_for_statement(node),
            ast::Kind::ForInStatement | ast::Kind::ForOfStatement => {
                self.bind_for_in_or_for_of_statement(node)
            }
            ast::Kind::IfStatement => self.bind_if_statement(node),
            ast::Kind::ReturnStatement => self.bind_return_statement(node),
            ast::Kind::ThrowStatement => self.bind_throw_statement(node),
            ast::Kind::BreakStatement => self.bind_break_statement(node),
            ast::Kind::ContinueStatement => self.bind_continue_statement(node),
            ast::Kind::TryStatement => self.bind_try_statement(node),
            ast::Kind::SwitchStatement => self.bind_switch_statement(node),
            ast::Kind::CaseBlock => self.bind_case_block(node),
            ast::Kind::CaseClause | ast::Kind::DefaultClause => {
                self.bind_case_or_default_clause(node)
            }
            ast::Kind::ExpressionStatement => self.bind_expression_statement(node),
            ast::Kind::LabeledStatement => self.bind_labeled_statement(node),
            ast::Kind::PrefixUnaryExpression => self.bind_prefix_unary_expression_flow(node),
            ast::Kind::PostfixUnaryExpression => self.bind_postfix_unary_expression_flow(node),
            ast::Kind::BinaryExpression => {
                if ast::is_destructuring_assignment(&self.store, *node) {
                    // Carry over whether we are in an assignment pattern to
                    // binary expressions that could actually be an initializer
                    self.in_assignment_pattern = save_in_assignment_pattern;
                    self.bind_destructuring_assignment_flow(node);
                    return;
                }
                self.bind_binary_expression_flow(node);
            }
            ast::Kind::DeleteExpression => self.bind_delete_expression_flow(node),
            ast::Kind::ConditionalExpression => self.bind_conditional_expression_flow(node),
            ast::Kind::VariableDeclaration => self.bind_variable_declaration_flow(node),
            ast::Kind::PropertyAccessExpression | ast::Kind::ElementAccessExpression => {
                self.bind_access_expression_flow(node)
            }
            ast::Kind::CallExpression => self.bind_call_expression_flow(node),
            ast::Kind::NonNullExpression => self.bind_non_null_expression_flow(node),
            ast::Kind::SourceFile => {
                let file_data = self.store.as_source_file(*node);
                let end_of_file_token = file_data.end_of_file_token();
                let statements = self
                    .store
                    .source_statements(*node)
                    .expect("source file should have statements");
                self.bind_statement_list_functions_first(&statements);
                if let Some(mut end_of_file_token) = end_of_file_token {
                    self.bind(Some(&mut end_of_file_token));
                }
            }
            ast::Kind::Block | ast::Kind::ModuleBlock => {
                if let Some(statements) = ast::statement_container_statements(self.store, *node) {
                    self.bind_statement_list_functions_first(&statements);
                }
            }
            ast::Kind::ClassDeclaration => {
                let modifiers = self.store.source_modifiers(*node);
                let mut name = self.store.name(*node);
                let type_parameters = self.store.source_type_parameters(*node);
                let heritage_clauses = self.store.source_heritage_clauses(*node);
                let members = self.store.source_members(*node);
                self.bind_modifiers(modifiers);
                self.bind(name.as_mut());
                self.bind_each_optional(type_parameters);
                self.bind_each_optional(heritage_clauses);
                self.bind_each_optional(members);
            }
            ast::Kind::ClassExpression => {
                let modifiers = self.store.source_modifiers(*node);
                let mut name = self.store.name(*node);
                let type_parameters = self.store.source_type_parameters(*node);
                let heritage_clauses = self.store.source_heritage_clauses(*node);
                let members = self.store.source_members(*node);
                self.bind_modifiers(modifiers);
                self.bind(name.as_mut());
                self.bind_each_optional(type_parameters);
                self.bind_each_optional(heritage_clauses);
                self.bind_each_optional(members);
            }
            ast::Kind::InterfaceDeclaration => {
                let modifiers = self.store.source_modifiers(*node);
                let mut name = self.store.name(*node);
                let type_parameters = self.store.source_type_parameters(*node);
                let heritage_clauses = self.store.source_heritage_clauses(*node);
                let members = self.store.source_members(*node);
                self.bind_modifiers(modifiers);
                self.bind(name.as_mut());
                self.bind_each_optional(type_parameters);
                self.bind_each_optional(heritage_clauses);
                self.bind_each_optional(members);
            }
            ast::Kind::TypeLiteral => {
                let members = self.store.source_members(*node);
                self.bind_each_optional(members);
            }
            ast::Kind::BindingElement => self.bind_binding_element_flow(node),
            ast::Kind::Parameter => self.bind_parameter_flow(node),
            ast::Kind::ObjectLiteralExpression
            | ast::Kind::ArrayLiteralExpression
            | ast::Kind::PropertyAssignment
            | ast::Kind::SpreadElement => {
                self.in_assignment_pattern = save_in_assignment_pattern;
                self.bind_each_child(node);
            }
            _ => self.bind_each_child(node),
        }
        self.in_assignment_pattern = save_in_assignment_pattern;
    }

    fn bind_ref(&mut self, node: Option<&mut ast::Node>) -> bool {
        self.bind(node)
    }

    fn bind_each_child(&mut self, node: &mut ast::Node) {
        let store = self.store;
        let _ = store.for_each_present_child(*node, |mut child| {
            self.bind(Some(&mut child));
            std::ops::ControlFlow::Continue(())
        });
    }

    fn bind_each(&mut self, nodes: impl IntoIterator<Item = ast::Node>) {
        for mut node in nodes {
            self.bind(Some(&mut node));
        }
    }

    fn bind_each_optional<T>(&mut self, nodes: Option<T>)
    where
        T: IntoIterator<Item = ast::Node>,
    {
        if let Some(nodes) = nodes {
            self.bind_each(nodes);
        }
    }

    fn bind_modifiers(&mut self, modifiers: Option<ast::SourceModifierList<'_>>) {
        if let Some(modifiers) = modifiers {
            self.bind_each(modifiers);
        }
    }

    fn bind_statement_list_functions_first(&mut self, statements: &ast::SourceNodeList<'_>) {
        for mut node in statements
            .iter()
            .filter(|statement| ast::is_function_declaration(self.store, *statement))
        {
            self.bind(Some(&mut node));
        }
        for mut node in statements
            .iter()
            .filter(|statement| !ast::is_function_declaration(self.store, *statement))
        {
            self.bind(Some(&mut node));
        }
    }
}

fn same_flow_node(left: Option<ast::FlowRef>, right: Option<ast::FlowRef>) -> bool {
    left == right
}

fn node_is_in_js_file(store: &ast::AstStore, node: ast::Node) -> bool {
    store.flags(node).contains(ast::NodeFlags::JAVA_SCRIPT_FILE)
}

fn is_expando_initializer_in_store(
    store: &ast::AstStore,
    declaration: ast::Node,
    initializer: ast::Node,
) -> bool {
    if ast::is_function_expression_or_arrow_function(store, initializer) {
        return true;
    }
    node_is_in_js_file(store, initializer)
        && (ast::is_class_expression(store, initializer)
            || ast::is_object_literal_expression(store, initializer)
                && store
                    .properties(initializer)
                    .is_none_or(|properties| properties.is_empty())
                && store.type_node(declaration).is_none())
}

fn get_assigned_expando_initializer_in_store(
    store: &ast::AstStore,
    node: ast::Node,
) -> Option<ast::Node> {
    let parent = store.parent(node)?;
    if ast::is_binary_expression(store, parent)
        && store
            .operator_token(parent)
            .is_some_and(|operator| store.kind(operator) == ast::Kind::EqualsToken)
    {
        let left = store.left(parent)?;
        let right = store.right(parent)?;
        let is_prototype_assignment = ast::is_prototype_access(store, left);
        if is_expando_initializer_in_store(store, node, right)
            || is_prototype_assignment && ast::is_object_literal_expression(store, right)
        {
            return Some(right);
        }
    }
    None
}

impl<'a> Binder<'a> {
    fn set_continue_target(&mut self, node: &mut ast::Node, target: ast::FlowRef) -> ast::FlowRef {
        let mut current = *node;
        let mut label = self.active_label_list.as_deref_mut();
        while let Some(active_label) = label {
            let Some(parent) = self.store.parent(current) else {
                break;
            };
            if self.store.kind(parent) != ast::Kind::LabeledStatement {
                break;
            }
            active_label.continue_target = Some(target);
            label = active_label.next.as_deref_mut();
            current = parent;
        }
        target
    }

    fn do_with_conditional_branches(
        &mut self,
        action: fn(&mut Binder<'a>, Option<&mut ast::Node>) -> bool,
        value: Option<&mut ast::Node>,
        true_target: ast::FlowRef,
        false_target: ast::FlowRef,
    ) {
        let saved_true_target = self.current_true_target.take();
        let saved_false_target = self.current_false_target.take();
        self.current_true_target = Some(true_target);
        self.current_false_target = Some(false_target);
        action(self, value);
        self.current_true_target = saved_true_target;
        self.current_false_target = saved_false_target;
    }

    fn bind_condition(
        &mut self,
        node: Option<&mut ast::Node>,
        true_target: ast::FlowRef,
        false_target: ast::FlowRef,
    ) {
        match node {
            Some(node) => {
                self.do_with_conditional_branches(
                    Binder::bind_ref,
                    Some(&mut *node),
                    true_target,
                    false_target,
                );
                if !is_logical_assignment_expression(&self.store, node)
                    && !ast::is_logical_expression(&self.store, *node)
                    && !(ast::is_optional_chain(&self.store, *node)
                        && ast::is_outermost_optional_chain(&self.store, *node))
                {
                    let true_flow = self.create_flow_condition(
                        ast::FlowFlags::TrueCondition,
                        self.current_flow.unwrap(),
                        Some(&mut *node),
                    );
                    self.add_antecedent(true_target, true_flow);
                    let false_flow = self.create_flow_condition(
                        ast::FlowFlags::FalseCondition,
                        self.current_flow.unwrap(),
                        Some(node),
                    );
                    self.add_antecedent(false_target, false_flow);
                }
            }
            None => {
                let true_flow = self.create_flow_condition(
                    ast::FlowFlags::TrueCondition,
                    self.current_flow.unwrap(),
                    None,
                );
                self.add_antecedent(true_target, true_flow);
                let false_flow = self.create_flow_condition(
                    ast::FlowFlags::FalseCondition,
                    self.current_flow.unwrap(),
                    None,
                );
                self.add_antecedent(false_target, false_flow);
            }
        }
    }

    fn bind_iterative_statement(
        &mut self,
        node: &mut ast::Node,
        break_target: ast::FlowRef,
        continue_target: ast::FlowRef,
    ) {
        let save_break_target = self.current_break_target.take();
        let save_continue_target = self.current_continue_target.take();
        self.current_break_target = Some(break_target);
        self.current_continue_target = Some(continue_target);
        self.bind(Some(node));
        self.current_break_target = save_break_target;
        self.current_continue_target = save_continue_target;
    }
}

fn is_logical_assignment_expression(store: &ast::AstStore, node: &ast::Node) -> bool {
    ast::is_logical_or_coalescing_assignment_expression(store, ast::skip_parentheses(store, *node))
}

impl<'a> Binder<'a> {
    fn bind_assignment_target_flow(&mut self, node: &mut ast::Node) {
        match self.store.kind(*node) {
            ast::Kind::ArrayLiteralExpression => {
                let elements: Vec<ast::Node> =
                    self.store.elements(*node).into_iter().flatten().collect();
                for mut e in elements {
                    if self.store.kind(e) == ast::Kind::SpreadElement {
                        if let Some(mut expression) = self.store.expression(e) {
                            self.bind_assignment_target_flow(&mut expression);
                        }
                    } else {
                        self.bind_destructuring_target_flow(&mut e);
                    }
                }
            }
            ast::Kind::ObjectLiteralExpression => {
                let properties: Vec<ast::Node> =
                    self.store.properties(*node).into_iter().flatten().collect();
                for p in properties {
                    match self.store.kind(p) {
                        ast::Kind::PropertyAssignment => {
                            if let Some(mut initializer) = self.store.initializer(p) {
                                self.bind_destructuring_target_flow(&mut initializer);
                            }
                        }
                        ast::Kind::ShorthandPropertyAssignment => {
                            if let Some(mut name) = self.store.name(p) {
                                self.bind_assignment_target_flow(&mut name);
                            }
                        }
                        ast::Kind::SpreadAssignment => {
                            if let Some(mut expression) = self.store.expression(p) {
                                self.bind_assignment_target_flow(&mut expression);
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {
                if is_narrowable_reference(&self.store, node) {
                    self.current_flow = Some(self.create_flow_mutation(
                        ast::FlowFlags::Assignment,
                        self.current_flow.unwrap(),
                        node,
                    ));
                }
            }
        }
    }

    fn bind_destructuring_target_flow(&mut self, node: &mut ast::Node) {
        if ast::is_binary_expression(&self.store, *node)
            && self
                .store
                .operator_token(*node)
                .is_some_and(|token| self.store.kind(token) == ast::Kind::EqualsToken)
        {
            let mut left = self.store.left(*node).unwrap();
            self.bind_assignment_target_flow(&mut left);
        } else {
            self.bind_assignment_target_flow(node);
        }
    }

    fn bind_while_statement(&mut self, node: &mut ast::Node) {
        let loop_label = self.create_loop_label();
        let pre_while_label = self.set_continue_target(node, loop_label);
        let pre_body_label = self.create_branch_label();
        let post_while_label = self.create_branch_label();
        self.add_antecedent(pre_while_label, self.current_flow.unwrap());
        self.current_flow = Some(pre_while_label);
        let mut expression = self.store.expression(*node).unwrap();
        let mut statement = self.store.statement(*node).unwrap();
        self.bind_condition(Some(&mut expression), pre_body_label, post_while_label);
        self.current_flow = Some(self.finish_flow_label(pre_body_label));
        self.bind_iterative_statement(&mut statement, post_while_label, pre_while_label);
        self.add_antecedent(pre_while_label, self.current_flow.unwrap());
        self.current_flow = Some(self.finish_flow_label(post_while_label));
    }

    fn bind_do_statement(&mut self, node: &mut ast::Node) {
        let pre_do_label = self.create_loop_label();
        let branch_label = self.create_branch_label();
        let pre_condition_label = self.set_continue_target(node, branch_label);
        let post_do_label = self.create_branch_label();
        self.add_antecedent(pre_do_label, self.current_flow.unwrap());
        self.current_flow = Some(pre_do_label);
        let mut statement = self.store.statement(*node).unwrap();
        let mut expression = self.store.expression(*node).unwrap();
        self.bind_iterative_statement(&mut statement, post_do_label, pre_condition_label);
        self.add_antecedent(pre_condition_label, self.current_flow.unwrap());
        self.current_flow = Some(self.finish_flow_label(pre_condition_label));
        self.bind_condition(Some(&mut expression), pre_do_label, post_do_label);
        self.current_flow = Some(self.finish_flow_label(post_do_label));
    }

    fn bind_for_statement(&mut self, node: &mut ast::Node) {
        let loop_label = self.create_loop_label();
        let pre_loop_label = self.set_continue_target(node, loop_label);
        let pre_body_label = self.create_branch_label();
        let pre_incrementor_label = self.create_branch_label();
        let post_loop_label = self.create_branch_label();
        let mut initializer = self.store.initializer(*node);
        let mut condition = self.store.condition(*node);
        let mut statement = self.store.statement(*node).unwrap();
        let mut incrementor = self.store.incrementor(*node);
        self.bind(initializer.as_mut());
        self.add_antecedent(pre_loop_label, self.current_flow.unwrap());
        self.current_flow = Some(pre_loop_label);
        self.bind_condition(condition.as_mut(), pre_body_label, post_loop_label);
        self.current_flow = Some(self.finish_flow_label(pre_body_label));
        self.bind_iterative_statement(&mut statement, post_loop_label, pre_incrementor_label);
        self.add_antecedent(pre_incrementor_label, self.current_flow.unwrap());
        self.current_flow = Some(self.finish_flow_label(pre_incrementor_label));
        self.bind(incrementor.as_mut());
        self.add_antecedent(pre_loop_label, self.current_flow.unwrap());
        self.current_flow = Some(self.finish_flow_label(post_loop_label));
    }

    fn bind_for_in_or_for_of_statement(&mut self, node: &mut ast::Node) {
        let loop_label = self.create_loop_label();
        let pre_loop_label = self.set_continue_target(node, loop_label);
        let post_loop_label = self.create_branch_label();
        let mut expression = self.store.expression(*node).unwrap();
        let mut await_modifier = self.store.await_modifier(*node);
        let mut initializer = self.store.initializer(*node).unwrap();
        let mut statement = self.store.statement(*node).unwrap();
        self.bind(Some(&mut expression));
        self.add_antecedent(pre_loop_label, self.current_flow.unwrap());
        self.current_flow = Some(pre_loop_label);
        if self.store.kind(*node) == ast::Kind::ForOfStatement {
            self.bind(await_modifier.as_mut());
        }
        self.add_antecedent(post_loop_label, self.current_flow.unwrap());
        self.bind(Some(&mut initializer));
        if self.store.kind(initializer) != ast::Kind::VariableDeclarationList {
            self.bind_assignment_target_flow(&mut initializer);
        }
        self.bind_iterative_statement(&mut statement, post_loop_label, pre_loop_label);
        self.add_antecedent(pre_loop_label, self.current_flow.unwrap());
        self.current_flow = Some(self.finish_flow_label(post_loop_label));
    }

    fn bind_if_statement(&mut self, node: &mut ast::Node) {
        let then_label = self.create_branch_label();
        let else_label = self.create_branch_label();
        let post_if_label = self.create_branch_label();
        let mut expression = self.store.expression(*node).unwrap();
        let mut then_statement = self.store.then_statement(*node).unwrap();
        let mut else_statement = self.store.else_statement(*node);
        self.bind_condition(Some(&mut expression), then_label, else_label);
        self.current_flow = Some(self.finish_flow_label(then_label));
        self.bind(Some(&mut then_statement));
        self.add_antecedent(post_if_label, self.current_flow.unwrap());
        self.current_flow = Some(self.finish_flow_label(else_label));
        self.bind(else_statement.as_mut());
        self.add_antecedent(post_if_label, self.current_flow.unwrap());
        self.current_flow = Some(self.finish_flow_label(post_if_label));
    }

    fn bind_return_statement(&mut self, node: &mut ast::Node) {
        let mut expression = self.store.expression(*node);
        self.bind(expression.as_mut());
        if let (Some(current_return_target), Some(current_flow)) =
            (self.current_return_target, self.current_flow)
        {
            self.add_antecedent(current_return_target, current_flow);
        }
        self.current_flow = self.unreachable_flow;
        self.has_explicit_return = true;
        self.has_flow_effects = true;
    }

    fn bind_throw_statement(&mut self, node: &mut ast::Node) {
        let mut expression = self.store.expression(*node);
        self.bind(expression.as_mut());
        self.current_flow = self.unreachable_flow;
        self.has_flow_effects = true;
    }

    fn bind_break_statement(&mut self, node: &mut ast::Node) {
        self.bind_break_or_continue_statement(
            self.store.label(*node),
            self.current_break_target,
            ActiveLabel::break_target,
        );
    }

    fn bind_continue_statement(&mut self, node: &mut ast::Node) {
        self.bind_break_or_continue_statement(
            self.store.label(*node),
            self.current_continue_target,
            ActiveLabel::continue_target,
        );
    }

    fn bind_break_or_continue_statement(
        &mut self,
        label: Option<ast::Node>,
        current_target: Option<ast::FlowRef>,
        get_target: fn(&ActiveLabel<'a>) -> Option<ast::FlowRef>,
    ) {
        if let Some(mut label) = label {
            self.bind(Some(&mut label));
            let label_text = self.store.text(label);
            let target = if let Some(active_label) = self.find_active_label(&label_text) {
                active_label.referenced = true;
                get_target(active_label)
            } else {
                None
            };
            if target.is_some() {
                self.bind_break_or_continue_flow(target);
            }
        } else {
            self.bind_break_or_continue_flow(current_target);
        }
    }

    fn find_active_label(&mut self, name: &str) -> Option<&mut ActiveLabel<'a>> {
        let mut label = self.active_label_list.as_deref_mut();
        while let Some(current) = label {
            if current.name == name {
                return Some(current);
            }
            label = current.next.as_deref_mut();
        }
        None
    }

    fn bind_break_or_continue_flow(&mut self, flow_label: Option<ast::FlowRef>) {
        if let Some(flow_label) = flow_label {
            self.add_antecedent(flow_label, self.current_flow.unwrap());
            self.current_flow = self.unreachable_flow;
            self.has_flow_effects = true;
        }
    }

    fn bind_try_statement(&mut self, node: &mut ast::Node) {
        // We conservatively assume that *any* code in the try block can cause an exception, but we only need
        // to track code that causes mutations (because only mutations widen the possible control flow type of
        // a variable). The exceptionLabel is the target label for control flows that result from exceptions.
        // We add all mutation flow nodes as antecedents of this label such that we can analyze them as possible
        // antecedents of the start of catch or finally blocks. Furthermore, we add the current control flow to
        // represent exceptions that occur before any mutations.
        let mut try_block = self.store.try_block(*node).unwrap();
        let mut catch_clause = self.store.catch_clause(*node);
        let mut finally_block = self.store.finally_block(*node);
        let save_return_target = self.current_return_target;
        let save_exception_target = self.current_exception_target;
        let normal_exit_label = self.create_branch_label();
        let return_label = self.create_branch_label();
        let mut exception_label = self.create_branch_label();
        if finally_block.is_some() {
            self.current_return_target = Some(return_label);
        }
        self.add_antecedent(exception_label, self.current_flow.unwrap());
        self.current_exception_target = Some(exception_label);
        self.bind(Some(&mut try_block));
        self.add_antecedent(normal_exit_label, self.current_flow.unwrap());
        if let Some(catch_clause) = catch_clause.as_mut() {
            // Start of catch clause is the target of exceptions from try block.
            self.current_flow = Some(self.finish_flow_label(exception_label));
            // The currentExceptionTarget now represents control flows from exceptions in the catch clause.
            // Effectively, in a try-catch-finally, if an exception occurs in the try block, the catch block
            // acts like a second try block.
            exception_label = self.create_branch_label();
            self.add_antecedent(exception_label, self.current_flow.unwrap());
            self.current_exception_target = Some(exception_label);
            self.bind(Some(catch_clause));
            self.add_antecedent(normal_exit_label, self.current_flow.unwrap());
        }
        self.current_return_target = save_return_target;
        self.current_exception_target = save_exception_target;
        if let Some(finally_block) = finally_block.as_mut() {
            // Possible ways control can reach the finally block:
            // 1) Normal completion of try block of a try-finally or try-catch-finally
            // 2) Normal completion of catch block (following exception in try block) of a try-catch-finally
            // 3) Return in try or catch block of a try-finally or try-catch-finally
            // 4) Exception in try block of a try-finally
            // 5) Exception in catch block of a try-catch-finally
            // When analyzing a control flow graph that starts inside a finally block we want to consider all
            // five possibilities above. However, when analyzing a control flow graph that starts outside (past)
            // the finally block, we only want to consider the first two (if we're past a finally block then it
            // must have completed normally). Likewise, when analyzing a control flow graph from return statements
            // in try or catch blocks in an IIFE, we only want to consider the third. To make this possible, we
            // inject a ReduceLabel node into the control flow graph. This node contains an alternate reduced
            // set of antecedents for the pre-finally label. As control flow analysis passes by a ReduceLabel
            // node, the pre-finally label is temporarily switched to the reduced antecedent set.
            let finally_label = self.create_branch_label();
            let exception_antecedents = self.flow_graph.node(exception_label).antecedents;
            let return_antecedents = self.flow_graph.node(return_label).antecedents;
            let normal_exit_antecedents = self.flow_graph.node(normal_exit_label).antecedents;
            let combined_exception_return =
                self.combine_flow_lists(exception_antecedents, return_antecedents);
            let antecedents =
                self.combine_flow_lists(normal_exit_antecedents, combined_exception_return);
            self.flow_graph.node_mut(finally_label).antecedents = antecedents;
            self.current_flow = Some(finally_label);
            self.bind(Some(finally_block));
            if self
                .flow_graph
                .node(self.current_flow.unwrap())
                .flags
                .intersects(ast::FlowFlags::Unreachable)
            {
                // If the end of the finally block is unreachable, the end of the entire try statement is unreachable.
                self.current_flow = self.unreachable_flow;
            } else {
                // If we have an IIFE return target and return statements in the try or catch blocks, add a control
                // flow that goes back through the finally block and back through only the return statements.
                let return_antecedents = self.flow_graph.node(return_label).antecedents;
                if self.current_return_target.is_some() && return_antecedents.is_some() {
                    let reduce = self.create_reduce_label(
                        finally_label,
                        return_antecedents,
                        self.current_flow.unwrap(),
                    );
                    self.add_antecedent(self.current_return_target.unwrap(), reduce);
                }
                // If we have an outer exception target (i.e. a containing try-finally or try-catch-finally), add a
                // control flow that goes back through the finally block and back through each possible exception source.
                let exception_antecedents = self.flow_graph.node(exception_label).antecedents;
                if self.current_exception_target.is_some() && exception_antecedents.is_some() {
                    let reduce = self.create_reduce_label(
                        finally_label,
                        exception_antecedents,
                        self.current_flow.unwrap(),
                    );
                    self.add_antecedent(self.current_exception_target.unwrap(), reduce);
                }
                // If the end of the finally block is reachable, but the end of the try and catch blocks are not,
                // convert the current flow to unreachable. For example, 'try { return 1; } finally { ... }' should
                // result in an unreachable current control flow.
                let normal_exit_antecedents = self.flow_graph.node(normal_exit_label).antecedents;
                if normal_exit_antecedents.is_some() {
                    self.current_flow = Some(self.create_reduce_label(
                        finally_label,
                        normal_exit_antecedents,
                        self.current_flow.unwrap(),
                    ));
                } else {
                    self.current_flow = self.unreachable_flow;
                }
            }
        } else {
            self.current_flow = Some(self.finish_flow_label(normal_exit_label));
        }
    }
}

impl<'a> Binder<'a> {
    fn bind_switch_statement(&mut self, node: &mut ast::Node) {
        let post_switch_label = self.create_branch_label();
        let mut expression = self.store.expression(*node).unwrap();
        let mut case_block = self.store.case_block(*node).unwrap();
        self.bind(Some(&mut expression));
        let save_break_target = self.current_break_target.take();
        let save_pre_switch_case_flow = self.pre_switch_case_flow.take();
        self.current_break_target = Some(post_switch_label);
        self.pre_switch_case_flow = self.current_flow;
        self.bind(Some(&mut case_block));
        self.add_antecedent(post_switch_label, self.current_flow.unwrap());
        let has_default = self.store.clauses(case_block).is_some_and(|clauses| {
            clauses
                .into_iter()
                .any(|c| self.store.kind(c) == ast::Kind::DefaultClause)
        });
        if !has_default {
            let switch_clause =
                self.create_flow_switch_clause(self.pre_switch_case_flow.unwrap(), node, 0, 0);
            self.add_antecedent(post_switch_label, switch_clause);
        }
        self.current_break_target = save_break_target;
        self.pre_switch_case_flow = save_pre_switch_case_flow;
        self.current_flow = Some(self.finish_flow_label(post_switch_label));
    }

    fn bind_case_block(&mut self, node: &mut ast::Node) {
        let mut switch_statement = self.store.parent(*node).unwrap();
        let clauses: Vec<ast::Node> = self.store.clauses(*node).unwrap().into_iter().collect();
        let switch_expression = self.store.expression(switch_statement).unwrap();
        let is_narrowing_switch = self.store.kind(switch_expression) == ast::Kind::TrueKeyword
            || is_narrowing_expression(&self.store, &switch_expression);
        let mut fallthrough_flow = self.unreachable_flow.unwrap();
        let mut i = 0;
        while i < clauses.len() {
            let clause_start = i;
            while self
                .store
                .statements(clauses[i])
                .is_none_or(|statements| statements.is_empty())
                && i + 1 < clauses.len()
            {
                if same_flow_node(Some(fallthrough_flow), self.unreachable_flow) {
                    self.current_flow = self.pre_switch_case_flow;
                }
                let mut clause = clauses[i];
                self.bind(Some(&mut clause));
                i += 1;
            }
            let pre_case_label = self.create_branch_label();
            let mut pre_case_flow = self.pre_switch_case_flow.unwrap();
            if is_narrowing_switch {
                pre_case_flow = self.create_flow_switch_clause(
                    self.pre_switch_case_flow.unwrap(),
                    &mut switch_statement,
                    clause_start as i32,
                    i as i32 + 1,
                );
            }
            self.add_antecedent(pre_case_label, pre_case_flow);
            self.add_antecedent(pre_case_label, fallthrough_flow);
            self.current_flow = Some(self.finish_flow_label(pre_case_label));
            let mut clause = clauses[i];
            self.bind(Some(&mut clause));
            fallthrough_flow = self.current_flow.unwrap();
            if !self
                .flow_graph
                .node(self.current_flow.unwrap())
                .flags
                .intersects(ast::FlowFlags::Unreachable)
                && i != clauses.len() - 1
            {
                self.binding_state
                    .set_fallthrough_flow_node(clauses[i], self.current_flow);
            }
            i += 1;
        }
    }

    fn bind_case_or_default_clause(&mut self, node: &mut ast::Node) {
        let mut expression = self.store.expression(*node);
        let statements: Vec<ast::Node> =
            self.store.statements(*node).into_iter().flatten().collect();
        if expression.is_some() {
            let save_current_flow = self.current_flow;
            self.current_flow = self.pre_switch_case_flow;
            self.bind(expression.as_mut());
            self.current_flow = save_current_flow;
        }
        self.bind_each(statements);
    }

    fn bind_expression_statement(&mut self, node: &mut ast::Node) {
        let mut expression = self.store.expression(*node).unwrap();
        self.bind(Some(&mut expression));
        self.maybe_bind_expression_flow_if_call(&mut expression);
    }

    fn maybe_bind_expression_flow_if_call(&mut self, node: &mut ast::Node) {
        // A top level or comma expression call expression with a dotted function name and at least one argument
        // is potentially an assertion and is therefore included in the control flow.
        if ast::is_call_expression(&self.store, *node) {
            if self
                .store
                .expression(*node)
                .is_some_and(|expression| self.store.kind(expression) != ast::Kind::SuperKeyword)
                && self
                    .store
                    .expression(*node)
                    .is_some_and(|node| ast::is_dotted_name(&self.store, node))
            {
                self.current_flow = Some(self.create_flow_call(self.current_flow.unwrap(), node));
            }
        }
    }

    fn bind_labeled_statement(&mut self, node: &mut ast::Node) {
        let mut label = self.store.label(*node).unwrap();
        let mut statement = self.store.statement(*node).unwrap();
        let post_statement_label = self.create_branch_label();
        self.active_label_list = Some(Box::new(ActiveLabel {
            next: self.active_label_list.take(),
            name: self.store.text(label),
            break_target: Some(post_statement_label),
            continue_target: None,
            referenced: false,
            _marker: PhantomData,
        }));
        self.bind(Some(&mut label));
        self.bind(Some(&mut statement));
        if !self.active_label_list.as_deref().unwrap().referenced {
            // Mark the label as unused; the checker will decide whether to report it
            self.mark_unreachable(label);
        }
        self.active_label_list = self.active_label_list.take().unwrap().next;
        self.add_antecedent(post_statement_label, self.current_flow.unwrap());
        self.current_flow = Some(self.finish_flow_label(post_statement_label));
    }

    fn bind_prefix_unary_expression_flow(&mut self, node: &mut ast::Node) {
        let operator = self.store.operator(*node).unwrap();
        let mut operand = self.store.operand(*node).unwrap();
        if operator == ast::Kind::ExclamationToken {
            let save_true_target = self.current_true_target.take();
            self.current_true_target = self.current_false_target.take();
            self.current_false_target = save_true_target;
            self.bind_each_child(node);
            self.current_false_target = self.current_true_target.take();
            self.current_true_target = save_true_target;
        } else {
            self.bind_each_child(node);
            if operator == ast::Kind::PlusPlusToken || operator == ast::Kind::MinusMinusToken {
                self.bind_assignment_target_flow(&mut operand);
            }
        }
    }

    fn bind_postfix_unary_expression_flow(&mut self, node: &mut ast::Node) {
        let operator = self.store.operator(*node).unwrap();
        let mut operand = self.store.operand(*node).unwrap();
        self.bind_each_child(node);
        if operator == ast::Kind::PlusPlusToken || operator == ast::Kind::MinusMinusToken {
            self.bind_assignment_target_flow(&mut operand);
        }
    }

    fn bind_destructuring_assignment_flow(&mut self, node: &mut ast::Node) {
        let mut operator_token = self.store.operator_token(*node);
        let mut left = self.store.left(*node).unwrap();
        let mut right = self.store.right(*node).unwrap();
        let mut type_node = self.store.type_node(*node);
        if self.in_assignment_pattern {
            self.in_assignment_pattern = false;
            self.bind(operator_token.as_mut());
            self.bind(Some(&mut right));
            self.in_assignment_pattern = true;
            self.bind(Some(&mut left));
            self.bind(type_node.as_mut());
        } else {
            self.in_assignment_pattern = true;
            self.bind(Some(&mut left));
            self.bind(type_node.as_mut());
            self.in_assignment_pattern = false;
            self.bind(operator_token.as_mut());
            self.bind(Some(&mut right));
        }
        self.bind_assignment_target_flow(&mut left);
    }

    fn bind_binary_expression_worker(&mut self, node: &mut ast::Node) {
        if let Some(kind) = ast::get_assignment_declaration_kind(&self.store, *node) {
            match kind {
                ast::JSDeclarationKind::ModuleExports => self.bind_module_exports_assignment(node),
                ast::JSDeclarationKind::ExportsProperty => {
                    self.bind_exports_property_assignment(node)
                }
                ast::JSDeclarationKind::PrototypeProperty => {
                    self.bind_prototype_property_assignment(node)
                }
                ast::JSDeclarationKind::Property => self.bind_expando_property_assignment(node),
                ast::JSDeclarationKind::ThisProperty => self.bind_this_property_assignment(node),
                _ => {}
            }
        }
        self.check_strict_mode_binary_expression(node);
    }

    fn bind_binary_expression_flow(&mut self, node: &mut ast::Node) {
        #[derive(Clone, Copy)]
        struct BinaryFrame {
            node: ast::Node,
            phase: u8,
            is_root: bool,
            skip: bool,
            saved_seen_parse_error: bool,
            this_node_or_any_subnodes_has_error: bool,
        }

        let mut stack = vec![BinaryFrame {
            node: *node,
            phase: 0,
            is_root: true,
            skip: false,
            saved_seen_parse_error: false,
            this_node_or_any_subnodes_has_error: false,
        }];

        while let Some(mut frame) = stack.pop() {
            let mut current = frame.node;
            let operator_token = self.store.operator_token(current).unwrap();
            let operator = self.store.kind(operator_token);

            if frame.phase == 0 {
                if !frame.is_root {
                    self.bind_binary_expression_worker(&mut current);
                    frame.this_node_or_any_subnodes_has_error = self
                        .store
                        .flags(current)
                        .intersects(ast::NodeFlags::ThisNodeHasError);
                    frame.saved_seen_parse_error = self.seen_parse_error;
                    self.seen_parse_error = false;
                }

                if ast::is_logical_or_coalescing_binary_operator(operator)
                    || ast::is_logical_or_coalescing_assignment_operator(operator)
                {
                    if is_top_level_logical_expression(&self.store, &current) {
                        let post_expression_label = self.create_branch_label();
                        let save_current_flow = self.current_flow;
                        let save_has_flow_effects = self.has_flow_effects;
                        self.has_flow_effects = false;
                        self.bind_logical_like_expression(
                            &mut current,
                            post_expression_label,
                            post_expression_label,
                        );
                        if self.has_flow_effects {
                            self.current_flow = Some(self.finish_flow_label(post_expression_label));
                        } else {
                            self.current_flow = save_current_flow;
                        }
                        self.has_flow_effects = self.has_flow_effects || save_has_flow_effects;
                    } else {
                        self.bind_logical_like_expression(
                            &mut current,
                            self.current_true_target.unwrap(),
                            self.current_false_target.unwrap(),
                        );
                    }
                    frame.skip = true;
                    frame.phase = 3;
                    stack.push(frame);
                    continue;
                }

                let left = self.store.left(current).unwrap();
                frame.phase = 1;
                stack.push(frame);
                if self.store.kind(left) == ast::Kind::BinaryExpression
                    && !ast::is_destructuring_assignment(&self.store, left)
                {
                    stack.push(BinaryFrame {
                        node: left,
                        phase: 0,
                        is_root: false,
                        skip: false,
                        saved_seen_parse_error: false,
                        this_node_or_any_subnodes_has_error: false,
                    });
                } else {
                    let mut left = left;
                    self.bind(Some(&mut left));
                    if operator == ast::Kind::CommaToken {
                        self.maybe_bind_expression_flow_if_call(&mut left);
                    }
                }
                continue;
            }

            if frame.phase == 1 {
                let mut type_node = self.store.type_node(current);
                let mut operator_token = Some(operator_token);
                self.bind(type_node.as_mut());
                self.bind(operator_token.as_mut());

                let right = self.store.right(current).unwrap();
                frame.phase = 2;
                stack.push(frame);
                if self.store.kind(right) == ast::Kind::BinaryExpression
                    && !ast::is_destructuring_assignment(&self.store, right)
                {
                    stack.push(BinaryFrame {
                        node: right,
                        phase: 0,
                        is_root: false,
                        skip: false,
                        saved_seen_parse_error: false,
                        this_node_or_any_subnodes_has_error: false,
                    });
                } else {
                    let mut right = right;
                    self.bind(Some(&mut right));
                    if operator == ast::Kind::CommaToken {
                        self.maybe_bind_expression_flow_if_call(&mut right);
                    }
                }
                continue;
            }

            if frame.phase == 2 {
                if ast::is_assignment_operator(operator)
                    && !ast::is_assignment_target(&self.store, current)
                {
                    let mut left = self.store.left(current).unwrap();
                    self.bind_assignment_target_flow(&mut left);
                    if operator == ast::Kind::EqualsToken
                        && self.store.kind(left) == ast::Kind::ElementAccessExpression
                    {
                        let is_narrowable_element_access =
                            self.store.expression(left).is_some_and(|expression| {
                                is_narrowable_operand(&self.store, &expression)
                            });
                        if is_narrowable_element_access {
                            self.current_flow = Some(self.create_flow_mutation(
                                ast::FlowFlags::ArrayMutation,
                                self.current_flow.unwrap(),
                                &mut current,
                            ));
                        }
                    }
                }
                frame.phase = 3;
            }

            if !frame.is_root {
                if self.seen_parse_error {
                    frame.this_node_or_any_subnodes_has_error = true;
                }
                self.seen_parse_error = frame.saved_seen_parse_error;
                if frame.this_node_or_any_subnodes_has_error {
                    self.mark_subtree_has_error(current);
                    self.seen_parse_error = true;
                }
            }
        }
    }

    fn bind_logical_like_expression(
        &mut self,
        node: &mut ast::Node,
        true_target: ast::FlowRef,
        false_target: ast::FlowRef,
    ) {
        let mut left = self.store.left(*node).unwrap();
        let mut right = self.store.right(*node).unwrap();
        let mut operator_token = self.store.operator_token(*node);
        let pre_right_label = self.create_branch_label();
        let operator = self.store.kind(operator_token.unwrap());
        if operator == ast::Kind::AmpersandAmpersandToken
            || operator == ast::Kind::AmpersandAmpersandEqualsToken
        {
            self.bind_condition(Some(&mut left), pre_right_label, false_target);
        } else {
            self.bind_condition(Some(&mut left), true_target, pre_right_label);
        }
        self.current_flow = Some(self.finish_flow_label(pre_right_label));
        self.bind(operator_token.as_mut());
        if ast::is_logical_or_coalescing_assignment_operator(operator) {
            self.do_with_conditional_branches(
                Binder::bind_ref,
                Some(&mut right),
                true_target,
                false_target,
            );
            self.bind_assignment_target_flow(&mut left);
            let true_flow = self.create_flow_condition(
                ast::FlowFlags::TrueCondition,
                self.current_flow.unwrap(),
                Some(node),
            );
            self.add_antecedent(true_target, true_flow);
            let false_flow = self.create_flow_condition(
                ast::FlowFlags::FalseCondition,
                self.current_flow.unwrap(),
                Some(node),
            );
            self.add_antecedent(false_target, false_flow);
        } else {
            self.bind_condition(Some(&mut right), true_target, false_target);
        }
    }

    fn bind_delete_expression_flow(&mut self, node: &mut ast::Node) {
        self.bind_each_child(node);
        let mut expression = self.store.expression(*node);
        if expression.as_ref().is_some_and(|expression| {
            self.store.kind(*expression) == ast::Kind::PropertyAccessExpression
        }) {
            self.bind_assignment_target_flow(expression.as_mut().unwrap());
        }
    }

    fn bind_conditional_expression_flow(&mut self, node: &mut ast::Node) {
        let mut condition = self.store.condition(*node).unwrap();
        let mut question_token = self.store.question_token(*node);
        let mut when_true = self.store.when_true(*node).unwrap();
        let mut colon_token = self.store.colon_token(*node);
        let mut when_false = self.store.when_false(*node).unwrap();
        let true_label = self.create_branch_label();
        let false_label = self.create_branch_label();
        let post_expression_label = self.create_branch_label();
        let save_current_flow = self.current_flow;
        let save_has_flow_effects = self.has_flow_effects;
        self.has_flow_effects = false;
        self.bind_condition(Some(&mut condition), true_label, false_label);
        self.current_flow = Some(self.finish_flow_label(true_label));
        self.bind(question_token.as_mut());
        self.bind(Some(&mut when_true));
        self.add_antecedent(post_expression_label, self.current_flow.unwrap());
        self.current_flow = Some(self.finish_flow_label(false_label));
        self.bind(colon_token.as_mut());
        self.bind(Some(&mut when_false));
        self.add_antecedent(post_expression_label, self.current_flow.unwrap());
        if self.has_flow_effects {
            self.current_flow = Some(self.finish_flow_label(post_expression_label));
        } else {
            self.current_flow = save_current_flow;
        }
        self.has_flow_effects = self.has_flow_effects || save_has_flow_effects;
    }
}

impl<'a> Binder<'a> {
    fn bind_variable_declaration_flow(&mut self, node: &mut ast::Node) {
        self.bind_each_child(node);
        let parent = self.store.parent(*node).unwrap();
        let grandparent = self.store.parent(parent);
        if self.store.initializer(*node).is_some()
            || grandparent.as_ref().is_some_and(|grandparent| {
                ast::is_for_in_or_of_statement(&self.store, Some(*grandparent))
            })
        {
            self.bind_initialized_variable_flow(node);
        }
    }

    fn bind_initialized_variable_flow(&mut self, node: &mut ast::Node) {
        if matches!(
            self.store.kind(*node),
            ast::Kind::VariableDeclaration | ast::Kind::BindingElement
        ) && let Some(name) = self.store.name(*node)
            && ast::is_binding_pattern(&self.store, name)
        {
            let children: Vec<ast::Node> =
                self.store.elements(name).into_iter().flatten().collect();
            for mut child in children {
                self.bind_initialized_variable_flow(&mut child);
            }
            return;
        }
        self.current_flow = Some(self.create_flow_mutation(
            ast::FlowFlags::Assignment,
            self.current_flow.unwrap(),
            node,
        ));
    }

    fn bind_access_expression_flow(&mut self, node: &mut ast::Node) {
        if ast::is_optional_chain(&self.store, *node) {
            self.bind_optional_chain_flow(node);
        } else {
            self.bind_each_child(node);
        }
    }

    fn bind_optional_chain_flow(&mut self, node: &mut ast::Node) {
        if is_top_level_logical_expression(&self.store, node) {
            let post_expression_label = self.create_branch_label();
            let save_current_flow = self.current_flow;
            let save_has_flow_effects = self.has_flow_effects;
            self.bind_optional_chain(node, post_expression_label, post_expression_label);
            if self.has_flow_effects {
                self.current_flow = Some(self.finish_flow_label(post_expression_label));
            } else {
                self.current_flow = save_current_flow;
            }
            self.has_flow_effects = self.has_flow_effects || save_has_flow_effects;
        } else {
            self.bind_optional_chain(
                node,
                self.current_true_target.unwrap(),
                self.current_false_target.unwrap(),
            );
        }
    }

    fn bind_optional_chain(
        &mut self,
        node: &mut ast::Node,
        true_target: ast::FlowRef,
        false_target: ast::FlowRef,
    ) {
        // For an optional chain, we emulate the behavior of a logical expression:
        //
        // a?.b         -> a && a.b
        // a?.b.c       -> a && a.b.c
        // a?.b?.c      -> a && a.b && a.b.c
        // a?.[x = 1]   -> a && a[x = 1]
        //
        // To do this we descend through the chain until we reach the root of a chain (the expression with a `?.`)
        // and build it's CFA graph as if it were the first condition (`a && ...`). Then we bind the rest
        // of the node as part of the "true" branch, and continue to do so as we ascend back up to the outermost
        // chain node. We then treat the entire node as the right side of the expression.
        let mut pre_chain_label = None;
        if ast::is_optional_chain_root(&self.store, *node) {
            pre_chain_label = Some(self.create_branch_label());
        }
        let mut expression = self.store.expression(*node);
        let optional_expression_true_target = if let Some(pre_chain_label) = pre_chain_label {
            pre_chain_label
        } else {
            true_target
        };
        self.bind_optional_expression(
            expression.as_mut(),
            optional_expression_true_target,
            false_target,
        );
        if let Some(pre_chain_label) = pre_chain_label {
            self.current_flow = Some(self.finish_flow_label(pre_chain_label));
        }
        self.do_with_conditional_branches(
            Binder::bind_optional_chain_rest,
            Some(&mut *node),
            true_target,
            false_target,
        );
        if ast::is_outermost_optional_chain(&self.store, *node) {
            let true_flow = self.create_flow_condition(
                ast::FlowFlags::TrueCondition,
                self.current_flow.unwrap(),
                Some(&mut *node),
            );
            self.add_antecedent(true_target, true_flow);
            let false_flow = self.create_flow_condition(
                ast::FlowFlags::FalseCondition,
                self.current_flow.unwrap(),
                Some(node),
            );
            self.add_antecedent(false_target, false_flow);
        }
    }

    fn bind_optional_expression(
        &mut self,
        node: Option<&mut ast::Node>,
        true_target: ast::FlowRef,
        false_target: ast::FlowRef,
    ) {
        if let Some(node) = node {
            self.do_with_conditional_branches(
                Binder::bind_ref,
                Some(&mut *node),
                true_target,
                false_target,
            );
            if !ast::is_optional_chain(&self.store, *node)
                || ast::is_outermost_optional_chain(&self.store, *node)
            {
                let true_flow = self.create_flow_condition(
                    ast::FlowFlags::TrueCondition,
                    self.current_flow.unwrap(),
                    Some(&mut *node),
                );
                self.add_antecedent(true_target, true_flow);
                let false_flow = self.create_flow_condition(
                    ast::FlowFlags::FalseCondition,
                    self.current_flow.unwrap(),
                    Some(&mut *node),
                );
                self.add_antecedent(false_target, false_flow);
            }
        }
    }

    fn bind_optional_chain_rest(&mut self, node: Option<&mut ast::Node>) -> bool {
        let node = node.unwrap();
        match self.store.kind(*node) {
            ast::Kind::PropertyAccessExpression => {
                let mut question_dot_token = self.store.question_dot_token(*node);
                let mut name = self.store.name(*node);
                self.bind(question_dot_token.as_mut());
                self.bind(name.as_mut());
            }
            ast::Kind::ElementAccessExpression => {
                let mut question_dot_token = self.store.question_dot_token(*node);
                let mut argument_expression = self.store.argument_expression(*node);
                self.bind(question_dot_token.as_mut());
                self.bind(argument_expression.as_mut());
            }
            ast::Kind::CallExpression => {
                let mut question_dot_token = self.store.question_dot_token(*node);
                let type_arguments: Vec<ast::Node> = self
                    .store
                    .type_arguments(*node)
                    .into_iter()
                    .flatten()
                    .collect();
                let arguments: Vec<ast::Node> =
                    self.store.arguments(*node).into_iter().flatten().collect();
                self.bind(question_dot_token.as_mut());
                self.bind_each(type_arguments);
                self.bind_each(arguments);
            }
            _ => {}
        }
        false
    }

    fn bind_call_expression_flow(&mut self, node: &mut ast::Node) {
        if ast::is_optional_chain(&self.store, *node) {
            self.bind_optional_chain_flow(node);
        } else {
            // If the target of the call expression is a function expression or arrow function we have
            // an immediately invoked function expression (IIFE). Initialize the flowNode property to
            // the current control flow (which includes evaluation of the IIFE arguments).
            let mut expression = self.store.expression(*node).unwrap();
            let type_arguments: Vec<ast::Node> = self
                .store
                .type_arguments(*node)
                .into_iter()
                .flatten()
                .collect();
            let arguments: Vec<ast::Node> =
                self.store.arguments(*node).into_iter().flatten().collect();
            let expr = Some(ast::skip_parentheses(&self.store, expression));
            if expr.is_some_and(|expr| {
                matches!(
                    self.store.kind(expr),
                    ast::Kind::FunctionExpression | ast::Kind::ArrowFunction
                )
            }) {
                self.bind_each(type_arguments);
                self.bind_each(arguments);
                self.bind(Some(&mut expression));
            } else {
                self.bind_each_child(node);
                if self.store.expression(*node).is_some_and(|expression| {
                    self.store.kind(expression) == ast::Kind::SuperKeyword
                }) {
                    self.current_flow =
                        Some(self.create_flow_call(self.current_flow.unwrap(), node));
                }
            }
        }
        if self
            .store
            .expression(*node)
            .is_some_and(|expression| ast::is_property_access_expression(&self.store, expression))
        {
            let is_narrowable_array_mutation =
                self.store
                    .expression(*node)
                    .map(|access| {
                        self.store.name(access).is_some_and(|name| {
                            ast::is_identifier(&self.store, name)
                                && ast::is_push_or_unshift_identifier(&self.store, name)
                        }) && self.store.expression(access).is_some_and(|expression| {
                            is_narrowable_operand(&self.store, &expression)
                        })
                    })
                    .unwrap_or(false);
            if is_narrowable_array_mutation {
                self.current_flow = Some(self.create_flow_mutation(
                    ast::FlowFlags::ArrayMutation,
                    self.current_flow.unwrap(),
                    node,
                ));
            }
        }
    }

    fn bind_non_null_expression_flow(&mut self, node: &mut ast::Node) {
        if ast::is_optional_chain(&self.store, *node) {
            self.bind_optional_chain_flow(node);
        } else {
            self.bind_each_child(node);
        }
    }

    fn bind_binding_element_flow(&mut self, node: &mut ast::Node) {
        // When evaluating a binding pattern, the initializer is evaluated before the binding pattern, per:
        // - https://tc39.es/ecma262/#sec-destructuring-binding-patterns-runtime-semantics-iteratorbindinginitialization
        //   - `BindingElement: BindingPattern Initializer?`
        // - https://tc39.es/ecma262/#sec-runtime-semantics-keyedbindinginitialization
        //   - `BindingElement: BindingPattern Initializer?`
        let mut dot_dot_dot_token = self.store.dot_dot_dot_token(*node);
        let mut property_name = self.store.property_name(*node);
        let mut initializer = self.store.initializer(*node);
        let mut name = self.store.name(*node);
        self.bind(dot_dot_dot_token.as_mut());
        self.bind(property_name.as_mut());
        self.bind_initializer(initializer.as_mut());
        self.bind(name.as_mut());
    }

    fn bind_parameter_flow(&mut self, node: &mut ast::Node) {
        let modifiers = self.store.source_modifiers(*node);
        let mut dot_dot_dot_token = self.store.dot_dot_dot_token(*node);
        let mut question_token = self.store.question_token(*node);
        let mut type_node = self.store.type_node(*node);
        let mut initializer = self.store.initializer(*node);
        let mut name = self.store.name(*node);
        self.bind_modifiers(modifiers);
        self.bind(dot_dot_dot_token.as_mut());
        self.bind(question_token.as_mut());
        self.bind(type_node.as_mut());
        self.bind_initializer(initializer.as_mut());
        self.bind(name.as_mut());
    }

    // a BindingElement/Parameter does not have side effects if initializers are not evaluated and used. (see GH#49759)
    fn bind_initializer(&mut self, node: Option<&mut ast::Node>) {
        let Some(node) = node else {
            return;
        };
        let entry_flow = self.current_flow;
        self.bind(Some(node));
        if same_flow_node(entry_flow, self.unreachable_flow)
            || same_flow_node(entry_flow, self.current_flow)
        {
            self.current_flow = entry_flow;
            return;
        }
        let exit_flow = self.create_branch_label();
        self.add_antecedent(exit_flow, entry_flow.unwrap());
        self.add_antecedent(exit_flow, self.current_flow.unwrap());
        self.current_flow = Some(self.finish_flow_label(exit_flow));
    }
}

fn is_generator_function_expression(store: &ast::AstStore, node: &ast::Node) -> bool {
    ast::is_function_expression(store, *node) && store.asterisk_token(*node).is_some()
}

impl<'a> Binder<'a> {
    fn add_to_container_chain(&mut self, next: &mut ast::Node) {
        if let Some(last_container) = self.last_container {
            self.binding_state.set_next_container(last_container, *next);
        }
        self.last_container = Some(*next);
    }
}

/**
 * Declares a Symbol for the node and adds it to symbols. Reports errors for conflicting identifier names.
 * @param symbolTable - The symbol table which node will be added to.
 * @param parent - node's parent declaration.
 * @param node - The declaration to be added to the symbol table
 * @param includes - The SymbolFlags that node has in addition to its declaration type (eg: export, ambient, etc.)
 * @param excludes - The flags which node cannot be declared alongside in a symbol table. Used to report forbidden declarations.
 */

pub fn get_container_flags(store: &ast::AstStore, node: ast::Node) -> ContainerFlags {
    match store.kind(node) {
        ast::Kind::ClassExpression
        | ast::Kind::ClassDeclaration
        | ast::Kind::EnumDeclaration
        | ast::Kind::ObjectLiteralExpression
        | ast::Kind::TypeLiteral
        | ast::Kind::JsxAttributes => CONTAINER_FLAGS_IS_CONTAINER,
        ast::Kind::InterfaceDeclaration => {
            CONTAINER_FLAGS_IS_CONTAINER | CONTAINER_FLAGS_IS_INTERFACE
        }
        ast::Kind::ModuleDeclaration
        | ast::Kind::TypeAliasDeclaration
        | ast::Kind::JSTypeAliasDeclaration
        | ast::Kind::MappedType
        | ast::Kind::IndexSignature => CONTAINER_FLAGS_IS_CONTAINER | CONTAINER_FLAGS_HAS_LOCALS,
        ast::Kind::SourceFile => {
            CONTAINER_FLAGS_IS_CONTAINER
                | CONTAINER_FLAGS_IS_CONTROL_FLOW_CONTAINER
                | CONTAINER_FLAGS_HAS_LOCALS
        }
        ast::Kind::GetAccessor | ast::Kind::SetAccessor | ast::Kind::MethodDeclaration => {
            if ast::is_object_literal_or_class_expression_method_or_accessor(store, node) {
                return CONTAINER_FLAGS_IS_CONTAINER
                    | CONTAINER_FLAGS_IS_CONTROL_FLOW_CONTAINER
                    | CONTAINER_FLAGS_HAS_LOCALS
                    | CONTAINER_FLAGS_IS_FUNCTION_LIKE
                    | CONTAINER_FLAGS_IS_OBJECT_LITERAL_OR_CLASS_EXPRESSION_METHOD_OR_ACCESSOR
                    | CONTAINER_FLAGS_IS_THIS_CONTAINER;
            }
            CONTAINER_FLAGS_IS_CONTAINER
                | CONTAINER_FLAGS_IS_CONTROL_FLOW_CONTAINER
                | CONTAINER_FLAGS_HAS_LOCALS
                | CONTAINER_FLAGS_IS_FUNCTION_LIKE
                | CONTAINER_FLAGS_IS_THIS_CONTAINER
        }
        ast::Kind::Constructor
        | ast::Kind::FunctionDeclaration
        | ast::Kind::ClassStaticBlockDeclaration => {
            CONTAINER_FLAGS_IS_CONTAINER
                | CONTAINER_FLAGS_IS_CONTROL_FLOW_CONTAINER
                | CONTAINER_FLAGS_HAS_LOCALS
                | CONTAINER_FLAGS_IS_FUNCTION_LIKE
                | CONTAINER_FLAGS_IS_THIS_CONTAINER
        }
        ast::Kind::MethodSignature
        | ast::Kind::CallSignature
        | ast::Kind::FunctionType
        | ast::Kind::ConstructSignature
        | ast::Kind::ConstructorType => {
            CONTAINER_FLAGS_IS_CONTAINER
                | CONTAINER_FLAGS_IS_CONTROL_FLOW_CONTAINER
                | CONTAINER_FLAGS_HAS_LOCALS
                | CONTAINER_FLAGS_IS_FUNCTION_LIKE
                | CONTAINER_FLAGS_PROPAGATES_THIS_KEYWORD
        }
        ast::Kind::FunctionExpression => {
            CONTAINER_FLAGS_IS_CONTAINER
                | CONTAINER_FLAGS_IS_CONTROL_FLOW_CONTAINER
                | CONTAINER_FLAGS_HAS_LOCALS
                | CONTAINER_FLAGS_IS_FUNCTION_LIKE
                | CONTAINER_FLAGS_IS_FUNCTION_EXPRESSION
                | CONTAINER_FLAGS_IS_THIS_CONTAINER
        }
        ast::Kind::ArrowFunction => {
            CONTAINER_FLAGS_IS_CONTAINER
                | CONTAINER_FLAGS_IS_CONTROL_FLOW_CONTAINER
                | CONTAINER_FLAGS_HAS_LOCALS
                | CONTAINER_FLAGS_IS_FUNCTION_LIKE
                | CONTAINER_FLAGS_IS_FUNCTION_EXPRESSION
                | CONTAINER_FLAGS_PROPAGATES_THIS_KEYWORD
        }
        ast::Kind::ModuleBlock => CONTAINER_FLAGS_IS_CONTROL_FLOW_CONTAINER,
        ast::Kind::PropertyDeclaration => {
            if store.initializer(node).is_some() {
                CONTAINER_FLAGS_IS_CONTROL_FLOW_CONTAINER | CONTAINER_FLAGS_IS_THIS_CONTAINER
            } else {
                CONTAINER_FLAGS_NONE
            }
        }
        ast::Kind::CatchClause
        | ast::Kind::ForStatement
        | ast::Kind::ForInStatement
        | ast::Kind::ForOfStatement
        | ast::Kind::CaseBlock => {
            CONTAINER_FLAGS_IS_BLOCK_SCOPED_CONTAINER | CONTAINER_FLAGS_HAS_LOCALS
        }
        ast::Kind::Block => {
            let parent = store.parent(node);
            if ast::is_function_like(store, parent)
                || parent
                    .as_ref()
                    .is_some_and(|parent| ast::is_class_static_block_declaration(store, *parent))
            {
                CONTAINER_FLAGS_NONE
            } else {
                CONTAINER_FLAGS_IS_BLOCK_SCOPED_CONTAINER | CONTAINER_FLAGS_HAS_LOCALS
            }
        }
        _ => CONTAINER_FLAGS_NONE,
    }
}

fn is_narrowing_expression(store: &ast::AstStore, expr: &ast::Node) -> bool {
    match store.kind(*expr) {
        ast::Kind::Identifier | ast::Kind::ThisKeyword => true,
        ast::Kind::PropertyAccessExpression | ast::Kind::ElementAccessExpression => {
            contains_narrowable_reference(store, expr)
        }
        ast::Kind::CallExpression => has_narrowable_argument(store, expr),
        ast::Kind::ParenthesizedExpression
        | ast::Kind::NonNullExpression
        | ast::Kind::TypeOfExpression => store
            .expression(*expr)
            .is_some_and(|expression| is_narrowing_expression(store, &expression)),
        ast::Kind::BinaryExpression => is_narrowing_binary_expression(store, expr),
        ast::Kind::PrefixUnaryExpression => {
            store.operator(*expr) == Some(ast::Kind::ExclamationToken)
                && store
                    .operand(*expr)
                    .is_some_and(|operand| is_narrowing_expression(store, &operand))
        }
        _ => false,
    }
}

fn contains_narrowable_reference(store: &ast::AstStore, expr: &ast::Node) -> bool {
    if is_narrowable_reference(store, expr) {
        return true;
    }
    if store.flags(*expr).intersects(ast::NodeFlags::OptionalChain) {
        match store.kind(*expr) {
            ast::Kind::PropertyAccessExpression
            | ast::Kind::ElementAccessExpression
            | ast::Kind::CallExpression
            | ast::Kind::NonNullExpression => {
                return store
                    .expression(*expr)
                    .is_some_and(|expression| contains_narrowable_reference(store, &expression));
            }
            _ => {}
        }
    }
    false
}

fn is_narrowable_reference(store: &ast::AstStore, node: &ast::Node) -> bool {
    match store.kind(*node) {
        ast::Kind::Identifier
        | ast::Kind::ThisKeyword
        | ast::Kind::SuperKeyword
        | ast::Kind::MetaProperty => true,
        ast::Kind::PropertyAccessExpression
        | ast::Kind::ParenthesizedExpression
        | ast::Kind::NonNullExpression => store
            .expression(*node)
            .is_some_and(|expression| is_narrowable_reference(store, &expression)),
        ast::Kind::ElementAccessExpression => {
            store.argument_expression(*node).is_some_and(|argument| {
                ast::is_string_or_numeric_literal_like(store, argument)
                    || ast::is_entity_name_expression(store, argument)
                        && store
                            .expression(*node)
                            .is_some_and(|expression| is_narrowable_reference(store, &expression))
            })
        }
        ast::Kind::BinaryExpression => {
            let operator = store
                .operator_token(*node)
                .map(|token| store.kind(token))
                .unwrap();
            operator == ast::Kind::CommaToken
                && store
                    .right(*node)
                    .is_some_and(|right| is_narrowable_reference(store, &right))
                || ast::is_assignment_operator(operator)
                    && store
                        .left(*node)
                        .is_some_and(|left| ast::is_left_hand_side_expression(store, left))
        }
        _ => false,
    }
}

fn has_narrowable_argument(store: &ast::AstStore, expr: &ast::Node) -> bool {
    for argument in store.arguments(*expr).into_iter().flatten() {
        //nolint:modernize
        if contains_narrowable_reference(store, &argument) {
            return true;
        }
    }
    if store
        .expression(*expr)
        .is_some_and(|expression| ast::is_property_access_expression(store, expression))
        && store
            .expression(*expr)
            .and_then(|expression| store.expression(expression))
            .is_some_and(|expression| contains_narrowable_reference(store, &expression))
    {
        return true;
    }
    false
}

fn is_narrowing_binary_expression(store: &ast::AstStore, expr: &ast::Node) -> bool {
    match store
        .operator_token(*expr)
        .map(|token| store.kind(token))
        .unwrap()
    {
        ast::Kind::EqualsToken
        | ast::Kind::BarBarEqualsToken
        | ast::Kind::AmpersandAmpersandEqualsToken
        | ast::Kind::QuestionQuestionEqualsToken => store
            .left(*expr)
            .is_some_and(|left| contains_narrowable_reference(store, &left)),
        ast::Kind::EqualsEqualsToken
        | ast::Kind::ExclamationEqualsToken
        | ast::Kind::EqualsEqualsEqualsToken
        | ast::Kind::ExclamationEqualsEqualsToken => {
            let left = ast::skip_parentheses(store, store.left(*expr).unwrap());
            let right = ast::skip_parentheses(store, store.right(*expr).unwrap());
            is_narrowable_operand(store, &left)
                || is_narrowable_operand(store, &right)
                || is_narrowing_type_of_operands(store, &right, &left)
                || is_narrowing_type_of_operands(store, &left, &right)
                || (ast::is_boolean_literal(store, right) && is_narrowing_expression(store, &left)
                    || ast::is_boolean_literal(store, left)
                        && is_narrowing_expression(store, &right))
        }
        ast::Kind::InstanceOfKeyword => store
            .left(*expr)
            .is_some_and(|left| is_narrowable_operand(store, &left)),
        ast::Kind::InKeyword => store
            .right(*expr)
            .is_some_and(|right| is_narrowing_expression(store, &right)),
        ast::Kind::CommaToken => store
            .right(*expr)
            .is_some_and(|right| is_narrowing_expression(store, &right)),
        _ => false,
    }
}

fn is_narrowable_operand(store: &ast::AstStore, expr: &ast::Node) -> bool {
    match store.kind(*expr) {
        ast::Kind::ParenthesizedExpression => {
            return store
                .expression(*expr)
                .is_some_and(|expression| is_narrowable_operand(store, &expression));
        }
        ast::Kind::BinaryExpression => {
            match store
                .operator_token(*expr)
                .map(|token| store.kind(token))
                .unwrap()
            {
                ast::Kind::EqualsToken => {
                    return store
                        .left(*expr)
                        .is_some_and(|left| is_narrowable_operand(store, &left));
                }
                ast::Kind::CommaToken => {
                    return store
                        .right(*expr)
                        .is_some_and(|right| is_narrowable_operand(store, &right));
                }
                _ => {}
            }
        }
        _ => {}
    }
    contains_narrowable_reference(store, expr)
}

fn is_narrowing_type_of_operands(
    store: &ast::AstStore,
    expr1: &ast::Node,
    expr2: &ast::Node,
) -> bool {
    ast::is_type_of_expression(store, *expr1)
        && store
            .expression(*expr1)
            .is_some_and(|expression| is_narrowable_operand(store, &expression))
        && ast::is_string_literal_like(store, *expr2)
}

impl<'a> Binder<'a> {
    fn error_on_node(
        &mut self,
        node: &ast::Node,
        message: &'static diagnostics::Message,
        args: &[String],
    ) {
        let diagnostic = self.create_diagnostic_for_node(node, message, args);
        self.add_diagnostic(diagnostic);
    }

    fn error_on_first_token(
        &mut self,
        node: &ast::Node,
        message: &'static diagnostics::Message,
        args: &[String],
    ) {
        let source_file = self.source_file_view();
        let span = scanner::get_range_of_token_at_position(
            &source_file,
            self.store.loc(*node).pos().max(0) as usize,
        );
        let args = args.iter().cloned().map(Into::into).collect::<Vec<_>>();
        self.add_diagnostic(ast::new_diagnostic_with_file(
            Some(source_file.diagnostic_file()),
            span,
            message,
            &args,
        ));
    }

    fn error_or_suggestion_on_node(
        &mut self,
        is_error: bool,
        node: &ast::Node,
        message: &'static diagnostics::Message,
    ) {
        self.error_or_suggestion_on_range(is_error, node, node, message);
    }

    fn error_or_suggestion_on_range(
        &mut self,
        is_error: bool,
        start_node: &ast::Node,
        end_node: &ast::Node,
        message: &'static diagnostics::Message,
    ) {
        let source_file = self.source_file_view();
        let text_range = core::TextRange::new(
            scanner::get_range_of_token_at_position(
                &source_file,
                self.store.loc(*start_node).pos().max(0) as usize,
            )
            .pos(),
            self.store.loc(*end_node).end(),
        );
        let mut diagnostic = ast::new_diagnostic_with_file(
            Some(source_file.diagnostic_file()),
            text_range,
            message,
            &[],
        );
        if is_error {
            self.add_diagnostic(diagnostic);
        } else {
            diagnostic.set_category(diagnostics::Category::Suggestion);
            self.file_bind_suggestion_diagnostics.push(diagnostic);
        }
    }

    // Inside the binder, we may create a diagnostic for an as-yet unbound node (with potentially no parent pointers, implying no accessible source file)
    // If so, the node _must_ be in the current file (as that's the only way anything could have traversed to it to yield it as the error node)
    // This version of `createDiagnosticForNode` uses the binder's context to account for this, and always yields correct diagnostics even in these situations.
    fn create_diagnostic_for_node(
        &self,
        node: &ast::Node,
        message: &'static diagnostics::Message,
        args: &[String],
    ) -> ast::Diagnostic {
        let args = args.iter().cloned().map(Into::into).collect::<Vec<_>>();
        let source_file = self.source_file_view();
        ast::new_diagnostic_with_file(
            Some(source_file.diagnostic_file()),
            scanner::get_error_range_for_node(&source_file, node),
            message,
            &args,
        )
    }

    fn add_diagnostic(&mut self, diagnostic: ast::Diagnostic) {
        self.file_bind_diagnostics.push(diagnostic);
    }
}

fn is_signed_numeric_literal(store: &ast::AstStore, node: &ast::Node) -> bool {
    if store.kind(*node) == ast::Kind::PrefixUnaryExpression {
        let operator = store.operator(*node);
        return (operator == Some(ast::Kind::PlusToken) || operator == Some(ast::Kind::MinusToken))
            && store
                .operand(*node)
                .is_some_and(|operand| ast::is_numeric_literal(store, operand));
    }
    false
}

fn is_statement_condition(store: &ast::AstStore, node: &ast::Node) -> bool {
    match store.kind(store.parent(*node).unwrap()) {
        ast::Kind::IfStatement | ast::Kind::WhileStatement | ast::Kind::DoStatement => {
            let parent = store.parent(*node).unwrap();
            store.expression(parent) == Some(*node)
        }
        ast::Kind::ForStatement => {
            let parent = store.parent(*node).unwrap();
            store.condition(parent).as_ref() == Some(node)
        }
        ast::Kind::ConditionalExpression => {
            let parent = store.parent(*node).unwrap();
            store.condition(parent).as_ref() == Some(node)
        }
        _ => false,
    }
}

fn is_top_level_logical_expression(store: &ast::AstStore, node: &ast::Node) -> bool {
    let mut current = node.clone();
    while let Some(parent) = store.parent(current)
        && (ast::is_parenthesized_expression(store, parent)
            || ast::is_prefix_unary_expression(store, parent)
                && store.operator(parent) == Some(ast::Kind::ExclamationToken))
    {
        current = parent;
    }
    let parent = store
        .parent(current)
        .expect("logical expression should have parent");
    !is_statement_condition(store, &current)
        && !ast::is_logical_expression(store, parent)
        && !(ast::is_optional_chain(store, parent) && store.expression(parent) == Some(current))
}

fn is_assignment_declaration(store: &ast::AstStore, decl: &ast::Node) -> bool {
    ast::is_binary_expression(store, *decl)
        || ast::is_access_expression(store, *decl)
        || ast::is_identifier(store, *decl)
        || ast::is_call_expression(store, *decl)
}

fn is_effective_module_declaration(store: &ast::AstStore, node: &ast::Node) -> bool {
    ast::is_module_declaration(store, *node) || ast::is_identifier(store, *node)
}
