use std::ops::{Deref, DerefMut};

use ts_collections::{FastHashMap as HashMap, FastHashMapExt};
use ts_printer as printer;

use crate::checker::*;
use crate::emitresolver::get_meaning_of_entity_name_reference;
use crate::nodebuilder_hover::is_expanding;
use crate::symbolaccessibility::{
    get_qualified_left_meaning, has_non_global_augmentation_external_module_symbol,
};
use crate::{
    ast, collections, core, debug, diagnostics, jsnum, module, modulespecifiers, nodebuilder,
    pseudochecker, scanner, stringutil, tspath,
};

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub(crate) struct CompositeSymbolIdentity {
    pub(crate) is_constructor_node: bool,
    pub(crate) symbol_id: Option<ast::SymbolId>,
    pub(crate) node_id: ast::NodeId,
}

#[derive(Clone)]
pub(crate) struct TrackedSymbolArgs {
    pub(crate) symbol: SymbolIdentity,
    pub(crate) symbol_flags: ast::SymbolFlags,
    pub(crate) enclosing_declaration: Option<ast::Node>,
    pub(crate) meaning: ast::SymbolFlags,
}

#[derive(Clone)]
struct SerializedTypeEntry {
    node: ast::Node,
    pub(crate) truncating: bool,
    pub(crate) added_length: i32,
    pub(crate) tracked_symbols: Vec<TrackedSymbolArgs>,
}

struct SourceNodeListSnapshot {
    loc: core::TextRange,
    range: core::TextRange,
    nodes: Vec<ast::Node>,
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
struct CompositeTypeCacheIdentity {
    pub(crate) type_id: TypeId,
    pub(crate) flags: nodebuilder::Flags,
    pub(crate) internal_flags: nodebuilder::InternalFlags,
}

#[derive(Default)]
pub(crate) struct NodeBuilderLinks {
    pub(crate) serialized_types: HashMap<CompositeTypeCacheIdentity, SerializedTypeEntry>, // Collection of types serialized at this location
    pub(crate) fake_scope_for_signature_declaration: Option<String>, // If present, this is a fake scope injected into an enclosing declaration chain.
}

#[derive(Default)]
struct NodeBuilderLinkStore(core::LinkStore<ast::Node, NodeBuilderLinks>);

impl NodeBuilderLinkStore {
    fn has(&self, node: ast::Node) -> bool {
        self.0.has(node)
    }

    fn with<R>(&self, node: ast::Node, f: impl FnOnce(&NodeBuilderLinks) -> R) -> R {
        let handle = self.0.ensure_handle(node);
        self.0.with_by_handle(handle, f)
    }

    fn with_mut<R>(&self, node: ast::Node, f: impl FnOnce(&mut NodeBuilderLinks) -> R) -> R {
        let handle = self.0.ensure_handle(node);
        self.0.with_by_handle_mut(handle, f)
    }

    fn fake_scope_for_signature_declaration(&self, node: ast::Node) -> Option<String> {
        self.0.try_handle(node).and_then(|handle| {
            self.0.with_by_handle(handle, |links| {
                links.fake_scope_for_signature_declaration.clone()
            })
        })
    }
}

#[derive(Default)]
struct NodeBuilderSymbolLinks {
    pub(crate) specifier_cache: module::ModeAwareCache<String>,
}

pub(crate) struct NodeBuilderContext<'a> {
    pub(crate) host: &'a dyn Host,
    pub(crate) approximate_length: usize,
    pub(crate) max_truncation_length: usize,
    pub(crate) encountered_error: bool,
    pub(crate) truncating: bool,
    pub(crate) reported_diagnostic: bool,
    pub(crate) flags: nodebuilder::Flags,
    pub(crate) internal_flags: nodebuilder::InternalFlags,
    pub(crate) depth: usize,
    pub(crate) max_expansion_depth: i32, // -1 means no expansion, 0+ = verbosity levels
    pub(crate) type_stack: Vec<Option<TypeId>>,
    pub(crate) can_increase_expansion_depth: bool,
    pub(crate) expansion_truncated: bool,
    pub(crate) enclosing_declaration: Option<ast::Node>,
    pub(crate) enclosing_file: Option<SourceFileIdentity>,
    pub(crate) infer_type_parameters: Vec<TypeHandle>,
    pub(crate) visited_types: collections::Set<TypeId>,
    pub(crate) symbol_depth: HashMap<CompositeSymbolIdentity, i32>,
    pub(crate) tracked_symbols: Vec<TrackedSymbolArgs>,
    pub(crate) mapper: Option<TypeMapperHandle>,
    pub(crate) reverse_mapped_stack: Vec<SymbolIdentity>,
    pub(crate) enclosing_symbol_types: HashMap<SymbolIdentity, TypeHandle>,
    pub(crate) remapped_symbol_references: HashMap<SymbolIdentity, SymbolIdentity>,
    pub(crate) suppress_report_inference_fallback: bool,
    // per signature scope state
    pub(crate) type_parameter_names: collections::CopyOnWriteMap<TypeId, ast::Node>,
    pub(crate) type_parameter_names_by_text: collections::CopyOnWriteSet<String>,
    pub(crate) type_parameter_names_by_text_next_name_count:
        collections::CopyOnWriteMap<String, i32>,
    pub(crate) type_parameter_symbol_list: collections::CopyOnWriteSet<SymbolIdentity>,
}

pub(crate) enum NodeBuilderEmitContext<'e> {
    Borrowed(&'e mut printer::EmitContext),
    Owned(printer::EmitContext),
}

impl<'e> Deref for NodeBuilderEmitContext<'e> {
    type Target = printer::EmitContext;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Borrowed(e) => e,
            Self::Owned(e) => e,
        }
    }
}

impl<'e> DerefMut for NodeBuilderEmitContext<'e> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::Borrowed(e) => e,
            Self::Owned(e) => e,
        }
    }
}

pub(crate) struct NodeBuilderImpl<'a, 'state, 'c, 'e> {
    // host members
    pub(crate) ch: &'c mut Checker<'a, 'state>,
    pub(crate) e: NodeBuilderEmitContext<'e>,
    pub(crate) pc: pseudochecker::PseudoChecker,

    // cache
    links: NodeBuilderLinkStore,
    pub(crate) symbol_links: HashMap<SymbolIdentity, NodeBuilderSymbolLinks>,

    // state
    pub(crate) ctx: NodeBuilderContext<'a>,
    pub(crate) tracker: Option<Box<dyn nodebuilder::SymbolTracker + 'a>>,

    // symbols for synthesized identifiers, needed for e.g. inlay hints
    pub(crate) id_to_symbol: HashMap<ast::IdentifierNode, SymbolIdentity>,
}

impl<'a, 'state, 'c, 'e> NodeBuilderImpl<'a, 'state, 'c, 'e> {
    pub(crate) fn source_file_identity_for_enclosing_declaration(
        &self,
        declaration: ast::Node,
    ) -> Option<SourceFileIdentity> {
        if let Some(source_file) = self.ch.try_source_file_for_node(declaration) {
            return Some(SourceFileIdentity::from_source_file(source_file));
        }

        let factory_store = self.e.factory.node_factory.store();
        if declaration.store_id() == factory_store.store_id() {
            let mut current = declaration;
            while current.store_id() == factory_store.store_id() {
                let Some(parent) = factory_store.parent(current) else {
                    return None;
                };
                current = parent;
                if let Some(source_file) = self.ch.try_source_file_for_node(current) {
                    return Some(SourceFileIdentity::from_source_file(source_file));
                }
            }
        }

        None
    }

    pub(crate) fn with_node_builder_links<R>(
        &self,
        node: ast::Node,
        f: impl FnOnce(&NodeBuilderLinks) -> R,
    ) -> R {
        self.links.with(node, f)
    }

    pub(crate) fn with_node_builder_links_mut<R>(
        &self,
        node: ast::Node,
        f: impl FnOnce(&mut NodeBuilderLinks) -> R,
    ) -> R {
        self.links.with_mut(node, f)
    }

    pub(crate) fn fake_scope_for_signature_declaration(&self, node: ast::Node) -> Option<String> {
        self.links.fake_scope_for_signature_declaration(node)
    }

    fn has_synthetic_builder_scope(&self, node: ast::Node) -> bool {
        self.fake_scope_for_signature_declaration(node).is_some()
            || self.ch.semantic_state.has_synthetic_node_locals(node)
    }

    fn synthetic_builder_scope_export(
        &mut self,
        node: ast::Node,
        name: &str,
        meaning: ast::SymbolFlags,
    ) -> Option<SymbolIdentity> {
        let symbol = self
            .ch
            .semantic_state
            .synthetic_node_symbol_identity(node)?;
        let export = self
            .ch
            .with_symbol_identity_export_table(symbol, |exports| {
                exports.and_then(|exports| exports.get(name))
            });
        self.ch.get_symbol_identity_from_raw(export, meaning)
    }

    fn get_symbol_identity_from_builder_scope_locals(
        &mut self,
        locals: &SymbolIdentityTable,
        name: &str,
        meaning: ast::SymbolFlags,
    ) -> Option<SymbolIdentity> {
        if !meaning.intersects(ast::SYMBOL_FLAGS_ALL) {
            return None;
        }
        let key: ast::SymbolName = name.into();
        let symbol = self
            .ch
            .get_merged_symbol_identity(locals.get(&key).copied())?;
        let flags = self.ch.missing_name_symbol_identity_flags(symbol);
        if flags.intersects(meaning) || self.ch.get_symbol_flags(symbol).intersects(meaning) {
            return Some(symbol);
        }
        if flags.intersects(ast::SYMBOL_FLAGS_ALIAS) {
            let target = self.ch.resolve_alias_identity(symbol);
            if self
                .ch
                .missing_name_symbol_identity_flags(target)
                .intersects(meaning)
            {
                return Some(symbol);
            }
        }
        None
    }

    fn on_diagnostic_reported(&mut self) {
        self.ctx.reported_diagnostic = true;
    }

    pub(crate) fn enclosing_file(&self) -> Option<&'a ast::SourceFile> {
        self.ctx
            .enclosing_file
            .and_then(|file| self.ch.try_source_file_for_identity(file))
    }

    pub(crate) fn is_enclosing_file(&self, file: &ast::SourceFile) -> bool {
        self.ctx.enclosing_file.is_some_and(|enclosing_file| {
            SourceFileIdentity::from_source_file(file) == enclosing_file
        })
    }

    fn is_enclosing_source_file_node(&self, source_file: ast::Node) -> bool {
        self.ctx.enclosing_file.is_some_and(|enclosing_file| {
            SourceFileIdentity::from_root(source_file) == enclosing_file
        })
    }

    pub(crate) fn track_symbol(
        &mut self,
        symbol: SymbolIdentity,
        enclosing_declaration: Option<ast::Node>,
        meaning: ast::SymbolFlags,
    ) -> bool {
        self.track_symbol_identity(symbol, enclosing_declaration, meaning)
    }

    pub(crate) fn track_symbol_identity(
        &mut self,
        symbol: SymbolIdentity,
        enclosing_declaration: Option<ast::Node>,
        meaning: ast::SymbolFlags,
    ) -> bool {
        let symbol_flags = self.symbol_identity_flags(symbol);
        self.track_symbol_identity_with_flags(symbol, symbol_flags, enclosing_declaration, meaning)
    }

    fn track_symbol_identity_with_flags(
        &mut self,
        symbol: SymbolIdentity,
        symbol_flags: ast::SymbolFlags,
        enclosing_declaration: Option<ast::Node>,
        meaning: ast::SymbolFlags,
    ) -> bool {
        let is_type_parameter = symbol_flags.intersects(ast::SYMBOL_FLAGS_TYPE_PARAMETER);
        let accessibility = if !is_type_parameter && self.tracker.is_some() {
            Some(self.is_symbol_accessible_in_builder_scope_by_identity(
                Some(symbol),
                enclosing_declaration,
                meaning,
                true,
            ))
        } else {
            None
        };
        if self.tracker.as_mut().is_some_and(|tracker| {
            tracker.track_symbol(
                symbol.ast_identity(),
                symbol_flags,
                enclosing_declaration,
                meaning,
            )
        }) {
            self.on_diagnostic_reported();
            return true;
        }
        if let (Some(accessibility), Some(tracker)) =
            (accessibility.as_ref(), self.tracker.as_mut())
        {
            match accessibility.accessibility {
                printer::SymbolAccessibility::Accessible => {
                    tracker.mark_aliases_visible(&accessibility.aliases_to_make_visible);
                }
                printer::SymbolAccessibility::CannotBeNamed
                | printer::SymbolAccessibility::NotAccessible => {
                    let accessibility_kind = if accessibility.accessibility
                        == printer::SymbolAccessibility::CannotBeNamed
                    {
                        nodebuilder::SymbolAccessibility::CannotBeNamed
                    } else {
                        nodebuilder::SymbolAccessibility::NotAccessible
                    };
                    if tracker.report_symbol_accessibility_error(
                        accessibility_kind,
                        &accessibility.error_symbol_name,
                        &accessibility.error_module_name,
                        accessibility.error_node,
                    ) {
                        self.on_diagnostic_reported();
                        return true;
                    }
                }
                printer::SymbolAccessibility::NotResolved => {}
            }
        }
        if !is_type_parameter {
            self.ctx.tracked_symbols.push(TrackedSymbolArgs {
                symbol,
                symbol_flags,
                enclosing_declaration,
                meaning,
            });
        };
        false
    }

    fn symbol_identity_flags(&self, symbol: SymbolIdentity) -> ast::SymbolFlags {
        self.ch.symbol_identity_flags(symbol)
    }

    fn symbol_identity_has_non_global_augmentation_external_module_symbol(
        &mut self,
        symbol: SymbolIdentity,
    ) -> bool {
        self.ch
            .any_symbol_identity_declaration(symbol, |checker, declaration| {
                let store = checker.store_for_node(declaration);
                has_non_global_augmentation_external_module_symbol(checker, store, declaration)
            })
    }

    pub(crate) fn report_likely_unsafe_import_required_error(
        &mut self,
        specifier: &str,
        symbol_name: &str,
    ) {
        self.on_diagnostic_reported();
        if let Some(tracker) = self.tracker.as_mut() {
            tracker.report_likely_unsafe_import_required_error(specifier, symbol_name);
        }
    }

    pub(crate) fn report_inference_fallback(&mut self, node: ast::Node) {
        if let Some(tracker) = self.tracker.as_mut() {
            tracker.report_inference_fallback(node);
        }
    }

    pub(crate) fn report_non_serializable_property(&mut self, property_name: &str) {
        self.on_diagnostic_reported();
        if let Some(tracker) = self.tracker.as_mut() {
            tracker.report_non_serializable_property(property_name);
        }
    }

    pub(crate) fn report_cyclic_structure_error(&mut self) {
        self.on_diagnostic_reported();
        if let Some(tracker) = self.tracker.as_mut() {
            tracker.report_cyclic_structure_error();
        }
    }

    pub(crate) fn report_private_in_base_of_class_expression(&mut self, property_name: &str) {
        self.on_diagnostic_reported();
        if let Some(tracker) = self.tracker.as_mut() {
            tracker.report_private_in_base_of_class_expression(property_name);
        }
    }

    pub(crate) fn report_inaccessible_this_error(&mut self) {
        self.on_diagnostic_reported();
        if let Some(tracker) = self.tracker.as_mut() {
            tracker.report_inaccessible_this_error();
        }
    }

    pub(crate) fn report_inaccessible_unique_symbol_error(&mut self) {
        self.on_diagnostic_reported();
        if let Some(tracker) = self.tracker.as_mut() {
            tracker.report_inaccessible_unique_symbol_error();
        }
    }

    pub(crate) fn report_truncation_error(&mut self) {
        self.on_diagnostic_reported();
        if let Some(tracker) = self.tracker.as_mut() {
            tracker.report_truncation_error();
        }
    }

    pub(crate) fn clone_node_with_loc(
        &mut self,
        node: ast::Node,
        loc: core::TextRange,
    ) -> ast::Node {
        let cloned = if node.store_id() == self.e.factory.node_factory.store().store_id() {
            self.e.factory.node_factory.clone_node(node)
        } else {
            let source = self.ch.store_for_node(node);
            self.e
                .factory
                .node_factory
                .deep_clone_node_from_store(source, node)
        };
        self.e.factory.node_factory.link_parsed_parent(cloned, None);
        self.e
            .factory
            .node_factory
            .place_checker_synthetic_node(cloned, loc);
        self.e.factory.node_factory.mark_checker_synthesized(cloned);
        cloned
    }

    fn clone_node_with_loc_preserving_metadata(
        &mut self,
        node: ast::Node,
        loc: core::TextRange,
    ) -> ast::Node {
        let cloned = Self::node_value(self.clone_node_with_loc(node, loc));
        self.set_original_ex(&cloned, &node, true);
        let emit_flags = self.e.emit_flags(&node);
        if emit_flags != printer::EF_NONE {
            self.e.set_emit_flags(&cloned, emit_flags);
        }
        if let Some(&symbol) = self.id_to_symbol.get(&node) {
            self.id_to_symbol.insert(cloned, symbol);
        }
        cloned
    }

    pub(crate) fn set_original(&mut self, node: &ast::Node, original: &ast::Node) {
        self.register_source_file_for_node(*original);
        self.e.set_original(node, original);
    }

    pub(crate) fn set_original_ex(
        &mut self,
        node: &ast::Node,
        original: &ast::Node,
        allow_overwrite: bool,
    ) {
        self.register_source_file_for_node(*original);
        self.e.set_original_ex(node, original, allow_overwrite);
    }

    pub(crate) fn register_source_file_for_node(&mut self, node: ast::Node) {
        if let Some(source_file) = self.ch.try_source_file_for_node(node) {
            self.e.add_source_file(source_file);
        }
    }

    pub(crate) fn deep_clone_node(&mut self, node: ast::Node) -> ast::Node {
        if node.store_id() == self.e.factory.node_factory.store().store_id() {
            self.e
                .factory
                .node_factory
                .deep_clone_node_in_current_store(node)
        } else {
            self.register_source_file_for_node(node);
            let source = self.ch.store_for_node(node);
            self.e
                .factory
                .node_factory
                .deep_clone_node_from_store(source, node)
        }
    }

    fn ensure_factory_node(&mut self, node: ast::Node) -> ast::Node {
        if node.store_id() == self.e.factory.node_factory.store().store_id() {
            node
        } else {
            self.deep_clone_node(node)
        }
    }

    fn ensure_optional_factory_node(&mut self, node: Option<ast::Node>) -> Option<ast::Node> {
        node.map(|node| self.ensure_factory_node(node))
    }

    pub(crate) fn new_factory_node_list(
        &mut self,
        nodes: impl IntoIterator<Item = ast::Node>,
    ) -> ast::NodeList {
        self.e.factory.node_factory.new_node_list(
            core::new_text_range(-1, -1),
            core::new_text_range(-1, -1),
            nodes,
        )
    }

    pub(crate) fn new_factory_modifier_list(
        &mut self,
        flags: ast::ModifierFlags,
    ) -> ast::ModifierList {
        let modifiers = self.create_modifiers_from_modifier_flags(flags);
        self.e.factory.node_factory.new_modifier_list(
            core::new_text_range(-1, -1),
            core::new_text_range(-1, -1),
            modifiers,
            flags,
        )
    }

    fn clone_source_node_list_to_factory(
        &mut self,
        list: ast::SourceNodeList<'_>,
    ) -> ast::NodeList {
        let list = snapshot_source_node_list(list);
        self.clone_source_node_list_snapshot_to_factory(list)
    }

    fn clone_source_node_list_snapshot_to_factory(
        &mut self,
        list: SourceNodeListSnapshot,
    ) -> ast::NodeList {
        let nodes = list
            .nodes
            .into_iter()
            .map(|node| self.ensure_factory_node(node))
            .collect::<Vec<_>>();
        self.e
            .factory
            .node_factory
            .new_node_list(list.loc, list.range, nodes)
    }

    pub(crate) fn store_for_node(&self, node: ast::Node) -> &ast::AstStore {
        if node.store_id() == self.e.factory.node_factory.store().store_id() {
            self.e.factory.node_factory.store()
        } else {
            self.ch.store_for_node(node)
        }
    }

    fn try_store_for_node(&self, node: ast::Node) -> Option<&ast::AstStore> {
        if node.store_id() == self.e.factory.node_factory.store().store_id() {
            Some(self.e.factory.node_factory.store())
        } else {
            let source_file = self.ch.try_source_file_for_node(node)?;
            (source_file.store().store_id() == node.store_id()).then_some(source_file.store())
        }
    }

    fn symbol_identity_display_name(&self, symbol: SymbolIdentity) -> String {
        let name = self.ch.symbol_identity_name(symbol).to_string();
        if let Some(value_declaration) = self
            .ch
            .missing_name_symbol_identity_value_declaration(symbol)
            && let Some(store) = self.try_store_for_node(value_declaration)
            && ast::is_private_identifier_class_element_declaration(store, value_declaration)
        {
            let name = store
                .name(value_declaration)
                .expect("private identifier class element should have a name");
            return store.text(name);
        }
        name
    }

    pub(crate) fn resolve_entity_name_in_builder_scope(
        &mut self,
        name: ast::Node,
        meaning: ast::SymbolFlags,
        ignore_errors: bool,
        dont_resolve_alias: bool,
        location: Option<ast::Node>,
    ) -> Option<SymbolIdentity> {
        let Some(location) = location else {
            return self.ch.resolve_entity_name(
                name,
                meaning,
                ignore_errors,
                dont_resolve_alias,
                None,
            );
        };
        if !self.has_synthetic_builder_scope(location) {
            return self.ch.resolve_entity_name(
                name,
                meaning,
                ignore_errors,
                dont_resolve_alias,
                Some(location),
            );
        }

        let name_store = self.store_for_node(name);
        if name_store.kind(name) != ast::Kind::Identifier {
            let parent = self.store_for_node(location).parent(location);
            return self.resolve_entity_name_in_builder_scope(
                name,
                meaning,
                ignore_errors,
                dont_resolve_alias,
                parent,
            );
        }

        let name_text = name_store.text(name).to_string();
        let (locals, parent) = {
            let location_store = self.store_for_node(location);
            (
                self.ch
                    .semantic_state
                    .collect_synthetic_node_locals(location),
                location_store.parent(location),
            )
        };
        if let Some(locals) = locals {
            if let Some(symbol) =
                self.get_symbol_identity_from_builder_scope_locals(&locals, &name_text, meaning)
            {
                return Some(symbol);
            }
        }

        self.resolve_entity_name_in_builder_scope(
            name,
            meaning,
            ignore_errors,
            dont_resolve_alias,
            parent,
        )
    }

    fn resolve_name_in_builder_scope(
        &mut self,
        location: Option<ast::Node>,
        name: &str,
        meaning: ast::SymbolFlags,
        name_not_found_message: Option<&'static diagnostics::Message>,
        is_use: bool,
        exclude_globals: bool,
    ) -> Option<SymbolIdentity> {
        let Some(location) = location else {
            return self
                .ch
                .resolve_name(
                    None,
                    name,
                    meaning,
                    name_not_found_message,
                    is_use,
                    exclude_globals,
                )
                .map(SymbolIdentity::from_symbol_handle);
        };
        if !self.has_synthetic_builder_scope(location) {
            return self
                .ch
                .resolve_name(
                    Some(location),
                    name,
                    meaning,
                    name_not_found_message,
                    is_use,
                    exclude_globals,
                )
                .map(SymbolIdentity::from_symbol_handle);
        }

        let (locals, parent) = {
            let store = self.store_for_node(location);
            (
                self.ch
                    .semantic_state
                    .collect_synthetic_node_locals(location),
                store.parent(location),
            )
        };
        if let Some(locals) = locals {
            if let Some(symbol) =
                self.get_symbol_identity_from_builder_scope_locals(&locals, name, meaning)
            {
                return Some(symbol);
            }
        }
        self.resolve_name_in_builder_scope(
            parent,
            name,
            meaning,
            name_not_found_message,
            is_use,
            exclude_globals,
        )
    }

    pub(crate) fn is_symbol_accessible_in_builder_scope_by_identity(
        &mut self,
        symbol: Option<SymbolIdentity>,
        enclosing_declaration: Option<ast::Node>,
        meaning: ast::SymbolFlags,
        should_compute_aliases_to_make_visible: bool,
    ) -> printer::SymbolAccessibilityResult {
        let Some(enclosing_declaration) = enclosing_declaration else {
            return self.ch.is_symbol_accessible_by_identity(
                symbol,
                None,
                meaning,
                should_compute_aliases_to_make_visible,
            );
        };
        if !self.has_synthetic_builder_scope(enclosing_declaration) {
            let enclosing_declaration =
                self.checker_accessible_enclosing_declaration(enclosing_declaration);
            return self.ch.is_symbol_accessible_by_identity(
                symbol,
                enclosing_declaration,
                meaning,
                should_compute_aliases_to_make_visible,
            );
        }

        if let Some(symbol) = symbol {
            if let Some(result) = self.is_any_symbol_accessible_in_builder_scope_by_identity(
                &[symbol],
                enclosing_declaration,
                symbol,
                meaning,
                should_compute_aliases_to_make_visible,
                true,
            ) {
                return result;
            }

            let error_enclosing_declaration =
                self.checker_accessible_enclosing_declaration(enclosing_declaration);

            // This could be a symbol that is not exported in the external module
            // or it could be a symbol from different external module that is not aliased and hence cannot be named
            let mut symbol_external_module = None;
            self.ch
                .find_symbol_handle_declaration(symbol.symbol_handle(), |checker, d| {
                    symbol_external_module = checker.get_external_module_container_identity(d);
                    symbol_external_module.is_some()
                });
            if let Some(symbol_external_module) = symbol_external_module {
                let enclosing_external_module =
                    error_enclosing_declaration.and_then(|declaration| {
                        self.ch.get_external_module_container_identity(declaration)
                    });
                if !self.ch.same_optional_symbol_identity(
                    Some(symbol_external_module),
                    enclosing_external_module,
                ) {
                    // name from different external module that is not visible
                    let error_symbol_name = self.ch.symbol_identity_to_string_ex(
                        symbol,
                        error_enclosing_declaration,
                        meaning,
                        crate::types::SYMBOL_FORMAT_FLAGS_ALLOW_ANY_NODE_KIND,
                    );
                    let error_module_name =
                        self.ch.symbol_identity_to_string(symbol_external_module);
                    return printer::SymbolAccessibilityResult {
                        accessibility: printer::SymbolAccessibility::CannotBeNamed,
                        error_symbol_name,
                        error_module_name,
                        error_node: error_enclosing_declaration.and_then(|declaration| {
                            self.ch
                                .try_source_file_for_node(declaration)
                                .and_then(|source_file| {
                                    ast::is_in_js_file(source_file.store(), declaration)
                                        .then_some(declaration)
                                })
                        }),
                        ..Default::default()
                    };
                }
            }

            // Just a local name that is not accessible
            let error_symbol_name = self.ch.symbol_identity_to_string_ex(
                symbol,
                error_enclosing_declaration,
                meaning,
                crate::types::SYMBOL_FORMAT_FLAGS_ALLOW_ANY_NODE_KIND,
            );
            return printer::SymbolAccessibilityResult {
                accessibility: printer::SymbolAccessibility::NotAccessible,
                error_symbol_name,
                ..Default::default()
            };
        }

        printer::SymbolAccessibilityResult {
            accessibility: printer::SymbolAccessibility::Accessible,
            ..Default::default()
        }
    }

    fn is_any_symbol_accessible_in_builder_scope_by_identity(
        &mut self,
        symbols: &[SymbolIdentity],
        enclosing_declaration: ast::Node,
        initial_symbol: SymbolIdentity,
        meaning: ast::SymbolFlags,
        should_compute_aliases_to_make_visible: bool,
        allow_modules: bool,
    ) -> Option<printer::SymbolAccessibilityResult> {
        if symbols.is_empty() {
            return None;
        }

        let mut had_accessible_chain = None;
        let mut early_module_bail = false;
        for &symbol in symbols {
            let accessible_symbol_chain = self
                .get_accessible_symbol_identity_chain_in_builder_scope(
                    symbol,
                    Some(enclosing_declaration),
                    meaning,
                    false,
                );
            if !accessible_symbol_chain.is_empty()
                && self.can_qualify_symbol_in_builder_scope_identity(
                    accessible_symbol_chain[0],
                    Some(enclosing_declaration),
                    if accessible_symbol_chain.len() == 1 {
                        meaning
                    } else {
                        get_qualified_left_meaning(meaning)
                    },
                    false,
                )
            {
                had_accessible_chain = Some(symbol);
                let has_accessible_declarations = self.ch.has_visible_declarations_by_identity(
                    accessible_symbol_chain[0],
                    should_compute_aliases_to_make_visible,
                );
                if has_accessible_declarations.is_some() {
                    return has_accessible_declarations;
                }
            }
            if allow_modules
                && self.symbol_identity_has_non_global_augmentation_external_module_symbol(symbol)
            {
                if should_compute_aliases_to_make_visible {
                    early_module_bail = true;
                    // Generally speaking, we want to use the aliases that already exist to refer to a module, if present
                    // In order to do so, we need to find those aliases in order to retain them in declaration emit; so
                    // if we are in declaration emit, we cannot use the fast path for module visibility until we've exhausted
                    // all other visibility options (in order to capture the possible aliases used to reference the module)
                    continue;
                }
                // Any meaning of a module symbol is always accessible via an `import` type
                return Some(printer::SymbolAccessibilityResult {
                    accessibility: printer::SymbolAccessibility::Accessible,
                    ..Default::default()
                });
            }

            // If we haven't got the accessible symbol, it doesn't mean the symbol is actually inaccessible.
            // It could be a qualified symbol and hence verify the path
            // e.g.:
            // module m {
            //     export class c {
            //     }
            // const x: typeof m.c
            // In the above example when we start with checking if typeof m.c symbol is accessible,
            // we are going to see if c can be accessed in scope directly.
            // But it can't, hence the accessible is going to be undefined, but that doesn't mean m.c is inaccessible
            // It is accessible if the parent m is accessible because then m.c can be accessed through qualification

            let checker_enclosing_declaration =
                self.checker_accessible_enclosing_declaration(enclosing_declaration);
            let containers = self.ch.get_containers_of_symbol_identity(
                symbol,
                checker_enclosing_declaration,
                meaning,
            );
            let is_initial_symbol = self.ch.same_symbol_identity(initial_symbol, symbol);
            let parent_result = self.is_any_symbol_accessible_in_builder_scope_by_identity(
                &containers,
                enclosing_declaration,
                initial_symbol,
                if is_initial_symbol {
                    get_qualified_left_meaning(meaning)
                } else {
                    meaning
                },
                should_compute_aliases_to_make_visible,
                allow_modules,
            );
            if parent_result.is_some() {
                return parent_result;
            }
        }

        if early_module_bail {
            return Some(printer::SymbolAccessibilityResult {
                accessibility: printer::SymbolAccessibility::Accessible,
                ..Default::default()
            });
        }

        if let Some(had_accessible_chain) = had_accessible_chain {
            let error_enclosing_declaration =
                self.checker_accessible_enclosing_declaration(enclosing_declaration);
            let mut error_module_name = String::new();
            if !self
                .ch
                .same_symbol_identity(had_accessible_chain, initial_symbol)
            {
                error_module_name = self.ch.symbol_identity_to_string_ex(
                    had_accessible_chain,
                    error_enclosing_declaration,
                    ast::SYMBOL_FLAGS_NAMESPACE,
                    crate::types::SYMBOL_FORMAT_FLAGS_ALLOW_ANY_NODE_KIND,
                );
            }
            let error_symbol_name = self.ch.symbol_identity_to_string_ex(
                initial_symbol,
                error_enclosing_declaration,
                meaning,
                crate::types::SYMBOL_FORMAT_FLAGS_ALLOW_ANY_NODE_KIND,
            );
            return Some(printer::SymbolAccessibilityResult {
                accessibility: printer::SymbolAccessibility::NotAccessible,
                error_symbol_name,
                error_module_name,
                ..Default::default()
            });
        }

        None
    }

    pub(crate) fn checker_accessible_enclosing_declaration(
        &self,
        node: ast::Node,
    ) -> Option<ast::Node> {
        let mut current = Some(node);
        while let Some(node) = current {
            if self.ch.try_source_file_for_node(node).is_some() {
                return Some(node);
            }
            current = self
                .try_store_for_node(node)
                .and_then(|store| store.parent(node));
        }
        self.ctx.enclosing_file.map(SourceFileIdentity::root)
    }

    pub(crate) fn is_symbol_accessible_in_builder_scope(
        &mut self,
        symbol: Option<SymbolIdentity>,
        enclosing_declaration: Option<ast::Node>,
        meaning: ast::SymbolFlags,
        should_compute_aliases_to_make_visible: bool,
    ) -> printer::SymbolAccessibilityResult {
        self.is_symbol_accessible_in_builder_scope_by_identity(
            symbol,
            enclosing_declaration,
            meaning,
            should_compute_aliases_to_make_visible,
        )
    }

    fn get_accessible_symbol_identity_chain_in_builder_scope(
        &mut self,
        symbol: SymbolIdentity,
        enclosing_declaration: Option<ast::Node>,
        meaning: ast::SymbolFlags,
        use_only_external_aliasing: bool,
    ) -> Vec<SymbolIdentity> {
        let Some(enclosing_declaration) =
            enclosing_declaration.or_else(|| self.ctx.enclosing_file.map(SourceFileIdentity::root))
        else {
            return self.ch.get_accessible_symbol_chain_identity(
                symbol,
                None,
                meaning,
                use_only_external_aliasing,
            );
        };
        if !self.has_synthetic_builder_scope(enclosing_declaration) {
            let enclosing_declaration =
                self.checker_accessible_enclosing_declaration(enclosing_declaration);
            return self.ch.get_accessible_symbol_chain_identity(
                symbol,
                enclosing_declaration,
                meaning,
                use_only_external_aliasing,
            );
        }

        let (locals, parent) = {
            let parent = self
                .try_store_for_node(enclosing_declaration)
                .and_then(|store| store.parent(enclosing_declaration));
            (
                self.ch
                    .semantic_state
                    .collect_synthetic_node_locals(enclosing_declaration),
                parent,
            )
        };
        if let Some(locals) = locals {
            let name = self.ch.symbol_identity_name(symbol).to_string();
            if let Some(local) = locals.get(name.as_str()).copied() {
                let local_flags = self.ch.missing_name_symbol_identity_flags(local);
                let symbol_flags = self.ch.missing_name_symbol_identity_flags(symbol);
                if self.ch.same_symbol_identity(local, symbol)
                    && (!local_flags.intersects(ast::SYMBOL_FLAGS_ASSIGNMENT)
                        || symbol_flags.intersects(ast::SYMBOL_FLAGS_ASSIGNMENT))
                {
                    return vec![symbol];
                }
                let matches_meaning = local_flags.intersects(meaning)
                    || local_flags.intersects(ast::SYMBOL_FLAGS_ALIAS)
                        && self.ch.get_symbol_flags(local).intersects(meaning);
                if matches_meaning {
                    return self.get_accessible_symbol_identity_chain_from_builder_parent(
                        symbol,
                        enclosing_declaration,
                        parent,
                        meaning,
                        use_only_external_aliasing,
                    );
                }
            }
            let export = self
                .ch
                .semantic_state
                .synthetic_node_symbol_identity(enclosing_declaration)
                .and_then(|scope_symbol| {
                    self.ch
                        .with_symbol_identity_export_table(scope_symbol, |exports| {
                            exports.and_then(|exports| exports.get(name.as_str()))
                        })
                });
            if let Some(export) = export {
                if self.ch.same_symbol_identity(export, symbol) {
                    return vec![symbol];
                }
                let flags = self.ch.missing_name_symbol_identity_flags(export);
                let matches_meaning = flags.intersects(meaning)
                    || flags.intersects(ast::SYMBOL_FLAGS_ALIAS)
                        && self.ch.get_symbol_flags(export).intersects(meaning);
                if matches_meaning {
                    return self.get_accessible_symbol_identity_chain_from_builder_parent(
                        symbol,
                        enclosing_declaration,
                        parent,
                        meaning,
                        use_only_external_aliasing,
                    );
                }
            }
        }

        self.get_accessible_symbol_identity_chain_from_builder_parent(
            symbol,
            enclosing_declaration,
            parent,
            meaning,
            use_only_external_aliasing,
        )
    }

    fn get_accessible_symbol_identity_chain_from_builder_parent(
        &mut self,
        symbol: SymbolIdentity,
        enclosing_declaration: ast::Node,
        parent: Option<ast::Node>,
        meaning: ast::SymbolFlags,
        use_only_external_aliasing: bool,
    ) -> Vec<SymbolIdentity> {
        let chain = self.get_accessible_symbol_identity_chain_in_builder_scope(
            symbol,
            parent,
            meaning,
            use_only_external_aliasing,
        );
        if chain.is_empty()
            || self.can_qualify_symbol_in_builder_scope_identity(
                chain[0],
                Some(enclosing_declaration),
                if chain.len() == 1 {
                    meaning
                } else {
                    get_qualified_left_meaning(meaning)
                },
                use_only_external_aliasing,
            )
        {
            return chain;
        }
        let global_this_symbol = self.ch.global_this_symbol_identity();
        let symbol_name = self.ch.symbol_identity_name(symbol);
        let global_this_export = self
            .ch
            .with_symbol_identity_export_table(global_this_symbol, |exports| {
                exports.and_then(|exports| exports.get(symbol_name.as_str()))
            });
        if self
            .ch
            .same_optional_symbol_identity(global_this_export, Some(symbol))
            && self.can_qualify_symbol_in_builder_scope_identity(
                global_this_symbol,
                Some(enclosing_declaration),
                get_qualified_left_meaning(meaning),
                use_only_external_aliasing,
            )
        {
            return vec![global_this_symbol, symbol];
        }
        Vec::new()
    }

    fn can_qualify_symbol_in_builder_scope_identity(
        &mut self,
        symbol_from_symbol_table: SymbolIdentity,
        enclosing_declaration: Option<ast::Node>,
        meaning: ast::SymbolFlags,
        use_only_external_aliasing: bool,
    ) -> bool {
        // If the symbol is equivalent and doesn't need further qualification, this symbol is accessible
        !self.needs_qualification_in_builder_scope_identity(
            symbol_from_symbol_table,
            enclosing_declaration,
            meaning,
        ) ||
            // If symbol needs qualification, make sure that parent is accessible, if it is then this symbol is accessible too
            self.ch
                .symbol_identity_parent(symbol_from_symbol_table)
                .is_some_and(|parent| {
                    !self
                        .get_accessible_symbol_identity_chain_in_builder_scope(
                            parent,
                            enclosing_declaration,
                            get_qualified_left_meaning(meaning),
                            use_only_external_aliasing,
                        )
                        .is_empty()
                })
    }

    fn fake_builder_scope_parent(&self, node: ast::Node) -> Option<ast::Node> {
        if !self.has_synthetic_builder_scope(node) {
            return Some(node);
        }
        self.try_store_for_node(node)
            .and_then(|store| store.parent(node))
    }

    fn needs_qualification_in_builder_scope_identity(
        &mut self,
        symbol: SymbolIdentity,
        enclosing_declaration: Option<ast::Node>,
        meaning: ast::SymbolFlags,
    ) -> bool {
        let Some(enclosing_declaration) =
            enclosing_declaration.or_else(|| self.ctx.enclosing_file.map(SourceFileIdentity::root))
        else {
            return self.ch.needs_qualification_identity(symbol, None, meaning);
        };
        if !self.has_synthetic_builder_scope(enclosing_declaration) {
            let enclosing_declaration =
                self.checker_accessible_enclosing_declaration(enclosing_declaration);
            return self
                .ch
                .needs_qualification_identity(symbol, enclosing_declaration, meaning);
        }

        let (locals, parent) = {
            let parent = self
                .try_store_for_node(enclosing_declaration)
                .and_then(|store| store.parent(enclosing_declaration));
            (
                self.ch
                    .semantic_state
                    .collect_synthetic_node_locals(enclosing_declaration),
                parent,
            )
        };
        if let Some(locals) = locals {
            let symbol_name = self.ch.symbol_identity_name(symbol).to_string();
            if let Some(symbol_from_table) = locals.get(symbol_name.as_str()).copied() {
                if self.ch.same_symbol_identity(symbol_from_table, symbol) {
                    return false;
                }
                let flags = self
                    .ch
                    .missing_name_symbol_identity_flags(symbol_from_table);
                if flags.intersects(meaning)
                    || flags.intersects(ast::SYMBOL_FLAGS_ALIAS)
                        && self
                            .ch
                            .get_symbol_flags(symbol_from_table)
                            .intersects(meaning)
                {
                    return true;
                }
            }
            if let Some(symbol_from_table) =
                self.synthetic_builder_scope_export(enclosing_declaration, &symbol_name, meaning)
            {
                if self.ch.same_symbol_identity(symbol_from_table, symbol) {
                    return false;
                }
                let flags = self
                    .ch
                    .missing_name_symbol_identity_flags(symbol_from_table);
                if flags.intersects(meaning)
                    || flags.intersects(ast::SYMBOL_FLAGS_ALIAS)
                        && self
                            .ch
                            .get_symbol_flags(symbol_from_table)
                            .intersects(meaning)
                {
                    return true;
                }
            }
        }

        self.needs_qualification_in_builder_scope_identity(symbol, parent, meaning)
    }

    pub(crate) fn source_store_for_node(&self, node: ast::Node) -> &'a ast::AstStore {
        assert_ne!(
            node.store_id(),
            self.e.factory.node_factory.store().store_id(),
            "factory-owned nodes are not source-store nodes"
        );
        self.ch.store_for_node(node)
    }

    fn has_single_variable_declaration_for_symbol_identity(&self, symbol: SymbolIdentity) -> bool {
        self.ch
            .with_symbol_identity_declarations(symbol, |declarations| {
                declarations.len() == 1
                    || declarations
                        .iter()
                        .copied()
                        .filter(|declaration| {
                            ast::is_variable_declaration(
                                self.ch.store_for_node(*declaration),
                                *declaration,
                            )
                        })
                        .count()
                        == 1
            })
    }

    fn symbol_handle_declaration_of_kind(
        &self,
        symbol: ast::SymbolHandle,
        kind: ast::Kind,
    ) -> Option<ast::Node> {
        self.ch
            .with_symbol_handle_declarations(symbol, |declarations| {
                declarations.iter().copied().find(|declaration| {
                    self.store_for_node(*declaration).kind(*declaration) == kind
                })
            })
    }

    fn declaration_of_kind_identity(
        &self,
        symbol: SymbolIdentity,
        kind: ast::Kind,
    ) -> Option<ast::Node> {
        self.ch
            .with_symbol_identity_declarations(symbol, |declarations| {
                declarations.iter().copied().find(|declaration| {
                    self.store_for_node(*declaration).kind(*declaration) == kind
                })
            })
    }
}

pub(crate) const DEFAULT_MAXIMUM_TRUNCATION_LENGTH: usize = 160;
pub(crate) const NO_TRUNCATION_MAXIMUM_TRUNCATION_LENGTH: usize = 1_000_000;

// Node builder utility functions

pub(crate) fn new_node_builder_impl<'a, 'state, 'c, 'e>(
    ch: &'c mut Checker<'a, 'state>,
    e: &'e mut printer::EmitContext,
    id_to_symbol: Option<HashMap<ast::IdentifierNode, SymbolIdentity>>,
) -> NodeBuilderImpl<'a, 'state, 'c, 'e> {
    new_node_builder_impl_with_emit_context(ch, NodeBuilderEmitContext::Borrowed(e), id_to_symbol)
}

pub(crate) fn new_node_builder_impl_owned<'a, 'state, 'c>(
    ch: &'c mut Checker<'a, 'state>,
    e: printer::EmitContext,
    id_to_symbol: Option<HashMap<ast::IdentifierNode, SymbolIdentity>>,
) -> NodeBuilderImpl<'a, 'state, 'c, 'c> {
    new_node_builder_impl_with_emit_context(ch, NodeBuilderEmitContext::Owned(e), id_to_symbol)
}

fn new_node_builder_impl_with_emit_context<'a, 'state, 'c, 'e>(
    ch: &'c mut Checker<'a, 'state>,
    e: NodeBuilderEmitContext<'e>,
    id_to_symbol: Option<HashMap<ast::IdentifierNode, SymbolIdentity>>,
) -> NodeBuilderImpl<'a, 'state, 'c, 'e> {
    let id_to_symbol = id_to_symbol.unwrap_or_default();
    let strict_null_checks = ch.strict_null_checks();
    let exact_optional_property_types = ch.exact_optional_property_types();
    let host = ch.program;
    let b = NodeBuilderImpl {
        ch,
        e,
        id_to_symbol,
        pc: pseudochecker::new_pseudo_checker(strict_null_checks, exact_optional_property_types),
        links: NodeBuilderLinkStore::default(),
        symbol_links: HashMap::new(),
        tracker: None,
        ctx: NodeBuilderContext {
            host,
            approximate_length: 0,
            max_truncation_length: 0,
            encountered_error: false,
            truncating: false,
            reported_diagnostic: false,
            flags: nodebuilder::FLAGS_NONE,
            internal_flags: nodebuilder::InternalFlags::default(),
            depth: 0,
            max_expansion_depth: -1,
            type_stack: Vec::new(),
            can_increase_expansion_depth: false,
            expansion_truncated: false,
            enclosing_declaration: None,
            enclosing_file: None,
            infer_type_parameters: Vec::new(),
            visited_types: collections::Set::new(),
            symbol_depth: HashMap::new(),
            tracked_symbols: Vec::new(),
            mapper: None,
            reverse_mapped_stack: Vec::new(),
            enclosing_symbol_types: HashMap::new(),
            remapped_symbol_references: HashMap::new(),
            suppress_report_inference_fallback: false,
            type_parameter_names: Default::default(),
            type_parameter_names_by_text: Default::default(),
            type_parameter_names_by_text_next_name_count: Default::default(),
            type_parameter_symbol_list: Default::default(),
        },
    };
    b
}

impl<'a, 'state, 'c, 'e> NodeBuilderImpl<'a, 'state, 'c, 'e> {
    pub(crate) fn save_restore_flags(
        &mut self,
    ) -> impl FnOnce(&mut NodeBuilderImpl<'a, 'state, 'c, 'e>) + use<'a, 'state, 'c, 'e> {
        let flags = self.ctx.flags;
        let internal_flags = self.ctx.internal_flags;
        let depth = self.ctx.depth;

        move |b: &mut NodeBuilderImpl<'a, 'state, 'c, 'e>| {
            b.ctx.flags = flags;
            b.ctx.internal_flags = internal_flags;
            b.ctx.depth = depth;
        }
    }

    fn check_truncation_length(&mut self) -> bool {
        if self.ctx.truncating {
            return self.ctx.truncating;
        }
        let max_length = if self.ctx.flags & nodebuilder::FLAGS_NO_TRUNCATION != 0 {
            NO_TRUNCATION_MAXIMUM_TRUNCATION_LENGTH
        } else if self.ctx.max_truncation_length > 0 {
            self.ctx.max_truncation_length
        } else {
            DEFAULT_MAXIMUM_TRUNCATION_LENGTH
        };
        self.ctx.truncating = self.ctx.approximate_length > max_length;
        self.ctx.truncating
    }

    // checkTruncationLengthIfExpanding returns true if maxExpansionDepth >= 0 and truncation length exceeded.
    // When expanding, we need to mark the output as truncated so we know not to offer further expansion.
    pub(crate) fn check_truncation_length_if_expanding(&mut self) -> bool {
        if self.ctx.max_expansion_depth >= 0 && self.check_truncation_length() {
            self.ctx.expansion_truncated = true;
            return true;
        }
        false
    }

    // isExpandableType reports whether t has a named representation that could be inlined
    // as its structural form during hover expansion. Filters out lib types.
    // When isAlias is true, checks whether t's alias symbol is from user code (not lib).
    fn is_expandable_type(&mut self, t: TypeHandle, is_alias: bool) -> bool {
        if is_alias {
            let alias_symbol = self
                .ch
                .type_alias_symbol_identity(self.ch.type_alias(t).unwrap())
                .expect("alias type must keep alias symbol");
            return !self
                .ch
                .is_lib_symbol_for_hover_verbosity(Some(alias_symbol));
        }
        if self.ch.is_lib_type_for_hover_verbosity(t) {
            return false;
        }
        let object_flags = self.ch.object_flags(t);
        if self.ch.type_flags(t) & TYPE_FLAGS_ENUM_LIKE != 0
            || object_flags & OBJECT_FLAGS_REFERENCE != 0
            || object_flags & OBJECT_FLAGS_CLASS_OR_INTERFACE != 0
        {
            return true;
        }
        if object_flags & OBJECT_FLAGS_ANONYMOUS != 0
            && self.ch.type_symbol_identity(t).is_some_and(|symbol| {
                self.symbol_identity_flags(symbol)
                    & (ast::SYMBOL_FLAGS_CLASS
                        | ast::SYMBOL_FLAGS_ENUM
                        | ast::SYMBOL_FLAGS_VALUE_MODULE
                        | ast::SYMBOL_FLAGS_FUNCTION
                        | ast::SYMBOL_FLAGS_METHOD)
                    != 0
            })
        {
            return true;
        }
        false
    }

    // isTypeOnStack reports whether t is already being processed in the current expansion,
    // excluding the last element (which is the type currently being serialized by typeToTypeNode).
    fn is_type_on_stack(&mut self, t: TypeHandle) -> bool {
        let type_id = self.ch.type_id(t);
        for i in 0..self.ctx.type_stack.len().saturating_sub(1) {
            if self.ctx.type_stack[i] == Some(type_id) {
                return true;
            }
        }
        false
    }

    fn can_possibly_expand_type(&mut self, t: TypeHandle) -> bool {
        if self.is_type_on_stack(t) {
            return false;
        }
        self.ctx.max_expansion_depth >= 0
            && (self.ctx.depth < self.ctx.max_expansion_depth as usize
                || self.ctx.depth == self.ctx.max_expansion_depth as usize
                    && !self.ctx.can_increase_expansion_depth)
    }

    fn should_expand_type(&mut self, t: TypeHandle, is_alias: bool) -> bool {
        if self.ctx.max_expansion_depth < 0 {
            return false;
        }
        if !self.is_expandable_type(t, is_alias) {
            return false;
        }
        if self.is_type_on_stack(t) {
            return false;
        }
        if self.ctx.depth < self.ctx.max_expansion_depth as usize {
            return true;
        }
        self.ctx.can_increase_expansion_depth = true;
        false
    }

    // isActivelyExpanding reports whether the current depth is below maxExpansionDepth,
    // meaning type-node reuse should be skipped so typeToTypeNode can expand named types.
    fn is_actively_expanding(&mut self) -> bool {
        self.ctx.max_expansion_depth > 0 && self.ctx.depth < self.ctx.max_expansion_depth as usize
    }

    // checkTypeExpandability probes whether a type (or its type arguments) could be expanded,
    // for use after type-node reuse where shouldExpandType was never called.
    // Delegates to shouldExpandType for the actual check, then recurses into type arguments
    // of reference types (e.g., Apple inside Promise<Apple>).
    pub(crate) fn check_type_expandability(&mut self, t: Option<TypeHandle>) {
        if self.ctx.max_expansion_depth < 0 || t.is_none() || self.ctx.can_increase_expansion_depth
        {
            return;
        }
        let t = t.unwrap();
        // Push t onto the type stack so shouldExpandType's cycle detection works correctly.
        self.ctx.type_stack.push(Some(self.ch.type_id(t)));
        if self.ch.type_alias_record(t).is_some() {
            self.should_expand_type(t, true);
        }
        if !self.ctx.can_increase_expansion_depth {
            self.should_expand_type(t, false);
        }
        self.ctx.type_stack.pop();
        if self.ctx.can_increase_expansion_depth {
            return;
        }
        // Recurse into type arguments (e.g., check Apple in Promise<Apple>).
        if self.ch.object_flags(t) & OBJECT_FLAGS_REFERENCE != 0 {
            for arg in self.ch.get_type_arguments(t) {
                self.check_type_expandability(Some(arg));
                if self.ctx.can_increase_expansion_depth {
                    return;
                }
            }
        }
    }

    fn append_reference_to_type(&mut self, root: ast::Node, reference: ast::Node) -> ast::Node {
        if ast::is_import_type_node(self.store_for_node(root), root) {
            let ids = get_access_stack(self.store_for_node(reference), reference);
            let qualifier = self.store_for_node(root).qualifier(root);
            let mut qualifier = qualifier;
            for id in ids {
                qualifier = Some(if let Some(qualifier) = qualifier {
                    self.e
                        .factory
                        .node_factory
                        .new_qualified_name(qualifier, id)
                } else {
                    id
                });
            }
            let type_arguments =
                source_type_argument_list_for_update(self.store_for_node(reference), reference)
                    .map(snapshot_source_node_list);
            let type_arguments = type_arguments.map(|type_arguments| {
                self.clone_source_node_list_snapshot_to_factory(type_arguments)
            });
            let is_type_of = self.store_for_node(root).is_type_of(root).unwrap_or(false);
            let argument = self.store_for_node(root).argument(root).unwrap();
            let attributes = self.store_for_node(root).attributes(root);
            return Self::node_value(self.e.factory.node_factory.new_import_type_node(
                is_type_of,
                argument,
                attributes,
                qualifier,
                type_arguments,
            ));
        } else if ast::is_type_reference_node(self.store_for_node(root), root) {
            if self.ctx.flags & nodebuilder::FLAGS_USE_INSTANTIATION_EXPRESSIONS != 0
                && self
                    .store_for_node(root)
                    .type_arguments(root)
                    .is_some_and(|type_arguments| !type_arguments.is_empty())
            {
                let type_arguments = self
                    .store_for_node(root)
                    .source_type_arguments(root)
                    .map(snapshot_source_node_list);
                let type_arguments = type_arguments.map(|type_arguments| {
                    Self::output_node_list_value(
                        self.clone_source_node_list_snapshot_to_factory(type_arguments),
                    )
                });
                // PORT NOTE: reshaped for borrowck; Go evaluates the access expression before the call.
                let type_name = self.store_for_node(root).type_name(root).unwrap();
                let access_expression = self.create_access_expression(Self::node_value(type_name));
                let mut expr = self
                    .create_expression_with_type_arguments(access_expression, type_arguments)
                    .clone();
                let ids = get_access_stack(self.store_for_node(reference), reference);
                for id in ids {
                    expr = self.e.factory.node_factory.new_property_access_expression(
                        expr,
                        None,
                        id,
                        ast::NODE_FLAGS_NONE,
                    );
                }
                return Self::node_value(expr);
            }
            let root_type_name = self.store_for_node(root).type_name(root).unwrap();
            let mut type_name = root_type_name;
            let ids = get_access_stack(self.store_for_node(reference), reference);
            for id in ids {
                type_name = self
                    .e
                    .factory
                    .node_factory
                    .new_qualified_name(type_name, id);
            }
            let type_arguments =
                source_type_argument_list_for_update(self.store_for_node(reference), reference)
                    .map(snapshot_source_node_list);
            let type_arguments = type_arguments.map(|type_arguments| {
                self.clone_source_node_list_snapshot_to_factory(type_arguments)
            });
            return Self::node_value(
                self.e
                    .factory
                    .node_factory
                    .new_type_reference_node(type_name, type_arguments),
            );
        }
        let mut expr = self.create_access_expression(root).clone();
        let ids = get_access_stack(self.store_for_node(reference), reference);
        for id in ids {
            expr = self.e.factory.node_factory.new_property_access_expression(
                expr,
                None,
                id,
                ast::NODE_FLAGS_NONE,
            );
        }
        Self::node_value(expr)
    }
}

fn get_access_stack(store: &ast::AstStore, ref_node: ast::Node) -> Vec<ast::Node> {
    let mut state = store.type_name(ref_node).unwrap();
    let mut ids = Vec::new();
    while !ast::is_identifier(store, state) {
        ids.insert(0, store.right(state).unwrap());
        state = store.left(state).unwrap();
    }
    ids.insert(0, state);
    ids
}

fn snapshot_source_node_list(list: ast::SourceNodeList<'_>) -> SourceNodeListSnapshot {
    SourceNodeListSnapshot {
        loc: list.loc(),
        range: list.range(),
        nodes: list.nodes(),
    }
}

fn source_type_argument_list_for_update(
    store: &ast::AstStore,
    node: ast::Node,
) -> Option<ast::SourceNodeList<'_>> {
    match store.kind(node) {
        ast::KIND_TYPE_REFERENCE => store.source_type_arguments(node),
        ast::KIND_EXPRESSION_WITH_TYPE_ARGUMENTS => store.source_type_arguments(node),
        ast::KIND_IMPORT_TYPE => store.source_type_arguments(node),
        _ => None,
    }
}

fn is_class_instance_side<'a, 'state>(c: &mut Checker<'a, 'state>, t: TypeHandle) -> bool {
    let Some(symbol) = c.type_symbol_identity(t) else {
        return false;
    };
    c.symbol_identity_flags(symbol) & ast::SYMBOL_FLAGS_CLASS != 0
        && (t == c.get_declared_type_of_symbol_identity_or_error(symbol)
            || (c.type_flags(t) & TYPE_FLAGS_OBJECT != 0
                && c.object_flags(t) & OBJECT_FLAGS_IS_CLASS_INSTANCE_CLONE != 0))
}

impl<'a, 'state, 'c, 'e> NodeBuilderImpl<'a, 'state, 'c, 'e> {
    pub(crate) fn node_value(node: ast::Node) -> ast::Node {
        node
    }

    pub(crate) fn output_node_list_value(node_list: ast::NodeList) -> ast::NodeList {
        node_list
    }

    fn add_synthetic_leading_comment_to_node(
        &mut self,
        node: ast::Node,
        kind: ast::Kind,
        text: impl Into<String>,
        has_trailing_new_line: bool,
    ) -> ast::Node {
        let node = Self::node_value(node);
        self.e
            .add_synthetic_leading_comment(&node, kind, text.into(), has_trailing_new_line);
        node
    }

    fn create_elided_information_placeholder(&mut self) -> ast::Node {
        self.ctx.approximate_length += 3;
        if self.ctx.flags & nodebuilder::FLAGS_NO_TRUNCATION == 0 {
            let name = self.e.factory.node_factory.new_identifier("...");
            let node = self
                .e
                .factory
                .node_factory
                .new_type_reference_node(name, None /*typeArguments*/);
            return Self::node_value(node);
        }
        let node = self
            .e
            .factory
            .node_factory
            .new_keyword_type_node(ast::KIND_ANY_KEYWORD);
        self.add_synthetic_leading_comment_to_node(
            node,
            ast::KIND_MULTI_LINE_COMMENT_TRIVIA,
            "elided",
            false, /*hasTrailingNewLine*/
        )
    }

    fn map_to_type_nodes(
        &mut self,
        list: Vec<TypeHandle>,
        is_bare_list: bool,
    ) -> Option<ast::NodeList> {
        if list.is_empty() {
            return None;
        }

        if self.check_truncation_length() {
            if !is_bare_list {
                let node = if self.ctx.flags & nodebuilder::FLAGS_NO_TRUNCATION != 0 {
                    let node = self
                        .e
                        .factory
                        .node_factory
                        .new_keyword_type_node(ast::KIND_ANY_KEYWORD);
                    self.add_synthetic_leading_comment_to_node(
                        node,
                        ast::KIND_MULTI_LINE_COMMENT_TRIVIA,
                        "elided",
                        false, /*hasTrailingNewLine*/
                    )
                } else {
                    let name = self.e.factory.node_factory.new_identifier("...");
                    let node = self
                        .e
                        .factory
                        .node_factory
                        .new_type_reference_node(name, None /*typeArguments*/);
                    Self::node_value(node)
                };
                return Some(Self::output_node_list_value(
                    self.new_factory_node_list(vec![node.clone()]),
                ));
            } else if list.len() > 2 {
                let mut nodes = vec![
                    self.type_to_type_node(list[0]),
                    None,
                    self.type_to_type_node(list[list.len() - 1]),
                ];

                nodes[1] = Some(if self.ctx.flags & nodebuilder::FLAGS_NO_TRUNCATION != 0 {
                    let node = self
                        .e
                        .factory
                        .node_factory
                        .new_keyword_type_node(ast::KIND_ANY_KEYWORD);
                    self.add_synthetic_leading_comment_to_node(
                        node,
                        ast::KIND_MULTI_LINE_COMMENT_TRIVIA,
                        format!("... {} more elided ...", list.len() - 2),
                        false, /*hasTrailingNewLine*/
                    )
                    .clone()
                } else {
                    let text = format!("... {} more ...", list.len() - 2);
                    let name = self.e.factory.node_factory.new_identifier(text);
                    self.e
                        .factory
                        .node_factory
                        .new_type_reference_node(name, None /*typeArguments*/)
                });
                return Some(Self::output_node_list_value(self.new_factory_node_list(
                    nodes.into_iter().flatten().collect::<Vec<_>>(),
                )));
            }
        }

        let may_have_name_collisions =
            self.ctx.flags & nodebuilder::FLAGS_USE_FULLY_QUALIFIED_TYPE == 0;
        struct SeenName {
            t: TypeHandle,
            i: usize,
        }
        let mut seen_names = if may_have_name_collisions {
            Some(collections::MultiMap::<String, SeenName>::new())
        } else {
            None
        };

        let mut result = Vec::with_capacity(list.len());

        for (i, t) in list.iter().enumerate() {
            let display_index = i + 1;
            if self.check_truncation_length() && display_index + 2 < list.len() - 1 {
                if self.ctx.flags & nodebuilder::FLAGS_NO_TRUNCATION != 0 {
                    let node = self
                        .e
                        .factory
                        .node_factory
                        .new_keyword_type_node(ast::KIND_ANY_KEYWORD);
                    result.push(
                        self.add_synthetic_leading_comment_to_node(
                            node,
                            ast::KIND_MULTI_LINE_COMMENT_TRIVIA,
                            format!("... {} more elided ...", list.len() - display_index),
                            false, /*hasTrailingNewLine*/
                        )
                        .clone(),
                    );
                } else {
                    let text = format!("... {} more ...", list.len() - display_index);
                    let name = self.e.factory.node_factory.new_identifier(text);
                    result.push(
                        self.e
                            .factory
                            .node_factory
                            .new_type_reference_node(name, None /*typeArguments*/),
                    );
                }
                let type_node = self.type_to_type_node(list[list.len() - 1]);
                if let Some(type_node) = type_node {
                    result.push(type_node.clone());
                }
                break;
            }
            self.ctx.approximate_length += 2; // Account for whitespace + separator
            let type_node = self.type_to_type_node(*t);
            if let Some(type_node) = type_node {
                result.push(type_node.clone());
                if let Some(seen_names) = seen_names.as_mut() {
                    let type_node_store = self.store_for_node(type_node);
                    if is_identifier_type_reference(type_node_store, type_node) {
                        seen_names.add(
                            type_node_store
                                .text(type_node_store.type_name(type_node).unwrap())
                                .to_string(),
                            SeenName {
                                t: *t,
                                i: result.len() - 1,
                            },
                        );
                    }
                }
            }
        }

        if let Some(seen_names) = seen_names {
            // To avoid printing types like `[Foo, Foo]` or `Bar & Bar` where
            // occurrences of the same name actually come from different
            // namespaces, go through the single-identifier type reference nodes
            // we just generated, and see if any names were generated more than
            // once while referring to different types. If so, regenerate the
            // type node for each entry by that name with the
            // `UseFullyQualifiedType` flag enabled.
            let old_flags = self.ctx.flags;
            let old_internal_flags = self.ctx.internal_flags;
            let old_depth = self.ctx.depth;
            self.ctx.flags |= nodebuilder::FLAGS_USE_FULLY_QUALIFIED_TYPE;
            for types in seen_names.m.values().map(Vec::as_slice) {
                if !array_is_homogeneous(types, |a, b| types_are_same_reference(self.ch, a.t, b.t))
                {
                    for seen in types {
                        result[seen.i] = self.type_to_type_node(seen.t).unwrap().clone();
                    }
                }
            }
            self.ctx.flags = old_flags;
            self.ctx.internal_flags = old_internal_flags;
            self.ctx.depth = old_depth;
        }

        Some(Self::output_node_list_value(
            self.new_factory_node_list(result),
        ))
    }

    pub(crate) fn serialize_type_name(
        &mut self,
        node: ast::Node,
        is_type_of: bool,
        type_arguments: Option<ast::NodeList>,
    ) -> Option<ast::Node> {
        let mut meaning = ast::SYMBOL_FLAGS_TYPE;
        if is_type_of {
            meaning = ast::SYMBOL_FLAGS_VALUE;
        }
        let symbol = self
            .ch
            .resolve_entity_name(node, meaning, true, false, Some(node));
        let Some(symbol) = symbol else {
            return None;
        };
        let mut resolved_symbol = symbol;
        if self.ch.symbol_identity_flags(symbol) & ast::SYMBOL_FLAGS_ALIAS != 0 {
            resolved_symbol = self.ch.resolve_symbol_identity(symbol, false);
        }

        if self
            .is_symbol_accessible_in_builder_scope_by_identity(
                Some(symbol.clone()),
                self.ctx.enclosing_declaration,
                meaning,
                false,
            )
            .accessibility
            != printer::SYMBOL_ACCESSIBILITY_ACCESSIBLE
        {
            return None;
        }
        self.symbol_identity_to_type_node(resolved_symbol, meaning, type_arguments)
    }

    fn set_comment_range(&mut self, node: ast::Node, range: Option<ast::Node>) {
        if let Some(range) = range {
            let Some(range_store) = self.try_store_for_node(range) else {
                return;
            };
            let range_loc = range_store.loc(range);
            if self.is_enclosing_file(self.ch.source_file_for_node(range)) {
                // Copy comments to node for declaration emit
                self.e
                    .assign_comment_range_from_source_loc(&node, &range, range_loc);
            }
        }
    }

    fn try_reuse_existing_type_node(
        &mut self,
        type_node: ast::Node,
        mut t: TypeHandle,
        host: ast::Node,
        add_undefined: bool,
    ) -> Option<ast::Node> {
        let original_type = t;
        if add_undefined {
            t = self.ch.get_optional_type(
                t,
                !ast::is_parameter_declaration(self.store_for_node(host), host),
            );
        }
        let clone = self.try_reuse_existing_non_parameter_type_node(type_node, t, Some(host), None);
        if let Some(clone) = clone {
            // explicitly add `| undefined` if it's missing from the input type nodes and the type contains `undefined` (and not the missing type)
            if add_undefined
                && contains_non_missing_undefined_type(self.ch, t)
                && !self.get_type_from_type_node(type_node, false).is_some_and(
                    |type_from_type_node| {
                        some_type(self.ch, type_from_type_node, |checker, t| {
                            checker.type_flags(t) & TYPE_FLAGS_UNDEFINED != 0
                        })
                    },
                )
            {
                let undefined = self
                    .e
                    .factory
                    .node_factory
                    .new_keyword_type_node(ast::KIND_UNDEFINED_KEYWORD);
                let types = self.new_factory_node_list(vec![clone.clone(), undefined]);
                return Some(Self::node_value(
                    self.e.factory.node_factory.new_union_type_node(types),
                ));
            }
            return Some(clone);
        }
        if add_undefined && original_type != t {
            let clone_missing_undefined = self.try_reuse_existing_non_parameter_type_node(
                type_node,
                original_type,
                Some(host),
                None,
            );
            if let Some(clone_missing_undefined) = clone_missing_undefined {
                let undefined = self
                    .e
                    .factory
                    .node_factory
                    .new_keyword_type_node(ast::KIND_UNDEFINED_KEYWORD);
                let types =
                    self.new_factory_node_list(vec![clone_missing_undefined.clone(), undefined]);
                return Some(Self::node_value(
                    self.e.factory.node_factory.new_union_type_node(types),
                ));
            }
        }
        None
    }

    fn type_node_is_equivalent_to_type(
        &mut self,
        annotated_declaration: Option<ast::Node>,
        t: TypeHandle,
        type_from_type_node: TypeHandle,
    ) -> bool {
        if type_from_type_node == t {
            return true;
        }
        let Some(annotated_declaration) = annotated_declaration else {
            return false;
        };
        if is_optional_declaration(
            self.store_for_node(annotated_declaration),
            annotated_declaration,
        ) {
            return self.ch.get_type_with_facts(t, TYPE_FACTS_NE_UNDEFINED) == type_from_type_node;
        }
        false
    }

    pub(crate) fn existing_type_node_is_not_reference_or_is_reference_with_compatible_type_argument_count(
        &mut self,
        existing: ast::Node,
        t: TypeHandle,
    ) -> bool {
        // In JS, you can say something like `Foo` and get a `Foo<any>` implicitly - we don't want to preserve that original `Foo` in these cases, though.
        if self.ch.object_flags(t) & OBJECT_FLAGS_REFERENCE == 0 {
            return true;
        }
        if !ast::is_type_reference_node(self.store_for_node(existing), existing) {
            return true;
        }
        // `type` is a reference type, and `existing` is a type reference node, but we still need to make sure they refer to the _same_ target type
        // before we go comparing their type argument counts.
        self.ch.get_type_from_type_reference(existing);
        // call to ensure symbol is resolved
        let symbol = self.ch.node_resolved_symbol(existing);
        if symbol.is_none() {
            return true;
        }
        let symbol = symbol.unwrap();
        let existing_target = self
            .ch
            .get_declared_type_of_symbol_identity_or_error(symbol);
        let target = self.ch.type_target(t);
        if existing_target != target {
            return true;
        }
        self.store_for_node(existing)
            .type_arguments(existing)
            .map(|type_arguments| type_arguments.len())
            .unwrap_or(0)
            >= self
                .ch
                .get_min_type_argument_count(self.ch.interface_type_parameters_slice(target))
    }

    fn try_reuse_existing_non_parameter_type_node(
        &mut self,
        existing: ast::Node,
        t: TypeHandle,
        mut host: Option<ast::Node>,
        mut annotation_type: Option<TypeHandle>,
    ) -> Option<ast::Node> {
        if self
            .ch
            .invalid_jsdoc_type_token(existing)
            .is_some_and(|(token, _)| token == '?')
        {
            return None;
        }
        if host.is_none() {
            host = self.ctx.enclosing_declaration;
        }
        if annotation_type.is_none() {
            annotation_type = self.get_type_from_type_node(existing, true);
        }
        if annotation_type.is_some()
            && self.type_node_is_equivalent_to_type(host, t, annotation_type.unwrap())
            && self
                .existing_type_node_is_not_reference_or_is_reference_with_compatible_type_argument_count(
                    existing, t,
                )
        {
            // PORT NOTE: reshaped for borrowck; Go reuses the current builder and then resumes this method.
            let result = self.try_reuse_existing_node_helper(existing);
            if result.is_some() {
                return result;
            }
        }
        None
    }

    pub(crate) fn can_reuse_type_node(&mut self, existing: ast::Node) -> bool {
        let Some(existing_type) = self.get_type_from_type_node(existing, true) else {
            return false;
        };

        if {
            let store = self.store_for_node(existing);
            ast::is_in_js_file(store, existing) && ast::is_literal_import_type_node(store, existing)
        } {
            let _ = self.ch.get_type_from_import_type_node(existing);
            let node_symbol = self.ch.node_resolved_symbol(existing);
            if let Some(node_symbol) = node_symbol {
                let (is_type_of, type_arguments_len) = {
                    let store = self.store_for_node(existing);
                    (
                        store.is_type_of(existing).unwrap_or(false),
                        store
                            .type_arguments(existing)
                            .map(|type_arguments| type_arguments.len())
                            .unwrap_or(0),
                    )
                };
                let type_parameters = self
                    .get_local_type_parameters_of_class_or_interface_or_type_alias_identity(
                        node_symbol,
                    );
                let min_type_argument_count = self.ch.get_min_type_argument_count(&type_parameters);
                if (!is_type_of
                    && !self
                        .ch
                        .missing_name_symbol_identity_flags(node_symbol)
                        .intersects(ast::SYMBOL_FLAGS_TYPE))
                    || type_arguments_len < min_type_argument_count
                {
                    return false;
                }
            }
        }

        if {
            let store = self.store_for_node(existing);
            ast::is_type_reference_node(store, existing)
        } {
            if {
                let store = self.store_for_node(existing);
                crate::utilities::is_const_type_reference(store, existing)
            } {
                return false;
            }
            let _ = self.ch.get_type_from_type_reference(existing);
            let symbol = self.ch.node_resolved_symbol(existing);
            let Some(symbol) = symbol else {
                return false;
            };
            {
                if self
                    .ch
                    .missing_name_symbol_identity_flags(symbol)
                    .intersects(ast::SYMBOL_FLAGS_TYPE_PARAMETER)
                {
                    let declared_type = self
                        .ch
                        .get_declared_type_of_symbol_identity_or_error(symbol);
                    if let Some(mapper) = self.ctx.mapper {
                        if self.ch.map_type_mapper_handle(mapper, declared_type) != declared_type {
                            return false;
                        }
                    }
                }
            }
        }

        if {
            let store = self.store_for_node(existing);
            ast::is_type_operator_node(store, existing)
                && store.operator(existing) == Some(ast::Kind::UniqueKeyword)
                && store
                    .r#type(existing)
                    .is_some_and(|ty| self.store_for_node(ty).kind(ty) == ast::Kind::SymbolKeyword)
        } {
            let Some(effective_enclosing) = self.get_enclosing_declaration_ignoring_fake_scope()
            else {
                return false;
            };
            let store = self.store_for_node(existing);
            return ast::find_ancestor(store, Some(existing), |_, ancestor| {
                ancestor == effective_enclosing
            })
            .is_some();
        }

        true
    }

    fn is_non_synthesized_declaration(&self, declaration: ast::Node) -> bool {
        self.try_store_for_node(declaration)
            .is_some_and(|store| !ast::node_is_synthesized(store, declaration))
    }

    fn get_enclosing_declaration_ignoring_fake_scope(&self) -> Option<ast::Node> {
        let mut enclosing_declaration = self.ctx.enclosing_declaration;
        while let Some(current) = enclosing_declaration {
            if self.fake_scope_for_signature_declaration(current).is_none() {
                return Some(current);
            }
            enclosing_declaration = self.store_for_node(current).parent(current);
        }
        None
    }

    fn get_resolved_type_without_abstract_construct_signatures(
        &mut self,
        t_type: TypeHandle,
    ) -> TypeHandle {
        let (call_signature_count, signature_count) = {
            let t = self.ch.structured_type_record(t_type);
            (t.call_signature_count, t.signatures.len())
        };
        if call_signature_count == signature_count {
            return t_type;
        }
        if let Some(object_type_without_abstract_construct_signatures) = self
            .ch
            .structured_type_record(t_type)
            .object_type_without_abstract_construct_signatures
        {
            return object_type_without_abstract_construct_signatures;
        }
        let construct_signatures = (call_signature_count..signature_count)
            .map(|index| self.ch.structured_type_record(t_type).signatures[index])
            .filter(|signature| {
                self.ch.signature_record(*signature).flags & SIGNATURE_FLAGS_ABSTRACT == 0
            })
            .collect::<Vec<_>>();
        if construct_signatures.len() == signature_count - call_signature_count {
            return t_type;
        }
        let type_copy = self.ch.new_object_type_from_identity(
            OBJECT_FLAGS_ANONYMOUS,
            self.ch.type_symbol_identity(t_type),
        );
        let (members, properties, call_signatures, index_infos) = {
            let t = self.ch.structured_type_record(t_type);
            (
                t.members.clone(),
                t.properties.clone(),
                t.signatures[..call_signature_count].to_vec(),
                t.index_infos.clone(),
            )
        };
        let call_signature_count = call_signatures.len();
        let signatures = call_signatures
            .into_iter()
            .chain(construct_signatures)
            .collect();
        self.ch.set_structured_type_member_identities(
            type_copy,
            members,
            properties,
            signatures,
            call_signature_count,
            index_infos,
        );
        type_copy
    }

    pub(crate) fn symbol_to_node(
        &mut self,
        symbol: SymbolIdentity,
        meaning: ast::SymbolFlags,
    ) -> ast::Node {
        if self.ctx.internal_flags & nodebuilder::INTERNAL_FLAGS_WRITE_COMPUTED_PROPS != 0 {
            if let Some(value_declaration) = self
                .ch
                .missing_name_symbol_identity_value_declaration(symbol)
            {
                let store = self.store_for_node(value_declaration);
                let name = ast::get_name_of_declaration(store, Some(value_declaration));
                if name
                    .as_ref()
                    .is_some_and(|name| ast::is_computed_property_name(store, *name))
                {
                    return Self::node_value(name.unwrap());
                }
            }
            if self.ch.semantic_state.has_value_symbol_link(symbol) {
                let name_type = self.ch.semantic_state.value_symbol_name_type(symbol);
                if name_type.is_some()
                    && self.ch.type_flags(name_type.unwrap())
                        & (TYPE_FLAGS_ENUM_LITERAL | TYPE_FLAGS_UNIQUE_ES_SYMBOL)
                        != 0
                {
                    let name_type_symbol =
                        self.ch.type_symbol_identity(name_type.unwrap()).unwrap();
                    let old_enclosing = self.ctx.enclosing_declaration;
                    self.ctx.enclosing_declaration = self
                        .ch
                        .missing_name_symbol_identity_value_declaration(name_type_symbol);
                    let Some(expression) =
                        self.symbol_identity_to_expression(name_type_symbol, meaning)
                    else {
                        self.ctx.enclosing_declaration = old_enclosing;
                        return self
                            .symbol_identity_to_expression(symbol, meaning)
                            .expect("symbol expression identity must resolve");
                    };
                    let result = self
                        .e
                        .factory
                        .node_factory
                        .new_computed_property_name(expression);
                    self.ctx.enclosing_declaration = old_enclosing;
                    return Self::node_value(result);
                }
            }
        }
        self.symbol_identity_to_expression(symbol, meaning)
            .expect("symbol expression identity must resolve")
    }

    pub(crate) fn symbol_to_name_identity(
        &mut self,
        symbol: SymbolIdentity,
        meaning: ast::SymbolFlags,
        expects_identifier: bool,
    ) -> ast::Node {
        let chain = self.lookup_symbol_identity_chain(symbol, meaning, false);
        if expects_identifier
            && chain.len() != 1
            && !self.ctx.encountered_error
            && self.ctx.flags & nodebuilder::FLAGS_ALLOW_QUALIFIED_NAME_IN_PLACE_OF_IDENTIFIER != 0
        {
            self.ctx.encountered_error = true;
        }
        self.create_entity_name_from_symbol_identity_chain(chain.clone(), chain.len() - 1)
    }

    fn create_entity_name_from_symbol_identity_chain(
        &mut self,
        chain: Vec<SymbolIdentity>,
        index: usize,
    ) -> ast::Node {
        // typeParameterNodes := b.lookupTypeParameterNodes(chain, index)
        let symbol = chain[index];

        if index == 0 {
            self.ctx.flags |= nodebuilder::FLAGS_IN_INITIAL_ENTITY_NAME;
        }
        let symbol_name = self.get_name_of_symbol_as_written_identity(symbol);
        if index == 0 {
            self.ctx.flags ^= nodebuilder::FLAGS_IN_INITIAL_ENTITY_NAME;
        }

        let identifier = self.new_identifier_with_symbol_identity(&symbol_name, Some(symbol));
        self.e
            .mark_emit_node(&identifier, printer::EF_NO_ASCII_ESCAPING);
        // !!! TODO: smuggle type arguments out
        // if (typeParameterNodes) setIdentifierTypeArguments(identifier, factory.createNodeArray<TypeNode | TypeParameterDeclaration>(typeParameterNodes));
        // identifier.symbol = symbol;
        // expression = identifier;
        if index > 0 {
            let left = self
                .create_entity_name_from_symbol_identity_chain(chain, index - 1)
                .clone();
            return Self::node_value(
                self.e
                    .factory
                    .node_factory
                    .new_qualified_name(left, identifier.clone()),
            );
        }
        identifier
    }

    fn symbol_identity_to_entity_name_node(&mut self, symbol: SymbolIdentity) -> ast::Node {
        let symbol_name = self.ch.symbol_identity_name(symbol);
        let identifier = self.new_identifier_with_symbol_identity(&symbol_name, Some(symbol));
        if let Some(parent) = self.ch.symbol_identity_parent(symbol) {
            let left = self.symbol_identity_to_entity_name_node(parent).clone();
            return Self::node_value(
                self.e
                    .factory
                    .node_factory
                    .new_qualified_name(left, identifier.clone()),
            );
        }
        identifier
    }

    fn symbol_to_type_node(
        &mut self,
        symbol: SymbolIdentity,
        mask: ast::SymbolFlags,
        type_arguments: Option<ast::NodeList>,
    ) -> Option<ast::Node> {
        self.symbol_identity_to_type_node(symbol, mask, type_arguments)
    }

    fn symbol_identity_to_type_node(
        &mut self,
        symbol: SymbolIdentity,
        mask: ast::SymbolFlags,
        type_arguments: Option<ast::NodeList>,
    ) -> Option<ast::Node> {
        let mut chain = self.lookup_symbol_identity_chain(
            symbol,
            mask,
            self.ctx.flags & nodebuilder::FLAGS_USE_ALIAS_DEFINED_OUTSIDE_CURRENT_SCOPE == 0,
        ); // If we're using aliases outside the current scope, dont bother with the module
        if chain.is_empty() {
            return None; // TODO: shouldn't be possible, `lookupSymbolChain` should always at least return the input symbol and issue an error
        }
        let is_type_of = mask == ast::SYMBOL_FLAGS_VALUE;
        if self.symbol_identity_has_non_global_augmentation_external_module_symbol(chain[0]) {
            // module is root, must use `ImportTypeNode`
            let mut non_root_parts = None;
            if chain.len() > 1 {
                non_root_parts = Some(self.create_access_from_symbol_identity_chain(
                    chain.clone(),
                    chain.len() - 1,
                    1,
                    type_arguments,
                ));
            }
            let mut type_parameter_nodes = type_arguments;
            if type_parameter_nodes.is_none() {
                type_parameter_nodes = self.lookup_type_parameter_nodes_identity(chain.clone(), 0);
            }
            let context_file = self
                .ctx
                .enclosing_declaration
                .as_ref()
                .map(|node| self.e.most_original(node))
                .and_then(|node| self.ch.try_source_file_for_node(node)); // TODO: Just use b.ctx.enclosingFile ? Or is the delayed lookup important for context moves?
            let chain_root = chain[0];
            let target_file = self
                .ch
                .missing_name_symbol_identity_value_declaration(chain_root)
                .or_else(|| {
                    self.ch
                        .collect_symbol_identity_declarations(chain_root)
                        .first()
                        .copied()
                })
                .and_then(|declaration| {
                    let store = self.store_for_node(declaration);
                    ast::get_source_file_of_node(store, Some(declaration))
                })
                .and_then(|source_file| self.ch.try_source_file_for_node(source_file));
            let mut specifier = String::new();
            let mut attributes = None;
            if self.ch.compiler_options.get_module_resolution_kind()
                == core::MODULE_RESOLUTION_KIND_NODE16
                || self.ch.compiler_options.get_module_resolution_kind()
                    == core::MODULE_RESOLUTION_KIND_NODE_NEXT
            {
                // An `import` type directed at an esm format file is only going to resolve in esm mode - set the esm mode assertion
                if target_file.is_some()
                    && context_file.is_some()
                    && self
                        .ch
                        .program
                        .get_emit_module_format_of_file(target_file.unwrap())
                        == core::MODULE_KIND_ES_NEXT
                    && self
                        .ch
                        .program
                        .get_emit_module_format_of_file(target_file.unwrap())
                        != self
                            .ch
                            .program
                            .get_emit_module_format_of_file(context_file.unwrap())
                {
                    specifier = self
                        .get_specifier_for_module_symbol_identity(
                            chain_root,
                            core::MODULE_KIND_ES_NEXT,
                        )
                        .to_string();
                    let name = self.new_string_literal("resolution-mode").clone();
                    let value = self.new_string_literal("import").clone();
                    let attribute = self
                        .e
                        .factory
                        .node_factory
                        .new_import_attribute(Some(name), value);
                    let elements = self.new_factory_node_list(vec![attribute]);
                    attributes = Some(self.e.factory.node_factory.new_import_attributes(
                        ast::KIND_WITH_KEYWORD,
                        elements,
                        false,
                    ));
                }
            }
            if specifier.is_empty() {
                specifier = self
                    .get_specifier_for_module_symbol_identity(
                        chain_root,
                        core::RESOLUTION_MODE_NONE,
                    )
                    .to_string();
            }
            if self.ctx.flags & nodebuilder::FLAGS_ALLOW_NODE_MODULES_RELATIVE_PATHS == 0
                && specifier.contains("/node_modules/")
            {
                let old_specifier = specifier.clone();

                if self.ch.compiler_options.get_module_resolution_kind()
                    == core::MODULE_RESOLUTION_KIND_NODE16
                    || self.ch.compiler_options.get_module_resolution_kind()
                        == core::MODULE_RESOLUTION_KIND_NODE_NEXT
                {
                    // We might be able to write a portable import type using a mode override; try specifier generation again, but with a different mode set
                    let mut swapped_mode = core::MODULE_KIND_ES_NEXT;
                    if context_file.map(|file| self.ch.program.get_emit_module_format_of_file(file))
                        == Some(core::MODULE_KIND_ES_NEXT)
                    {
                        swapped_mode = core::MODULE_KIND_COMMON_JS;
                    }
                    specifier = self
                        .get_specifier_for_module_symbol_identity(chain_root, swapped_mode)
                        .to_string();

                    if specifier.contains("/node_modules/") {
                        // Still unreachable :(
                        specifier = old_specifier.clone();
                    } else {
                        let mode_str = if swapped_mode == core::MODULE_KIND_ES_NEXT {
                            "import"
                        } else {
                            "require"
                        };
                        let name = self.new_string_literal("resolution-mode").clone();
                        let value = self.new_string_literal(mode_str).clone();
                        let attribute = self
                            .e
                            .factory
                            .node_factory
                            .new_import_attribute(Some(name), value);
                        let elements = self.new_factory_node_list(vec![attribute]);
                        attributes = Some(self.e.factory.node_factory.new_import_attributes(
                            ast::KIND_WITH_KEYWORD,
                            elements,
                            false,
                        ));
                    }
                }

                if attributes.is_none() {
                    // If ultimately we can only name the symbol with a reference that dives into a `node_modules` folder, we should error
                    // since declaration files with these kinds of references are liable to fail when published :(
                    self.ctx.encountered_error = true;
                    let error_symbol_name = self.ch.symbol_identity_name(symbol).to_string();
                    self.report_likely_unsafe_import_required_error(
                        &old_specifier,
                        &error_symbol_name,
                    );
                }
            }

            let specifier_literal = self.new_string_literal(specifier.as_str()).clone();
            let lit = self
                .e
                .factory
                .node_factory
                .new_literal_type_node(specifier_literal);
            self.ctx.approximate_length += specifier.len() + 10; // specifier + import("")
            if non_root_parts.as_ref().is_none_or(|non_root_parts| {
                ast::is_entity_name(self.store_for_node(*non_root_parts), *non_root_parts)
            }) {
                return Some(Self::node_value(
                    self.e.factory.node_factory.new_import_type_node(
                        is_type_of,
                        lit,
                        attributes,
                        non_root_parts,
                        type_parameter_nodes,
                    ),
                ));
            }

            let non_root_parts_node = *non_root_parts.as_ref().unwrap();
            let non_root_parts_store = self.source_store_for_node(non_root_parts_node);
            let split_node =
                get_topmost_indexed_access_type(non_root_parts_store, non_root_parts_node);
            let qualifier = non_root_parts_store.object_type(split_node).unwrap();
            let qualifier = non_root_parts_store.type_name(qualifier);
            let object_type = self.e.factory.node_factory.new_import_type_node(
                is_type_of,
                lit,
                attributes,
                qualifier,
                type_parameter_nodes,
            );
            return Some(Self::node_value(
                self.e.factory.node_factory.new_indexed_access_type_node(
                    object_type,
                    non_root_parts_store.index_type(split_node).unwrap(),
                ),
            ));
        }

        let entity_name = self.create_access_from_symbol_identity_chain(
            chain.clone(),
            chain.len() - 1,
            0,
            type_arguments,
        );
        if ast::is_indexed_access_type_node(self.store_for_node(entity_name), entity_name) {
            return Some(entity_name); // Indexed accesses can never be `typeof`
        }
        if ast::is_entity_name(self.store_for_node(entity_name), entity_name) {
            if is_type_of {
                return Some(Self::node_value(
                    self.e
                        .factory
                        .node_factory
                        .new_type_query_node(entity_name.clone(), None),
                ));
            }
            return Some(Self::node_value(
                self.e
                    .factory
                    .node_factory
                    .new_type_reference_node(entity_name.clone(), type_arguments),
            ));
        }
        if is_type_of
            && ast::is_expression_with_type_arguments(self.store_for_node(entity_name), entity_name)
        {
            let (expr_expression, expr_type_arguments) = {
                let entity_name_store = self.store_for_node(entity_name);
                (
                    entity_name_store.expression(entity_name).unwrap(),
                    entity_name_store
                        .source_type_arguments(entity_name)
                        .map(snapshot_source_node_list),
                )
            };
            let expression = self.deep_clone_node(expr_expression);
            let expr_type_arguments = expr_type_arguments.map(|type_arguments| {
                self.clone_source_node_list_snapshot_to_factory(type_arguments)
            });
            return Some(Self::node_value(
                self.e
                    .factory
                    .node_factory
                    .new_type_query_node(expression, expr_type_arguments),
            ));
        }
        Some(entity_name)
    }

    fn create_access_from_symbol_identity_chain(
        &mut self,
        chain: Vec<SymbolIdentity>,
        index: usize,
        stopper: usize,
        override_type_arguments: Option<ast::NodeList>,
    ) -> ast::Node {
        let mut type_parameter_nodes = override_type_arguments;
        if index != chain.len() - 1 {
            type_parameter_nodes = self.lookup_type_parameter_nodes_identity(chain.clone(), index);
        }
        let symbol = chain[index];
        let parent = if index > 0 {
            Some(chain[index - 1])
        } else {
            None
        };

        let mut symbol_name = String::new();
        if index == 0 {
            self.ctx.flags |= nodebuilder::FLAGS_IN_INITIAL_ENTITY_NAME;
            symbol_name = self
                .get_name_of_symbol_as_written_identity(symbol)
                .to_string();
            self.ctx.approximate_length += symbol_name.len() + 1;
            self.ctx.flags ^= nodebuilder::FLAGS_IN_INITIAL_ENTITY_NAME;
        } else {
            // lookup a ref to symbol within parent to handle export aliases
            if let Some(parent) = parent {
                if !self.ch.symbol_identity_exports_are_empty(parent) {
                    // avoid exhaustive iteration in the common case
                    let raw_symbol_name = self.ch.symbol_identity_name(symbol).to_string();
                    let res = self
                        .ch
                        .lookup_symbol_identity_export(parent, raw_symbol_name.as_str());
                    let res_is_same = res.is_some_and(|res| {
                        self.ch.get_symbol_if_same_reference(res, symbol).is_some()
                    });
                    if raw_symbol_name != ast::INTERNAL_SYMBOL_NAME_EXPORT_EQUALS
                        && !is_late_bound_name(raw_symbol_name.as_str())
                        && res_is_same
                    {
                        symbol_name = raw_symbol_name;
                    } else {
                        let exports =
                            self.ch
                                .with_symbol_identity_export_table(parent, |exports| {
                                    exports
                                .map(crate::checker::SymbolIdentityTableView::materialize_entries)
                                .unwrap_or_default()
                                });
                        let mut results = HashMap::with_capacity(1);
                        for (name, ex) in exports {
                            {
                                if self.ch.get_symbol_if_same_reference(ex, symbol).is_some()
                                    && !is_late_bound_name(&name)
                                    && name != ast::INTERNAL_SYMBOL_NAME_EXPORT_EQUALS
                                {
                                    results.insert(ex, name);
                                }
                                // break // must collect all results and sort them - exports are randomly iterated
                            }
                        }
                        let mut result_symbols = results.keys().copied().collect::<Vec<_>>();
                        if !result_symbols.is_empty() {
                            self.ch.sort_symbol_identities(&mut result_symbols);
                            symbol_name = results[&result_symbols[0]].to_string();
                        }
                    }
                }
            }
        }

        if symbol_name.is_empty() {
            let mut name = None;
            for d in self.ch.collect_symbol_identity_declarations(symbol) {
                let store = self.store_for_node(d);
                name = ast::get_name_of_declaration(store, Some(d));
                if name.is_some() {
                    break;
                }
            }
            let computed_name_expression = name
                .as_ref()
                .filter(|name| ast::is_computed_property_name(self.store_for_node(**name), **name))
                .and_then(|name| self.store_for_node(*name).expression(*name));
            if computed_name_expression.as_ref().is_some_and(|expression| {
                ast::is_entity_name(self.store_for_node(*expression), *expression)
            }) {
                let lhs = self.create_access_from_symbol_identity_chain(
                    chain.clone(),
                    index - 1,
                    stopper,
                    override_type_arguments,
                );
                if ast::is_entity_name(self.store_for_node(lhs), lhs) {
                    let type_query = self
                        .e
                        .factory
                        .node_factory
                        .new_type_query_node(lhs.clone(), None);
                    let object_type = self
                        .e
                        .factory
                        .node_factory
                        .new_parenthesized_type_node(type_query);
                    let index_type = self
                        .e
                        .factory
                        .node_factory
                        .new_type_query_node(computed_name_expression.unwrap(), None);
                    return Self::node_value(
                        self.e
                            .factory
                            .node_factory
                            .new_indexed_access_type_node(object_type, index_type),
                    );
                }
                return lhs;
            }
            symbol_name = self
                .get_name_of_symbol_as_written_identity(symbol)
                .to_string();
        }
        self.ctx.approximate_length += symbol_name.len() + 1;

        if self.ctx.flags & nodebuilder::FLAGS_FORBID_INDEXED_ACCESS_SYMBOL_REFERENCES == 0
            && parent.is_some()
        {
            let parent = parent.unwrap();
            let raw_symbol_name = self.ch.symbol_identity_name(symbol).to_string();
            let member = self
                .ch
                .with_members_of_symbol_identities(parent, |members| {
                    members.and_then(|members| members.get(raw_symbol_name.as_str()))
                });
            let member_is_same = member.is_some_and(|member| {
                self.ch
                    .get_symbol_if_same_reference(member, symbol)
                    .is_some()
            });
            if member_is_same {
                // Should use an indexed access
                let lhs = self.create_access_from_symbol_identity_chain(
                    chain.clone(),
                    index - 1,
                    stopper,
                    override_type_arguments,
                );
                let literal = self.new_string_literal(symbol_name.as_str()).clone();
                let index_type = self.e.factory.node_factory.new_literal_type_node(literal);
                if ast::is_indexed_access_type_node(self.store_for_node(lhs), lhs) {
                    return Self::node_value(
                        self.e
                            .factory
                            .node_factory
                            .new_indexed_access_type_node(lhs.clone(), index_type),
                    );
                }
                let object_type = self
                    .e
                    .factory
                    .node_factory
                    .new_type_reference_node(lhs.clone(), type_parameter_nodes);
                return Self::node_value(
                    self.e
                        .factory
                        .node_factory
                        .new_indexed_access_type_node(object_type, index_type),
                );
            }
        }

        let identifier =
            self.new_identifier_with_symbol_identity(symbol_name.as_str(), Some(symbol));
        self.e
            .mark_emit_node(&identifier, printer::EF_NO_ASCII_ESCAPING);

        if index > stopper {
            let lhs = self.create_access_from_symbol_identity_chain(
                chain,
                index - 1,
                stopper,
                override_type_arguments,
            );
            if self.ctx.flags & nodebuilder::FLAGS_USE_INSTANTIATION_EXPRESSIONS == 0
                || ast::is_entity_name(self.store_for_node(lhs), lhs)
                    && (type_parameter_nodes.is_none()
                        || self
                            .e
                            .factory
                            .node_factory
                            .emit_node_list_nodes(type_parameter_nodes.unwrap())
                            .is_empty())
            {
                return Self::node_value(
                    self.e
                        .factory
                        .node_factory
                        .new_qualified_name(lhs.clone(), identifier.clone()),
                );
            }
            let expression = self.create_access_expression(lhs).clone();
            let access =
                Self::node_value(self.e.factory.node_factory.new_property_access_expression(
                    expression,
                    None,
                    identifier.clone(),
                    ast::NODE_FLAGS_NONE,
                ));
            return self.create_expression_with_type_arguments(access, type_parameter_nodes);
        }
        identifier
    }

    pub(crate) fn symbol_to_expression(
        &mut self,
        symbol: SymbolIdentity,
        mask: ast::SymbolFlags,
    ) -> ast::Node {
        self.symbol_identity_to_expression(symbol, mask)
            .expect("symbol expression identity must resolve")
    }

    fn get_local_type_parameters_of_class_or_interface_or_type_alias_identity(
        &mut self,
        symbol: SymbolIdentity,
    ) -> Vec<TypeHandle> {
        let mut results = Vec::new();
        self.ch
            .for_each_symbol_identity_declaration(symbol, |checker, node| {
                let store = checker.store_for_node(node);
                if ast::node_kind_is(
                    store,
                    &node,
                    &[
                        ast::Kind::InterfaceDeclaration,
                        ast::Kind::ClassDeclaration,
                        ast::Kind::ClassExpression,
                    ],
                ) || is_type_alias(store, node)
                {
                    let current = std::mem::take(&mut results);
                    results = checker.append_type_parameters(
                        current,
                        store
                            .type_parameters(node)
                            .map(|type_parameters| type_parameters.iter().collect())
                            .unwrap_or_default(),
                    );
                }
            });
        results
    }

    pub(crate) fn symbol_identity_to_expression(
        &mut self,
        symbol: SymbolIdentity,
        mask: ast::SymbolFlags,
    ) -> Option<ast::Node> {
        let chain = self.lookup_symbol_identity_chain(symbol, mask, false);
        Some(self.create_expression_from_symbol_identity_chain(chain.clone(), chain.len() - 1))
    }

    fn create_expression_from_symbol_identity_chain(
        &mut self,
        chain: Vec<SymbolIdentity>,
        index: usize,
    ) -> ast::Node {
        let type_parameter_nodes =
            self.lookup_expression_chain_type_argument_nodes_identity(chain.clone(), index);
        let symbol = chain[index];

        if index == 0 {
            self.ctx.flags |= nodebuilder::FLAGS_IN_INITIAL_ENTITY_NAME;
        }
        let mut symbol_name = self
            .get_name_of_symbol_as_written_identity(symbol)
            .to_string();
        if index == 0 {
            self.ctx.flags ^= nodebuilder::FLAGS_IN_INITIAL_ENTITY_NAME;
        }

        if starts_with_single_or_double_quote(symbol_name.as_str())
            && self.symbol_identity_has_non_global_augmentation_external_module_symbol(symbol)
        {
            let specifier =
                self.get_specifier_for_module_symbol_identity(symbol, core::RESOLUTION_MODE_NONE);
            self.ctx.approximate_length += 2 + specifier.len();
            return self.new_string_literal(&specifier);
        }

        if index == 0 || can_use_property_access(symbol_name.as_str()) {
            let identifier =
                self.new_identifier_with_symbol_identity(symbol_name.as_str(), Some(symbol));
            self.e
                .mark_emit_node(&identifier, printer::EF_NO_ASCII_ESCAPING);
            self.ctx.approximate_length += 1 + symbol_name.len();
            if index > 0 {
                let expression = self
                    .create_expression_from_symbol_identity_chain(chain, index - 1)
                    .clone();
                let result =
                    Self::node_value(self.e.factory.node_factory.new_property_access_expression(
                        expression,
                        None,
                        identifier.clone(),
                        ast::NODE_FLAGS_NONE,
                    ));
                self.e.mark_emit_node(&result, printer::EF_NO_INDENTATION);
                return self.create_expression_with_type_arguments(result, type_parameter_nodes);
            }
            return self.create_expression_with_type_arguments(identifier, type_parameter_nodes);
        }

        if starts_with_square_bracket(symbol_name.as_str()) {
            symbol_name = symbol_name[1..symbol_name.len() - 1].to_string();
        }

        let mut expression = None;
        if starts_with_single_or_double_quote(symbol_name.as_str())
            && self.symbol_identity_flags(symbol) & ast::SYMBOL_FLAGS_ENUM_MEMBER == 0
        {
            let literal_text = stringutil::unquote_string(symbol_name.as_str());
            self.ctx.approximate_length += literal_text.len() + 2;
            expression =
                Some(self.new_string_literal_ex(&literal_text, symbol_name.as_bytes()[0] == b'\''));
        } else if jsnum::from_string(symbol_name.as_str()).to_string() == symbol_name {
            // TODO: the follwing in strada would assert if the number is negative, but no such assertion exists here
            // Moreover, what's even guaranteeing the name *isn't* -1 here anyway? Needs double-checking.
            self.ctx.approximate_length += symbol_name.len();
            expression = Some(Self::node_value(
                self.e
                    .factory
                    .node_factory
                    .new_numeric_literal(symbol_name.as_str(), ast::TOKEN_FLAGS_NONE),
            ));
        }
        let expression = expression.unwrap_or_else(|| {
            self.ctx.approximate_length += symbol_name.len();
            let expression =
                self.new_identifier_with_symbol_identity(symbol_name.as_str(), Some(symbol));
            self.e
                .mark_emit_node(&expression, printer::EF_NO_ASCII_ESCAPING);
            expression
        });
        self.ctx.approximate_length += 2; // []
        let target = self
            .create_expression_from_symbol_identity_chain(chain, index - 1)
            .clone();
        let access = Self::node_value(self.e.factory.node_factory.new_element_access_expression(
            target,
            None,
            expression.clone(),
            ast::NODE_FLAGS_NONE,
        ));
        self.create_expression_with_type_arguments(access, type_parameter_nodes)
    }

    fn get_name_of_symbol_from_name_type_identity(&mut self, symbol: SymbolIdentity) -> String {
        let name_type = self.ch.semantic_state.try_value_symbol_name_type(symbol);
        let Some(name_type) = name_type else {
            return String::new();
        };
        if self.ch.type_flags(name_type) & TYPE_FLAGS_STRING_OR_NUMBER_LITERAL != 0 {
            let mut name = String::new();
            match &self.ch.type_record(name_type).as_literal_type().value {
                LiteralValue::String(v) => name = v.to_string(),
                LiteralValue::Number(v) => name = v.to_string(),
                _ => {}
            }
            if !scanner::is_identifier_text(name.as_str(), core::LANGUAGE_VARIANT_STANDARD)
                && !is_numeric_literal_name(name.as_str())
            {
                return self.ch.get_property_name_from_type(name_type);
            }
            if is_numeric_literal_name(name.as_str()) && name.starts_with('-') {
                return format!("[{}]", name);
            }
            return name;
        }
        if self.ch.type_flags(name_type) & TYPE_FLAGS_UNIQUE_ES_SYMBOL != 0 {
            let symbol = self.ch.type_symbol_identity(name_type).unwrap();
            let text = self.get_name_of_symbol_as_written_identity(symbol);
            return format!("[{}]", text);
        }
        String::new()
    }

    fn get_name_of_symbol_as_written_identity(&mut self, symbol: SymbolIdentity) -> String {
        let symbol = self
            .ctx
            .remapped_symbol_references
            .get(&symbol)
            .copied()
            .unwrap_or(symbol);
        let symbol_name = self.ch.symbol_identity_name(symbol).to_string();
        let declarations = self.ch.collect_symbol_identity_declarations(symbol);
        if symbol_name == ast::INTERNAL_SYMBOL_NAME_DEFAULT
            && self.ctx.flags & nodebuilder::FLAGS_USE_ALIAS_DEFINED_OUTSIDE_CURRENT_SCOPE == 0
            && (self.ctx.flags & nodebuilder::FLAGS_IN_INITIAL_ENTITY_NAME == 0
                || declarations.is_empty()
                || (self.ctx.enclosing_declaration.is_some() && {
                    let declaration_store = self.store_for_node(declarations[0]);
                    let symbol_ancestor = ast::find_ancestor(
                        declaration_store,
                        Some(declarations[0]),
                        |store, node| is_default_binding_context(store, node),
                    );
                    let enclosing_declaration = self.ctx.enclosing_declaration.unwrap();
                    let enclosing_store = self.store_for_node(enclosing_declaration);
                    let enclosing_ancestor = ast::find_ancestor(
                        enclosing_store,
                        Some(enclosing_declaration),
                        |store, node| is_default_binding_context(store, node),
                    );
                    symbol_ancestor != enclosing_ancestor
                }))
        {
            return "default".to_string();
        }
        if !declarations.is_empty() {
            let declaration_and_name = declarations.iter().find_map(|d| {
                let store = self.store_for_node(*d);
                ast::get_name_of_declaration(store, Some(*d)).map(|name| (*d, name))
            });
            if let Some((declaration, name)) = declaration_and_name {
                if ast::is_computed_property_name(self.store_for_node(name), name)
                    && self.ch.symbol_identity_check_flags(symbol) & ast::CHECK_FLAGS_LATE == 0
                {
                    let name_type = self.ch.semantic_state.try_value_symbol_name_type(symbol);
                    if name_type.is_some_and(|name_type| {
                        self.ch.type_flags(name_type) & TYPE_FLAGS_STRING_OR_NUMBER_LITERAL != 0
                    }) {
                        let result = self.get_name_of_symbol_from_name_type_identity(symbol);
                        if !result.is_empty() {
                            return result;
                        }
                    }
                }
                let declaration_name = scanner::declaration_name_to_string(
                    self.ch.source_file_for_node(name),
                    Some(&name),
                );
                if is_late_bound_name(&symbol_name)
                    && declaration_name.starts_with(ast::INTERNAL_SYMBOL_NAME_PREFIX)
                    && let Some(name) = late_bound_symbol_name_to_string(&symbol_name)
                {
                    return name;
                }
                return declaration_name;
            }
            let declaration = declarations[0];
            let declaration_store = self.store_for_node(declaration);
            if let Some(parent) = declaration_store.parent(declaration) {
                if declaration_store.kind(parent) == ast::KIND_VARIABLE_DECLARATION {
                    let name = declaration_store.name(parent);
                    if let Some(name) = name {
                        return scanner::declaration_name_to_string(
                            self.ch.source_file_for_node(name),
                            Some(&name),
                        );
                    }
                    return symbol_name;
                }
            }
            let declaration_kind = declaration_store.kind(declaration);
            if declaration_kind == ast::KIND_CLASS_EXPRESSION
                || declaration_kind == ast::KIND_FUNCTION_EXPRESSION
                || declaration_kind == ast::KIND_ARROW_FUNCTION
            {
                if !self.ctx.encountered_error
                    && self.ctx.flags & nodebuilder::FLAGS_ALLOW_ANONYMOUS_IDENTIFIER == 0
                {
                    self.ctx.encountered_error = true;
                }
                match declaration_kind {
                    ast::KIND_CLASS_EXPRESSION => return "(Anonymous class)".to_string(),
                    ast::KIND_FUNCTION_EXPRESSION | ast::KIND_ARROW_FUNCTION => {
                        return "(Anonymous function)".to_string();
                    }
                    _ => {}
                }
            }
        }
        let name = self.get_name_of_symbol_from_name_type_identity(symbol);
        if !name.is_empty() {
            return name;
        }
        if symbol_name == ast::INTERNAL_SYMBOL_NAME_MISSING {
            return "__missing".to_string();
        }
        if let Some(name) = late_bound_symbol_name_to_string(&symbol_name) {
            return name;
        }
        symbol_name
    }

    fn get_type_parameters_of_class_or_interface_identity(
        &mut self,
        symbol: SymbolIdentity,
    ) -> Vec<TypeHandle> {
        let mut result = Vec::new();
        result.extend(self.get_outer_type_parameters_of_class_or_interface_identity(symbol));
        result.extend(
            self.get_local_type_parameters_of_class_or_interface_or_type_alias_identity(symbol),
        );
        result
    }

    fn get_base_type_variable_of_class_identity(
        &mut self,
        symbol: SymbolIdentity,
    ) -> Option<TypeHandle> {
        let declared_type = self
            .ch
            .get_declared_type_of_symbol_identity_or_error(symbol);
        let base_constructor_type = self.ch.get_base_constructor_type_of_class(declared_type);
        if self.ch.type_flags(base_constructor_type) & TYPE_FLAGS_TYPE_VARIABLE != 0 {
            return Some(base_constructor_type);
        }
        if self.ch.type_flags(base_constructor_type) & TYPE_FLAGS_INTERSECTION != 0 {
            return self
                .ch
                .type_types(base_constructor_type)
                .iter()
                .find(|t| self.ch.type_flags(**t) & TYPE_FLAGS_TYPE_VARIABLE != 0)
                .copied();
        }
        None
    }

    fn get_outer_type_parameters_of_class_or_interface_identity(
        &mut self,
        symbol: SymbolIdentity,
    ) -> Vec<TypeHandle> {
        let symbol_flags = self.symbol_identity_flags(symbol);
        let mut declaration = self
            .ch
            .missing_name_symbol_identity_value_declaration(symbol);
        if !symbol_flags.intersects(ast::SYMBOL_FLAGS_CLASS | ast::SYMBOL_FLAGS_FUNCTION) {
            declaration = self
                .ch
                .collect_symbol_identity_declarations(symbol)
                .into_iter()
                .find(|d| {
                    if ast::is_interface_declaration(self.store_for_node(*d), *d) {
                        return true;
                    }
                    if !ast::is_variable_declaration(self.store_for_node(*d), *d) {
                        return false;
                    }
                    let initializer = self.store_for_node(*d).initializer(*d);
                    initializer.is_some()
                        && ast::is_function_expression_or_arrow_function(
                            self.store_for_node(initializer.unwrap()),
                            initializer.unwrap(),
                        )
                });
        }
        debug::assert(
            declaration.is_some(),
            Some(
                "Class was missing valueDeclaration -OR- non-class had no interface declarations"
                    .to_string(),
            ),
        );
        self.ch
            .get_outer_type_parameters(declaration.unwrap(), false /*includeThisTypes*/)
    }

    fn lookup_type_parameter_nodes_identity(
        &mut self,
        chain: Vec<SymbolIdentity>,
        index: usize,
    ) -> Option<ast::NodeList> {
        debug_assert!(!chain.is_empty() && index < chain.len());
        let symbol = chain[index];
        if self.ctx.type_parameter_symbol_list.has(&symbol) {
            return None;
        }
        self.ctx.type_parameter_symbol_list.add(symbol);

        if self.ctx.flags & nodebuilder::FLAGS_WRITE_TYPE_PARAMETERS_IN_QUALIFIED_NAME != 0
            && index < chain.len() - 1
        {
            if let Some(type_argument_nodes) =
                self.lookup_instantiated_type_argument_nodes_identity(chain.clone(), index)
            {
                return Some(type_argument_nodes);
            }
            let type_parameter_nodes =
                self.type_parameters_to_type_parameter_declarations_identity(symbol);
            if !type_parameter_nodes.is_empty() {
                return Some(Self::output_node_list_value(self.new_factory_node_list(
                    type_parameter_nodes.into_iter().collect::<Vec<_>>(),
                )));
            }
        }

        None
    }

    fn type_parameters_to_type_parameter_declarations_identity(
        &mut self,
        symbol: SymbolIdentity,
    ) -> Vec<ast::Node> {
        let target_flags = self.symbol_identity_flags(symbol);
        if target_flags
            & (ast::SYMBOL_FLAGS_CLASS | ast::SYMBOL_FLAGS_INTERFACE | ast::SYMBOL_FLAGS_ALIAS)
            != 0
        {
            let mut results = Vec::new();
            let declarations = self.ch.collect_symbol_identity_declarations(symbol);
            let mut params = Vec::new();
            for node in declarations {
                let store = self.store_for_node(node);
                if ast::node_kind_is(
                    store,
                    &node,
                    &[
                        ast::Kind::InterfaceDeclaration,
                        ast::Kind::ClassDeclaration,
                        ast::Kind::ClassExpression,
                    ],
                ) || is_type_alias(store, node)
                {
                    params = self.ch.append_type_parameters(
                        params,
                        store
                            .type_parameters(node)
                            .map(|type_parameters| type_parameters.iter().collect())
                            .unwrap_or_default(),
                    );
                }
            }
            for param in params {
                results.push(self.type_parameter_to_declaration(param));
            }
            return results;
        } else if target_flags & ast::SYMBOL_FLAGS_FUNCTION != 0 {
            let mut results = Vec::new();
            let value_declaration = self
                .ch
                .missing_name_symbol_identity_value_declaration(symbol)
                .expect("function symbol should have a value declaration");
            for param in self
                .ch
                .get_type_parameters_from_declaration(value_declaration)
            {
                results.push(self.type_parameter_to_declaration(param));
            }
            return results;
        }
        Vec::new()
    }

    // TODO: move `lookupSymbolChain` and co to `symbolaccessibility.go` (but getSpecifierForModuleSymbol uses much context which makes that hard?)
    pub(crate) fn lookup_symbol_identity_chain(
        &mut self,
        symbol: SymbolIdentity,
        meaning: ast::SymbolFlags,
        yield_module_symbol: bool,
    ) -> Vec<SymbolIdentity> {
        let symbol_flags = self.symbol_identity_flags(symbol);
        self.track_symbol_identity_with_flags(
            symbol,
            symbol_flags,
            self.ctx.enclosing_declaration,
            meaning,
        );
        self.lookup_symbol_identity_chain_worker(symbol, symbol_flags, meaning, yield_module_symbol)
    }

    fn lookup_symbol_identity_chain_worker(
        &mut self,
        symbol: SymbolIdentity,
        symbol_flags: ast::SymbolFlags,
        meaning: ast::SymbolFlags,
        yield_module_symbol: bool,
    ) -> Vec<SymbolIdentity> {
        let is_type_parameter = symbol_flags & ast::SYMBOL_FLAGS_TYPE_PARAMETER != 0;
        if !is_type_parameter
            && (self.ctx.enclosing_declaration.is_some()
                || self.ctx.flags & nodebuilder::FLAGS_USE_FULLY_QUALIFIED_TYPE != 0)
            && self.ctx.internal_flags & nodebuilder::INTERNAL_FLAGS_DO_NOT_INCLUDE_SYMBOL_CHAIN
                == 0
        {
            let chain = self.get_symbol_identity_chain(
                symbol,
                meaning, /*endOfChain*/
                true,
                yield_module_symbol,
            );
            debug_assert!(!chain.is_empty());
            return chain;
        }
        vec![symbol]
    }

    /** @param endOfChain Set to false for recursive calls; non-recursive calls should always output something. */
    fn get_symbol_identity_chain(
        &mut self,
        symbol: SymbolIdentity,
        meaning: ast::SymbolFlags,
        end_of_chain: bool,
        yield_module_symbol: bool,
    ) -> Vec<SymbolIdentity> {
        let mut accessible_symbol_chain = self
            .get_accessible_symbol_identity_chain_in_builder_scope(
                symbol,
                self.ctx.enclosing_declaration,
                meaning,
                self.ctx.flags & nodebuilder::FLAGS_USE_ONLY_EXTERNAL_ALIASING != 0,
            );
        let mut qualifier_meaning = meaning;
        if accessible_symbol_chain.len() > 1 {
            qualifier_meaning = get_qualified_left_meaning(meaning);
        }
        if accessible_symbol_chain.is_empty()
            || self.needs_qualification_in_builder_scope_identity(
                accessible_symbol_chain[0],
                self.ctx.enclosing_declaration,
                qualifier_meaning,
            )
        {
            let root = accessible_symbol_chain.first().copied().unwrap_or(symbol);
            let checker_enclosing_declaration = self
                .ctx
                .enclosing_declaration
                .and_then(|declaration| self.checker_accessible_enclosing_declaration(declaration));
            let parents = self.ch.get_containers_of_symbol_identity(
                root,
                checker_enclosing_declaration,
                meaning,
            );
            if !parents.is_empty() {
                let mut parent_specifiers = parents
                    .into_iter()
                    .map(|symbol| {
                        let ch = &*self.ch;
                        if ch.with_symbol_identity_declarations(symbol, |declarations| {
                            declarations.iter().any(|d| {
                                let store = ch.store_for_node(*d);
                                has_non_global_augmentation_external_module_symbol(ch, store, *d)
                            })
                        }) {
                            return SortedSymbolIdentityNamePair {
                                sym: symbol,
                                name: self
                                    .get_specifier_for_module_symbol_identity(
                                        symbol,
                                        core::RESOLUTION_MODE_NONE,
                                    )
                                    .to_string(),
                            };
                        }
                        SortedSymbolIdentityNamePair {
                            sym: symbol,
                            name: String::new(),
                        }
                    })
                    .collect::<Vec<_>>();
                parent_specifiers.sort_by(|a, b| self.sort_by_best_identity_name(a, b).cmp(&0));
                for pair in parent_specifiers {
                    let parent = pair.sym;
                    let parent_chain = self.get_symbol_identity_chain(
                        parent,
                        get_qualified_left_meaning(meaning),
                        false,
                        yield_module_symbol,
                    );
                    if !parent_chain.is_empty() {
                        if let Some(exported) = self.ch.lookup_symbol_identity_export(
                            parent,
                            ast::INTERNAL_SYMBOL_NAME_EXPORT_EQUALS,
                        ) {
                            if self
                                .ch
                                .get_symbol_if_same_reference(exported, symbol)
                                .is_some()
                            {
                                accessible_symbol_chain = parent_chain;
                                break;
                            }
                        }
                        let mut next_syms = accessible_symbol_chain.clone();
                        if next_syms.is_empty() {
                            let fallback = self
                                .ch
                                .get_alias_for_symbol_in_container_identity(parent, symbol)
                                .unwrap_or(symbol);
                            next_syms.push(fallback);
                        }
                        accessible_symbol_chain = parent_chain;
                        accessible_symbol_chain.extend(next_syms);
                        break;
                    }
                }
            }
        }
        if !accessible_symbol_chain.is_empty() {
            return accessible_symbol_chain;
        }
        let symbol_flags = self.symbol_identity_flags(symbol);
        if end_of_chain
            || symbol_flags & (ast::SYMBOL_FLAGS_TYPE_LITERAL | ast::SYMBOL_FLAGS_OBJECT_LITERAL)
                == 0
        {
            if !end_of_chain
                && !yield_module_symbol
                && self
                    .ch
                    .collect_symbol_identity_declarations(symbol)
                    .iter()
                    .any(|d| {
                        let store = self.store_for_node(*d);
                        has_non_global_augmentation_external_module_symbol(self.ch, store, *d)
                    })
            {
                return Vec::new();
            }
            return vec![symbol];
        }
        Vec::new()
    }

    fn sort_by_best_identity_name(
        &mut self,
        a: &SortedSymbolIdentityNamePair,
        b: &SortedSymbolIdentityNamePair,
    ) -> i32 {
        let specifier_a = a.name.as_str();
        let specifier_b = b.name.as_str();
        if !specifier_a.is_empty() && !specifier_b.is_empty() {
            let is_b_relative = tspath::path_is_relative(specifier_b);
            if tspath::path_is_relative(specifier_a) == is_b_relative {
                return modulespecifiers::count_path_components(specifier_a) as i32
                    - modulespecifiers::count_path_components(specifier_b) as i32;
            }
            if is_b_relative {
                return -1;
            }
            return 1;
        }
        match self.ch.compare_symbol_identities(a.sym, b.sym) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }
    }

    pub(crate) fn get_specifier_for_module_symbol_identity(
        &mut self,
        symbol: SymbolIdentity,
        override_import_mode: core::ResolutionMode,
    ) -> String {
        let symbol_handle = symbol.symbol_handle();
        self.get_specifier_for_module_symbol_handle(symbol_handle, override_import_mode)
    }

    pub(crate) fn get_specifier_for_module_symbol_handle(
        &mut self,
        symbol: ast::SymbolHandle,
        override_import_mode: core::ResolutionMode,
    ) -> String {
        let cache_identity = SymbolIdentity::from_symbol_handle(symbol);
        let mut specifier_symbol = symbol;
        if self
            .symbol_handle_declaration_of_kind(symbol, ast::KIND_SOURCE_FILE)
            .is_none()
        {
            let equivalent_symbol = self
                .ch
                .collect_symbol_handle_declarations(symbol)
                .into_iter()
                .find_map(|declaration| {
                    self.get_file_symbol_if_file_symbol_export_equals_container_handle(
                        declaration,
                        symbol,
                    )
                });
            if let Some(equivalent_symbol) = equivalent_symbol {
                if self
                    .symbol_handle_declaration_of_kind(equivalent_symbol, ast::KIND_SOURCE_FILE)
                    .is_some()
                {
                    specifier_symbol = equivalent_symbol;
                }
            }
        }

        let symbol_data = self.module_symbol_data_from_handle(specifier_symbol);
        self.compute_specifier_for_module_symbol_data(
            &symbol_data,
            cache_identity,
            override_import_mode,
        )
    }

    fn compute_specifier_for_module_symbol_data(
        &mut self,
        symbol: &modulespecifiers::ModuleSymbolData,
        cache_identity: SymbolIdentity,
        override_import_mode: core::ResolutionMode,
    ) -> String {
        if self
            .declaration_of_kind_data(symbol, ast::KIND_SOURCE_FILE)
            .is_none()
            && ast::is_ambient_module_symbol_name(symbol.name.as_str())
        {
            return stringutil::strip_quotes(symbol.name.as_str()).to_string();
        }
        if self.ctx.enclosing_file.is_none() {
            if ast::is_ambient_module_symbol_name(symbol.name.as_str()) {
                return stringutil::strip_quotes(symbol.name.as_str()).to_string();
            }
            return symbol
                .value_declaration
                .as_ref()
                .or_else(|| symbol.declarations.first())
                .and_then(|declaration| {
                    let store = self.store_for_node(*declaration);
                    ast::get_source_file_of_node(store, Some(*declaration))
                })
                .map(|source_file| {
                    let store = self.store_for_node(source_file);
                    store.source_file_view(source_file).file_name()
                })
                .unwrap();
        }

        let enclosing_declaration = self
            .ctx
            .enclosing_declaration
            .as_ref()
            .map(|node| self.e.most_original(node));
        let mut original_module_specifier = None;
        if let Some(enclosing_declaration) = enclosing_declaration.as_ref() {
            let store = self.store_for_node(*enclosing_declaration);
            if can_have_module_specifier(store, Some(*enclosing_declaration)) {
                original_module_specifier =
                    try_get_module_specifier_from_declaration(store, Some(*enclosing_declaration));
            }
        }
        let context_file = self
            .enclosing_file()
            .expect("enclosing file required when serializing module specifier");
        let mut resolution_mode = override_import_mode;
        if resolution_mode == core::RESOLUTION_MODE_NONE && original_module_specifier.is_some() {
            resolution_mode = self
                .ch
                .program
                .get_mode_for_usage_location(context_file, &original_module_specifier.unwrap());
        } else if resolution_mode == core::RESOLUTION_MODE_NONE {
            resolution_mode = self
                .ch
                .program
                .get_default_resolution_mode_for_file(context_file);
        }
        let cache_key = module::ModeAwareCacheKey {
            name: context_file.path().to_string(),
            mode: resolution_mode,
        };
        let links = self.symbol_links.entry(cache_identity).or_default();
        if let Some(result) = links.specifier_cache.get(&cache_key) {
            return result.clone();
        }
        // For declaration bundles, we need to generate absolute paths relative to the common source dir for imports,
        // just like how the declaration emitter does for the ambient module declarations - we can easily accomplish this
        // using the `baseUrl` compiler option (which we would otherwise never use in declaration emit) and a non-relative
        // specifier preference
        let host = self.ctx.host;
        let specifier_compiler_options = self.ch.compiler_options;
        let specifier_pref = modulespecifiers::IMPORT_MODULE_SPECIFIER_PREFERENCE_PROJECT_RELATIVE;
        let mut ending_pref = modulespecifiers::IMPORT_MODULE_SPECIFIER_ENDING_PREFERENCE_NONE;
        if resolution_mode == core::RESOLUTION_MODE_ESM {
            ending_pref = modulespecifiers::IMPORT_MODULE_SPECIFIER_ENDING_PREFERENCE_JS;
        }

        let all_specifiers = modulespecifiers::get_module_specifiers(
            symbol,
            self.ch,
            specifier_compiler_options,
            context_file,
            host,
            modulespecifiers::GetModuleSpecifiersOptions {
                user_preferences: modulespecifiers::UserPreferences {
                    import_module_specifier_preference: specifier_pref,
                    import_module_specifier_ending: ending_pref,
                    auto_import_specifier_exclude_regexes: Vec::new(),
                },
                options: modulespecifiers::ModuleSpecifierOptions {
                    override_import_mode,
                },
                for_auto_imports: false,
            },
        );
        if all_specifiers.is_empty() {
            links.specifier_cache.insert(cache_key, String::new());
            return String::new();
        }
        let specifier = all_specifiers[0].clone();
        links.specifier_cache.insert(cache_key, specifier.clone());
        specifier
    }

    fn module_symbol_data_from_handle(
        &self,
        symbol: ast::SymbolHandle,
    ) -> modulespecifiers::ModuleSymbolData {
        modulespecifiers::ModuleSymbolData::new(
            ast::SymbolIdentity::from_symbol_handle(symbol),
            self.ch.symbol_handle_name(symbol),
            self.ch.collect_symbol_handle_declarations(symbol),
            self.ch.symbol_handle_value_declaration(symbol),
        )
    }

    fn declaration_of_kind_data(
        &self,
        symbol: &modulespecifiers::ModuleSymbolData,
        kind: ast::Kind,
    ) -> Option<ast::Node> {
        symbol
            .declarations
            .iter()
            .copied()
            .find(|declaration| self.store_for_node(*declaration).kind(*declaration) == kind)
    }

    fn get_file_symbol_if_file_symbol_export_equals_container_handle(
        &mut self,
        declaration: ast::Node,
        container: ast::SymbolHandle,
    ) -> Option<ast::SymbolHandle> {
        let file_symbol = self.get_external_module_container_symbol_handle(declaration)?;
        let exported = self
            .ch
            .lookup_symbol_handle_export(file_symbol, ast::INTERNAL_SYMBOL_NAME_EXPORT_EQUALS)?;
        if self.ch.same_symbol_identity(
            SymbolIdentity::from_symbol_handle(exported),
            SymbolIdentity::from_symbol_handle(container),
        ) {
            return Some(file_symbol);
        }
        None
    }

    fn get_external_module_container_symbol_handle(
        &self,
        declaration: ast::Node,
    ) -> Option<ast::SymbolHandle> {
        let store = self.store_for_node(declaration);
        let node = ast::find_ancestor(store, Some(declaration), |store, node| {
            self.has_external_module_symbol_handle_container(store, node)
        })?;
        self.ch.node_symbol(node)
    }

    fn has_external_module_symbol_handle_container(
        &self,
        store: &ast::AstStore,
        declaration: ast::Node,
    ) -> bool {
        ast::is_ambient_module(store, declaration)
            || (store.kind(declaration) == ast::Kind::SourceFile
                && self.ch.source_file_is_external_or_common_js_module(
                    self.ch.source_file_for_node(declaration),
                ))
    }

    fn type_parameter_to_declaration_with_constraint(
        &mut self,
        type_parameter: TypeHandle,
        constraint_node: Option<ast::Node>,
    ) -> ast::Node {
        let old_flags = self.ctx.flags;
        let old_internal_flags = self.ctx.internal_flags;
        let old_depth = self.ctx.depth;
        self.ctx.flags &= !nodebuilder::FLAGS_WRITE_TYPE_PARAMETERS_IN_QUALIFIED_NAME; // Avoids potential infinite loop when building for a claimspace with a generic
        let modifiers = self.create_modifiers_from_modifier_flags(
            self.ch.get_type_parameter_modifiers(type_parameter),
        );
        let modifiers_list = if !modifiers.is_empty() {
            Some(self.e.factory.node_factory.new_modifier_list(
                core::new_text_range(-1, -1),
                core::new_text_range(-1, -1),
                modifiers,
                self.ch.get_type_parameter_modifiers(type_parameter),
            ))
        } else {
            None
        };
        let name = self.type_parameter_to_name(type_parameter);
        let name = self.ensure_factory_node(name);
        let constraint_node = self.ensure_optional_factory_node(constraint_node);
        let default_parameter = self.ch.get_default_from_type_parameter(type_parameter);
        let mut default_parameter_declaration_node = None;
        if let Some(default_parameter) = default_parameter {
            default_parameter_declaration_node = self.type_to_type_node(default_parameter);
        }
        default_parameter_declaration_node =
            self.ensure_optional_factory_node(default_parameter_declaration_node);
        self.ctx.flags = old_flags;
        self.ctx.internal_flags = old_internal_flags;
        self.ctx.depth = old_depth;
        let node = self.e.factory.node_factory.new_type_parameter_declaration(
            modifiers_list,
            name.clone(),
            constraint_node,
            None, // expression
            default_parameter_declaration_node,
        );
        Self::node_value(node)
    }

    fn create_modifiers_from_modifier_flags(
        &mut self,
        flags: ast::ModifierFlags,
    ) -> Vec<ast::Node> {
        let mut result = Vec::new();
        if (flags & ast::ModifierFlags::EXPORT) != ast::ModifierFlags::NONE {
            result.push(
                self.e
                    .factory
                    .node_factory
                    .new_modifier(ast::Kind::ExportKeyword),
            );
        }
        if (flags & ast::ModifierFlags::AMBIENT) != ast::ModifierFlags::NONE {
            result.push(
                self.e
                    .factory
                    .node_factory
                    .new_modifier(ast::Kind::DeclareKeyword),
            );
        }
        if (flags & ast::ModifierFlags::DEFAULT) != ast::ModifierFlags::NONE {
            result.push(
                self.e
                    .factory
                    .node_factory
                    .new_modifier(ast::Kind::DefaultKeyword),
            );
        }
        if (flags & ast::ModifierFlags::CONST) != ast::ModifierFlags::NONE {
            result.push(
                self.e
                    .factory
                    .node_factory
                    .new_modifier(ast::Kind::ConstKeyword),
            );
        }
        if (flags & ast::ModifierFlags::PUBLIC) != ast::ModifierFlags::NONE {
            result.push(
                self.e
                    .factory
                    .node_factory
                    .new_modifier(ast::Kind::PublicKeyword),
            );
        }
        if (flags & ast::ModifierFlags::PRIVATE) != ast::ModifierFlags::NONE {
            result.push(
                self.e
                    .factory
                    .node_factory
                    .new_modifier(ast::Kind::PrivateKeyword),
            );
        }
        if (flags & ast::ModifierFlags::PROTECTED) != ast::ModifierFlags::NONE {
            result.push(
                self.e
                    .factory
                    .node_factory
                    .new_modifier(ast::Kind::ProtectedKeyword),
            );
        }
        if (flags & ast::ModifierFlags::STATIC) != ast::ModifierFlags::NONE {
            result.push(
                self.e
                    .factory
                    .node_factory
                    .new_modifier(ast::Kind::StaticKeyword),
            );
        }
        if (flags & ast::ModifierFlags::ABSTRACT) != ast::ModifierFlags::NONE {
            result.push(
                self.e
                    .factory
                    .node_factory
                    .new_modifier(ast::Kind::AbstractKeyword),
            );
        }
        if (flags & ast::ModifierFlags::ASYNC) != ast::ModifierFlags::NONE {
            result.push(
                self.e
                    .factory
                    .node_factory
                    .new_modifier(ast::Kind::AsyncKeyword),
            );
        }
        if (flags & ast::ModifierFlags::READONLY) != ast::ModifierFlags::NONE {
            result.push(
                self.e
                    .factory
                    .node_factory
                    .new_modifier(ast::Kind::ReadonlyKeyword),
            );
        }
        if (flags & ast::ModifierFlags::OVERRIDE) != ast::ModifierFlags::NONE {
            result.push(
                self.e
                    .factory
                    .node_factory
                    .new_modifier(ast::Kind::OverrideKeyword),
            );
        }
        if (flags & ast::ModifierFlags::IN) != ast::ModifierFlags::NONE {
            result.push(
                self.e
                    .factory
                    .node_factory
                    .new_modifier(ast::Kind::InKeyword),
            );
        }
        if (flags & ast::ModifierFlags::OUT) != ast::ModifierFlags::NONE {
            result.push(
                self.e
                    .factory
                    .node_factory
                    .new_modifier(ast::Kind::OutKeyword),
            );
        }
        result
    }

    /**
     * Unlike the utilities `setTextRange`, this checks if the `location` we're trying to set on `range` is within the
     * same file as the active context. If not, the range is not applied. This prevents us from copying ranges across files,
     * which will confuse the node printer (as it assumes all node ranges are within the current file).
     * Additionally, if `range` _isn't synthetic_, or isn't in the current file, it will _copy_ it to _remove_ its' position
     * information.
     *
     * It also calls `setOriginalNode` to setup a `.original` pointer, since you basically *always* want these in the node builder.
     */
    pub(crate) fn set_text_range(
        &mut self,
        mut range: Option<ast::Node>,
        location: Option<ast::Node>,
    ) -> Option<ast::Node> {
        range?;
        let original_range = range.unwrap();
        let original_store = self.store_for_node(original_range);
        if !ast::node_is_synthesized(original_store, original_range)
            || original_store.flags(original_range) & ast::NODE_FLAGS_SYNTHESIZED == 0
            || self.ctx.enclosing_file.is_none()
            || {
                let most_original = self.e.most_original(&original_range);
                let store = self.store_for_node(most_original);
                let source_file = ast::get_source_file_of_node(store, Some(most_original))
                    .map(|source_file| store.source_file_view(source_file).file_name());
                self.ctx.enclosing_file.is_some_and(|enclosing_file| {
                    let enclosing_file = self.ch.source_file_for_identity(enclosing_file);
                    Some(enclosing_file.file_name().to_string()) != source_file
                })
            }
        {
            let original = original_range;
            let cloned = self
                .clone_node_with_loc_preserving_metadata(original, core::new_text_range(-1, -1)); // if `range` is synthesized or originates in another file, copy it so it definitely has synthetic positions
            range = Some(cloned);
        }
        let range = range.unwrap();
        if Some(range) == location || location.is_none() {
            return Some(range);
        }
        // Don't overwrite the original node if `range` has an `original` node that points either directly or indirectly to `location`
        let mut original = self.e.original(&range);
        while original.as_ref() != location.as_ref() {
            let Some(current) = original.as_ref() else {
                break;
            };
            original = self.e.original(current);
        }
        if original.is_none() {
            self.set_original_ex(&range, &location.unwrap(), true);
        }

        // only set positions if range comes from the same file since copying text across files isn't supported by the emitter
        if self.ctx.enclosing_file.is_some() && {
            let most_original = self.e.most_original(&location.unwrap());
            let store = self.store_for_node(most_original);
            ast::get_source_file_of_node(store, Some(most_original))
                .is_some_and(|source_file| self.is_enclosing_source_file_node(source_file))
        } {
            let location = location.unwrap();
            let loc = self.store_for_node(location).loc(location);
            self.e
                .factory
                .node_factory
                .place_checker_synthetic_node(range, loc);
            return Some(range);
        } else {
            self.e
                .factory
                .node_factory
                .place_checker_synthetic_node(range, core::new_text_range(-1, -1));
            return Some(range);
        }
    }

    fn type_parameter_shadows_other_type_parameter_in_scope(
        &mut self,
        name: &str,
        type_parameter: TypeHandle,
    ) -> bool {
        let result = self.resolve_name_in_builder_scope(
            self.ctx.enclosing_declaration,
            name,
            ast::SYMBOL_FLAGS_TYPE,
            None,
            false,
            false,
        );
        if let Some(result) = result {
            if self
                .ch
                .missing_name_symbol_identity_flags(result)
                .intersects(ast::SYMBOL_FLAGS_TYPE_PARAMETER)
            {
                return !self.ch.same_optional_symbol_identity(
                    Some(result),
                    self.ch.type_symbol_identity(type_parameter),
                );
            }
        }
        false
    }

    pub(crate) fn type_parameter_to_name(&mut self, type_parameter: TypeHandle) -> ast::Node {
        if self.ctx.flags & nodebuilder::FLAGS_GENERATE_NAMES_FOR_SHADOWED_TYPE_PARAMS != 0 {
            let type_id = self.ch.type_id(type_parameter);
            if let (Some(cached), true) = self.ctx.type_parameter_names.get(&type_id) {
                return *cached;
            }
        }
        let symbol = self
            .ch
            .type_symbol_identity(type_parameter)
            .expect("type parameter should have a symbol for serialization");
        let old_flags = self.ctx.flags;
        self.ctx.flags |= nodebuilder::FLAGS_IN_INITIAL_ENTITY_NAME;
        let symbol_name = self.get_name_of_symbol_as_written_identity(symbol);
        self.ctx.flags = old_flags;
        let mut result = self
            .new_identifier_with_symbol_identity(&symbol_name, Some(symbol))
            .clone();
        self.e
            .mark_emit_node(&result, printer::EF_NO_ASCII_ESCAPING);
        if !ast::is_identifier(self.store_for_node(result), result) {
            let missing = self
                .e
                .factory
                .node_factory
                .new_identifier("(Missing type parameter)");
            return Self::node_value(missing);
        }
        if let Some(symbol) = self.ch.type_symbol_identity(type_parameter) {
            if let Some(decl) = self.ch.first_symbol_identity_declaration(symbol) {
                if ast::is_type_parameter_declaration(self.store_for_node(decl), decl) {
                    let name = self.store_for_node(decl).name(decl);
                    result = self.set_text_range(Some(result), name).unwrap();
                }
            }
        }
        if self.ctx.flags & nodebuilder::FLAGS_GENERATE_NAMES_FOR_SHADOWED_TYPE_PARAMS != 0 {
            let raw_text = self.store_for_node(result).text(result).to_string();
            let mut i = self
                .ctx
                .type_parameter_names_by_text_next_name_count
                .get(&raw_text)
                .0
                .copied()
                .unwrap_or(0);
            let mut text = raw_text.clone();

            loop {
                if !self.ctx.type_parameter_names_by_text.has(&text)
                    && !self.type_parameter_shadows_other_type_parameter_in_scope(
                        text.as_str(),
                        type_parameter,
                    )
                {
                    break;
                }
                i += 1;
                text = format!("{}_{}", raw_text, i);
            }
            if text != raw_text {
                let type_arguments = self.e.get_identifier_type_arguments(&result);
                result = self.new_identifier_with_symbol_identity(
                    text.as_str(),
                    self.ch.type_symbol_identity(type_parameter),
                );
                self.e
                    .set_identifier_type_arguments(&result, type_arguments);
            }

            // avoiding iterations of the above loop turns out to be worth it when `i` starts to get large, so we cache the max
            // `i` we've used thus far, to save work later
            self.ctx
                .type_parameter_names_by_text_next_name_count
                .set(raw_text, i);
            self.ctx
                .type_parameter_names
                .set(self.ch.type_id(type_parameter), result);
            self.ctx.type_parameter_names_by_text.add(text);
        }

        result
    }

    fn is_mapped_type_homomorphic(&mut self, mapped: TypeHandle) -> bool {
        self.ch.get_homomorphic_type_variable(mapped).is_some()
    }

    fn is_homomorphic_mapped_type_with_non_homomorphic_instantiation(
        &mut self,
        mapped: TypeHandle,
    ) -> bool {
        let target = self.ch.type_record(mapped).as_mapped_type().object.target;
        target.is_some()
            && !self.is_mapped_type_homomorphic(mapped)
            && self.is_mapped_type_homomorphic(target.unwrap())
    }

    pub(crate) fn type_predicate_to_type_predicate_node(
        &mut self,
        predicate: TypePredicateHandle,
    ) -> ast::Node {
        let predicate = self.ch.type_predicate_record(predicate).clone();
        let mut asserts_modifier = None;
        if predicate.kind == TYPE_PREDICATE_KIND_ASSERTS_IDENTIFIER
            || predicate.kind == TYPE_PREDICATE_KIND_ASSERTS_THIS
        {
            asserts_modifier = Some(
                self.e
                    .factory
                    .node_factory
                    .new_token(ast::KIND_ASSERTS_KEYWORD),
            );
        }
        let parameter_name = if predicate.kind == TYPE_PREDICATE_KIND_IDENTIFIER
            || predicate.kind == TYPE_PREDICATE_KIND_ASSERTS_IDENTIFIER
        {
            let parameter_name = self
                .e
                .factory
                .node_factory
                .new_identifier(predicate.parameter_name.as_str());
            self.e
                .mark_emit_node(&parameter_name, printer::EF_NO_ASCII_ESCAPING);
            parameter_name
        } else {
            self.e.factory.node_factory.new_this_type_node()
        };
        let mut type_node = None;
        if let Some(t) = predicate.t {
            type_node = self.type_to_type_node(t);
        }
        let node = self.e.factory.node_factory.new_type_predicate_node(
            asserts_modifier,
            parameter_name,
            type_node,
        );
        Self::node_value(node)
    }

    fn type_to_type_node_helper_with_possible_reusable_type_node(
        &mut self,
        t: Option<TypeHandle>,
        type_node: Option<ast::Node>,
    ) -> ast::Node {
        let Some(t) = t else {
            return Self::node_value(
                self.e
                    .factory
                    .node_factory
                    .new_keyword_type_node(ast::KIND_ANY_KEYWORD),
            );
        };
        if !self.is_actively_expanding()
            && type_node.is_some()
            && self.get_type_from_type_node(type_node.unwrap(), false) == Some(t)
        {
            let reused = self.try_reuse_existing_node_helper(type_node.unwrap());
            if let Some(reused) = reused {
                self.check_type_expandability(Some(t));
                return reused;
            }
        }
        self.type_to_type_node(t).unwrap()
    }

    pub(crate) fn type_parameter_to_declaration(&mut self, parameter: TypeHandle) -> ast::Node {
        let constraint = self.ch.get_constraint_of_type_parameter(parameter);
        let mut constraint_node = None;
        if let Some(constraint) = constraint {
            constraint_node = Some(
                self.type_to_type_node_helper_with_possible_reusable_type_node(
                    Some(constraint),
                    self.ch.get_constraint_declaration(parameter),
                ),
            );
        }
        self.type_parameter_to_declaration_with_constraint(parameter, constraint_node)
    }

    pub(crate) fn symbol_to_type_parameter_declarations_handle(
        &mut self,
        symbol: ast::SymbolHandle,
    ) -> Vec<ast::Node> {
        self.type_parameters_to_type_parameter_declarations_handle(symbol)
    }

    pub(crate) fn symbol_to_type_parameter_declarations_identity(
        &mut self,
        symbol: SymbolIdentity,
    ) -> Vec<ast::Node> {
        let target_symbol = self.ch.get_target_symbol_identity(symbol);
        let target_flags = self.ch.missing_name_symbol_identity_flags(target_symbol);
        if target_flags
            & (ast::SYMBOL_FLAGS_CLASS | ast::SYMBOL_FLAGS_INTERFACE | ast::SYMBOL_FLAGS_ALIAS)
            != 0
        {
            let mut results = Vec::new();
            let declarations = self.ch.collect_symbol_identity_declarations(symbol);
            let mut params = Vec::new();
            for node in declarations {
                let store = self.store_for_node(node);
                if ast::node_kind_is(
                    store,
                    &node,
                    &[
                        ast::Kind::InterfaceDeclaration,
                        ast::Kind::ClassDeclaration,
                        ast::Kind::ClassExpression,
                    ],
                ) || is_type_alias(store, node)
                {
                    params = self.ch.append_type_parameters(
                        params,
                        store
                            .type_parameters(node)
                            .map(|type_parameters| type_parameters.iter().collect())
                            .unwrap_or_default(),
                    );
                }
            }
            for param in params {
                results.push(self.type_parameter_to_declaration(param));
            }
            return results;
        } else if target_flags & ast::SYMBOL_FLAGS_FUNCTION != 0 {
            let mut results = Vec::new();
            let value_declaration = self
                .ch
                .missing_name_symbol_identity_value_declaration(symbol)
                .expect("function symbol should have a value declaration");
            for param in self
                .ch
                .get_type_parameters_from_declaration(value_declaration)
            {
                results.push(self.type_parameter_to_declaration(param));
            }
            return results;
        }
        Vec::new()
    }

    fn type_parameters_to_type_parameter_declarations_handle(
        &mut self,
        symbol: ast::SymbolHandle,
    ) -> Vec<ast::Node> {
        let target_flags = self.ch.symbol_handle_flags(symbol);
        if target_flags
            & (ast::SYMBOL_FLAGS_CLASS | ast::SYMBOL_FLAGS_INTERFACE | ast::SYMBOL_FLAGS_ALIAS)
            != 0
        {
            let mut results = Vec::new();
            let declarations = self.ch.collect_symbol_handle_declarations(symbol);
            let mut params = Vec::new();
            for node in declarations {
                let store = self.store_for_node(node);
                if ast::node_kind_is(
                    store,
                    &node,
                    &[
                        ast::Kind::InterfaceDeclaration,
                        ast::Kind::ClassDeclaration,
                        ast::Kind::ClassExpression,
                    ],
                ) || is_type_alias(store, node)
                {
                    params = self.ch.append_type_parameters(
                        params,
                        store
                            .type_parameters(node)
                            .map(|type_parameters| type_parameters.iter().collect())
                            .unwrap_or_default(),
                    );
                }
            }
            for param in params {
                results.push(self.type_parameter_to_declaration(param));
            }
            return results;
        } else if target_flags & ast::SYMBOL_FLAGS_FUNCTION != 0 {
            let mut results = Vec::new();
            let value_declaration = self
                .ch
                .symbol_handle_value_declaration(symbol)
                .expect("function symbol should have a value declaration");
            for param in self
                .ch
                .get_type_parameters_from_declaration(value_declaration)
            {
                results.push(self.type_parameter_to_declaration(param));
            }
            return results;
        }
        Vec::new()
    }

    pub(crate) fn symbol_to_parameter_declaration(
        &mut self,
        parameter_symbol: SymbolIdentity,
        preserve_modifier_flags: bool,
    ) -> ast::Node {
        let parameter_declaration = get_effective_parameter_declaration(self.ch, parameter_symbol);

        let parameter_type = self.ch.get_type_of_symbol_identity(parameter_symbol);
        let parameter_type_node = self.serialize_type_for_declaration_for_symbol_identity(
            parameter_declaration,
            Some(parameter_type),
            Some(parameter_symbol),
            true,
        );
        let mut modifiers = None;
        if self.ctx.flags & nodebuilder::FLAGS_OMIT_PARAMETER_MODIFIERS == 0
            && preserve_modifier_flags
            && parameter_declaration.is_some()
            && ast::can_have_modifiers(
                self.store_for_node(parameter_declaration.unwrap()),
                parameter_declaration.unwrap(),
            )
        {
            let parameter_declaration = parameter_declaration.unwrap();
            let parameter_store = self.store_for_node(parameter_declaration);
            let flags = ast::get_combined_modifier_flags(parameter_store, parameter_declaration);
            let originals = parameter_store
                .modifier_nodes(parameter_declaration)
                .into_iter()
                .filter(|node| ast::is_modifier(parameter_store, *node))
                .map(|node| (node, parameter_store.loc(node)))
                .collect::<Vec<_>>();
            let clones = originals
                .into_iter()
                .map(|(node, loc)| self.clone_node_with_loc(node, loc))
                .collect::<Vec<_>>();
            if !clones.is_empty() {
                modifiers = Some(self.e.factory.node_factory.new_modifier_list(
                    core::new_text_range(-1, -1),
                    core::new_text_range(-1, -1),
                    clones,
                    flags,
                ));
            }
        }
        let is_rest = parameter_declaration.is_some()
            && is_rest_parameter(
                self.store_for_node(parameter_declaration.unwrap()),
                parameter_declaration.unwrap(),
            )
            || self
                .ch
                .missing_name_symbol_identity_check_flags(parameter_symbol)
                & ast::CHECK_FLAGS_REST_PARAMETER
                != 0;
        let mut dot_dot_dot_token = None;
        if is_rest {
            dot_dot_dot_token = Some(
                self.e
                    .factory
                    .node_factory
                    .new_token(ast::KIND_DOT_DOT_DOT_TOKEN),
            );
        }
        let name =
            self.parameter_to_parameter_declaration_name(parameter_symbol, parameter_declaration);
        let is_optional = parameter_declaration.is_some()
            && self
                .ch
                .is_optional_parameter(parameter_declaration.unwrap())
            || self
                .ch
                .missing_name_symbol_identity_check_flags(parameter_symbol)
                & ast::CHECK_FLAGS_OPTIONAL_PARAMETER
                != 0;
        let mut question_token = None;
        if is_optional {
            question_token = Some(
                self.e
                    .factory
                    .node_factory
                    .new_token(ast::KIND_QUESTION_TOKEN),
            );
        }

        let parameter_node = self.e.factory.node_factory.new_parameter_declaration(
            modifiers,
            dot_dot_dot_token,
            name.clone(),
            question_token,
            Some(parameter_type_node.clone()),
            None, /*initializer*/
        );
        self.ctx.approximate_length += self
            .ch
            .missing_name_symbol_identity_name(parameter_symbol)
            .len()
            + 3;
        Self::node_value(parameter_node)
    }

    pub(crate) fn parameter_to_parameter_declaration_name(
        &mut self,
        parameter_symbol: SymbolIdentity,
        parameter_declaration: Option<ast::Node>,
    ) -> ast::Node {
        let parameter_name = parameter_declaration
            .and_then(|declaration| self.store_for_node(declaration).name(declaration));
        if parameter_name.is_none() {
            let name = self.ch.missing_name_symbol_identity_name(parameter_symbol);
            return self.new_identifier_with_symbol_identity(&name, Some(parameter_symbol));
        }

        let name = parameter_name.unwrap();
        match self.store_for_node(name).kind(name) {
            ast::KIND_IDENTIFIER => {
                let cloned = Self::node_value(self.deep_clone_node(name));
                self.e
                    .set_emit_flags(&cloned, printer::EF_NO_ASCII_ESCAPING);
                self.id_to_symbol.insert(cloned, parameter_symbol);
                cloned
            }
            ast::KIND_QUALIFIED_NAME => {
                let right = {
                    let name_store = self.store_for_node(name);
                    name_store.right(name).unwrap()
                };
                let cloned = Self::node_value(self.deep_clone_node(right));
                self.e
                    .set_emit_flags(&cloned, printer::EF_NO_ASCII_ESCAPING);
                self.id_to_symbol.insert(cloned, parameter_symbol);
                cloned
            }
            _ => self.clone_binding_name(name),
        }
    }

    pub(crate) fn clone_binding_name(&mut self, node: ast::Node) -> ast::Node {
        Self::node_value(self.clone_binding_name_node(node))
    }

    fn clone_binding_name_node(&mut self, node: ast::Node) -> ast::Node {
        self.track_late_bindable_computed_name(node);

        let mut visited = match self.store_for_node(node).kind(node) {
            ast::Kind::ComputedPropertyName => self.clone_computed_property_name_for_binding(node),
            ast::Kind::ObjectBindingPattern | ast::Kind::ArrayBindingPattern => {
                self.clone_binding_pattern_for_binding(node)
            }
            ast::Kind::BindingElement => self.clone_binding_element_for_binding(node),
            _ => {
                self.track_late_bindable_computed_names_in_children(node);
                node
            }
        };

        if !ast::node_is_synthesized(self.store_for_node(visited), visited) {
            visited = self.deep_clone_node(visited);
        }

        self.e.set_emit_flags(
            &visited,
            printer::EF_SINGLE_LINE | printer::EF_NO_ASCII_ESCAPING,
        );
        visited
    }

    fn clone_computed_property_name_for_binding(&mut self, node: ast::Node) -> ast::Node {
        let expression = {
            let store = self.store_for_node(node);
            store.expression(node)
        };
        let expression = expression.map(|expression| self.clone_binding_name_node(expression));

        if node.store_id() == self.e.factory.node_factory.store().store_id() {
            self.e
                .factory
                .node_factory
                .update_computed_property_name(node, expression)
        } else {
            let source = self.ch.store_for_node(node);
            self.e
                .factory
                .node_factory
                .update_computed_property_name_from_store(source, node, expression)
        }
    }

    fn clone_binding_pattern_for_binding(&mut self, node: ast::Node) -> ast::Node {
        let (elements_loc, elements_range, elements_has_trailing_comma, elements) = {
            let store = self.store_for_node(node);
            let elements = store.elements(node).unwrap();
            (
                elements.loc(),
                elements.range(),
                elements.has_trailing_comma(),
                elements.into_iter().collect::<Vec<_>>(),
            )
        };
        let elements = elements
            .into_iter()
            .map(|element| self.clone_binding_name_node(element))
            .collect::<Vec<_>>();
        let elements = self
            .e
            .factory
            .node_factory
            .new_node_list_with_trailing_comma(
                elements_loc,
                elements_range,
                elements,
                elements_has_trailing_comma,
            );

        if node.store_id() == self.e.factory.node_factory.store().store_id() {
            self.e
                .factory
                .node_factory
                .update_binding_pattern(node, elements)
        } else {
            let source = self.ch.store_for_node(node);
            self.e
                .factory
                .node_factory
                .update_binding_pattern_from_store(source, node, elements)
        }
    }

    fn clone_binding_element_for_binding(&mut self, node: ast::Node) -> ast::Node {
        let (dot_dot_dot_token, property_name, name, initializer) = {
            let store = self.store_for_node(node);
            (
                store.dot_dot_dot_token(node),
                store.property_name(node),
                store.name(node),
                store.initializer(node),
            )
        };
        let dot_dot_dot_token = dot_dot_dot_token.map(|token| self.clone_binding_name_node(token));
        let property_name = property_name.map(|name| self.clone_binding_name_node(name));
        let name = name.map(|name| self.clone_binding_name_node(name));
        if let Some(initializer) = initializer {
            let _ = self.clone_binding_name_node(initializer);
        }

        if node.store_id() == self.e.factory.node_factory.store().store_id() {
            self.e.factory.node_factory.update_binding_element(
                node,
                dot_dot_dot_token,
                property_name,
                name,
                None::<ast::Node>,
            )
        } else {
            let source = self.ch.store_for_node(node);
            self.e
                .factory
                .node_factory
                .update_binding_element_from_store(
                    source,
                    node,
                    dot_dot_dot_token,
                    property_name,
                    name,
                    None::<ast::Node>,
                )
        }
    }

    fn track_late_bindable_computed_name(&mut self, node: ast::Node) {
        if !ast::is_computed_property_name(self.store_for_node(node), node) {
            return;
        }

        let node = Self::node_value(node);
        if self.ch.is_late_bindable_name(node) {
            let expression = self.store_for_node(node).expression(node).unwrap();
            self.track_computed_name(Self::node_value(expression), self.ctx.enclosing_declaration);
        }
    }

    fn track_late_bindable_computed_names_in_children(&mut self, node: ast::Node) {
        let children = {
            let store = self.store_for_node(node);
            let mut children = Vec::new();
            let _ = store.for_each_child(node, |child| {
                if let Some(child) = child {
                    children.push(child);
                }
                std::ops::ControlFlow::Continue(())
            });
            children
        };

        for child in children {
            self.track_late_bindable_computed_name(child);
            self.track_late_bindable_computed_names_in_children(child);
        }
    }

    pub(crate) fn serialize_type_for_expression(&mut self, expr: ast::Node) -> ast::Node {
        // !!! TODO: shim, add node reuse
        let regular_type = self.ch.get_regular_type_of_expression(expr);
        let widened_type = self.ch.get_widened_type(regular_type);
        let t = self
            .ch
            .instantiate_type_with_mapper_handle(Some(widened_type), self.ctx.mapper);
        self.type_to_type_node(t.unwrap()).unwrap()
    }

    fn serialize_inferred_return_type_for_signature(
        &mut self,
        signature: SignatureHandle,
        return_type: TypeHandle,
    ) -> ast::Node {
        let old_suppress_report_inference_fallback = self.ctx.suppress_report_inference_fallback;
        self.ctx.suppress_report_inference_fallback = true;
        let type_predicate = self.ch.get_type_predicate_of_signature(signature);
        let return_type_node = if let Some(type_predicate) = type_predicate {
            let mapper = self.ctx.mapper.clone();
            let predicate = if mapper.is_some() {
                self.ch
                    .instantiate_type_predicate_with_mapper_handle(type_predicate, mapper)
            } else {
                type_predicate
            };
            self.type_predicate_to_type_predicate_node_helper(predicate)
        } else {
            self.type_to_type_node(return_type).unwrap()
        };
        self.ctx.suppress_report_inference_fallback = old_suppress_report_inference_fallback;
        return_type_node
    }

    fn type_predicate_to_type_predicate_node_helper(
        &mut self,
        type_predicate: TypePredicateHandle,
    ) -> ast::Node {
        let type_predicate = self.ch.type_predicate_record(type_predicate).clone();
        let asserts_modifier = if type_predicate.kind == TYPE_PREDICATE_KIND_ASSERTS_THIS
            || type_predicate.kind == TYPE_PREDICATE_KIND_ASSERTS_IDENTIFIER
        {
            Some(
                self.e
                    .factory
                    .node_factory
                    .new_token(ast::KIND_ASSERTS_KEYWORD),
            )
        } else {
            None
        };
        let parameter_name = if type_predicate.kind == TYPE_PREDICATE_KIND_IDENTIFIER
            || type_predicate.kind == TYPE_PREDICATE_KIND_ASSERTS_IDENTIFIER
        {
            let parameter_name =
                self.new_identifier(&type_predicate.parameter_name, None /*symbol*/);
            self.e
                .set_emit_flags(&parameter_name, printer::EF_NO_ASCII_ESCAPING);
            parameter_name
        } else {
            Self::node_value(self.e.factory.node_factory.new_this_type_node())
        };
        let mut type_node = None;
        if let Some(t) = type_predicate.t {
            type_node = self.type_to_type_node(t);
        }
        let node = self.e.factory.node_factory.new_type_predicate_node(
            asserts_modifier,
            parameter_name.clone(),
            type_node,
        );
        Self::node_value(node)
    }

    fn try_get_this_parameter_declaration(
        &mut self,
        signature: SignatureHandle,
    ) -> Option<ast::Node> {
        if let Some(this_parameter) = self.ch.signature_this_parameter(signature) {
            return Some(self.symbol_to_parameter_declaration(this_parameter, false));
        }
        None
    }

    pub(crate) fn index_info_to_index_signature_declaration_helper(
        &mut self,
        index_info: IndexInfoHandle,
        mut type_node: Option<ast::Node>,
    ) -> ast::Node {
        let index_info_record = self.ch.index_info_record(index_info).clone();
        let name = if let Some(declaration) = index_info_record.declaration {
            let parameter = self
                .store_for_node(declaration)
                .parameters(declaration)
                .unwrap()
                .iter()
                .next()
                .unwrap();
            scanner::declaration_name_to_string(
                self.ch.source_file_for_node(parameter),
                self.store_for_node(parameter).name(parameter).as_ref(),
            )
        } else {
            "x".to_string()
        };
        let indexer_type_node = self.type_to_type_node(index_info_record.key_type.unwrap());

        let indexer_name = self.new_identifier(&name, None /*symbol*/).clone();
        let indexing_parameter = self.e.factory.node_factory.new_parameter_declaration(
            None,
            None,
            indexer_name,
            None,
            indexer_type_node,
            None,
        );
        if type_node.is_none() {
            if index_info_record.value_type.is_none() {
                type_node = Some(Self::node_value(
                    self.e
                        .factory
                        .node_factory
                        .new_keyword_type_node(ast::KIND_ANY_KEYWORD),
                ));
            } else {
                type_node = self.type_to_type_node(index_info_record.value_type.unwrap());
            }
        }
        if index_info_record.value_type.is_none()
            && self.ctx.flags & nodebuilder::FLAGS_ALLOW_EMPTY_INDEX_INFO_TYPE == 0
        {
            self.ctx.encountered_error = true;
        }
        self.ctx.approximate_length += name.len() + 4;
        let mut modifiers = None;
        if index_info_record.is_readonly {
            self.ctx.approximate_length += 9;
            let readonly_modifier = self
                .e
                .factory
                .node_factory
                .new_modifier(ast::KIND_READONLY_KEYWORD);
            modifiers = Some(self.e.factory.node_factory.new_modifier_list(
                core::new_text_range(-1, -1),
                core::new_text_range(-1, -1),
                vec![readonly_modifier],
                ast::MODIFIER_FLAGS_READONLY,
            ));
        }
        let parameters = self.new_factory_node_list(vec![indexing_parameter]);
        let node = self
            .e
            .factory
            .node_factory
            .new_index_signature_declaration(modifiers, parameters, type_node);
        Self::node_value(node)
    }

    fn is_entity_name_visible_in_builder_scope(
        &mut self,
        entity_name: ast::Node,
        enclosing_declaration: ast::Node,
        should_compute_alias_to_make_visible: bool,
    ) -> printer::SymbolAccessibilityResult {
        let (meaning, first_identifier, first_identifier_text, is_this_identifier) = {
            let entity_store = self.store_for_node(entity_name);
            if !ast::is_parse_tree_node(entity_store, entity_name) {
                return printer::SymbolAccessibilityResult {
                    accessibility: printer::SymbolAccessibility::NotAccessible,
                    ..Default::default()
                };
            }

            let meaning = get_meaning_of_entity_name_reference(entity_store, entity_name);
            let first_identifier = ast::get_first_identifier(entity_store, entity_name).unwrap();
            let first_identifier_text = entity_store.text(first_identifier).to_string();
            let is_this_identifier = ast::is_this_identifier(entity_store, first_identifier);
            (
                meaning,
                first_identifier,
                first_identifier_text,
                is_this_identifier,
            )
        };
        let symbol = self.resolve_name_in_builder_scope(
            Some(enclosing_declaration),
            &first_identifier_text,
            meaning,
            None,
            false,
            false,
        );

        if let Some(symbol) = symbol.as_ref() {
            if self.symbol_identity_flags(*symbol) & ast::SYMBOL_FLAGS_TYPE_PARAMETER != 0
                && meaning & ast::SYMBOL_FLAGS_TYPE != 0
            {
                return printer::SymbolAccessibilityResult {
                    accessibility: printer::SymbolAccessibility::Accessible,
                    ..Default::default()
                };
            }
        }

        if symbol.is_none() && is_this_identifier {
            let container = self.ch.get_this_container(first_identifier, false, false);
            let sym = self.ch.get_symbol_of_declaration(container);
            if let Some(sym) = sym {
                let accessibility = self.ch.is_symbol_accessible_by_identity(
                    Some(SymbolIdentity::from_symbol_handle(sym)),
                    Some(enclosing_declaration),
                    meaning,
                    false,
                );
                if accessibility.accessibility == printer::SymbolAccessibility::Accessible {
                    return printer::SymbolAccessibilityResult {
                        accessibility: printer::SymbolAccessibility::Accessible,
                        ..Default::default()
                    };
                }
            }
        }

        let Some(symbol) = symbol else {
            return printer::SymbolAccessibilityResult {
                accessibility: printer::SymbolAccessibility::NotResolved,
                error_symbol_name: first_identifier_text,
                error_node: Some(first_identifier),
                ..Default::default()
            };
        };

        let visible = self
            .ch
            .has_visible_declarations_by_identity(symbol, should_compute_alias_to_make_visible);
        if let Some(visible) = visible {
            return visible;
        }

        printer::SymbolAccessibilityResult {
            accessibility: printer::SymbolAccessibility::NotAccessible,
            error_symbol_name: first_identifier_text,
            error_node: Some(first_identifier),
            ..Default::default()
        }
    }

    fn should_use_placeholder_for_property_identity(
        &mut self,
        property_identity: SymbolIdentity,
    ) -> bool {
        // Use placeholders for reverse mapped types we've either
        // (1) already descended into, or
        // (2) are nested reverse mappings within a mapping over a non-anonymous type, or
        // (3) are deeply nested properties that originate from the same mapped type.
        // Condition (2) is a restriction mostly just to
        // reduce the blowup in printback size from doing, eg, a deep reverse mapping over `Window`.
        // Since anonymous types usually come from expressions, this allows us to preserve the output
        // for deep mappings which likely come from expressions, while truncating those parts which
        // come from mappings over library functions.
        // Condition (3) limits printing of possibly infinitely deep reverse mapped types.
        if self.ch.symbol_identity_check_flags(property_identity) & ast::CHECK_FLAGS_REVERSE_MAPPED
            == 0
        {
            return false;
        }
        // (1)
        if self
            .ctx
            .reverse_mapped_stack
            .iter()
            .any(|&symbol| symbol == property_identity)
        {
            return true;
        }
        // (2)
        if !self.ctx.reverse_mapped_stack.is_empty() {
            let last = self.ctx.reverse_mapped_stack[self.ctx.reverse_mapped_stack.len() - 1];
            if let Some((property_type, _, _)) = self
                .ch
                .semantic_state
                .try_reverse_mapped_symbol_link_types(last)
            {
                if property_type.is_some()
                    && self.ch.object_flags(property_type.unwrap()) & OBJECT_FLAGS_ANONYMOUS == 0
                {
                    return true;
                }
            }
        }
        // (3) - we only inspect the last MAX_REVERSE_MAPPED_NESTING_INSPECTION_DEPTH elements of the
        // stack for approximate matches to catch tight infinite loops
        // TODO: Why? Reasoning lost to time. this could probably stand to be improved?
        if self.ctx.reverse_mapped_stack.len() < MAX_REVERSE_MAPPED_NESTING_INSPECTION_DEPTH {
            return false;
        }
        if !self
            .ch
            .semantic_state
            .has_reverse_mapped_symbol_links(property_identity)
        {
            return false;
        }
        let prop_mapped_type = self
            .ch
            .semantic_state
            .reverse_mapped_mapped_type(property_identity);
        if prop_mapped_type.is_none()
            || self
                .ch
                .type_symbol_identity(prop_mapped_type.unwrap())
                .is_none()
        {
            return false;
        }
        let prop_mapped_symbol = self.ch.type_symbol_identity(prop_mapped_type.unwrap());
        for i in 0..self.ctx.reverse_mapped_stack.len() {
            if i > MAX_REVERSE_MAPPED_NESTING_INSPECTION_DEPTH {
                break;
            }
            let prop = self.ctx.reverse_mapped_stack[self.ctx.reverse_mapped_stack.len() - 1 - i];
            let mapped_type = self
                .ch
                .semantic_state
                .try_reverse_mapped_symbol_link_types(prop)
                .and_then(|(_, mapped_type, _)| mapped_type);
            if mapped_type.is_some()
                && self.ch.same_optional_symbol_identity(
                    self.ch.type_symbol_identity(mapped_type.unwrap()),
                    prop_mapped_symbol,
                )
            {
                return true;
            }
        }
        false
    }

    pub(crate) fn track_computed_name(
        &mut self,
        access_expression: ast::Node,
        enclosing_declaration: Option<ast::Node>,
    ) {
        // get symbol of the first identifier of the entityName
        let access_store = self.store_for_node(access_expression);
        let first_identifier =
            Self::node_value(ast::get_first_identifier(access_store, access_expression).unwrap());
        let first_identifier_text = access_store.text(first_identifier).to_string();
        let name = self.resolve_name_in_builder_scope(
            enclosing_declaration,
            &first_identifier_text,
            ast::SYMBOL_FLAGS_VALUE | ast::SYMBOL_FLAGS_EXPORT_VALUE,
            None, /*nameNotFoundMessage*/
            true, /*isUse*/
            false,
        );
        if let Some(name) = name {
            self.track_symbol_identity(name, enclosing_declaration, ast::SYMBOL_FLAGS_VALUE);
        } else {
            // Name does not resolve at target location, track symbol at dest location (should be inaccessible)
            let fallback = self.ch.resolve_name(
                Some(first_identifier),
                &first_identifier_text,
                ast::SYMBOL_FLAGS_VALUE | ast::SYMBOL_FLAGS_EXPORT_VALUE,
                None, /*nameNotFoundMessage*/
                true, /*isUse*/
                false,
            );
            if let Some(fallback) = fallback {
                self.track_symbol_identity(
                    SymbolIdentity::from_symbol_handle(fallback),
                    enclosing_declaration,
                    ast::SYMBOL_FLAGS_VALUE,
                );
            }
        }
    }

    fn create_property_name_node_for_identifier_or_literal(
        &mut self,
        name: &str,
        single_quote: bool,
        string_named: bool,
        is_method: bool,
        symbol: Option<SymbolIdentity>,
    ) -> ast::Node {
        self.create_property_name_node_for_identifier_or_literal_identity(
            name,
            single_quote,
            string_named,
            is_method,
            symbol,
        )
    }

    fn create_property_name_node_for_identifier_or_literal_identity(
        &mut self,
        name: &str,
        single_quote: bool,
        string_named: bool,
        is_method: bool,
        symbol: Option<SymbolIdentity>,
    ) -> ast::Node {
        let is_method_named_new = is_method && name == "new";
        if !is_method_named_new
            && scanner::is_identifier_text(name, core::LANGUAGE_VARIANT_STANDARD)
        {
            return self.new_identifier_with_symbol_identity(name, symbol);
        }
        if !string_named
            && !is_method_named_new
            && is_numeric_literal_name(name)
            && jsnum::from_string(name).0 >= 0.0
        {
            return Self::node_value(
                self.e
                    .factory
                    .node_factory
                    .new_numeric_literal(name, ast::TOKEN_FLAGS_NONE),
            );
        }
        let result = self.e.factory.node_factory.new_string_literal(
            name,
            if single_quote {
                ast::TOKEN_FLAGS_SINGLE_QUOTE
            } else {
                ast::TOKEN_FLAGS_NONE
            },
        );
        Self::node_value(result)
    }

    fn is_string_named(&mut self, d: ast::Node) -> bool {
        let Some(store) = self.try_store_for_node(d) else {
            return false;
        };
        let name = ast::get_name_of_declaration(store, Some(d));
        if name.is_none() {
            return false;
        }
        let name = name.unwrap();
        if ast::is_computed_property_name(store, name) {
            let expression = Self::node_value(store.expression(name).unwrap());
            let t = self.ch.check_expression(expression);
            return self.ch.type_flags(t) & TYPE_FLAGS_STRING_LIKE != 0;
        }
        if ast::is_element_access_expression(store, name) {
            let argument_expression = Self::node_value(store.argument_expression(name).unwrap());
            let t = self.ch.check_expression(argument_expression);
            return self.ch.type_flags(t) & TYPE_FLAGS_STRING_LIKE != 0;
        }
        ast::is_string_literal(store, name)
    }

    fn is_single_quoted_string_named(&mut self, d: ast::Node) -> bool {
        let Some(store) = self.try_store_for_node(d) else {
            return false;
        };
        let name = ast::get_name_of_declaration(store, Some(d));
        name.is_some()
            && ast::is_string_literal(store, name.unwrap())
            && store.token_flags(name.unwrap()).is_some_and(|flags| {
                (flags & ast::TOKEN_FLAGS_SINGLE_QUOTE) != ast::TOKEN_FLAGS_NONE
            })
    }

    fn get_property_name_node_for_symbol_identity(&mut self, symbol: SymbolIdentity) -> ast::Node {
        // For hash-private names, clone the original private identifier from the declaration
        if let Some(value_declaration) = self
            .ch
            .missing_name_symbol_identity_value_declaration(symbol)
            && let Some(store) = self.try_store_for_node(value_declaration)
        {
            let decl_name = store.name(value_declaration);
            if decl_name.is_some() && ast::is_private_identifier(store, decl_name.unwrap()) {
                return Self::node_value(self.deep_clone_node(decl_name.unwrap()));
            }
        }
        let declarations = self.ch.collect_symbol_identity_declarations(symbol);
        let string_named =
            !declarations.is_empty() && declarations.iter().all(|d| self.is_string_named(*d));
        let single_quote = !declarations.is_empty()
            && declarations
                .iter()
                .all(|d| self.is_single_quoted_string_named(*d));
        let is_method = self.symbol_identity_flags(symbol) & ast::SYMBOL_FLAGS_METHOD != 0;
        let from_name_type = self.get_property_name_node_for_symbol_from_name_type_identity(
            symbol,
            single_quote,
            string_named,
            is_method,
        );
        if let Some(from_name_type) = from_name_type {
            return from_name_type;
        }

        let mut name = self.ch.symbol_identity_name(symbol).to_string();
        let private_name_prefix = format!("{}#", ast::INTERNAL_SYMBOL_NAME_PREFIX);
        if name.starts_with(private_name_prefix.as_str()) {
            // symbol IDs are unstable - replace #nnn# with #private#
            name = name[private_name_prefix.len()..].to_string();
            name = name.trim_start_matches(stringutil::is_digit).to_string();
            name = format!("__#private{}", name);
        }

        self.create_property_name_node_for_identifier_or_literal_identity(
            name.as_str(),
            single_quote,
            string_named,
            is_method,
            Some(symbol),
        )
    }

    fn get_property_name_node_for_symbol_from_name_type_identity(
        &mut self,
        symbol: SymbolIdentity,
        single_quote: bool,
        string_named: bool,
        is_method: bool,
    ) -> Option<ast::Node> {
        if !self.ch.semantic_state.has_value_symbol_link(symbol) {
            return None;
        }
        let name_type = self.ch.semantic_state.value_symbol_name_type(symbol);
        let Some(name_type) = name_type else {
            return None;
        };
        if self.ch.type_flags(name_type) & TYPE_FLAGS_STRING_OR_NUMBER_LITERAL != 0 {
            let name = match &self.ch.type_record(name_type).as_literal_type().value {
                LiteralValue::Number(v) => v.to_string(),
                LiteralValue::String(v) => v.to_string(),
                _ => String::new(),
            };
            if !scanner::is_identifier_text(name.as_str(), core::LANGUAGE_VARIANT_STANDARD)
                && (string_named || !is_numeric_literal_name(name.as_str()))
            {
                let node = self.e.factory.node_factory.new_string_literal(
                    name.as_str(),
                    if single_quote {
                        ast::TOKEN_FLAGS_SINGLE_QUOTE
                    } else {
                        ast::TOKEN_FLAGS_NONE
                    },
                );
                return Some(Self::node_value(node));
            }
            if is_numeric_literal_name(name.as_str()) && name.as_bytes()[0] == b'-' {
                let numeric = self
                    .e
                    .factory
                    .node_factory
                    .new_numeric_literal(&name[1..], ast::TOKEN_FLAGS_NONE);
                let expression = self
                    .e
                    .factory
                    .node_factory
                    .new_prefix_unary_expression(ast::KIND_MINUS_TOKEN, numeric);
                let node = self
                    .e
                    .factory
                    .node_factory
                    .new_computed_property_name(expression);
                return Some(Self::node_value(node));
            }
            return Some(
                self.create_property_name_node_for_identifier_or_literal_identity(
                    name.as_str(),
                    single_quote,
                    string_named,
                    is_method,
                    Some(symbol),
                ),
            );
        }
        if self.ch.type_flags(name_type) & TYPE_FLAGS_UNIQUE_ES_SYMBOL != 0 {
            let symbol = self.ch.type_symbol_identity(name_type).unwrap();
            let expression = self.symbol_identity_to_expression(symbol, ast::SYMBOL_FLAGS_VALUE)?;
            return Some(Self::node_value(
                self.e
                    .factory
                    .node_factory
                    .new_computed_property_name(expression.clone()),
            ));
        }
        None
    }

    pub(crate) fn get_type_from_type_node(
        &mut self,
        node: ast::Node,
        no_mapped_types: bool,
    ) -> Option<TypeHandle> {
        // !!! noMappedTypes optional param support
        let t = self.ch.get_type_from_type_node(node);
        if self.ctx.mapper.is_none() {
            return Some(t);
        }

        let instantiated = self
            .ch
            .instantiate_type_with_mapper_handle(Some(t), self.ctx.mapper);
        if no_mapped_types && instantiated != Some(t) {
            return None;
        }
        instantiated
    }

    pub(crate) fn new_string_literal(&mut self, text: &str) -> ast::Node {
        self.new_string_literal_ex(text, false /*isSingleQuote*/)
    }

    fn new_string_literal_ex(&mut self, text: &str, is_single_quote: bool) -> ast::Node {
        let mut flags = ast::TOKEN_FLAGS_NONE;
        if is_single_quote
            || self.ctx.flags & nodebuilder::FLAGS_USE_SINGLE_QUOTES_FOR_STRING_LITERAL_TYPE != 0
        {
            flags |= ast::TOKEN_FLAGS_SINGLE_QUOTE;
        }
        let node = self.e.factory.node_factory.new_string_literal(text, flags);
        Self::node_value(node)
    }

    pub(crate) fn new_identifier(
        &mut self,
        text: &str,
        symbol: Option<SymbolIdentity>,
    ) -> ast::Node {
        self.new_identifier_with_symbol_identity(text, symbol)
    }

    pub(crate) fn new_identifier_with_symbol_identity(
        &mut self,
        text: &str,
        symbol: Option<SymbolIdentity>,
    ) -> ast::Node {
        let escaped_text;
        let text = if text.contains(ast::INTERNAL_SYMBOL_NAME_PREFIX) {
            escaped_text = ast::escape_all_internal_symbol_names(text);
            escaped_text.as_str()
        } else {
            text
        };
        let id = self.e.factory.node_factory.new_identifier(text);
        let id = Self::node_value(id);
        if let Some(symbol) = symbol {
            self.id_to_symbol.insert(id, symbol);
        }
        id
    }

    fn create_access_expression(&mut self, node: ast::Node) -> ast::Node {
        let node_store = self.store_for_node(node);
        if ast::is_qualified_name(node_store, node) {
            let (left, right) = {
                let store = self.store_for_node(node);
                (store.left(node).unwrap(), store.right(node).unwrap())
            };
            let expression = self
                .create_access_expression(Self::node_value(left))
                .clone();
            let name = self.deep_clone_node(right);
            return Self::node_value(self.e.factory.node_factory.new_property_access_expression(
                expression,
                None, /*questionDotToken*/
                name,
                ast::NODE_FLAGS_NONE,
            ));
        }
        if ast::is_identifier(node_store, node)
            || ast::is_property_access_expression(node_store, node)
            || ast::is_expression_with_type_arguments(node_store, node)
        {
            return Self::node_value(self.deep_clone_node(node));
        }
        panic!(
            "unexpected access node kind: {}",
            node_store.kind(node).to_string()
        );
    }

    fn create_expression_with_type_arguments(
        &mut self,
        expr: ast::Node,
        type_arguments: Option<ast::NodeList>,
    ) -> ast::Node {
        if type_arguments.is_none()
            || self
                .e
                .factory
                .node_factory
                .emit_node_list_nodes(type_arguments.unwrap())
                .is_empty()
        {
            return expr;
        }
        Self::node_value(
            self.e
                .factory
                .node_factory
                .new_expression_with_type_arguments(expr, type_arguments),
        )
    }

    fn lookup_instantiated_type_argument_nodes_identity(
        &mut self,
        chain: Vec<SymbolIdentity>,
        index: usize,
    ) -> Option<ast::NodeList> {
        if self.should_write_type_parameters_in_qualified_name_identity(&chain, index) {
            let symbol = chain[index];
            let next_symbol = chain[index + 1];
            if self.ch.symbol_identity_check_flags(next_symbol) & ast::CHECK_FLAGS_INSTANTIATED == 0
            {
                return None;
            }

            let target_symbol = if self
                .symbol_identity_flags(symbol)
                .intersects(ast::SYMBOL_FLAGS_ALIAS)
            {
                self.ch.resolve_symbol_identity(symbol, false)
            } else {
                symbol
            };

            let type_parameters =
                self.get_type_parameters_of_class_or_interface_identity(target_symbol);
            let target_mapper = self.ch.semantic_state.value_symbol_mapper(next_symbol);
            let mut params = type_parameters.clone();
            if let Some(target_mapper) = target_mapper {
                params = params
                    .into_iter()
                    .map(|p| self.ch.map_type_mapper_handle(target_mapper, p))
                    .collect();
            }
            return self.map_to_type_nodes(params, false /*isBareList*/);
        }
        None
    }

    fn lookup_expression_chain_type_argument_nodes_identity(
        &mut self,
        chain: Vec<SymbolIdentity>,
        index: usize,
    ) -> Option<ast::NodeList> {
        if self.should_write_type_parameters_in_qualified_name_identity(&chain, index) {
            let symbol = chain[index];
            if self.ctx.type_parameter_symbol_list.has(&symbol) {
                return None;
            }
            self.ctx.type_parameter_symbol_list.add(symbol);
            if let Some(type_argument_nodes) =
                self.lookup_instantiated_type_argument_nodes_identity(chain.clone(), index)
            {
                return Some(type_argument_nodes);
            }
            let type_parameter_nodes =
                self.type_parameters_to_type_parameter_declarations_identity(symbol);
            if !type_parameter_nodes.is_empty() {
                return Some(Self::output_node_list_value(
                    self.e.factory.node_factory.new_node_list(
                        core::new_text_range(-1, -1),
                        core::new_text_range(-1, -1),
                        type_parameter_nodes.into_iter().collect::<Vec<_>>(),
                    ),
                ));
            }
        }
        None
    }

    fn should_write_type_parameters_in_qualified_name_identity(
        &mut self,
        chain: &[SymbolIdentity],
        index: usize,
    ) -> bool {
        self.ctx.flags & nodebuilder::FLAGS_WRITE_TYPE_PARAMETERS_IN_QUALIFIED_NAME != 0
            && index < chain.len() - 1
    }

    pub(crate) fn signature_to_signature_declaration_helper(
        &mut self,
        signature: SignatureHandle,
        kind: ast::Kind,
        options: Option<SignatureToSignatureDeclarationOptions>,
    ) -> ast::Node {
        let mut type_parameters: Vec<ast::Node> = Vec::new();

        let (expanded_params, cleanup) = self.enter_signature_scope(signature);
        self.ctx.approximate_length += 3;
        // Usually a signature contributes a few more characters than this, but 3 is the minimum

        let signature_record = self.ch.signature_record(signature).clone();
        if self.ctx.flags & nodebuilder::FLAGS_WRITE_TYPE_ARGUMENTS_OF_SIGNATURE != 0
            && signature_record.target.is_some()
            && signature_record.mapper.is_some()
            && !self
                .ch
                .signature_record(signature_record.target.unwrap())
                .type_parameters
                .is_empty()
        {
            let mapper = signature_record.mapper;
            let target_type_parameters = self
                .ch
                .signature_record(signature_record.target.unwrap())
                .type_parameters
                .clone();
            for parameter in target_type_parameters {
                let instantiated = self
                    .ch
                    .instantiate_type_with_mapper_handle(Some(parameter), mapper)
                    .unwrap();
                type_parameters.push(self.type_to_type_node(instantiated).unwrap().clone());
            }
        } else {
            for parameter in signature_record.type_parameters.clone() {
                type_parameters.push(self.type_parameter_to_declaration(parameter).clone());
            }
        }

        let restore_flags = self.save_restore_flags();
        self.ctx.flags &= !nodebuilder::FLAGS_SUPPRESS_ANY_RETURN_TYPE;
        // If the expanded parameter list had a variadic in a non-trailing position, don't expand it
        let params_source = if !expanded_params.is_empty()
            && expanded_params.iter().any(|p| {
                *p != expanded_params[expanded_params.len() - 1]
                    && self.ch.symbol_identity_check_flags(*p) & ast::CHECK_FLAGS_REST_PARAMETER
                        != 0
            }) {
            signature_record.parameters.to_vec()
        } else {
            expanded_params
        };
        let mut parameters: Vec<ast::Node> = params_source
            .into_iter()
            .map(|parameter| {
                self.symbol_to_parameter_declaration(parameter, kind == ast::KIND_CONSTRUCTOR)
                    .clone()
            })
            .collect::<Vec<_>>();
        let this_parameter = if self.ctx.flags & nodebuilder::FLAGS_OMIT_THIS_PARAMETER != 0 {
            None
        } else {
            self.try_get_this_parameter_declaration(signature)
        };
        if let Some(this_parameter) = this_parameter {
            parameters.insert(0, this_parameter.clone());
        }
        restore_flags(self);

        let mut return_type_node = self.serialize_return_type_for_signature(signature, true);

        let mut modifiers: Vec<ast::Node> = options
            .as_ref()
            .map(|o| o.modifiers.clone())
            .map(|modifiers| modifiers.into_iter().collect())
            .unwrap_or_default();
        if kind == ast::KIND_CONSTRUCTOR_TYPE
            && self.ch.signature_record(signature).flags & SIGNATURE_FLAGS_ABSTRACT != 0
        {
            let flags = ast::modifiers_to_flags(self.e.factory.node_factory.store(), &modifiers);
            modifiers =
                self.create_modifiers_from_modifier_flags(flags | ast::MODIFIER_FLAGS_ABSTRACT);
        }

        let param_list = self.new_factory_node_list(parameters);
        let type_param_list = if !type_parameters.is_empty() {
            Some(self.new_factory_node_list(type_parameters))
        } else {
            None
        };
        let modifier_list = if !modifiers.is_empty() {
            let modifier_flags =
                ast::modifiers_to_flags(self.e.factory.node_factory.store(), &modifiers);
            Some(self.e.factory.node_factory.new_modifier_list(
                core::new_text_range(-1, -1),
                core::new_text_range(-1, -1),
                modifiers,
                modifier_flags,
            ))
        } else {
            None
        };
        let mut name = options.as_ref().and_then(|o| o.name);
        if name.is_none() {
            name = Some(self.e.factory.node_factory.new_identifier(""));
        }

        let node = match kind {
            ast::KIND_CALL_SIGNATURE => self.e.factory.node_factory.new_call_signature_declaration(
                type_param_list.clone(),
                param_list.clone(),
                return_type_node,
            ),
            ast::KIND_CONSTRUCT_SIGNATURE => self
                .e
                .factory
                .node_factory
                .new_construct_signature_declaration(
                    type_param_list.clone(),
                    param_list.clone(),
                    return_type_node,
                ),
            ast::KIND_METHOD_SIGNATURE => {
                let question_token = options.as_ref().and_then(|o| o.question_token);
                self.e
                    .factory
                    .node_factory
                    .new_method_signature_declaration(
                        modifier_list.clone(),
                        name.clone().unwrap(),
                        question_token,
                        type_param_list.clone(),
                        param_list.clone(),
                        return_type_node,
                    )
            }
            ast::KIND_METHOD_DECLARATION => self.e.factory.node_factory.new_method_declaration(
                modifier_list.clone(),
                None, /*asteriskToken*/
                name.clone().unwrap(),
                None, /*questionToken*/
                type_param_list.clone(),
                param_list.clone(),
                return_type_node,
                None, /*fullSignature*/
                None, /*body*/
            ),
            ast::KIND_CONSTRUCTOR => self.e.factory.node_factory.new_constructor_declaration(
                modifier_list.clone(),
                None, /*typeParamList*/
                param_list.clone(),
                None, /*returnTypeNode*/
                None, /*fullSignature*/
                None, /*body*/
            ),
            ast::KIND_GET_ACCESSOR => self.e.factory.node_factory.new_get_accessor_declaration(
                modifier_list.clone(),
                name.clone().unwrap(),
                None, /*typeParamList*/
                param_list.clone(),
                return_type_node,
                None, /*fullSignature*/
                None, /*body*/
            ),
            ast::KIND_SET_ACCESSOR => self.e.factory.node_factory.new_set_accessor_declaration(
                modifier_list.clone(),
                name.clone().unwrap(),
                None, /*typeParamList*/
                param_list.clone(),
                None, /*returnTypeNode*/
                None, /*fullSignature*/
                None, /*body*/
            ),
            ast::KIND_INDEX_SIGNATURE => {
                self.e.factory.node_factory.new_index_signature_declaration(
                    modifier_list.clone(),
                    param_list.clone(),
                    return_type_node,
                )
            }
            ast::KIND_FUNCTION_TYPE => {
                if return_type_node.is_none() {
                    let empty_name = self.e.factory.node_factory.new_identifier("");
                    let type_reference = self
                        .e
                        .factory
                        .node_factory
                        .new_type_reference_node(empty_name, None);
                    return_type_node = Some(Self::node_value(type_reference));
                }
                self.e.factory.node_factory.new_function_type_node(
                    type_param_list.clone(),
                    param_list.clone(),
                    return_type_node,
                )
            }
            ast::KIND_CONSTRUCTOR_TYPE => {
                if return_type_node.is_none() {
                    let empty_name = self.e.factory.node_factory.new_identifier("");
                    let type_reference = self
                        .e
                        .factory
                        .node_factory
                        .new_type_reference_node(empty_name, None);
                    return_type_node = Some(Self::node_value(type_reference));
                }
                self.e.factory.node_factory.new_constructor_type_node(
                    modifier_list.clone(),
                    type_param_list.clone(),
                    param_list.clone(),
                    return_type_node,
                )
            }
            ast::KIND_FUNCTION_DECLARATION => {
                // TODO: assert name is Identifier
                self.e.factory.node_factory.new_function_declaration(
                    modifier_list.clone(),
                    None, /*asteriskToken*/
                    name.clone(),
                    type_param_list.clone(),
                    param_list.clone(),
                    return_type_node,
                    None, /*fullSignature*/
                    None, /*body*/
                )
            }
            ast::KIND_FUNCTION_EXPRESSION => {
                // TODO: assert name is Identifier
                let body_statements = self.new_factory_node_list(Vec::new());
                let body = self
                    .e
                    .factory
                    .node_factory
                    .new_block(body_statements, false);
                self.e.factory.node_factory.new_function_expression(
                    modifier_list.clone(),
                    None, /*asteriskToken*/
                    name.clone(),
                    type_param_list.clone(),
                    param_list.clone(),
                    return_type_node,
                    None, /*fullSignature*/
                    body,
                )
            }
            ast::KIND_ARROW_FUNCTION => {
                let body_statements = self.new_factory_node_list(Vec::new());
                let body = self
                    .e
                    .factory
                    .node_factory
                    .new_block(body_statements, false);
                self.e.factory.node_factory.new_arrow_function(
                    modifier_list.clone(),
                    type_param_list.clone(),
                    param_list.clone(),
                    return_type_node,
                    None, /*fullSignature*/
                    None, /*equalsGreaterThanToken*/
                    body,
                )
            }
            _ => panic!("Unhandled kind in signatureToSignatureDeclarationHelper"),
        };

        // !!! TODO: Smuggle type arguments of signatures out for quickinfo
        // if typeArguments != nil {
        // 	node.TypeArguments = b.f.NewNodeList(typeArguments)
        // }

        self.exit_scope(cleanup);
        Self::node_value(node)
    }

    /**
     * Serializes the return type of the signature by first trying to use the syntactic printer if possible and falling back to the checker type if not.
     */
    pub(crate) fn serialize_return_type_for_signature(
        &mut self,
        signature: SignatureHandle,
        try_reuse: bool,
    ) -> Option<ast::Node> {
        let suppress_any = self.ctx.flags & nodebuilder::FLAGS_SUPPRESS_ANY_RETURN_TYPE != 0;
        let restore_flags = self.save_restore_flags();
        if suppress_any {
            self.ctx.flags &= !nodebuilder::FLAGS_SUPPRESS_ANY_RETURN_TYPE; // suppress only toplevel `any`s
        }
        let mut return_type_node = None;

        let signature_record = self.ch.signature_record(signature).clone();
        let signature_declaration_is_non_synthesized = signature_record
            .declaration
            .is_some_and(|declaration| self.is_non_synthesized_declaration(declaration));
        let return_type =
            if signature_record.declaration.is_some() && signature_declaration_is_non_synthesized {
                let signature_declaration = signature_record.declaration.unwrap();
                let declaration_symbol = self
                    .ch
                    .get_symbol_of_declaration(signature_declaration)
                    .unwrap();
                let declaration_symbol = SymbolIdentity::from_symbol_handle(declaration_symbol);
                if let Some(return_type) = self
                    .ctx
                    .enclosing_symbol_types
                    .get(&declaration_symbol)
                    .copied()
                {
                    return_type
                } else {
                    let return_type = self.ch.get_return_type_of_signature(signature);
                    self.ch
                        .instantiate_type_with_mapper_handle(Some(return_type), self.ctx.mapper)
                        .unwrap()
                }
            } else {
                self.ch.get_return_type_of_signature(signature)
            };
        if !(suppress_any && is_type_any(self.ch, Some(return_type))) {
            if !self.is_actively_expanding()
                && try_reuse
                && self.ctx.enclosing_declaration.is_some()
                && signature_record.declaration.is_some()
                && signature_declaration_is_non_synthesized
            {
                let signature_declaration = signature_record.declaration.unwrap();
                let declaration_symbol = self
                    .ch
                    .get_symbol_of_declaration(signature_declaration)
                    .unwrap();
                let restore_symbol_type =
                    self.add_symbol_handle_type_to_context(declaration_symbol, return_type);
                let pt = self.pc.get_return_type_of_signature(
                    self.ch.store_for_node(signature_declaration),
                    &signature_declaration,
                );
                let pseudo_matches_return_type = self.pseudo_type_equivalent_to_type(
                    &pt,
                    Some(return_type),
                    false,
                    !self.ctx.suppress_report_inference_fallback,
                );
                if pseudo_matches_return_type {
                    // Also verify the pseudo type captures any inferred type predicate, not just the boolean return type.
                    // The pseudochecker is unaware of inferred type predicates, so it produces boolean where
                    // the checker infers e.g. `x is string`.
                    let mut pt = Some(pt);
                    if let Some(type_predicate) = self.ch.get_type_predicate_of_signature(signature)
                        && !self.pseudo_return_type_matches_predicate(
                            pt.as_ref().unwrap(),
                            type_predicate,
                        )
                    {
                        if !self.ctx.suppress_report_inference_fallback {
                            self.report_inference_fallback(signature_declaration);
                        }
                        pt = None;
                    }
                    if let Some(pt) = pt {
                        // !!! TODO: If annotated type node is a reference with insufficient type arguments, we should still fall back to type serialization
                        // see: canReuseTypeNodeAnnotation in strada for context
                        let should_fall_back_to_type_predicate_serialization =
                            pt.kind == pseudochecker::PseudoTypeKind::Direct && {
                                let existing = pt.as_pseudo_type_direct().type_node;
                                ast::is_type_predicate_node(self.store_for_node(existing), existing)
                                    && self.store_for_node(existing).r#type(existing).is_some_and(
                                        |type_node| {
                                            self.ch
                                                .invalid_jsdoc_type_token(type_node)
                                                .is_some_and(|(token, _)| token == '?')
                                        },
                                    )
                            };
                        if !should_fall_back_to_type_predicate_serialization {
                            return_type_node =
                                self.pseudo_type_to_node_with_checker_fallback(&pt, return_type);
                        }
                    }
                }
                restore_symbol_type(self);
            }
            if return_type_node.is_none() {
                return_type_node =
                    Some(self.serialize_inferred_return_type_for_signature(signature, return_type));
            }
        }

        if return_type_node.is_none() && !suppress_any {
            return_type_node = Some(Self::node_value(
                self.e
                    .factory
                    .node_factory
                    .new_keyword_type_node(ast::KIND_ANY_KEYWORD),
            ));
        }
        restore_flags(self);
        return_type_node
    }
}

// Direct serialization core functions for types, type aliases, and symbols

const MAX_REVERSE_MAPPED_NESTING_INSPECTION_DEPTH: usize = 3;

impl<'a, 'state, 'c, 'e> NodeBuilderImpl<'a, 'state, 'c, 'e> {
    pub(crate) fn get_declaration_statements_for_source_file(
        &mut self,
        source_file: ast::Node,
    ) -> Vec<ast::Node> {
        debug_assert!(ast::is_source_file(
            self.store_for_node(source_file),
            source_file
        ));
        let symbol = self.ch.get_symbol_of_declaration(source_file);
        let (symbols, modifier_flags) = if let Some(symbol) = symbol {
            let symbol = SymbolIdentity::from_symbol_handle(symbol);
            // resolveExternalModuleSymbol(sym); // ensures cjs export assignment is setup
            let symbols = if let Some(resolved) = self
                .ch
                .resolve_external_module_symbol_identity(symbol, false /*dontResolveAlias*/)
            {
                self.ch
                    .with_symbol_identity_export_table(resolved, |exports| {
                        exports
                            .map(|exports| {
                                let mut symbols = Vec::with_capacity(exports.len());
                                exports.for_each_value(|symbol| symbols.push(symbol));
                                symbols
                            })
                            .unwrap_or_default()
                    })
            } else {
                Vec::new()
            };
            (symbols, ast::ModifierFlags::EXPORT)
        } else {
            let symbols = self
                .ch
                .with_node_locals(source_file, |locals| {
                    locals
                        .values()
                        .map(|symbol| SymbolIdentity::from_symbol_handle(*symbol))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            (symbols, ast::ModifierFlags::AMBIENT)
        };
        self.symbol_table_to_declaration_statements(symbols, modifier_flags)
    }

    fn symbol_table_to_declaration_statements(
        &mut self,
        symbols: Vec<SymbolIdentity>,
        modifier_flags: ast::ModifierFlags,
    ) -> Vec<ast::Node> {
        let mut results = Vec::new();
        let mut used_symbol_names = Vec::new();
        for symbol in &symbols {
            used_symbol_names.push(self.ch.missing_name_symbol_identity_name(*symbol));
        }
        for symbol in symbols {
            if !self.serialize_symbol(
                symbol,
                false, /*isPrivate*/
                false, /*propertyAsAlias*/
                modifier_flags,
                &mut used_symbol_names,
                &mut results,
            ) {
                return Vec::new();
            }
        }
        self.merge_redundant_statements(results)
    }

    fn merge_redundant_statements(&mut self, statements: Vec<ast::Node>) -> Vec<ast::Node> {
        self.merge_export_declarations(statements)
    }

    fn merge_export_declarations(&mut self, statements: Vec<ast::Node>) -> Vec<ast::Node> {
        struct ExportGroup {
            key: String,
            module_specifier: Option<ast::Node>,
            specifiers: Vec<ast::Node>,
            count: usize,
        }

        let mut non_exports = Vec::new();
        let mut groups: Vec<ExportGroup> = Vec::new();
        let mut export_count = 0;
        for statement in statements {
            let store = self.store_for_node(statement);
            let is_export_declaration = ast::is_export_declaration(store, statement)
                && store.attributes(statement).is_none();
            let Some(export_clause) = is_export_declaration
                .then(|| store.export_clause(statement))
                .flatten()
            else {
                non_exports.push(statement);
                continue;
            };
            if store.kind(export_clause) != ast::Kind::NamedExports {
                non_exports.push(statement);
                continue;
            }
            let module_specifier = store.module_specifier(statement);
            let key = module_specifier
                .map(|specifier| format!(">{}", store.text(specifier)))
                .unwrap_or_default();
            let Some(elements) = store.elements(export_clause) else {
                non_exports.push(statement);
                continue;
            };
            export_count += 1;
            if let Some(group) = groups.iter_mut().find(|group| group.key == key) {
                group.specifiers.extend(elements.iter());
                group.count += 1;
            } else {
                groups.push(ExportGroup {
                    key,
                    module_specifier,
                    specifiers: elements.iter().collect(),
                    count: 1,
                });
            }
        }
        if export_count == groups.len() {
            for group in groups {
                let specifiers = self.new_factory_node_list(group.specifiers);
                let export_clause = self.e.factory.node_factory.new_named_exports(specifiers);
                non_exports.push(self.e.factory.node_factory.new_export_declaration(
                    None,
                    false,
                    Some(export_clause),
                    group.module_specifier,
                    None,
                ));
            }
            return non_exports;
        }
        for group in groups {
            let specifiers = self.new_factory_node_list(group.specifiers);
            let export_clause = self.e.factory.node_factory.new_named_exports(specifiers);
            non_exports.push(self.e.factory.node_factory.new_export_declaration(
                None,
                false,
                Some(export_clause),
                group.module_specifier,
                None,
            ));
        }
        non_exports
    }

    fn serialize_symbol(
        &mut self,
        symbol: SymbolIdentity,
        _is_private: bool,
        property_as_alias: bool,
        modifier_flags: ast::ModifierFlags,
        used_symbol_names: &mut Vec<String>,
        results: &mut Vec<ast::Node>,
    ) -> bool {
        let symbol_flags = self.ch.symbol_identity_flags(symbol);
        if symbol_flags
            & (ast::SYMBOL_FLAGS_BLOCK_SCOPED_VARIABLE
                | ast::SYMBOL_FLAGS_FUNCTION_SCOPED_VARIABLE
                | ast::SYMBOL_FLAGS_FUNCTION
                | ast::SYMBOL_FLAGS_METHOD
                | ast::SYMBOL_FLAGS_PROPERTY
                | ast::SYMBOL_FLAGS_ACCESSOR
                | ast::SYMBOL_FLAGS_ALIAS)
            == 0
        {
            return false;
        }
        if symbol_flags & ast::SYMBOL_FLAGS_ALIAS != 0 {
            let local_name = self.get_internal_symbol_name(symbol, used_symbol_names);
            return self.serialize_as_alias(symbol, &local_name, modifier_flags, results);
        }
        if property_as_alias
            && symbol_flags
                & (ast::SYMBOL_FLAGS_BLOCK_SCOPED_VARIABLE
                    | ast::SYMBOL_FLAGS_FUNCTION_SCOPED_VARIABLE
                    | ast::SYMBOL_FLAGS_PROPERTY
                    | ast::SYMBOL_FLAGS_ACCESSOR)
                != 0
            && self.ch.missing_name_symbol_identity_name(symbol)
                != ast::INTERNAL_SYMBOL_NAME_EXPORT_EQUALS
            && symbol_flags
                & (ast::SYMBOL_FLAGS_PROTOTYPE | ast::SYMBOL_FLAGS_CLASS | ast::SYMBOL_FLAGS_METHOD)
                == 0
        {
            return self.serialize_maybe_alias_assignment(symbol, used_symbol_names, results);
        }
        let t = self.ch.get_type_of_symbol_identity(symbol);
        if let Some(type_symbol) = self.ch.type_symbol_identity(t) {
            let type_symbol_flags = self.ch.symbol_identity_flags(type_symbol);
            let is_anonymous_expando_function = type_symbol != symbol
                && type_symbol_flags & ast::SYMBOL_FLAGS_FUNCTION != 0
                && self
                    .ch
                    .collect_symbol_identity_declarations(type_symbol)
                    .iter()
                    .any(|declaration| {
                        ast::is_function_expression_or_arrow_function(
                            self.store_for_node(*declaration),
                            *declaration,
                        )
                    })
                && (self
                    .ch
                    .collect_symbol_identity_member_table(type_symbol)
                    .is_some_and(|members| !members.is_empty())
                    || self
                        .ch
                        .collect_symbol_identity_export_table(type_symbol)
                        .is_some_and(|exports| !exports.is_empty()));
            if is_anonymous_expando_function {
                self.ctx
                    .remapped_symbol_references
                    .insert(type_symbol, symbol);
                let result = self.serialize_symbol(
                    type_symbol,
                    _is_private,
                    property_as_alias,
                    modifier_flags,
                    used_symbol_names,
                    results,
                );
                self.ctx.remapped_symbol_references.remove(&type_symbol);
                return result;
            }
        }
        if self.is_type_representable_as_function_namespace_merge(t, symbol) {
            let local_name = self.get_internal_symbol_name(symbol, used_symbol_names);
            self.serialize_as_function_namespace_merge(
                t,
                symbol,
                &local_name,
                modifier_flags,
                results,
            );
            return true;
        }
        false
    }

    fn serialize_as_alias(
        &mut self,
        symbol: SymbolIdentity,
        _local_name: &str,
        _modifier_flags: ast::ModifierFlags,
        results: &mut Vec<ast::Node>,
    ) -> bool {
        let Some(node) = self.ch.get_declaration_of_alias_symbol_identity(symbol) else {
            return false;
        };
        let target = self
            .ch
            .get_target_of_alias_declaration(Some(node), true /*dontRecursivelyResolve*/);
        let Some(target) = self.ch.get_merged_symbol_identity(target) else {
            return false;
        };
        let mut verbatim_target_name = self.ch.missing_name_symbol_identity_name(target);
        let (kind, specifier_text, is_default_property_name) = {
            let store = self.store_for_node(node);
            let specifier = store
                .parent(node)
                .and_then(|parent| store.parent(parent))
                .and_then(|parent| store.module_specifier(parent));
            (
                store.kind(node),
                specifier.and_then(|specifier| {
                    ast::is_string_literal_like(store, specifier).then(|| store.text(specifier))
                }),
                store.property_name(node).is_some_and(|property_name| {
                    ast::module_export_name_is_default(store, property_name)
                }),
            )
        };
        match kind {
            ast::Kind::ExportSpecifier => {
                if is_default_property_name {
                    verbatim_target_name = ast::INTERNAL_SYMBOL_NAME_DEFAULT.to_owned();
                }
                let local_name = self.ch.missing_name_symbol_identity_name(symbol);
                let mut used_symbol_names = vec![local_name.clone()];
                let target_name = if specifier_text.is_some() {
                    verbatim_target_name
                } else {
                    self.get_internal_symbol_name(target, &mut used_symbol_names)
                };
                let specifier = specifier_text.map(|specifier| self.new_string_literal(&specifier));
                self.serialize_export_specifier(&local_name, &target_name, specifier, results);
                true
            }
            _ => false,
        }
    }

    /**
     * Returns `true` if an export assignment or declaration was produced for the symbol
     */
    fn serialize_maybe_alias_assignment(
        &mut self,
        symbol: SymbolIdentity,
        used_symbol_names: &mut Vec<String>,
        results: &mut Vec<ast::Node>,
    ) -> bool {
        if self.ch.symbol_identity_flags(symbol) & ast::SYMBOL_FLAGS_PROTOTYPE != 0 {
            return false;
        }
        let name = self.ch.missing_name_symbol_identity_name(symbol);
        let is_export_equals = name == ast::INTERNAL_SYMBOL_NAME_EXPORT_EQUALS;
        let is_default = name == ast::INTERNAL_SYMBOL_NAME_DEFAULT;
        let is_export_assignment_compatible_symbol_name = is_export_equals || is_default;
        // synthesize export = ref
        // ref should refer to either be a locally scoped symbol which we need to emit, or
        // a reference to another namespace/module which we may need to emit an `import` statement for
        let alias_decl = self.ch.get_declaration_of_alias_symbol_identity(symbol);
        // serialize what the alias points to, preserve the declaration's initializer
        let target = alias_decl.and_then(|alias_decl| {
            self.ch.get_target_of_alias_declaration(
                Some(alias_decl),
                true, /*dontRecursivelyResolve*/
            )
        });
        // If the target resolves and resolves to a thing defined in this file, emit as an alias, otherwise emit as a const
        if let Some(target) =
            target.filter(|target| self.symbol_has_declaration_in_enclosing_file(*target))
        {
            // In case `target` refers to a namespace member, look at the declaration and serialize the leftmost symbol in it
            // eg, `namespace A { export class B {} }; exports = A.B;`
            // Technically, this is all that's required in the case where the assignment is an entity name expression
            let expr = alias_decl.and_then(|alias_decl| {
                let store = self.store_for_node(alias_decl);
                if ast::is_export_assignment(store, alias_decl)
                    || ast::is_binary_expression(store, alias_decl)
                {
                    self.get_export_assignment_expression(alias_decl)
                } else {
                    self.get_property_assignment_alias_like_expression(alias_decl)
                }
            });
            let first = expr.and_then(|expr| {
                ast::is_entity_name_expression(self.store_for_node(expr), expr)
                    .then(|| self.get_first_non_module_exports_identifier(expr))
                    .flatten()
            });
            if is_export_assignment_compatible_symbol_name {
                self.ctx.approximate_length += 10; // `export = ;`
                let expression = self.symbol_to_expression(target, ast::SYMBOL_FLAGS_ALL);
                results.push(self.e.factory.node_factory.new_export_assignment(
                    None,
                    is_export_equals,
                    None,
                    expression,
                ));
            } else if first == expr && first.is_some() {
                // serialize as `export {target as name}`
                let first = first.unwrap();
                let target_name = self.store_for_node(first).text(first);
                self.serialize_export_specifier(&name, &target_name, None, results);
            } else if expr
                .is_some_and(|expr| ast::is_class_expression(self.store_for_node(expr), expr))
            {
                let target_name = self.get_internal_symbol_name(target, used_symbol_names);
                self.serialize_export_specifier(&name, &target_name, None, results);
            } else {
                // serialize as `import _Ref = t.arg.et; export { _Ref as name }`
                let var_name = self.get_unused_name(&name, symbol, used_symbol_names);
                self.ctx.approximate_length += var_name.len() + 10; // `import name = ;`
                let import_name = self.e.factory.node_factory.new_identifier(var_name.clone());
                let module_reference = self.symbol_identity_to_entity_name_node(target).clone();
                results.push(self.e.factory.node_factory.new_import_equals_declaration(
                    None,
                    false,
                    import_name,
                    module_reference,
                ));
                self.serialize_export_specifier(&name, &var_name, None, results);
            }
            return true;
        }
        false
    }

    fn symbol_has_declaration_in_enclosing_file(&mut self, symbol: SymbolIdentity) -> bool {
        let ctx_src = self.ctx.enclosing_file;
        let declarations = self.ch.collect_symbol_identity_declarations(symbol);
        !declarations.is_empty()
            && declarations.iter().any(|declaration| {
                self.ch
                    .try_source_file_for_node(*declaration)
                    .map(SourceFileIdentity::from_source_file)
                    == ctx_src
            })
    }

    fn get_export_assignment_expression(&mut self, node: ast::Node) -> Option<ast::Node> {
        let store = self.store_for_node(node);
        if ast::is_export_assignment(store, node) {
            store.expression(node)
        } else if ast::is_binary_expression(store, node) {
            store.right(node)
        } else {
            None
        }
    }

    fn get_property_assignment_alias_like_expression(
        &mut self,
        node: ast::Node,
    ) -> Option<ast::Node> {
        let store = self.store_for_node(node);
        match store.kind(node) {
            ast::Kind::ShorthandPropertyAssignment => store.name(node),
            ast::Kind::PropertyAssignment => store.initializer(node),
            ast::Kind::PropertyAccessExpression => store.parent(node).and_then(|parent| {
                let parent_store = self.store_for_node(parent);
                ast::is_binary_expression(parent_store, parent)
                    .then(|| parent_store.right(parent))
                    .flatten()
            }),
            _ => None,
        }
    }

    fn get_first_non_module_exports_identifier(
        &mut self,
        mut node: ast::Node,
    ) -> Option<ast::Node> {
        loop {
            let store = self.store_for_node(node);
            match store.kind(node) {
                ast::Kind::Identifier => return Some(node),
                ast::Kind::QualifiedName => {
                    node = store.left(node)?;
                    while !ast::is_identifier(self.store_for_node(node), node) {
                        node = self.store_for_node(node).left(node)?;
                    }
                    return Some(node);
                }
                ast::Kind::PropertyAccessExpression => {
                    let expression = store.expression(node)?;
                    if ast::is_module_exports_access_expression(
                        self.store_for_node(expression),
                        expression,
                    ) {
                        return store.name(node);
                    }
                    node = expression;
                }
                _ => return None,
            }
        }
    }

    fn serialize_export_specifier(
        &mut self,
        local_name: &str,
        target_name: &str,
        specifier: Option<ast::Node>,
        results: &mut Vec<ast::Node>,
    ) {
        self.ctx.approximate_length += 16
            + local_name.len()
            + if local_name != target_name {
                target_name.len()
            } else {
                0
            };
        let property_name = if local_name != target_name {
            Some(self.e.factory.node_factory.new_identifier(target_name))
        } else {
            None
        };
        let name = self.e.factory.node_factory.new_identifier(local_name);
        let specifier_node =
            self.e
                .factory
                .node_factory
                .new_export_specifier(false, property_name, name);
        let specifiers = self.new_factory_node_list(vec![specifier_node]);
        let export_clause = self.e.factory.node_factory.new_named_exports(specifiers);
        results.push(self.e.factory.node_factory.new_export_declaration(
            None,
            false,
            Some(export_clause),
            specifier,
            None,
        ));
    }

    fn get_internal_symbol_name(
        &mut self,
        symbol: SymbolIdentity,
        used_symbol_names: &mut Vec<String>,
    ) -> String {
        let name = self.get_name_of_symbol_as_written_identity(symbol);
        if !used_symbol_names.iter().any(|used| used == &name) {
            used_symbol_names.push(name.clone());
            return name;
        }
        name
    }

    fn get_unused_name(
        &mut self,
        name: &str,
        symbol: SymbolIdentity,
        used_symbol_names: &mut Vec<String>,
    ) -> String {
        if !used_symbol_names.iter().any(|used| used == name) {
            used_symbol_names.push(name.to_owned());
            return name.to_owned();
        }
        let symbol_declarations = self.ch.collect_symbol_identity_declarations(symbol);
        if symbol_declarations.iter().all(|declaration| {
            self.store_for_node(*declaration).kind(*declaration) != ast::KIND_VARIABLE_DECLARATION
        }) {
            let mut i = 1;
            loop {
                let candidate = format!("{name}_{i}");
                if !used_symbol_names.iter().any(|used| used == &candidate) {
                    used_symbol_names.push(candidate.clone());
                    return candidate;
                }
                i += 1;
            }
        }
        name.to_owned()
    }

    fn serialize_as_function_namespace_merge(
        &mut self,
        t: TypeHandle,
        symbol: SymbolIdentity,
        local_name: &str,
        modifier_flags: ast::ModifierFlags,
        results: &mut Vec<ast::Node>,
    ) {
        let signatures = self.ch.get_signatures_of_type(t, SIGNATURE_KIND_CALL);
        for sig in signatures {
            self.ctx.approximate_length += 1; // ;
            let name = self
                .e
                .factory
                .node_factory
                .new_identifier(local_name.to_owned());
            let modifiers = self.create_modifiers_from_modifier_flags(modifier_flags);
            let decl = self.signature_to_signature_declaration_helper(
                sig,
                ast::KIND_FUNCTION_DECLARATION,
                Some(SignatureToSignatureDeclarationOptions {
                    modifiers,
                    name: Some(name),
                    ..Default::default()
                }),
            );
            results.push(decl);
        }
        let symbol_flags = self.ch.symbol_identity_flags(symbol);
        let has_module_exports = symbol_flags
            & (ast::SYMBOL_FLAGS_VALUE_MODULE | ast::SYMBOL_FLAGS_NAMESPACE_MODULE)
            != 0
            && self
                .ch
                .collect_symbol_identity_export_table(symbol)
                .is_some_and(|exports| !exports.is_empty());
        if !has_module_exports {
            let props = self
                .ch
                .get_properties_of_type(t)
                .into_iter()
                .filter(|prop| self.is_declaration_namespace_member(*prop))
                .collect::<Vec<_>>();
            self.ctx.approximate_length += local_name.len();
            self.serialize_as_namespace_declaration(
                props,
                local_name,
                modifier_flags,
                true, /*suppressNewPrivateContext*/
                results,
            );
        }
    }

    fn serialize_as_namespace_declaration(
        &mut self,
        props: Vec<SymbolIdentity>,
        local_name: &str,
        modifier_flags: ast::ModifierFlags,
        _suppress_new_private_context: bool,
        results: &mut Vec<ast::Node>,
    ) {
        if props.is_empty() {
            return;
        }
        self.ctx.approximate_length += 14; // "namespace { }"
        let mut used_symbol_names = Vec::new();
        let mut body_statements = Vec::new();
        let force_alias_for_properties = props.iter().any(|prop| {
            let flags = self.ch.symbol_identity_flags(*prop);
            flags & ast::SYMBOL_FLAGS_GET_ACCESSOR != 0
                && flags & ast::SYMBOL_FLAGS_SET_ACCESSOR != 0
        });
        for prop in props {
            if self.serialize_symbol(
                prop,
                false, /*isPrivate*/
                true,  /*propertyAsAlias*/
                ast::ModifierFlags::EXPORT,
                &mut used_symbol_names,
                &mut body_statements,
            ) {
                continue;
            }
            self.serialize_declaration_namespace_member(
                prop,
                &mut used_symbol_names,
                &mut body_statements,
                force_alias_for_properties,
            );
        }
        self.strip_export_modifiers_if_all_statements_are_exported(&mut body_statements);
        let body_statements = self.new_factory_node_list(body_statements);
        let body = self
            .e
            .factory
            .node_factory
            .new_module_block(body_statements);
        let name = self
            .e
            .factory
            .node_factory
            .new_identifier(local_name.to_owned());
        let modifiers = if modifier_flags == ast::ModifierFlags::NONE {
            None
        } else {
            Some(self.new_factory_modifier_list(modifier_flags))
        };
        results.push(self.e.factory.node_factory.new_module_declaration(
            modifiers,
            ast::KIND_NAMESPACE_KEYWORD,
            name,
            Some(body),
        ));
    }

    fn serialize_declaration_namespace_member(
        &mut self,
        symbol: SymbolIdentity,
        used_symbol_names: &mut Vec<String>,
        body_statements: &mut Vec<ast::Node>,
        force_alias_for_properties: bool,
    ) {
        let name = self.ch.missing_name_symbol_identity_name(symbol);
        let symbol_flags = self.ch.symbol_identity_flags(symbol);
        let local_name =
            if force_alias_for_properties && symbol_flags & ast::SYMBOL_FLAGS_ACCESSOR == 0 {
                let mut i = 1;
                loop {
                    let candidate = format!("{name}_{i}");
                    if !used_symbol_names.iter().any(|used| used == &candidate) {
                        used_symbol_names.push(candidate.clone());
                        break candidate;
                    }
                    i += 1;
                }
            } else {
                self.get_unused_name(&name, symbol, used_symbol_names)
            };
        let t = if symbol_flags & ast::SYMBOL_FLAGS_ACCESSOR != 0
            && symbol_flags & ast::SYMBOL_FLAGS_SET_ACCESSOR != 0
        {
            self.ch
                .get_write_type_of_symbol_handle(symbol.symbol_handle())
        } else {
            self.ch.get_type_of_symbol_identity(symbol)
        };
        if self.is_type_representable_as_function_namespace_merge(t, symbol) {
            self.serialize_as_function_namespace_merge(
                t,
                symbol,
                &local_name,
                ast::ModifierFlags::EXPORT,
                body_statements,
            );
            return;
        }
        let type_node = self.serialize_type_for_declaration_for_symbol_identity(
            None,
            Some(t),
            Some(symbol),
            true,
        );
        let declaration_name = self
            .e
            .factory
            .node_factory
            .new_identifier(local_name.clone());
        let declaration = self.e.factory.node_factory.new_variable_declaration(
            declaration_name,
            None,
            Some(type_node),
            None,
        );
        let declarations = self.new_factory_node_list(vec![declaration]);
        let flags = if symbol_flags & ast::SYMBOL_FLAGS_ACCESSOR != 0
            && symbol_flags & ast::SYMBOL_FLAGS_SET_ACCESSOR == 0
        {
            ast::NodeFlags::CONST
        } else {
            ast::NodeFlags::LET
        };
        let declaration_list = self
            .e
            .factory
            .node_factory
            .new_variable_declaration_list(declarations, flags);
        let modifiers = if local_name == name {
            Some(self.new_factory_modifier_list(ast::ModifierFlags::EXPORT))
        } else {
            None
        };
        body_statements.push(
            self.e
                .factory
                .node_factory
                .new_variable_statement(modifiers, declaration_list),
        );
        if local_name != name {
            let property_name = self.e.factory.node_factory.new_identifier(local_name);
            let export_name = self.e.factory.node_factory.new_identifier(name);
            let specifier = self.e.factory.node_factory.new_export_specifier(
                false,
                Some(property_name),
                export_name,
            );
            let specifiers = self.new_factory_node_list(vec![specifier]);
            let export_clause = self.e.factory.node_factory.new_named_exports(specifiers);
            body_statements.push(self.e.factory.node_factory.new_export_declaration(
                None,
                false,
                Some(export_clause),
                None,
                None,
            ));
        }
    }

    fn strip_export_modifiers_if_all_statements_are_exported(
        &mut self,
        body_statements: &mut [ast::Node],
    ) {
        if body_statements.is_empty()
            || body_statements.iter().any(|statement| {
                !ast::can_have_modifiers(self.store_for_node(*statement), *statement)
                    || ast::get_combined_modifier_flags(self.store_for_node(*statement), *statement)
                        & ast::ModifierFlags::EXPORT
                        == ast::ModifierFlags::NONE
            })
        {
            return;
        }
        for statement in body_statements {
            let flags =
                ast::get_combined_modifier_flags(self.store_for_node(*statement), *statement)
                    & !ast::ModifierFlags::EXPORT;
            let modifiers = self.new_factory_modifier_list(flags);
            *statement = ast::replace_modifiers(
                &mut self.e.factory.node_factory,
                *statement,
                Some(modifiers),
            );
        }
    }

    fn is_type_representable_as_function_namespace_merge(
        &mut self,
        type_to_serialize: TypeHandle,
        host_symbol: SymbolIdentity,
    ) -> bool {
        let object_flags = self.ch.object_flags(type_to_serialize);
        if object_flags & (OBJECT_FLAGS_ANONYMOUS | OBJECT_FLAGS_MAPPED) == 0 {
            return false;
        }
        if self
            .ch
            .type_symbol_identity(type_to_serialize)
            .is_some_and(|symbol| {
                self.ch
                    .collect_symbol_identity_declarations(symbol)
                    .iter()
                    .any(|declaration| {
                        ast::is_type_node(self.store_for_node(*declaration), *declaration)
                    })
            })
        {
            return false;
        }
        if !self
            .ch
            .get_index_infos_of_type(type_to_serialize)
            .is_empty()
            || is_class_instance_side(self.ch, type_to_serialize)
        {
            return false;
        }
        let properties = self.ch.get_properties_of_type(type_to_serialize);
        let has_namespace_members = properties
            .iter()
            .any(|prop| self.is_declaration_namespace_member(*prop));
        if !has_namespace_members
            && self
                .ch
                .get_signatures_of_type(type_to_serialize, SIGNATURE_KIND_CALL)
                .is_empty()
        {
            return false;
        }
        if !self
            .ch
            .get_signatures_of_type(type_to_serialize, SIGNATURE_KIND_CONSTRUCT)
            .is_empty()
        {
            return false;
        }
        if self
            .get_declaration_with_type_annotation(host_symbol)
            .is_some()
        {
            return false;
        }
        let ctx_src = self.ctx.enclosing_file;
        if self
            .ch
            .type_symbol_identity(type_to_serialize)
            .is_some_and(|symbol| {
                self.ch
                    .collect_symbol_identity_declarations(symbol)
                    .iter()
                    .any(|declaration| {
                        self.ch
                            .try_source_file_for_node(*declaration)
                            .map(SourceFileIdentity::from_source_file)
                            != ctx_src
                    })
            })
        {
            return false;
        }
        for prop in &properties {
            let name = self.ch.missing_name_symbol_identity_name(*prop);
            if is_late_bound_name(&name)
                || !scanner::is_identifier_text(&name, core::LANGUAGE_VARIANT_STANDARD)
            {
                return false;
            }
            if self
                .ch
                .collect_symbol_identity_declarations(*prop)
                .iter()
                .any(|declaration| {
                    self.ch
                        .try_source_file_for_node(*declaration)
                        .map(SourceFileIdentity::from_source_file)
                        != ctx_src
                })
            {
                return false;
            }
            let prop_flags = self.ch.symbol_identity_flags(*prop);
            if prop_flags & ast::SYMBOL_FLAGS_ACCESSOR != 0
                && self.ch.get_non_missing_type_of_symbol_identity(*prop)
                    != self
                        .ch
                        .get_write_type_of_symbol_handle(prop.symbol_handle())
            {
                return false;
            }
        }
        true
    }

    fn get_declaration_with_type_annotation(
        &mut self,
        symbol: SymbolIdentity,
    ) -> Option<ast::Node> {
        self.ch
            .collect_symbol_identity_declarations(symbol)
            .into_iter()
            .find(|declaration| {
                let store = self.store_for_node(*declaration);
                store.type_node(*declaration).is_some()
            })
    }

    fn is_declaration_namespace_member(&mut self, symbol: SymbolIdentity) -> bool {
        let flags = self.ch.symbol_identity_flags(symbol);
        if flags & (ast::SYMBOL_FLAGS_TYPE | ast::SYMBOL_FLAGS_NAMESPACE | ast::SYMBOL_FLAGS_ALIAS)
            != 0
        {
            return true;
        }
        if flags & ast::SYMBOL_FLAGS_PROTOTYPE != 0
            || self.ch.missing_name_symbol_identity_name(symbol) == "prototype"
        {
            return false;
        }
        let Some(value_declaration) = self
            .ch
            .missing_name_symbol_identity_value_declaration(symbol)
        else {
            return true;
        };
        let store = self.store_for_node(value_declaration);
        if !ast::is_class_like(
            store,
            store.parent(value_declaration).unwrap_or(value_declaration),
        ) {
            return true;
        }
        !ast::is_static(store, value_declaration)
    }

    fn alias_to_type_reference_node(
        &mut self,
        alias: &crate::semantic::TypeAliasRecord,
    ) -> ast::Node {
        let symbol = alias
            .symbol
            .expect("alias type reference must keep alias symbol");
        let type_name = self.symbol_identity_to_entity_name_node(symbol).clone();
        let type_arguments = self.map_to_alias_type_argument_nodes(alias);
        Self::node_value(
            self.e
                .factory
                .node_factory
                .new_type_reference_node(type_name, type_arguments),
        )
    }

    fn map_to_alias_type_argument_nodes(
        &mut self,
        alias: &crate::semantic::TypeAliasRecord,
    ) -> Option<ast::NodeList> {
        self.map_to_type_nodes(alias.type_arguments.clone(), false /*isBareList*/)
    }

    pub(crate) fn add_property_identity_to_element_list(
        &mut self,
        property_symbol: SymbolIdentity,
        mut type_elements: Vec<ast::Node>,
    ) -> Vec<ast::Node> {
        let property_flags = self.ch.symbol_identity_flags(property_symbol);
        let property_name = self.ch.missing_name_symbol_identity_name(property_symbol);
        let property_declarations = self
            .ch
            .collect_symbol_identity_declarations(property_symbol);
        let property_value_declaration = self
            .ch
            .missing_name_symbol_identity_value_declaration(property_symbol);
        let property_is_reverse_mapped = self.ch.symbol_identity_check_flags(property_symbol)
            & ast::CHECK_FLAGS_REVERSE_MAPPED
            != 0;
        let property_type = if self.should_use_placeholder_for_property_identity(property_symbol) {
            self.ch.semantic_state.semantic_handles().any_type
        } else {
            self.ch
                .get_non_missing_type_of_symbol_identity(property_symbol)
        };
        let save_enclosing_declaration = self.ctx.enclosing_declaration;
        self.ctx.enclosing_declaration = None;
        if is_late_bound_name(property_name.as_str()) {
            if !property_declarations.is_empty() {
                let decl = property_declarations[0];
                if self.ch.has_late_bindable_name(decl) {
                    let decl_store = self.store_for_node(decl);
                    if ast::is_binary_expression(decl_store, decl) {
                        if let Some(name) = ast::get_name_of_declaration(decl_store, Some(decl)) {
                            let name = Self::node_value(name);
                            if ast::is_element_access_expression(self.store_for_node(name), name)
                                && {
                                    let name_store = self.store_for_node(name);
                                    let argument_expression = name_store
                                        .argument_expression(name)
                                        .expect("element access should have argument expression");
                                    ast::is_property_access_entity_name_expression(
                                        name_store,
                                        argument_expression,
                                        false, /*allowJs*/
                                    )
                                }
                            {
                                let name_store = self.store_for_node(name);
                                let argument_expression =
                                    name_store.argument_expression(name).unwrap();
                                self.track_computed_name(
                                    Self::node_value(argument_expression),
                                    save_enclosing_declaration,
                                );
                            }
                        }
                    } else if let Some(name) = decl_store.name(decl) {
                        let name_store = self.store_for_node(name);
                        if let Some(expression) = name_store.expression(name) {
                            let expression = Self::node_value(expression);
                            self.track_computed_name(expression, save_enclosing_declaration);
                        }
                    }
                }
            } else {
                let property_name = self.ch.symbol_identity_to_string(property_symbol);
                self.report_non_serializable_property(&property_name);
            }
        }
        self.ctx.enclosing_declaration = property_value_declaration
            .or_else(|| property_declarations.first().copied())
            .filter(|declaration| self.try_store_for_node(*declaration).is_some())
            .or(save_enclosing_declaration);
        let property_name_node = self.get_property_name_node_for_symbol_identity(property_symbol);
        self.ctx.enclosing_declaration = save_enclosing_declaration;
        self.ctx.approximate_length += property_name.len() + 1;

        if property_flags & ast::SYMBOL_FLAGS_ACCESSOR != 0 {
            let symbol_handle = property_symbol.symbol_handle();
            let write_type = self.ch.get_write_type_of_symbol_handle(symbol_handle);
            if !self.ch.is_error_type(property_type) && !self.ch.is_error_type(write_type) {
                let prop_declaration = self
                    .declaration_of_kind_identity(property_symbol, ast::KIND_PROPERTY_DECLARATION)
                    .map(Self::node_value);
                let parent_is_class =
                    self.ch
                        .symbol_identity_parent(property_symbol)
                        .is_some_and(|parent| {
                            self.ch.symbol_identity_flags(parent) & ast::SYMBOL_FLAGS_CLASS != 0
                        });
                if property_type != write_type || parent_is_class && prop_declaration.is_none() {
                    let symbol_mapper = self.ch.semantic_state.value_symbol_mapper(property_symbol);
                    if let Some(getter_declaration) =
                        self.declaration_of_kind_identity(property_symbol, ast::KIND_GET_ACCESSOR)
                    {
                        let getter_declaration = Self::node_value(getter_declaration);
                        let mut getter_signature =
                            self.ch.get_signature_from_declaration(getter_declaration);
                        if let Some(symbol_mapper) = symbol_mapper {
                            getter_signature = self.ch.instantiate_signature_with_mapper_handle(
                                getter_signature,
                                symbol_mapper,
                            );
                        }
                        let getter = self.signature_to_signature_declaration_helper(
                            getter_signature,
                            ast::KIND_GET_ACCESSOR,
                            Some(SignatureToSignatureDeclarationOptions {
                                name: Some(property_name_node),
                                ..Default::default()
                            }),
                        );
                        self.set_comment_range(getter, Some(getter_declaration));
                        type_elements.push(getter);
                    }
                    if let Some(setter_declaration) =
                        self.declaration_of_kind_identity(property_symbol, ast::KIND_SET_ACCESSOR)
                    {
                        let setter_declaration = Self::node_value(setter_declaration);
                        let mut setter_signature =
                            self.ch.get_signature_from_declaration(setter_declaration);
                        if let Some(symbol_mapper) = symbol_mapper {
                            setter_signature = self.ch.instantiate_signature_with_mapper_handle(
                                setter_signature,
                                symbol_mapper,
                            );
                        }
                        let setter = self.signature_to_signature_declaration_helper(
                            setter_signature,
                            ast::KIND_SET_ACCESSOR,
                            Some(SignatureToSignatureDeclarationOptions {
                                name: Some(property_name_node),
                                ..Default::default()
                            }),
                        );
                        self.set_comment_range(setter, Some(setter_declaration));
                        type_elements.push(setter);
                    }
                    return type_elements;
                }
                if parent_is_class
                    && prop_declaration.is_some_and(|prop_declaration| {
                        let store = self.store_for_node(prop_declaration);
                        ast::is_auto_accessor_property_declaration(store, prop_declaration)
                    })
                {
                    let prop_declaration = prop_declaration.unwrap();
                    let fake_getter_signature = self.ch.new_signature(
                        SIGNATURE_FLAGS_NONE,
                        None, /*declaration*/
                        vec![],
                        None, /*this_parameter*/
                        vec![],
                        Some(property_type),
                        None, /*resolved_type_predicate*/
                        0,
                    );
                    let getter = self.signature_to_signature_declaration_helper(
                        fake_getter_signature,
                        ast::KIND_GET_ACCESSOR,
                        Some(SignatureToSignatureDeclarationOptions {
                            name: Some(property_name_node),
                            ..Default::default()
                        }),
                    );
                    self.set_comment_range(getter, Some(prop_declaration));
                    type_elements.push(getter);

                    let setter_param = self.ch.new_symbol(
                        ast::SYMBOL_FLAGS_FUNCTION_SCOPED_VARIABLE,
                        "arg".to_string(),
                    );
                    let setter_param = self.ch.transient_symbol_handle(setter_param);
                    let setter_param_identity = SymbolIdentity::from_symbol_handle(setter_param);
                    self.ch
                        .semantic_state
                        .set_value_symbol_resolved_type(setter_param_identity, Some(write_type));
                    let fake_setter_signature = self.ch.new_signature(
                        SIGNATURE_FLAGS_NONE,
                        None, /*declaration*/
                        vec![],
                        None, /*this_parameter*/
                        vec![setter_param_identity],
                        Some(self.ch.semantic_state.semantic_handles().void_type),
                        None, /*resolved_type_predicate*/
                        0,
                    );
                    type_elements.push(self.signature_to_signature_declaration_helper(
                        fake_setter_signature,
                        ast::KIND_SET_ACCESSOR,
                        Some(SignatureToSignatureDeclarationOptions {
                            name: Some(property_name_node),
                            ..Default::default()
                        }),
                    ));
                    return type_elements;
                }
            }
        }

        let optional_token = if property_flags & ast::SYMBOL_FLAGS_OPTIONAL != 0 {
            Some(Self::node_value(
                self.e
                    .factory
                    .node_factory
                    .new_token(ast::KIND_QUESTION_TOKEN),
            ))
        } else {
            None
        };
        if property_flags & (ast::SYMBOL_FLAGS_FUNCTION | ast::SYMBOL_FLAGS_METHOD) != 0
            && self
                .ch
                .get_properties_of_object_type(property_type)
                .is_empty()
            && !self.ch.is_readonly_symbol_identity(property_symbol)
        {
            let filtered_type = self
                .ch
                .filter_type_with_checker(property_type, |checker, t| {
                    checker.type_flags(t) & TYPE_FLAGS_UNDEFINED == 0
                });
            let signatures = self
                .ch
                .get_signatures_of_type(filtered_type, SIGNATURE_KIND_CALL);
            for signature in &signatures {
                let method_declaration = self.signature_to_signature_declaration_helper(
                    *signature,
                    ast::KIND_METHOD_SIGNATURE,
                    Some(SignatureToSignatureDeclarationOptions {
                        name: Some(property_name_node),
                        question_token: optional_token,
                        ..Default::default()
                    }),
                );
                self.set_comment_range(
                    method_declaration,
                    self.ch
                        .signature_record(*signature)
                        .declaration
                        .or(property_value_declaration),
                );
                type_elements.push(method_declaration);
            }
            if !signatures.is_empty() || optional_token.is_none() {
                return type_elements;
            }
        }
        let property_type_node =
            if self.should_use_placeholder_for_property_identity(property_symbol) {
                self.create_elided_information_placeholder()
            } else {
                if property_is_reverse_mapped {
                    self.ctx.reverse_mapped_stack.push(property_symbol);
                }
                let property_type_node = if self.ch.type_flags(property_type) != 0 {
                    self.serialize_type_for_declaration_for_symbol_identity(
                        None, /*declaration*/
                        Some(property_type),
                        Some(property_symbol),
                        true,
                    )
                } else {
                    Self::node_value(
                        self.e
                            .factory
                            .node_factory
                            .new_keyword_type_node(ast::KIND_ANY_KEYWORD),
                    )
                };
                if property_is_reverse_mapped {
                    self.ctx.reverse_mapped_stack.pop();
                }
                property_type_node
            };

        let mut modifiers = None;
        if self.ch.is_readonly_symbol_identity(property_symbol) {
            let readonly_modifier = self
                .e
                .factory
                .node_factory
                .new_modifier(ast::KIND_READONLY_KEYWORD);
            modifiers = Some(self.e.factory.node_factory.new_modifier_list(
                core::new_text_range(-1, -1),
                core::new_text_range(-1, -1),
                vec![readonly_modifier],
                ast::MODIFIER_FLAGS_READONLY,
            ));
            self.ctx.approximate_length += 9;
        }
        let property_signature = self
            .e
            .factory
            .node_factory
            .new_property_signature_declaration(
                modifiers,
                property_name_node.clone(),
                optional_token,
                Some(property_type_node.clone()),
                None,
            );

        let property_signature = Self::node_value(property_signature);
        self.set_comment_range(property_signature, property_value_declaration);
        type_elements.push(property_signature);

        type_elements
    }

    fn create_anonymous_type_node(&mut self, t: TypeHandle) -> ast::Node {
        self.create_anonymous_type_node_ex(t, false, false)
    }

    fn type_to_type_node_or_circularity_elision(&mut self, t: TypeHandle) -> ast::Node {
        if self.ch.type_flags(t) & TYPE_FLAGS_UNION != 0 {
            if self.ctx.visited_types.has(&self.ch.type_id(t)) {
                if self.ctx.flags & nodebuilder::FLAGS_ALLOW_ANONYMOUS_IDENTIFIER == 0 {
                    self.ctx.encountered_error = true;
                    self.report_cyclic_structure_error();
                }
                return self.create_elided_information_placeholder();
            }
            return self
                .visit_and_transform_type(t, |builder, t| builder.type_to_type_node(t).unwrap());
        }
        self.type_to_type_node(t).unwrap()
    }

    fn create_mapped_type_node_from_type(&mut self, t: TypeHandle) -> ast::Node {
        debug_assert!(self.ch.type_flags(t) & TYPE_FLAGS_OBJECT != 0);
        let mapped = self.ch.type_record(t).as_mapped_type().clone();
        let mut readonly_token: Option<ast::Node> = None;
        let mapped_declaration = mapped.declaration.unwrap();
        let (
            mapped_readonly_token,
            mapped_question_token,
            mapped_type_parameter_node,
            mapped_declaration_has_name_type,
        ) = {
            let mapped_declaration_store = self.store_for_node(mapped_declaration);
            (
                mapped_declaration_store.readonly_token(mapped_declaration),
                mapped_declaration_store.question_token(mapped_declaration),
                mapped_declaration_store.type_parameter(mapped_declaration),
                mapped_declaration_store
                    .name_type(mapped_declaration)
                    .is_some(),
            )
        };
        let (mapped_readonly_kind, mapped_question_kind, mapped_type_parameter_node) = {
            let mapped_declaration_store = self.store_for_node(mapped_declaration);
            (
                mapped_readonly_token.map(|token| mapped_declaration_store.kind(token)),
                mapped_question_token.map(|token| mapped_declaration_store.kind(token)),
                mapped_type_parameter_node,
            )
        };
        if let Some(existing_readonly_kind) = mapped_readonly_kind {
            readonly_token = Some(
                self.e
                    .factory
                    .node_factory
                    .new_token(existing_readonly_kind),
            );
        }
        let mut question_token: Option<ast::Node> = None;
        if let Some(question_token_kind) = mapped_question_kind {
            question_token = Some(self.e.factory.node_factory.new_token(question_token_kind));
        }
        let mut appropriate_constraint_type_node = None;
        let mut new_type_variable = None;
        let mut template_type = self.ch.get_template_type_from_mapped_type(t);
        let type_parameter = self.ch.get_type_parameter_from_mapped_type(t);

        // If the mapped type isn't `keyof` constraint-declared, _but_ still has modifiers preserved, and its naive instantiation won't preserve modifiers because its constraint isn't `keyof` constrained, we have work to do
        let modifiers_type = self.ch.get_modifiers_type_from_mapped_type(t);
        let needs_modifier_preserving_wrapper =
            !self.ch.is_mapped_type_with_keyof_constraint_declaration(t)
                && self.ch.type_flags(modifiers_type) & TYPE_FLAGS_UNKNOWN == 0
                && self.ctx.flags & nodebuilder::FLAGS_GENERATE_NAMES_FOR_SHADOWED_TYPE_PARAMS != 0
                && !({
                    let constraint_type = self.ch.get_constraint_type_from_mapped_type(t);
                    self.ch.type_flags(constraint_type) & TYPE_FLAGS_TYPE_PARAMETER != 0 && {
                        let constraint = self
                            .ch
                            .get_constraint_of_type_parameter(constraint_type)
                            .unwrap();
                        self.ch.type_flags(constraint) & TYPE_FLAGS_INDEX != 0
                    }
                });

        if self.ch.is_mapped_type_with_keyof_constraint_declaration(t) {
            // We have a { [P in keyof T]: X }
            // We do this to ensure we retain the toplevel keyof-ness of the type which may be lost due to keyof distribution during `getConstraintTypeFromMappedType`
            if self.ctx.flags & nodebuilder::FLAGS_GENERATE_NAMES_FOR_SHADOWED_TYPE_PARAMS != 0
                && self.is_homomorphic_mapped_type_with_non_homomorphic_instantiation(t)
            {
                let new_constraint_symbol = self
                    .ch
                    .new_symbol(ast::SYMBOL_FLAGS_TYPE_PARAMETER, "T".to_string());
                let new_constraint_symbol = self.ch.transient_symbol_handle(new_constraint_symbol);
                let new_constraint_param = self.ch.new_type_parameter_from_identity(Some(
                    SymbolIdentity::from_symbol_handle(new_constraint_symbol),
                ));
                let name = self.type_parameter_to_name(new_constraint_param);
                let target = self.ch.type_target(t);
                new_type_variable = Some(Self::node_value(
                    self.e
                        .factory
                        .node_factory
                        .new_type_reference_node(name, None),
                ));
                let type_parameter_from_mapped =
                    self.ch.get_type_parameter_from_mapped_type(target);
                let modifiers_type_from_mapped =
                    self.ch.get_modifiers_type_from_mapped_type(target);
                let mapper = self.ch.new_type_mapper_handle(
                    [type_parameter_from_mapped, modifiers_type_from_mapped],
                    [type_parameter, new_constraint_param],
                );
                let target_template_type = self.ch.get_template_type_from_mapped_type(target);
                template_type = self
                    .ch
                    .instantiate_type_with_mapper_handle(Some(target_template_type), Some(mapper))
                    .unwrap();
            }
            let index_target = if let Some(new_type_variable) = new_type_variable {
                new_type_variable
            } else {
                {
                    let modifiers_type = self.ch.get_modifiers_type_from_mapped_type(t);
                    self.type_to_type_node(modifiers_type).unwrap()
                }
            };
            let index_target = self.ensure_factory_node(index_target);
            appropriate_constraint_type_node = Some(Self::node_value(
                self.e
                    .factory
                    .node_factory
                    .new_type_operator_node(ast::KIND_KEY_OF_KEYWORD, index_target),
            ));
        } else if needs_modifier_preserving_wrapper {
            // So, step 1: new type variable
            let new_param_symbol = self
                .ch
                .new_symbol(ast::SYMBOL_FLAGS_TYPE_PARAMETER, "T".to_string());
            let new_param_symbol = self.ch.transient_symbol_handle(new_param_symbol);
            let new_param =
                self.ch
                    .new_type_parameter_from_identity(Some(SymbolIdentity::from_symbol_handle(
                        new_param_symbol,
                    )));
            let name = self.type_parameter_to_name(new_param);
            new_type_variable = Some(Self::node_value(
                self.e
                    .factory
                    .node_factory
                    .new_type_reference_node(name, None),
            ));
            // step 2: make that new type variable itself the constraint node, making the mapped type `{[K in T_1]: Template}`
            appropriate_constraint_type_node = new_type_variable;
        } else {
            let constraint_type = self.ch.get_constraint_type_from_mapped_type(t);
            appropriate_constraint_type_node = self.type_to_type_node(constraint_type);
        }

        // nameType and templateType nodes have to be in the new scope
        let cleanup = self.enter_new_scope(
            Some(mapped_declaration),
            None,
            vec![type_parameter],
            None,
            None,
        );
        let type_parameter_declaration_node = self.type_parameter_to_declaration_with_constraint(
            type_parameter,
            appropriate_constraint_type_node,
        );
        let mut name_type_node = None;
        if mapped_declaration_has_name_type {
            name_type_node = {
                let name_type = self.ch.get_name_type_from_mapped_type(t).unwrap();
                self.type_to_type_node(name_type)
            };
        }
        let remove_missing_type = self.ch.remove_missing_type(
            template_type,
            self.ch.get_mapped_type_modifiers(t) & MAPPED_TYPE_MODIFIERS_INCLUDE_OPTIONAL != 0,
        );
        let template_type_node = self.type_to_type_node(remove_missing_type);
        self.exit_scope(cleanup);
        let result = self.e.factory.node_factory.new_mapped_type_node(
            readonly_token,
            type_parameter_declaration_node.clone(),
            name_type_node,
            question_token,
            template_type_node,
            None,
        );
        let result = Self::node_value(result);
        self.ctx.approximate_length += 10;
        self.e.mark_emit_node(&result, printer::EF_SINGLE_LINE);

        if self.ctx.flags & nodebuilder::FLAGS_GENERATE_NAMES_FOR_SHADOWED_TYPE_PARAMS != 0
            && self.is_homomorphic_mapped_type_with_non_homomorphic_instantiation(t)
        {
            // homomorphic mapped type with a non-homomorphic naive inlining
            // wrap it with a conditional like `SomeModifiersType extends infer U ? {..the mapped type...} : never` to ensure the resulting
            // type stays homomorphic
            let mapped_declaration = mapped.declaration.unwrap();
            let mapped_declaration_store = self.store_for_node(mapped_declaration);
            let mapped_type_parameter_node = mapped_declaration_store
                .type_parameter(mapped_declaration)
                .unwrap();
            let mapped_type_parameter_store = self.store_for_node(mapped_type_parameter_node);
            let mapped_constraint_type_node = mapped_type_parameter_store
                .constraint(mapped_type_parameter_node)
                .unwrap();
            let mapped_constraint_type_node = mapped_type_parameter_store
                .r#type(mapped_constraint_type_node)
                .unwrap();
            let mut raw_constraint_type_from_declaration =
                self.get_type_from_type_node(mapped_constraint_type_node, false);
            if let Some(raw) = raw_constraint_type_from_declaration {
                raw_constraint_type_from_declaration =
                    self.ch.get_constraint_of_type_parameter(raw);
            }
            let raw_constraint_type_from_declaration = raw_constraint_type_from_declaration
                .unwrap_or(self.ch.semantic_state.semantic_handles().unknown_type);
            let original_constraint = self
                .ch
                .instantiate_type_with_mapper_handle(
                    Some(raw_constraint_type_from_declaration),
                    Some(self.ch.type_mapper_handle(t)),
                )
                .unwrap();

            let mut original_constraint_node = None;
            if self.ch.type_flags(original_constraint) & TYPE_FLAGS_UNKNOWN == 0 {
                original_constraint_node = self.type_to_type_node(original_constraint);
            }

            let modifiers_type = self.ch.get_modifiers_type_from_mapped_type(t);
            let modifiers_type_node = self.type_to_type_node(modifiers_type).unwrap().clone();
            let infer_name = {
                let new_type_variable = new_type_variable.unwrap();
                let store = self.store_for_node(new_type_variable);
                store.type_name(new_type_variable).unwrap()
            };
            let type_parameter = self.e.factory.node_factory.new_type_parameter_declaration(
                None,
                infer_name,
                original_constraint_node,
                None,
                None,
            );
            let infer_type = self
                .e
                .factory
                .node_factory
                .new_infer_type_node(type_parameter);
            let never_type = self
                .e
                .factory
                .node_factory
                .new_keyword_type_node(ast::KIND_NEVER_KEYWORD);
            let conditional = self.e.factory.node_factory.new_conditional_type_node(
                modifiers_type_node,
                infer_type,
                result,
                never_type,
            );
            return Self::node_value(conditional);
        } else if needs_modifier_preserving_wrapper {
            // and step 3: once the mapped type is reconstructed, create a `ConstraintType extends infer T_1 extends keyof ModifiersType ? {[K in T_1]: Template} : never`
            // subtly different from the `keyof` constraint case, by including the `keyof` constraint on the `infer` type parameter, it doesn't rely on the constraint type being itself
            // constrained to a `keyof` type to preserve its modifier-preserving behavior. This is all basically because we preserve modifiers for a wider set of mapped types than
            // just homomorphic ones.
            let modifiers_type = self.ch.get_modifiers_type_from_mapped_type(t);
            let modifiers_type_node = self.type_to_type_node(modifiers_type).unwrap().clone();
            let keyof_modifiers_type_node = self
                .e
                .factory
                .node_factory
                .new_type_operator_node(ast::KIND_KEY_OF_KEYWORD, modifiers_type_node);
            let keyof_modifiers_type = Self::node_value(keyof_modifiers_type_node);
            let constraint_type = self.ch.get_constraint_type_from_mapped_type(t);
            let constraint_type_node = self.type_to_type_node(constraint_type).unwrap().clone();
            let infer_name = {
                let new_type_variable = new_type_variable.unwrap();
                let store = self.store_for_node(new_type_variable);
                store.type_name(new_type_variable).unwrap()
            };
            let type_parameter = self.e.factory.node_factory.new_type_parameter_declaration(
                None,
                infer_name,
                Some(keyof_modifiers_type),
                None,
                None,
            );
            let infer_type = self
                .e
                .factory
                .node_factory
                .new_infer_type_node(type_parameter);
            let never_type = self
                .e
                .factory
                .node_factory
                .new_keyword_type_node(ast::KIND_NEVER_KEYWORD);
            let conditional = self.e.factory.node_factory.new_conditional_type_node(
                constraint_type_node,
                infer_type,
                result,
                never_type,
            );
            return Self::node_value(conditional);
        }

        result
    }

    fn create_type_nodes_from_resolved_type(&mut self, t: TypeHandle) -> Option<ast::NodeList> {
        if self.check_truncation_length() {
            if self.ctx.flags & nodebuilder::FLAGS_NO_TRUNCATION != 0 {
                let elem = self.e.factory.node_factory.new_not_emitted_type_element();
                self.e.add_synthetic_trailing_comment(
                    &elem,
                    ast::KIND_MULTI_LINE_COMMENT_TRIVIA,
                    "elided".to_string(),
                    false, /*hasTrailingNewLine*/
                );
                return Some(Self::output_node_list_value(
                    self.new_factory_node_list(vec![elem]),
                ));
            }
            let property_name = self.e.factory.node_factory.new_identifier("...");
            let property = self
                .e
                .factory
                .node_factory
                .new_property_signature_declaration(None, property_name, None, None, None);
            return Some(Self::output_node_list_value(
                self.new_factory_node_list(vec![property]),
            ));
        }
        self.ctx.type_stack.push(None);
        let mut type_elements = Vec::new();
        let (call_signature_count, signature_count, index_info_count, property_count) = {
            let resolved_type = self.ch.structured_type_record(t);
            (
                resolved_type.call_signature_count,
                resolved_type.signatures.len(),
                resolved_type.index_infos.len(),
                resolved_type.properties.len(),
            )
        };
        for signature_index in 0..call_signature_count {
            let signature = self.ch.structured_type_record(t).signatures[signature_index];
            type_elements.push(self.signature_to_signature_declaration_helper(
                signature,
                ast::KIND_CALL_SIGNATURE,
                None,
            ));
        }
        for signature_index in call_signature_count..signature_count {
            let signature = self.ch.structured_type_record(t).signatures[signature_index];
            if self.ch.signature_record(signature).flags & SIGNATURE_FLAGS_ABSTRACT != 0 {
                continue;
            }
            type_elements.push(self.signature_to_signature_declaration_helper(
                signature,
                ast::KIND_CONSTRUCT_SIGNATURE,
                None,
            ));
        }
        for index_info_index in 0..index_info_count {
            let info = self.ch.structured_type_record(t).index_infos[index_info_index];
            let type_node = if self.ch.object_flags(t) & OBJECT_FLAGS_REVERSE_MAPPED != 0 {
                Some(self.create_elided_information_placeholder())
            } else {
                None
            };
            type_elements
                .push(self.index_info_to_index_signature_declaration_helper(info, type_node));
        }

        if property_count == 0 {
            let result = Some(Self::output_node_list_value(
                self.new_factory_node_list(type_elements.into_iter().collect::<Vec<_>>()),
            ));
            self.ctx.type_stack.pop();
            return result;
        }

        let mut i = 0;
        for property_index in 0..property_count {
            let property_identity = self.ch.structured_type_record(t).properties[property_index];
            let property_flags = self.symbol_identity_flags(property_identity);
            if is_expanding(&self.ctx) && property_flags & ast::SYMBOL_FLAGS_PROTOTYPE != 0 {
                continue;
            }
            i += 1;
            if self.ctx.flags & nodebuilder::FLAGS_WRITE_CLASS_EXPRESSION_AS_TYPE_LITERAL != 0 {
                if property_flags & ast::SYMBOL_FLAGS_PROTOTYPE != 0 {
                    continue;
                }
                if self
                    .ch
                    .declaration_modifier_flags_from_symbol_identity(property_identity)
                    & (ast::MODIFIER_FLAGS_PRIVATE | ast::MODIFIER_FLAGS_PROTECTED)
                    != 0
                {
                    let name = self.ch.missing_name_symbol_identity_name(property_identity);
                    self.report_private_in_base_of_class_expression(&name);
                }
                if is_private_identifier_symbol_identity(self.ch, property_identity) {
                    let name = self.symbol_identity_display_name(property_identity);
                    self.report_private_in_base_of_class_expression(&name);
                }
            }
            if self.check_truncation_length() && i + 2 < property_count - 1 {
                if self.ctx.flags & nodebuilder::FLAGS_NO_TRUNCATION != 0 {
                    let last = type_elements.len() - 1;
                    self.e.add_synthetic_trailing_comment(
                        &type_elements[last],
                        ast::KIND_MULTI_LINE_COMMENT_TRIVIA,
                        format!("... {} more elided ...", property_count - i),
                        false, /*hasTrailingNewLine*/
                    );
                } else {
                    let text = format!("... {} more ...", property_count - i);
                    let name = self.e.factory.node_factory.new_identifier(text);
                    let property = self
                        .e
                        .factory
                        .node_factory
                        .new_property_signature_declaration(None, name, None, None, None);
                    type_elements.push(Self::node_value(property));
                }
                let last_property_identity =
                    self.ch.structured_type_record(t).properties[property_count - 1];
                type_elements = self
                    .add_property_identity_to_element_list(last_property_identity, type_elements);
                break;
            }
            type_elements =
                self.add_property_identity_to_element_list(property_identity, type_elements);
        }
        if !type_elements.is_empty() {
            let result = Some(Self::output_node_list_value(
                self.new_factory_node_list(type_elements.into_iter().collect::<Vec<_>>()),
            ));
            self.ctx.type_stack.pop();
            result
        } else {
            self.ctx.type_stack.pop();
            None
        }
    }

    fn create_type_node_from_object_type(&mut self, t: TypeHandle) -> ast::Node {
        if self.ch.is_generic_mapped_type(t)
            || (self.ch.object_flags(t) & OBJECT_FLAGS_MAPPED != 0
                && self.ch.type_record(t).as_mapped_type().contains_error)
        {
            return self.create_mapped_type_node_from_type(t);
        }

        self.ch.resolve_structured_type_members(t);
        let (property_count, index_info_count, call_signature_count, signature_count) = {
            let resolved = self.ch.structured_type_record(t);
            (
                resolved.properties.len(),
                resolved.index_infos.len(),
                resolved.call_signature_count,
                resolved.signatures.len(),
            )
        };
        let construct_signature_count = signature_count - call_signature_count;
        if property_count == 0 && index_info_count == 0 {
            if call_signature_count == 0 && construct_signature_count == 0 {
                self.ctx.approximate_length += 2;
                let members = self.new_factory_node_list(Vec::new());
                let result = self.e.factory.node_factory.new_type_literal_node(members);
                let result = Self::node_value(result);
                self.e.set_emit_flags(&result, printer::EF_SINGLE_LINE);
                return result;
            }

            if call_signature_count == 1 && construct_signature_count == 0 {
                let signature = self.ch.structured_type_record(t).signatures[0];
                return self.signature_to_signature_declaration_helper(
                    signature,
                    ast::KIND_FUNCTION_TYPE,
                    None,
                );
            }

            if construct_signature_count == 1 && call_signature_count == 0 {
                let signature = self.ch.structured_type_record(t).signatures[call_signature_count];
                return self.signature_to_signature_declaration_helper(
                    signature,
                    ast::KIND_CONSTRUCTOR_TYPE,
                    None,
                );
            }
        }

        let abstract_signatures = (call_signature_count..signature_count)
            .map(|index| self.ch.structured_type_record(t).signatures[index])
            .filter(|signature| {
                self.ch.signature_record(*signature).flags & SIGNATURE_FLAGS_ABSTRACT != 0
            })
            .collect::<Vec<_>>();
        if !abstract_signatures.is_empty() {
            let mut types = abstract_signatures
                .iter()
                .map(|s| self.ch.get_or_create_type_from_signature(*s))
                .collect::<Vec<_>>();
            let type_element_count = call_signature_count
                + (construct_signature_count - abstract_signatures.len())
                + index_info_count
                + if self.ctx.flags & nodebuilder::FLAGS_WRITE_CLASS_EXPRESSION_AS_TYPE_LITERAL != 0
                {
                    let mut count = 0;
                    for property_index in 0..property_count {
                        let property_identity =
                            self.ch.structured_type_record(t).properties[property_index];
                        if self
                            .ch
                            .missing_name_symbol_identity_flags(property_identity)
                            & ast::SYMBOL_FLAGS_PROTOTYPE
                            == 0
                        {
                            count += 1;
                        }
                    }
                    count
                } else {
                    property_count
                };
            if type_element_count != 0 {
                types.push(self.get_resolved_type_without_abstract_construct_signatures(t));
            }
            let intersection_type = self.ch.get_intersection_type(types);
            return self.type_to_type_node(intersection_type).unwrap();
        }

        let restore_flags = self.save_restore_flags();
        self.ctx.flags |= nodebuilder::FLAGS_IN_OBJECT_TYPE_LITERAL;
        let members = self.create_type_nodes_from_resolved_type(t);
        restore_flags(self);
        let members = members.unwrap_or_else(|| self.new_factory_node_list(Vec::new()));
        let type_literal_node = self.e.factory.node_factory.new_type_literal_node(members);
        let type_literal_node = Self::node_value(type_literal_node);
        self.ctx.approximate_length += 2;
        self.e.set_emit_flags(
            &type_literal_node,
            if self.ctx.flags & nodebuilder::FLAGS_MULTILINE_OBJECT_LITERALS != 0 {
                0
            } else {
                printer::EF_SINGLE_LINE
            },
        );
        type_literal_node
    }

    fn should_write_type_of_function_symbol(
        &mut self,
        symbol: SymbolIdentity,
        type_id: TypeId,
    ) -> bool {
        let symbol_flags = self.symbol_identity_flags(symbol);
        let symbol_declarations = self.ch.collect_symbol_identity_declarations(symbol);
        let is_static_method_symbol = symbol_flags & ast::SYMBOL_FLAGS_METHOD != 0
            && symbol_declarations.iter().any(|declaration| {
                let store = self.store_for_node(*declaration);
                if !ast::is_static(store, *declaration) {
                    return false;
                }
                let Some(name) = ast::get_name_of_declaration(store, Some(*declaration)) else {
                    return true;
                };
                !self
                    .ch
                    .is_late_bindable_index_signature(Self::node_value(name))
            });
        let mut is_non_local_function_symbol = false;
        if symbol_flags & ast::SYMBOL_FLAGS_FUNCTION != 0 {
            if self.ch.symbol_identity_parent(symbol).is_some() {
                is_non_local_function_symbol = true;
            } else {
                for declaration in &symbol_declarations {
                    let store = self.store_for_node(*declaration);
                    let Some(parent) = store.parent(*declaration) else {
                        continue;
                    };
                    if store.kind(parent) == ast::KIND_SOURCE_FILE
                        || store.kind(parent) == ast::KIND_MODULE_BLOCK
                    {
                        is_non_local_function_symbol = true;
                        break;
                    }
                }
            }
        }
        if is_static_method_symbol || is_non_local_function_symbol {
            // typeof is allowed only for static/non local functions
            return (self.ctx.flags & nodebuilder::FLAGS_USE_TYPE_OF_FUNCTION != 0
                || self.ctx.visited_types.has(&type_id))
                && (self.ctx.flags & nodebuilder::FLAGS_USE_STRUCTURAL_FALLBACK == 0
                    || self
                        .is_symbol_accessible_in_builder_scope_by_identity(
                            Some(symbol),
                            self.ctx.enclosing_declaration,
                            ast::SYMBOL_FLAGS_VALUE,
                            false,
                        )
                        .accessibility
                        == printer::SYMBOL_ACCESSIBILITY_ACCESSIBLE);
        }
        false
    }

    fn create_anonymous_type_node_ex(
        &mut self,
        t: TypeHandle,
        force_class_expansion: bool,
        force_expansion: bool,
    ) -> ast::Node {
        let type_id = self.ch.type_id(t);
        let symbol = self.ch.type_symbol(t);
        if let Some(symbol) = symbol {
            let is_instantiation_expression_type =
                self.ch.object_flags(t) & OBJECT_FLAGS_INSTANTIATION_EXPRESSION_TYPE != 0;
            if is_instantiation_expression_type {
                let instantiation_expression_type =
                    self.ch.type_record(t).as_instantiation_expression_type();
                let existing = instantiation_expression_type.node.unwrap();
                if ast::is_type_query_node(self.store_for_node(existing), existing)
                    && self.get_type_from_type_node(existing, false) == Some(t)
                {
                    if let Some(type_node) =
                        self.try_reuse_existing_non_parameter_type_node(existing, t, None, None)
                    {
                        return type_node;
                    }
                }
                if self.ctx.visited_types.has(&type_id) {
                    return self.create_elided_information_placeholder();
                }
                return self.visit_and_transform_type(
                    t,
                    NodeBuilderImpl::create_type_node_from_object_type,
                );
            }
            let is_instance_type = if is_class_instance_side(self.ch, t) {
                ast::SYMBOL_FLAGS_TYPE
            } else {
                ast::SYMBOL_FLAGS_VALUE
            };

            let (
                symbol_flags,
                has_class_like_value_declaration,
                is_class_declaration_value,
                should_write_type_of_function_symbol,
            ) = {
                let value_declaration = self
                    .ch
                    .missing_name_symbol_identity_value_declaration(symbol);
                (
                    self.symbol_identity_flags(symbol),
                    value_declaration.is_some_and(|declaration| {
                        ast::is_class_like(self.store_for_node(declaration), declaration)
                    }),
                    value_declaration.is_some_and(|declaration| {
                        ast::is_class_declaration(self.store_for_node(declaration), declaration)
                    }),
                    self.should_write_type_of_function_symbol(symbol, type_id),
                )
            };
            let class_symbol_without_base_type_variable = symbol_flags & ast::SYMBOL_FLAGS_CLASS
                != 0
                && !force_class_expansion
                && self
                    .get_base_type_variable_of_class_identity(symbol)
                    .is_none();
            if !force_expansion
                && (class_symbol_without_base_type_variable
                    && !(has_class_like_value_declaration
                        && self.ctx.flags
                            & nodebuilder::FLAGS_WRITE_CLASS_EXPRESSION_AS_TYPE_LITERAL
                            != 0
                        && (!is_class_declaration_value
                            || self
                                .is_symbol_accessible_in_builder_scope_by_identity(
                                    Some(symbol),
                                    self.ctx.enclosing_declaration,
                                    is_instance_type,
                                    false,
                                )
                                .accessibility
                                != printer::SYMBOL_ACCESSIBILITY_ACCESSIBLE))
                    || symbol_flags & ast::SYMBOL_FLAGS_ENUM != 0
                    || symbol_flags & ast::SYMBOL_FLAGS_VALUE_MODULE != 0
                    || should_write_type_of_function_symbol)
            {
                if self.should_expand_type(t, false /*isAlias*/) {
                    self.ctx.depth += 1;
                } else {
                    return self
                        .symbol_identity_to_type_node(symbol, is_instance_type, None)
                        .unwrap();
                }
            }
            if self.ctx.visited_types.has(&type_id) {
                // If type is an anonymous type literal in a type alias declaration, use type alias name
                if let Some(type_alias) = get_type_alias_for_type_literal(self.ch, t) {
                    return self
                        .symbol_identity_to_type_node(type_alias, ast::SYMBOL_FLAGS_TYPE, None)
                        .unwrap();
                }
                return self.create_elided_information_placeholder();
            }
            return self
                .visit_and_transform_type(t, NodeBuilderImpl::create_type_node_from_object_type);
        }
        // Anonymous types without a symbol are never circular.
        self.create_type_node_from_object_type(t)
    }

    fn conditional_type_to_type_node(&mut self, t_: TypeHandle) -> ast::Node {
        if self.check_truncation_length() {
            return self.create_elided_information_placeholder();
        }
        let t = self.ch.type_record(t_).as_conditional_type().clone();
        let root = self
            .ch
            .semantic_state
            .conditional_root_record(t.root.unwrap())
            .clone();
        let check_type = t.check_type.unwrap();
        let check_type_node = self.type_to_type_node(check_type).unwrap();
        self.ctx.approximate_length += 15;
        if self.ctx.flags & nodebuilder::FLAGS_GENERATE_NAMES_FOR_SHADOWED_TYPE_PARAMS != 0
            && root.is_distributive
            && self.ch.type_flags(check_type) & TYPE_FLAGS_TYPE_PARAMETER == 0
        {
            let new_param_symbol = self.ch.new_symbol(
                ast::SYMBOL_FLAGS_TYPE_PARAMETER,
                "T".to_string(), /* as __String */
            );
            let new_param_symbol = self.ch.transient_symbol_handle(new_param_symbol);
            let new_param =
                self.ch
                    .new_type_parameter_from_identity(Some(SymbolIdentity::from_symbol_handle(
                        new_param_symbol,
                    )));
            let name = self.type_parameter_to_name(new_param);
            let new_type_variable = self
                .e
                .factory
                .node_factory
                .new_type_reference_node(name, None);
            self.ctx.approximate_length += 37;
            // 15 each for two added conditionals, 7 for an added infer type
            let new_mapper =
                self.ch
                    .prepend_type_mapping_handle(root.check_type.unwrap(), new_param, t.mapper);
            let save_infer_type_parameters = self.ctx.infer_type_parameters.clone();
            self.ctx.infer_type_parameters = root.infer_type_parameters.clone();
            let extends_type = self
                .ch
                .instantiate_type_with_mapper_handle(root.extends_type, Some(new_mapper))
                .unwrap();
            let extends_type_node = self.type_to_type_node(extends_type).unwrap();
            self.ctx.infer_type_parameters = save_infer_type_parameters;
            let root_node = root.node.unwrap();
            let (root_true_type_node, root_false_type_node) = {
                let root_store = self.store_for_node(root_node);
                (
                    root_store.true_type(root_node).unwrap(),
                    root_store.false_type(root_node).unwrap(),
                )
            };
            let true_type = self
                .get_type_from_type_node(Self::node_value(root_true_type_node), false)
                .unwrap();
            let true_type = self
                .ch
                .instantiate_type_with_mapper_handle(Some(true_type), Some(new_mapper));
            let true_type_node = self.type_to_type_node_or_circularity_elision(true_type.unwrap());
            let false_type = self
                .get_type_from_type_node(Self::node_value(root_false_type_node), false)
                .unwrap();
            let false_type = self
                .ch
                .instantiate_type_with_mapper_handle(Some(false_type), Some(new_mapper));
            let false_type_node =
                self.type_to_type_node_or_circularity_elision(false_type.unwrap());

            // outermost conditional makes `T` a type parameter, allowing the inner conditionals to be distributive
            // second conditional makes `T` have `T & checkType` substitution, so it is correctly usable as the checkType
            // inner conditional runs the check the user provided on the check type (distributively) and returns the result
            // checkType extends infer T ? T extends checkType ? T extends extendsType<T> ? trueType<T> : falseType<T> : never : never;
            // this is potentially simplifiable to
            // checkType extends infer T ? T extends checkType & extendsType<T> ? trueType<T> : falseType<T> : never;
            // but that may confuse users who read the output more.
            // On the other hand,
            // checkType extends infer T extends checkType ? T extends extendsType<T> ? trueType<T> : falseType<T> : never;
            // may also work with `infer ... extends ...` in, but would produce declarations only compatible with the latest TS.
            let new_id = {
                self.e
                    .factory
                    .node_factory
                    .store()
                    .type_name(new_type_variable)
                    .unwrap()
            };
            let synthetic_type_parameter = self
                .e
                .factory
                .node_factory
                .new_type_parameter_declaration(None, new_id, None, None, None);
            let outer_extends_node = self
                .e
                .factory
                .node_factory
                .new_infer_type_node(synthetic_type_parameter);
            let inner_check_conditional_node =
                self.e.factory.node_factory.new_conditional_type_node(
                    new_type_variable,
                    extends_type_node.clone(),
                    true_type_node.clone(),
                    false_type_node.clone(),
                );
            let inner_extends_node = self.type_to_type_node(check_type).unwrap();
            let synthetic_true_check = self
                .e
                .factory
                .node_factory
                .new_type_reference_node(name, None);
            let synthetic_never_type = self
                .e
                .factory
                .node_factory
                .new_keyword_type_node(ast::KIND_NEVER_KEYWORD);
            let synthetic_true_node = self.e.factory.node_factory.new_conditional_type_node(
                synthetic_true_check,
                inner_extends_node,
                inner_check_conditional_node,
                synthetic_never_type,
            );
            let result_never_type = self
                .e
                .factory
                .node_factory
                .new_keyword_type_node(ast::KIND_NEVER_KEYWORD);
            let result = self.e.factory.node_factory.new_conditional_type_node(
                check_type_node.clone(),
                outer_extends_node,
                synthetic_true_node,
                result_never_type,
            );
            return Self::node_value(result);
        }
        let save_infer_type_parameters = self.ctx.infer_type_parameters.clone();
        self.ctx.infer_type_parameters = root.infer_type_parameters.clone();
        let extends_type_node = self.type_to_type_node(t.extends_type.unwrap()).unwrap();
        self.ctx.infer_type_parameters = save_infer_type_parameters;
        let true_type = self.ch.get_true_type_from_conditional_type(t_);
        let true_type_node = self.type_to_type_node_or_circularity_elision(true_type);
        let false_type = self.ch.get_false_type_from_conditional_type(t_);
        let false_type_node = self.type_to_type_node_or_circularity_elision(false_type);
        let result = self.e.factory.node_factory.new_conditional_type_node(
            check_type_node.clone(),
            extends_type_node.clone(),
            true_type_node.clone(),
            false_type_node.clone(),
        );
        Self::node_value(result)
    }

    pub(crate) fn serialize_type_for_declaration(
        &mut self,
        declaration: Option<ast::Node>,
        t: Option<TypeHandle>,
        symbol: Option<SymbolIdentity>,
        try_reuse: bool,
    ) -> ast::Node {
        self.serialize_type_for_declaration_for_symbol_identity_core(
            declaration,
            t,
            symbol,
            try_reuse,
        )
    }

    pub(crate) fn serialize_type_for_declaration_for_symbol_identity(
        &mut self,
        declaration: Option<ast::Node>,
        t: Option<TypeHandle>,
        symbol: Option<SymbolIdentity>,
        try_reuse: bool,
    ) -> ast::Node {
        self.serialize_type_for_declaration_for_symbol_identity_core(
            declaration,
            t,
            symbol,
            try_reuse,
        )
    }

    fn serialize_type_for_declaration_for_symbol_identity_core(
        &mut self,
        mut declaration: Option<ast::Node>,
        mut t: Option<TypeHandle>,
        symbol_identity: Option<SymbolIdentity>,
        try_reuse: bool,
    ) -> ast::Node {
        if declaration.is_none() {
            if let Some(symbol_identity) = symbol_identity {
                declaration = self
                    .ch
                    .missing_name_symbol_identity_value_declaration(symbol_identity)
                    .or_else(|| {
                        self.ch
                            .collect_symbol_identity_declarations(symbol_identity)
                            .first()
                            .copied()
                    });
            }
        }
        if declaration.is_some_and(|declaration| self.try_store_for_node(declaration).is_none()) {
            declaration = None;
        }
        let declaration_symbol_handle = if symbol_identity.is_none() {
            declaration.and_then(|declaration| self.ch.get_symbol_of_declaration(declaration))
        } else {
            None
        };
        if t.is_none() {
            let resolved_symbol_identity = symbol_identity
                .or_else(|| declaration_symbol_handle.map(SymbolIdentity::from_symbol_handle))
                .expect("declaration serialization requires a symbol when type is omitted");
            t = self
                .ctx
                .enclosing_symbol_types
                .get(&resolved_symbol_identity)
                .copied();
            if t.is_none() {
                let symbol_flags = symbol_identity
                    .map(|symbol| self.ch.symbol_identity_flags(symbol))
                    .or_else(|| {
                        declaration_symbol_handle.map(|symbol| self.ch.symbol_handle_flags(symbol))
                    })
                    .unwrap_or(ast::SYMBOL_FLAGS_NONE);
                if symbol_flags & ast::SYMBOL_FLAGS_ACCESSOR != 0
                    && self
                        .store_for_node(declaration.unwrap())
                        .kind(declaration.unwrap())
                        == ast::KIND_SET_ACCESSOR
                {
                    let write_type = if let Some(symbol_identity) = symbol_identity {
                        self.ch.get_write_type_of_symbol_identity(symbol_identity)
                    } else {
                        self.ch
                            .get_type_of_symbol_handle(declaration_symbol_handle.unwrap())
                    };
                    t = self
                        .ch
                        .instantiate_type_with_mapper_handle(Some(write_type), self.ctx.mapper);
                } else if symbol_flags
                    & (ast::SYMBOL_FLAGS_TYPE_LITERAL | ast::SYMBOL_FLAGS_SIGNATURE)
                    == 0
                {
                    let symbol_type = if let Some(symbol_identity) = symbol_identity {
                        self.ch.get_type_of_symbol_identity(symbol_identity)
                    } else {
                        self.ch
                            .get_type_of_symbol_handle(declaration_symbol_handle.unwrap())
                    };
                    let widened = self.ch.get_widened_literal_type(symbol_type);
                    t = self
                        .ch
                        .instantiate_type_with_mapper_handle(Some(widened), self.ctx.mapper);
                } else {
                    t = Some(self.ch.semantic_state.semantic_handles().error_type);
                }
            }
        }
        let mut t = t.unwrap();
        let requires_adding_undefined = declaration.is_some()
            && (ast::is_parameter_declaration(
                self.store_for_node(declaration.unwrap()),
                declaration.unwrap(),
            ) || ast::is_property_signature_declaration(
                self.store_for_node(declaration.unwrap()),
                declaration.unwrap(),
            ) || ast::is_property_declaration(
                self.store_for_node(declaration.unwrap()),
                declaration.unwrap(),
            ))
            && {
                if let Some(symbol_identity) = symbol_identity {
                    self.ch.requires_adding_implicit_undefined(
                        declaration.unwrap(),
                        Some(symbol_identity),
                        self.ctx.enclosing_declaration,
                    )
                } else {
                    self.ch.requires_adding_implicit_undefined(
                        declaration.unwrap(),
                        None,
                        self.ctx.enclosing_declaration,
                    )
                }
            };
        let add_undefined_for_parameter = requires_adding_undefined
            && ast::is_parameter_declaration(
                self.store_for_node(declaration.unwrap()),
                declaration.unwrap(),
            );
        if add_undefined_for_parameter {
            t = self.ch.get_optional_type(t, false);
        }

        let restore_flags = self.save_restore_flags();
        if self.ch.type_flags(t) & TYPE_FLAGS_UNIQUE_ES_SYMBOL != 0
            && self
                .ch
                .type_symbol_identity(t)
                .zip(
                    symbol_identity.or_else(|| {
                        declaration_symbol_handle.map(SymbolIdentity::from_symbol_handle)
                    }),
                )
                .is_some_and(|(left, right)| left == right)
            && (self.ctx.enclosing_declaration.is_none() || {
                let symbol = symbol_identity
                    .map(|symbol| symbol.symbol_handle())
                    .or(declaration_symbol_handle);
                let enclosing_file = self.ctx.enclosing_file;
                symbol.is_some_and(|symbol| {
                    let ch = &*self.ch;
                    ch.with_symbol_handle_declarations(symbol, |declarations| {
                        declarations.iter().copied().any(|declaration| {
                            let store = ch.store_for_node(declaration);
                            let declaration_file =
                                ast::get_source_file_of_node(store, Some(declaration));
                            declaration_file.as_ref().is_some_and(|declaration_file| {
                                enclosing_file.is_some_and(|enclosing_file| {
                                    SourceFileIdentity::from_root(*declaration_file)
                                        == enclosing_file
                                })
                            })
                        })
                    })
                })
            })
        {
            self.ctx.flags |= nodebuilder::FLAGS_ALLOW_UNIQUE_ES_SYMBOL_TYPE;
        }
        let mut result: Option<ast::Node> = None;
        let mut reported_inference_fallback = false;
        if !self.is_actively_expanding()
            && try_reuse
            && self.ctx.enclosing_declaration.is_some()
            && declaration.is_some()
        {
            let declaration = declaration.unwrap();
            let store = self.ch.store_for_node(declaration);
            let can_reuse = ast::is_accessor(store, declaration)
                || (ast::has_inferred_type(store, declaration)
                    && !ast::node_is_synthesized(store, declaration)
                    && self.ch.object_flags(t) & OBJECT_FLAGS_REQUIRES_WIDENING == 0);
            if can_reuse {
                let restore_symbol_type = if let Some(symbol_identity) = symbol_identity {
                    self.add_symbol_identity_type_to_context(symbol_identity, t)
                } else {
                    self.add_symbol_handle_type_to_context(declaration_symbol_handle.unwrap(), t)
                };
                let mut pt = if ast::is_accessor(store, declaration) {
                    self.pc.get_type_of_accessor(store, declaration)
                } else if ast::is_variable_declaration(store, declaration) {
                    let declaration_symbol_identity = symbol_identity.or_else(|| {
                        declaration_symbol_handle.map(SymbolIdentity::from_symbol_handle)
                    });
                    self.pc
                        .get_type_of_declaration_with_single_variable_declaration(
                            store,
                            declaration,
                            declaration_symbol_identity.map(|symbol| {
                                self.has_single_variable_declaration_for_symbol_identity(symbol)
                            }),
                        )
                } else {
                    self.pc.get_type_of_declaration(store, declaration)
                };
                let report_errors = !self.ctx.suppress_report_inference_fallback;
                if self.pseudo_type_equivalent_to_type(
                    &pt,
                    Some(t),
                    !requires_adding_undefined
                        && (ast::is_parameter_declaration(store, declaration)
                            || ast::is_property_signature_declaration(store, declaration)
                            || ast::is_property_declaration(store, declaration))
                        && is_optional_declaration(store, declaration),
                    report_errors,
                ) {
                    let type_from_pseudo = self.pseudo_type_to_type(&pt);
                    if type_from_pseudo.is_some()
                        && requires_adding_undefined
                        && contains_non_missing_undefined_type(self.ch, t)
                        && !contains_non_missing_undefined_type(self.ch, type_from_pseudo.unwrap())
                    {
                        pt = pseudochecker::new_pseudo_type_union(vec![
                            pt,
                            pseudochecker::pseudo_type_undefined(),
                        ]);
                    }
                    result = self.pseudo_type_to_node_with_checker_fallback(&pt, t);
                } else {
                    reported_inference_fallback = report_errors
                        && pt.kind == pseudochecker::PseudoTypeKind::Inferred
                        && !pt.as_pseudo_type_inferred().error_nodes.is_empty();
                    if requires_adding_undefined {
                        pt = pseudochecker::new_pseudo_type_union(vec![
                            pt,
                            pseudochecker::pseudo_type_undefined(),
                        ]);
                        if self.pseudo_type_equivalent_to_type(&pt, Some(t), false, report_errors) {
                            result = self.pseudo_type_to_node_with_checker_fallback(&pt, t);
                            reported_inference_fallback = false;
                        }
                    }
                }
                restore_symbol_type(self);
            }
        }

        if result.is_none() {
            if reported_inference_fallback {
                let old_suppress = self.ctx.suppress_report_inference_fallback;
                self.ctx.suppress_report_inference_fallback = true;
                result = self.type_to_type_node(t);
                self.ctx.suppress_report_inference_fallback = old_suppress;
            } else {
                result = self.type_to_type_node(t);
            }
        }
        restore_flags(self);
        result.unwrap_or_else(|| {
            Self::node_value(
                self.e
                    .factory
                    .node_factory
                    .new_keyword_type_node(ast::KIND_ANY_KEYWORD),
            )
        })
    }

    fn type_from_parameter_declaration(
        &mut self,
        declaration: ast::Node,
        parameter_type: TypeHandle,
        requires_adding_undefined: bool,
    ) -> Option<ast::Node> {
        let store = self.store_for_node(declaration);
        if !ast::is_parameter_declaration(store, declaration) {
            return None;
        }
        if let Some(declared_type) = store.r#type(declaration) {
            return self.try_reuse_existing_type_node(
                declared_type,
                parameter_type,
                declaration,
                requires_adding_undefined,
            );
        }
        let initializer = store.initializer(declaration)?;
        let initializer_store = self.store_for_node(initializer);
        if !ast::is_assertion_expression(initializer_store, initializer) {
            return None;
        }
        let assertion_type = initializer_store.r#type(initializer)?;
        if ast::is_const_type_reference(initializer_store, assertion_type) {
            return None;
        }
        self.try_reuse_existing_type_node(
            assertion_type,
            parameter_type,
            declaration,
            requires_adding_undefined,
        )
    }

    fn type_from_variable_declaration(
        &mut self,
        declaration: ast::Node,
        symbol: SymbolIdentity,
    ) -> Option<ast::Node> {
        let store = self.store_for_node(declaration);
        if !ast::is_variable_declaration(store, declaration)
            || store.type_node(declaration).is_some()
            || self.is_contextually_typed(declaration)
        {
            return None;
        }
        let initializer = store.initializer(declaration)?;

        let has_single_variable_declaration = {
            let ch = &*self.ch;
            ch.with_symbol_identity_declarations(symbol, |declarations| {
                declarations.len() == 1
                    || declarations
                        .iter()
                        .filter(|declaration| {
                            ast::is_variable_declaration(
                                ch.store_for_node(**declaration),
                                **declaration,
                            )
                        })
                        .count()
                        == 1
            })
        };
        if !has_single_variable_declaration
            || self
                .ch
                .get_emit_resolver()
                .is_expando_function_declaration(declaration)
        {
            return None;
        }

        Some(self.serialize_type_for_expression(initializer))
    }

    fn is_contextually_typed(&self, node: ast::Node) -> bool {
        let store = self.store_for_node(node);
        let mut current = store.parent(node);
        while let Some(node) = current {
            if ast::is_call_expression(store, node)
                || store.kind(node) == ast::Kind::SatisfiesExpression
            {
                return true;
            }
            if (matches!(
                store.kind(node),
                ast::Kind::VariableDeclaration
                    | ast::Kind::Parameter
                    | ast::Kind::PropertyDeclaration
                    | ast::Kind::PropertySignature
            ) || ast::is_assertion_expression(store, node))
                && store.r#type(node).is_some()
                && !ast::is_const_assertion(store, node)
            {
                return true;
            }
            if matches!(
                store.kind(node),
                ast::Kind::JsxElement | ast::Kind::JsxSelfClosingElement | ast::Kind::JsxExpression
            ) {
                return true;
            }
            current = store.parent(node);
        }
        false
    }

    fn type_from_accessor_declaration(
        &mut self,
        declaration: ast::Node,
        accessor_type: TypeHandle,
    ) -> Option<ast::Node> {
        let annotation = self
            .ch
            .get_annotated_accessor_type_node(Some(declaration))?;
        self.try_reuse_existing_type_node(annotation, accessor_type, declaration, false)
    }

    fn type_from_expression(&mut self, node: ast::Node) -> Option<ast::Node> {
        let store = self.store_for_node(node);
        match store.kind(node) {
            ast::Kind::ParenthesizedExpression => store
                .expression(node)
                .and_then(|expression| self.type_from_expression(expression)),
            ast::Kind::ObjectLiteralExpression => self.type_from_object_literal_expression(node),
            ast::Kind::NullKeyword => {
                if self.ch.strict_null_checks() {
                    let null_literal = self
                        .e
                        .factory
                        .node_factory
                        .new_keyword_expression(ast::KIND_NULL_KEYWORD);
                    Some(Self::node_value(
                        self.e
                            .factory
                            .node_factory
                            .new_literal_type_node(null_literal),
                    ))
                } else {
                    Some(Self::node_value(
                        self.e
                            .factory
                            .node_factory
                            .new_keyword_type_node(ast::KIND_ANY_KEYWORD),
                    ))
                }
            }
            ast::Kind::ArrowFunction | ast::Kind::FunctionExpression => {
                Some(self.type_from_function_like_expression(node))
            }
            ast::Kind::StringLiteral | ast::Kind::NoSubstitutionTemplateLiteral => {
                Some(Self::node_value(
                    self.e
                        .factory
                        .node_factory
                        .new_keyword_type_node(ast::KIND_STRING_KEYWORD),
                ))
            }
            ast::Kind::NumericLiteral => Some(Self::node_value(
                self.e
                    .factory
                    .node_factory
                    .new_keyword_type_node(ast::KIND_NUMBER_KEYWORD),
            )),
            ast::Kind::BigIntLiteral => Some(Self::node_value(
                self.e
                    .factory
                    .node_factory
                    .new_keyword_type_node(ast::KIND_BIG_INT_KEYWORD),
            )),
            ast::Kind::TrueKeyword | ast::Kind::FalseKeyword => Some(Self::node_value(
                self.e
                    .factory
                    .node_factory
                    .new_keyword_type_node(ast::KIND_BOOLEAN_KEYWORD),
            )),
            _ => None,
        }
    }

    fn type_from_function_like_expression(&mut self, node: ast::Node) -> ast::Node {
        let signature = self.ch.get_signature_from_declaration(node);
        let return_type_node = self.serialize_return_type_for_signature(signature, true);
        let type_parameters = self.type_parameters_from_function_like_expression(node);
        let parameters = self.parameters_from_function_like_expression(node);
        Self::node_value(self.e.factory.node_factory.new_function_type_node(
            type_parameters,
            parameters,
            return_type_node,
        ))
    }

    fn type_parameters_from_function_like_expression(
        &mut self,
        node: ast::Node,
    ) -> Option<ast::NodeList> {
        let type_parameters = {
            let store = self.store_for_node(node);
            store
                .type_parameters(node)
                .map(|type_parameters| type_parameters.iter().collect::<Vec<_>>())
        }?;
        let nodes = type_parameters
            .into_iter()
            .map(|type_parameter| Self::node_value(self.deep_clone_node(type_parameter)))
            .collect::<Vec<_>>();
        Some(self.new_factory_node_list(nodes))
    }

    fn parameters_from_function_like_expression(&mut self, node: ast::Node) -> ast::NodeList {
        let parameters = {
            let store = self.store_for_node(node);
            store
                .source_parameters(node)
                .map(|parameters| parameters.iter().collect::<Vec<_>>())
                .unwrap_or_default()
        };
        let nodes = parameters
            .into_iter()
            .map(|parameter| self.ensure_parameter_for_function_like_expression(parameter))
            .collect::<Vec<_>>();
        self.new_factory_node_list(nodes)
    }

    fn ensure_parameter_for_function_like_expression(&mut self, parameter: ast::Node) -> ast::Node {
        let (dot_dot_dot_token, name) = {
            let store = self.store_for_node(parameter);
            (store.dot_dot_dot_token(parameter), store.name(parameter))
        };
        let symbol_identity = self
            .ch
            .get_symbol_of_declaration(parameter)
            .map(SymbolIdentity::from_symbol_handle);

        let dot_dot_dot_token = dot_dot_dot_token.and_then(|token| {
            let cloned = Self::node_value(self.deep_clone_node(token));
            self.set_text_range(Some(cloned), Some(token))
        });
        let name = if let Some(symbol_identity) = symbol_identity {
            Some(self.parameter_to_parameter_declaration_name(symbol_identity, Some(parameter)))
        } else {
            name.map(|name| Self::node_value(self.deep_clone_node(name)))
        };
        let question_token = if self.ch.is_optional_parameter(parameter) {
            Some(
                self.e
                    .factory
                    .node_factory
                    .new_token(ast::KIND_QUESTION_TOKEN),
            )
        } else {
            None
        };
        let type_node = Some(self.serialize_type_for_declaration_for_symbol_identity(
            Some(parameter),
            None,
            symbol_identity,
            true,
        ));
        let parameter_node =
            Self::node_value(self.e.factory.node_factory.new_parameter_declaration(
                None,
                dot_dot_dot_token,
                name,
                question_token,
                type_node,
                None,
            ));
        let parameter_node = self
            .set_text_range(Some(parameter_node), Some(parameter))
            .unwrap_or(parameter_node);
        self.set_comment_range(parameter_node, Some(parameter));
        parameter_node
    }

    fn type_from_object_literal_expression(&mut self, node: ast::Node) -> Option<ast::Node> {
        if !self.can_get_type_from_object_literal(node) {
            return Some(self.serialize_type_for_expression(node));
        }

        let properties = self
            .store_for_node(node)
            .properties(node)?
            .iter()
            .collect::<Vec<_>>();
        let mut members = Vec::with_capacity(properties.len());
        for property in properties {
            let property_store = self.store_for_node(property);
            if !ast::is_property_assignment(property_store, property) {
                return None;
            }
            let name = property_store.name(property)?;
            let initializer = property_store.initializer(property)?;
            let value_type_node = self.type_from_expression(initializer).or_else(|| {
                let property_type = {
                    let symbol = self.ch.get_symbol_of_declaration(property)?;
                    self.ch.get_type_of_symbol_handle(symbol)
                };
                Some(self.serialize_type_for_declaration(
                    Some(property),
                    Some(property_type),
                    None,
                    true,
                ))
            })?;
            let property_name = self.ensure_factory_node(name);
            let member = self
                .e
                .factory
                .node_factory
                .new_property_signature_declaration(
                    None,
                    property_name,
                    None,
                    Some(value_type_node),
                    None,
                );
            members.push(Self::node_value(member));
        }
        let members = self.new_factory_node_list(members);
        let type_literal_node =
            Self::node_value(self.e.factory.node_factory.new_type_literal_node(members));
        self.e.set_emit_flags(
            &type_literal_node,
            if self.ctx.flags & nodebuilder::FLAGS_MULTILINE_OBJECT_LITERALS != 0 {
                0
            } else {
                printer::EF_SINGLE_LINE
            },
        );
        Some(type_literal_node)
    }

    fn can_get_type_from_object_literal(&mut self, object_literal: ast::Node) -> bool {
        let properties = self
            .store_for_node(object_literal)
            .properties(object_literal)
            .map(|properties| properties.iter().collect::<Vec<_>>())
            .unwrap_or_default();
        let mut result = true;

        for property in properties {
            let (property_flags, is_shorthand, is_spread, name) = {
                let store = self.store_for_node(property);
                (
                    store.flags(property),
                    ast::is_shorthand_property_assignment(store, property),
                    ast::is_spread_assignment(store, property),
                    store.name(property),
                )
            };
            if property_flags.intersects(ast::NODE_FLAGS_THIS_NODE_HAS_ERROR) {
                result = false;
                break;
            }
            if is_shorthand || is_spread {
                self.report_inference_fallback(property);
                result = false;
                continue;
            }

            let Some(name) = name else {
                result = false;
                continue;
            };
            let (name_flags, is_private_identifier, computed_expression) = {
                let name_store = self.store_for_node(name);
                (
                    name_store.flags(name),
                    ast::is_private_identifier(name_store, name),
                    if ast::is_computed_property_name(name_store, name) {
                        name_store.expression(name)
                    } else {
                        None
                    },
                )
            };
            if name_flags.intersects(ast::NODE_FLAGS_THIS_NODE_HAS_ERROR) {
                result = false;
                break;
            }
            if is_private_identifier {
                result = false;
            } else if let Some(expression) = computed_expression {
                let is_serializable_computed_name = {
                    let expression_store = self.store_for_node(expression);
                    ast::is_primitive_literal_value(expression_store, expression, false)
                } || self
                    .ch
                    .get_emit_resolver()
                    .is_definitely_reference_to_global_symbol_object(expression);
                if !is_serializable_computed_name {
                    self.report_inference_fallback(name);
                    result = false;
                }
            }
        }

        result
    }

    fn type_from_property_assignment_assertion(
        &mut self,
        declaration: ast::Node,
        property_type: TypeHandle,
    ) -> Option<ast::Node> {
        let store = self.store_for_node(declaration);
        if !ast::is_property_assignment(store, declaration) {
            return None;
        }
        let initializer = store.initializer(declaration)?;
        let initializer_store = self.store_for_node(initializer);
        if !ast::is_assertion_expression(initializer_store, initializer) {
            return None;
        }
        let assertion_type = initializer_store.r#type(initializer)?;
        if ast::is_const_type_reference(initializer_store, assertion_type) {
            return None;
        }
        self.try_reuse_existing_type_node(assertion_type, property_type, declaration, false)
    }

    fn get_parent_symbol_of_type_parameter(
        &mut self,
        type_parameter: TypeHandle,
    ) -> Option<SymbolIdentity> {
        let symbol = self.ch.type_symbol_identity(type_parameter)?;
        let tp = self
            .ch
            .find_symbol_identity_declaration(symbol, |checker, declaration| {
                checker.store_for_node(declaration).kind(declaration) == ast::KIND_TYPE_PARAMETER
            });
        let tp = tp.unwrap();
        let host = self.store_for_node(tp).parent(tp);
        if host.is_none() {
            return None;
        }
        let host = host.unwrap();
        self.ch
            .get_symbol_of_node(host)
            .map(SymbolIdentity::from_symbol_handle)
    }

    fn visit_and_transform_type(
        &mut self,
        t: TypeHandle,
        transform: fn(&mut NodeBuilderImpl<'a, 'state, 'c, 'e>, TypeHandle) -> ast::Node,
    ) -> ast::Node {
        let type_id = self.ch.type_id(t);
        let type_symbol = self.ch.type_symbol_identity(t);
        let is_constructor_object = self.ch.object_flags(t) & OBJECT_FLAGS_ANONYMOUS != 0
            && type_symbol.is_some_and(|symbol| {
                self.symbol_identity_flags(symbol) & ast::SYMBOL_FLAGS_CLASS != 0
            });
        let type_reference_node = self
            .ch
            .type_record(t)
            .as_type_reference()
            .and_then(|record| record.node);
        let id = if self.ch.object_flags(t) & OBJECT_FLAGS_REFERENCE != 0
            && type_reference_node.is_some()
        {
            Some(CompositeSymbolIdentity {
                is_constructor_node: false,
                symbol_id: None,
                node_id: {
                    let node = type_reference_node.unwrap();
                    ast::get_node_id(self.store_for_node(node), node)
                },
            })
        } else if self.ch.type_flags(t) & TYPE_FLAGS_CONDITIONAL != 0 {
            let conditional = self.ch.type_record(t).as_conditional_type();
            let root = self
                .ch
                .semantic_state
                .conditional_root_record(conditional.root.unwrap());
            let root_node = root.node.unwrap();
            Some(CompositeSymbolIdentity {
                is_constructor_node: false,
                symbol_id: None,
                node_id: ast::get_node_id(self.store_for_node(root_node), root_node),
            })
        } else if let Some(symbol) = type_symbol {
            Some(CompositeSymbolIdentity {
                is_constructor_node: is_constructor_object,
                symbol_id: Some(self.ch.symbol_handle_id(symbol.symbol_handle())),
                node_id: 0,
            })
        } else {
            None
        };
        // Since instantiations of the same anonymous type have the same symbol, tracking symbols instead
        // of types allows us to catch circular references to instantiations of the same anonymous type
        let key = CompositeTypeCacheIdentity {
            type_id,
            flags: self.ctx.flags,
            internal_flags: self.ctx.internal_flags,
        };
        if self.ctx.max_expansion_depth <= 0
            && self.ctx.enclosing_declaration.is_some()
            && self.links.has(self.ctx.enclosing_declaration.unwrap())
        {
            let cached_result = {
                self.with_node_builder_links(self.ctx.enclosing_declaration.unwrap(), |links| {
                    links.serialized_types.get(&key).cloned()
                })
            };
            if let Some(cached_result) = cached_result {
                for arg in &cached_result.tracked_symbols {
                    self.track_symbol_identity_with_flags(
                        arg.symbol,
                        arg.symbol_flags,
                        arg.enclosing_declaration,
                        arg.meaning,
                    );
                }
                if cached_result.truncating {
                    self.ctx.truncating = true;
                }
                self.ctx.approximate_length += cached_result.added_length as usize;
                let cached_node = cached_result.node.clone();
                let cloned = self.deep_clone_node(cached_node);
                return Self::node_value(cloned);
            }
        }

        let mut depth = 0;
        if let Some(id) = &id {
            depth = *self.ctx.symbol_depth.get(id).unwrap_or(&0);
            if depth > 10 {
                return self.create_elided_information_placeholder();
            }
            self.ctx.symbol_depth.insert(id.clone(), depth + 1);
        }
        self.ctx.visited_types.add(type_id);
        let prev_tracked_symbols = std::mem::take(&mut self.ctx.tracked_symbols);
        let start_length = self.ctx.approximate_length;
        let result = transform(self, t);
        let added_length = self.ctx.approximate_length - start_length;
        if !self.ctx.reported_diagnostic
            && !self.ctx.encountered_error
            && self.ctx.enclosing_declaration.is_some()
        {
            self.with_node_builder_links_mut(self.ctx.enclosing_declaration.unwrap(), |links| {
                links.serialized_types.insert(
                    key,
                    SerializedTypeEntry {
                        node: result,
                        truncating: self.ctx.truncating,
                        added_length: added_length as i32,
                        tracked_symbols: self.ctx.tracked_symbols.clone(),
                    },
                );
            });
        }
        self.ctx.visited_types.delete(&type_id);
        if let Some(id) = id {
            self.ctx.symbol_depth.insert(id, depth);
        }
        self.ctx.tracked_symbols = prev_tracked_symbols;
        result
    }

    fn type_reference_to_type_node(&mut self, t: TypeHandle) -> ast::Node {
        let mut type_arguments = self.ch.get_type_arguments(t);
        let target = self.ch.type_target(t);
        if target == self.ch.semantic_state.semantic_handles().global_array_type
            || target
                == self
                    .ch
                    .semantic_state
                    .semantic_handles()
                    .global_readonly_array_type
        {
            if self.ctx.flags & nodebuilder::FLAGS_WRITE_ARRAY_AS_GENERIC_TYPE != 0 {
                let type_argument_node = self.type_to_type_node(type_arguments[0]).unwrap();
                let type_name = self
                    .new_identifier_with_symbol_identity(
                        if target == self.ch.semantic_state.semantic_handles().global_array_type {
                            "Array"
                        } else {
                            "ReadonlyArray"
                        },
                        self.ch.type_symbol_identity(target),
                    )
                    .clone();
                let type_arguments = self.new_factory_node_list(vec![type_argument_node.clone()]);
                let type_reference_node = self
                    .e
                    .factory
                    .node_factory
                    .new_type_reference_node(type_name, Some(type_arguments));
                return Self::node_value(type_reference_node);
            }
            let element_type = self.type_to_type_node(type_arguments[0]).unwrap();
            let array_type = self
                .e
                .factory
                .node_factory
                .new_array_type_node(element_type.clone());
            if target == self.ch.semantic_state.semantic_handles().global_array_type {
                return Self::node_value(array_type);
            }
            let readonly_array_type = self
                .e
                .factory
                .node_factory
                .new_type_operator_node(ast::KIND_READONLY_KEYWORD, array_type);
            return Self::node_value(readonly_array_type);
        } else if self.ch.object_flags(target) & OBJECT_FLAGS_TUPLE != 0 {
            let tuple = self.ch.target_tuple_type_record(t);
            let tuple_element_infos = tuple.element_infos.clone();
            let tuple_readonly = tuple.readonly;
            type_arguments = type_arguments
                .into_iter()
                .enumerate()
                .map(|(i, arg)| {
                    let mut is_optional = false;
                    if i < tuple_element_infos.len() {
                        is_optional = tuple_element_infos[i].flags & ELEMENT_FLAGS_OPTIONAL != 0;
                    }
                    self.ch.remove_missing_type(arg, is_optional)
                })
                .collect();
            if !type_arguments.is_empty() {
                let arity = self.ch.get_type_reference_arity(t);
                let tuple_constituent_nodes = self.map_to_type_nodes(
                    type_arguments[0..arity].to_vec(),
                    false, /*isBareList*/
                );
                if let Some(tuple_constituent_nodes) = tuple_constituent_nodes {
                    let mut tuple_constituent_nodes = self
                        .e
                        .factory
                        .node_factory
                        .emit_node_list_nodes(tuple_constituent_nodes);
                    for i in 0..tuple_constituent_nodes.len() {
                        let flags = tuple_element_infos[i].flags;
                        let labeled_element_declaration =
                            tuple_element_infos[i].labeled_declaration;
                        if labeled_element_declaration.is_some() {
                            let name = {
                                let tuple_element_label = self.ch.get_tuple_element_label(
                                    tuple_element_infos[i],
                                    None,
                                    i,
                                );
                                self.new_identifier(&tuple_element_label, None /*symbol*/)
                            }
                            .clone();
                            let dot_dot_dot_token = if flags & ELEMENT_FLAGS_VARIABLE != 0 {
                                Some(
                                    self.e
                                        .factory
                                        .node_factory
                                        .new_token(ast::KIND_DOT_DOT_DOT_TOKEN),
                                )
                            } else {
                                None
                            };
                            let question_token = if flags & ELEMENT_FLAGS_OPTIONAL != 0 {
                                Some(
                                    self.e
                                        .factory
                                        .node_factory
                                        .new_token(ast::KIND_QUESTION_TOKEN),
                                )
                            } else {
                                None
                            };
                            let tuple_type = if flags & ELEMENT_FLAGS_REST != 0 {
                                self.e
                                    .factory
                                    .node_factory
                                    .new_array_type_node(tuple_constituent_nodes[i])
                            } else {
                                tuple_constituent_nodes[i]
                            };
                            tuple_constituent_nodes[i] =
                                self.e.factory.node_factory.new_named_tuple_member(
                                    dot_dot_dot_token,
                                    name,
                                    question_token,
                                    tuple_type,
                                );
                        } else if flags & ELEMENT_FLAGS_VARIABLE != 0 {
                            let rest_type = if flags & ELEMENT_FLAGS_REST != 0 {
                                self.e
                                    .factory
                                    .node_factory
                                    .new_array_type_node(tuple_constituent_nodes[i])
                            } else {
                                tuple_constituent_nodes[i]
                            };
                            tuple_constituent_nodes[i] =
                                self.e.factory.node_factory.new_rest_type_node(rest_type);
                        } else if flags & ELEMENT_FLAGS_OPTIONAL != 0 {
                            tuple_constituent_nodes[i] = self
                                .e
                                .factory
                                .node_factory
                                .new_optional_type_node(tuple_constituent_nodes[i]);
                        }
                    }
                    let tuple_constituent_nodes =
                        self.new_factory_node_list(tuple_constituent_nodes);
                    let tuple_type_node = self
                        .e
                        .factory
                        .node_factory
                        .new_tuple_type_node(tuple_constituent_nodes);
                    self.e
                        .set_emit_flags(&tuple_type_node, printer::EF_SINGLE_LINE);
                    if tuple_readonly {
                        let readonly_tuple_type = self
                            .e
                            .factory
                            .node_factory
                            .new_type_operator_node(ast::KIND_READONLY_KEYWORD, tuple_type_node);
                        return Self::node_value(readonly_tuple_type);
                    }
                    return Self::node_value(tuple_type_node);
                }
            }
            if self.ctx.encountered_error
                || self.ctx.flags & nodebuilder::FLAGS_ALLOW_EMPTY_TUPLE != 0
            {
                let elements = self.new_factory_node_list(Vec::new());
                let tuple_type_node = self.e.factory.node_factory.new_tuple_type_node(elements);
                self.e
                    .set_emit_flags(&tuple_type_node, printer::EF_SINGLE_LINE);
                if tuple_readonly {
                    let readonly_tuple_type = self
                        .e
                        .factory
                        .node_factory
                        .new_type_operator_node(ast::KIND_READONLY_KEYWORD, tuple_type_node);
                    return Self::node_value(readonly_tuple_type);
                }
                return Self::node_value(tuple_type_node);
            }
            self.ctx.encountered_error = true;
            return Self::node_value(
                self.e
                    .factory
                    .node_factory
                    .new_keyword_type_node(ast::KIND_ANY_KEYWORD),
            );
        } else if self.ctx.flags & nodebuilder::FLAGS_WRITE_CLASS_EXPRESSION_AS_TYPE_LITERAL != 0
            && self
                .ch
                .type_symbol_identity(t)
                .and_then(|symbol| {
                    self.ch
                        .missing_name_symbol_identity_value_declaration(symbol)
                })
                .is_some()
            && {
                let value_declaration = self
                    .ch
                    .type_symbol_identity(t)
                    .and_then(|symbol| {
                        self.ch
                            .missing_name_symbol_identity_value_declaration(symbol)
                    })
                    .unwrap();
                ast::is_class_like(self.store_for_node(value_declaration), value_declaration)
            }
            && {
                let symbol = self.ch.type_symbol_identity(t).unwrap();
                self.is_symbol_accessible_in_builder_scope_by_identity(
                    Some(symbol),
                    self.ctx.enclosing_declaration,
                    ast::SYMBOL_FLAGS_VALUE,
                    false,
                )
                .accessibility
                    != printer::SYMBOL_ACCESSIBILITY_ACCESSIBLE
            }
        {
            return self.create_anonymous_type_node(t);
        }
        let outer_type_parameter_count = self.ch.interface_outer_type_parameter_count(target);
        let mut i = 0;
        let mut result_type = None;
        if outer_type_parameter_count != 0 {
            let length = outer_type_parameter_count;
            while i < length {
                // Find group of type arguments for type parameters with the same declaring container.
                let start = i;
                let type_parameter = self.ch.interface_outer_type_parameter_at(target, i);
                let parent = self
                    .get_parent_symbol_of_type_parameter(type_parameter)
                    .expect("outer type parameter should have a parent symbol");
                while i < length {
                    let type_parameter = self.ch.interface_outer_type_parameter_at(target, i);
                    if !self
                        .get_parent_symbol_of_type_parameter(type_parameter)
                        .is_some_and(|current| current == parent)
                    {
                        break;
                    }
                    i += 1;
                }

                // When type parameters are their own type arguments for the whole group (i.e. we have
                // the default outer type arguments), we don't show the group.
                if !(start..i).all(|index| {
                    self.ch.interface_outer_type_parameter_at(target, index)
                        == type_arguments[index]
                }) {
                    let type_argument_slice = self.map_to_type_nodes(
                        type_arguments[start..i].to_vec(),
                        false, /*isBareList*/
                    );
                    let restore_flags = self.save_restore_flags();
                    self.ctx.flags |= nodebuilder::FLAGS_FORBID_INDEXED_ACCESS_SYMBOL_REFERENCES;
                    let reference = self
                        .symbol_identity_to_type_node(
                            parent,
                            ast::SYMBOL_FLAGS_TYPE,
                            type_argument_slice,
                        )
                        .unwrap();
                    restore_flags(self);
                    result_type = Some(if let Some(result_type) = result_type {
                        self.append_reference_to_type(result_type, reference)
                    } else {
                        reference
                    });
                }
            }
        }

        let mut type_argument_nodes = None;
        if !type_arguments.is_empty() {
            let mut type_parameter_count = self
                .ch
                .interface_type_parameter_count(target)
                .min(type_arguments.len());
            let global_iterable_type = self
                .ch
                .resolve_global_type(self.ch.semantic_state.get_global_iterable_type);
            let global_iterable_iterator_type = self
                .ch
                .resolve_global_type(self.ch.semantic_state.get_global_iterable_iterator_type);
            let global_async_iterable_type = self
                .ch
                .resolve_global_type(self.ch.semantic_state.get_global_async_iterable_type);
            let global_async_iterable_iterator_type = self.ch.resolve_global_type(
                self.ch
                    .semantic_state
                    .get_global_async_iterable_iterator_type,
            );
            let should_elide_defaults = self.ch.is_reference_to_type(Some(t), global_iterable_type)
                || self
                    .ch
                    .is_reference_to_type(Some(t), global_iterable_iterator_type)
                || self
                    .ch
                    .is_reference_to_type(Some(t), global_async_iterable_type)
                || self
                    .ch
                    .is_reference_to_type(Some(t), global_async_iterable_iterator_type);
            // Maybe we should do this for more types, but for now we only elide type arguments that are
            // identical to their associated type parameters' defaults for `Iterable`, `IterableIterator`,
            // `AsyncIterable`, and `AsyncIterableIterator` to provide backwards-compatible .d.ts emit due
            // to each now having three type parameters instead of only one.
            if should_elide_defaults && {
                if let Some(node) = self
                    .ch
                    .type_record(t)
                    .as_type_reference()
                    .and_then(|record| record.node)
                {
                    let node_store = self.store_for_node(node);
                    !ast::is_type_reference_node(node_store, node)
                        || node_store
                            .type_arguments(node)
                            .is_none_or(|type_arguments| {
                                type_arguments.len() < type_parameter_count
                            })
                } else {
                    true
                }
            } {
                while type_parameter_count > 0 {
                    let type_argument = type_arguments[type_parameter_count - 1];
                    let type_parameter = self
                        .ch
                        .interface_type_parameter_at(target, type_parameter_count - 1);
                    let Some(default_type) =
                        self.ch.get_default_from_type_parameter(type_parameter)
                    else {
                        break;
                    };
                    if !self.ch.is_type_identical_to(type_argument, default_type) {
                        break;
                    }
                    type_parameter_count -= 1;
                }
            }
            type_argument_nodes = self.map_to_type_nodes(
                type_arguments[i..type_parameter_count].to_vec(),
                false, /*isBareList*/
            );
        }
        let restore_flags = self.save_restore_flags();
        self.ctx.flags |= nodebuilder::FLAGS_FORBID_INDEXED_ACCESS_SYMBOL_REFERENCES;
        let final_ref = self
            .symbol_identity_to_type_node(
                self.ch
                    .type_symbol_identity(t)
                    .expect("type reference should have a symbol for serialization"),
                ast::SYMBOL_FLAGS_TYPE,
                type_argument_nodes,
            )
            .unwrap();
        restore_flags(self);
        if let Some(result_type) = result_type {
            self.append_reference_to_type(result_type, final_ref)
        } else {
            final_ref
        }
    }

    pub(crate) fn type_to_type_node(&mut self, mut t: TypeHandle) -> Option<ast::Node> {
        let restore_flags = self.save_restore_flags();
        self.ctx.type_stack.push(Some(self.ch.type_id(t)));
        let in_type_alias = self.ctx.flags & nodebuilder::FLAGS_IN_TYPE_ALIAS;
        self.ctx.flags &= !nodebuilder::FLAGS_IN_TYPE_ALIAS;
        if self.ctx.flags & nodebuilder::FLAGS_NO_TYPE_REDUCTION == 0 {
            t = self.ch.get_reduced_type(t);
        }
        let mut expanding_enum = false;
        let type_flags = self.ch.type_flags(t);
        let type_alias = self.ch.type_alias_record(t).cloned();
        let type_symbol = self.ch.type_symbol(t);
        let result = 'build: {
            if type_flags & TYPE_FLAGS_ANY != 0 {
                if let Some(alias) = type_alias.as_ref() {
                    Some(self.alias_to_type_reference_node(alias))
                } else if t == self.ch.semantic_state.semantic_handles().unresolved_type {
                    let any_type = self
                        .e
                        .factory
                        .node_factory
                        .new_keyword_type_node(ast::KIND_ANY_KEYWORD);
                    Some(self.add_synthetic_leading_comment_to_node(
                        any_type,
                        ast::KIND_MULTI_LINE_COMMENT_TRIVIA,
                        "unresolved",
                        false, /*hasTrailingNewLine*/
                    ))
                } else {
                    self.ctx.approximate_length += 3;
                    Some(Self::node_value(
                        self.e.factory.node_factory.new_keyword_type_node(
                            if t == self
                                .ch
                                .semantic_state
                                .semantic_handles()
                                .intrinsic_marker_type
                            {
                                ast::KIND_INTRINSIC_KEYWORD
                            } else {
                                ast::KIND_ANY_KEYWORD
                            },
                        ),
                    ))
                }
            } else if type_flags & TYPE_FLAGS_UNKNOWN != 0 {
                Some(Self::node_value(
                    self.e
                        .factory
                        .node_factory
                        .new_keyword_type_node(ast::KIND_UNKNOWN_KEYWORD),
                ))
            } else if type_flags & TYPE_FLAGS_STRING != 0 {
                self.ctx.approximate_length += 6;
                Some(Self::node_value(
                    self.e
                        .factory
                        .node_factory
                        .new_keyword_type_node(ast::KIND_STRING_KEYWORD),
                ))
            } else if type_flags & TYPE_FLAGS_NUMBER != 0 {
                self.ctx.approximate_length += 6;
                Some(Self::node_value(
                    self.e
                        .factory
                        .node_factory
                        .new_keyword_type_node(ast::KIND_NUMBER_KEYWORD),
                ))
            } else if type_flags & TYPE_FLAGS_BIG_INT != 0 {
                self.ctx.approximate_length += 6;
                Some(Self::node_value(
                    self.e
                        .factory
                        .node_factory
                        .new_keyword_type_node(ast::KIND_BIG_INT_KEYWORD),
                ))
            } else if type_flags & TYPE_FLAGS_BOOLEAN != 0 && type_alias.is_none() {
                self.ctx.approximate_length += 7;
                Some(Self::node_value(
                    self.e
                        .factory
                        .node_factory
                        .new_keyword_type_node(ast::KIND_BOOLEAN_KEYWORD),
                ))
            } else if type_flags & TYPE_FLAGS_ENUM_LIKE != 0 {
                let symbol =
                    type_symbol.expect("enum-like type should have a symbol for serialization");
                if self.symbol_identity_flags(symbol) & ast::SYMBOL_FLAGS_ENUM_MEMBER != 0 {
                    let parent_symbol = self
                        .ch
                        .get_parent_of_symbol_identity(symbol)
                        .expect("enum member symbol should have an enum parent");
                    let parent_name = self.symbol_identity_to_type_node(
                        parent_symbol,
                        ast::SYMBOL_FLAGS_TYPE,
                        None,
                    )?;
                    let declared_parent_type = self
                        .ch
                        .get_declared_type_of_symbol_identity_or_error(parent_symbol);
                    if declared_parent_type == t {
                        Some(parent_name)
                    } else {
                        let member_name = self.ch.missing_name_symbol_identity_name(symbol);
                        if scanner::is_identifier_text(
                            &member_name,
                            core::LanguageVariant::Standard,
                        ) {
                            let member_identifier =
                                self.e.factory.node_factory.new_identifier(member_name);
                            let member_reference = self
                                .e
                                .factory
                                .node_factory
                                .new_type_reference_node(member_identifier, None);
                            Some(self.append_reference_to_type(parent_name, member_reference))
                        } else if ast::is_import_type_node(
                            self.store_for_node(parent_name),
                            parent_name,
                        ) {
                            let index_type = {
                                let member_name = member_name
                                    .replace(ast::INTERNAL_SYMBOL_NAME_PREFIX, "\u{fffd}");
                                let literal = self.new_string_literal(&member_name);
                                self.e.factory.node_factory.new_literal_type_node(literal)
                            };
                            Some(Self::node_value(
                                self.e
                                    .factory
                                    .node_factory
                                    .new_indexed_access_type_node(parent_name, index_type),
                            ))
                        } else if ast::is_type_reference_node(
                            self.store_for_node(parent_name),
                            parent_name,
                        ) {
                            let parent_store = self.store_for_node(parent_name);
                            let type_name = parent_store.type_name(parent_name).unwrap();
                            let object_type = self
                                .e
                                .factory
                                .node_factory
                                .new_type_query_node(type_name, None);
                            let index_type = {
                                let member_name = member_name
                                    .replace(ast::INTERNAL_SYMBOL_NAME_PREFIX, "\u{fffd}");
                                let literal = self.new_string_literal(&member_name);
                                self.e.factory.node_factory.new_literal_type_node(literal)
                            };
                            Some(Self::node_value(
                                self.e
                                    .factory
                                    .node_factory
                                    .new_indexed_access_type_node(object_type, index_type),
                            ))
                        } else {
                            panic!("Unhandled type node kind returned from `symbol_to_type_node`.");
                        }
                    }
                } else if type_flags & TYPE_FLAGS_UNION == 0
                    || !self.should_expand_type(t, false /*isAlias*/)
                {
                    Some(self.symbol_identity_to_type_node(symbol, ast::SYMBOL_FLAGS_TYPE, None)?)
                } else {
                    let types = self.ch.format_union_types(
                        self.ch
                            .type_record(t)
                            .as_union_type()
                            .union_or_intersection
                            .types
                            .clone(),
                        true, /*expandingEnum*/
                    );
                    if types.len() == 1 {
                        self.type_to_type_node(types[0])
                    } else {
                        let type_nodes = self.map_to_type_nodes(types, true /*isBareList*/);
                        if let Some(type_nodes) = type_nodes {
                            if !self
                                .e
                                .factory
                                .node_factory
                                .emit_node_list_nodes(type_nodes)
                                .is_empty()
                            {
                                Some(Self::node_value(
                                    self.e.factory.node_factory.new_union_type_node(type_nodes),
                                ))
                            } else {
                                if !self.ctx.encountered_error
                                    && self.ctx.flags
                                        & nodebuilder::FLAGS_ALLOW_EMPTY_UNION_OR_INTERSECTION
                                        == 0
                                {
                                    self.ctx.encountered_error = true;
                                }
                                None
                            }
                        } else {
                            None
                        }
                    }
                }
            } else if type_flags & TYPE_FLAGS_STRING_LITERAL != 0 {
                let value = self
                    .ch
                    .type_record(t)
                    .as_literal_type()
                    .value
                    .as_string()
                    .to_string();
                self.ctx.approximate_length += value.len() + 2;
                let lit = self.new_string_literal(&value);
                self.e.mark_emit_node(&lit, printer::EF_NO_ASCII_ESCAPING);
                Some(Self::node_value(
                    self.e
                        .factory
                        .node_factory
                        .new_literal_type_node(lit.clone()),
                ))
            } else if type_flags & TYPE_FLAGS_NUMBER_LITERAL != 0 {
                let value = self.ch.type_record(t).as_literal_type().value.as_number();
                self.ctx.approximate_length += value.to_string().len();
                if jsnum::compare(value, jsnum::Number(0.0)) < 0 {
                    let numeric = self
                        .e
                        .factory
                        .node_factory
                        .new_numeric_literal(&value.to_string()[1..], ast::TOKEN_FLAGS_NONE);
                    let expression = self
                        .e
                        .factory
                        .node_factory
                        .new_prefix_unary_expression(ast::KIND_MINUS_TOKEN, numeric);
                    let literal = self
                        .e
                        .factory
                        .node_factory
                        .new_literal_type_node(expression);
                    Some(Self::node_value(literal))
                } else {
                    let numeric = self
                        .e
                        .factory
                        .node_factory
                        .new_numeric_literal(value.to_string(), ast::TOKEN_FLAGS_NONE);
                    let literal = self.e.factory.node_factory.new_literal_type_node(numeric);
                    Some(Self::node_value(literal))
                }
            } else if type_flags & TYPE_FLAGS_BIG_INT_LITERAL != 0 {
                let value = self
                    .ch
                    .type_record(t)
                    .as_literal_type()
                    .value
                    .as_big_int()
                    .to_string();
                self.ctx.approximate_length += value.len() + 1;
                let bigint = self
                    .e
                    .factory
                    .node_factory
                    .new_big_int_literal(value + "n", ast::TOKEN_FLAGS_NONE);
                let literal = self.e.factory.node_factory.new_literal_type_node(bigint);
                Some(Self::node_value(literal))
            } else if type_flags & TYPE_FLAGS_BOOLEAN_LITERAL != 0 {
                let value = self.ch.type_record(t).as_literal_type().value.as_bool();
                self.ctx.approximate_length += if value { 4 } else { 5 };
                let keyword = self
                    .e
                    .factory
                    .node_factory
                    .new_keyword_expression(if value {
                        ast::KIND_TRUE_KEYWORD
                    } else {
                        ast::KIND_FALSE_KEYWORD
                    });
                let literal = self.e.factory.node_factory.new_literal_type_node(keyword);
                Some(Self::node_value(literal))
            } else if type_flags & TYPE_FLAGS_UNIQUE_ES_SYMBOL != 0 {
                let symbol = type_symbol
                    .expect("unique symbol type should have a symbol")
                    .clone();
                if self.ctx.flags & nodebuilder::FLAGS_ALLOW_UNIQUE_ES_SYMBOL_TYPE == 0 {
                    let is_accessible = self
                        .is_symbol_accessible_in_builder_scope_by_identity(
                            Some(symbol),
                            self.ctx.enclosing_declaration,
                            ast::SYMBOL_FLAGS_VALUE,
                            false,
                        )
                        .accessibility
                        == printer::SYMBOL_ACCESSIBILITY_ACCESSIBLE;
                    if is_accessible {
                        self.ctx.approximate_length += 6;
                        break 'build self.symbol_identity_to_type_node(
                            symbol,
                            ast::SYMBOL_FLAGS_VALUE,
                            None,
                        );
                    }
                    self.report_inaccessible_unique_symbol_error();
                }
                self.ctx.approximate_length += 13;
                let symbol_type = self
                    .e
                    .factory
                    .node_factory
                    .new_keyword_type_node(ast::KIND_SYMBOL_KEYWORD);
                Some(Self::node_value(
                    self.e
                        .factory
                        .node_factory
                        .new_type_operator_node(ast::KIND_UNIQUE_KEYWORD, symbol_type),
                ))
            } else if type_flags & TYPE_FLAGS_VOID != 0 {
                self.ctx.approximate_length += 4;
                Some(Self::node_value(
                    self.e
                        .factory
                        .node_factory
                        .new_keyword_type_node(ast::KIND_VOID_KEYWORD),
                ))
            } else if type_flags & TYPE_FLAGS_UNDEFINED != 0 {
                self.ctx.approximate_length += 9;
                Some(Self::node_value(
                    self.e
                        .factory
                        .node_factory
                        .new_keyword_type_node(ast::KIND_UNDEFINED_KEYWORD),
                ))
            } else if type_flags & TYPE_FLAGS_NULL != 0 {
                self.ctx.approximate_length += 4;
                let keyword = self
                    .e
                    .factory
                    .node_factory
                    .new_keyword_expression(ast::KIND_NULL_KEYWORD);
                let literal = self.e.factory.node_factory.new_literal_type_node(keyword);
                Some(Self::node_value(literal))
            } else if type_flags & TYPE_FLAGS_NEVER != 0 {
                self.ctx.approximate_length += 5;
                Some(Self::node_value(
                    self.e
                        .factory
                        .node_factory
                        .new_keyword_type_node(ast::KIND_NEVER_KEYWORD),
                ))
            } else if type_flags & TYPE_FLAGS_ES_SYMBOL != 0 {
                self.ctx.approximate_length += 6;
                Some(Self::node_value(
                    self.e
                        .factory
                        .node_factory
                        .new_keyword_type_node(ast::KIND_SYMBOL_KEYWORD),
                ))
            } else if type_flags & TYPE_FLAGS_NON_PRIMITIVE != 0 {
                self.ctx.approximate_length += 6;
                Some(Self::node_value(
                    self.e
                        .factory
                        .node_factory
                        .new_keyword_type_node(ast::KIND_OBJECT_KEYWORD),
                ))
            } else if is_this_type_parameter(self.ch, t) {
                if self.ctx.flags & nodebuilder::FLAGS_IN_OBJECT_TYPE_LITERAL != 0 {
                    if !self.ctx.encountered_error
                        && self.ctx.flags & nodebuilder::FLAGS_ALLOW_THIS_IN_OBJECT_LITERAL == 0
                    {
                        self.ctx.encountered_error = true;
                    }
                    self.report_inaccessible_this_error();
                }
                self.ctx.approximate_length += 4;
                Some(Self::node_value(
                    self.e.factory.node_factory.new_this_type_node(),
                ))
            } else {
                if in_type_alias == 0
                    && type_alias.is_some()
                    && (self.ctx.flags & nodebuilder::FLAGS_USE_ALIAS_DEFINED_OUTSIDE_CURRENT_SCOPE
                        != 0
                        || {
                            let symbol = type_alias
                                .as_ref()
                                .unwrap()
                                .symbol
                                .expect("alias type must keep alias symbol");
                            self.is_symbol_accessible_in_builder_scope_by_identity(
                                Some(symbol),
                                self.ctx.enclosing_declaration,
                                ast::SYMBOL_FLAGS_TYPE,
                                false,
                            )
                            .accessibility
                                == printer::SYMBOL_ACCESSIBILITY_ACCESSIBLE
                        })
                {
                    if !self.should_expand_type(t, true /*isAlias*/) {
                        let alias = type_alias.as_ref().unwrap().clone();
                        let sym_identity = alias.symbol.expect("alias type must keep alias symbol");
                        let type_argument_nodes = self.map_to_alias_type_argument_nodes(&alias);
                        let sym_name = self.ch.symbol_identity_name(sym_identity);
                        if is_reserved_member_name(&sym_name)
                            && self.symbol_identity_flags(sym_identity) & ast::SYMBOL_FLAGS_CLASS
                                == 0
                        {
                            let name = self.e.factory.node_factory.new_identifier("");
                            let type_reference = self
                                .e
                                .factory
                                .node_factory
                                .new_type_reference_node(name, type_argument_nodes);
                            break 'build Some(Self::node_value(type_reference));
                        }
                        if type_argument_nodes.is_some()
                            && self
                                .e
                                .factory
                                .node_factory
                                .emit_node_list_nodes(type_argument_nodes.unwrap())
                                .len()
                                == 1
                            && self.ch.same_optional_symbol_identity(
                                self.ch.type_symbol_identity(
                                    self.ch.semantic_state.semantic_handles().global_array_type,
                                ),
                                Some(sym_identity),
                            )
                        {
                            let array_element = self
                                .e
                                .factory
                                .node_factory
                                .emit_node_list_nodes(type_argument_nodes.unwrap())
                                .first()
                                .copied()
                                .unwrap();
                            break 'build Some(Self::node_value(
                                self.e
                                    .factory
                                    .node_factory
                                    .new_array_type_node(array_element),
                            ));
                        }
                        break 'build self.symbol_identity_to_type_node(
                            sym_identity,
                            ast::SYMBOL_FLAGS_TYPE,
                            type_argument_nodes,
                        );
                    }
                    self.ctx.depth += 1;
                }
                let object_flags = self.ch.object_flags(t);
                if object_flags & OBJECT_FLAGS_REFERENCE != 0 {
                    if self.should_expand_type(t, false /*isAlias*/) {
                        self.ctx.depth += 1;
                        let result = self.create_anonymous_type_node_ex(
                            t, true, /*forceClassExpansion*/
                            true, /*forceExpansion*/
                        );
                        self.ctx.depth -= 1;
                        Some(result)
                    } else if self
                        .ch
                        .type_record(t)
                        .as_type_reference()
                        .and_then(|record| record.node)
                        .is_some()
                    {
                        Some(self.visit_and_transform_type(
                            t,
                            NodeBuilderImpl::type_reference_to_type_node,
                        ))
                    } else {
                        Some(self.type_reference_to_type_node(t))
                    }
                } else if self.ch.type_flags(t) & TYPE_FLAGS_TYPE_PARAMETER != 0
                    || object_flags & OBJECT_FLAGS_CLASS_OR_INTERFACE != 0
                {
                    if object_flags & OBJECT_FLAGS_CLASS_OR_INTERFACE != 0
                        && self.should_expand_type(t, false /*isAlias*/)
                    {
                        self.ctx.depth += 1;
                        let result = self.create_anonymous_type_node_ex(
                            t, true, /*forceClassExpansion*/
                            true, /*forceExpansion*/
                        );
                        self.ctx.depth -= 1;
                        Some(result)
                    } else if self.ch.type_flags(t) & TYPE_FLAGS_TYPE_PARAMETER != 0
                        && self.ctx.infer_type_parameters.contains(&t)
                    {
                        let symbol = self.ch.type_symbol_identity(t).unwrap();
                        let symbol_name = self.symbol_identity_display_name(symbol);
                        self.ctx.approximate_length += symbol_name.len() + 6;
                        let mut constraint_node = None;
                        if let Some(constraint) = self.ch.get_constraint_of_type_parameter(t) {
                            // If the infer type has a constraint that is not the same as the constraint
                            // we would have normally inferred based on context, we emit the constraint
                            // using `infer T extends ?`. We omit inferred constraints from type references
                            // as they may be elided.
                            let inferred_constraint =
                                self.ch.get_inferred_type_parameter_constraint(t, true);
                            if !inferred_constraint.is_some_and(|inferred_constraint| {
                                self.ch
                                    .is_type_identical_to(constraint, inferred_constraint)
                            }) {
                                self.ctx.approximate_length += 9;
                                constraint_node = self.type_to_type_node(constraint);
                            }
                        }
                        let declaration =
                            self.type_parameter_to_declaration_with_constraint(t, constraint_node);
                        let infer_type =
                            self.e.factory.node_factory.new_infer_type_node(declaration);
                        Some(Self::node_value(infer_type))
                    } else if self.ctx.flags
                        & nodebuilder::FLAGS_GENERATE_NAMES_FOR_SHADOWED_TYPE_PARAMS
                        != 0
                        && self.ch.type_flags(t) & TYPE_FLAGS_TYPE_PARAMETER != 0
                    {
                        let name = self.type_parameter_to_name(t);
                        let name_text = self.store_for_node(name).text(name).to_string();
                        self.ctx.approximate_length += name_text.len();
                        let type_name = self
                            .new_identifier_with_symbol_identity(
                                &name_text,
                                self.ch.type_symbol_identity(t),
                            )
                            .clone();
                        Some(Self::node_value(
                            self.e
                                .factory
                                .node_factory
                                .new_type_reference_node(type_name, None /*typeArguments*/),
                        ))
                    } else if let Some(symbol) = self.ch.type_symbol_identity(t) {
                        self.symbol_identity_to_type_node(symbol, ast::SYMBOL_FLAGS_TYPE, None)
                    } else {
                        let marker_super_type_for_check = self
                            .ch
                            .semantic_state
                            .semantic_handles()
                            .marker_super_type_for_check;
                        let marker_sub_type_for_check = self
                            .ch
                            .semantic_state
                            .semantic_handles()
                            .marker_sub_type_for_check;
                        let name = if (t == marker_super_type_for_check
                            || t == marker_sub_type_for_check)
                            && let Some(variance_type_parameter) =
                                self.ch.semantic_state.variance_type_parameter
                            && let Some(symbol) =
                                self.ch.type_symbol_identity(variance_type_parameter)
                        {
                            let prefix = if t == marker_sub_type_for_check {
                                "sub-"
                            } else {
                                "super-"
                            };
                            let name = self.ch.symbol_identity_name(symbol);
                            format!("{}{}", prefix, name)
                        } else {
                            "?".to_owned()
                        };
                        let type_name = self.new_identifier(&name, None /*symbol*/).clone();
                        Some(Self::node_value(
                            self.e
                                .factory
                                .node_factory
                                .new_type_reference_node(type_name, None /*typeArguments*/),
                        ))
                    }
                } else {
                    let mut current_type_flags = self.ch.type_flags(t);
                    if current_type_flags & TYPE_FLAGS_UNION != 0 {
                        if let Some(origin) = self.ch.type_record(t).as_union_type().origin {
                            t = origin;
                            current_type_flags = self.ch.type_flags(t);
                        }
                    }
                    if current_type_flags & (TYPE_FLAGS_UNION | TYPE_FLAGS_INTERSECTION) != 0 {
                        let types = if current_type_flags & TYPE_FLAGS_UNION != 0 {
                            self.ch.format_union_types(
                                self.ch
                                    .type_record(t)
                                    .as_union_type()
                                    .union_or_intersection
                                    .types
                                    .clone(),
                                expanding_enum,
                            )
                        } else {
                            self.ch
                                .type_record(t)
                                .as_intersection_type()
                                .union_or_intersection
                                .types
                                .clone()
                        };
                        if types.len() == 1 {
                            self.type_to_type_node(types[0])
                        } else {
                            let type_nodes =
                                self.map_to_type_nodes(types, true /*isBareList*/);
                            if type_nodes.is_some()
                                && !self
                                    .e
                                    .factory
                                    .node_factory
                                    .emit_node_list_nodes(type_nodes.unwrap())
                                    .is_empty()
                            {
                                if current_type_flags & TYPE_FLAGS_UNION != 0 {
                                    Some(Self::node_value(
                                        self.e
                                            .factory
                                            .node_factory
                                            .new_union_type_node(type_nodes.unwrap().clone()),
                                    ))
                                } else {
                                    Some(Self::node_value(
                                        self.e.factory.node_factory.new_intersection_type_node(
                                            type_nodes.unwrap().clone(),
                                        ),
                                    ))
                                }
                            } else {
                                if !self.ctx.encountered_error
                                    && self.ctx.flags
                                        & nodebuilder::FLAGS_ALLOW_EMPTY_UNION_OR_INTERSECTION
                                        == 0
                                {
                                    self.ctx.encountered_error = true;
                                }
                                None
                            }
                        }
                    } else if object_flags & (OBJECT_FLAGS_ANONYMOUS | OBJECT_FLAGS_MAPPED) != 0 {
                        Some(self.create_anonymous_type_node(t))
                    } else if current_type_flags & TYPE_FLAGS_INDEX != 0 {
                        let indexed_type = self.ch.type_target(t);
                        self.ctx.approximate_length += 6;
                        let index_type_node = self.type_to_type_node(indexed_type).unwrap();
                        Some(Self::node_value(
                            self.e.factory.node_factory.new_type_operator_node(
                                ast::KIND_KEY_OF_KEYWORD,
                                index_type_node.clone(),
                            ),
                        ))
                    } else if current_type_flags & TYPE_FLAGS_TEMPLATE_LITERAL != 0 {
                        let template_literal_type =
                            self.ch.type_record(t).as_template_literal_type().clone();
                        let template_head = self.e.factory.node_factory.new_template_head(
                            template_literal_type.texts[0].clone(),
                            "",
                            ast::TokenFlags::NONE,
                        );
                        let mut template_spans =
                            Vec::with_capacity(template_literal_type.types.len());
                        for (i, type_) in template_literal_type.types.iter().copied().enumerate() {
                            let literal = if i < template_literal_type.types.len() - 1 {
                                self.e.factory.node_factory.new_template_middle(
                                    template_literal_type.texts[i + 1].clone(),
                                    "",
                                    ast::TokenFlags::NONE,
                                )
                            } else {
                                self.e.factory.node_factory.new_template_tail(
                                    template_literal_type.texts[i + 1].clone(),
                                    "",
                                    ast::TokenFlags::NONE,
                                )
                            };
                            let type_node = self.type_to_type_node(type_).unwrap();
                            template_spans.push(Self::node_value(
                                self.e
                                    .factory
                                    .node_factory
                                    .new_template_literal_type_span(type_node, literal),
                            ));
                        }
                        let template_spans = self.new_factory_node_list(template_spans);
                        self.ctx.approximate_length += 2;
                        Some(Self::node_value(
                            self.e
                                .factory
                                .node_factory
                                .new_template_literal_type_node(template_head, template_spans),
                        ))
                    } else if current_type_flags & TYPE_FLAGS_STRING_MAPPING != 0 {
                        let type_node = self.type_to_type_node(self.ch.type_target(t)).unwrap();
                        let type_arguments = self.new_factory_node_list([type_node]);
                        self.symbol_identity_to_type_node(
                            self.ch
                                .type_symbol_identity(t)
                                .expect("string mapping type should have a symbol"),
                            ast::SYMBOL_FLAGS_TYPE,
                            Some(Self::output_node_list_value(type_arguments)),
                        )
                    } else if current_type_flags & TYPE_FLAGS_INDEXED_ACCESS != 0 {
                        let indexed_access_type =
                            self.ch.type_record(t).as_indexed_access_type().clone();
                        let object_type_node = self
                            .type_to_type_node(indexed_access_type.object_type.unwrap())
                            .unwrap();
                        let index_type_node = self
                            .type_to_type_node(indexed_access_type.index_type.unwrap())
                            .unwrap();
                        self.ctx.approximate_length += 2;
                        Some(Self::node_value(
                            self.e.factory.node_factory.new_indexed_access_type_node(
                                object_type_node.clone(),
                                index_type_node.clone(),
                            ),
                        ))
                    } else if current_type_flags & TYPE_FLAGS_CONDITIONAL != 0 {
                        Some(self.visit_and_transform_type(
                            t,
                            NodeBuilderImpl::conditional_type_to_type_node,
                        ))
                    } else if current_type_flags & TYPE_FLAGS_SUBSTITUTION != 0 {
                        let substitution_type =
                            self.ch.type_record(t).as_substitution_type().clone();
                        let type_node = self
                            .type_to_type_node(substitution_type.base_type.unwrap())
                            .unwrap();
                        if !self.ch.is_no_infer_type(t) {
                            Some(Self::node_value(type_node))
                        } else if let Some(no_infer_symbol) =
                            self.ch.get_global_type_alias_symbol("NoInfer", 1, false)
                        {
                            let type_args = self.new_factory_node_list([type_node]);
                            self.symbol_identity_to_type_node(
                                no_infer_symbol,
                                ast::SYMBOL_FLAGS_TYPE,
                                Some(Self::output_node_list_value(type_args)),
                            )
                        } else {
                            Some(Self::node_value(type_node))
                        }
                    } else {
                        Some(Self::node_value(
                            self.e
                                .factory
                                .node_factory
                                .new_keyword_type_node(ast::KIND_ANY_KEYWORD),
                        ))
                    }
                }
            }
        };
        self.ctx.type_stack.pop();
        restore_flags(self);
        result
    }
}

impl<'a, 'state> Checker<'a, 'state> {
    pub(crate) fn get_expanded_parameters(
        &mut self,
        sig: SignatureHandle,
        skip_union_expanding: bool,
    ) -> Vec<Vec<SymbolIdentity>> {
        let signature_parameters = self.signature_parameter_identities(sig);
        if self.signature_has_rest_parameter(sig) {
            let rest_index = signature_parameters.len() - 1;
            let rest_symbol = signature_parameters[rest_index];
            let rest_type = self.get_type_of_symbol_identity(rest_symbol);
            let expand_signature_parameters_with_tuple_members =
                |checker: &mut Checker<'a, 'state>,
                 rest_type: TypeHandle,
                 rest_index: usize,
                 rest_symbol: SymbolIdentity| {
                    let element_types = checker.get_element_types(rest_type);
                    let tuple_element_infos = checker
                        .target_tuple_type_record(rest_type)
                        .element_infos
                        .clone();
                    let associated_names = checker
                        .get_uniq_associated_names_from_tuple_type_identity(rest_type, rest_symbol);
                    let rest_params = element_types
                        .into_iter()
                        .enumerate()
                        .map(|(i, t)| {
                            let element_info = tuple_element_infos[i];
                            let name = associated_names[i].clone();
                            let flags = element_info.flags;
                            let mut check_flags = ast::CHECK_FLAGS_NONE;
                            if flags & ELEMENT_FLAGS_VARIABLE != 0 {
                                check_flags = ast::CHECK_FLAGS_REST_PARAMETER;
                            } else if flags & ELEMENT_FLAGS_OPTIONAL != 0 {
                                check_flags = ast::CHECK_FLAGS_OPTIONAL_PARAMETER;
                            }
                            let symbol = checker.new_symbol_ex(
                                ast::SYMBOL_FLAGS_FUNCTION_SCOPED_VARIABLE,
                                name,
                                check_flags,
                            );
                            let symbol = checker.transient_symbol_handle(symbol);
                            let resolved_type = if flags & ELEMENT_FLAGS_REST != 0 {
                                checker.create_array_type(t)
                            } else {
                                t
                            };
                            checker.semantic_state.set_value_symbol_resolved_type(
                                SymbolIdentity::from_symbol_handle(symbol),
                                Some(resolved_type),
                            );
                            SymbolIdentity::from_symbol_handle(symbol)
                        })
                        .collect::<Vec<_>>();
                    let mut result = signature_parameters[0..rest_index].to_vec();
                    result.extend(rest_params);
                    result
                };

            if self.is_tuple_type(rest_type) {
                return vec![expand_signature_parameters_with_tuple_members(
                    self,
                    rest_type,
                    rest_index,
                    rest_symbol,
                )];
            } else if !skip_union_expanding
                && self.type_flags(rest_type) & TYPE_FLAGS_UNION != 0
                && self
                    .type_record(rest_type)
                    .as_union_type()
                    .union_or_intersection
                    .types
                    .iter()
                    .all(|&t| self.is_tuple_type(t))
            {
                return self
                    .type_record(rest_type)
                    .as_union_type()
                    .union_or_intersection
                    .types
                    .clone()
                    .iter()
                    .map(|&t| {
                        expand_signature_parameters_with_tuple_members(
                            self,
                            t,
                            rest_index,
                            rest_symbol,
                        )
                    })
                    .collect();
            }
        }
        vec![signature_parameters]
    }
}

fn get_type_alias_for_type_literal<'a, 'state>(
    c: &mut Checker<'a, 'state>,
    t: TypeHandle,
) -> Option<SymbolIdentity> {
    let symbol = c.type_symbol_identity(t)?;
    let symbol_flags = c.symbol_identity_flags(symbol);
    if symbol_flags & ast::SYMBOL_FLAGS_TYPE_LITERAL != 0 {
        if let Some(declaration) = c.first_symbol_identity_declaration(symbol) {
            let store = c.store_for_node(declaration);
            let parent = store.parent(declaration);
            let node = ast::walk_up_parenthesized_types(store, parent);
            if node
                .as_ref()
                .is_some_and(|node| ast::is_type_alias_declaration(store, *node))
            {
                return c
                    .get_symbol_of_declaration(node.unwrap())
                    .map(SymbolIdentity::from_symbol_handle);
            }
        }
    }
    None
}

#[derive(Default)]
pub(crate) struct SignatureToSignatureDeclarationOptions {
    pub(crate) modifiers: Vec<ast::Node>,
    pub(crate) name: Option<ast::Node>,
    pub(crate) question_token: Option<ast::Node>,
}

fn get_effective_parameter_declaration<'a, 'state>(
    ch: &Checker<'a, 'state>,
    symbol: SymbolIdentity,
) -> Option<ast::Node> {
    let parameter_declaration = ch.with_symbol_identity_declarations(symbol, |declarations| {
        declarations.iter().copied().find(|declaration| {
            ch.store_for_node(*declaration).kind(*declaration) == ast::KIND_PARAMETER
        })
    });
    if parameter_declaration.is_some() {
        return Some(NodeBuilderImpl::node_value(parameter_declaration.unwrap()));
    }
    if ch.symbol_identity_flags(symbol) & ast::SYMBOL_FLAGS_TRANSIENT == 0 {
        return ch.with_symbol_identity_declarations(symbol, |_| None);
    }
    None
}

struct SortedSymbolIdentityNamePair {
    pub(crate) sym: SymbolIdentity,
    pub(crate) name: String,
}

fn is_private_identifier_symbol_identity(ch: &Checker<'_, '_>, symbol: SymbolIdentity) -> bool {
    ch.symbol_identity_name(symbol)
        .as_str()
        .starts_with(&(ast::INTERNAL_SYMBOL_NAME_PREFIX.to_string() + "#"))
}

fn can_have_module_specifier(store: &ast::AstStore, node: Option<ast::Node>) -> bool {
    let Some(node) = node else {
        return false;
    };
    match store.kind(node) {
        ast::KIND_VARIABLE_DECLARATION
        | ast::KIND_BINDING_ELEMENT
        | ast::KIND_IMPORT_DECLARATION
        | ast::KIND_EXPORT_DECLARATION
        | ast::KIND_IMPORT_EQUALS_DECLARATION
        | ast::KIND_IMPORT_CLAUSE
        | ast::KIND_NAMESPACE_EXPORT
        | ast::KIND_NAMESPACE_IMPORT
        | ast::KIND_EXPORT_SPECIFIER
        | ast::KIND_IMPORT_SPECIFIER
        | ast::KIND_IMPORT_TYPE => true,
        _ => false,
    }
}

pub(crate) fn try_get_module_specifier_from_declaration<'a>(
    store: &'a ast::AstStore,
    node: Option<ast::Node>,
) -> Option<ast::Node> {
    let res = try_get_module_specifier_from_declaration_worker(store, node?);
    if res.is_none() || !ast::is_string_literal(store, res.unwrap()) {
        return None;
    }
    res
}

fn try_get_module_specifier_from_declaration_worker<'a>(
    store: &'a ast::AstStore,
    node: ast::Node,
) -> Option<ast::Node> {
    match store.kind(node) {
        ast::KIND_VARIABLE_DECLARATION | ast::KIND_BINDING_ELEMENT => {
            let initializer = store.initializer(node);
            let require_call = ast::find_ancestor(store, initializer, |store, node| {
                ast::is_require_call(store, node, true)
            });
            require_call
                .and_then(|require_call| {
                    store.arguments(require_call).and_then(|args| args.first())
                })
                .map(NodeBuilderImpl::node_value)
        }
        ast::KIND_IMPORT_DECLARATION | ast::KIND_EXPORT_DECLARATION => store
            .module_specifier(node)
            .map(|node| NodeBuilderImpl::node_value(node)),
        ast::KIND_IMPORT_EQUALS_DECLARATION => {
            let ref_ = store.module_reference(node).unwrap();
            if store.kind(ref_) != ast::KIND_EXTERNAL_MODULE_REFERENCE {
                return None;
            }
            store
                .expression(ref_)
                .map(|node| NodeBuilderImpl::node_value(node))
        }
        ast::KIND_IMPORT_CLAUSE | ast::KIND_NAMESPACE_EXPORT => {
            store.parent(node).and_then(|parent| {
                store
                    .module_specifier(parent)
                    .map(|node| NodeBuilderImpl::node_value(node))
            })
        }
        ast::KIND_NAMESPACE_IMPORT | ast::KIND_EXPORT_SPECIFIER => store
            .parent(node)
            .and_then(|parent| store.parent(parent))
            .and_then(|parent| {
                store
                    .module_specifier(parent)
                    .map(|node| NodeBuilderImpl::node_value(node))
            }),
        ast::KIND_IMPORT_SPECIFIER => store
            .parent(node)
            .and_then(|parent| store.parent(parent))
            .and_then(|parent| store.parent(parent))
            .and_then(|parent| {
                store
                    .module_specifier(parent)
                    .map(|node| NodeBuilderImpl::node_value(node))
            }),
        ast::KIND_IMPORT_TYPE => {
            if ast::is_literal_import_type_node(store, node) {
                let argument = store.argument(node).unwrap();
                return store.literal(argument).map(NodeBuilderImpl::node_value);
            }
            None
        }
        _ => {
            debug::assert_never(&store.kind(node), None);
            None
        }
    }
}

fn can_use_property_access(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    if name.starts_with('#') {
        return name.len() > 1
            && scanner::is_identifier_text(&name[1..], core::LANGUAGE_VARIANT_STANDARD);
    }
    scanner::is_identifier_text(name, core::LANGUAGE_VARIANT_STANDARD)
}

fn starts_with_single_or_double_quote(str_: &str) -> bool {
    str_.starts_with('\'') || str_.starts_with('"')
}

fn starts_with_square_bracket(str_: &str) -> bool {
    str_.starts_with('[')
}

fn is_default_binding_context(store: &ast::AstStore, location: ast::Node) -> bool {
    store.kind(location) == ast::KIND_SOURCE_FILE || ast::is_ambient_module(store, location)
}

fn get_topmost_indexed_access_type<'s>(store: &'s ast::AstStore, node: ast::Node) -> ast::Node {
    let object_type = store.object_type(node).unwrap();
    if ast::is_indexed_access_type_node(store, object_type) {
        return get_topmost_indexed_access_type(store, object_type);
    }
    node
}

fn is_identifier_type_reference(store: &ast::AstStore, node: ast::Node) -> bool {
    ast::is_type_reference_node(store, node)
        && store
            .type_name(node)
            .is_some_and(|type_name| ast::is_identifier(store, type_name))
}

fn late_bound_symbol_name_to_string(name: &str) -> Option<String> {
    let encoded = name
        .strip_prefix(ast::INTERNAL_SYMBOL_NAME_PREFIX)?
        .strip_prefix('@')?;
    let (base_name, _) = encoded.rsplit_once('@')?;
    if base_name.is_empty() {
        return None;
    }
    let name = if is_well_known_symbol_name(base_name) {
        format!("Symbol.{base_name}")
    } else {
        base_name.to_string()
    };
    Some(format!("[{name}]"))
}

fn is_well_known_symbol_name(name: &str) -> bool {
    matches!(
        name,
        "asyncDispose"
            | "asyncIterator"
            | "dispose"
            | "hasInstance"
            | "isConcatSpreadable"
            | "iterator"
            | "match"
            | "matchAll"
            | "replace"
            | "search"
            | "species"
            | "split"
            | "toPrimitive"
            | "toStringTag"
            | "unscopables"
    )
}

fn array_is_homogeneous<T>(array: &[T], comparer: impl Fn(&T, &T) -> bool) -> bool {
    if array.len() < 2 {
        return true;
    }
    let first = &array[0];
    for target in array.iter().skip(1) {
        if !comparer(first, target) {
            return false;
        }
    }
    true
}

fn types_are_same_reference<'a, 'state>(
    checker: &Checker<'a, 'state>,
    a: TypeHandle,
    b: TypeHandle,
) -> bool {
    a == b
        || checker.type_symbol_identity(a).is_some()
            && checker.type_symbol_identity(a) == checker.type_symbol_identity(b)
        || match (checker.type_alias_record(a), checker.type_alias_record(b)) {
            (Some(a_alias), Some(b_alias)) => {
                a_alias.symbol == b_alias.symbol && a_alias.type_arguments == b_alias.type_arguments
            }
            _ => false,
        }
}
