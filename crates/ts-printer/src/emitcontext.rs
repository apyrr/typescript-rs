use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::ControlFlow;
use std::sync::atomic::AtomicU32;
use std::sync::{Arc, Mutex, MutexGuard};

use ts_ast as ast;
use ts_collections as collections;
use ts_core as core;

use crate::{
    EF_CUSTOM_PROLOGUE, EF_EXTERNAL_HELPERS, EF_HELPER_NAME, EF_LOCAL_NAME, EF_NO_COMMENTS,
    EF_NO_NESTED_SOURCE_MAPS, EF_NO_SOURCE_MAP, EF_NO_TOKEN_SOURCE_MAPS, EF_NO_TRAILING_SOURCE_MAP,
    EF_NONE, EF_SINGLE_LINE, EmitFlags, EmitHelper, GeneratedIdentifierFlags,
    factory::{AssignedNameOptions, NodeFactory, new_node_factory_with_state},
};

#[derive(Clone)]
pub(crate) struct EmitContextStateRef(Arc<Mutex<EmitContextState>>);

impl EmitContextStateRef {
    fn new(state: EmitContextState) -> Self {
        Self(Arc::new(Mutex::new(state)))
    }

    pub(crate) fn borrow(&self) -> MutexGuard<'_, EmitContextState> {
        self.0.lock().unwrap_or_else(|err| err.into_inner())
    }

    pub(crate) fn borrow_mut(&self) -> MutexGuard<'_, EmitContextState> {
        self.0.lock().unwrap_or_else(|err| err.into_inner())
    }

    fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

// Stores side-table information used during transformation that can be read by the printer to customize emit
//
// NOTE: EmitContext is not guaranteed to be thread-safe.
pub struct EmitContext {
    pub factory: NodeFactory, // Required. The NodeFactory to use to create new nodes
    pub(crate) state: EmitContextStateRef,
    source_file: Option<ast::SourceFile>,
    parse_source_file: Option<ast::SourceFile>,
    source_files: Vec<ast::SourceFile>,
}

// Stores mutable emit side tables shared with factory hooks and name generator.
pub(crate) struct EmitContextState {
    pub(crate) auto_generate: ast::NodeSideTable<AutoGenerateInfo>,
    pub(crate) text_source: ast::NodeSideTable<ast::Node>,
    pub(crate) original: ast::NodeSideTable<ast::Node>,
    pub(crate) emit_nodes: core::LinkStore<ast::Node, EmitNode>,
    pub(crate) assigned_name: ast::NodeSideTable<ast::Node>,
    pub(crate) class_this: ast::NodeSideTable<ast::Node>,
    pub(crate) var_scope_stack: core::Stack<VarScope>,
    pub(crate) let_scope_stack: core::Stack<VarScope>,
    pub(crate) emit_helpers: collections::OrderedSet<usize>,
}

impl EmitContextState {
    fn ensure_emit_node<R>(&self, node: ast::Node, f: impl FnOnce(&mut EmitNode) -> R) -> R {
        let handle = self.emit_nodes.ensure_handle(node);
        self.emit_nodes.with_by_handle_mut(handle, f)
    }

    fn with_emit_node<R>(&self, node: ast::Node, f: impl FnOnce(&EmitNode) -> R) -> Option<R> {
        let handle = self.emit_nodes.try_handle(node)?;
        Some(self.emit_nodes.with_by_handle(handle, f))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvironmentFlags {
    None = 0,
    InParameters = 1 << 0, // currently visiting a parameter list
    VariablesHoistedInParameters = 1 << 1, // a temp variable was hoisted while visiting a parameter list
}

#[derive(Default, Clone)]
pub struct VarScope {
    variables: Vec<ast::Node>,
    functions: Vec<ast::Node>,
    flags: i32,
    initialization_statements: Vec<ast::Node>,
}

pub fn new_emit_context() -> EmitContext {
    let state = EmitContextStateRef::new(EmitContextState::default());
    let mut ctx = EmitContext {
        factory: NodeFactory::default(),
        state,
        source_file: None,
        parse_source_file: None,
        source_files: Vec::new(),
    };
    ctx.reset();
    ctx
}

thread_local! {
    static EMIT_CONTEXT_POOL: RefCell<Vec<EmitContext>> = const { RefCell::new(Vec::new()) };
}

pub fn get_emit_context() -> (EmitContext, impl FnOnce(EmitContext)) {
    let ctx = EMIT_CONTEXT_POOL
        .with(|pool| pool.borrow_mut().pop())
        .unwrap_or_else(new_emit_context);
    (ctx, |mut ctx| {
        ctx.reset();
        EMIT_CONTEXT_POOL.with(|pool| pool.borrow_mut().push(ctx));
    })
}

impl EmitContext {
    pub fn merge_from(&mut self, other: EmitContext) {
        if self.state.ptr_eq(&other.state) {
            return;
        }
        let mut state = self.state.borrow_mut();
        let mut other_state = other.state.borrow_mut();
        state.auto_generate.append(&mut other_state.auto_generate);
        state.text_source.append(&mut other_state.text_source);
        state.original.append(&mut other_state.original);
        state
            .emit_nodes
            .extend_from(std::mem::take(&mut other_state.emit_nodes));
        state.assigned_name.append(&mut other_state.assigned_name);
        state.class_this.append(&mut other_state.class_this);
        for helper in other_state.emit_helpers.values() {
            state.emit_helpers.add(*helper);
        }
    }

    pub fn reset(&mut self) {
        *self.state.borrow_mut() = EmitContextState::default();
        self.factory = new_node_factory_with_state(self.state.clone());
        self.source_file = None;
        self.parse_source_file = None;
        self.source_files.clear();
    }

    pub(crate) fn state_ref(&self) -> EmitContextStateRef {
        self.state.clone()
    }

    pub fn activate(&mut self) {
        self.factory = new_node_factory_with_state(self.state_ref());
    }

    pub fn set_source_file(&mut self, source_file: Option<&ast::SourceFile>) {
        self.set_current_source_file(source_file);
        if let Some(source_file) = source_file {
            self.add_source_file(source_file);
        }
    }

    pub fn set_current_source_file(&mut self, source_file: Option<&ast::SourceFile>) {
        self.source_file = source_file.map(ast::SourceFile::share_readonly);
        if self.parse_source_file.is_none() {
            self.parse_source_file = source_file.map(ast::SourceFile::share_readonly);
        }
    }

    pub fn add_source_file(&mut self, source_file: &ast::SourceFile) {
        if !self
            .source_files
            .iter()
            .any(|file| file.store().store_id() == source_file.store().store_id())
        {
            self.source_files
                .push(ast::SourceFile::share_readonly(source_file));
        }
    }

    pub fn store_for_node(&self, node: ast::Node) -> &ast::AstStore {
        self.store_for_store_id(node.store_id())
    }

    pub fn store_for_store_id(&self, store_id: ast::StoreId) -> &ast::AstStore {
        let factory_store = self.factory.node_factory.store();
        if store_id == factory_store.store_id() {
            return factory_store;
        }
        if let Some(source_file) = self.source_file.as_ref()
            && store_id == source_file.store().store_id()
        {
            return source_file.store();
        }
        if let Some(parse_source_file) = self.parse_source_file.as_ref()
            && store_id == parse_source_file.store().store_id()
        {
            return parse_source_file.store();
        }
        if let Some(source_file) = self
            .source_files
            .iter()
            .find(|file| store_id == file.store().store_id())
        {
            return source_file.store();
        }
        panic!("emit context cannot resolve AST store {:?}", store_id);
    }

    pub fn environment_stack_depths(&self) -> (usize, usize) {
        let state = self.state.borrow();
        (state.var_scope_stack.len(), state.let_scope_stack.len())
    }

    pub fn assert_environment_balanced(&self) {
        assert_eq!(
            self.environment_stack_depths(),
            (0, 0),
            "emit context environments must be balanced before finishing a source file"
        );
    }

    pub fn with_store_for_node<R>(
        &self,
        node: ast::Node,
        f: impl FnOnce(&ast::AstStore) -> R,
    ) -> R {
        f(self.store_for_node(node))
    }

    pub fn with_store_for_store_id<R>(
        &self,
        store_id: ast::StoreId,
        f: impl FnOnce(&ast::AstStore) -> R,
    ) -> R {
        f(self.store_for_store_id(store_id))
    }

    pub fn with_source_file_view<R>(
        &self,
        root: ast::Node,
        f: impl FnOnce(ast::SourceFileView<'_>) -> R,
    ) -> R {
        self.with_store_for_node(root, |store| f(store.source_file_view(root)))
    }

    pub fn resolve_source_node_list(
        &self,
        source: ast::SourceNodeListRef,
    ) -> ast::SourceNodeList<'_> {
        source.resolve(self.store_for_store_id(source.store_id()))
    }

    pub fn resolve_source_modifier_list(
        &self,
        source: ast::SourceModifierListRef,
    ) -> ast::SourceModifierList<'_> {
        source.resolve(self.store_for_store_id(source.store_id()))
    }

    pub fn resolve_source_raw_node_slice(
        &self,
        source: ast::SourceRawNodeSliceRef,
    ) -> ast::SourceRawNodeSlice<'_> {
        source.resolve(self.store_for_store_id(source.store_id()))
    }

    pub fn resolve_source_raw_string_slice(
        &self,
        source: ast::SourceRawStringSliceRef,
    ) -> ast::SourceRawStringSlice<'_> {
        source.resolve(self.store_for_store_id(source.store_id()))
    }

    pub fn source_file_for_node(&self, node: ast::Node) -> Option<ast::SourceFileView<'_>> {
        let store = self.store_for_node(node);
        ast::get_source_file_of_node(store, Some(node))
            .map(|source_file| store.source_file_view(source_file))
    }

    pub fn source_file_handle_for_node(&self, node: ast::Node) -> Option<ast::SourceFile> {
        self.source_file
            .as_ref()
            .filter(|source_file| source_file.store().store_id() == node.store_id())
            .or_else(|| {
                self.parse_source_file
                    .as_ref()
                    .filter(|source_file| source_file.store().store_id() == node.store_id())
            })
            .or_else(|| {
                self.source_files
                    .iter()
                    .find(|source_file| source_file.store().store_id() == node.store_id())
            })
            .map(ast::SourceFile::share_readonly)
    }

    pub fn new_generated_name_for_node(&mut self, node: ast::Node) -> ast::Node {
        let factory_store = self.factory.node_factory.store();
        if node.store_id() == factory_store.store_id() {
            let generated_node = get_node_for_generated_name_worker_in_state(&self.state, &node, 0);
            if generated_node.store_id() != factory_store.store_id()
                && let Some(source_file) = self.source_file_handle_for_node(generated_node)
            {
                return self
                    .factory
                    .new_generated_name_for_node(source_file.store(), &node);
            }
            return self.factory.new_generated_name_for_factory_node(&node);
        }
        if let Some(source_file) = self.source_file.as_ref()
            && node.store_id() == source_file.store().store_id()
        {
            return self
                .factory
                .new_generated_name_for_node(source_file.store(), &node);
        }
        if let Some(parse_source_file) = self.parse_source_file.as_ref()
            && node.store_id() == parse_source_file.store().store_id()
        {
            return self
                .factory
                .new_generated_name_for_node(parse_source_file.store(), &node);
        }
        if let Some(source_file) = self
            .source_files
            .iter()
            .find(|file| node.store_id() == file.store().store_id())
        {
            return self
                .factory
                .new_generated_name_for_node(source_file.store(), &node);
        }
        panic!(
            "emit context cannot resolve node from AST store {:?}",
            node.store_id()
        );
    }

    // Gets the local name of a declaration. This is primarily used for declarations that can be referred to by name in the
    // declaration's immediate scope (classes, enums, namespaces). A local name will *never* be prefixed with a module or
    // namespace export modifier like "exports." when emitted as an expression.
    pub fn get_local_name(&mut self, node: ast::Node) -> ast::Node {
        self.get_local_name_ex(node, AssignedNameOptions::default())
    }

    // Gets the local name of a declaration. This is primarily used for declarations that can be referred to by name in the
    // declaration's immediate scope (classes, enums, namespaces). A local name will *never* be prefixed with a module or
    // namespace export modifier like "exports." when emitted as an expression.
    pub fn get_local_name_ex(&mut self, node: ast::Node, opts: AssignedNameOptions) -> ast::Node {
        let node_name = {
            let source = self.store_for_node(node);
            if opts.ignore_assigned_name {
                ast::get_non_assigned_name_of_declaration(source, node)
            } else {
                ast::get_name_of_declaration(source, Some(node))
            }
        };

        if let Some(node_name) = node_name {
            let name = if node_name.store_id() == self.factory.node_factory.store().store_id() {
                self.factory
                    .node_factory
                    .deep_clone_node_in_current_store_preserve_location(node_name)
            } else {
                let source_file = self
                    .source_file_handle_for_node(node_name)
                    .expect("emit context cannot resolve source node without a source file");
                self.factory
                    .node_factory
                    .deep_clone_node_from_store_preserve_location(source_file.store(), node_name)
            };
            let mut emit_flags = EF_LOCAL_NAME;
            if !opts.allow_comments {
                emit_flags |= EF_NO_COMMENTS;
            }
            if !opts.allow_source_maps {
                emit_flags |= EF_NO_SOURCE_MAP;
            }
            self.mark_emit_node(&name, emit_flags);
            return name;
        }

        self.new_generated_name_for_node(node)
    }

    pub fn new_generated_private_name_for_node_ex(
        &mut self,
        node: ast::Node,
        options: AutoGenerateOptions,
    ) -> ast::Node {
        let factory_store = self.factory.node_factory.store();
        if node.store_id() == factory_store.store_id() {
            let generated_node = get_node_for_generated_name_worker_in_state(&self.state, &node, 0);
            if generated_node.store_id() != factory_store.store_id()
                && let Some(source_file) = self.source_file_handle_for_node(generated_node)
            {
                return self.factory.new_generated_private_name_for_node_ex(
                    source_file.store(),
                    &node,
                    options,
                );
            }
            return self
                .factory
                .new_generated_private_name_for_factory_node_ex(&node, options);
        }
        if let Some(source_file) = self.source_file.as_ref()
            && node.store_id() == source_file.store().store_id()
        {
            return self.factory.new_generated_private_name_for_node_ex(
                source_file.store(),
                &node,
                options,
            );
        }
        if let Some(parse_source_file) = self.parse_source_file.as_ref()
            && node.store_id() == parse_source_file.store().store_id()
        {
            return self.factory.new_generated_private_name_for_node_ex(
                parse_source_file.store(),
                &node,
                options,
            );
        }
        if let Some(source_file) = self
            .source_files
            .iter()
            .find(|file| node.store_id() == file.store().store_id())
        {
            return self.factory.new_generated_private_name_for_node_ex(
                source_file.store(),
                &node,
                options,
            );
        }
        panic!(
            "emit context cannot resolve node from AST store {:?}",
            node.store_id()
        );
    }

    pub fn import_foreign_references_from_known_source_files(&mut self) {
        let source_files = self
            .source_files
            .iter()
            .map(ast::SourceFile::share_readonly)
            .collect::<Vec<_>>();
        for source_file in source_files {
            self.import_foreign_references_from_store(source_file.store());
        }
    }

    pub fn import_foreign_references_from_store(&mut self, source: &ast::AstStore) {
        let mut replacements = self
            .factory
            .node_factory
            .import_foreign_aggregate_nodes_from_store(source);
        let mut auto_generate_replacements = ast::NodeSideTable::default();
        let source_store_id = source.store_id();
        let factory_store_id = self.factory.node_factory.store().store_id();
        let (referenced_nodes, auto_generate_nodes) = {
            let state = self.state.borrow();
            let original_for_factory = state.original.store(factory_store_id);
            let auto_generate_for_factory = state.auto_generate.store(factory_store_id);
            for (cloned, original) in original_for_factory.iter() {
                if original.store_id() == source_store_id
                    && !ast::node_is_synthesized(self.factory.node_factory.store(), cloned)
                    && !auto_generate_for_factory.contains_key_same_store(cloned)
                {
                    replacements.insert(*original, cloned);
                }
            }
            let mut nodes = Vec::new();
            let mut auto_generate_nodes = Vec::new();
            state.original.for_each_value(|node| {
                if node.store_id() == source_store_id {
                    nodes.push(*node);
                }
            });
            state.text_source.for_each_value(|node| {
                if node.store_id() == source_store_id {
                    nodes.push(*node);
                }
            });
            state.assigned_name.for_each_value(|node| {
                if node.store_id() == source_store_id {
                    nodes.push(*node);
                }
            });
            state.class_this.for_each_value(|node| {
                if node.store_id() == source_store_id {
                    nodes.push(*node);
                }
            });
            state.auto_generate.for_each_value(|info| {
                if info.node.store_id() == source_store_id {
                    auto_generate_nodes.push(info.node);
                }
            });
            (nodes, auto_generate_nodes)
        };

        self.factory
            .node_factory
            .import_foreign_nodes_from_store_into(source, referenced_nodes, &mut replacements);
        self.factory
            .node_factory
            .import_foreign_nodes_from_store_into(
                source,
                auto_generate_nodes,
                &mut auto_generate_replacements,
            );

        if replacements.is_empty() && auto_generate_replacements.is_empty() {
            return;
        }

        let mut state = self.state.borrow_mut();
        let replacements_for_source = replacements.store(source_store_id);
        let auto_generate_replacements_for_source =
            auto_generate_replacements.store(source_store_id);
        let mut original_updates = Vec::new();
        {
            let original_for_factory = state.original.store(factory_store_id);
            let auto_generate_for_factory = state.auto_generate.store(factory_store_id);
            state.original.for_each(|cloned, original| {
                let is_auto_generated = if cloned.store_id() == factory_store_id {
                    auto_generate_for_factory.contains_key_same_store(cloned)
                } else {
                    state.auto_generate.contains_key(cloned)
                };
                if !is_auto_generated
                    && original.store_id() == source_store_id
                    && let Some(imported) = replacements_for_source.get_copied_same_store(*original)
                {
                    let imported_original = if imported.store_id() == factory_store_id {
                        original_for_factory
                            .get_copied_same_store(imported)
                            .unwrap_or(imported)
                    } else {
                        state
                            .original
                            .get_copied(node_key(&imported))
                            .unwrap_or(imported)
                    };
                    original_updates.push((cloned, imported_original));
                }
            });
        }
        for (cloned, imported) in original_updates {
            state.original.insert(cloned, imported);
        }
        for (source_node, imported_node) in replacements_for_source.iter() {
            let Some(emit_node) = state.with_emit_node(source_node, Clone::clone) else {
                continue;
            };
            state.ensure_emit_node(*imported_node, |target| {
                let existing_helpers = std::mem::take(&mut target.helpers);
                *target = emit_node;
                for helper in existing_helpers {
                    target.helpers = core::append_if_unique(&target.helpers, helper);
                }
            });
        }
        state.text_source.for_each_value_mut(|text_source| {
            if text_source.store_id() == source_store_id
                && let Some(imported) = replacements_for_source.get_copied_same_store(*text_source)
            {
                *text_source = imported;
            }
        });
        state.assigned_name.for_each_value_mut(|assigned_name| {
            if assigned_name.store_id() == source_store_id
                && let Some(imported) =
                    replacements_for_source.get_copied_same_store(*assigned_name)
            {
                *assigned_name = imported;
            }
        });
        state.class_this.for_each_value_mut(|class_this| {
            if class_this.store_id() == source_store_id
                && let Some(imported) = replacements_for_source.get_copied_same_store(*class_this)
            {
                *class_this = imported;
            }
        });
        state.auto_generate.for_each_value_mut(|auto_generate| {
            if auto_generate.node.store_id() == source_store_id
                && let Some(imported) =
                    auto_generate_replacements_for_source.get_copied_same_store(auto_generate.node)
            {
                auto_generate.node = imported;
            }
        });
    }

    pub fn fork(&self) -> Self {
        let state = self.state_ref();
        Self {
            factory: new_node_factory_with_state(state.clone()),
            state,
            source_file: self
                .source_file
                .as_ref()
                .map(ast::SourceFile::share_readonly),
            parse_source_file: self
                .parse_source_file
                .as_ref()
                .map(ast::SourceFile::share_readonly),
            source_files: self
                .source_files
                .iter()
                .map(ast::SourceFile::share_readonly)
                .collect(),
        }
    }

    pub(crate) fn clone_node_for_emit(&mut self, node: &ast::Node) -> ast::Node {
        let factory_store_id = self.factory.node_factory.store().store_id();
        let cloned = if node.store_id() == factory_store_id {
            let (kind, text, loc) = {
                let store = self.factory.node_factory.store();
                (store.kind(*node), store.text(*node), store.loc(*node))
            };
            let cloned = match kind {
                ast::Kind::Identifier => self.factory.node_factory.new_identifier(text),
                ast::Kind::PrivateIdentifier => {
                    self.factory.node_factory.new_private_identifier(text)
                }
                _ => panic!("emit-context same-store clone only supports generated names"),
            };
            self.factory
                .node_factory
                .place_emit_synthetic_node(cloned, loc);
            cloned
        } else {
            let source_file = self
                .source_file_handle_for_node(*node)
                .expect("emit context cannot resolve source node without a source file");
            let source = source_file.store();
            self.factory
                .node_factory
                .deep_clone_node_from_store(source, *node)
        };

        self.set_original(&cloned, node);
        if matches!(
            self.store_for_node(*node).kind(*node),
            ast::Kind::Identifier | ast::Kind::PrivateIdentifier
        ) && let Some(auto_generate) = self.get_auto_generate_info(Some(node))
        {
            self.state
                .borrow_mut()
                .auto_generate
                .insert(cloned, auto_generate);
        }
        if self.store_for_node(*node).kind(*node) == ast::Kind::Identifier
            && let Some(type_arguments) = self.get_identifier_type_arguments(node)
        {
            self.set_identifier_type_arguments(&cloned, Some(type_arguments));
        }
        cloned
    }

    pub(crate) fn import_node_for_emit(&mut self, node: &ast::Node) -> ast::Node {
        let factory_store_id = self.factory.node_factory.store().store_id();
        if node.store_id() == factory_store_id {
            return *node;
        }

        let source_file = if let Some(source_file) = self.source_file.as_ref()
            && node.store_id() == source_file.store().store_id()
        {
            source_file.share_readonly()
        } else if let Some(parse_source_file) = self.parse_source_file.as_ref()
            && node.store_id() == parse_source_file.store().store_id()
        {
            parse_source_file.share_readonly()
        } else if let Some(source_file) = self
            .source_files
            .iter()
            .find(|file| node.store_id() == file.store().store_id())
        {
            source_file.share_readonly()
        } else {
            panic!(
                "emit context cannot import node from AST store {:?}",
                node.store_id()
            );
        };
        let cloned = self
            .factory
            .node_factory
            .deep_clone_node_from_store_preserve_location(source_file.store(), *node);
        self.set_original(&cloned, node);
        cloned
    }
}

impl Default for EmitContextState {
    fn default() -> Self {
        Self {
            auto_generate: ast::NodeSideTable::default(),
            text_source: ast::NodeSideTable::default(),
            original: ast::NodeSideTable::default(),
            emit_nodes: core::LinkStore::default(),
            assigned_name: ast::NodeSideTable::default(),
            class_this: ast::NodeSideTable::default(),
            var_scope_stack: core::Stack::default(),
            let_scope_stack: core::Stack::default(),
            emit_helpers: collections::OrderedSet::default(),
        }
    }
}

impl EmitContext {
    fn store_for_source_node<'a>(
        &'a self,
        source: &'a ast::AstStore,
        node: &ast::Node,
    ) -> &'a ast::AstStore {
        let factory_store = self.factory.node_factory.store();
        if node.store_id() == factory_store.store_id() {
            factory_store
        } else {
            source
        }
    }

    fn set_factory_node_loc(&mut self, node: ast::Node, loc: core::TextRange) -> ast::Node {
        self.factory
            .node_factory
            .place_emit_synthetic_node(node, loc);
        node
    }

    //
    // Environment tracking
    //

    // Starts a new VariableEnvironment used to track hoisted `var` statements and function declarations.
    //
    // see: https://tc39.es/ecma262/#table-additional-state-components-for-ecmascript-code-execution-contexts
    //
    // NOTE: This is the equivalent of `transformContext.startLexicalEnvironment` in Strada.
    pub fn start_variable_environment(&mut self) {
        self.state
            .borrow_mut()
            .var_scope_stack
            .push(VarScope::default());
        self.start_lexical_environment();
    }

    // Ends the current VariableEnvironment, returning a list of statements that should be emitted at the start of the current scope.
    //
    // NOTE: This is the equivalent of `transformContext.endLexicalEnvironment` in Strada.
    pub fn end_variable_environment(&mut self) -> Vec<ast::Node> {
        let scope = self.state.borrow_mut().var_scope_stack.pop();
        let mut statements = Vec::new();
        if !scope.functions.is_empty() {
            statements = scope.functions.clone();
        }
        if !scope.variables.is_empty() {
            let declarations = self.factory.new_node_list(scope.variables);
            let variable_list = self
                .factory
                .new_variable_declaration_list(declarations, ast::NodeFlags::NONE);
            self.set_emit_flags(&variable_list, EF_NO_COMMENTS);
            let variable_statement = self.factory.new_variable_statement(None, variable_list);
            self.set_emit_flags(&variable_statement, EF_CUSTOM_PROLOGUE | EF_NO_COMMENTS);
            statements.push(variable_statement);
        }
        if !scope.initialization_statements.is_empty() {
            statements.extend(scope.initialization_statements);
        }
        statements.extend(self.end_lexical_environment());
        statements
    }

    // Invokes c.EndVariableEnvironment() and merges the results into `statements`
    pub fn end_and_merge_variable_environment(
        &mut self,
        source: &ast::AstStore,
        statements: &[ast::Node],
    ) -> Vec<ast::Node> {
        let (result, _) = self.end_and_merge_variable_environment_worker(source, statements);
        result
    }

    fn end_and_merge_variable_environment_worker(
        &mut self,
        source: &ast::AstStore,
        statements: &[ast::Node],
    ) -> (Vec<ast::Node>, bool) {
        // PORT NOTE: reshaped for borrowck; Go evaluates EndVariableEnvironment before MergeEnvironment.
        let environment = self.end_variable_environment();
        self.merge_environment(source, statements, &environment)
    }

    // Adds a `var` declaration to the current VariableEnvironment
    //
    // NOTE: This is the equivalent of `transformContext.hoistVariableDeclaration` in Strada.
    pub fn add_variable_declaration(&mut self, name: ast::Node) {
        let variable_declaration = self.create_hoisted_variable_declaration(name);
        let mut state = self.state.borrow_mut();
        let scope = state.var_scope_stack.peek_mut();
        scope.variables.push(variable_declaration);
        if scope.flags & EnvironmentFlags::InParameters as i32 != 0 {
            scope.flags |= EnvironmentFlags::VariablesHoistedInParameters as i32;
        }
    }

    pub fn current_variable_declaration_count(&self) -> usize {
        self.state.borrow().var_scope_stack.peek().variables.len()
    }

    pub fn insert_variable_declaration(&mut self, name: ast::Node, index: usize) {
        let variable_declaration = self.create_hoisted_variable_declaration(name);
        let mut state = self.state.borrow_mut();
        let scope = state.var_scope_stack.peek_mut();
        scope.variables.insert(index, variable_declaration);
        if scope.flags & EnvironmentFlags::InParameters as i32 != 0 {
            scope.flags |= EnvironmentFlags::VariablesHoistedInParameters as i32;
        }
    }

    fn create_hoisted_variable_declaration(&mut self, name: ast::Node) -> ast::Node {
        let variable_declaration = self
            .factory
            .new_variable_declaration(name, None, None, None);
        self.set_emit_flags(
            &variable_declaration,
            EF_NO_NESTED_SOURCE_MAPS | EF_NO_COMMENTS,
        );
        variable_declaration
    }

    pub fn reorder_current_variable_declarations(&mut self, ordered_names: &[ast::Node]) {
        if ordered_names.len() < 2 {
            return;
        }
        let mut state = self.state.borrow_mut();
        let scope = state.var_scope_stack.peek_mut();
        let mut ranked = Vec::new();
        for (slot, declaration) in scope.variables.iter().copied().enumerate() {
            let Some(name) = self.factory.node_factory.store().name(declaration) else {
                continue;
            };
            let Some(rank) = ordered_names.iter().position(|ordered| *ordered == name) else {
                continue;
            };
            ranked.push((slot, rank, declaration));
        }
        if ranked.len() < 2 {
            return;
        }
        let mut declarations = ranked.clone();
        declarations.sort_by_key(|(_, rank, _)| *rank);
        for ((slot, _, _), (_, _, declaration)) in ranked.into_iter().zip(declarations) {
            scope.variables[slot] = declaration;
        }
    }

    pub fn move_current_variable_declarations_to_end(&mut self, names: &[ast::Node]) {
        if names.is_empty() {
            return;
        }
        let mut state = self.state.borrow_mut();
        let scope = state.var_scope_stack.peek_mut();
        let mut remaining = Vec::with_capacity(scope.variables.len());
        let mut moved = Vec::new();
        for declaration in scope.variables.drain(..) {
            let Some(name) = self.factory.node_factory.store().name(declaration) else {
                remaining.push(declaration);
                continue;
            };
            if names.contains(&name) {
                moved.push(declaration);
            } else {
                remaining.push(declaration);
            }
        }
        remaining.extend(moved);
        scope.variables = remaining;
    }

    // Adds a hoisted function declaration to the current VariableEnvironment
    //
    // NOTE: This is the equivalent of `transformContext.hoistFunctionDeclaration` in Strada.
    pub fn add_hoisted_function_declaration(&mut self, node: ast::Node) {
        self.set_emit_flags(&node, EF_CUSTOM_PROLOGUE);
        self.state
            .borrow_mut()
            .var_scope_stack
            .peek_mut()
            .functions
            .push(node);
    }

    // Starts a new LexicalEnvironment used to track block-scoped `let`, `const`, and `using` declarations.
    //
    // see: https://tc39.es/ecma262/#table-additional-state-components-for-ecmascript-code-execution-contexts
    //
    // NOTE: This is the equivalent of `transformContext.startBlockScope` in Strada.
    // NOTE: This is *not* the same as `startLexicalEnvironment` in Strada as that method is incorrectly named.
    pub fn start_lexical_environment(&mut self) {
        self.state
            .borrow_mut()
            .let_scope_stack
            .push(VarScope::default());
    }

    // Ends the current EndLexicalEnvironment, returning a list of statements that should be emitted at the start of the current scope.
    //
    // NOTE: This is the equivalent of `transformContext.endLexicalEnvironment` in Strada.
    // NOTE: This is *not* the same as `endLexicalEnvironment` in Strada as that method is incorrectly named.
    pub fn end_lexical_environment(&mut self) -> Vec<ast::Node> {
        let scope = self.state.borrow_mut().let_scope_stack.pop();
        let mut statements = Vec::new();
        if !scope.variables.is_empty() {
            let declarations = self.factory.new_node_list(scope.variables);
            let variable_list = self
                .factory
                .new_variable_declaration_list(declarations, ast::NodeFlags::LET);
            let variable_statement = self.factory.new_variable_statement(None, variable_list);
            self.set_emit_flags(&variable_statement, EF_CUSTOM_PROLOGUE);
            statements.push(variable_statement);
        }
        statements
    }

    // Invokes c.EndLexicalEnvironment() and merges the results into `statements`
    pub fn end_and_merge_lexical_environment(
        &mut self,
        source: &ast::AstStore,
        statements: &[ast::Node],
    ) -> Vec<ast::Node> {
        let (result, _) = self.end_and_merge_lexical_environment_worker(source, statements);
        result
    }

    fn end_and_merge_lexical_environment_worker(
        &mut self,
        source: &ast::AstStore,
        statements: &[ast::Node],
    ) -> (Vec<ast::Node>, bool) {
        // PORT NOTE: reshaped for borrowck; Go evaluates EndLexicalEnvironment before MergeEnvironment.
        let environment = self.end_lexical_environment();
        self.merge_environment(source, statements, &environment)
    }

    // Adds a `let` declaration to the current LexicalEnvironment.
    pub fn add_lexical_declaration(&mut self, name: ast::Node) {
        let variable_declaration = self
            .factory
            .new_variable_declaration(name, None, None, None);
        self.set_emit_flags(&variable_declaration, EF_NO_NESTED_SOURCE_MAPS);
        self.state
            .borrow_mut()
            .let_scope_stack
            .peek_mut()
            .variables
            .push(variable_declaration);
    }

    // Merges declarations produced by c.EndVariableEnvironment() or c.EndLexicalEnvironment() into a slice of statements
    pub fn merge_environment(
        &mut self,
        source: &ast::AstStore,
        statements: &[ast::Node],
        declarations: &[ast::Node],
    ) -> (Vec<ast::Node>, bool) {
        if declarations.is_empty() {
            return (statements.to_vec(), false);
        }

        // When we merge new lexical statements into an existing statement list, we merge them in the following manner:
        //
        // Given:
        //
        // | Left                               | Right                               |
        // |------------------------------------|-------------------------------------|
        // | [standard prologues (left)]        | [standard prologues (right)]        |
        // | [hoisted functions (left)]         | [hoisted functions (right)]         |
        // | [hoisted variables (left)]         | [hoisted variables (right)]         |
        // | [lexical init statements (left)]   | [lexical init statements (right)]   |
        // | [other statements (left)]          |                                     |
        //
        // The resulting statement list will be:
        //
        // | Result                              |
        // |-------------------------------------|
        // | [standard prologues (right)]        |
        // | [standard prologues (left)]         |
        // | [hoisted functions (right)]         |
        // | [hoisted functions (left)]          |
        // | [hoisted variables (right)]         |
        // | [hoisted variables (left)]          |
        // | [lexical init statements (right)]   |
        // | [lexical init statements (left)]    |
        // | [other statements (left)]           |
        //
        // NOTE: It is expected that new lexical init statements must be evaluated before existing lexical init statements,
        // as the prior transformation may depend on the evaluation of the lexical init statements to be in the correct state.

        let mut changed = false;

        // find standard prologues on left in the following order: standard directives, hoisted functions, hoisted variables, other custom
        let left_standard_prologue_end =
            find_span_end(self, source, statements, ast::is_prologue_directive, 0);
        let left_hoisted_functions_end = find_span_end_with_emit_context(
            self,
            source,
            statements,
            EmitContext::is_hoisted_function_in_source,
            left_standard_prologue_end,
        );
        let left_hoisted_variables_end = find_span_end_with_emit_context(
            self,
            source,
            statements,
            EmitContext::is_hoisted_variable_statement_in_source,
            left_hoisted_functions_end,
        );

        // find standard prologues on right in the following order: standard directives, hoisted functions, hoisted variables, other custom
        let right_standard_prologue_end =
            find_span_end(self, source, declarations, ast::is_prologue_directive, 0);
        let right_hoisted_functions_end = find_span_end_with_emit_context(
            self,
            source,
            declarations,
            EmitContext::is_hoisted_function_in_source,
            right_standard_prologue_end,
        );
        let right_hoisted_variables_end = find_span_end_with_emit_context(
            self,
            source,
            declarations,
            EmitContext::is_hoisted_variable_statement_in_source,
            right_hoisted_functions_end,
        );
        let right_custom_prologue_end = find_span_end_with_emit_context(
            self,
            source,
            declarations,
            EmitContext::is_custom_prologue_in_source,
            right_hoisted_variables_end,
        );
        if right_custom_prologue_end != declarations.len() {
            panic!("Expected declarations to be valid standard or custom prologues");
        }

        let mut left = statements.to_vec();

        // splice other custom prologues from right into left
        if right_custom_prologue_end > right_hoisted_variables_end {
            left = core::splice(
                &left,
                left_hoisted_variables_end as isize,
                0,
                &declarations[right_hoisted_variables_end..right_custom_prologue_end],
            );
            changed = true;
        }

        // splice hoisted variables from right into left
        if right_hoisted_variables_end > right_hoisted_functions_end {
            left = core::splice(
                &left,
                left_hoisted_functions_end as isize,
                0,
                &declarations[right_hoisted_functions_end..right_hoisted_variables_end],
            );
            changed = true;
        }

        // splice hoisted functions from right into left
        if right_hoisted_functions_end > right_standard_prologue_end {
            left = core::splice(
                &left,
                left_standard_prologue_end as isize,
                0,
                &declarations[right_standard_prologue_end..right_hoisted_functions_end],
            );
            changed = true;
        }

        // splice standard prologues from right into left (that are not already in left)
        if right_standard_prologue_end > 0 {
            if left_standard_prologue_end == 0 {
                left = core::splice(&left, 0, 0, &declarations[..right_standard_prologue_end]);
                changed = true;
            } else {
                let mut left_prologues = collections::Set::<String>::default();
                for left_prologue in &statements[..left_standard_prologue_end] {
                    let store = self.store_for_source_node(source, left_prologue);
                    if let Some(expr) = store.expression(*left_prologue) {
                        left_prologues.add(store.text(expr));
                    }
                }
                for i in (0..right_standard_prologue_end).rev() {
                    let right_prologue = &declarations[i];
                    let store = self.store_for_source_node(source, right_prologue);
                    if let Some(expr) = store.expression(*right_prologue) {
                        let text = store.text(expr);
                        if !left_prologues.has(&text) {
                            left = core::concatenate(&[right_prologue.clone()], &left);
                            changed = true;
                        }
                    }
                }
            }
        }

        (left, changed)
    }

    pub fn is_custom_prologue(&mut self, node: &ast::Node) -> bool {
        self.emit_flags(node) & EF_CUSTOM_PROLOGUE != 0
    }

    fn is_custom_prologue_in_source(&mut self, _source: &ast::AstStore, node: &ast::Node) -> bool {
        self.is_custom_prologue(node)
    }

    pub fn is_hoisted_function(&mut self, node: &ast::Node) -> bool {
        if !self.is_custom_prologue(node) {
            return false;
        }
        let store = self.store_for_node(*node);
        ast::is_function_declaration(store, *node)
    }

    pub fn is_hoisted_variable_statement(&mut self, node: &ast::Node) -> bool {
        if !self.is_custom_prologue(node) {
            return false;
        }
        let store = self.store_for_node(*node);
        if !ast::is_variable_statement(store, *node) {
            return false;
        };
        let Some(declaration_list) = store.declaration_list(*node) else {
            return false;
        };
        store
            .declarations(declaration_list)
            .is_some_and(|declarations| {
                declarations
                    .into_iter()
                    .all(|node| is_hoisted_variable(store, &node))
            })
    }

    fn is_hoisted_function_in_source(&mut self, source: &ast::AstStore, node: &ast::Node) -> bool {
        if !self.is_custom_prologue(node) {
            return false;
        }
        let store = self.store_for_source_node(source, node);
        ast::is_function_declaration(store, *node)
    }

    fn is_hoisted_variable_statement_in_source(
        &mut self,
        source: &ast::AstStore,
        node: &ast::Node,
    ) -> bool {
        if !self.is_custom_prologue(node) {
            return false;
        }
        let store = self.store_for_source_node(source, node);
        if !ast::is_variable_statement(store, *node) {
            return false;
        }
        let Some(declaration_list) = store.declaration_list(*node) else {
            return false;
        };
        store
            .declarations(declaration_list)
            .is_some_and(|declarations| {
                declarations
                    .into_iter()
                    .all(|node| is_hoisted_variable(store, &node))
            })
    }
    //
    // Name Generation
    //

    // Gets whether a given name has an associated AutoGenerateInfo entry.
    pub fn has_auto_generate_info(&self, node: Option<&ast::Node>) -> bool {
        node.is_some_and(|node| {
            self.state
                .borrow()
                .auto_generate
                .contains_key(node_key(node))
        })
    }

    // Gets the associated AutoGenerateInfo entry for a given name.
    pub fn get_auto_generate_info(&self, name: Option<&ast::Node>) -> Option<AutoGenerateInfo> {
        name.and_then(|name| self.state.borrow().auto_generate.get_cloned(node_key(name)))
    }

    pub fn set_auto_generate_info(&mut self, name: &ast::Node, auto_generate: AutoGenerateInfo) {
        self.state
            .borrow_mut()
            .auto_generate
            .insert(node_key(name), auto_generate);
    }

    pub fn copy_auto_generate_info(&mut self, source: &ast::Node, target: &ast::Node) {
        if let Some(mut auto_generate) = self.get_auto_generate_info(Some(source)) {
            if auto_generate.node == *source {
                auto_generate.node = *target;
            }
            self.set_auto_generate_info(target, auto_generate);
        }
    }

    pub fn copy_emit_metadata_for_cloned_tree(
        &mut self,
        source_store: &ast::AstStore,
        source: ast::Node,
        target: ast::Node,
    ) {
        let flags = self.emit_flags(&source);
        if flags != EF_NONE {
            self.set_emit_flags(&target, flags);
        }

        let leading_comments = self.get_synthetic_leading_comments(&source);
        if !leading_comments.is_empty() {
            self.set_synthetic_leading_comments(&target, leading_comments);
        }

        let trailing_comments = self.get_synthetic_trailing_comments(&source);
        if !trailing_comments.is_empty() {
            self.set_synthetic_trailing_comments(&target, trailing_comments);
        }

        if matches!(
            source_store.kind(source),
            ast::Kind::Identifier | ast::Kind::PrivateIdentifier
        ) {
            self.copy_auto_generate_info(&source, &target);
        }

        let source_children = {
            let mut children = Vec::new();
            let _ = source_store.for_each_child(source, |child| {
                if let Some(child) = child {
                    children.push(child);
                }
                ControlFlow::Continue(())
            });
            children
        };
        let target_children = {
            let target_store = self.factory.node_factory.store();
            let mut children = Vec::new();
            let _ = target_store.for_each_child(target, |child| {
                if let Some(child) = child {
                    children.push(child);
                }
                ControlFlow::Continue(())
            });
            children
        };

        for (source_child, target_child) in source_children.into_iter().zip(target_children) {
            let same_kind = {
                let target_store = self.factory.node_factory.store();
                source_store.kind(source_child) == target_store.kind(target_child)
            };
            if same_kind {
                self.copy_emit_metadata_for_cloned_tree(source_store, source_child, target_child);
            }
        }
    }

    // Walks the associated AutoGenerateInfo entries of a name to find the root Node from which the name should be generated.
    pub fn get_node_for_generated_name(&self, name: &ast::Node) -> ast::Node {
        let auto_generate = self.state.borrow().auto_generate.get_cloned(node_key(name));
        if let Some(auto_generate) = auto_generate {
            if auto_generate.flags.is_node() {
                return self
                    .get_node_for_generated_name_worker(&auto_generate.node, auto_generate.id);
            }
        }
        name.clone()
    }

    pub fn get_node_for_generated_name_worker(
        &self,
        node: &ast::Node,
        auto_generate_id: AutoGenerateId,
    ) -> ast::Node {
        let mut node = node.clone();
        let mut original = self.original(&node);
        while let Some(next_original) = original {
            node = next_original;
            if ast::is_member_name(self.store_for_node(node), node) {
                // if "node" is a different generated name (having a different "autoGenerateId"), use it and stop traversing.
                let auto_generate = self
                    .state
                    .borrow()
                    .auto_generate
                    .get_cloned(node_key(&node));
                if auto_generate.is_none()
                    || (auto_generate.as_ref().unwrap().flags.is_node()
                        && auto_generate.as_ref().unwrap().id != auto_generate_id)
                {
                    break;
                }
                if auto_generate.as_ref().unwrap().flags.is_node() {
                    original = Some(auto_generate.unwrap().node);
                    continue;
                }
            }
            original = self.original(&node);
        }
        node
    }

    //
    // Original Node Tracking
    //

    // Sets the original node for a given node.
    //
    // NOTE: This is the equivalent to `setOriginalNode` in Strada.
    pub fn set_original(&mut self, node: &ast::Node, original: &ast::Node) {
        self.set_original_ex(node, original, false)
    }

    pub fn unset_original(&mut self, node: &ast::Node) {
        self.state.borrow_mut().original.remove(node_key(node));
    }

    pub fn set_original_ex(
        &mut self,
        node: &ast::Node,
        original: &ast::Node,
        allow_overwrite: bool,
    ) {
        set_original_in_state_ex(&self.state, node, original, allow_overwrite)
    }

    // Gets the original node for a given node.
    //
    // NOTE: This is the equivalent to reading `node.original` in Strada.
    pub fn original(&self, node: &ast::Node) -> Option<ast::Node> {
        original_in_state(&self.state, node)
    }

    // Gets the most original node associated with this node by walking Original pointers.
    //
    // NOTE: This method is analogous to `getOriginalNode` in the old compiler, but the name has changed to avoid accidental
    // conflation with `SetOriginal`/`Original`
    pub fn most_original(&self, node: &ast::Node) -> ast::Node {
        let mut node = node.clone();
        while let Some(original) = self.original(&node) {
            node = original;
        }
        node
    }

    // Gets the original parse tree node for a given node.
    //
    // NOTE: This is the equivalent to `getParseTreeNode` in Strada.
    pub fn parse_node(&self, node: &ast::Node) -> Option<ast::Node> {
        let node = self.most_original(node);
        if ast::is_parse_tree_node(self.store_for_node(node), node) {
            return Some(node);
        }
        None
    }

    //
    // Emit-related Data
    //

    pub fn emit_flags(&mut self, node: &ast::Node) -> EmitFlags {
        self.state
            .borrow()
            .with_emit_node(node_key(node), |emit_node| emit_node.emit_flags)
            .unwrap_or(EF_NONE)
    }

    pub fn set_emit_flags(&mut self, node: &ast::Node, flags: EmitFlags) {
        let state = self.state.borrow();
        state.ensure_emit_node(node_key(node), |emit_node| emit_node.emit_flags = flags);
    }

    pub fn mark_emit_node(&mut self, node: &ast::Node, flags: EmitFlags) {
        let state = self.state.borrow();
        state.ensure_emit_node(node_key(node), |emit_node| emit_node.emit_flags |= flags);
    }

    pub fn add_emit_flags(&mut self, node: &ast::Node, flags: EmitFlags) {
        self.mark_emit_node(node, flags);
    }

    // Gets the range to use for a node when emitting comments.
    pub fn comment_range(&mut self, node: &ast::Node) -> core::TextRange {
        if let Some(emit_node) = self
            .state
            .borrow()
            .with_emit_node(node_key(node), Clone::clone)
        {
            if emit_node.flags & EmitNodeFlags::HAS_COMMENT_RANGE as u32 != 0 {
                return emit_node.comment_range;
            }
        }
        self.store_for_node(*node).loc(*node)
    }

    pub fn comment_range_with_fallback_loc(
        &mut self,
        node: &ast::Node,
        fallback_loc: core::TextRange,
    ) -> core::TextRange {
        if let Some(emit_node) = self
            .state
            .borrow()
            .with_emit_node(node_key(node), Clone::clone)
        {
            if emit_node.flags & EmitNodeFlags::HAS_COMMENT_RANGE as u32 != 0 {
                return emit_node.comment_range;
            }
        }
        fallback_loc
    }

    // Sets the range to use for a node when emitting comments.
    pub fn set_comment_range(&mut self, node: &ast::Node, loc: core::TextRange) {
        let state = self.state.borrow();
        state.ensure_emit_node(node_key(node), |emit_node| {
            emit_node.comment_range = loc;
            emit_node.flags |= EmitNodeFlags::HAS_COMMENT_RANGE as u32;
        });
    }

    // Sets the range to use for a node when emitting comments.
    pub fn assign_comment_range(&mut self, to: &ast::Node, from: &ast::Node) {
        // PORT NOTE: reshaped for borrowck; preserve Go's left-to-right argument evaluation.
        let comment_range = self.comment_range(from);
        self.set_comment_range(to, comment_range)
    }

    pub fn assign_comment_range_from_source_loc(
        &mut self,
        to: &ast::Node,
        from: &ast::Node,
        from_loc: core::TextRange,
    ) {
        let comment_range = self.comment_range_with_fallback_loc(from, from_loc);
        self.set_comment_range(to, comment_range)
    }

    // Gets the range to use for a node when emitting source maps.
    pub fn source_map_range(&mut self, node: &ast::Node) -> core::TextRange {
        if let Some(emit_node) = self
            .state
            .borrow()
            .with_emit_node(node_key(node), Clone::clone)
        {
            if emit_node.flags & EmitNodeFlags::HAS_SOURCE_MAP_RANGE as u32 != 0 {
                return emit_node.source_map_range;
            }
        }
        self.store_for_node(*node).loc(*node)
    }

    // Sets the range to use for a node when emitting source maps.
    pub fn set_source_map_range(&mut self, node: &ast::Node, loc: core::TextRange) {
        let state = self.state.borrow();
        state.ensure_emit_node(node_key(node), |emit_node| {
            emit_node.source_map_range = loc;
            emit_node.flags |= EmitNodeFlags::HAS_SOURCE_MAP_RANGE as u32;
        });
    }

    // Sets the range to use for a node when emitting source maps.
    pub fn assign_source_map_range(&mut self, to: &ast::Node, from: &ast::Node) {
        // PORT NOTE: reshaped for borrowck; preserve Go's left-to-right argument evaluation.
        let source_map_range = self.source_map_range(from);
        self.set_source_map_range(to, source_map_range)
    }

    // Sets the range to use for a node when emitting comments and source maps.
    pub fn assign_comment_and_source_map_ranges(&mut self, to: &ast::Node, from: &ast::Node) {
        let comment_range = self.comment_range(from);
        let source_map_range = self.source_map_range(from);
        let state = self.state.borrow();
        state.ensure_emit_node(node_key(to), |emit_node| {
            emit_node.comment_range = comment_range;
            emit_node.source_map_range = source_map_range;
            emit_node.flags |= EmitNodeFlags::HAS_COMMENT_RANGE as u32
                | EmitNodeFlags::HAS_SOURCE_MAP_RANGE as u32;
        });
    }

    // Gets the range for a token of a node when emitting source maps.
    pub fn token_source_map_range(
        &mut self,
        node: &ast::Node,
        kind: ast::Kind,
    ) -> Option<core::TextRange> {
        self.state
            .borrow()
            .with_emit_node(node_key(node), |emit_node| {
                emit_node.token_source_map_ranges.get(&kind).copied()
            })
            .flatten()
    }

    // Sets the range for a token of a node when emitting source maps.
    pub fn set_token_source_map_range(
        &mut self,
        node: &ast::Node,
        kind: ast::Kind,
        loc: core::TextRange,
    ) {
        let state = self.state.borrow();
        state.ensure_emit_node(node_key(node), |emit_node| {
            emit_node.token_source_map_ranges.insert(kind, loc);
        });
    }

    pub fn assigned_name(&self, node: &ast::Node) -> Option<ast::Node> {
        self.state
            .borrow_mut()
            .assigned_name
            .get_copied(node_key(node))
    }

    pub fn text_source(&self, node: &ast::Node) -> Option<ast::Node> {
        self.state.borrow().text_source.get_copied(node_key(node))
    }

    pub fn set_assigned_name(&mut self, node: &ast::Node, name: &ast::Node) {
        self.state
            .borrow_mut()
            .assigned_name
            .insert(node_key(node), *name);
    }

    pub fn class_this(&self, node: &ast::Node) -> Option<ast::Node> {
        self.state
            .borrow_mut()
            .class_this
            .get_copied(node_key(node))
    }

    pub fn set_class_this(&mut self, node: &ast::Node, class_this: &ast::Node) {
        self.state
            .borrow_mut()
            .class_this
            .insert(node_key(node), *class_this);
    }

    pub fn request_emit_helper(&mut self, helper: &EmitHelper) {
        if helper.scoped {
            panic!("Cannot request a scoped emit helper")
        }
        for dependency in helper.dependencies.iter().copied() {
            self.request_emit_helper(dependency);
        }
        self.state.borrow_mut().emit_helpers.add(helper_key(helper));
    }

    pub fn read_emit_helpers(&mut self) -> Vec<usize> {
        let helpers = self
            .state
            .borrow_mut()
            .emit_helpers
            .values()
            .cloned()
            .collect();
        self.state.borrow_mut().emit_helpers.clear();
        helpers
    }

    pub fn add_requested_emit_helpers(&mut self, node: &ast::Node) {
        let helpers = self.read_emit_helpers();
        if helpers.is_empty() {
            return;
        }
        let state = self.state.borrow();
        state.ensure_emit_node(node_key(node), |emit_node| {
            for helper in helpers {
                emit_node.helpers = core::append_if_unique(&emit_node.helpers, helper);
            }
        });
    }

    pub fn add_emit_helper(&mut self, node: &ast::Node, helper: &[EmitHelper]) {
        let state = self.state.borrow();
        state.ensure_emit_node(node_key(node), |emit_node| {
            for h in helper {
                emit_node.helpers = core::append_if_unique(&emit_node.helpers, helper_key(h));
            }
        });
    }

    pub fn move_emit_helpers(
        &mut self,
        source: &ast::Node,
        target: &ast::Node,
        predicate: fn(&EmitHelper) -> bool,
    ) {
        let target_key = node_key(target);
        let source_key = node_key(source);
        let mut source_emit_helpers = {
            let state = self.state.borrow();
            let Some(helpers) = state.with_emit_node(source_key, |source_emit_node| {
                source_emit_node.helpers.clone()
            }) else {
                return;
            };
            if helpers.is_empty() {
                return;
            }
            helpers
        };
        let mut helpers_removed = 0;
        let mut helpers_to_move = Vec::new();
        for i in 0..source_emit_helpers.len() {
            let helper = source_emit_helpers[i];
            if predicate(helper_from_key(helper)) {
                helpers_removed += 1;
                helpers_to_move.push(helper);
            } else if helpers_removed > 0 {
                source_emit_helpers[i - helpers_removed] = helper;
            }
        }
        if !helpers_to_move.is_empty() {
            let state = self.state.borrow();
            state.ensure_emit_node(target_key, |target_emit_node| {
                for helper in helpers_to_move {
                    target_emit_node.helpers =
                        core::append_if_unique(&target_emit_node.helpers, helper);
                }
            });
        }
        if helpers_removed > 0 {
            source_emit_helpers.truncate(source_emit_helpers.len() - helpers_removed);
            let state = self.state.borrow();
            state.ensure_emit_node(source_key, |source_emit_node| {
                source_emit_node.helpers = source_emit_helpers;
            });
        }
    }

    pub fn get_emit_helpers(&mut self, node: &ast::Node) -> Vec<usize> {
        self.state
            .borrow()
            .with_emit_node(node_key(node), |emit_node| emit_node.helpers.clone())
            .unwrap_or_default()
    }

    pub fn has_recorded_external_helpers(&mut self, node: &ast::SourceFile) -> bool {
        if let Some(parse_node) = self.parse_node(&node.as_node()) {
            if let Some(has_helpers) =
                self.state
                    .borrow()
                    .with_emit_node(node_key(&parse_node), |emit_node| {
                        emit_node.external_helpers_module_name.is_some()
                            || emit_node.emit_flags & EF_EXTERNAL_HELPERS != 0
                    })
            {
                return has_helpers;
            }
        }
        false
    }

    pub fn set_external_helpers(&mut self, node: &ast::SourceFile) {
        let Some(parse_node) = self.parse_node(&node.as_node()) else {
            panic!(
                "Node must be a parse tree node or have an Original pointer to a parse tree node."
            );
        };
        self.mark_emit_node(&parse_node, EF_EXTERNAL_HELPERS);
    }

    pub fn get_external_helpers_module_name(
        &mut self,
        node: &ast::SourceFile,
    ) -> Option<ast::Node> {
        if let Some(parse_node) = self.parse_node(&node.as_node()) {
            if let Some(module_name) = self
                .state
                .borrow()
                .with_emit_node(node_key(&parse_node), |emit_node| {
                    emit_node.external_helpers_module_name
                })
            {
                return module_name;
            }
        }
        None
    }

    pub fn set_external_helpers_module_name(&mut self, node: &ast::SourceFile, name: &ast::Node) {
        let Some(parse_node) = self.parse_node(&node.as_node()) else {
            panic!(
                "Node must be a parse tree node or have an Original pointer to a parse tree node."
            );
        };
        let state = self.state.borrow();
        state.ensure_emit_node(node_key(&parse_node), |emit_node| {
            emit_node.external_helpers_module_name = Some(*name);
        });
    }

    pub fn add_initialization_statement(&mut self, node: ast::Node) {
        self.mark_emit_node(&node, EF_CUSTOM_PROLOGUE);
        self.state
            .borrow_mut()
            .var_scope_stack
            .peek_mut()
            .initialization_statements
            .push(node);
    }

    //
    // Visitor Hooks
    //

    pub fn begin_visit_parameters(&mut self) -> i32 {
        self.start_variable_environment();
        let mut state = self.state.borrow_mut();
        let scope = state.var_scope_stack.peek_mut();
        let old_flags = scope.flags;
        scope.flags |= EnvironmentFlags::InParameters as i32;
        old_flags
    }

    pub fn finish_visit_parameters(
        &mut self,
        old_flags: i32,
        parameters: Vec<ast::Node>,
        changed: bool,
    ) -> (Vec<ast::Node>, bool) {
        let should_add_default_assignments = {
            let state = self.state.borrow();
            let scope = state.var_scope_stack.peek();
            scope.flags & EnvironmentFlags::VariablesHoistedInParameters as i32 != 0
        };

        let (parameters, changed) = if should_add_default_assignments {
            self.add_default_value_assignments_if_needed(parameters)
        } else {
            (parameters, changed)
        };

        self.state.borrow_mut().var_scope_stack.peek_mut().flags = old_flags;
        (parameters, changed)
    }

    fn add_default_value_assignments_if_needed(
        &mut self,
        parameters: Vec<ast::Node>,
    ) -> (Vec<ast::Node>, bool) {
        let mut result = parameters.clone();
        let mut changed = false;
        for (index, parameter) in parameters.into_iter().enumerate() {
            let updated = self.add_default_value_assignment_if_needed(parameter);
            if updated != parameter {
                result[index] = updated;
                changed = true;
            }
        }
        (result, changed)
    }

    fn add_default_value_assignment_if_needed(&mut self, parameter: ast::Node) -> ast::Node {
        let store = self.store_for_node(parameter);
        assert_eq!(
            store.kind(parameter),
            ast::Kind::Parameter,
            "VisitParameters must receive Parameter nodes"
        );

        let dot_dot_dot_token = store.dot_dot_dot_token(parameter);
        if dot_dot_dot_token.is_some() {
            return parameter;
        }

        let name = store
            .name(parameter)
            .expect("parameter declaration should have a name");
        if ast::is_binding_pattern(self.store_for_node(name), name) {
            return self.add_default_value_assignment_for_binding_pattern(parameter);
        }

        if store.initializer(parameter).is_some() {
            return self.add_default_value_assignment_for_initializer(parameter);
        }

        parameter
    }

    fn add_default_value_assignment_for_binding_pattern(
        &mut self,
        parameter: ast::Node,
    ) -> ast::Node {
        self.assert_factory_node(parameter, "parameter default rewriting");
        let (modifiers, dot_dot_dot_token, name, question_token, type_node, initializer) = {
            let store = self.factory.node_factory.store();
            (
                store.modifiers(parameter).map(|list| {
                    let nodes = list.nodes();
                    (
                        nodes.loc(),
                        nodes.range(),
                        nodes.iter().collect::<Vec<_>>(),
                        list.modifier_flags(),
                    )
                }),
                store.dot_dot_dot_token(parameter),
                store
                    .name(parameter)
                    .expect("parameter declaration should have a name"),
                store.question_token(parameter),
                store.type_node(parameter),
                store.initializer(parameter),
            )
        };

        let modifiers = modifiers.map(|(loc, range, nodes, flags)| {
            self.factory
                .node_factory
                .new_modifier_list(loc, range, nodes, flags)
        });
        let parameter_name = self.new_generated_name_for_node(parameter);
        let init_node = if let Some(initializer) = initializer {
            let generated = self.new_generated_name_for_node(parameter);
            let void_zero = self.factory.new_void_zero_expression();
            let condition = self
                .factory
                .new_strict_equality_expression(generated, void_zero);
            let question = self
                .factory
                .node_factory
                .new_token(ast::Kind::QuestionToken);
            let colon = self.factory.node_factory.new_token(ast::Kind::ColonToken);
            let alternate = self.new_generated_name_for_node(parameter);
            self.factory.node_factory.new_conditional_expression(
                condition,
                question,
                initializer,
                colon,
                alternate,
            )
        } else {
            self.new_generated_name_for_node(parameter)
        };

        let declaration = self
            .factory
            .node_factory
            .new_variable_declaration(name, None, type_node, init_node);
        let declarations = self.factory.new_node_list([declaration]);
        let declaration_list = self
            .factory
            .node_factory
            .new_variable_declaration_list(declarations, ast::NodeFlags::NONE);
        let statement = self
            .factory
            .node_factory
            .new_variable_statement(None, declaration_list);
        self.add_initialization_statement(statement);

        self.factory.node_factory.update_parameter_declaration(
            parameter,
            modifiers,
            dot_dot_dot_token,
            parameter_name,
            question_token,
            type_node,
            None,
        )
    }

    fn add_default_value_assignment_for_initializer(&mut self, parameter: ast::Node) -> ast::Node {
        self.assert_factory_node(parameter, "parameter default rewriting");
        let (modifiers, dot_dot_dot_token, name, question_token, type_node, initializer, loc) = {
            let store = self.factory.node_factory.store();
            (
                store.modifiers(parameter).map(|list| {
                    let nodes = list.nodes();
                    (
                        nodes.loc(),
                        nodes.range(),
                        nodes.iter().collect::<Vec<_>>(),
                        list.modifier_flags(),
                    )
                }),
                store.dot_dot_dot_token(parameter),
                store
                    .name(parameter)
                    .expect("parameter declaration should have a name"),
                store.question_token(parameter),
                store.type_node(parameter),
                store
                    .initializer(parameter)
                    .expect("parameter initializer should exist"),
                store.loc(parameter),
            )
        };

        let modifiers = modifiers.map(|(loc, range, nodes, flags)| {
            self.factory
                .node_factory
                .new_modifier_list(loc, range, nodes, flags)
        });
        self.mark_emit_node(&initializer, EF_NO_SOURCE_MAP | EF_NO_COMMENTS);
        let name_clone = self.clone_node_for_emit(&name);
        self.mark_emit_node(&name_clone, EF_NO_SOURCE_MAP);
        let mut init_assignment = self
            .factory
            .new_assignment_expression(name_clone, initializer);
        init_assignment = self.set_factory_node_loc(init_assignment, loc);
        self.mark_emit_node(&init_assignment, EF_NO_COMMENTS);

        let expression_statement = self
            .factory
            .node_factory
            .new_expression_statement(init_assignment);
        let init_statements = self.factory.new_node_list([expression_statement]);
        let mut init_block = self.factory.node_factory.new_block(init_statements, false);
        init_block = self.set_factory_node_loc(init_block, loc);
        self.mark_emit_node(
            &init_block,
            EF_SINGLE_LINE | EF_NO_TRAILING_SOURCE_MAP | EF_NO_TOKEN_SOURCE_MAPS | EF_NO_COMMENTS,
        );

        let name_check = self.clone_node_for_emit(&name);
        let type_check = self.factory.new_type_check(&name_check, "undefined");
        let if_statement = self
            .factory
            .node_factory
            .new_if_statement(type_check, init_block, None);
        self.add_initialization_statement(if_statement);

        self.factory.node_factory.update_parameter_declaration(
            parameter,
            modifiers,
            dot_dot_dot_token,
            name,
            question_token,
            type_node,
            None,
        )
    }

    pub fn finish_visit_function_body(&mut self, updated: Option<ast::Node>) -> Option<ast::Node> {
        let declarations = self.end_variable_environment();
        if declarations.is_empty() {
            return updated;
        }

        let Some(updated) = updated else {
            let statements = self.factory.new_node_list(declarations);
            return Some(self.factory.node_factory.new_block(statements, true));
        };

        let store = self.store_for_node(updated);
        if !ast::is_block(store, updated) {
            let return_statement = self.factory.node_factory.new_return_statement(updated);
            let (statements, _) =
                self.merge_environment_for_nodes(&[return_statement], &declarations);
            let statements = self.factory.new_node_list(statements);
            return Some(self.factory.node_factory.new_block(statements, false));
        }

        let (source_store_id, statements, loc, range, has_trailing_comma, multi_line) = {
            let store = self.store_for_node(updated);
            let statements = store
                .statements(updated)
                .expect("block should have statements");
            (
                store.store_id(),
                statements.iter().collect::<Vec<_>>(),
                statements.loc(),
                statements.range(),
                statements.has_trailing_comma(),
                store.multi_line(updated).unwrap_or(false),
            )
        };
        let (statements, changed) = self.merge_environment_for_nodes(&statements, &declarations);
        if !changed {
            return Some(updated);
        }
        Some(self.update_block_from_parts(
            updated,
            source_store_id,
            loc,
            range,
            statements,
            has_trailing_comma,
            multi_line,
        ))
    }

    pub fn begin_visit_iteration_body(&mut self) {
        self.start_lexical_environment();
    }

    pub fn finish_visit_iteration_body(&mut self, updated: Option<ast::Node>) -> Option<ast::Node> {
        let updated = updated.expect("Expected visitor to return a statement.");
        let declarations = self.end_lexical_environment();
        if declarations.is_empty() {
            return Some(updated);
        }

        let store = self.store_for_node(updated);
        if ast::is_block(store, updated) {
            let (source_store_id, statements, loc, range, has_trailing_comma, multi_line) = {
                let store = self.store_for_node(updated);
                let statements = store
                    .statements(updated)
                    .expect("block should have statements");
                (
                    store.store_id(),
                    statements.iter().collect::<Vec<_>>(),
                    statements.loc(),
                    statements.range(),
                    statements.has_trailing_comma(),
                    store.multi_line(updated).unwrap_or(false),
                )
            };
            let statements = declarations
                .into_iter()
                .chain(statements)
                .collect::<Vec<_>>();
            return Some(self.update_block_from_parts(
                updated,
                source_store_id,
                loc,
                range,
                statements,
                has_trailing_comma,
                multi_line,
            ));
        }

        let statements = declarations
            .into_iter()
            .chain([updated])
            .collect::<Vec<_>>();
        let statements = self.factory.new_node_list(statements);
        Some(self.factory.node_factory.new_block(statements, true))
    }

    pub fn finish_visit_embedded_statement(
        &mut self,
        original: &ast::Node,
        embedded_statement: Option<ast::Node>,
    ) -> Option<ast::Node> {
        let embedded_statement = embedded_statement?;
        if ast::is_not_emitted_statement(
            self.store_for_node(embedded_statement),
            embedded_statement,
        ) {
            let statement = self.factory.node_factory.new_empty_statement();
            let loc = self.store_for_node(*original).loc(*original);
            self.set_factory_node_loc(statement, loc);
            self.set_original(&statement, original);
            self.assign_comment_range(&statement, original);
            return Some(statement);
        }
        Some(embedded_statement)
    }

    fn merge_environment_for_nodes(
        &mut self,
        statements: &[ast::Node],
        declarations: &[ast::Node],
    ) -> (Vec<ast::Node>, bool) {
        if declarations.is_empty() {
            return (statements.to_vec(), false);
        }

        let left_standard_prologue_end =
            find_span_end_resolved(self, statements, ast::is_prologue_directive, 0);
        let left_hoisted_functions_end = find_span_end_with_emit_context_resolved(
            self,
            statements,
            EmitContext::is_hoisted_function_resolved,
            left_standard_prologue_end,
        );
        let left_hoisted_variables_end = find_span_end_with_emit_context_resolved(
            self,
            statements,
            EmitContext::is_hoisted_variable_statement_resolved,
            left_hoisted_functions_end,
        );

        let right_standard_prologue_end =
            find_span_end_resolved(self, declarations, ast::is_prologue_directive, 0);
        let right_hoisted_functions_end = find_span_end_with_emit_context_resolved(
            self,
            declarations,
            EmitContext::is_hoisted_function_resolved,
            right_standard_prologue_end,
        );
        let right_hoisted_variables_end = find_span_end_with_emit_context_resolved(
            self,
            declarations,
            EmitContext::is_hoisted_variable_statement_resolved,
            right_hoisted_functions_end,
        );
        let right_custom_prologue_end = find_span_end_with_emit_context_resolved(
            self,
            declarations,
            EmitContext::is_custom_prologue_resolved,
            right_hoisted_variables_end,
        );
        if right_custom_prologue_end != declarations.len() {
            panic!("Expected declarations to be valid standard or custom prologues");
        }

        let mut changed = false;
        let mut left = statements.to_vec();

        if right_custom_prologue_end > right_hoisted_variables_end {
            left = core::splice(
                &left,
                left_hoisted_variables_end as isize,
                0,
                &declarations[right_hoisted_variables_end..right_custom_prologue_end],
            );
            changed = true;
        }
        if right_hoisted_variables_end > right_hoisted_functions_end {
            left = core::splice(
                &left,
                left_hoisted_functions_end as isize,
                0,
                &declarations[right_hoisted_functions_end..right_hoisted_variables_end],
            );
            changed = true;
        }
        if right_hoisted_functions_end > right_standard_prologue_end {
            left = core::splice(
                &left,
                left_standard_prologue_end as isize,
                0,
                &declarations[right_standard_prologue_end..right_hoisted_functions_end],
            );
            changed = true;
        }
        if right_standard_prologue_end > 0 {
            if left_standard_prologue_end == 0 {
                left = core::splice(&left, 0, 0, &declarations[..right_standard_prologue_end]);
                changed = true;
            } else {
                let mut left_prologues = collections::Set::<String>::default();
                for left_prologue in &statements[..left_standard_prologue_end] {
                    let store = self.store_for_node(*left_prologue);
                    if let Some(expr) = store.expression(*left_prologue) {
                        left_prologues.add(store.text(expr));
                    }
                }
                for i in (0..right_standard_prologue_end).rev() {
                    let right_prologue = &declarations[i];
                    let store = self.store_for_node(*right_prologue);
                    if let Some(expr) = store.expression(*right_prologue) {
                        let text = store.text(expr);
                        if !left_prologues.has(&text) {
                            left = core::concatenate(&[*right_prologue], &left);
                            changed = true;
                        }
                    }
                }
            }
        }

        (left, changed)
    }

    pub fn merge_environment_for_resolved_nodes(
        &mut self,
        statements: &[ast::Node],
        declarations: &[ast::Node],
    ) -> (Vec<ast::Node>, bool) {
        self.merge_environment_for_nodes(statements, declarations)
    }

    fn update_block_from_parts(
        &mut self,
        updated: ast::Node,
        source_store_id: ast::StoreId,
        loc: core::TextRange,
        range: core::TextRange,
        statements: Vec<ast::Node>,
        has_trailing_comma: bool,
        multi_line: bool,
    ) -> ast::Node {
        let statements = self.factory.node_factory.new_node_list_with_trailing_comma(
            loc,
            range,
            statements,
            has_trailing_comma,
        );
        if source_store_id == self.factory.node_factory.store().store_id() {
            self.factory
                .node_factory
                .update_block(updated, statements, multi_line)
        } else {
            let source_file = self
                .source_file
                .as_ref()
                .expect("emit context cannot resolve source node without a source file")
                .share_readonly();
            let source = source_file.store();
            self.factory
                .node_factory
                .update_block_from_store(source, updated, statements, multi_line)
        }
    }

    fn is_custom_prologue_resolved(&mut self, node: &ast::Node) -> bool {
        self.is_custom_prologue(node)
    }

    fn is_hoisted_function_resolved(&mut self, node: &ast::Node) -> bool {
        self.is_hoisted_function(node)
    }

    fn is_hoisted_variable_statement_resolved(&mut self, node: &ast::Node) -> bool {
        self.is_hoisted_variable_statement(node)
    }

    fn assert_factory_node(&self, node: ast::Node, operation: &str) {
        assert_eq!(
            node.store_id(),
            self.factory.node_factory.store().store_id(),
            "{operation} requires nodes imported into the emit factory store"
        );
    }

    pub fn is_call_to_helper(&mut self, first_segment: &ast::Node, helper_name: &str) -> bool {
        let store = self.store_for_node(*first_segment);
        if !ast::is_call_expression(store, *first_segment) {
            return false;
        }
        let Some(expression) = store.expression(*first_segment) else {
            return false;
        };
        let helper_text = store.text(expression);
        ast::is_identifier(store, expression)
            && self.emit_flags(&expression) & EF_HELPER_NAME != 0
            && helper_text == helper_name
    }

    pub fn set_synthetic_leading_comments(
        &mut self,
        node: &ast::Node,
        comments: Vec<SynthesizedComment>,
    ) {
        let state = self.state.borrow();
        state.ensure_emit_node(node_key(node), |emit_node| {
            emit_node.leading_comments = comments;
        });
    }

    pub fn add_synthetic_leading_comment(
        &mut self,
        node: &ast::Node,
        kind: ast::Kind,
        text: String,
        has_trailing_new_line: bool,
    ) {
        let state = self.state.borrow();
        state.ensure_emit_node(node_key(node), |emit_node| {
            emit_node.leading_comments.push(SynthesizedComment {
                kind,
                loc: core::TextRange::new(-1, -1),
                has_leading_new_line: false,
                has_trailing_new_line,
                text,
            });
        });
    }

    pub fn get_synthetic_leading_comments(&mut self, node: &ast::Node) -> Vec<SynthesizedComment> {
        if self.state.borrow().emit_nodes.has(&node_key(node)) {
            let state = self.state.borrow();
            return state
                .with_emit_node(node_key(node), |emit_node| {
                    emit_node.leading_comments.clone()
                })
                .unwrap_or_default();
        }
        Vec::new()
    }

    pub fn set_synthetic_trailing_comments(
        &mut self,
        node: &ast::Node,
        comments: Vec<SynthesizedComment>,
    ) {
        let state = self.state.borrow();
        state.ensure_emit_node(node_key(node), |emit_node| {
            emit_node.trailing_comments = comments;
        });
    }

    pub fn add_synthetic_trailing_comment(
        &mut self,
        node: &ast::Node,
        kind: ast::Kind,
        text: String,
        has_trailing_new_line: bool,
    ) {
        let state = self.state.borrow();
        state.ensure_emit_node(node_key(node), |emit_node| {
            emit_node.trailing_comments.push(SynthesizedComment {
                kind,
                loc: core::TextRange::new(-1, -1),
                has_leading_new_line: false,
                has_trailing_new_line,
                text,
            });
        });
    }

    pub fn get_synthetic_trailing_comments(&mut self, node: &ast::Node) -> Vec<SynthesizedComment> {
        if self.state.borrow().emit_nodes.has(&node_key(node)) {
            let state = self.state.borrow();
            return state
                .with_emit_node(node_key(node), |emit_node| {
                    emit_node.trailing_comments.clone()
                })
                .unwrap_or_default();
        }
        Vec::new()
    }

    // SetTypeNode stores the original type node on a name node when the type is erased,
    // so the emitter can use the type's position for comment preservation.
    pub fn set_type_node(&mut self, node: &ast::Node, type_node: &ast::Node) {
        let state = self.state.borrow();
        state.ensure_emit_node(node_key(node), |emit_node| {
            emit_node.type_node = Some(type_node.clone());
        });
    }

    // GetTypeNode gets the type node stored on a name node by the type eraser.
    pub fn get_type_node(&mut self, node: &ast::Node) -> Option<ast::Node> {
        self.state
            .borrow()
            .with_emit_node(node_key(node), |emit_node| emit_node.type_node)
            .flatten()
    }

    pub fn set_identifier_type_arguments(
        &mut self,
        node: &ast::Node,
        type_arguments: Option<ast::NodeList>,
    ) {
        let state = self.state.borrow();
        state.ensure_emit_node(node_key(node), |emit_node| {
            emit_node.identifier_type_arguments = type_arguments;
        });
    }

    pub fn get_identifier_type_arguments(&mut self, node: &ast::Node) -> Option<ast::NodeList> {
        self.state
            .borrow()
            .with_emit_node(node_key(node), |emit_node| {
                emit_node.identifier_type_arguments
            })
            .flatten()
    }

    pub fn new_not_emitted_statement(&mut self, node: &ast::Node) -> ast::Node {
        let statement = self.factory.new_not_emitted_statement();
        let loc = self.store_for_node(*node).loc(*node);
        self.set_factory_node_loc(statement, loc);
        self.set_original(&statement, node);
        self.assign_comment_range(&statement, node);
        statement
    }
}

pub(crate) const EMIT_NODE_FLAGS_HAS_COMMENT_RANGE: u32 = 1 << 0;
pub(crate) const EMIT_NODE_FLAGS_HAS_SOURCE_MAP_RANGE: u32 = 1 << 1;

#[derive(Clone, Copy)]
enum EmitNodeFlags {
    HAS_COMMENT_RANGE = 1 << 0,
    HAS_SOURCE_MAP_RANGE = 1 << 1,
}

#[derive(Clone)]
pub struct SynthesizedComment {
    pub kind: ast::Kind,
    pub loc: core::TextRange,
    pub has_leading_new_line: bool,
    pub has_trailing_new_line: bool,
    pub text: String,
}

#[derive(Default, Clone)]
pub(crate) struct EmitNode {
    pub(crate) flags: u32,
    pub(crate) emit_flags: EmitFlags,
    pub(crate) comment_range: core::TextRange,
    pub(crate) source_map_range: core::TextRange,
    pub(crate) token_source_map_ranges: HashMap<ast::Kind, core::TextRange>,
    pub(crate) helpers: Vec<usize>,
    pub(crate) external_helpers_module_name: Option<ast::Node>,
    pub(crate) leading_comments: Vec<SynthesizedComment>,
    pub(crate) trailing_comments: Vec<SynthesizedComment>,
    pub(crate) type_node: Option<ast::Node>,
    pub(crate) identifier_type_arguments: Option<ast::NodeList>,
}

// NOTE: This method is not guaranteed to be thread-safe
impl EmitNode {
    pub(crate) fn copy_from(&mut self, source: &EmitNode) {
        self.flags = source.flags;
        self.emit_flags = source.emit_flags;
        self.comment_range = source.comment_range;
        self.source_map_range = source.source_map_range;
        self.token_source_map_ranges = source.token_source_map_ranges.clone();
        self.helpers = source.helpers.clone();
        self.external_helpers_module_name = source.external_helpers_module_name.clone();
    }
}

pub(crate) fn try_emit_node_in_state<R>(
    state: &EmitContextState,
    node: &ast::Node,
    f: impl FnOnce(&EmitNode) -> R,
) -> Option<R> {
    let handle = state.emit_nodes.try_handle(node_key(node))?;
    Some(state.emit_nodes.with_by_handle(handle, f))
}

pub(crate) fn with_emit_node_in_state_mut<R>(
    state: &EmitContextState,
    node: &ast::Node,
    f: impl FnOnce(&mut EmitNode) -> R,
) -> R {
    let handle = state.emit_nodes.ensure_handle(node_key(node));
    state.emit_nodes.with_by_handle_mut(handle, f)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AutoGenerateOptions {
    pub flags: GeneratedIdentifierFlags,
    pub prefix: &'static str,
    pub suffix: &'static str,
}

impl Default for AutoGenerateOptions {
    fn default() -> Self {
        Self {
            flags: GeneratedIdentifierFlags::NONE,
            prefix: "",
            suffix: "",
        }
    }
}

pub(crate) static NEXT_AUTO_GENERATE_ID: AtomicU32 = AtomicU32::new(0);

pub type AutoGenerateId = u32;

#[derive(Clone)]
pub struct AutoGenerateInfo {
    pub flags: GeneratedIdentifierFlags, // Specifies whether to auto-generate the text for an identifier.
    pub id: AutoGenerateId, // Ensures unique generated identifiers get unique names, but clones get the same name.
    pub prefix: String,     // Optional prefix to apply to the start of the generated name
    pub suffix: String,     // Optional suffix to apply to the end of the generated name
    pub node: ast::Node, // For a GeneratedIdentifierFlagsNode, the node from which to generate an identifier
}

pub(crate) fn on_create(_node: ast::Node) -> ast::NodeFlags {
    ast::NodeFlags::SYNTHESIZED
}

pub(crate) fn set_original_in_state(
    state: &EmitContextStateRef,
    node: &ast::Node,
    original: &ast::Node,
) {
    set_original_in_state_ex(state, node, original, false)
}

fn set_original_in_state_ex(
    state: &EmitContextStateRef,
    node: &ast::Node,
    original: &ast::Node,
    allow_overwrite: bool,
) {
    let key = node_key(node);
    let original_key = node_key(original);
    let mut state = state.borrow_mut();
    match state.original.get(key) {
        None => {
            state.original.insert(key, *original);
            if !state.emit_nodes.is_empty()
                && let Some(emit_node) = state.with_emit_node(original_key, Clone::clone)
            {
                state.ensure_emit_node(key, |target| target.copy_from(&emit_node));
            }
        }
        Some(existing) if !allow_overwrite && node_key(existing) != original_key => {
            panic!("Original node already set.");
        }
        Some(_) if allow_overwrite => {
            state.original.insert(key, *original);
        }
        Some(_) => {}
    }
}

pub(crate) fn original_in_state(
    state: &EmitContextStateRef,
    node: &ast::Node,
) -> Option<ast::Node> {
    state.borrow().original.get_copied(node_key(node))
}

pub(crate) fn get_node_for_generated_name_worker_in_state(
    state: &EmitContextStateRef,
    node: &ast::Node,
    auto_generate_id: AutoGenerateId,
) -> ast::Node {
    let mut node = node.clone();
    let mut original = original_in_state(state, &node);
    let factory_store_id = new_node_factory_with_state(state.clone())
        .node_factory
        .store()
        .store_id();
    while let Some(next_original) = original {
        node = next_original;
        if node.store_id() == factory_store_id
            && ast::is_member_name(
                new_node_factory_with_state(state.clone())
                    .node_factory
                    .store(),
                node,
            )
        {
            let auto_generate = state.borrow().auto_generate.get_cloned(node_key(&node));
            if auto_generate.is_none()
                || (auto_generate.as_ref().unwrap().flags.is_node()
                    && auto_generate.as_ref().unwrap().id != auto_generate_id)
            {
                break;
            }
            if auto_generate.as_ref().unwrap().flags.is_node() {
                original = Some(auto_generate.unwrap().node);
                continue;
            }
        }
        original = original_in_state(state, &node);
    }
    node
}

fn is_hoisted_variable(store: &ast::AstStore, node: &ast::Node) -> bool {
    store
        .name(*node)
        .is_some_and(|name| ast::is_identifier(store, name))
        && store.initializer(*node).is_none()
}

fn find_span_end(
    ctx: &EmitContext,
    source: &ast::AstStore,
    nodes: &[ast::Node],
    predicate: fn(&ast::AstStore, ast::Node) -> bool,
    start: usize,
) -> usize {
    let mut i = start;
    while i < nodes.len() {
        let store = ctx.store_for_source_node(source, &nodes[i]);
        if !predicate(store, nodes[i]) {
            break;
        }
        i += 1;
    }
    i
}

fn find_span_end_with_emit_context(
    ctx: &mut EmitContext,
    source: &ast::AstStore,
    nodes: &[ast::Node],
    predicate: fn(&mut EmitContext, &ast::AstStore, &ast::Node) -> bool,
    start: usize,
) -> usize {
    let mut i = start;
    while i < nodes.len() && predicate(ctx, source, &nodes[i]) {
        i += 1;
    }
    i
}

fn find_span_end_resolved(
    ctx: &EmitContext,
    nodes: &[ast::Node],
    predicate: fn(&ast::AstStore, ast::Node) -> bool,
    start: usize,
) -> usize {
    let mut i = start;
    while i < nodes.len() {
        let store = ctx.store_for_node(nodes[i]);
        if !predicate(store, nodes[i]) {
            break;
        }
        i += 1;
    }
    i
}

fn find_span_end_with_emit_context_resolved(
    ctx: &mut EmitContext,
    nodes: &[ast::Node],
    predicate: fn(&mut EmitContext, &ast::Node) -> bool,
    start: usize,
) -> usize {
    let mut i = start;
    while i < nodes.len() && predicate(ctx, &nodes[i]) {
        i += 1;
    }
    i
}

fn node_key(node: &ast::Node) -> ast::Node {
    *node
}

fn helper_key(helper: &EmitHelper) -> usize {
    crate::helpers::helper_key(helper)
}

fn helper_from_key(key: usize) -> &'static EmitHelper {
    crate::helpers::helper_from_key(key)
}
